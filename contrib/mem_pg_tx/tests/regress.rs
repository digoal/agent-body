//! `ext-mem-pg-tx` 集成回归测试：微内核安装、私有表前缀校验、四端 Parity 派生与离线降级

use agent_body_kernel::{HttpRequest, KernelRuntime};
use agent_body_sdk::{
    evaluate_fast_weight_gates, validate_artifact_uri, AdapterKind, FastWeightCompactRequest,
    FastWeightGateMetrics, FastWeightGatePolicy, FastWeightRuntimeClient,
};
use ext_mem_pg_tx::{
    body_module_magic, compact_episodic_texts, estimate_tokens_deterministic, MemPgTxExtension,
    MockFastWeightRuntime, MockRuntimeFault, PgTxEngine, QueryPredictabilityGate, SalienceInput,
    SalienceScorer, SalienceTier, VectorBackendKind, PG_TX_SCHEMA_SQL,
};

#[test]
fn mem_pg_tx_installs_in_microkernel_and_derives_parity_and_offline_status() {
    let magic = body_module_magic();
    assert_eq!(magic.crate_name, "ext-mem-pg-tx");
    assert_eq!(magic.sdk_api_version, agent_body_sdk::SDK_API_VERSION);

    let mut rt = KernelRuntime::open_sqlite_in_memory(None).expect("kernel init");
    let order = rt
        .install_extensions(&[Box::new(MemPgTxExtension::default())])
        .expect("install mem_pg_tx");
    assert_eq!(order, vec!["mem_pg_tx"]);

    // 验证私有表前缀迁移已成功写入
    rt.table_am()
        .exec_scoped_migration(
            "mem_pg_tx",
            "ext_mem_pg_tx__",
            "0.1.1",
            "INSERT INTO ext_mem_pg_tx__catalog_meta (meta_key, meta_json) VALUES ('schema_ver', '\"0.1.0\"');",
        )
        .expect("scoped private table write");

    // 验证 Parity Matrix 自动包含两个算子
    let parity_resp = rt.handle_http(&HttpRequest {
        method: "GET".to_string(),
        path: "/api/v1/parity/matrix".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::Value::Null,
    });
    assert_eq!(parity_resp.status_code, 200);
    assert_eq!(parity_resp.body["total"], 2);

    // 验证未配置 PG 时 GET /api/v1/mem/pg-tx/status 优雅降级返回 fallback_active=true
    let st_resp = rt.handle_http(&HttpRequest {
        method: "GET".to_string(),
        path: "/api/v1/mem/pg-tx/status".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::Value::Null,
    });
    assert_eq!(st_resp.status_code, 200);
    assert_eq!(st_resp.body["data"]["connected"], serde_json::json!(false));
    assert_eq!(st_resp.body["data"]["fallback_active"], serde_json::json!(true));
}

#[test]
fn pg_tx_schema_ddl_and_vector_backend_validation() {
    assert!(PG_TX_SCHEMA_SQL.contains("ext_mem_pg_tx__events"));
    assert!(PG_TX_SCHEMA_SQL.contains("ext_mem_pg_tx__segments"));
    assert!(PG_TX_SCHEMA_SQL.contains("ext_mem_pg_tx__fast_weight_snapshots"));
    assert!(PG_TX_SCHEMA_SQL.contains("ext_mem_pg_tx__compaction_jobs"));
    assert!(PG_TX_SCHEMA_SQL.contains("idx_ext_mem_pg_tx_active_fact_unique"));
    assert!(PG_TX_SCHEMA_SQL.contains("agent_memory_events"));

    assert_eq!(
        VectorBackendKind::parse_str("pgvector").expect("pgvector"),
        VectorBackendKind::Pgvector
    );
    assert_eq!(
        VectorBackendKind::parse_str("pg_vectorsearch").expect("pg_vectorsearch"),
        VectorBackendKind::PgVectorsearch
    );
    assert_eq!(
        VectorBackendKind::parse_str("external_lsm").expect("external_lsm"),
        VectorBackendKind::ExternalLsm
    );
    assert!(VectorBackendKind::parse_str("invalid_vec").is_err());

    let h1 = PgTxEngine::compute_evidence_hash("b1", "t1", "k1", "fact", "hello");
    let h2 = PgTxEngine::compute_evidence_hash("b1", "t1", "k1", "fact", "hello");
    let h3 = PgTxEngine::compute_evidence_hash("b1", "t1", "k1", "fact", "world");
    assert_eq!(h1.len(), 32);
    assert_eq!(h1, h2);
    assert_ne!(h1, h3);
}

#[test]
fn salience_scorer_and_predictability_gate_and_deterministic_compactor() {
    let high = SalienceScorer::score(&SalienceInput {
        content: "Keep PG MVCC pool_size=8 and reject long 2PL inference locks".to_string(),
        memory_type: "decision".to_string(),
        evidence_tier: "L1_EXPLICIT".to_string(),
        cross_session_reuses: 4,
        conflict_corrections: 2,
        novelty_ratio: 0.9,
        age_hours: 6.0,
        is_auto_sync: false,
        pinned: true,
    });
    assert_eq!(high.tier, SalienceTier::High);
    assert!(high.score >= 0.85);

    let low = SalienceScorer::score(&SalienceInput {
        content: "auto_sync: routine check ok".to_string(),
        memory_type: "note".to_string(),
        evidence_tier: "L3_SEMANTIC".to_string(),
        cross_session_reuses: 0,
        conflict_corrections: 0,
        novelty_ratio: 0.1,
        age_hours: 120.0,
        is_auto_sync: true,
        pinned: false,
    });
    assert_eq!(low.tier, SalienceTier::Low);
    assert!(low.score < 0.35);

    let items = vec![
        (
            "auto_sync:1".to_string(),
            "auto_sync: Checked host status and everything looks fine. PG primary port=5432.".to_string(),
        ),
        (
            "auto_sync:2".to_string(),
            "routine log: Checked host status and everything looks good. PG primary port=5432. max_wal_size=2GB.".to_string(),
        ),
        (
            "auto_sync:3".to_string(),
            "session check: No anomaly detected during routine check. Fallback SQLite must stay transparent.".to_string(),
        ),
    ];
    let raw_tok: i64 = items
        .iter()
        .map(|(_, c)| estimate_tokens_deterministic(c))
        .sum();
    let (compacted, facts_cnt) = compact_episodic_texts("pg_pool", &items);
    let comp_tok = estimate_tokens_deterministic(&compacted);
    assert!(compacted.contains("port=5432"));
    assert!(compacted.contains("max_wal_size=2GB"));
    assert!(compacted.contains("Fallback SQLite"));
    assert!(facts_cnt >= 3);
    assert!(comp_tok < raw_tok);

    let summary_ref = [compacted.as_str()];
    let predictable = QueryPredictabilityGate::evaluate("pg port=5432 max_wal_size", &summary_ref, None, false);
    assert!(!predictable.bypass_compacted, "routine query should hit compacted summary: {}", predictable.reason);

    let unpredictable = QueryPredictabilityGate::evaluate(
        "PgTxEngine::search_raw_bypass SQLSTATE 40001 /src/memory/spi.rs",
        &summary_ref,
        None,
        false,
    );
    assert!(unpredictable.bypass_compacted, "exploratory diagnostic query must bypass: {}", unpredictable.reason);
}

#[test]
fn fast_weight_mock_runtime_gates_and_fault_injection() {
    let rt = MockFastWeightRuntime::default();
    let policy = FastWeightGatePolicy::default();
    let ok_resp = rt
        .submit_compaction(&FastWeightCompactRequest {
            body_id: "body-1".into(),
            tenant_scope: "default".into(),
            base_model_id: "qwen3-coder-next".into(),
            adapter_kind: AdapterKind::ScolLora,
            parent_snapshot_id: None,
            from_epoch: 1,
            to_epoch: 10,
            episode_payloads: vec!["fact a".into(), "fact b".into()],
            tombstone_ids: vec![7],
            audit_trace_id: "trace-ok".into(),
        })
        .expect("mock compact ok");
    assert!(validate_artifact_uri(&ok_resp.artifact_uri).is_ok());
    let ok_v = evaluate_fast_weight_gates(
        &FastWeightGateMetrics {
            norm_ratio: ok_resp.norm_ratio,
            eval_glue_delta: ok_resp.eval_glue_delta,
            unlearn_leak_rate: ok_resp.unlearn_leak_rate,
        },
        &policy,
    );
    assert!(ok_v.passed);
    assert_eq!(rt.purged_event_ids().expect("purged"), vec![7]);

    rt.set_fault(MockRuntimeFault {
        norm_ratio: Some(2.4),
        eval_glue_delta: Some(-0.08),
        unlearn_leak_rate: Some(0.02),
        rpc_error: None,
    })
    .expect("set fault");
    let bad_resp = rt
        .submit_compaction(&FastWeightCompactRequest {
            body_id: "body-1".into(),
            tenant_scope: "default".into(),
            base_model_id: "qwen3-coder-next".into(),
            adapter_kind: AdapterKind::InplaceTtt,
            parent_snapshot_id: Some(ok_resp.snapshot_id),
            from_epoch: 11,
            to_epoch: 20,
            episode_payloads: vec!["fact c".into()],
            tombstone_ids: vec![],
            audit_trace_id: "trace-bad".into(),
        })
        .expect("mock fault response");
    let bad_v = evaluate_fast_weight_gates(
        &FastWeightGateMetrics {
            norm_ratio: bad_resp.norm_ratio,
            eval_glue_delta: bad_resp.eval_glue_delta,
            unlearn_leak_rate: bad_resp.unlearn_leak_rate,
        },
        &policy,
    );
    assert!(!bad_v.passed);
    assert!(!bad_v.norm_ok && !bad_v.glue_ok && !bad_v.unlearn_ok);
}
