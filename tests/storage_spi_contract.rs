use agent_body::storage::{
    ActiveProcessRecord, AssetRecord, AuditEventRecord, AuditQueryFilter, BenchmarkRunRecord,
    BodyCheckpointRecord, ChangeRecord, CoordinationLock, DuckDbStorageProvider,
    EffectivenessLedgerRecord, ExternalServiceRecord, FactEdgeRecord, FactNodeRecord, LinkRecord,
    LockWaitEntry, McpCatalogRecord, MemoryRecord, MeshDialogueRecord, MeshTransferRecord,
    PlacementRule, ScanJobRecord, ScanPolicy, SkillCatalogRecord, SqliteStorageProvider,
    StorageProvider,
};

fn run_shared_spi_contract(p: &dyn StorageProvider, expected_name: &str) {
    p.init().expect("spi init");
    assert_eq!(p.provider_name(), expected_name);

    let now_iso = chrono::Utc::now().to_rfc3339();
    let future_iso = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    let expired_iso = (chrono::Utc::now() - chrono::Duration::hours(1)).to_rfc3339();

    // 1. Placement rules & scan policies
    let rules = p.get_placement_rules().expect("get_placement_rules");
    assert!(!rules.is_empty(), "init 应预置默认放置规则");
    p.insert_placement_rule(&PlacementRule {
        id: "rule-contract-1".into(),
        rule_name: "contract_tmp".into(),
        pattern_type: "tmp".into(),
        target_base_dir: "/tmp/contract".into(),
        naming_pattern: Some("{name}".into()),
        description: Some("contract test rule".into()),
        guideline: Some("keep isolated".into()),
        priority: 99,
    })
    .expect("insert_placement_rule");
    assert!(p
        .get_placement_rules()
        .expect("re-read placement rules")
        .iter()
        .any(|r| r.id == "rule-contract-1"));

    p.upsert_scan_policy(&ScanPolicy {
        id: "pol-contract-1".into(),
        path_pattern: "**/contract_ignore/**".into(),
        tier: "shallow".into(),
        max_depth: 1,
        enable_fts: false,
        enable_vec: false,
        enable_graph: false,
        notes: Some("contract ignore".into()),
    })
    .expect("upsert_scan_policy");
    assert!(p
        .get_scan_policies()
        .expect("get_scan_policies")
        .iter()
        .any(|sp| sp.id == "pol-contract-1"));

    // 2. Assets + FTS + Embeddings
    let asset = AssetRecord {
        id: "asset-contract-1".into(),
        path: "/workspace/contract_demo/app.rs".into(),
        asset_type: "script".into(),
        owner: "agent-body".into(),
        security_level: "internal".into(),
        scan_tier: "deep".into(),
        description: Some("SQLite DuckDB shared contract probe".into()),
        desc_kv: serde_json::json!({"schema_version": "2.0", "modality": "code"}),
        usage: Some("verify SPI parity".into()),
        size_bytes: Some(512),
        file_count: Some(1),
        tags: vec!["contract".into(), "spi".into()],
        status: "active".into(),
    };
    p.upsert_asset(&asset).expect("upsert_asset");
    let got_asset = p
        .get_asset_by_path(&asset.path)
        .expect("get_asset_by_path")
        .expect("asset exists");
    assert_eq!(got_asset.id, asset.id);
    assert!(p
        .list_assets(20)
        .expect("list_assets")
        .iter()
        .any(|a| a.id == asset.id));
    assert!(p
        .search_assets_fts("shared contract", 10)
        .expect("search_assets_fts")
        .iter()
        .any(|a| a.id == asset.id));
    p.update_asset_embedding(&asset.path, &[0.25, 0.5, 0.75])
        .expect("update_asset_embedding");
    let a_emb = p.get_asset_embedding(&asset.path).expect("get_asset_embedding");
    assert_eq!(a_emb.len(), 3);

    // 3. Active processes & Changes
    p.replace_active_processes(&[ActiveProcessRecord {
        id: "proc-contract-1".into(),
        pid: 43210,
        name: "contract-worker".into(),
        command: "contract-worker --port 9876".into(),
        owner: "tester".into(),
        associated_path: Some(asset.path.clone()),
        listening_ports: vec![9876],
        status: "running".into(),
        cpu_pct: 0.5,
        mem_bytes: 1024 * 1024,
        gpu_allocated_mb: 0,
        started_at: Some(now_iso.clone()),
    }])
    .expect("replace_active_processes");
    assert_eq!(p.list_active_processes().expect("list_active_processes").len(), 1);

    p.record_change(&ChangeRecord {
        id: None,
        change_type: "file_created".into(),
        target: asset.path.clone(),
        description: Some("created in contract".into()),
        details: serde_json::json!({"bytes": 512}),
        source: "contract_test".into(),
        created_at: Some(now_iso.clone()),
    })
    .expect("record_change");
    assert!(!p.list_changes(10).expect("list_changes").is_empty());

    // 4. Memories + FTS + Status transition
    let mem = MemoryRecord {
        id: "mem-contract-1".into(),
        memory_type: "decision".into(),
        content: "SQLite and DuckDB share identical SPI contract invariants".into(),
        summary: Some("SPI parity invariant".into()),
        source_agent: Some("tester".into()),
        source_session: Some("sess-contract".into()),
        tags: vec!["contract".into()],
        weight: 0.9,
        pinned: true,
        status: "active".into(),
        embedding: vec![0.1, 0.2, 0.3],
    };
    p.insert_memory(&mem).expect("insert_memory");
    assert!(p
        .search_memories_fts("contract invariants", 10)
        .expect("search_memories_fts")
        .iter()
        .any(|m| m.id == mem.id));
    assert_eq!(p.update_memory_status(&mem.id, "archived").expect("archive mem"), 1);
    assert!(!p
        .list_all_active_memories(50)
        .expect("list_all_active_memories")
        .iter()
        .any(|m| m.id == mem.id));
    assert!(p
        .list_all_memories(50)
        .expect("list_all_memories")
        .iter()
        .any(|m| m.id == mem.id));

    // 5. Links & Scan jobs
    p.add_link(&LinkRecord {
        id: None,
        source_type: "asset".into(),
        source_id: "src-node".into(),
        target_type: "asset".into(),
        target_id: "old-target".into(),
        relation: "references".into(),
        confidence: 0.95,
        auto_discovered: true,
        line_number: Some(12),
        snippet: Some("use old_target".into()),
        provenance: Some("static".into()),
        created_at: Some(now_iso.clone()),
    })
    .expect("add_link");
    assert_eq!(
        p.update_link_target("src-node", "old-target", "new-target")
            .expect("update_link_target"),
        1
    );
    let links = p.query_links_for_node("asset", "src-node").expect("query_links_for_node");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].target_id, "new-target");
    if let Some(lid) = links[0].id {
        p.delete_link(lid).expect("delete_link");
    }

    let mut job = ScanJobRecord {
        id: "job-contract-1".into(),
        root_path: "/workspace/contract_demo".into(),
        status: "running".into(),
        last_path: None,
        scanned_dirs: 2,
        scanned_files: 10,
        total_assets: 10,
        shallow_blocked: 1,
        error_message: None,
        started_at: Some(now_iso.clone()),
        paused_at: None,
        completed_at: None,
        created_at: Some(now_iso.clone()),
        updated_at: Some(now_iso.clone()),
    };
    p.create_scan_job(&job).expect("create_scan_job");
    assert!(p.get_active_scan_job().expect("get_active_scan_job").is_some());
    job.status = "completed".into();
    job.completed_at = Some(now_iso.clone());
    p.update_scan_job(&job).expect("update_scan_job");
    assert_eq!(
        p.get_scan_job(&job.id).expect("get_scan_job").expect("job").status,
        "completed"
    );
    assert!(!p.list_scan_jobs(10).expect("list_scan_jobs").is_empty());

    // 6. Fact graph + embeddings + filtered traversal + deletion
    p.upsert_fact_node(&FactNodeRecord {
        id: "fn:root".into(),
        node_type: "project".into(),
        name: "contract_root".into(),
        label: "Contract Root".into(),
        properties: serde_json::json!({}),
        updated_at: Some(now_iso.clone()),
    })
    .expect("upsert root node");
    p.upsert_fact_node(&FactNodeRecord {
        id: "fn:child".into(),
        node_type: "service".into(),
        name: "contract_child".into(),
        label: "Contract Child".into(),
        properties: serde_json::json!({}),
        updated_at: Some(now_iso.clone()),
    })
    .expect("upsert child node");
    p.upsert_fact_edge(&FactEdgeRecord::new_explicit(
        "fn:root",
        "fn:child",
        "depends_on",
        serde_json::json!({}),
        0.9,
    ))
    .expect("upsert edge");
    assert!(!p.find_fact_nodes_by_name("contract_root").expect("find_fact_nodes_by_name").is_empty());
    let sub = p.traverse_fact_graph("fn:root", 2).expect("traverse_fact_graph");
    assert_eq!(sub.edges.len(), 1);
    let sub_filtered = p
        .traverse_fact_graph_filtered("fn:root", 2, 0.8, Some(&["L1_EXPLICIT"]))
        .expect("traverse_fact_graph_filtered");
    assert_eq!(sub_filtered.edges.len(), 1);
    p.update_fact_node_embedding("fn:root", &[0.5, 0.5])
        .expect("update_fact_node_embedding");
    assert_eq!(p.get_fact_node_embedding("fn:root").expect("get_fact_node_embedding").len(), 2);
    assert_eq!(p.delete_fact_edges_by_tier("L1_EXPLICIT").expect("delete_fact_edges_by_tier"), 1);
    assert_eq!(p.delete_fact_node("fn:child").expect("delete_fact_node"), 1);

    // 7. Coordination locks & wait queue
    let active_lock = CoordinationLock {
        lock_id: "lock-active-1".into(),
        resource_type: "port".into(),
        resource_id: "18080".into(),
        scope: "exclusive".into(),
        holder_agent: "agent-a".into(),
        holder_pid: Some(1111),
        acquired_at: now_iso.clone(),
        expires_at: future_iso.clone(),
        metadata: None,
    };
    let expired_lock = CoordinationLock {
        lock_id: "lock-exp-1".into(),
        resource_type: "port".into(),
        resource_id: "18081".into(),
        scope: "exclusive".into(),
        holder_agent: "agent-b".into(),
        holder_pid: Some(2222),
        acquired_at: expired_iso.clone(),
        expires_at: expired_iso.clone(),
        metadata: None,
    };
    assert!(p.insert_coordination_lock(&active_lock).expect("insert active lock"));
    assert!(p.insert_coordination_lock(&expired_lock).expect("insert expired lock"));
    assert!(p.count_expired_coordination_locks().expect("count expired") >= 1);
    assert!(p.gc_expired_coordination_locks().expect("gc expired") >= 1);
    assert!(p.get_coordination_lock("port", "18080").expect("get lock").is_some());
    assert_eq!(p.list_coordination_locks(Some("port")).expect("list locks").len(), 1);
    assert!(p.delete_coordination_lock("lock-active-1").expect("delete lock"));

    p.enqueue_lock_wait(&LockWaitEntry {
        wait_id: "wait-1".into(),
        resource_type: "port".into(),
        resource_id: "18080".into(),
        scope: "exclusive".into(),
        waiting_agent: "agent-c".into(),
        priority: 10,
        enqueued_at: now_iso.clone(),
        timeout_at: future_iso,
        status: "waiting".into(),
    })
    .expect("enqueue_lock_wait");
    assert_eq!(p.list_lock_wait_queue(Some("port"), Some("18080")).expect("list wait").len(), 1);
    assert!(p.pop_next_lock_waiter("port", "18080").expect("pop waiter").is_some());
    assert!(p.update_lock_wait_status("wait-1", "acquired").expect("update wait status"));

    // 8. Ecosystem catalogs
    p.upsert_mcp_catalog(&McpCatalogRecord {
        id: "mcp-1".into(),
        server_name: "codegraph".into(),
        client_source: "codex".into(),
        transport: "stdio".into(),
        command_or_url: "codegraph mcp".into(),
        tools_json: serde_json::json!(["codegraph_explore"]),
        tool_count: 1,
        est_total_tokens: 120,
        status: "active".into(),
        last_scanned_at: Some(now_iso.clone()),
    })
    .expect("upsert_mcp_catalog");
    assert!(!p.list_mcp_catalog().expect("list_mcp_catalog").is_empty());

    p.upsert_skill_catalog(&SkillCatalogRecord {
        id: "sk-1".into(),
        skill_name: "agent-body-guide".into(),
        root_source: "/skills".into(),
        skill_path: "/skills/agent-body-guide/SKILL.md".into(),
        description: "guide".into(),
        triggers: vec!["body".into()],
        est_tokens: 200,
        has_scripts: true,
        has_references: false,
        status: "active".into(),
        last_scanned_at: Some(now_iso.clone()),
    })
    .expect("upsert_skill_catalog");
    assert!(!p.list_skills_catalog().expect("list_skills_catalog").is_empty());

    p.upsert_external_service(&ExternalServiceRecord {
        id: "svc-1".into(),
        service_name: "ollama".into(),
        category: "llm".into(),
        endpoint: "http://127.0.0.1:11434".into(),
        protocol: "http".into(),
        capabilities: "embeddings".into(),
        auth_type: "none".into(),
        auth_env_var: None,
        health_endpoint: Some("http://127.0.0.1:11434/api/tags".into()),
        usage_example: None,
        status: "active".into(),
        last_checked_at: Some(now_iso.clone()),
    })
    .expect("upsert_external_service");
    assert!(!p.list_external_services(Some("llm")).expect("list_external_services").is_empty());

    // 9. Benchmark & Effectiveness Ledger
    p.insert_benchmark_run(&BenchmarkRunRecord {
        run_id: "bench-contract-1".into(),
        suite_name: "contract".into(),
        git_commit: "deadbeef".into(),
        app_version: "1.1.0".into(),
        host_fingerprint: serde_json::json!({"os": "test"}),
        workload_count: 1,
        total_baseline_tokens: 1000,
        total_body_tokens: 100,
        overall_token_savings_pct: 90.0,
        overall_speedup_factor: 10.0,
        total_hazards_intercepted: 1,
        total_noise_blocked: 5,
        sla_passed: true,
        results_json: serde_json::json!({"ok": true}),
        report_sha256: "a".repeat(64),
        duration_ms: 12,
        created_at: now_iso.clone(),
    })
    .expect("insert_benchmark_run");
    assert!(p.get_benchmark_run("bench-contract-1").expect("get_benchmark_run").is_some());
    assert!(!p.list_benchmark_runs(10).expect("list_benchmark_runs").is_empty());
    assert!(p.delete_benchmark_run("bench-contract-1").expect("delete_benchmark_run"));

    p.insert_effectiveness_event(&EffectivenessLedgerRecord {
        event_id: "eff-contract-1".into(),
        session_id: "sess-contract".into(),
        source_channel: "mcp".into(),
        tool_or_action: "body_brief".into(),
        provenance_rule: "PR-01".into(),
        baseline_tokens: 500,
        body_tokens: 50,
        tokens_saved: 450,
        baseline_rounds: 3,
        body_rounds: 1,
        rounds_saved: 2,
        hazard_intercepted: 0,
        noise_blocked: 0,
        elapsed_us: 800,
        evidence_summary: "contract event".into(),
        created_at: now_iso.clone(),
    })
    .expect("insert_effectiveness_event");
    let eff = p
        .query_effectiveness_ledger("7d", Some("body_brief"), 10)
        .expect("query_effectiveness_ledger");
    assert_eq!(eff.total_events, 1);
    assert_eq!(eff.total_body_tokens, 50);

    // 10. Mesh dialogues & transfers
    p.insert_mesh_dialogue(&MeshDialogueRecord {
        dialogue_id: "dlg-1".into(),
        thread_id: "th-1".into(),
        from_node: "node-a".into(),
        from_agent: "agent-a".into(),
        to_node: "node-b".into(),
        to_agent: "agent-b".into(),
        intent: "ASK_CONTEXT".into(),
        content: "status?".into(),
        redaction_hits: 0,
        injection_flagged: false,
        verified_signature: true,
        status: "received".into(),
        created_at: now_iso.clone(),
    })
    .expect("insert_mesh_dialogue");
    assert_eq!(p.list_mesh_dialogues(10).expect("list_mesh_dialogues").len(), 1);
    assert!(p.update_mesh_dialogue_status("dlg-1", "acked").expect("update dialogue"));

    p.insert_mesh_transfer(&MeshTransferRecord {
        transfer_id: "tx-1".into(),
        from_node: "node-a".into(),
        from_agent: "agent-a".into(),
        to_node: "node-b".into(),
        file_name: "patch.diff".into(),
        size_bytes: 128,
        sha256: "b".repeat(64),
        inbox_path: "/tmp/inbox/patch.diff".into(),
        secret_warnings: 0,
        verified_signature: true,
        status: "quarantined".into(),
        accepted_path: None,
        created_at: now_iso.clone(),
        updated_at: now_iso.clone(),
    })
    .expect("insert_mesh_transfer");
    assert_eq!(p.list_mesh_transfers(10).expect("list_mesh_transfers").len(), 1);
    assert!(p
        .update_mesh_transfer_status("tx-1", "accepted", Some("/tmp/accepted/patch.diff"))
        .expect("update transfer"));

    // 11. Audit events & summary
    p.insert_audit_event(&AuditEventRecord {
        event_id: "aud-contract-1".into(),
        trace_id: "tr-1".into(),
        session_id: "sess-1".into(),
        domain: "IN_AGENT".into(),
        actor_kind: "mcp_client".into(),
        actor_id: "agent-body".into(),
        source_channel: "mcp".into(),
        category: "tool_call".into(),
        action: "body_brief".into(),
        target_type: "host".into(),
        target_id: "localhost".into(),
        concurrency_mode: None,
        outcome: "success".into(),
        before_after_delta: serde_json::json!({}),
        elapsed_us: 250,
        evidence_sha256: "d".repeat(64),
        created_at: now_iso.clone(),
    })
    .expect("insert_audit_event");
    assert_eq!(
        p.query_audit_events(&AuditQueryFilter {
            domain: Some("IN_AGENT".into()),
            limit: 10,
            ..Default::default()
        })
        .expect("query_audit_events")
        .len(),
        1
    );
    assert_eq!(p.audit_domain_summary("7d").expect("audit_domain_summary").total_events, 1);
    p.prune_old_audit_events(&expired_iso).expect("prune_old_audit_events");

    // 12. Body checkpoints
    p.insert_body_checkpoint(&BodyCheckpointRecord {
        checkpoint_id: "cp-contract-1".into(),
        body_id: "body-contract".into(),
        from_brain: "codex".into(),
        session_id: "sess-cp".into(),
        goal_summary: "verify SPI".into(),
        active_files: vec!["src/lib.rs".into()],
        active_locks: vec![],
        verified_facts: vec!["303 unit tests pass".into()],
        open_blockers: vec![],
        next_step_hint: "commit".into(),
        token_budget: 250,
        status: "active".into(),
        evidence_sha256: "e".repeat(64),
        created_at: now_iso.clone(),
        resumed_by_brain: None,
        resumed_at: None,
    })
    .expect("insert_body_checkpoint");
    assert!(p
        .get_latest_active_body_checkpoint("body-contract")
        .expect("get_latest_active_body_checkpoint")
        .is_some());
    assert_eq!(
        p.list_body_checkpoints(Some("active"), 10)
            .expect("list_body_checkpoints")
            .len(),
        1
    );
    assert!(p
        .mark_body_checkpoint_resumed("cp-contract-1", "claude-code", &now_iso)
        .expect("mark_body_checkpoint_resumed"));
}

#[test]
fn test_sqlite_storage_spi_contract() {
    let provider = SqliteStorageProvider::open_in_memory().expect("sqlite in-memory");
    run_shared_spi_contract(&provider, "sqlite");
}

#[test]
fn test_duckdb_storage_spi_contract() {
    let provider = DuckDbStorageProvider::open_in_memory().expect("duckdb in-memory");
    run_shared_spi_contract(&provider, "duckdb");
}
