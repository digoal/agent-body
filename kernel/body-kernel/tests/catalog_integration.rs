//! 系统目录 (`body_cat_*`) 集成测试：Schema、DAG 拓扑排序、状态机跃迁与 Registrar 同步

use std::sync::Arc;
use std::time::Duration;

use agent_body_kernel::{
    catalog::{compute_control_hash, new_catalog_trace_id, validate_scoped_table_name},
    topo_sort_extensions, CatalogStore, ExtensionState, KernelRuntime,
};
use agent_body_sdk::{
    AuditEnvelope, AuthVerdict, AutonomyLevel, BgWorkerRoutine, BodyExtension, CancellationToken,
    ClientAuthContext, ClientAuthHook, EmitAuditHook, EvidenceTuple, ExtensionKind, ExtensionMeta,
    ExtensionRegistrar, FailPolicy, FdwRoutine, FdwScanState, FdwSourceHealth, FunctionCallInfo,
    HookRegistration, KernelServices, OperatorRoutine, ProbeContext, ProcRoutine, ProcSpec,
    ProcVolatility, TransportKind, TypeRoutine, WorkerTickOutcome, WorkerTrigger, SDK_API_VERSION,
};
use anyhow::bail;
use rusqlite::Connection;
use tempfile::NamedTempFile;

struct DummyType;
impl TypeRoutine for DummyType {
    fn type_id(&self) -> &'static str {
        "entity:sample"
    }
    fn validate_in(&self, raw: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
        Ok(raw.clone())
    }
    fn render_compact_summary(&self, _val: &serde_json::Value) -> String {
        "sample".to_string()
    }
}

struct DummyOp;
impl OperatorRoutine for DummyOp {
    fn op_id(&self) -> &'static str {
        "op:eq"
    }
    fn strategy_num(&self) -> u16 {
        1
    }
    fn evaluate(&self, lhs: &serde_json::Value, rhs: &serde_json::Value) -> anyhow::Result<bool> {
        Ok(lhs == rhs)
    }
}

struct DummyFdw;
impl FdwRoutine for DummyFdw {
    fn fdw_id(&self) -> &'static str {
        "fdw:sample"
    }
    fn supported_types(&self) -> &'static [&'static str] {
        &["entity:sample"]
    }
    fn explain_source(&self, _ctx: &ProbeContext) -> FdwSourceHealth {
        FdwSourceHealth {
            healthy: true,
            degraded_reason: None,
            missing_deps: vec![],
        }
    }
    fn begin_scan(
        &self,
        _target: Option<&str>,
        _ctx: &ProbeContext,
    ) -> anyhow::Result<Box<dyn FdwScanState>> {
        bail!("not used in catalog test")
    }
    fn poll_drift(
        &self,
        _cursor: Option<&str>,
        _ctx: &ProbeContext,
    ) -> anyhow::Result<(Vec<EvidenceTuple>, Option<String>)> {
        Ok((vec![], None))
    }
}

struct DummyProc;
impl ProcRoutine for DummyProc {
    fn spec(&self) -> ProcSpec {
        ProcSpec {
            proc_name: "sample_ping",
            cli_path: &["sample", "ping"],
            rest_route: ("GET", "/api/v1/sample/ping"),
            profiles: &["minimal"],
            volatility: ProcVolatility::Immutable,
            autonomy_level: AutonomyLevel::L0Observe,
            input_schema: serde_json::json!({"type": "object"}),
            provenance_rule: None,
        }
    }
    fn invoke(
        &self,
        _fcinfo: FunctionCallInfo,
        _ctx: &KernelServices,
    ) -> anyhow::Result<serde_json::Value> {
        Ok(serde_json::json!({"pong": true}))
    }
}

struct DummyHook;
impl ClientAuthHook for DummyHook {
    fn hook_id(&self) -> &'static str {
        "hook:dummy_auth"
    }
    fn authenticate(&self, _ctx: &ClientAuthContext) -> anyhow::Result<AuthVerdict> {
        Ok(AuthVerdict::Next)
    }
}

struct DummyWorker;
impl BgWorkerRoutine for DummyWorker {
    fn worker_id(&self) -> &'static str {
        "worker:sample"
    }
    fn trigger(&self) -> WorkerTrigger {
        WorkerTrigger::Interval(Duration::from_secs(30))
    }
    fn autonomy_level(&self) -> AutonomyLevel {
        AutonomyLevel::L0Observe
    }
    fn run_tick(
        &self,
        _ctx: &KernelServices,
        _cancel: &CancellationToken,
    ) -> anyhow::Result<WorkerTickOutcome> {
        Ok(WorkerTickOutcome::default())
    }
}

fn meta_with(id: &str, reqs: &[&str]) -> ExtensionMeta {
    ExtensionMeta {
        ext_id: id.to_string(),
        version: "0.1.0".to_string(),
        kind: ExtensionKind::CompiledCrate,
        min_sdk_version: "2.0".to_string(),
        table_prefix: format!("ext_{id}__"),
        requires: reqs.iter().map(|s| (*s).to_string()).collect(),
    }
}

#[test]
fn topo_sort_handles_dag_cycles_and_missing_deps() {
    let valid = vec![
        meta_with("c", &["b"]),
        meta_with("b", &["a"]),
        meta_with("a", &[]),
    ];
    let order = topo_sort_extensions(&valid).unwrap();
    assert_eq!(order, vec!["a", "b", "c"]);

    let missing = vec![meta_with("b", &["unknown_ext"])];
    let err_missing = topo_sort_extensions(&missing).unwrap_err();
    assert!(err_missing.to_string().contains("unknown_ext"));

    let cycle = vec![meta_with("a", &["b"]), meta_with("b", &["a"])];
    let err_cycle = topo_sort_extensions(&cycle).unwrap_err();
    assert!(err_cycle.to_string().contains("环路"));
}

#[test]
fn state_machine_control_toml_and_catalog_sync_work_end_to_end() {
    let db_file = NamedTempFile::new().unwrap();
    let conn = Connection::open(db_file.path()).unwrap();
    CatalogStore::init(&conn).unwrap();

    let raw_toml = r#"
[extension]
id = "sample"
version = "0.1.0"
kind = "compiled_crate"

[schema]
table_prefix = "ext_sample__"
default_autonomy = "L0_OBSERVE"
"#;
    let parsed_meta = CatalogStore::register_from_control_toml(&conn, raw_toml).unwrap();
    assert_eq!(parsed_meta.ext_id, "sample");
    assert!(validate_scoped_table_name("sample", "ext_sample__runs").is_ok());
    assert!(validate_scoped_table_name("sample", "ext_other__runs").is_err());
    assert!(!new_catalog_trace_id().is_empty());

    let mut reg = ExtensionRegistrar::new(parsed_meta);
    reg.register_type(Arc::new(DummyType));
    reg.register_operator("idx:core", "entity:sample", Arc::new(DummyOp));
    reg.register_fdw(Arc::new(DummyFdw));
    reg.register_proc(Arc::new(DummyProc));
    reg.register_hook(HookRegistration::ClientAuth(Arc::new(DummyHook)));
    reg.register_bgworker(Arc::new(DummyWorker));

    CatalogStore::sync_registrar(&conn, &reg, &compute_control_hash(raw_toml)).unwrap();

    assert_eq!(
        CatalogStore::get_extension(&conn, "sample")
            .unwrap()
            .unwrap()
            .state,
        ExtensionState::Discovered
    );
    CatalogStore::transition_state(&conn, "sample", ExtensionState::Verified).unwrap();
    CatalogStore::transition_state(&conn, "sample", ExtensionState::Installed).unwrap();
    CatalogStore::transition_state(&conn, "sample", ExtensionState::Enabled).unwrap();
    assert!(CatalogStore::transition_state(&conn, "sample", ExtensionState::Dropped).is_err());
    CatalogStore::transition_state(&conn, "sample", ExtensionState::Disabled).unwrap();
    CatalogStore::transition_state(&conn, "sample", ExtensionState::Dropped).unwrap();

    for tbl in [
        "body_cat_extension",
        "body_cat_type",
        "body_cat_opclass",
        "body_cat_fdw",
        "body_cat_proc",
        "body_cat_hook",
        "body_cat_bgworker",
    ] {
        let count: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {tbl}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "expected 1 row in {tbl}");
    }
}

struct CaptureAuditHook(Arc<std::sync::Mutex<Vec<AuditEnvelope>>>);
impl EmitAuditHook for CaptureAuditHook {
    fn hook_id(&self) -> &'static str {
        "hook:capture_audit"
    }
    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailOpen
    }
    fn emit(&self, envelope: &AuditEnvelope) -> anyhow::Result<()> {
        self.0.lock().unwrap().push(envelope.clone());
        Ok(())
    }
}

struct TraceProbeExt;
impl BodyExtension for TraceProbeExt {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "trace_probe".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: SDK_API_VERSION.to_string(),
            table_prefix: "ext_trace_probe__".to_string(),
            requires: vec![],
        }
    }
    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_proc(Arc::new(DummyProc));
        Ok(())
    }
}

#[test]
fn verifies_trace_id_parity_across_dispatch_audit_and_observability() {
    let mut rt = KernelRuntime::open_sqlite_in_memory(None).unwrap();
    let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
    rt.register_hook(HookRegistration::EmitAudit(Arc::new(CaptureAuditHook(
        Arc::clone(&captured),
    ))));
    rt.install_extensions(&[Box::new(TraceProbeExt)]).unwrap();
    rt.observability().set_slow_threshold_us(0);

    let auth = ClientAuthContext {
        transport: TransportKind::Cli,
        cert: None,
        bearer_token: None,
        method: "GET".to_string(),
        path: "/api/v1/sample/ping".to_string(),
        is_loopback: true,
        is_read_only: true,
    };
    let ok = rt
        .handle_cli(&["sample".to_string(), "ping".to_string()], None, &auth)
        .unwrap();
    let audit_list = captured.lock().unwrap().clone();
    assert_eq!(audit_list.len(), 1);
    assert_eq!(
        audit_list[0].trace_id, ok.trace_id,
        "DispatchSuccess.trace_id 与 AuditEnvelope.trace_id 必须完全一致"
    );

    let snap = rt.observability_snapshot();
    assert!(snap.in_flight.is_empty());
    assert!(
        snap.slow_ops
            .iter()
            .any(|s| s.trace_id == ok.trace_id && s.target == "sample_ping"),
        "SlowOpRecord 必须延续同一个 trace_id"
    );
    assert!(snap
        .op_stats
        .iter()
        .any(|s| s.kind == "proc" && s.target == "sample_ping" && s.ok_count == 1));
}
