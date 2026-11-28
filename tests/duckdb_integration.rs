use std::sync::Arc;
use tempfile::tempdir;
use agent_body::storage::{
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
    let recent_ts = (chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339();
    let old_ts = (chrono::Utc::now() - chrono::Duration::days(45)).to_rfc3339();

    // 插入测试变更记录（3 条近 1 天 + 1 条 45 天前历史记录）
    let changes = vec![
        ChangeRecord {
            id: None,
            change_type: "file_created".to_string(),
            target: "/root/new/work/proj_a/src/main.rs".to_string(),
            description: Some("create main".to_string()),
            details: serde_json::json!({}),
            source: "agent".to_string(),
            created_at: Some(recent_ts.clone()),
        },
        ChangeRecord {
            id: None,
            change_type: "file_modified".to_string(),
            target: "/root/new/work/proj_a/src/main.rs".to_string(),
            description: Some("edit main".to_string()),
            details: serde_json::json!({}),
            source: "agent".to_string(),
            created_at: Some(recent_ts.clone()),
        },
        ChangeRecord {
            id: None,
            change_type: "file_modified".to_string(),
            target: "/root/new/shared_proj/sec_scan/run.py".to_string(),
            description: Some("update scan".to_string()),
            details: serde_json::json!({}),
            source: "user".to_string(),
            created_at: Some(recent_ts),
        },
        ChangeRecord {
            id: None,
            change_type: "file_modified".to_string(),
            target: "/root/new/work/old_proj/legacy.rs".to_string(),
            description: Some("old edit beyond 30d window".to_string()),
            details: serde_json::json!({}),
            source: "agent".to_string(),
            created_at: Some(old_ts),
        },
    ];

    for c in &changes {
        provider.record_change(c).expect("record change");
    }

    let engine = DuckDbAnalyticsEngine::new(provider.conn());

    // 1. 变更热力图（30 天时间窗应过滤掉 45 天前记录）
    let heatmap = engine.changes_heatmap(30).expect("heatmap");
    assert!(!heatmap.is_empty(), "热力图应包含统计单元");
    let total_hits: usize = heatmap.iter().map(|h| h.change_count).sum();
    assert_eq!(total_hits, 3, "45 天前的变更记录不应进入 30 天热力图");

    // 2. 变更 Top-N（应过滤掉 45 天前记录）
    let top = engine.most_changed_assets(30, 5).expect("most changed");
    assert_eq!(top.len(), 2);
    assert_eq!(top[0].target, "/root/new/work/proj_a/src/main.rs");
    assert_eq!(top[0].change_count, 2);
    assert!(top.iter().all(|t| t.target != "/root/new/work/old_proj/legacy.rs"));

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
    assert_eq!(search_res[0].embedding, vec![0.1, 0.2, 0.3]);
}

#[test]
fn test_duckdb_provider_parity_rules_links_stale_emb_awr_sandbox() {
    use agent_body::storage::{
        AuditEventRecord, BenchmarkRunRecord, LinkRecord, ScanPolicy,
    };

    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("parity.duckdb");

    {
        let provider = DuckDbStorageProvider::open(&db_path).expect("open 1");
        // 自定义覆写 rule-1 与 policy-1
        provider
            .insert_placement_rule(&PlacementRule {
                id: "rule-1".to_string(),
                rule_name: "shared_tools".to_string(),
                pattern_type: "script".to_string(),
                target_base_dir: "/custom/shared_proj/".to_string(),
                naming_pattern: Some("{name}/".to_string()),
                description: Some("custom desc".to_string()),
                guideline: None,
                priority: 5,
            })
            .expect("custom rule");
        provider
            .upsert_scan_policy(&ScanPolicy {
                id: "policy-1".to_string(),
                path_pattern: "/root/new/shared_proj/**".to_string(),
                tier: "restricted".to_string(),
                max_depth: 3,
                enable_fts: true,
                enable_vec: false,
                enable_graph: true,
                notes: Some("user tuned".to_string()),
            })
            .expect("custom policy");
    }

    // 再次打开同一磁盘 DuckDB（重跑 init），用户自定义的 rule-1 和 policy-1 绝不能被覆写
    let provider = DuckDbStorageProvider::open(&db_path).expect("reopen");
    let rules = provider.get_placement_rules().expect("rules");
    let r1 = rules.iter().find(|r| r.rule_name == "shared_tools").expect("rule-1");
    assert_eq!(r1.target_base_dir, "/custom/shared_proj/");
    let policies = provider.get_scan_policies().expect("policies");
    let p1 = policies
        .iter()
        .find(|p| p.path_pattern == "/root/new/shared_proj/**")
        .expect("policy-1");
    assert_eq!(p1.tier, "restricted");
    assert_eq!(p1.max_depth, 3);

    // add_link 去重（同一五元组两次写入合并为 1 条）
    let link = LinkRecord {
        id: None,
        source_type: "asset".to_string(),
        source_id: "/a.rs".to_string(),
        target_type: "asset".to_string(),
        target_id: "/b.rs".to_string(),
        relation: "depends_on".to_string(),
        confidence: 0.7,
        auto_discovered: true,
        line_number: Some(10),
        snippet: Some("use b;".to_string()),
        provenance: Some("static_scan".to_string()),
        created_at: None,
    };
    provider.add_link(&link).expect("link 1");
    let mut link2 = link.clone();
    link2.confidence = 0.99;
    provider.add_link(&link2).expect("link 2");
    let all_links = provider.list_all_links(20).expect("all links");
    assert_eq!(all_links.len(), 1);
    assert!((all_links[0].confidence - 0.99).abs() < 1e-4);

    // upsert_asset stale 清空向量
    let asset = AssetRecord {
        id: "duck-stale-1".to_string(),
        path: "/root/new/work/duck_stale".to_string(),
        asset_type: "project".to_string(),
        owner: "user:digoal".to_string(),
        security_level: "internal".to_string(),
        scan_tier: "deep".to_string(),
        description: Some("desc".to_string()),
        desc_kv: serde_json::json!({}),
        usage: None,
        size_bytes: None,
        file_count: None,
        tags: vec![],
        status: "active".to_string(),
    };
    provider.upsert_asset(&asset).expect("upsert");
    provider
        .update_asset_embedding(&asset.path, &[0.3, 0.7])
        .expect("update emb");
    let mut asset_stale = asset.clone();
    asset_stale.desc_kv = serde_json::json!({"embedding_status": "stale"});
    provider.upsert_asset(&asset_stale).expect("upsert stale");
    assert!(provider.get_asset_embedding(&asset.path).expect("emb").is_empty());

    // benchmark_runs awr:% 过滤与 delete_bench_sandbox_records 清理 audit_events
    let awr = BenchmarkRunRecord {
        run_id: "awr:snap:duck-1".to_string(),
        suite_name: "awr_internal".to_string(),
        git_commit: "c".to_string(),
        app_version: "0.4.0".to_string(),
        host_fingerprint: serde_json::json!({}),
        workload_count: 0,
        total_baseline_tokens: 0,
        total_body_tokens: 0,
        overall_token_savings_pct: 0.0,
        overall_speedup_factor: 1.0,
        total_hazards_intercepted: 0,
        total_noise_blocked: 0,
        sla_passed: true,
        results_json: serde_json::json!({}),
        report_sha256: "awr".to_string(),
        duration_ms: 0,
        created_at: chrono::Utc::now().to_rfc3339(),
    };
    provider.insert_benchmark_run(&awr).expect("insert awr");
    assert!(provider.list_benchmark_runs(10).expect("list").is_empty());
    assert_eq!(provider.list_all_benchmark_runs(10).expect("list all").len(), 1);
    assert!(provider.delete_benchmark_run("awr:snap:duck-1").expect("del awr"));

    provider
        .insert_audit_event(&AuditEventRecord {
            event_id: "aud-bench-1".to_string(),
            trace_id: "tr-b".to_string(),
            session_id: "bench:sandbox-session".to_string(),
            domain: "IN_AGENT".to_string(),
            actor_kind: "mcp_client".to_string(),
            actor_id: "bench".to_string(),
            source_channel: "bench".to_string(),
            category: "tool_call".to_string(),
            action: "bench_act".to_string(),
            target_type: "asset".to_string(),
            target_id: "bench:t1".to_string(),
            concurrency_mode: None,
            outcome: "success".to_string(),
            before_after_delta: serde_json::json!({}),
            elapsed_us: 100,
            evidence_sha256: "a".repeat(64),
            created_at: chrono::Utc::now().to_rfc3339(),
        })
        .expect("insert bench audit");
    assert_eq!(provider.delete_bench_sandbox_records().expect("del sandbox"), 1);
    assert!(
        provider
            .query_audit_events(&Default::default())
            .expect("query")
            .is_empty()
    );
}

#[test]
fn test_sync_pipeline_timestamps_all_tables_olap_and_export() {
    use agent_body::storage::{
        ActiveProcessRecord, AuditEventRecord, BodyCheckpointRecord, LinkRecord,
        SqliteStorageProvider,
    };

    let sqlite_prov = Arc::new(SqliteStorageProvider::open_in_memory().expect("sqlite mem"));
    let src = Storage::new(sqlite_prov.clone());

    let asset_recent = AssetRecord {
        id: "a-recent".to_string(),
        path: "/root/new/work/recent_proj".to_string(),
        asset_type: "project".to_string(),
        owner: "user:digoal".to_string(),
        security_level: "internal".to_string(),
        scan_tier: "deep".to_string(),
        description: Some("recent".to_string()),
        desc_kv: serde_json::json!({}),
        usage: None,
        size_bytes: Some(10),
        file_count: Some(1),
        tags: vec![],
        status: "active".to_string(),
    };
    let asset_old = AssetRecord {
        id: "a-old".to_string(),
        path: "/root/new/work/old_proj_40d".to_string(),
        ..asset_recent.clone()
    };
    src.upsert_asset(&asset_recent).expect("upsert recent");
    src.upsert_asset(&asset_old).expect("upsert old");

    let mem_archived_old = MemoryRecord {
        id: "m-old-arch".to_string(),
        memory_type: "fact".to_string(),
        content: "40 天前已归档的历史记忆".to_string(),
        summary: None,
        source_agent: None,
        source_session: None,
        tags: vec![],
        weight: 0.5,
        pinned: false,
        status: "archived".to_string(),
        embedding: vec![0.4, 0.6],
    };
    src.insert_memory(&mem_archived_old).expect("insert mem");

    let old_iso = (chrono::Utc::now() - chrono::Duration::days(40)).to_rfc3339();
    {
        let conn_arc = sqlite_prov.conn();
        let conn = conn_arc.lock().expect("lock sqlite");
        conn.execute(
            "UPDATE assets SET created_at = ?1 WHERE id = 'a-old'",
            rusqlite::params![old_iso],
        )
        .expect("backdate asset");
        conn.execute(
            "UPDATE memories SET created_at = ?1 WHERE id = 'm-old-arch'",
            rusqlite::params![old_iso],
        )
        .expect("backdate memory");
    }

    src.add_link(&LinkRecord {
        id: None,
        source_type: "asset".to_string(),
        source_id: asset_recent.path.clone(),
        target_type: "asset".to_string(),
        target_id: asset_old.path.clone(),
        relation: "references".to_string(),
        confidence: 0.9,
        auto_discovered: true,
        line_number: Some(12),
        snippet: None,
        provenance: Some("static_scan".to_string()),
        created_at: None,
    })
    .expect("add link");
    src.replace_active_processes(&[ActiveProcessRecord {
        id: "proc-1".to_string(),
        pid: 4242,
        name: "worker".to_string(),
        command: "worker --port 8080".to_string(),
        owner: "digoal".to_string(),
        associated_path: Some("/root/new/work/recent_proj".to_string()),
        listening_ports: vec![8080],
        status: "running".to_string(),
        cpu_pct: 1.5,
        mem_bytes: 1024 * 1024,
        gpu_allocated_mb: 0,
        started_at: None,
    }])
    .expect("replace procs");
    src.insert_body_checkpoint(&BodyCheckpointRecord {
        checkpoint_id: "cp-sync-1".to_string(),
        body_id: "body-1".to_string(),
        from_brain: "codex".to_string(),
        session_id: "sess-cp".to_string(),
        goal_summary: "verify sync".to_string(),
        active_files: vec!["src/main.rs".to_string()],
        active_locks: vec![],
        verified_facts: vec![],
        open_blockers: vec![],
        next_step_hint: "next".to_string(),
        token_budget: 200,
        status: "active".to_string(),
        evidence_sha256: "c".repeat(64),
        created_at: chrono::Utc::now().to_rfc3339(),
        resumed_by_brain: None,
        resumed_at: None,
        tx_epoch: 0,
    })
    .expect("insert cp");

    for us in [100i64, 200, 300, 900] {
        src.insert_audit_event(&AuditEventRecord {
            event_id: format!("aud-{}", us),
            trace_id: "tr-sync".to_string(),
            session_id: "sess-1".to_string(),
            domain: "IN_AGENT".to_string(),
            actor_kind: "mcp_client".to_string(),
            actor_id: "agent".to_string(),
            source_channel: "mcp".to_string(),
            category: "tool_call".to_string(),
            action: "body_brief".to_string(),
            target_type: "host".to_string(),
            target_id: "local".to_string(),
            concurrency_mode: None,
            outcome: "success".to_string(),
            before_after_delta: serde_json::json!({}),
            elapsed_us: us,
            evidence_sha256: "d".repeat(64),
            created_at: chrono::Utc::now().to_rfc3339(),
        })
        .expect("insert audit");
    }

    let duck_prov = DuckDbStorageProvider::open_in_memory().expect("duckdb");
    let pipeline = SyncPipeline::new(src.provider().clone(), Arc::new(duck_prov.clone()));
    let rep = pipeline.sync_all().expect("sync all");
    assert_eq!(rep.assets_synced, 2);
    assert_eq!(rep.memories_synced, 1);
    assert_eq!(rep.links_synced, 1);
    assert_eq!(rep.active_processes_synced, 1);
    assert_eq!(rep.body_checkpoints_synced, 1);
    assert_eq!(rep.audit_events_synced, 4);

    // 验证 sync_watermarks 已更新
    {
        let conn_arc = duck_prov.conn();
        let conn = conn_arc.lock().expect("lock duck");
        let wm_cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM sync_watermarks", [], |r| r.get(0))
            .expect("watermarks");
        assert_eq!(wm_cnt, 10);
    }

    let analytics = DuckDbAnalyticsEngine::new(duck_prov.conn());
    // 7 天增长趋势仅包含 recent_proj
    let trend = analytics.asset_growth_trend(7).expect("trend 7d");
    let trend_total: usize = trend.iter().map(|t| t.daily_count).sum();
    assert_eq!(trend_total, 1, "40 天前资产应被 7 天趋势窗排除");

    // 记忆衰减曲线必须反映真实账龄 (>= 10 天) 与 archived 状态
    let decay = analytics.memory_decay_curve().expect("decay curve");
    assert_eq!(decay.len(), 1);
    assert!(decay[0].age_days >= 10, "实测 age_days={}", decay[0].age_days);
    assert_eq!(decay[0].archived_count, 1);

    // 审计 OLAP 包含真实 P50 / P95 分位
    let audit_rep = analytics.audit_analytics_summary(7).expect("audit summary");
    assert_eq!(audit_rep.top_actions.len(), 1);
    assert!(audit_rep.top_actions[0].p50_elapsed_us > 0.0);
    assert!(audit_rep.top_actions[0].p95_elapsed_us >= audit_rep.top_actions[0].p50_elapsed_us);

    // links / active_processes / body_checkpoints 原生导出不再 0 行
    let tmp = tempdir().expect("tempdir");
    for tbl in ["links", "active_processes", "body_checkpoints"] {
        let out = tmp.path().join(format!("{}.csv", tbl));
        let exp = analytics
            .export_to_parquet(tbl, &out.to_string_lossy())
            .expect("export");
        assert_eq!(exp.row_count, 1, "table {} should export 1 row", tbl);
        assert!(out.exists());
    }
}

#[test]
fn test_storage_migration_full_20_entities_and_idempotent() {
    use agent_body::storage::{
        AuditEventRecord, BenchmarkRunRecord, BodyCheckpointRecord, CoordinationLock,
        EffectivenessLedgerRecord, FactEdgeRecord, LinkRecord, LockWaitEntry, McpCatalogRecord,
        MeshDialogueRecord, MeshTransferRecord, ScanJobRecord, ScanPolicy, SkillCatalogRecord,
        ExternalServiceRecord,
    };

    let src = Storage::open_in_memory().expect("src sqlite");
    let now = chrono::Utc::now().to_rfc3339();
    let future = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();

    src.insert_placement_rule(&PlacementRule {
        id: "rule-mig-custom".to_string(),
        rule_name: "mig_custom_rule".to_string(),
        pattern_type: "script".to_string(),
        target_base_dir: "/root/new/shared_proj/".to_string(),
        naming_pattern: Some("{name}".to_string()),
        description: Some("custom".to_string()),
        guideline: None,
        priority: 15,
    })
    .expect("rule");
    src.upsert_scan_policy(&ScanPolicy {
        id: "policy-mig-custom".to_string(),
        path_pattern: "/root/new/custom_zone/**".to_string(),
        tier: "deep".to_string(),
        max_depth: 5,
        enable_fts: true,
        enable_vec: true,
        enable_graph: false,
        notes: Some("mig note".to_string()),
    })
    .expect("policy");

    let asset = AssetRecord {
        id: "mig-a-1".to_string(),
        path: "/root/new/work/mig_full_asset".to_string(),
        asset_type: "project".to_string(),
        owner: "user:digoal".to_string(),
        security_level: "internal".to_string(),
        scan_tier: "deep".to_string(),
        description: Some("full asset".to_string()),
        desc_kv: serde_json::json!({"k": "v"}),
        usage: None,
        size_bytes: Some(512),
        file_count: Some(2),
        tags: vec!["m".to_string()],
        status: "active".to_string(),
    };
    src.upsert_asset(&asset).expect("asset");
    src.update_asset_embedding(&asset.path, &[0.11, 0.22, 0.33])
        .expect("asset emb");

    src.replace_active_processes(&[agent_body::storage::ActiveProcessRecord {
        id: "mig-p-1".to_string(),
        pid: 7777,
        name: "daemon".to_string(),
        command: "daemon".to_string(),
        owner: "digoal".to_string(),
        associated_path: None,
        listening_ports: vec![8765],
        status: "running".to_string(),
        cpu_pct: 0.5,
        mem_bytes: 1024,
        gpu_allocated_mb: 0,
        started_at: Some(now.clone()),
    }])
    .expect("proc");

    src.insert_memory(&MemoryRecord {
        id: "mig-mem-arch".to_string(),
        memory_type: "note".to_string(),
        content: "archived mig memory".to_string(),
        summary: None,
        source_agent: Some("agent".to_string()),
        source_session: Some("sess".to_string()),
        tags: vec!["arch".to_string()],
        weight: 0.4,
        pinned: false,
        status: "archived".to_string(),
        embedding: vec![0.44, 0.55],
    })
    .expect("mem");

    src.record_change(&ChangeRecord {
        id: None,
        change_type: "file_created".to_string(),
        target: asset.path.clone(),
        description: Some("created".to_string()),
        details: serde_json::json!({}),
        source: "test".to_string(),
        created_at: Some(now.clone()),
    })
    .expect("change");

    src.add_link(&LinkRecord {
        id: None,
        source_type: "asset".to_string(),
        source_id: asset.path.clone(),
        target_type: "service".to_string(),
        target_id: "pg:5432".to_string(),
        relation: "depends_on".to_string(),
        confidence: 1.0,
        auto_discovered: false,
        line_number: Some(1),
        snippet: None,
        provenance: Some("manual".to_string()),
        created_at: Some(now.clone()),
    })
    .expect("link");

    src.create_scan_job(&ScanJobRecord {
        id: "job-mig-1".to_string(),
        root_path: "/root/new/work".to_string(),
        status: "completed".to_string(),
        last_path: Some(asset.path.clone()),
        scanned_dirs: 1,
        scanned_files: 2,
        total_assets: 1,
        shallow_blocked: 0,
        error_message: None,
        started_at: Some(now.clone()),
        paused_at: None,
        completed_at: Some(now.clone()),
        created_at: Some(now.clone()),
        updated_at: Some(now.clone()),
    })
    .expect("job");

    src.upsert_fact_node(&FactNodeRecord {
        id: "proj:mig_1".to_string(),
        node_type: "project".to_string(),
        name: "mig_1".to_string(),
        label: "Mig1".to_string(),
        properties: serde_json::json!({}),
        updated_at: Some(now.clone()),
    })
    .expect("node1");
    src.update_fact_node_embedding("proj:mig_1", &[0.7, 0.8])
        .expect("node emb");
    src.upsert_fact_node(&FactNodeRecord {
        id: "port:8765".to_string(),
        node_type: "port".to_string(),
        name: "8765".to_string(),
        label: "8765".to_string(),
        properties: serde_json::json!({}),
        updated_at: Some(now.clone()),
    })
    .expect("node2");
    src.upsert_fact_edge(&FactEdgeRecord::new_explicit(
        "proj:mig_1",
        "port:8765",
        "listens_on",
        serde_json::json!({}),
        1.0,
    ))
    .expect("edge");

    assert!(src
        .insert_coordination_lock(&CoordinationLock {
            lock_id: "lock-mig-1".to_string(),
            resource_type: "directory".to_string(),
            resource_id: asset.path.clone(),
            scope: "exclusive".to_string(),
            holder_agent: "agent-1".to_string(),
            holder_pid: Some(111),
            acquired_at: now.clone(),
            expires_at: future.clone(),
            metadata: None,
        })
        .expect("lock"));
    src.enqueue_lock_wait(&LockWaitEntry {
        wait_id: "wait-mig-1".to_string(),
        resource_type: "directory".to_string(),
        resource_id: asset.path.clone(),
        scope: "exclusive".to_string(),
        waiting_agent: "agent-2".to_string(),
        priority: 10,
        enqueued_at: now.clone(),
        timeout_at: future,
        status: "waiting".to_string(),
    })
    .expect("wait");

    src.upsert_mcp_catalog(&McpCatalogRecord {
        id: "mcp-mig-1".to_string(),
        server_name: "mig-mcp".to_string(),
        client_source: "agy".to_string(),
        transport: "stdio".to_string(),
        command_or_url: "agent-body mcp".to_string(),
        tools_json: serde_json::json!([]),
        tool_count: 1,
        est_total_tokens: 50,
        status: "active".to_string(),
        last_scanned_at: Some(now.clone()),
    })
    .expect("mcp");
    src.upsert_skill_catalog(&SkillCatalogRecord {
        id: "skill-mig-1".to_string(),
        skill_name: "mig-skill".to_string(),
        root_source: "local".to_string(),
        skill_path: "/root/new/skills/mig-skill".to_string(),
        description: "mig".to_string(),
        triggers: vec!["mig".to_string()],
        est_tokens: 30,
        has_scripts: false,
        has_references: false,
        status: "active".to_string(),
        last_scanned_at: Some(now.clone()),
    })
    .expect("skill");
    src.upsert_external_service(&ExternalServiceRecord {
        id: "svc-mig-1".to_string(),
        service_name: "mig-svc".to_string(),
        category: "llm".to_string(),
        endpoint: "http://127.0.0.1:11434".to_string(),
        protocol: "http".to_string(),
        capabilities: "embed".to_string(),
        auth_type: "none".to_string(),
        auth_env_var: None,
        health_endpoint: None,
        usage_example: None,
        status: "up".to_string(),
        last_checked_at: Some(now.clone()),
    })
    .expect("svc");

    src.insert_benchmark_run(&BenchmarkRunRecord {
        run_id: "awr:snap:mig-1".to_string(),
        suite_name: "awr_internal".to_string(),
        git_commit: "g".to_string(),
        app_version: "0.4.0".to_string(),
        host_fingerprint: serde_json::json!({}),
        workload_count: 0,
        total_baseline_tokens: 0,
        total_body_tokens: 0,
        overall_token_savings_pct: 0.0,
        overall_speedup_factor: 1.0,
        total_hazards_intercepted: 0,
        total_noise_blocked: 0,
        sla_passed: true,
        results_json: serde_json::json!({}),
        report_sha256: "awr".to_string(),
        duration_ms: 0,
        created_at: now.clone(),
    })
    .expect("awr run");
    src.insert_effectiveness_event(&EffectivenessLedgerRecord {
        event_id: "eff-mig-1".to_string(),
        session_id: "s".to_string(),
        source_channel: "mcp".to_string(),
        tool_or_action: "body_brief".to_string(),
        provenance_rule: "PR-01".to_string(),
        baseline_tokens: 100,
        body_tokens: 20,
        tokens_saved: 80,
        baseline_rounds: 2,
        body_rounds: 1,
        rounds_saved: 1,
        hazard_intercepted: 0,
        noise_blocked: 0,
        elapsed_us: 10,
        evidence_summary: "ok".to_string(),
        created_at: now.clone(),
    })
    .expect("eff");
    src.insert_mesh_dialogue(&MeshDialogueRecord {
        dialogue_id: "dlg-mig-1".to_string(),
        thread_id: "th-1".to_string(),
        from_node: "n1".to_string(),
        from_agent: "a1".to_string(),
        to_node: "n2".to_string(),
        to_agent: "a2".to_string(),
        intent: "ASK_CONTEXT".to_string(),
        content: "hi".to_string(),
        redaction_hits: 0,
        injection_flagged: false,
        verified_signature: true,
        status: "received".to_string(),
        created_at: now.clone(),
    })
    .expect("dlg");
    src.insert_mesh_transfer(&MeshTransferRecord {
        transfer_id: "tr-mig-1".to_string(),
        from_node: "n1".to_string(),
        from_agent: "a1".to_string(),
        to_node: "n2".to_string(),
        file_name: "notes.md".to_string(),
        size_bytes: 12,
        sha256: "e".repeat(64),
        inbox_path: "/tmp/notes.md".to_string(),
        secret_warnings: 0,
        verified_signature: true,
        status: "quarantined".to_string(),
        accepted_path: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    })
    .expect("transfer");
    src.insert_audit_event(&AuditEventRecord {
        event_id: "aud-mig-1".to_string(),
        trace_id: "tr-mig".to_string(),
        session_id: "s".to_string(),
        domain: "IN_AGENT".to_string(),
        actor_kind: "mcp_client".to_string(),
        actor_id: "a".to_string(),
        source_channel: "mcp".to_string(),
        category: "tool_call".to_string(),
        action: "body_brief".to_string(),
        target_type: "host".to_string(),
        target_id: "local".to_string(),
        concurrency_mode: None,
        outcome: "success".to_string(),
        before_after_delta: serde_json::json!({}),
        elapsed_us: 15,
        evidence_sha256: "f".repeat(64),
        created_at: now.clone(),
    })
    .expect("audit");
    src.insert_body_checkpoint(&BodyCheckpointRecord {
        checkpoint_id: "cp-mig-1".to_string(),
        body_id: "b1".to_string(),
        from_brain: "agy".to_string(),
        session_id: "s".to_string(),
        goal_summary: "g".to_string(),
        active_files: vec![],
        active_locks: vec![],
        verified_facts: vec![],
        open_blockers: vec![],
        next_step_hint: "".to_string(),
        token_budget: 200,
        status: "active".to_string(),
        evidence_sha256: "a".repeat(64),
        created_at: now,
        resumed_by_brain: None,
        resumed_at: None,
        tx_epoch: 0,
    })
    .expect("cp");

    let dst_prov = DuckDbStorageProvider::open_in_memory().expect("duckdb dst");
    let dst = Storage::new(Arc::new(dst_prov));

    // 执行两次迁移，验证全 20 类实体无损迁移与幂等可重入
    let rep1 = StorageMigrator::migrate(&src, &dst).expect("migrate 1");
    let rep2 = StorageMigrator::migrate(&src, &dst).expect("migrate 2 (idempotent)");
    assert_eq!(rep1.asset_embeddings_migrated, 1);
    assert_eq!(rep1.fact_node_embeddings_migrated, 1);
    assert_eq!(rep1.coordination_locks_migrated, 1);
    assert_eq!(rep1.lock_wait_queue_migrated, 1);
    assert_eq!(rep1.mcp_catalog_migrated, 1);
    assert_eq!(rep1.skills_catalog_migrated, 1);
    assert_eq!(rep1.external_services_migrated, 1);
    assert_eq!(rep1.benchmark_runs_migrated, 1);
    assert_eq!(rep1.effectiveness_ledger_migrated, 1);
    assert_eq!(rep1.mesh_dialogues_migrated, 1);
    assert_eq!(rep1.mesh_transfers_migrated, 1);
    assert_eq!(rep2.assets_migrated, rep1.assets_migrated);

    // 校验目标端全部实体的真实还原度
    assert!(dst
        .get_scan_policies()
        .expect("policies")
        .iter()
        .any(|p| p.path_pattern == "/root/new/custom_zone/**"));
    assert_eq!(
        dst.get_asset_embedding(&asset.path).expect("asset emb"),
        vec![0.11, 0.22, 0.33]
    );
    assert_eq!(
        dst.get_fact_node_embedding("proj:mig_1").expect("node emb"),
        vec![0.7, 0.8]
    );
    assert_eq!(dst.list_all_memories(10).expect("arch mem").len(), 1);
    assert_eq!(dst.list_all_links(10).expect("links").len(), 1);
    assert_eq!(dst.list_coordination_locks(None).expect("locks").len(), 1);
    assert_eq!(dst.list_lock_wait_queue(None, None).expect("waits").len(), 1);
    assert_eq!(dst.list_mcp_catalog().expect("mcps").len(), 1);
    assert_eq!(dst.list_skills_catalog().expect("skills").len(), 1);
    assert_eq!(dst.list_external_services(None).expect("svcs").len(), 1);
    assert_eq!(dst.list_all_benchmark_runs(10).expect("awr migrated").len(), 1);
    assert_eq!(dst.list_mesh_dialogues(10).expect("dlgs").len(), 1);
    assert_eq!(dst.list_mesh_transfers(10).expect("trs").len(), 1);
    assert_eq!(dst.list_body_checkpoints(None, 10).expect("cps").len(), 1);
}
