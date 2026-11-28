//! Phase 76 (M1 · 方案 A 事务底座) 全链路集成与 PostgreSQL 18 并发压测套件
//!
//! 覆盖：
//! 1. 真机 PostgreSQL 18.6 隔离临时库（RAII Drop 强制清理零残留）：
//!    - 16 线程 × 5 次并发写热点 `fact_key` → 0% 丢失更新、严格单调 `tx_epoch`、0 重叠 `valid_range`
//!    - 双时态 `as_of_epoch` 时间旅行回放
//!    - 严格 CAS `expected_epoch` 冲突 → `MVCC_EPOCH_CONFLICT` → AOPCC `< 2ms` 秒级自愈处方
//!    - 写穿墓碑删除 (`tombstone_fact`) → 0% 僵尸分段/向量召回
//!    - 写前 `SecretRedactor` 脱敏防明文泄露
//!    - 解耦检索段 `search_tx` P99 延迟 `< 20ms` 实测
//!    - SQLite ⇄ PostgreSQL 双向热同步 + `ContinuityCapsule` 锚定真实 PG `tx_epoch`
//! 2. PostgreSQL 不可达透明 SQLite 降级 (`fallback_to_sqlite = true`) 与严格拒绝 (`false`)
//! 3. 历史 SQLite `body_checkpoints`（无 `tx_epoch` 列）无损自动迁移兼容
//! 4. 43 原生 MCP 工具零膨胀 + `body_memory_{add,search,audit}` 双时态参数端到端验证

use std::sync::Arc;
use std::thread;
use std::time::Instant;

use agent_body::api::mcp_tools::McpToolRegistry;
use agent_body::benchmark::provenance::estimate_tokens;
use agent_body::body::continuity::{ContinuityInput, ContinuityManager};
use agent_body::hooks::diagnoser::OptimisticFailureDiagnoser;
use agent_body::memory::adapters::pg_tx::{PgTxAdapterConfig, PgTxMemoryAdapter};
use agent_body::memory::spi::{TxEvidenceTier, TxMemoryProvider, TxMemoryWriteInput, TxSearchOptions};
use agent_body::storage::models::MemoryRecord;
use agent_body::storage::Storage;
use postgres::{Client, NoTls};

const ADMIN_PG_CONN: &str = "host=127.0.0.1 port=5432 user=postgres dbname=postgres connect_timeout=2";

/// 隔离临时 PostgreSQL 测试库，退出作用域时无条件 `DROP DATABASE ... WITH (FORCE)` 保障零残留
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
                eprintln!(
                    "[pg_tx_test] 本地 PostgreSQL 不可达 ({})，跳过真机 PG 18 压测用例",
                    err
                );
                return None;
            }
        };
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let db_name = format!(
            "ab_pgtx_t_{}_{}",
            std::process::id(),
            &suffix[..8]
        );
        let create_sql = format!("CREATE DATABASE {}", db_name);
        admin
            .batch_execute(&create_sql)
            .expect("create isolated temporary pg test database");
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
            let drop_sql = format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", self.db_name);
            let _ = admin.batch_execute(&drop_sql);
        }
    }
}

#[test]
fn pg_tx_live_pg18_16thread_mvcc_stress_and_p99() {
    let Some(temp_db) = TempPgDatabase::try_create() else {
        return;
    };

    let storage = Storage::open_in_memory().expect("open sqlite mirror");
    let cfg = PgTxAdapterConfig {
        pg_url: temp_db.pg_url.clone(),
        pool_size: 16,
        tenant_scope: "stress_tenant".to_string(),
        vector_backend: "none".to_string(),
        fallback_to_sqlite: false,
        mirror_writes_to_sqlite: false,
    };
    let adapter = Arc::new(
        PgTxMemoryAdapter::connect_or_fallback(cfg, storage.clone())
            .expect("connect live PostgreSQL 18 PgTxMemoryAdapter"),
    );
    assert!(adapter.is_pg_active(), "应直连本地 PostgreSQL 18 而非降级");

    // 1. 16 线程 × 5 次更新 = 80 并发写同一热点 fact_key → 0% 丢失更新
    let threads = 16usize;
    let writes_per_thread = 5usize;
    let total_expected = threads * writes_per_thread;
    let mut handles = Vec::with_capacity(threads);
    let stress_t0 = Instant::now();
    for tid in 0..threads {
        let ad = Arc::clone(&adapter);
        handles.push(thread::spawn(move || {
            let mut local_epochs = Vec::with_capacity(writes_per_thread);
            let mut local_retries = 0u32;
            for step in 0..writes_per_thread {
                let out = ad
                    .append_tx_event(&TxMemoryWriteInput {
                        body_id: "body-stress".to_string(),
                        tenant_scope: "stress_tenant".to_string(),
                        fact_key: "stress:hot_key".to_string(),
                        memory_type: "decision".to_string(),
                        content: format!(
                            "concurrent update from worker={} step={} marker=pg18_mvcc",
                            tid, step
                        ),
                        summary: Some(format!("worker={} step={}", tid, step)),
                        tags: vec!["stress".to_string(), format!("t{}", tid)],
                        evidence_tier: TxEvidenceTier::L1Explicit,
                        salience_score: 0.9,
                        expected_epoch: None,
                        source_agent: Some(format!("worker-{}", tid)),
                        source_session: Some("stress-session".to_string()),
                        embedding_ref: None,
                    })
                    .expect("concurrent append_tx_event must converge via OCC retry");
                local_epochs.push(out.tx_epoch);
                local_retries += out.retries;
            }
            (local_epochs, local_retries)
        }));
    }

    let mut all_epochs = Vec::with_capacity(total_expected);
    let mut total_occ_retries = 0u32;
    for h in handles {
        let (eps, retries) = h.join().expect("worker thread join");
        all_epochs.extend(eps);
        total_occ_retries += retries;
    }
    let stress_ms = stress_t0.elapsed().as_secs_f64() * 1000.0;
    all_epochs.sort_unstable();
    all_epochs.dedup();
    assert_eq!(
        all_epochs.len(),
        total_expected,
        "16 线程 × 5 并发写不得出现任何丢失更新或重复 tx_epoch"
    );

    // 校验历史版本数恰好 80 条、活跃事件恰好 1 条、活跃分段恰好 1 条、已删分段恰好 79 条、且 valid_range 零重叠
    let history = adapter
        .fact_history("body-stress", "stress_tenant", "stress:hot_key", 200)
        .expect("fact_history");
    assert_eq!(history.len(), total_expected);
    let active_in_hist = history.iter().filter(|e| e.valid_to.is_none()).count();
    assert_eq!(active_in_hist, 1, "同一 fact_key 有且仅有 1 条当前活跃版本");

    let mut raw_pg = Client::connect(&temp_db.pg_url, NoTls).expect("direct pg inspect");
    let overlap_cnt: i64 = raw_pg
        .query_one(
            "SELECT COUNT(*) FROM ext_mem_pg_tx__events a
             JOIN ext_mem_pg_tx__events b
               ON a.body_id = b.body_id
              AND a.tenant_scope = b.tenant_scope
              AND a.fact_key = b.fact_key
              AND a.event_id < b.event_id
             WHERE a.fact_key = 'stress:hot_key'
               AND a.valid_range && b.valid_range",
            &[],
        )
        .expect("check non-overlapping valid_range")
        .get(0);
    assert_eq!(overlap_cnt, 0, "双时态 valid_range 不允许任何区间重叠");

    let health_after_stress = adapter.tx_health().expect("tx_health");
    assert_eq!(health_after_stress.active_events, 1);
    assert_eq!(health_after_stress.active_segments, 1);
    assert_eq!(
        health_after_stress.deleted_segments,
        (total_expected - 1) as i64
    );

    // 2. 双时态 as_of_epoch 时间旅行 + 写前脱敏 + 严格 CAS 冲突触发 AOPCC <2ms 自愈 + 写穿墓碑 0% 僵尸召回
    let secret_raw = "sk-proj-LivePgSecret9876543210abcdef";
    let v1 = adapter
        .append_tx_event(&TxMemoryWriteInput {
            body_id: "body-stress".to_string(),
            tenant_scope: "stress_tenant".to_string(),
            fact_key: "deploy:pg_port".to_string(),
            memory_type: "fact".to_string(),
            content: format!("pg primary runs on port 5432 token={}", secret_raw),
            summary: Some("pg port v1=5432".to_string()),
            tags: vec!["deploy".to_string()],
            evidence_tier: TxEvidenceTier::L1Explicit,
            salience_score: 0.95,
            expected_epoch: None,
            source_agent: Some("agent-a".to_string()),
            source_session: Some("sess-1".to_string()),
            embedding_ref: None,
        })
        .expect("write v1");

    let v2 = adapter
        .append_tx_event(&TxMemoryWriteInput {
            body_id: "body-stress".to_string(),
            tenant_scope: "stress_tenant".to_string(),
            fact_key: "deploy:pg_port".to_string(),
            memory_type: "fact".to_string(),
            content: "pg primary migrated to port 5433".to_string(),
            summary: Some("pg port v2=5433".to_string()),
            tags: vec!["deploy".to_string()],
            evidence_tier: TxEvidenceTier::L1Explicit,
            salience_score: 0.98,
            expected_epoch: Some(v1.tx_epoch),
            source_agent: Some("agent-b".to_string()),
            source_session: Some("sess-2".to_string()),
            embedding_ref: None,
        })
        .expect("write v2 with valid CAS expected_epoch");
    assert!(v2.tx_epoch > v1.tx_epoch);

    // 当前快照只见 v2 (5433)，时间旅行回 v1.tx_epoch 精准见 v1 (5432) 且明文密钥已脱敏
    let cur_snap = adapter
        .search_tx(
            "5433",
            &TxSearchOptions {
                tenant_scope: "stress_tenant".to_string(),
                limit: 5,
                as_of_epoch: None,
                include_history: false,
            },
        )
        .expect("search latest v2");
    assert_eq!(cur_snap.hits.len(), 1);
    assert!(cur_snap.hits[0].content.contains("5433"));

    let old_snap = adapter
        .search_tx(
            "5432",
            &TxSearchOptions {
                tenant_scope: "stress_tenant".to_string(),
                limit: 5,
                as_of_epoch: Some(v1.tx_epoch),
                include_history: false,
            },
        )
        .expect("as_of_epoch time travel");
    assert_eq!(old_snap.hits.len(), 1);
    assert!(old_snap.hits[0].content.contains("5432"));
    assert!(
        !old_snap.hits[0].content.contains(secret_raw),
        "PG 落盘与召回严禁包含明文 API 密钥"
    );

    // 过时 CAS 触发 MVCC_EPOCH_CONFLICT → AOPCC Diagnoser < 2ms 自愈处方
    let cas_err = adapter
        .append_tx_event(&TxMemoryWriteInput {
            body_id: "body-stress".to_string(),
            tenant_scope: "stress_tenant".to_string(),
            fact_key: "deploy:pg_port".to_string(),
            memory_type: "fact".to_string(),
            content: "stale writer tries port 5434".to_string(),
            summary: None,
            tags: vec![],
            evidence_tier: TxEvidenceTier::L3Semantic,
            salience_score: 0.7,
            expected_epoch: Some(v1.tx_epoch),
            source_agent: Some("stale-agent".to_string()),
            source_session: None,
            embedding_ref: None,
        })
        .expect_err("stale expected_epoch must fail with MVCC_EPOCH_CONFLICT");
    let cas_err_msg = format!("{:#}", cas_err);
    assert!(cas_err_msg.contains("MVCC_EPOCH_CONFLICT"));
    let rx = OptimisticFailureDiagnoser::diagnose(
        &storage,
        "body_memory_add",
        &serde_json::json!({ "fact_key": "deploy:pg_port" }),
        &cas_err_msg,
    )
    .expect("AOPCC diagnoser must prescribe MVCC_EPOCH_CONFLICT recovery");
    assert_eq!(rx.failure_kind, "MVCC_EPOCH_CONFLICT");
    assert!(rx.elapsed_us < 2_000, "OCC 自愈诊断必须 < 2ms");

    // 写穿墓碑删除 → 该 fact_key 全部历史+当前版本均置 tombstoned 且 0% 僵尸召回
    let tomb_n = adapter
        .tombstone_fact("body-stress", "stress_tenant", "deploy:pg_port", "decommissioned")
        .expect("tombstone_fact");
    assert_eq!(tomb_n, 2, "tombstone 应彻底墓碑化该 fact_key 的 v1+v2 共 2 条版本记录");
    let post_tomb_snap = adapter
        .search_tx(
            "5433",
            &TxSearchOptions {
                tenant_scope: "stress_tenant".to_string(),
                limit: 5,
                as_of_epoch: None,
                include_history: false,
            },
        )
        .expect("search after tombstone");
    assert!(
        post_tomb_snap.hits.is_empty(),
        "写穿墓碑删除后必须 0% 僵尸召回"
    );

    // 3. 写入 200 条逻辑事实，实测解耦段检索 P50 / P99 延迟 (< 20ms)
    for i in 0..200usize {
        adapter
            .append_tx_event(&TxMemoryWriteInput {
                body_id: "body-stress".to_string(),
                tenant_scope: "stress_tenant".to_string(),
                fact_key: format!("kb:topic_{:03}", i),
                memory_type: "experience".to_string(),
                content: format!(
                    "PostgreSQL MVCC checkpoint tuning note #{} wal_buffers max_wal_size cluster_{}",
                    i,
                    i % 7
                ),
                summary: Some(format!("PG MVCC tuning note #{}", i)),
                tags: vec!["postgres".to_string(), format!("cluster_{}", i % 7)],
                evidence_tier: TxEvidenceTier::L2Structural,
                salience_score: 0.75,
                expected_epoch: None,
                source_agent: Some("seed-bot".to_string()),
                source_session: None,
                embedding_ref: None,
            })
            .expect("seed 200 facts");
    }

    let mut latencies_ms = Vec::with_capacity(50);
    for q_idx in 0..50usize {
        let q = if q_idx % 2 == 0 {
            "PostgreSQL MVCC checkpoint"
        } else {
            "wal_buffers max_wal_size"
        };
        let qt0 = Instant::now();
        let res = adapter
            .search_tx(
                q,
                &TxSearchOptions {
                    tenant_scope: "stress_tenant".to_string(),
                    limit: 10,
                    as_of_epoch: None,
                    include_history: false,
                },
            )
            .expect("timed search_tx");
        let ms = qt0.elapsed().as_secs_f64() * 1000.0;
        assert!(!res.hits.is_empty());
        latencies_ms.push(ms);
    }
    latencies_ms.sort_by(|a, b| a.total_cmp(b));
    let p50_ms = latencies_ms[latencies_ms.len() / 2];
    let p99_idx = ((latencies_ms.len() as f64 * 0.99).ceil() as usize)
        .saturating_sub(1)
        .min(latencies_ms.len() - 1);
    let p99_ms = latencies_ms[p99_idx];
    println!(
        "\n⚡ [PG 18 MVCC实测] 16线程×5并发写(80tx)耗时={:.1}ms, OCC自动重试={}次, 丢失更新=0, valid_range重叠=0 | 200事实解耦段检索 P50={:.2}ms, P99={:.2}ms (SLA < 20ms)",
        stress_ms, total_occ_retries, p50_ms, p99_ms
    );
    assert!(
        p99_ms < 20.0,
        "解耦检索段 P99 延迟应 < 20ms，实测 {:.2}ms",
        p99_ms
    );

    // 4. SQLite ⇄ PostgreSQL 双向热同步 + ContinuityCapsule 锚定真实 tx_epoch
    storage
        .insert_memory(&MemoryRecord {
            id: "sqlite-origin-1".to_string(),
            memory_type: "decision".to_string(),
            content: "Use pg_tx Scheme A as transactional memory core".to_string(),
            summary: Some("Scheme A decision".to_string()),
            source_agent: Some("local".to_string()),
            source_session: None,
            tags: vec!["architecture".to_string()],
            weight: 0.9,
            pinned: true,
            status: "active".to_string(),
            embedding: vec![],
        })
        .expect("insert sqlite origin memory");
    let up_synced = adapter.sync_sqlite_to_pg(100).expect("sync_sqlite_to_pg");
    assert!(up_synced >= 1);
    let down_synced = adapter.sync_pg_to_sqlite(50).expect("sync_pg_to_sqlite");
    assert!(down_synced >= 10);

    let live_epoch = adapter.current_tx_epoch().expect("current_tx_epoch");
    assert!(live_epoch >= (total_expected as i64) + 200);
    let capsule = ContinuityManager::save_capsule_with_epoch(
        &storage,
        ContinuityInput {
            from_brain: "claude-code".to_string(),
            session_id: "pg18-live-handoff".to_string(),
            goal_summary: "Handoff after live PG 18 MVCC verification".to_string(),
            active_files: vec!["contrib/mem_pg_tx/src/engine.rs".to_string()],
            active_locks: vec![],
            verified_facts: vec![format!("PG 18 live epoch={}", live_epoch)],
            open_blockers: vec![],
            next_step_hint: "Proceed to Phase 77 M2".to_string(),
            token_budget: Some(300),
        },
        live_epoch,
    )
    .expect("save capsule with live pg epoch");
    let snippet = ContinuityManager::render_brief_snippet(&capsule, 300);
    assert!(snippet.contains(&format!("tx_epoch=#{}", live_epoch)));
    assert!(estimate_tokens(&snippet) <= 300);
}

#[test]
fn pg_tx_unreachable_graceful_fallback_and_strict_mode() {
    let storage = Storage::open_in_memory().expect("open sqlite");

    let fallback_cfg = PgTxAdapterConfig {
        pg_url: "postgres://postgres@127.0.0.1:19991/does_not_exist".to_string(),
        pool_size: 2,
        tenant_scope: "fallback_scope".to_string(),
        vector_backend: "none".to_string(),
        fallback_to_sqlite: true,
        mirror_writes_to_sqlite: true,
    };
    let adapter = PgTxMemoryAdapter::connect_or_fallback(fallback_cfg, storage.clone())
        .expect("fallback_to_sqlite=true must gracefully degrade");
    assert!(!adapter.is_pg_active());
    let health = adapter.tx_health().expect("tx_health");
    assert!(health.fallback_active);
    assert_eq!(health.provider, "pg_tx_fallback_sqlite");

    adapter
        .append_tx_event(&TxMemoryWriteInput {
            body_id: "body-fb".to_string(),
            tenant_scope: "fallback_scope".to_string(),
            fact_key: "fb:key1".to_string(),
            memory_type: "fact".to_string(),
            content: "offline resilience fact alpha".to_string(),
            summary: Some("offline fact".to_string()),
            tags: vec!["offline".to_string()],
            evidence_tier: TxEvidenceTier::L3Semantic,
            salience_score: 0.8,
            expected_epoch: None,
            source_agent: Some("test".to_string()),
            source_session: None,
            embedding_ref: None,
        })
        .expect("append via sqlite fallback");

    let snap = adapter
        .search_tx(
            "resilience",
            &TxSearchOptions {
                tenant_scope: "fallback_scope".to_string(),
                limit: 5,
                as_of_epoch: None,
                include_history: false,
            },
        )
        .expect("search via sqlite fallback");
    assert!(snap.fallback_active);
    assert_eq!(snap.hits.len(), 1);

    // 验证降级事件已记入 OUT_OF_AGENT 审计账本
    let audits = storage
        .query_audit_events(&agent_body::storage::models::AuditQueryFilter {
            domain: Some("OUT_OF_AGENT".to_string()),
            category: Some("memory_op".to_string()),
            limit: 10,
            ..Default::default()
        })
        .expect("query fallback audit");
    assert!(audits.iter().any(|a| a.action == "fallback_to_sqlite"));

    // 严格模式（fallback_to_sqlite = false）必须显式报错
    let strict_cfg = PgTxAdapterConfig {
        pg_url: "postgres://postgres@127.0.0.1:19991/does_not_exist".to_string(),
        pool_size: 2,
        tenant_scope: "fallback_scope".to_string(),
        vector_backend: "none".to_string(),
        fallback_to_sqlite: false,
        mirror_writes_to_sqlite: false,
    };
    assert!(
        PgTxMemoryAdapter::connect_or_fallback(strict_cfg, storage).is_err(),
        "fallback_to_sqlite=false 时 PG 不可达必须显式报错"
    );
}

#[test]
fn legacy_sqlite_body_checkpoints_without_tx_epoch_auto_migrates() {
    let dir = tempfile::tempdir().expect("tempdir");
    let db_path = dir.path().join("legacy_checkpoints.db");
    {
        let conn = rusqlite::Connection::open(&db_path).expect("raw sqlite");
        conn.execute_batch(
            r#"
            CREATE TABLE body_checkpoints (
                checkpoint_id TEXT PRIMARY KEY,
                body_id TEXT NOT NULL,
                from_brain TEXT NOT NULL,
                session_id TEXT NOT NULL,
                goal_summary TEXT NOT NULL,
                active_files_json TEXT NOT NULL DEFAULT '[]',
                active_locks_json TEXT NOT NULL DEFAULT '[]',
                verified_facts_json TEXT NOT NULL DEFAULT '[]',
                open_blockers_json TEXT NOT NULL DEFAULT '[]',
                next_step_hint TEXT NOT NULL DEFAULT '',
                token_budget INTEGER NOT NULL DEFAULT 300,
                status TEXT NOT NULL DEFAULT 'active',
                evidence_sha256 TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL,
                resumed_by_brain TEXT,
                resumed_at TEXT
            );
            INSERT INTO body_checkpoints (
                checkpoint_id, body_id, from_brain, session_id, goal_summary,
                next_step_hint, status, evidence_sha256, created_at
            ) VALUES (
                'cp-legacy-001', 'body-old', 'cursor', 'sess-old', 'Legacy task before Phase 76',
                'Continue smoothly', 'active', 'abc123', '2026-10-01T00:00:00Z'
            );
            "#,
        )
        .expect("seed legacy body_checkpoints");
    }

    let storage = Storage::open(&db_path).expect("open via Storage::open with auto-reconcile");
    let all_before = storage
        .list_body_checkpoints(None, 20)
        .expect("list migrated legacy checkpoints");
    let old = all_before
        .iter()
        .find(|c| c.checkpoint_id == "cp-legacy-001")
        .expect("legacy checkpoint preserved");
    assert_eq!(old.tx_epoch, 0, "历史接力胶囊自动补齐默认 tx_epoch = 0");
    assert_eq!(old.goal_summary, "Legacy task before Phase 76");

    let fresh = ContinuityManager::save_capsule_with_epoch(
        &storage,
        ContinuityInput {
            from_brain: "antigravity".to_string(),
            session_id: "sess-new".to_string(),
            goal_summary: "New capsule anchored at tx_epoch 77".to_string(),
            active_files: vec![],
            active_locks: vec![],
            verified_facts: vec![],
            open_blockers: vec![],
            next_step_hint: "Done".to_string(),
            token_budget: Some(300),
        },
        77,
    )
    .expect("save new capsule after migration");
    let all_after = storage
        .list_body_checkpoints(None, 20)
        .expect("reload new capsule");
    let loaded = all_after
        .iter()
        .find(|c| c.checkpoint_id == fresh.checkpoint_id)
        .expect("new capsule exists");
    assert_eq!(loaded.tx_epoch, 77);
}

#[test]
fn mcp_43_tools_zero_bloat_and_bitemporal_memory_contract() {
    let storage = Storage::open_in_memory().expect("open sqlite");
    let registry = McpToolRegistry::new(storage);
    let tools = registry.list_tools();
    assert_eq!(tools.len(), 43, "43 原生 MCP 工具严格零膨胀");

    let add_schema = tools
        .iter()
        .find(|t| t.get("name").and_then(|v| v.as_str()) == Some("body_memory_add"))
        .expect("body_memory_add schema");
    let add_props = &add_schema["inputSchema"]["properties"];
    assert!(add_props.get("fact_key").is_some());
    assert!(add_props.get("evidence_tier").is_some());
    assert!(add_props.get("expected_epoch").is_some());

    let search_schema = tools
        .iter()
        .find(|t| t.get("name").and_then(|v| v.as_str()) == Some("body_memory_search"))
        .expect("body_memory_search schema");
    let search_props = &search_schema["inputSchema"]["properties"];
    assert!(search_props.get("as_of_epoch").is_some());
    assert!(search_props.get("include_history").is_some());

    let audit_schema = tools
        .iter()
        .find(|t| t.get("name").and_then(|v| v.as_str()) == Some("body_memory_audit"))
        .expect("body_memory_audit schema");
    let audit_props = &audit_schema["inputSchema"]["properties"];
    assert!(audit_props.get("tx_status").is_some());
    assert!(audit_props.get("tombstone_fact_key").is_some());

    // 端到端调用验证向后兼容
    let add_out = registry
        .call_tool(
            "body_memory_add",
            serde_json::json!({
                "content": "Phase 76 zero bloat bitemporal verification",
                "memory_type": "decision",
                "fact_key": "verify:zero_bloat",
                "evidence_tier": "L1_EXPLICIT"
            }),
        )
        .expect("call body_memory_add");
    assert!(!add_out.is_empty());

    let audit_out = registry
        .call_tool(
            "body_memory_audit",
            serde_json::json!({ "tx_status": true }),
        )
        .expect("call body_memory_audit with tx_status");
    assert!(!audit_out.is_empty());
}
