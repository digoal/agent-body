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
            let edge = FactEdgeRecord {
                id: None,
                source_id: format!("node:{}", i - 1),
                target_id: format!("node:{}", i),
                relation: "depends_on".to_string(),
                properties: serde_json::json!({}),
                confidence: 0.95,
                updated_at: None,
            };
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
