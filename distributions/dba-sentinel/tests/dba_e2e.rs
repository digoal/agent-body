//! `distributions/dba-sentinel` 集成测试

use std::fs;
use std::path::Path;

use agent_body_dba::{build_dba_runtime, ingest_tuples, run_self_check, DbaSentinelConfig};
use agent_body_kernel::dispatcher::HttpRequest;
use agent_body_sdk::{ClientAuthContext, ProbeContext, TransportKind};
use ext_hook_audit_siem::format_rfc5424;

fn write_file(p: &Path, content: &str) {
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(p, content).unwrap();
}

#[test]
fn parses_pg_dm_kingbase_with_redaction_and_siem_audit() {
    let tmp = tempfile::tempdir().unwrap();
    let db_root = tmp.path().join("dbs");
    let os_root = tmp.path().join("os");

    write_file(
        &db_root.join("pg/postgresql.conf"),
        "port = 5432\nmax_connections = 1000\nshared_buffers = '16GB'\nwal_level = replica\n# url = postgresql://replicator:ReplSecret!88@10.1.2.3:5432/main\n",
    );
    write_file(
        &db_root.join("dm/dm.ini"),
        "INSTANCE_NAME = DMPROD\nPORT_NUM = 5236\nMAX_SESSIONS = 2000\nMEMORY_POOL = 2048\n",
    );
    write_file(
        &db_root.join("kingbase/kingbase.conf"),
        "port = 54321\nmax_connections = 800\n",
    );
    write_file(
        &os_root.join("proc/6001/cmdline"),
        "postgres\0-D\0/data/pgdata\0",
    );

    let cfg = DbaSentinelConfig {
        db_roots: vec![db_root],
        os_root,
        host_id: "dba-host-01".to_string(),
        host_labels: Default::default(),
    };
    let dba_rt = build_dba_runtime(&cfg, None).unwrap();
    let auth = ClientAuthContext {
        transport: TransportKind::Internal,
        cert: None,
        bearer_token: None,
        method: "GET".to_string(),
        path: "/api/v1/fdw".to_string(),
        is_loopback: true,
        is_read_only: true,
    };

    let mut ctx = ProbeContext {
        target_host: Some("dba-host-01".to_string()),
        timeout_ms: 3000,
        ..Default::default()
    };
    let db_tuples = dba_rt
        .runtime
        .scan_fdw("fdw:db_engines", None, &mut ctx, &auth)
        .unwrap();
    assert_eq!(db_tuples.len(), 3);

    let os_tuples = dba_rt
        .runtime
        .scan_fdw("fdw:linux_ops", None, &mut ctx, &auth)
        .unwrap();
    assert!(!os_tuples.is_empty());

    let serialized = serde_json::to_string(&db_tuples).unwrap();
    assert!(!serialized.contains("ReplSecret!88"));

    ingest_tuples(&dba_rt, &db_tuples).unwrap();
    let snap = dba_rt.audit_buffer.snapshot();
    assert!(snap.len() >= 2);
    let rfc = format_rfc5424(&snap[0], "dba-host-01", "agent-body-dba");
    assert!(rfc.starts_with("<134>1 "));

    let metrics = dba_rt.runtime.handle_http(&HttpRequest {
        method: "GET".to_string(),
        path: "/api/v1/obs/metrics".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::json!({"format": "prometheus"}),
    });
    assert_eq!(metrics.status_code, 200);
    assert!(metrics.body["data"]["metrics_text"]
        .as_str()
        .unwrap()
        .contains("agent_body_kernel_wait_duration_seconds_bucket"));

    let diag = dba_rt.runtime.handle_http(&HttpRequest {
        method: "POST".to_string(),
        path: "/api/v1/obs/diagnose".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::json!({"limit": 10}),
    });
    assert_eq!(diag.status_code, 200);
    assert_eq!(diag.body["data"]["health_grade"], "healthy");

    run_self_check().unwrap();
}
