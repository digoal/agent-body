use std::path::Path;

use agent_body_kernel::KernelRuntime;
use agent_body_sdk::{
    ClientAuthContext, EvidenceTuple, IndexAmRoutine, IndexScanDesc, OpStrategy, TransportKind,
};
use serde_json::{Map, Value};
use tempfile::TempDir;

use ext_graph_relate::{
    ensure_readonly_cypher, resolve_library_candidates, GraphEntityNode, GraphRelateExtension,
    GraphRelationEdge, RelateGraphConfig, RelateGraphEngine,
};

fn has_relate_lib() -> bool {
    RelateGraphEngine::probe_library(None).available
}

#[test]
fn test_library_candidate_resolution_and_missing_fallback() {
    let cands = resolve_library_candidates(Some(Path::new("/tmp/custom_librelate_ffi.so")));
    assert_eq!(cands[0], Path::new("/tmp/custom_librelate_ffi.so"));
    assert!(cands
        .iter()
        .any(|p| p == Path::new("/usr/local/lib/librelate_ffi.so")));

    // 显式指定不存在的库路径时，若其它候选也找不到（测试纯降级构造函数）
    let probe = RelateGraphEngine::probe_library(Some(Path::new("/nonexistent/librelate.so")));
    // 注意：若宿主机已安装 /usr/local/lib/librelate_ffi.so，probe 会自动回退到系统候选并成功
    if Path::new("/usr/local/lib/librelate_ffi.so").exists() {
        assert!(probe.available);
        assert_eq!(probe.abi_version, Some(2));
    }
}

#[test]
fn test_readonly_cypher_guard_rejects_mutations_and_allows_reads() {
    assert!(ensure_readonly_cypher("MATCH (n:Entity) RETURN n LIMIT 10").is_ok());
    assert!(ensure_readonly_cypher(
        "/* CREATE fake */ MATCH (n:Entity {label: 'MERGE DELETE'}) RETURN n.id // SET x=1"
    )
    .is_ok());

    for bad in [
        "CREATE (n:Entity {id: 'x'})",
        "MATCH (n) MERGE (m:Entity {id: 'y'})",
        "MATCH (n) SET n.x = 1",
        "MATCH (n) DETACH DELETE n",
        "MATCH (n) REMOVE n.x",
        "DROP INDEX entity_id_idx",
        "LOAD CSV FROM 'file:///etc/passwd' AS row RETURN row",
    ] {
        assert!(
            ensure_readonly_cypher(bad).is_err(),
            "应当拒绝写 Cypher: {}",
            bad
        );
    }
}

#[test]
fn test_relate_engine_full_lifecycle_and_kernel_extension() {
    if !has_relate_lib() {
        eprintln!("⚠️ 跳过需 librelate_ffi.so 的 live 测试（尚未安装至 /usr/local/lib/librelate_ffi.so）");
        return;
    }

    // 1. 探测 ABI == 2
    let probe = RelateGraphEngine::probe_library(None);
    assert!(probe.available);
    assert_eq!(probe.abi_version, Some(2));

    // 2. :memory: 打开、参数化 CRUD 与 Typed Value Walker（标量/列表/映射/节点/关系/路径）
    let mem = RelateGraphEngine::open_in_memory(None).expect("open_in_memory");
    let mut params = Map::new();
    params.insert("name".to_string(), Value::String("Alice".to_string()));
    params.insert("age".to_string(), Value::Number(30.into()));
    params.insert(
        "tags".to_string(),
        Value::Array(vec![
            Value::String("pg".to_string()),
            Value::String("rust".to_string()),
        ]),
    );
    let mut meta_map = Map::new();
    meta_map.insert("score".to_string(), Value::from(0.95));
    params.insert("meta".to_string(), Value::Object(meta_map));

    let res = mem
        .query(
            "CREATE (a:Person {name: $name, age: $age, tags: $tags, score: $meta.score}) \
             CREATE (b:Person {name: 'Bob'}) \
             CREATE (a)-[r:KNOWS {since: 2024}]->(b) \
             RETURN a, r, $tags AS tags_echo",
            &params,
        )
        .expect("create & return");
    assert_eq!(res.row_count, 1);
    assert_eq!(res.rows[0]["a"]["labels"][0], "Person");
    assert_eq!(res.rows[0]["a"]["properties"]["name"], "Alice");
    assert_eq!(res.rows[0]["a"]["properties"]["age"], 30);
    assert_eq!(res.rows[0]["r"]["type"], "KNOWS");
    assert_eq!(res.rows[0]["r"]["properties"]["since"], 2024);
    assert_eq!(res.rows[0]["tags_echo"][1], "rust");

    // 路径返回校验
    let path_res = mem
        .query_readonly(
            "MATCH p = (a:Person {name: 'Alice'})-[:KNOWS]->(b:Person) RETURN p",
            &Map::new(),
        )
        .expect("path query");
    assert_eq!(path_res.row_count, 1);
    assert_eq!(
        path_res.rows[0]["p"]["elements"]
            .as_array()
            .map(|a| a.len()),
        Some(3)
    );

    // 3. 只读护栏拒绝写查询
    assert!(mem
        .query_readonly("CREATE (x:Tmp {id: 1})", &Map::new())
        .is_err());

    // 3.5 statement timeout 超时测试与连接恢复
    let timeout_eng = RelateGraphEngine::open(RelateGraphConfig {
        db_path: ":memory:".to_string(),
        timeout_ms: 1,
        ..Default::default()
    })
    .expect("open timeout engine");
    let timeout_err = timeout_eng.query_readonly(
        "UNWIND range(1, 50000) AS a UNWIND range(1, 50000) AS b RETURN count(*) AS c",
        &Map::new(),
    );
    assert!(
        timeout_err.is_err(),
        "1ms 超时应当中止大笛卡尔积查询"
    );
    let recovered = timeout_eng
        .query_readonly("RETURN 1 AS ok", &Map::new())
        .expect("超时后连接应保持可用");
    assert_eq!(recovered.rows[0]["ok"], 1);

    // 4. 显式事务 commit (bookmark) & rollback
    mem.begin_tx().expect("begin");
    mem.query("CREATE (:TxTest {k: 'rolled_back'})", &Map::new())
        .expect("tx write");
    mem.rollback_tx().expect("rollback");
    let rb_check = mem
        .query_readonly("MATCH (n:TxTest) RETURN count(n) AS c", &Map::new())
        .expect("check rollback");
    assert_eq!(rb_check.rows[0]["c"], 0);

    let bm = mem
        .execute_tx(&[(
            "CREATE (:TxTest {k: $k})",
            Map::from_iter([("k".to_string(), Value::String("committed".to_string()))]),
        )])
        .expect("execute_tx");
    assert!(bm.as_deref().unwrap_or("").starts_with("relate:"));
    let cm_check = mem
        .query_readonly("MATCH (n:TxTest) RETURN count(n) AS c", &Map::new())
        .expect("check commit");
    assert_eq!(cm_check.rows[0]["c"], 1);

    // 5. 持久化打开 + sync_graph + analyze_impact + shortest_path
    let tmp = TempDir::new().expect("tempdir");
    let db_dir = tmp.path().join("graph_store");
    let cfg = RelateGraphConfig {
        db_path: db_dir.to_string_lossy().into_owned(),
        timeout_ms: 3000,
        ..Default::default()
    };
    {
        let eng = RelateGraphEngine::open(cfg.clone()).expect("open persistent");
        let nodes = vec![
            GraphEntityNode {
                id: "core".to_string(),
                node_type: "project".to_string(),
                name: "core".to_string(),
                label: "core".to_string(),
                properties_json: "{}".to_string(),
            },
            GraphEntityNode {
                id: "svc_a".to_string(),
                node_type: "service".to_string(),
                name: "svc_a".to_string(),
                label: "svc_a".to_string(),
                properties_json: "{}".to_string(),
            },
            GraphEntityNode {
                id: "cron_b".to_string(),
                node_type: "cron".to_string(),
                name: "cron_b".to_string(),
                label: "cron_b".to_string(),
                properties_json: "{}".to_string(),
            },
        ];
        let edges = vec![
            GraphRelationEdge {
                source_id: "svc_a".to_string(),
                source_type: "service".to_string(),
                target_id: "core".to_string(),
                relation: "depends_on".to_string(),
                weight: 0.9,
                evidence_tier: "L1_EXPLICIT".to_string(),
                line_number: Some(12),
                snippet: None,
                provenance: Some("test".to_string()),
            },
            GraphRelationEdge {
                source_id: "cron_b".to_string(),
                source_type: "cron".to_string(),
                target_id: "svc_a".to_string(),
                relation: "depends_on".to_string(),
                weight: 0.8,
                evidence_tier: "L2_STRUCTURAL".to_string(),
                line_number: None,
                snippet: None,
                provenance: Some("test".to_string()),
            },
        ];
        let stats = eng.sync_graph(&nodes, &edges).expect("sync_graph");
        assert_eq!(stats.nodes_upserted, 3);
        assert_eq!(stats.edges_upserted, 2);

        let hits = eng.analyze_impact("core", 3, 0.5, false).expect("impact");
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].node_id, "svc_a");
        assert_eq!(hits[0].depth, 1);
        assert_eq!(hits[1].node_id, "cron_b");
        assert_eq!(hits[1].depth, 2);

        let sp = eng.shortest_path("cron_b", "core", 4).expect("shortest_path");
        assert!(sp.is_some());
    }

    // 重开持久化库确认落盘不丢
    let reopened = RelateGraphEngine::open(cfg).expect("reopen");
    let st = reopened.status().expect("status");
    assert_eq!(st.node_count, 3);
    assert_eq!(st.edge_count, 2);

    // 6. KernelRuntime 挂载 GraphRelateExtension (IndexAmRoutine + 3 ProcRoutine)
    let mut kernel = KernelRuntime::open_sqlite_in_memory(None).expect("kernel");
    let ext = GraphRelateExtension::new(RelateGraphConfig::default());
    let index_am = ext.index_am();
    kernel
        .install_extensions(&[Box::new(ext)])
        .expect("install_extensions");

    let t1 = EvidenceTuple {
        evidence_id: "ev-alpha".to_string(),
        host_id: "test-host".to_string(),
        host_labels: Default::default(),
        entity_type: "asset".to_string(),
        entity_id: "node_alpha".to_string(),
        source_ext: "graph_relate".to_string(),
        evidence_tier: "L1_EXPLICIT".to_string(),
        collected_at: "2026-11-28T20:00:00+08:00".to_string(),
        ttl_sec: 3600,
        payload: serde_json::json!({"label": "alpha asset"}),
        redacted: false,
        evidence_sha256: String::new(),
    }
    .with_computed_sha256();
    let t2 = EvidenceTuple {
        evidence_id: "ev-beta".to_string(),
        host_id: "test-host".to_string(),
        host_labels: Default::default(),
        entity_type: "asset".to_string(),
        entity_id: "node_beta".to_string(),
        source_ext: "graph_relate".to_string(),
        evidence_tier: "L1_EXPLICIT".to_string(),
        collected_at: "2026-11-28T20:00:00+08:00".to_string(),
        ttl_sec: 3600,
        payload: serde_json::json!({"label": "beta asset"}),
        redacted: false,
        evidence_sha256: String::new(),
    }
    .with_computed_sha256();
    index_am.aminsert(&t1).expect("aminsert 1");
    index_am.aminsert(&t2).expect("aminsert 2");
    index_am
        .link_entities("node_alpha", "node_beta", "depends_on", 0.95)
        .expect("link");

    let scan = IndexScanDesc {
        query_text: "node_alpha".to_string(),
        strategy_num: OpStrategy::GraphHop.strategy_num(),
        scalar_filters: Default::default(),
        rls_filter: None,
        limit: 10,
    };
    let est = index_am.amcostestimate(&scan);
    assert!(est.available);
    let hits = index_am
        .amgetmulti(&scan, kernel.table_am().as_ref())
        .expect("amgetmulti");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].entity_id, "node_beta");

    // 验证 3 个 ProcRoutine 已通过 KernelDispatcher 注册并可通过 CLI/MCP 调用
    let auth = ClientAuthContext {
        transport: TransportKind::Cli,
        cert: None,
        bearer_token: None,
        method: "cli".to_string(),
        path: "graph relate status".to_string(),
        is_loopback: true,
        is_read_only: true,
    };
    let status_out = kernel
        .handle_cli(
            &["graph".into(), "relate".into(), "status".into()],
            Some(serde_json::json!({})),
            &auth,
        )
        .expect("cli status proc");
    assert_eq!(status_out.data["available"], true);

    let mcp_list = kernel.handle_mcp(
        &serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
        &auth,
    );
    let tools = mcp_list["result"]["tools"].as_array().expect("tools array");
    for proc_name in [
        "ext_graph_relate_status",
        "ext_graph_relate_query",
        "ext_graph_relate_impact",
    ] {
        assert!(
            tools.iter().any(|t| t["name"] == proc_name),
            "缺少 ProcRoutine: {}",
            proc_name
        );
    }
}
