//! `ext-mem-pg-tx` 集成回归测试：微内核安装、私有表前缀校验、四端 Parity 派生与离线降级

use agent_body_kernel::{HttpRequest, KernelRuntime};
use ext_mem_pg_tx::{
    body_module_magic, MemPgTxExtension, PgTxEngine, VectorBackendKind, PG_TX_SCHEMA_SQL,
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
