//! `contrib/hook_audit_siem` 集成回归测试

use std::sync::Arc;

use agent_body_kernel::dispatcher::HttpRequest;
use agent_body_kernel::KernelRuntime;
use agent_body_sdk::{
    AuditEnvelope, CallerIdentity, CancellationToken, ClientAuthContext, EmitAuditHook,
    EventQueryFilter, ExtensionKind, ExtensionMeta, ExtensionRegistrar, FdwRoutine, FdwScanState,
    FdwSourceHealth, ProbeContext, TransportKind, SDK_API_VERSION,
};
use ext_hook_audit_siem::{
    format_otlp_log_record, format_rfc5424, HookAuditSiemExtension, SiemAuditBuffer, SiemFormat,
    SiemSinkConfig,
};

struct StubReadFdw;

struct EmptyScan;
impl FdwScanState for EmptyScan {
    fn iterate_scan(&mut self, _batch_limit: usize) -> anyhow::Result<Vec<agent_body_sdk::EvidenceTuple>> {
        Ok(vec![])
    }
    fn end_scan(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}

impl FdwRoutine for StubReadFdw {
    fn fdw_id(&self) -> &'static str {
        "fdw:stub_read"
    }
    fn supported_types(&self) -> &'static [&'static str] {
        &["entity:stub"]
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
        Ok(Box::new(EmptyScan))
    }
    fn poll_drift(
        &self,
        _cursor: Option<&str>,
        _ctx: &ProbeContext,
    ) -> anyhow::Result<(Vec<agent_body_sdk::EvidenceTuple>, Option<String>)> {
        Ok((vec![], None))
    }
}

struct StubExt;
impl agent_body_sdk::BodyExtension for StubExt {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "stub_ext".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: SDK_API_VERSION.to_string(),
            table_prefix: "ext_stub_ext__".to_string(),
            requires: vec![],
        }
    }
    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_fdw(Arc::new(StubReadFdw));
        Ok(())
    }
}

#[test]
fn captures_read_probe_and_auth_audit_and_flushes_rfc5424_and_otlp() {
    let siem = HookAuditSiemExtension::new(SiemSinkConfig {
        app_name: "agent-body-probe".to_string(),
        host_name: "gov-node-01".to_string(),
        ring_capacity: 128,
        format: SiemFormat::Rfc5424Syslog,
    });
    let buffer = siem.buffer();

    let mut rt = KernelRuntime::open_sqlite_in_memory(None).unwrap();
    rt.install_extensions(&[Box::new(siem), Box::new(StubExt)]).unwrap();

    // 1. 只读 HTTP /catalog/extensions 与只读 FDW 探测均强制留下审计事件
    let resp = rt.handle_http(&HttpRequest {
        method: "GET".to_string(),
        path: "/api/v1/catalog/extensions".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::Value::Null,
    });
    assert_eq!(resp.status_code, 200);

    let auth = ClientAuthContext {
        transport: TransportKind::Internal,
        cert: None,
        bearer_token: None,
        method: "GET".to_string(),
        path: "/api/v1/fdw".to_string(),
        is_loopback: true,
        is_read_only: true,
    };
    let mut ctx = ProbeContext::default();
    rt.scan_fdw("fdw:stub_read", None, &mut ctx, &auth).unwrap();

    let snap = buffer.snapshot();
    assert!(snap.len() >= 2);
    assert!(snap.iter().any(|e| e.operation == "fdw_scan"));
    assert!(snap.iter().any(|e| e.target.contains("/api/v1/catalog/extensions")));

    let rfc_line = format_rfc5424(&snap[0], "gov-node-01", "agent-body-probe");
    assert!(rfc_line.starts_with("<134>1 "));
    assert!(rfc_line.contains("[agentbody@59999 "));
    let otlp = format_otlp_log_record(&snap[0], "gov-node-01", "agent-body-probe");
    assert_eq!(
        otlp["resourceLogs"][0]["scopeLogs"][0]["logRecords"][0]["attributes"][0]["key"],
        "trace_id"
    );

    // 2. BgWorker 刷新至 siem_outbox
    let outcomes = rt.run_bgworkers_once(&CancellationToken::new()).unwrap();
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].1.emitted_events >= 2);

    let outbox = rt
        .table_am()
        .event_query(
            &EventQueryFilter {
                stream: Some("siem_outbox".to_string()),
                ..Default::default()
            },
            None,
        )
        .unwrap();
    assert!(outbox.len() >= 2);
}

#[test]
fn ring_buffer_bounds_memory_and_counts_drops() {
    let ring = SiemAuditBuffer::new(3);
    for idx in 0..5 {
        ring.push(AuditEnvelope {
            trace_id: format!("tr-{idx}"),
            caller: CallerIdentity::default(),
            domain: "test".to_string(),
            operation: "read".to_string(),
            target: format!("tgt-{idx}"),
            outcome: "ok".to_string(),
            before_after_delta: None,
            evidence_sha256: "00".to_string(),
            recorded_at: "2026-11-27T16:30:00+08:00".to_string(),
        })
        .unwrap();
    }
    assert_eq!(ring.len(), 3);
    assert_eq!(ring.total_received(), 5);
    assert_eq!(ring.dropped_count(), 2);
    let ids: Vec<String> = ring.snapshot().into_iter().map(|e| e.trace_id).collect();
    assert_eq!(ids, vec!["tr-2", "tr-3", "tr-4"]);

    let _trait_check: &dyn Fn(&AuditEnvelope) -> anyhow::Result<()> = &|env| {
        let hook_buf = SiemAuditBuffer::new(4);
        hook_buf.push(env.clone())
    };
    let _type_check: Option<&dyn EmitAuditHook> = None;
}
