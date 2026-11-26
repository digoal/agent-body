use agent_body::audit::{reset_watcher_fold_cache, AuditEventInput, AuditEvolutionAnalyzer, AuditLogger};
use agent_body::storage::models::AuditQueryFilter;
use agent_body::storage::Storage;

#[test]
fn test_dual_domain_audit_logging_query_and_fold() {
    reset_watcher_fold_cache();
    let storage = Storage::open_in_memory().expect("open storage");

    // 1. Record IN_AGENT events with sensitive secret (verify SecretRedactor)
    let in_rec = AuditLogger::record(
        &storage,
        AuditEventInput::in_agent("cli", "asset", "register")
            .with_session("sess-audit-1")
            .with_target("file", "/tmp/demo_secret_sk-1234567890abcdef123456.py")
            .with_outcome("success")
            .with_elapsed_us(420),
    )
    .expect("record in_agent")
    .expect("not folded");
    assert_eq!(in_rec.domain, "IN_AGENT");
    assert!(!in_rec.target_id.contains("sk-1234567890abcdef"));

    // 2. Record OUT_OF_AGENT events + verify watcher 60s flood folding
    let w1 = AuditLogger::record(
        &storage,
        AuditEventInput::out_of_agent("watcher_inotify", "watcher", "fs_watch_sync", "modified")
            .with_target("file", "/tmp/hot_file.rs"),
    )
    .expect("watcher 1");
    assert!(w1.is_some(), "第一次 watcher 事件应立即落库");

    let w2 = AuditLogger::record(
        &storage,
        AuditEventInput::out_of_agent("watcher_inotify", "watcher", "fs_watch_sync", "modified")
            .with_target("file", "/tmp/hot_file.rs"),
    )
    .expect("watcher 2");
    assert!(w2.is_none(), "60s 窗口内同路径 watcher 事件应被折叠防洪");

    // 3. Domain-filtered queries + domain summary
    let in_rows = storage
        .query_audit_events(&AuditQueryFilter {
            domain: Some("IN_AGENT".to_string()),
            limit: 20,
            ..Default::default()
        })
        .expect("query IN_AGENT");
    assert_eq!(in_rows.len(), 1);

    let out_rows = storage
        .query_audit_events(&AuditQueryFilter {
            domain: Some("OUT_OF_AGENT".to_string()),
            limit: 20,
            ..Default::default()
        })
        .expect("query OUT_OF_AGENT");
    assert_eq!(out_rows.len(), 1);

    let summary = storage.provider().audit_domain_summary("24h").expect("summary");
    assert_eq!(summary.in_agent_count, 1);
    assert_eq!(summary.out_of_agent_count, 1);
}

#[test]
fn test_audit_evolution_cycle_detects_friction_hotspots() {
    let storage = Storage::open_in_memory().expect("open storage");
    for idx in 0..4 {
        AuditLogger::record(
            &storage,
            AuditEventInput::in_agent("mcp", "hook_pre_guard", "pre_tool_call")
                .with_session(format!("s-{}", idx))
                .with_target("port", "port:5432")
                .with_outcome("blocked"),
        )
        .expect("record blocked");
    }

    let report = AuditEvolutionAnalyzer::run_evolution_cycle(&storage, None, false, 3)
        .expect("evolution cycle");
    assert!(report.evaluated_events >= 4);
    assert!(
        !report.friction_hotspots.is_empty(),
        "连续 4 次被阻断后应被识别为摩擦热点"
    );
    assert_eq!(report.friction_hotspots[0].target_id, "port:5432");
}
