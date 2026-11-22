use std::time::Instant;
use agent_steward::storage::models::*;
use agent_steward::storage::Storage;
use agent_steward::placement::engine_v2::{PlacementEngineV2, PlacementRequestV2};

#[test]
fn bench_property_graph_traversal() {
    let storage = Storage::open_in_memory().expect("storage");

    // 建立 100 个节点与 150 条拓扑边
    for i in 0..100 {
        let node = FactNodeRecord {
            id: format!("node:{}", i),
            node_type: if i % 3 == 0 { "service" } else if i % 3 == 1 { "port" } else { "project" }.to_string(),
            name: format!("entity_{}", i),
            label: format!("实体 {}", i),
            properties: serde_json::json!({"index": i}),
            updated_at: None,
        };
        storage.upsert_fact_node(&node).expect("upsert node");

        if i > 0 {
            let edge = FactEdgeRecord::new_explicit(
                format!("node:{}", i - 1),
                format!("node:{}", i),
                "depends_on",
                serde_json::json!({}),
                0.95,
            );
            storage.upsert_fact_edge(&edge).expect("upsert edge");
        }
    }

    // 执行 3 跳图遍历基准测试
    let start = Instant::now();
    let iters = 50;
    for _ in 0..iters {
        let sub = storage.traverse_fact_graph("node:10", 3).expect("traverse");
        assert!(!sub.nodes.is_empty());
    }
    let total_elapsed = start.elapsed();
    let avg_us = total_elapsed.as_micros() / iters as u128;
    println!("\n⚡ [BENCH] 属性图 3 跳遍历平均延迟: {} μs ({} iters)", avg_us, iters);
    assert!(avg_us < 5000, "属性图遍历应在 5ms 以内，实际: {} μs", avg_us);
}

#[test]
fn bench_fts_asset_search() {
    let storage = Storage::open_in_memory().expect("storage");

    // 批量录入 300 条资产
    for i in 0..300 {
        let asset = AssetRecord {
            id: format!("asset-{}", i),
            path: format!("/root/new/shared_proj/project_{}/module.rs", i),
            asset_type: "script".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some(format!("PostgreSQL 数据库内核高可用及监控脚本 模块 {}", i)),
            desc_kv: serde_json::json!({"mod": i}),
            usage: Some("cargo run".to_string()),
            size_bytes: Some(1024),
            file_count: Some(1),
            tags: vec!["postgres".to_string(), "script".to_string()],
            status: "active".to_string(),
        };
        storage.upsert_asset(&asset).expect("upsert");
    }

    // 执行 FTS 检索基准测试
    let start = Instant::now();
    let iters = 50;
    for _ in 0..iters {
        let hits = storage.search_assets_fts("PostgreSQL", 10).expect("fts");
        assert!(!hits.is_empty());
    }
    let total_elapsed = start.elapsed();
    let avg_us = total_elapsed.as_micros() / iters as u128;
    println!("⚡ [BENCH] 资产 FTS 全文检索平均延迟: {} μs ({} iters)", avg_us, iters);
    assert!(avg_us < 5000, "FTS 检索单次应在 5ms 以内，实际: {} μs", avg_us);
}

#[test]
fn bench_placement_engine_decision() {
    let storage = Storage::open_in_memory().expect("storage");
    let engine = PlacementEngineV2::new(storage);

    let start = Instant::now();
    let iters = 200;
    for i in 0..iters {
        let req = PlacementRequestV2 {
            object_type: "script".to_string(),
            name: format!("pg_vacuum_analyzer_{}.py", i),
            intent: "PostgreSQL 垃圾回收与死元组诊断工具".to_string(),
            tags: vec!["postgres".to_string()],
            security_level: "internal".to_string(),
            current_workspace: None,
        };
        let res = engine.suggest(&req).expect("suggest");
        assert!(res.suggested_path.starts_with("/root/new/shared_proj"));
    }
    let total_elapsed = start.elapsed();
    let avg_us = total_elapsed.as_micros() / iters as u128;
    println!("⚡ [BENCH] 放置决策引擎平均延迟: {} μs ({} iters)", avg_us, iters);
    assert!(avg_us < 1000, "放置决策单次应在 1ms 以内，实际: {} μs", avg_us);
}

#[test]
fn bench_duckdb_aggregate_changes() {
    use agent_steward::storage::{DuckDbStorageProvider, DuckDbAnalyticsEngine, AnalyticsProvider};
    let provider = DuckDbStorageProvider::open_in_memory().expect("duckdb");

    // 向量化批量生成 1000 条变更记录（模拟真实批量同步吞吐）
    {
        let conn = provider.conn();
        let conn = conn.lock().unwrap();
        conn.execute_batch(
            "INSERT INTO changes (type, target, description, details, source, created_at)
             SELECT 
                 CASE WHEN i % 2 = 0 THEN 'file_modified' ELSE 'file_created' END,
                 '/root/new/work/proj_' || (i % 20) || '/src/file_' || (i % 50) || '.rs',
                 'change ' || i,
                 '{\"iter\": ' || i || '}',
                 'agent',
                 strftime(now(), '%Y-%m-%dT%H:%M:%SZ')
             FROM generate_series(1, 1000) t(i);"
        ).expect("bulk insert changes");
    }

    let engine = DuckDbAnalyticsEngine::new(provider.conn());

    let start = Instant::now();
    let iters = 20;
    for _ in 0..iters {
        let heatmap = engine.changes_heatmap(30).expect("heatmap");
        assert!(!heatmap.is_empty());
        let top = engine.most_changed_assets(30, 10).expect("top");
        assert_eq!(top.len(), 10);
    }
    let avg_us = start.elapsed().as_micros() / iters as u128;
    println!("⚡ [BENCH] DuckDB 1000条变更热力图与Top-N聚合延迟: {} μs ({} iters)", avg_us, iters);
    assert!(avg_us < 150_000, "聚合分析延迟在 debug 模式下应在 150ms 以内，实际: {} μs", avg_us);
}

#[test]
fn bench_duckdb_parquet_export() {
    use agent_steward::storage::{DuckDbStorageProvider, DuckDbAnalyticsEngine, AnalyticsProvider};
    let provider = DuckDbStorageProvider::open_in_memory().expect("duckdb");

    // 向量化批量生成 1000 条资产记录
    {
        let conn = provider.conn();
        let conn = conn.lock().unwrap();
        conn.execute_batch(
            "INSERT INTO assets (id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status, updated_at)
             SELECT 
                 'asset-bench-' || i,
                 '/root/new/work/repo_' || (i % 10) || '/item_' || i || '.rs',
                 'project',
                 'user:digoal',
                 'internal',
                 'deep',
                 'DuckDB 性能基准测试资产条目 ' || i,
                 '{\"index\": ' || i || '}',
                 NULL,
                 1024,
                 5,
                 '[\"benchmark\"]',
                 'active',
                 strftime(now(), '%Y-%m-%dT%H:%M:%SZ')
             FROM generate_series(1, 1000) t(i);"
        ).expect("bulk insert assets");
    }

    let engine = DuckDbAnalyticsEngine::new(provider.conn());
    let tmp = tempfile::tempdir().expect("tempdir");
    let export_path = tmp.path().join("bench_export.parquet");

    let start = Instant::now();
    let summary = engine.export_to_parquet("assets", export_path.to_str().unwrap()).expect("export");
    let elapsed_ms = start.elapsed().as_millis();

    println!("⚡ [BENCH] DuckDB 1000条资产全量导出耗时: {} ms (文件大小: {} bytes)", elapsed_ms, summary.file_size_bytes);
    assert_eq!(summary.row_count, 1000);
    assert!(elapsed_ms < 500, "1000条资产导出应在 500ms 内，实际: {} ms", elapsed_ms);
}

#[test]
fn bench_empirical_mcp_ledger_and_ab_suite() {
    use std::sync::Arc;
    use agent_steward::api::mcp_tools::McpToolRegistry;
    use agent_steward::benchmark::BenchmarkEngine;
    use agent_steward::storage::{Storage, SqliteStorageProvider};

    let provider = Arc::new(SqliteStorageProvider::open_in_memory().expect("sqlite"));
    let storage = Storage::new(provider.clone());
    let registry = McpToolRegistry::new(storage.clone());

    // 1. 验证 MCP 工具已注册至 41 个且包含 steward_benchmark 与 steward_feedback
    let tools = registry.list_tools();
    assert_eq!(tools.len(), 41, "原生 MCP 工具数应为 41 个");
    assert!(tools.iter().any(|t| t.get("name").and_then(|v| v.as_str()) == Some("steward_benchmark")));
    assert!(tools.iter().any(|t| t.get("name").and_then(|v| v.as_str()) == Some("steward_feedback")));

    // 2. 调用典型工具并验证生产效能账本真实带出处入账
    let _ = registry.call_tool("steward_brief", serde_json::json!({"query": "benchmark test", "budget_tokens": 800})).expect("brief");
    let _ = registry.call_tool("steward_skill_route", serde_json::json!({"intent": "postgres", "budget_tokens": 600})).expect("route");

    let ledger = storage.query_effectiveness_ledger("7d", None, 10).expect("ledger");
    assert_eq!(ledger.total_events, 2, "两次 MCP 调用应生成 2 条账本记录");
    assert!(ledger.total_rounds_saved >= 7, "brief(5) + route(2) 至少缩减 7 轮探查");
    assert!(ledger.by_rule.iter().any(|r| r.provenance_rule == "PR-08"));
    assert!(ledger.by_rule.iter().any(|r| r.provenance_rule == "PR-01"));

    // 3. 通过第 40 工具运行 macro A/B 套件并验证 SHA-256 存证与清理
    let run_out = registry.call_tool("steward_benchmark", serde_json::json!({
        "action": "run",
        "suite": "macro",
        "save": true
    })).expect("steward_benchmark run");
    assert!(run_out.contains("report_sha256"));

    let cleaned = BenchmarkEngine::reset_sandbox(provider.as_ref()).expect("reset");
    assert_eq!(cleaned, 0, "未注入 bench:* 沙盒前缀时应 0 残留");
}
