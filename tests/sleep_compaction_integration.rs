//! Phase 77 (M2 · 方案 A 满血形态) 全链路集成与 PostgreSQL 18.6 睡眠整理压测套件
//!
//! 覆盖：
//! 1. 真机 PG 18.6 隔离库（RAII Drop 强制 `DROP DATABASE ... WITH (FORCE)` 零残留）：
//!    - `rescore_salience` 显著性分层 + `dry_run` 预览 + `sleep_context` 作业 `committed`
//!    - 压缩比 >= 3x 且关键事实 100% 保留 + 0 明文密钥泄漏
//!    - `QueryPredictabilityGate` 常规查询命中摘要 vs 不可预测探索诊断查询自动旁路原始事件流 (`ΔRecall >= 0`)
//!    - `tombstone_fact` 墓碑化保持 0% 僵尸召回（含旁路直查路径）
//!    - 异常中断 `sleep_context(running)` 作业恢复 (`aborted`) + 热路径 P99 < 20ms 不劣化
//! 2. 无 PG 离线 SQLite 降级完整闭环 + `pause`/`resume` + `OUT_OF_AGENT` 审计 + `PR-15` 记账
//! 3. 43 MCP 工具零膨胀 (`body_memory_audit` `sleep_action`) + Web `GET` 只读 / `POST` 鉴权控制

use std::sync::Arc;
use std::time::Instant;

use agent_body::api::mcp_tools::McpToolRegistry;
use agent_body::config::Config;
use agent_body::memory::adapters::pg_tx::{PgTxAdapterConfig, PgTxMemoryAdapter};
use agent_body::memory::spi::{TxEvidenceTier, TxMemoryProvider, TxMemoryWriteInput, TxSearchOptions};
use agent_body::memory::SleepCompactionOrchestrator;
use agent_body::storage::models::MemoryRecord;
use agent_body::storage::Storage;
use agent_body::web::http::{HttpRequest, Method};
use agent_body::web::WebServer;
use ext_mem_pg_tx::SleepCompactionConfig;
use postgres::{Client, NoTls};

const ADMIN_PG_CONN: &str = "host=127.0.0.1 port=5432 user=postgres dbname=postgres connect_timeout=2";

struct TempPgDatabase {
    db_name: String,
    pg_url: String,
}

impl TempPgDatabase {
    fn try_create() -> Option<Self> {
        let admin_url = std::env::var("AGENT_BODY_TEST_PG_ADMIN_URL")
            .unwrap_or_else(|_| ADMIN_PG_CONN.to_string());
        let mut admin = match Client::connect(&admin_url, NoTls) {
            Ok(c) => c,
            Err(err) => {
                eprintln!("[sleep_test] PostgreSQL 不可达 ({})，跳过 PG 18 用例", err);
                return None;
            }
        };
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let db_name = format!("ab_pgtx_s77_{}_{}", std::process::id(), &suffix[..8]);
        admin
            .batch_execute(&format!("CREATE DATABASE {}", db_name))
            .expect("create isolated pg db for Phase 77");
        let pg_url = format!(
            "host=127.0.0.1 port=5432 user=postgres dbname={} connect_timeout=5",
            db_name
        );
        Some(Self { db_name, pg_url })
    }
}

impl Drop for TempPgDatabase {
    fn drop(&mut self) {
        let admin_url = std::env::var("AGENT_BODY_TEST_PG_ADMIN_URL")
            .unwrap_or_else(|_| ADMIN_PG_CONN.to_string());
        if let Ok(mut admin) = Client::connect(&admin_url, NoTls) {
            let _ = admin.batch_execute(&format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", self.db_name));
        }
    }
}

#[test]
fn sleep_compaction_live_pg18_end_to_end() {
    let Some(temp_db) = TempPgDatabase::try_create() else {
        return;
    };

    let storage = Storage::open_in_memory().expect("open sqlite");
    let adapter = PgTxMemoryAdapter::connect_or_fallback(
        PgTxAdapterConfig {
            pg_url: temp_db.pg_url.clone(),
            pool_size: 8,
            tenant_scope: "sleep_tenant".to_string(),
            vector_backend: "none".to_string(),
            fallback_to_sqlite: false,
            mirror_writes_to_sqlite: false,
        },
        storage,
    )
    .expect("connect live PG 18");
    let engine = adapter.engine().expect("live pg engine");

    // 1. 写入 1 个高价值决策 + 6 条高冗余周期情景日志 + 1 条待墓碑敏感记录
    adapter
        .append_tx_event(&TxMemoryWriteInput {
            body_id: "body-s77".to_string(),
            tenant_scope: "sleep_tenant".to_string(),
            fact_key: "arch:pool_rule".to_string(),
            memory_type: "decision".to_string(),
            content: "Keep PG MVCC pool_size=8 and disallow long 2PL locks across inference".to_string(),
            summary: Some("PG MVCC pool decision".to_string()),
            tags: vec!["architecture".to_string()],
            evidence_tier: TxEvidenceTier::L1Explicit,
            salience_score: 0.9,
            expected_epoch: None,
            source_agent: Some("architect".to_string()),
            source_session: None,
            embedding_ref: None,
        })
        .expect("write decision");

    let secret = "sk-proj-SleepLivePgSecret99887766554433";
    let episodic_samples = [
        (
            "auto_sync:ep1",
            "auto_sync: Checked host status and everything looks healthy during periodic patrol round 101 with verbose filler narrative repeated across many idle ticks. PostgreSQL primary port=5432 max_connections=250.",
        ),
        (
            "auto_sync:ep2",
            "routine log: Checked host status and everything looks healthy during periodic patrol round 102 with verbose filler narrative repeated across many idle ticks. Standby replica lag_ms<=12.",
        ),
        (
            "auto_sync:ep3",
            "session check: No anomaly detected during routine verification pass 103 when scanning background services and checking runtime telemetry. Must enforce max_wal_size=4GB.",
        ),
        (
            "auto_sync:ep4",
            "periodic note: No anomaly detected during routine verification pass 104 when scanning background services and checking runtime telemetry. Secret guard key=sk-proj-SleepLivePgSecret99887766554433 redacted.",
        ),
        (
            "auto_sync:ep5",
            "auto_sync: Routine telemetry healthy check pass 105 with extended operational commentary on host health status. Unique trace PgTxEngine::search_raw_bypass SQLSTATE 40001 in /src/memory/spi.rs.",
        ),
        (
            "auto_sync:ep6",
            "routine log: Routine telemetry healthy check pass 106 with extended operational commentary on host health status. Checkpoint timeout=15min verified.",
        ),
    ];

    for (idx, (fk, body)) in episodic_samples.iter().enumerate() {
        adapter
            .append_tx_event(&TxMemoryWriteInput {
                body_id: "body-s77".to_string(),
                tenant_scope: "sleep_tenant".to_string(),
                fact_key: (*fk).to_string(),
                memory_type: "note".to_string(),
                content: (*body).to_string(),
                summary: Some(format!("episodic {}", idx + 1)),
                tags: vec!["pg".to_string(), "auto_sync".to_string()],
                evidence_tier: TxEvidenceTier::L3Semantic,
                salience_score: 0.4,
                expected_epoch: None,
                source_agent: Some("hook".to_string()),
                source_session: Some("sess-sleep".to_string()),
                embedding_ref: None,
            })
            .expect("append episodic event");
    }

    adapter
        .append_tx_event(&TxMemoryWriteInput {
            body_id: "body-s77".to_string(),
            tenant_scope: "sleep_tenant".to_string(),
            fact_key: "tomb:obsolete_port".to_string(),
            memory_type: "fact".to_string(),
            content: "Obsolete legacy cluster token=ObsoletePort9191".to_string(),
            summary: None,
            tags: vec!["pg".to_string()],
            evidence_tier: TxEvidenceTier::L3Semantic,
            salience_score: 0.4,
            expected_epoch: None,
            source_agent: Some("test".to_string()),
            source_session: None,
            embedding_ref: None,
        })
        .expect("append obsolete event");
    adapter
        .tombstone_fact("body-s77", "sleep_tenant", "tomb:obsolete_port", "obsolete")
        .expect("tombstone obsolete fact");

    // 2. 注入一条模拟崩溃遗留的 running 睡眠整理作业，验证启动自恢复为 aborted
    {
        let mut raw_pg = Client::connect(&temp_db.pg_url, NoTls).expect("direct pg");
        raw_pg
            .execute(
                "INSERT INTO ext_mem_pg_tx__compaction_jobs
                    (job_kind, tenant_scope, from_epoch, to_epoch, state, audit_trace_id)
                 VALUES ('sleep_context', 'sleep_tenant', 1, 2, 'running', 'crashed-prev-run')",
                &[],
            )
            .expect("seed interrupted running job");
    }

    // 3. dry_run 预览不得落盘 committed 作业
    let preview = engine
        .run_sleep_compaction(
            "body-s77",
            "sleep_tenant",
            &SleepCompactionConfig {
                min_events: 3,
                max_batch: 64,
                salience_ceiling: 0.75,
                dry_run: true,
                audit_trace_id: "sleep-dry".to_string(),
            },
            agent_body::memory::sleep_compaction::sanitize_sleep_summary_text,
        )
        .expect("dry_run sleep compaction");
    assert!(preview.dry_run);
    assert!(preview.events_compacted >= 6);
    assert!(preview.compression_ratio >= 2.0, "preview ratio={}", preview.compression_ratio);

    // 4. 真实落盘执行睡眠整理
    let committed = engine
        .run_sleep_compaction(
            "body-s77",
            "sleep_tenant",
            &SleepCompactionConfig {
                min_events: 3,
                max_batch: 64,
                salience_ceiling: 0.75,
                dry_run: false,
                audit_trace_id: "sleep-commit-1".to_string(),
            },
            agent_body::memory::sleep_compaction::sanitize_sleep_summary_text,
        )
        .expect("commit sleep compaction");
    assert_eq!(committed.state, "committed");
    assert!(committed.job_id.is_some());
    assert!(
        committed.compression_ratio >= 3.0,
        "睡眠整理压缩比应 >= 3.0x，实测: {:.2}x ({} -> {} tok)",
        committed.compression_ratio,
        committed.raw_tokens,
        committed.compacted_tokens
    );

    let jobs = engine
        .list_compaction_jobs("sleep_tenant", 10)
        .expect("list compaction jobs");
    assert!(
        jobs.iter()
            .any(|j| j.audit_trace_id == "crashed-prev-run" && j.state == "aborted"),
        "残留 running 作业应已恢复标记为 aborted: {:?}",
        jobs
    );
    assert!(
        jobs.iter()
            .any(|j| j.audit_trace_id == "sleep-commit-1" && j.state == "committed")
    );

    // 5. 常规可预测查询命中高密度摘要且核心数字 100% 保留、明文密钥 0 泄露
    let norm_snap = adapter
        .search_tx(
            "port=5432 max_wal_size=4GB timeout=15min",
            &TxSearchOptions {
                tenant_scope: "sleep_tenant".to_string(),
                limit: 5,
                as_of_epoch: None,
                include_history: false,
            },
        )
        .expect("routine query after sleep compaction");
    assert!(!norm_snap.hits.is_empty());
    assert_eq!(norm_snap.hits[0].fact_key, "sleep_summary:pg");
    assert!(norm_snap.hits[0].content.contains("port=5432"));
    assert!(norm_snap.hits[0].content.contains("max_wal_size=4GB"));
    assert!(norm_snap.hits[0].content.contains("lag_ms<=12"));
    assert!(norm_snap.hits[0].content.contains("timeout=15min"));
    assert!(!norm_snap.hits[0].content.contains(secret));

    // 6. 不可预测探索型诊断查询自动旁路摘要直查原始事件流，且墓碑事件保持 0% 僵尸召回
    let bypass_snap = adapter
        .search_tx(
            "PgTxEngine::search_raw_bypass SQLSTATE 40001 /src/memory/spi.rs",
            &TxSearchOptions {
                tenant_scope: "sleep_tenant".to_string(),
                limit: 5,
                as_of_epoch: None,
                include_history: false,
            },
        )
        .expect("unpredictable bypass search");
    assert!(bypass_snap.hits.iter().any(|h| h.fact_key == "auto_sync:ep5"));

    let zombie_check = adapter
        .search_tx(
            "PgTxEngine::ObsoletePort9191",
            &TxSearchOptions {
                tenant_scope: "sleep_tenant".to_string(),
                limit: 5,
                as_of_epoch: None,
                include_history: false,
            },
        )
        .expect("zombie bypass check");
    assert!(
        zombie_check.hits.iter().all(|h| h.fact_key != "tomb:obsolete_port"),
        "墓碑事实绝不能通过旁路复活"
    );

    // 7. 热路径检索 P99 延迟 < 20ms
    let mut lats = Vec::with_capacity(30);
    for _ in 0..30 {
        let qt = Instant::now();
        let res = adapter
            .search_tx(
                "port=5432 max_wal_size",
                &TxSearchOptions {
                    tenant_scope: "sleep_tenant".to_string(),
                    limit: 5,
                    as_of_epoch: None,
                    include_history: false,
                },
            )
            .expect("p99 probe");
        assert!(!res.hits.is_empty());
        lats.push(qt.elapsed().as_secs_f64() * 1000.0);
    }
    lats.sort_by(|a, b| a.total_cmp(b));
    let p50 = lats[lats.len() / 2];
    let p99 = lats[((lats.len() as f64 * 0.99).ceil() as usize).saturating_sub(1).min(lats.len() - 1)];
    println!(
        "\n⚡ [PG 18 M2睡眠整理实测] 压缩比={:.2}x ({}->{} tok), 保留事实={}, 热路径 P50={:.2}ms, P99={:.2}ms (SLA < 20ms)",
        committed.compression_ratio, committed.raw_tokens, committed.compacted_tokens, committed.preserved_fact_count, p50, p99
    );
    assert!(p99 < 20.0, "睡眠整理后热路径 P99 应 < 20ms，实测 {:.2}ms", p99);
}

#[test]
fn sleep_compaction_sqlite_fallback_pause_resume_and_audit() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut cfg = Config::default();
    cfg.data_dir = dir.path().to_path_buf();
    cfg.memory.provider = "builtin".to_string();
    cfg.memory.sleep_min_events = 3;
    cfg.memory.sleep_max_batch = 64;

    let storage = Storage::open_in_memory().expect("open sqlite");
    for i in 1..=5 {
        storage
            .insert_memory(&MemoryRecord {
                id: format!("auto_sync:local-{}", i),
                memory_type: "note".to_string(),
                content: format!(
                    "auto_sync: Checked host status and everything looks fine during periodic patrol #{}. Port 8765 must stay bound on loopback.",
                    i
                ),
                summary: Some("local episodic".to_string()),
                source_agent: Some("hook".to_string()),
                source_session: Some("sess-local".to_string()),
                tags: vec!["webui".to_string(), "auto_sync".to_string()],
                weight: 0.4,
                pinned: false,
                status: "active".to_string(),
                embedding: vec![],
            })
            .expect("insert sqlite episodic");
    }

    // 暂停时周期调度必须跳过；非 force execute 也必须返回 paused
    SleepCompactionOrchestrator::set_paused(&cfg.data_dir, true).expect("pause");
    assert!(SleepCompactionOrchestrator::is_paused(&cfg.data_dir));
    assert!(SleepCompactionOrchestrator::maybe_run_idle_cycle(&storage, &cfg).is_none());
    let paused_rep = SleepCompactionOrchestrator::execute(&storage, &cfg, false, false).expect("paused exec");
    assert_eq!(paused_rep.state, "paused");

    // 恢复后执行聚类压缩，原件归档为 archived，新增摘要保留关键事实，写审计 + PR-15
    SleepCompactionOrchestrator::set_paused(&cfg.data_dir, false).expect("resume");
    let run_rep = SleepCompactionOrchestrator::execute(&storage, &cfg, false, true).expect("run sqlite sleep");
    assert_eq!(run_rep.state, "committed");
    assert_eq!(run_rep.events_compacted, 5);
    assert!(run_rep.compression_ratio >= 2.0);

    let active = storage.list_all_active_memories(20).expect("active memories");
    assert!(active.iter().any(|m| m.id == "sleep-sum-webui" && m.content.contains("8765")));
    let all = storage.list_all_memories(20).expect("all memories");
    assert_eq!(all.iter().filter(|m| m.status == "archived").count(), 5);

    let ledger = storage
        .query_effectiveness_ledger("7d", Some("sleep_compaction"), 10)
        .expect("query PR-15 ledger");
    assert!(ledger.by_rule.iter().any(|r| r.provenance_rule == "PR-15"));
}

#[test]
fn sleep_compaction_mcp_web_parity_and_43_tools_zero_bloat() {
    let storage = Storage::open_in_memory().expect("open sqlite");
    let registry = McpToolRegistry::new(storage.clone());
    let tools = registry.list_tools();
    assert_eq!(tools.len(), 43, "43 原生 MCP 工具严格零膨胀");

    let audit_schema = tools
        .iter()
        .find(|t| t.get("name").and_then(|v| v.as_str()) == Some("body_memory_audit"))
        .expect("body_memory_audit");
    let props = &audit_schema["inputSchema"]["properties"];
    assert!(props.get("sleep_action").is_some());
    assert!(props.get("force").is_some());

    let mcp_out = registry
        .call_tool(
            "body_memory_audit",
            serde_json::json!({ "sleep_action": "dry_run" }),
        )
        .expect("mcp sleep dry_run");
    assert!(mcp_out.contains("sleep_compaction"));

    // Web GET /api/v1/memories/audit 只读带 sleep_compaction，POST 未携带鉴权拦截、携带合法 Token 放行
    let st_arc = Arc::clone(storage.provider());
    let start = Instant::now();
    let get_resp = WebServer::route_request_arc(
        &HttpRequest {
            method: Method::Get,
            path: "/api/v1/memories/audit".to_string(),
            raw_query: String::new(),
            query_params: Default::default(),
            headers: Default::default(),
            body: Vec::new(),
        },
        st_arc.clone(),
        &start,
        &Some("secret-tok-77".to_string()),
    );
    assert_eq!(get_resp.status.0, 200);
    let get_txt = String::from_utf8_lossy(&get_resp.body);
    assert!(get_txt.contains("sleep_compaction"));

    let unauth_post = WebServer::route_request_arc(
        &HttpRequest {
            method: Method::Post,
            path: "/api/v1/memories/audit".to_string(),
            raw_query: String::new(),
            query_params: Default::default(),
            headers: Default::default(),
            body: br#"{"sleep_action":"status"}"#.to_vec(),
        },
        st_arc.clone(),
        &start,
        &Some("secret-tok-77".to_string()),
    );
    assert_eq!(unauth_post.status.0, 401);

    let mut auth_headers = std::collections::HashMap::new();
    auth_headers.insert("authorization".to_string(), "Bearer secret-tok-77".to_string());
    let auth_post = WebServer::route_request_arc(
        &HttpRequest {
            method: Method::Post,
            path: "/api/v1/memories/audit".to_string(),
            raw_query: String::new(),
            query_params: Default::default(),
            headers: auth_headers,
            body: br#"{"sleep_action":"dry_run"}"#.to_vec(),
        },
        st_arc,
        &start,
        &Some("secret-tok-77".to_string()),
    );
    assert_eq!(auth_post.status.0, 200);
    assert!(String::from_utf8_lossy(&auth_post.body).contains("sleep_compaction"));
}
