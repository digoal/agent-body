use std::sync::Arc;
use tempfile::tempdir;
use agent_steward::storage::{
    AssetRecord, ChangeRecord, DuckDbAnalyticsEngine, DuckDbStorageProvider,
    FactNodeRecord, MemoryRecord, PlacementRule, Storage,
    StorageMigrator, SyncPipeline, AnalyticsProvider, StorageProvider,
};

#[test]
fn test_duckdb_provider_full_lifecycle_on_disk() {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("test_lifecycle.duckdb");

    let provider = DuckDbStorageProvider::open(&db_path).expect("open duckdb on disk");
    assert_eq!(provider.provider_name(), "duckdb");

    // 1. Placement Rules
    let rules = provider.get_placement_rules().expect("get placement rules");
    assert!(rules.len() >= 8);

    // 2. Asset CRUD
    let asset = AssetRecord {
        id: "asset-duck-disk".to_string(),
        path: "/root/new/work/duck-disk-proj".to_string(),
        asset_type: "project".to_string(),
        owner: "user:digoal".to_string(),
        security_level: "internal".to_string(),
        scan_tier: "deep".to_string(),
        description: Some("持久化 DuckDB 测试资产".to_string()),
        desc_kv: serde_json::json!({"persistent": true}),
        usage: Some("验证磁盘文件落盘与读取".to_string()),
        size_bytes: Some(2048),
        file_count: Some(12),
        tags: vec!["disk".to_string(), "duckdb".to_string()],
        status: "active".to_string(),
    };
    provider.upsert_asset(&asset).expect("upsert asset");

    let fetched = provider.get_asset_by_path(&asset.path).expect("get").expect("found");
    assert_eq!(fetched.id, asset.id);
    assert_eq!(fetched.size_bytes, Some(2048));

    // 3. 全文匹配
    let fts_res = provider.search_assets_fts("持久化", 5).expect("fts");
    assert_eq!(fts_res.len(), 1);
    assert_eq!(fts_res[0].id, asset.id);
}

#[test]
fn test_duckdb_analytics_features() {
    let provider = DuckDbStorageProvider::open_in_memory().expect("open in memory");

    // 插入测试变更记录
    let changes = vec![
        ChangeRecord {
            id: None,
            change_type: "file_created".to_string(),
            target: "/root/new/work/proj_a/src/main.rs".to_string(),
            description: Some("create main".to_string()),
            details: serde_json::json!({}),
            source: "agent".to_string(),
            created_at: Some("2026-11-18T10:00:00Z".to_string()),
        },
        ChangeRecord {
            id: None,
            change_type: "file_modified".to_string(),
            target: "/root/new/work/proj_a/src/main.rs".to_string(),
            description: Some("edit main".to_string()),
            details: serde_json::json!({}),
            source: "agent".to_string(),
            created_at: Some("2026-11-18T10:30:00Z".to_string()),
        },
        ChangeRecord {
            id: None,
            change_type: "file_modified".to_string(),
            target: "/root/new/shared_proj/sec_scan/run.py".to_string(),
            description: Some("update scan".to_string()),
            details: serde_json::json!({}),
            source: "user".to_string(),
            created_at: Some("2026-11-18T11:00:00Z".to_string()),
        },
    ];

    for c in &changes {
        provider.record_change(c).expect("record change");
    }

    let engine = DuckDbAnalyticsEngine::new(provider.conn());

    // 1. 变更热力图
    let heatmap = engine.changes_heatmap(30).expect("heatmap");
    assert!(!heatmap.is_empty(), "热力图应包含统计单元");

    // 2. 变更 Top-N
    let top = engine.most_changed_assets(30, 5).expect("most changed");
    assert!(!top.is_empty());
    assert_eq!(top[0].target, "/root/new/work/proj_a/src/main.rs");
    assert_eq!(top[0].change_count, 2);

    // 3. 事实图谱全景统计
    let node = FactNodeRecord {
        id: "node:proj_a".to_string(),
        node_type: "project".to_string(),
        name: "proj_a".to_string(),
        label: "项目A".to_string(),
        properties: serde_json::json!({}),
        updated_at: Some("2026-11-18T12:00:00Z".to_string()),
    };
    provider.upsert_fact_node(&node).expect("upsert node");

    let stats = engine.fact_graph_stats().expect("graph stats");
    assert_eq!(stats.total_nodes, 1);
    assert_eq!(stats.node_counts_by_type.get("project"), Some(&1));
}

#[test]
fn test_duckdb_parquet_export() {
    let tmp = tempdir().expect("tempdir");
    let parquet_path = tmp.path().join("assets.parquet");

    let provider = DuckDbStorageProvider::open_in_memory().expect("open in memory");
    let asset = AssetRecord {
        id: "asset-parquet-1".to_string(),
        path: "/root/new/skills/duck-skill".to_string(),
        asset_type: "skill".to_string(),
        owner: "user:digoal".to_string(),
        security_level: "internal".to_string(),
        scan_tier: "deep".to_string(),
        description: Some("测试导出 Parquet 文件".to_string()),
        desc_kv: serde_json::json!({"format": "parquet"}),
        usage: None,
        size_bytes: Some(512),
        file_count: Some(2),
        tags: vec!["parquet".to_string()],
        status: "active".to_string(),
    };
    provider.upsert_asset(&asset).expect("upsert asset");

    let engine = DuckDbAnalyticsEngine::new(provider.conn());
    let summary = engine.export_to_parquet("assets", parquet_path.to_str().unwrap())
        .expect("export to parquet");

    assert_eq!(summary.table, "assets");
    assert_eq!(summary.row_count, 1);
    assert!(summary.file_size_bytes > 0, "导出的 Parquet 文件大小必须大于 0");
    assert!(parquet_path.exists(), "导出的 Parquet 文件必须在磁盘真实存在");
}

#[test]
fn test_storage_migration_sqlite_to_duckdb() {
    // 1. 创建源 SQLite
    let src = Storage::open_in_memory().expect("open sqlite in memory");
    let rule = PlacementRule {
        id: "rule-custom-1".to_string(),
        rule_name: "custom_rule".to_string(),
        pattern_type: "script".to_string(),
        target_base_dir: "/root/new/shared_proj/".to_string(),
        naming_pattern: Some("{name}/".to_string()),
        description: Some("自定义规则".to_string()),
        guideline: Some("无".to_string()),
        priority: 99,
    };
    src.insert_placement_rule(&rule).expect("insert rule to sqlite");

    let asset = AssetRecord {
        id: "asset-mig-1".to_string(),
        path: "/root/new/work/mig-test".to_string(),
        asset_type: "project".to_string(),
        owner: "user:digoal".to_string(),
        security_level: "internal".to_string(),
        scan_tier: "deep".to_string(),
        description: Some("迁移测试资产".to_string()),
        desc_kv: serde_json::json!({}),
        usage: None,
        size_bytes: Some(1024),
        file_count: Some(3),
        tags: vec!["mig".to_string()],
        status: "active".to_string(),
    };
    src.upsert_asset(&asset).expect("upsert asset to sqlite");

    // 2. 创建目标 DuckDB
    let dst_provider = DuckDbStorageProvider::open_in_memory().expect("open duckdb in memory");
    let dst = Storage::new(Arc::new(dst_provider.clone()));

    // 3. 执行热迁移
    let report = StorageMigrator::migrate(&src, &dst).expect("migrate sqlite -> duckdb");
    assert!(report.placement_rules_migrated >= 1);
    assert_eq!(report.assets_migrated, 1);

    // 4. 验证 DuckDB 中的数据
    let migrated_asset = dst.get_asset_by_path(&asset.path).expect("get").expect("found");
    assert_eq!(migrated_asset.id, asset.id);
    assert_eq!(migrated_asset.asset_type, "project");
}

#[test]
fn test_sync_pipeline_incremental() {
    let src = Storage::open_in_memory().expect("open sqlite");
    let dst_provider = DuckDbStorageProvider::open_in_memory().expect("open duckdb");

    let mem = MemoryRecord {
        id: "mem-sync-1".to_string(),
        memory_type: "experience".to_string(),
        content: "DuckDB 列存分析同步验证成功".to_string(),
        summary: Some("同步验证".to_string()),
        source_agent: Some("agent-antigravity".to_string()),
        source_session: Some("sess-1".to_string()),
        tags: vec!["sync".to_string()],
        weight: 1.0,
        pinned: false,
        status: "active".to_string(),
        embedding: vec![0.1, 0.2, 0.3],
    };
    src.insert_memory(&mem).expect("insert memory");

    let pipeline = SyncPipeline::new(src.provider().clone(), Arc::new(dst_provider.clone()));
    let rep = pipeline.sync_all().expect("sync all");
    assert_eq!(rep.memories_synced, 1);

    let search_res = dst_provider.search_memories_fts("列存分析", 5).expect("fts in duckdb");
    assert_eq!(search_res.len(), 1);
    assert_eq!(search_res[0].id, mem.id);
}
