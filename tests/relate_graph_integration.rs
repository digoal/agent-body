use std::path::PathBuf;
use std::sync::Arc;

use agent_body::config::Config;
use agent_body::links::impact::ImpactAnalyzer;
use agent_body::links::relate_bridge::RelateGraphBridge;
use agent_body::links::LinksEngine;
use agent_body::storage::models::{AssetRecord, FactEdgeRecord, FactNodeRecord};
use agent_body::storage::Storage;
use ext_graph_relate::{RelateGraphConfig, RelateGraphEngine};
use serde_json::json;
use tempfile::TempDir;

#[test]
fn test_config_fallback_when_relate_disabled_or_library_missing() {
    // 1. 默认关闭时 try_open_relate_graph 应直接返回 Ok(None)
    let mut cfg = Config::default();
    assert!(!cfg.knowledge_graph.relate_enabled);
    assert!(cfg.try_open_relate_graph().expect("disabled ok").is_none());

    // 2. 开启但强制指定无效 RELATE_LIB 且宿主机库不走回退时的行为测试（仅当无默认库或单独构造失败场景）
    cfg.knowledge_graph.relate_enabled = true;
    cfg.knowledge_graph.relate_fallback_to_sqlite = true;
    // 指定不存在的 db 父路径下的只读库必定失败 -> fallback_to_sqlite=true 应返回 Ok(None) 而不报错
    let tmp = TempDir::new().expect("tempdir");
    cfg.knowledge_graph.relate_db_path = Some(tmp.path().join("nonexistent_readonly_dir"));
    let mut ro_cfg = cfg.resolve_relate_config();
    ro_cfg.read_only = true;
    assert!(RelateGraphEngine::open(ro_cfg).is_err());

    // 通过设置一个不可打开的 db_path（指向已存在的普通文件作为目录或无效只读路径）验证降级分支
    let bad_file = tmp.path().join("not_a_dir");
    std::fs::write(&bad_file, b"plain").expect("write file");
    cfg.knowledge_graph.relate_db_path = Some(bad_file.join("nested").join("graph.rl"));
    assert!(cfg.try_open_relate_graph().expect("fallback ok").is_none());

    cfg.knowledge_graph.relate_fallback_to_sqlite = false;
    assert!(cfg.try_open_relate_graph().is_err());
}

#[test]
fn test_storage_to_relate_5hop_impact_shortest_path_and_semantic_filter() {
    if !RelateGraphEngine::probe_library(None).available {
        eprintln!("⚠️ 跳过需 librelate_ffi.so 的集成测试");
        return;
    }

    let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
    let links_engine = LinksEngine::new(storage.clone());

    // 注册 5 跳依赖链节点（资产 + 属性图谱节点）:
    // n5 -> n4 -> n3 -> n2 -> n1 -> core_target
    // 且增加一条 L3_SEMANTIC 边: sem_peer -> core_target
    for path in [
        "/root/new/shared_proj/core_target.rs",
        "/root/new/work/n1.rs",
        "/root/new/work/n2.rs",
        "/root/new/work/n3.rs",
        "/root/new/work/n4.rs",
        "/root/new/cron/n5.sh",
    ] {
        storage
            .upsert_asset(&AssetRecord {
                id: path.to_string(),
                path: path.to_string(),
                asset_type: "script".to_string(),
                owner: "digoal".to_string(),
                security_level: "internal".to_string(),
                scan_tier: "deep".to_string(),
                description: Some(format!("asset {}", path)),
                desc_kv: json!({}),
                usage: None,
                size_bytes: Some(128),
                file_count: Some(1),
                tags: vec!["test".to_string()],
                status: "active".to_string(),
            })
            .expect("upsert_asset");
    }

    // 第 1~3 跳走显式 links 表
    links_engine
        .create_link(
            "asset",
            "/root/new/work/n1.rs",
            "asset",
            "/root/new/shared_proj/core_target.rs",
            "depends_on",
            0.95,
        )
        .unwrap();
    links_engine
        .create_link(
            "asset",
            "/root/new/work/n2.rs",
            "asset",
            "/root/new/work/n1.rs",
            "depends_on",
            0.90,
        )
        .unwrap();
    links_engine
        .create_link(
            "asset",
            "/root/new/work/n3.rs",
            "asset",
            "/root/new/work/n2.rs",
            "depends_on",
            0.85,
        )
        .unwrap();

    // 第 4~5 跳走 fact_nodes + fact_edges（混合 L2 结构边和 L3 语义边）
    for id in [
        "/root/new/shared_proj/core_target.rs",
        "/root/new/work/n3.rs",
        "/root/new/work/n4.rs",
        "/root/new/cron/n5.sh",
        "project:sem_peer",
    ] {
        storage
            .upsert_fact_node(&FactNodeRecord {
                id: id.to_string(),
                node_type: if id.starts_with("project:") {
                    "project".to_string()
                } else {
                    "file".to_string()
                },
                name: id.to_string(),
                label: id.to_string(),
                properties: json!({}),
                updated_at: None,
            })
            .unwrap();
    }
    storage
        .upsert_fact_edge(&FactEdgeRecord {
            id: None,
            source_id: "/root/new/work/n4.rs".to_string(),
            target_id: "/root/new/work/n3.rs".to_string(),
            relation: "calls".to_string(),
            properties: json!({"reason": "hop 4"}),
            confidence: 0.80,
            evidence_tier: "L2_STRUCTURAL".to_string(),
            weight: 0.80,
            direction_kind: "forward_dependency".to_string(),
            updated_at: None,
        })
        .unwrap();
    storage
        .upsert_fact_edge(&FactEdgeRecord {
            id: None,
            source_id: "/root/new/cron/n5.sh".to_string(),
            target_id: "/root/new/work/n4.rs".to_string(),
            relation: "scheduled_by".to_string(),
            properties: json!({"reason": "hop 5"}),
            confidence: 0.75,
            evidence_tier: "L2_STRUCTURAL".to_string(),
            weight: 0.75,
            direction_kind: "reverse_schedule".to_string(),
            updated_at: None,
        })
        .unwrap();
    storage
        .upsert_fact_edge(&FactEdgeRecord {
            id: None,
            source_id: "project:sem_peer".to_string(),
            target_id: "/root/new/shared_proj/core_target.rs".to_string(),
            relation: "semantically_related".to_string(),
            properties: json!({"reason": "L3 weak link"}),
            confidence: 0.45,
            evidence_tier: "L3_SEMANTIC".to_string(),
            weight: 0.45,
            direction_kind: "symmetric_affinity".to_string(),
            updated_at: None,
        })
        .unwrap();

    // 同步到临时 Relate 图库
    let tmp = TempDir::new().expect("tempdir");
    let relate_db: PathBuf = tmp.path().join("test_graph.rl");
    let engine = RelateGraphEngine::open(RelateGraphConfig {
        db_path: relate_db.to_string_lossy().into_owned(),
        timeout_ms: 5000,
        ..Default::default()
    })
    .expect("open relate");

    let sync_stats = RelateGraphBridge::sync_from_storage(&storage, &engine).expect("sync");
    assert!(sync_stats.nodes_upserted >= 7);
    assert!(sync_stats.edges_upserted >= 6);

    let analyzer = ImpactAnalyzer::new(storage);

    // 1. 严格模式（排除 L3_SEMANTIC，min_weight=0.50，max_depth=5）
    let strict = analyzer
        .analyze_impact_with_relate(
            "/root/new/shared_proj/core_target.rs",
            5,
            0.50,
            false,
            &engine,
        )
        .expect("strict relate impact");
    assert!(strict.summary.contains("[engine: relate-ffi]"));
    assert_eq!(strict.risk_level, "CRITICAL");
    assert_eq!(strict.direct_dependents.len(), 1);
    assert_eq!(strict.direct_dependents[0].node_id, "/root/new/work/n1.rs");
    assert_eq!(strict.indirect_dependents.len(), 4);
    assert!(strict
        .indirect_dependents
        .iter()
        .any(|d| d.node_id == "/root/new/cron/n5.sh" && d.depth == 5));
    assert!(!strict
        .direct_dependents
        .iter()
        .any(|d| d.node_id == "project:sem_peer"));

    // 验证跳数衰减单调性（depth 1 > depth 2 > depth 3 > depth 4 > depth 5）
    let d1 = strict.direct_dependents[0].effective_score;
    let d2 = strict
        .indirect_dependents
        .iter()
        .find(|d| d.depth == 2)
        .unwrap()
        .effective_score;
    let d5 = strict
        .indirect_dependents
        .iter()
        .find(|d| d.depth == 5)
        .unwrap()
        .effective_score;
    assert!(d1 > d2 && d2 > d5);

    // 2. 开启 L3_SEMANTIC 并降低阈值后，sem_peer 应命中
    let full = analyzer
        .analyze_impact_with_relate(
            "/root/new/shared_proj/core_target.rs",
            5,
            0.35,
            true,
            &engine,
        )
        .expect("full relate impact");
    assert!(full
        .direct_dependents
        .iter()
        .any(|d| d.node_id == "project:sem_peer"));

    // 3. 最短路径查询（5 跳）
    let sp = engine
        .shortest_path(
            "/root/new/cron/n5.sh",
            "/root/new/shared_proj/core_target.rs",
            6,
        )
        .expect("shortest_path");
    let sp_obj = sp.expect("path must exist");
    assert_eq!(sp_obj["hop_count"], 5);
}
