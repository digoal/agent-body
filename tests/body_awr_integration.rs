use agent_body::body::{
    AutonomyEnvelope, AutonomyLevel, BodyManifestEngine, BodyPerfEngine, ContinuityInput,
    ContinuityManager, HomeostasisVitalsEngine, WaitEventGuard, WaitEventId, WaitEventRegistry,
};
use agent_body::storage::Storage;

#[test]
fn test_body_manifest_and_vitals_assessment() {
    let storage = Storage::open_in_memory().expect("open storage");
    let manifest = BodyManifestEngine::build(&storage);
    assert!(manifest.body_id.starts_with("body-"));
    assert_eq!(manifest.organs.len(), 6);

    let vitals = HomeostasisVitalsEngine::assess(&storage);
    assert_eq!(vitals.dimensions.len(), 5);
    assert!((0.0..=100.0).contains(&vitals.overall_score));
}

#[test]
fn test_awr_snapshot_diff_report_persistence_and_delete() {
    let storage = Storage::open_in_memory().expect("open storage");

    {
        let _g1 = WaitEventGuard::enter(WaitEventId::IoSqliteRead);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    WaitEventRegistry::record_us(WaitEventId::NetOllamaEmbed, 250);

    let rows = WaitEventRegistry::snapshot_all();
    assert_eq!(rows.len(), 18, "必须包含全部 18 个 WaitEventId");

    let snap_a = BodyPerfEngine::capture_snapshot(&storage, Some("it-baseline"), true)
        .expect("capture baseline");
    std::thread::sleep(std::time::Duration::from_millis(5));
    WaitEventRegistry::record_us(WaitEventId::IoSqliteWrite, 800);
    let snap_b = BodyPerfEngine::capture_snapshot(&storage, Some("it-target"), false)
        .expect("capture target");

    let report = BodyPerfEngine::generate_awr_report(
        &storage,
        Some(&snap_a.snapshot_id),
        Some(&snap_b.snapshot_id),
        true,
    )
    .expect("generate AWR report");
    assert!(report.markdown_summary.contains("Agent Body 内生 AWR 性能诊断报告"));
    assert!(!report.addm_findings.is_empty());

    let reports_meta = BodyPerfEngine::list_awr_reports_meta(storage.provider().as_ref());
    assert!(
        reports_meta.iter().any(|r| r.report_id == report.report_id),
        "AWR 报告必须持久化并可通过 list_awr_reports_meta 查回"
    );

    // Delete report & verify gone
    let del_rep = BodyPerfEngine::delete_perf_artifact(&storage, "report", &report.report_id)
        .expect("delete report");
    assert_eq!(del_rep["deleted"].as_bool(), Some(true));
    let after_del = BodyPerfEngine::list_awr_reports_meta(storage.provider().as_ref());
    assert!(!after_del.iter().any(|r| r.report_id == report.report_id));

    // Delete snapshot B & verify
    let del_snap = BodyPerfEngine::delete_perf_artifact(&storage, "snapshot", &snap_b.snapshot_id)
        .expect("delete snapshot");
    assert_eq!(del_snap["deleted"].as_bool(), Some(true));
}

#[test]
fn test_autonomy_envelope_and_continuity_handoff_roundtrip() {
    let storage = Storage::open_in_memory().expect("open storage");

    let l3 = AutonomyEnvelope::evaluate("write_file", Some("/etc/shadow"), false);
    assert_eq!(l3.level, AutonomyLevel::L3Forbidden);
    assert!(!l3.allowed);

    let l0 = AutonomyEnvelope::evaluate("brief", Some("/tmp/notes.md"), false);
    assert_eq!(l0.level, AutonomyLevel::L0Observe);
    assert!(l0.allowed);

    let saved = ContinuityManager::save_capsule(
        &storage,
        ContinuityInput {
            from_brain: "antigravity".to_string(),
            session_id: "sess-it-body".to_string(),
            goal_summary: "完成 Phase 56 架构拆分与 AWR 集成验证".to_string(),
            active_files: vec!["src/body/perf_awr/mod.rs".to_string()],
            active_locks: vec![],
            verified_facts: vec!["18/18 WaitEventId 全量覆盖".to_string()],
            open_blockers: vec![],
            next_step_hint: "继续执行 Phase 57 文档同步".to_string(),
            token_budget: Some(300),
        },
    )
    .expect("save capsule");

    let resumed = ContinuityManager::resume_latest(&storage, "claude-code", Some(&saved.checkpoint_id))
        .expect("resume")
        .expect("capsule present");
    assert_eq!(resumed.checkpoint_id, saved.checkpoint_id);
    assert_eq!(resumed.status, "resumed");
    assert_eq!(resumed.resumed_by_brain.as_deref(), Some("claude-code"));
}

#[test]
fn test_perf_l1_autonomic_tune_and_baseline_metadata() {
    let storage = Storage::open_in_memory().expect("open storage");
    BodyPerfEngine::capture_snapshot(&storage, Some("gold"), true).expect("gold");
    let tune = BodyPerfEngine::run_l1_auto_tune(&storage).expect("tune");
    assert!(!tune.actions_executed.is_empty());
    let (snaps, baseline) = BodyPerfEngine::list_snapshots_meta(storage.provider().as_ref());
    assert!(!snaps.is_empty());
    assert!(baseline.is_some());
}

