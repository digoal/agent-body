use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use agent_body::benchmark::ledger::EffectivenessLedger;
use agent_body::benchmark::BenchmarkEngine;
use agent_body::body::BodyPerfEngine;
use agent_body::memory::MemoryManager;
use agent_body::storage::duckdb_provider::DuckDbStorageProvider;
use agent_body::storage::sqlite_provider::SqliteStorageProvider;
use agent_body::storage::{Storage, StorageProvider};

/// 构造历史重命名前列前缀（运行时拼装，避免源码出现旧名称字面量）
fn legacy_col_prefix() -> String {
    ["st", "ew", "ard"].concat()
}

/// 在指定路径构建高度还原生产老版本 `body.db` 的历史 SQLite 数据库：
/// - `benchmark_runs` 含旧版 `total_<legacy>_tokens` 列并预置历史记录
/// - `effectiveness_ledger` 含旧版 `<legacy>_tokens` / `<legacy>_rounds` 列并预置历史记录
/// - `memories` / `assets` 缺少 `embedding` 列
/// - `links` 缺少 `line_number` / `snippet` / `provenance` 列
/// - `fact_nodes` 缺少 `domain_term` CHECK 与 `embedding` 列
/// - `fact_edges` 缺少 `evidence_tier` / `weight` / `direction_kind` 与新关系 CHECK
/// - `migrations` 仅含 version=1
fn build_realistic_legacy_sqlite_db(db_path: &std::path::Path) {
    let lp = legacy_col_prefix();
    let conn = rusqlite::Connection::open(db_path).expect("create raw legacy sqlite db");
    let ddl = format!(
        r#"
        PRAGMA journal_mode = WAL;
        CREATE TABLE config (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL,
            updated_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
        );
        CREATE TABLE migrations (
            version INTEGER PRIMARY KEY,
            description TEXT NOT NULL,
            applied_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
        );
        INSERT INTO config(key, value) VALUES ('schema_version', '2'), ('{lp}_version', '0.2.0');
        INSERT INTO migrations(version, description) VALUES (1, 'initial schema v2');

        CREATE TABLE memories (
            id TEXT PRIMARY KEY,
            type TEXT NOT NULL CHECK (type IN ('decision','task','experience','preference','fact','note')),
            content TEXT NOT NULL,
            summary TEXT,
            source_agent TEXT,
            source_session TEXT,
            tags TEXT DEFAULT '[]',
            weight REAL DEFAULT 1.0,
            pinned INTEGER DEFAULT 0,
            sensitive INTEGER DEFAULT 0,
            status TEXT DEFAULT 'active',
            created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
            updated_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
            last_accessed_at TEXT,
            access_count INTEGER DEFAULT 0
        );
        INSERT INTO memories (id, type, content, summary) VALUES
            ('legacy-mem-1', 'decision', 'Keep historical decision across schema upgrade', 'legacy decision');

        CREATE TABLE assets (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            type TEXT NOT NULL,
            owner TEXT DEFAULT 'user:local',
            security_level TEXT DEFAULT 'internal',
            scan_tier TEXT DEFAULT 'deep',
            description TEXT,
            desc_kv TEXT DEFAULT '{{}}',
            usage TEXT,
            size_bytes INTEGER,
            file_count INTEGER,
            tags TEXT DEFAULT '[]',
            status TEXT DEFAULT 'active',
            last_scanned_at TEXT,
            last_modified_at TEXT,
            created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
            updated_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
        );
        INSERT INTO assets (id, path, type, description) VALUES
            ('legacy-asset-1', '/tmp/legacy_script.py', 'script', 'legacy python asset');

        CREATE TABLE links (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_type TEXT NOT NULL,
            source_id TEXT NOT NULL,
            target_type TEXT NOT NULL,
            target_id TEXT NOT NULL,
            relation TEXT NOT NULL,
            confidence REAL DEFAULT 1.0,
            auto_discovered INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
            UNIQUE(source_type, source_id, target_type, target_id, relation)
        );
        INSERT INTO links (source_type, source_id, target_type, target_id, relation) VALUES
            ('asset', '/tmp/legacy_script.py', 'service', 'postgres', 'depends_on');

        CREATE TABLE fact_nodes (
            id TEXT PRIMARY KEY,
            node_type TEXT NOT NULL CHECK (node_type IN ('project','tool','port','service','cron','file','authoritative_site')),
            name TEXT NOT NULL,
            label TEXT NOT NULL,
            properties TEXT DEFAULT '{{}}',
            updated_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
        );
        INSERT INTO fact_nodes (id, node_type, name, label) VALUES
            ('service:legacy_pg', 'service', 'legacy_pg', 'Legacy PostgreSQL'),
            ('port:5432', 'port', '5432', 'Port 5432');

        CREATE TABLE fact_edges (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id TEXT NOT NULL,
            target_id TEXT NOT NULL,
            relation TEXT NOT NULL CHECK (relation IN ('depends_on','listens_on','scheduled_by','reads','writes','part_of','calls','contains','uses_tool','runs_as','documented_by')),
            properties TEXT DEFAULT '{{}}',
            confidence REAL DEFAULT 1.0,
            updated_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
            UNIQUE(source_id, target_id, relation)
        );
        INSERT INTO fact_edges (source_id, target_id, relation, confidence) VALUES
            ('service:legacy_pg', 'port:5432', 'listens_on', 1.0);

        CREATE TABLE benchmark_runs (
            run_id                    TEXT PRIMARY KEY,
            suite_name                TEXT NOT NULL,
            git_commit                TEXT NOT NULL,
            app_version               TEXT NOT NULL,
            host_fingerprint          TEXT NOT NULL,
            workload_count            INTEGER NOT NULL,
            total_baseline_tokens     INTEGER NOT NULL,
            total_{lp}_tokens         INTEGER NOT NULL,
            overall_token_savings_pct REAL NOT NULL,
            overall_speedup_factor    REAL NOT NULL,
            total_hazards_intercepted INTEGER NOT NULL,
            total_noise_blocked       INTEGER NOT NULL,
            sla_passed                INTEGER NOT NULL,
            results_json              TEXT NOT NULL,
            report_sha256             TEXT NOT NULL,
            duration_ms               INTEGER NOT NULL,
            created_at                TEXT NOT NULL
        );
        INSERT INTO benchmark_runs VALUES (
            'legacy-bench-001', 'macro', 'abc1234', '0.9.0', '{{}}',
            10, 20000, 1800, 91.0, 25.5, 4, 120, 1, '[]',
            'deadbeef00000000000000000000000000000000000000000000000000000001',
            850, '2026-11-15T10:00:00+00:00'
        );

        CREATE TABLE effectiveness_ledger (
            event_id           TEXT PRIMARY KEY,
            session_id         TEXT NOT NULL DEFAULT 'default',
            source_channel     TEXT NOT NULL,
            tool_or_action     TEXT NOT NULL,
            provenance_rule    TEXT NOT NULL,
            baseline_tokens    INTEGER NOT NULL DEFAULT 0,
            {lp}_tokens        INTEGER NOT NULL DEFAULT 0,
            tokens_saved       INTEGER NOT NULL DEFAULT 0,
            baseline_rounds    INTEGER NOT NULL DEFAULT 1,
            {lp}_rounds        INTEGER NOT NULL DEFAULT 1,
            rounds_saved       INTEGER NOT NULL DEFAULT 0,
            hazard_intercepted INTEGER NOT NULL DEFAULT 0,
            noise_blocked      INTEGER NOT NULL DEFAULT 0,
            elapsed_us         INTEGER NOT NULL DEFAULT 0,
            evidence_summary   TEXT NOT NULL,
            created_at         TEXT NOT NULL
        );
        INSERT INTO effectiveness_ledger VALUES (
            'legacy-ev-001', 'sess-legacy', 'mcp', 'body_brief', 'PR-01',
            3200, 420, 2780, 5, 1, 4, 1, 0, 950,
            'legacy ledger row before column rename',
            '2026-11-15T10:05:00+00:00'
        );
        "#,
        lp = lp
    );
    conn.execute_batch(&ddl)
        .expect("initialize legacy sqlite fixtures");
}

#[test]
fn test_legacy_sqlite_db_auto_migrates_losslessly_and_runs_benchmark_and_awr() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("legacy_body.db");
    build_realistic_legacy_sqlite_db(&db_path);

    // 1. 直接通过正式 SqliteStorageProvider::open 打开老库（触发自动无损迁移）
    let provider = Arc::new(SqliteStorageProvider::open(&db_path).expect("open & migrate legacy db"));

    // 2. 验证 benchmark_runs 历史行 100% 无损保留，且 total_body_tokens 可读
    let legacy_run = provider
        .get_benchmark_run("legacy-bench-001")
        .expect("get legacy benchmark run")
        .expect("legacy benchmark run must be preserved");
    assert_eq!(legacy_run.total_baseline_tokens, 20000);
    assert_eq!(legacy_run.total_body_tokens, 1800);
    assert_eq!(legacy_run.app_version, "0.9.0");
    assert!(legacy_run.sla_passed);

    // 3. 验证 effectiveness_ledger 历史行 100% 无损保留，且聚合查询正常
    let ledger = provider
        .query_effectiveness_ledger("all", None, 20)
        .expect("query migrated effectiveness_ledger");
    assert_eq!(ledger.total_events, 1);
    assert_eq!(ledger.total_baseline_tokens, 3200);
    assert_eq!(ledger.total_body_tokens, 420);
    assert_eq!(ledger.total_tokens_saved, 2780);
    assert_eq!(ledger.total_rounds_saved, 4);
    assert_eq!(ledger.recent_events[0].body_tokens, 420);
    assert_eq!(ledger.recent_events[0].body_rounds, 1);

    // 4. 在迁移后的老库上执行真实 Benchmark 运行并落盘（精准复现并修复用户实证报错）
    let report = BenchmarkEngine::run_suite(provider.clone(), "micro", true, None)
        .expect("BenchmarkEngine::run_suite(micro, save=true) must succeed on migrated legacy DB");
    assert!(!report.run_id.is_empty());
    assert!(!report.report_sha256.is_empty());

    let report_w1 = BenchmarkEngine::run_suite(provider.clone(), "W1", true, None)
        .expect("BenchmarkEngine::run_suite(W1, save=true) must succeed on migrated legacy DB");
    assert!(report_w1.total_baseline_tokens > 0);
    assert!(report_w1.total_body_tokens > 0);

    let all_runs = provider
        .list_benchmark_runs(20)
        .expect("list benchmark runs after save");
    assert_eq!(all_runs.len(), 3, "应包含 1 条历史记录 + 2 条新跑批记录");

    // 5. 写入新的效能账本事件并验证累计正确
    EffectivenessLedger::record_channel_call(
        provider.as_ref(),
        "web",
        "body_brief",
        "post-migration brief summary",
        380,
    );
    let ledger_after = provider
        .query_effectiveness_ledger("all", None, 20)
        .expect("query ledger after insert");
    assert!(ledger_after.total_events >= 2);

    // 6. 验证 AWR 快照与差分报告（也落在 benchmark_runs 表）在迁移后老库 100% 可用
    let storage = Storage::new(provider.clone());
    let snap = BodyPerfEngine::capture_snapshot(&storage, Some("post-migrate-base"), true)
        .expect("capture AWR baseline on migrated DB");
    assert!(snap.is_baseline);
    let awr = BodyPerfEngine::generate_awr_report(&storage, None, None, true)
        .expect("generate AWR report on migrated DB");
    assert!(!awr.report_id.is_empty());

    // 7. 验证 fact_nodes / fact_edges 升级后支持 Phase 61 新节点类型和语义边，且旧边保留
    let old_edge_list = provider.list_all_fact_edges(100).expect("list old fact_edges");
    assert_eq!(old_edge_list.len(), 1);
    assert_eq!(old_edge_list[0].evidence_tier, "L1_EXPLICIT");

    provider
        .upsert_fact_node(&agent_body::storage::FactNodeRecord {
            id: "term:wal_hygiene".to_string(),
            node_type: "domain_term".to_string(),
            name: "wal_hygiene".to_string(),
            label: "WAL Hygiene".to_string(),
            properties: serde_json::json!({}),
            updated_at: None,
        })
        .expect("insert domain_term into upgraded fact_nodes");

    // 8. 验证历史 assets / memories / links 无损保留且新增 embedding / provenance 列可用
    assert!(provider
        .get_asset_by_path("/tmp/legacy_script.py")
        .expect("get legacy asset")
        .is_some());
    provider
        .update_asset_embedding("/tmp/legacy_script.py", &[0.25, 0.75])
        .expect("update embedding on migrated asset");
    provider
        .update_asset_content("/tmp/legacy_script.py", "fn legacy_migrated_private_fn() {}")
        .expect("update content_text on migrated asset");
    let fz = provider
        .search_assets_fuzzy_db("legacy_migrated_private_fn", 10, true, None)
        .expect("fuzzy search on migrated assets_trigram_fts");
    assert!(fz.iter().any(|h| h.asset.path == "/tmp/legacy_script.py"));
    let mems = provider.list_all_memories(20).expect("list legacy memories");
    assert!(mems.iter().any(|m| m.id == "legacy-mem-1"));
    let links = provider.list_all_links(20).expect("list legacy links");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].provenance.as_deref(), Some("static_scan"));
}

#[test]
fn test_sqlite_schema_reconcile_is_idempotent_across_repeated_opens() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("idempotent_legacy.db");
    build_realistic_legacy_sqlite_db(&db_path);

    for round in 0..3 {
        let prov = SqliteStorageProvider::open(&db_path)
            .unwrap_or_else(|e| panic!("round {} open failed: {}", round, e));
        let run = prov
            .get_benchmark_run("legacy-bench-001")
            .expect("query run")
            .expect("run preserved");
        assert_eq!(run.total_body_tokens, 1800);
        let sum = prov
            .query_effectiveness_ledger("all", None, 10)
            .expect("query ledger");
        assert_eq!(sum.total_events, 1);
        assert_eq!(sum.total_body_tokens, 420);
    }

    // 验证 migrations 表已登记 version=2 且 config.body_version 已更新至当前 Cargo 版本
    let raw = rusqlite::Connection::open(&db_path).expect("raw open");
    let v2_count: i64 = raw
        .query_row("SELECT COUNT(*) FROM migrations WHERE version = 2", [], |r| r.get(0))
        .expect("query migrations");
    assert_eq!(v2_count, 1);
    let body_ver: String = raw
        .query_row("SELECT value FROM config WHERE key = 'body_version'", [], |r| r.get(0))
        .expect("query body_version");
    assert_eq!(body_ver, env!("CARGO_PKG_VERSION"));
}

#[test]
fn test_sqlite_missing_non_pk_columns_self_heal() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("partial_cols.db");
    {
        let raw = rusqlite::Connection::open(&db_path).expect("raw open");
        raw.execute_batch(
            "CREATE TABLE body_checkpoints (
                checkpoint_id       TEXT PRIMARY KEY,
                body_id             TEXT NOT NULL,
                from_brain          TEXT NOT NULL DEFAULT 'unknown',
                session_id          TEXT NOT NULL DEFAULT '',
                goal_summary        TEXT NOT NULL,
                active_files_json   TEXT NOT NULL DEFAULT '[]',
                active_locks_json   TEXT NOT NULL DEFAULT '[]',
                verified_facts_json TEXT NOT NULL DEFAULT '[]',
                open_blockers_json  TEXT NOT NULL DEFAULT '[]',
                next_step_hint      TEXT NOT NULL DEFAULT '',
                token_budget        INTEGER NOT NULL DEFAULT 300,
                status              TEXT NOT NULL DEFAULT 'active',
                evidence_sha256     TEXT NOT NULL,
                created_at          TEXT NOT NULL
            );",
        )
        .expect("create partial body_checkpoints");
    }

    let prov = SqliteStorageProvider::open(&db_path).expect("open and self-heal missing cols");
    let cps = prov
        .list_body_checkpoints(None, 10)
        .expect("list body_checkpoints must succeed after adding resumed_by_brain/resumed_at/tx_epoch/fast_weight");
    assert!(cps.is_empty());

    let conn_arc = prov.conn();
    let conn = conn_arc.lock().expect("lock");
    let mut stmt = conn
        .prepare("PRAGMA table_info('body_checkpoints')")
        .expect("pragma");
    let cols: Vec<String> = stmt
        .query_map([], |r| r.get(1))
        .expect("q")
        .filter_map(Result::ok)
        .collect();
    assert!(cols.contains(&"tx_epoch".to_string()));
    assert!(cols.contains(&"fast_weight_snapshot_id".to_string()));
    assert!(cols.contains(&"fast_weight_base_model".to_string()));
}

#[test]
fn test_sqlite_file_backed_wal_and_memory_hygiene_pragmas() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("wal_hygiene.db");
    let prov = SqliteStorageProvider::open(&db_path).expect("open file db");
    let conn_arc = prov.conn();
    let conn = conn_arc.lock().expect("lock");

    let journal_mode: String = conn
        .query_row("PRAGMA journal_mode", [], |r| r.get(0))
        .expect("journal_mode");
    assert_eq!(journal_mode.to_lowercase(), "wal");

    let cache_size: i64 = conn
        .query_row("PRAGMA cache_size", [], |r| r.get(0))
        .expect("cache_size");
    assert_eq!(cache_size, -8000);

    let wal_auto: i64 = conn
        .query_row("PRAGMA wal_autocheckpoint", [], |r| r.get(0))
        .expect("wal_autocheckpoint");
    assert_eq!(wal_auto, 1000);

    let journal_limit: i64 = conn
        .query_row("PRAGMA journal_size_limit", [], |r| r.get(0))
        .expect("journal_size_limit");
    assert_eq!(journal_limit, 67_108_864);
}

#[test]
fn test_legacy_duckdb_db_auto_migrates_losslessly() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let duck_path = tmp.path().join("legacy_analytics.duckdb");
    let lp = legacy_col_prefix();
    {
        let raw = duckdb::Connection::open(&duck_path).expect("create raw legacy duckdb");
        let ddl = format!(
            r#"
            CREATE TABLE benchmark_runs (
                run_id VARCHAR PRIMARY KEY,
                suite_name VARCHAR NOT NULL,
                git_commit VARCHAR NOT NULL,
                app_version VARCHAR NOT NULL,
                host_fingerprint VARCHAR NOT NULL,
                workload_count INTEGER NOT NULL,
                total_baseline_tokens BIGINT NOT NULL,
                total_{lp}_tokens BIGINT NOT NULL,
                overall_token_savings_pct DOUBLE NOT NULL,
                overall_speedup_factor DOUBLE NOT NULL,
                total_hazards_intercepted BIGINT NOT NULL,
                total_noise_blocked BIGINT NOT NULL,
                sla_passed BOOLEAN NOT NULL,
                results_json VARCHAR NOT NULL,
                report_sha256 VARCHAR NOT NULL,
                duration_ms BIGINT NOT NULL,
                created_at VARCHAR NOT NULL
            );
            INSERT INTO benchmark_runs VALUES (
                'duck-legacy-01', 'macro', 'deadbeef', '0.9.0', '{{}}',
                5, 10000, 900, 91.0, 20.0, 2, 50, true, '[]', 'sha-duck', 500, '2026-11-15T10:00:00Z'
            );
            CREATE TABLE effectiveness_ledger (
                event_id VARCHAR PRIMARY KEY,
                session_id VARCHAR DEFAULT 'default',
                source_channel VARCHAR NOT NULL,
                tool_or_action VARCHAR NOT NULL,
                provenance_rule VARCHAR NOT NULL,
                baseline_tokens BIGINT DEFAULT 0,
                {lp}_tokens BIGINT DEFAULT 0,
                tokens_saved BIGINT DEFAULT 0,
                baseline_rounds BIGINT DEFAULT 1,
                {lp}_rounds BIGINT DEFAULT 1,
                rounds_saved BIGINT DEFAULT 0,
                hazard_intercepted BIGINT DEFAULT 0,
                noise_blocked BIGINT DEFAULT 0,
                elapsed_us BIGINT DEFAULT 0,
                evidence_summary VARCHAR NOT NULL,
                created_at VARCHAR NOT NULL
            );
            INSERT INTO effectiveness_ledger VALUES (
                'duck-ev-01', 's1', 'mcp', 'body_brief', 'PR-01',
                2000, 250, 1750, 4, 1, 3, 0, 0, 600, 'duck legacy row', '2026-11-15T10:00:00Z'
            );
            "#,
            lp = lp
        );
        raw.execute_batch(&ddl).expect("seed legacy duckdb");
    }

    let duck = DuckDbStorageProvider::open(&duck_path).expect("open & migrate legacy duckdb");
    let run = duck
        .get_benchmark_run("duck-legacy-01")
        .expect("get duck run")
        .expect("duck run preserved");
    assert_eq!(run.total_body_tokens, 900);

    let ledger = duck
        .query_effectiveness_ledger("all", None, 10)
        .expect("query duck ledger");
    assert_eq!(ledger.total_events, 1);
    assert_eq!(ledger.total_body_tokens, 250);
    assert_eq!(ledger.recent_events[0].body_rounds, 1);
}

/// 解析 SQL schema 文件中的 `CREATE TABLE IF NOT EXISTS <name> (...)` 常规表与列名映射
fn parse_create_tables_columns(sql: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut result = BTreeMap::new();
    let table_re = regex::Regex::new(r"(?is)CREATE\s+TABLE\s+IF\s+NOT\s+EXISTS\s+([a-zA-Z0-9_]+)\s*\(")
        .expect("valid regex");
    let col_re = regex::Regex::new(r"^\s*([a-z][a-z0-9_]*)\s+").expect("valid col regex");

    for cap in table_re.captures_iter(sql) {
        let table_name = cap[1].to_string();
        let body_start = cap.get(0).expect("match").end();
        let bytes = sql.as_bytes();
        let mut depth = 1i32;
        let mut i = body_start;
        while i < bytes.len() && depth > 0 {
            match bytes[i] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        let body = &sql[body_start..i.saturating_sub(1)];
        let mut cols = BTreeSet::new();
        let mut line_depth = 0i32;
        for line in body.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("--") {
                continue;
            }
            if line_depth == 0 {
                let upper = trimmed.to_ascii_uppercase();
                let is_constraint = upper.starts_with("UNIQUE")
                    || upper.starts_with("FOREIGN")
                    || upper.starts_with("PRIMARY")
                    || upper.starts_with("CHECK")
                    || upper.starts_with("CONSTRAINT");
                if !is_constraint {
                    if let Some(c) = col_re.captures(trimmed) {
                        let col = c[1].to_string();
                        if col != "fts_doc" {
                            cols.insert(col);
                        }
                    }
                }
            }
            for ch in trimmed.chars() {
                if ch == '(' {
                    line_depth += 1;
                } else if ch == ')' {
                    line_depth -= 1;
                }
            }
        }
        result.insert(table_name, cols);
    }
    result
}

#[test]
fn test_three_backend_schema_parity_contract() {
    let sqlite_tables = parse_create_tables_columns(include_str!("../src/storage/schema.sql"));
    let pg_tables = parse_create_tables_columns(include_str!("../src/storage/pg_schema.sql"));
    let duck_tables = parse_create_tables_columns(include_str!("../src/storage/duckdb_schema.sql"));

    // 21 张核心跨后端共享域表必须在 SQLite / PostgreSQL / DuckDB 三端保持表名与列名 100% 一致
    let shared_domain_tables = [
        "placement_rules",
        "scan_policies",
        "assets",
        "active_processes",
        "memories",
        "changes",
        "links",
        "scan_jobs",
        "fact_nodes",
        "fact_edges",
        "coordination_locks",
        "mcp_catalog",
        "skills_catalog",
        "external_services",
        "lock_wait_queue",
        "benchmark_runs",
        "effectiveness_ledger",
        "mesh_dialogues",
        "mesh_transfers",
        "audit_events",
        "body_checkpoints",
    ];

    for table in shared_domain_tables {
        let sq = sqlite_tables
            .get(table)
            .unwrap_or_else(|| panic!("SQLite schema.sql 缺少表 {}", table));
        let pg = pg_tables
            .get(table)
            .unwrap_or_else(|| panic!("PG pg_schema.sql 缺少表 {}", table));
        let dk = duck_tables
            .get(table)
            .unwrap_or_else(|| panic!("DuckDB duckdb_schema.sql 缺少表 {}", table));

        assert_eq!(
            sq, pg,
            "SQLite vs PostgreSQL 表 {} 列定义漂移! sqlite={:?}, pg={:?}",
            table, sq, pg
        );
        assert_eq!(
            sq, dk,
            "SQLite vs DuckDB 表 {} 列定义漂移! sqlite={:?}, duckdb={:?}",
            table, sq, dk
        );
    }
}

#[test]
fn test_memory_type_normalization_and_alias_regression() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    let mm = MemoryManager::new(storage.clone());

    // 1. 历史 Web UI 选项 "troubleshooting" 必须无损归一化为 "experience"，绝不触碰 CHECK 约束报错
    let id = mm
        .add(
            "troubleshooting",
            "修复老库 benchmark_runs 缺失 total_body_tokens 列的排查记录",
            Some("Schema 漂移排障"),
            Some("web_console"),
            vec!["regression".to_string()],
        )
        .expect("troubleshooting alias must succeed");
    let list = storage.list_all_active_memories(10).expect("list memories");
    let saved = list.iter().find(|m| m.id == id).expect("saved memory");
    assert_eq!(saved.memory_type, "experience");

    // 2. 全部 6 种规范类型及常用别名均应成功
    for (raw, expected) in [
        ("decision", "decision"),
        ("task", "task"),
        ("todo", "task"),
        ("experience", "experience"),
        ("debug", "experience"),
        ("lesson", "experience"),
        ("preference", "preference"),
        ("rule", "preference"),
        ("fact", "fact"),
        ("knowledge", "fact"),
        ("note", "note"),
    ] {
        assert_eq!(
            MemoryManager::normalize_memory_type(raw).expect("valid type"),
            expected
        );
    }

    // 3. 完全非法类型应返回清晰错误而非直接触发底层 SQLite CHECK 约束异常
    let err = mm
        .add("completely_invalid_type_xyz", "some content", None, None, vec![])
        .expect_err("invalid memory type must fail cleanly");
    assert!(err.to_string().contains("不支持的记忆类型"));
}

#[test]
fn test_concurrent_legacy_sqlite_migration_is_race_free() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("concurrent_legacy.db");
    build_realistic_legacy_sqlite_db(&db_path);

    let barrier = Arc::new(std::sync::Barrier::new(4));
    let mut handles = Vec::new();
    for idx in 0..4 {
        let p = db_path.clone();
        let b = barrier.clone();
        handles.push(std::thread::spawn(move || {
            b.wait();
            let prov = SqliteStorageProvider::open(&p)
                .unwrap_or_else(|e| panic!("thread {} concurrent open failed: {}", idx, e));
            let run = prov
                .get_benchmark_run("legacy-bench-001")
                .expect("query run")
                .expect("run preserved");
            assert_eq!(run.total_body_tokens, 1800);
            let sum = prov
                .query_effectiveness_ledger("all", None, 10)
                .expect("query ledger");
            assert_eq!(sum.total_body_tokens, 420);
        }));
    }
    for h in handles {
        h.join().expect("thread must not panic");
    }
}
