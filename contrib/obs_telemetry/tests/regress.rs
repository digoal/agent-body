//! `contrib/obs_telemetry` 五件套隔离回归测试

use std::sync::{Arc, Mutex};

use agent_body_kernel::dispatcher::HttpRequest;
use agent_body_kernel::{ExtensionState, KernelRuntime};
use agent_body_sdk::{
    AuditEnvelope, AuthVerdict, ClientAuthContext, ClientAuthHook, EmitAuditHook, FailPolicy,
    HookRegistration, SlowOpRecord, TransportKind,
};
use ext_obs_telemetry::{
    format_jsonl_obs_event, format_w3c_traceparent, ObsTelemetryConfig, ObsTelemetryExtension,
};

struct TokenOnlyAuth;
impl ClientAuthHook for TokenOnlyAuth {
    fn hook_id(&self) -> &'static str {
        "hook:test_token_auth"
    }
    fn authenticate(&self, ctx: &ClientAuthContext) -> anyhow::Result<AuthVerdict> {
        if ctx.bearer_token.as_deref() == Some("valid-tok") {
            Ok(AuthVerdict::Allow(agent_body_sdk::CallerIdentity {
                caller_id: "tester".to_string(),
                ..Default::default()
            }))
        } else {
            Ok(AuthVerdict::Deny {
                status_code: 401,
                reason: "missing valid bearer token".to_string(),
            })
        }
    }
}

struct CaptureAudit(Arc<Mutex<Vec<AuditEnvelope>>>);
impl EmitAuditHook for CaptureAudit {
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

fn assert_prometheus_openmetrics_valid(metrics: &str) {
    assert!(metrics.contains("# HELP agent_body_kernel_wait_duration_seconds "));
    assert!(metrics.contains("# TYPE agent_body_kernel_wait_duration_seconds histogram"));

    let mut buckets_by_series: std::collections::BTreeMap<String, Vec<(String, u64)>> =
        std::collections::BTreeMap::new();
    let mut counts_by_series: std::collections::BTreeMap<String, u64> =
        std::collections::BTreeMap::new();

    for line in metrics.lines() {
        if let Some(rest) = line.strip_prefix("agent_body_kernel_wait_duration_seconds_bucket{") {
            let (labels, val_str) = rest.rsplit_once("} ").expect("valid bucket line");
            let val: u64 = val_str.trim().parse().expect("u64 bucket count");
            let (series_key, le) = labels
                .rsplit_once(",le=\"")
                .map(|(s, r)| (s.to_string(), r.trim_end_matches('"').to_string()))
                .expect("le label present");
            buckets_by_series.entry(series_key).or_default().push((le, val));
        } else if let Some(rest) =
            line.strip_prefix("agent_body_kernel_wait_duration_seconds_count{")
        {
            let (labels, val_str) = rest.rsplit_once("} ").expect("valid count line");
            let val: u64 = val_str.trim().parse().expect("u64 total count");
            counts_by_series.insert(labels.to_string(), val);
        }
    }

    assert!(!buckets_by_series.is_empty());
    for (series, entries) in &buckets_by_series {
        let mut prev = 0u64;
        let mut inf_val = None;
        for (le, val) in entries {
            assert!(*val >= prev, "bucket {series} le={le} non-monotonic: {val} < {prev}");
            prev = *val;
            if le == "+Inf" {
                inf_val = Some(*val);
            }
        }
        assert_eq!(
            inf_val,
            counts_by_series.get(series).copied(),
            "+Inf bucket must equal _count for {series}"
        );
    }
}

#[test]
fn obs_telemetry_install_prometheus_diagnose_and_trace_consistency() {
    let mut rt = KernelRuntime::open_sqlite_in_memory(Some(Arc::new(TokenOnlyAuth))).unwrap();
    let captured = Arc::new(Mutex::new(Vec::new()));
    rt.register_hook(HookRegistration::EmitAudit(Arc::new(CaptureAudit(
        Arc::clone(&captured),
    ))));

    let cfg = ObsTelemetryConfig {
        service_name: "obs-unit".to_string(),
        host_id: "node-unit".to_string(),
        ..Default::default()
    };
    rt.install_extensions(&[Box::new(ObsTelemetryExtension::new(cfg))])
        .unwrap();

    let rec = rt
        .get_extension_record("obs_telemetry")
        .unwrap()
        .expect("extension registered");
    assert_eq!(rec.state, ExtensionState::Enabled);

    rt.observability().set_slow_threshold_us(0);

    // 1. 触发一次 401 鉴权拒绝
    let unauth = rt.handle_http(&HttpRequest {
        method: "GET".to_string(),
        path: "/api/v1/catalog/extensions".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::Value::Null,
    });
    assert_eq!(unauth.status_code, 401);
    let unauth_trace = unauth.body["trace_id"].as_str().unwrap().to_string();

    // 2. 通过 CLI 读取 JSON 指标并检查 trace_id 与 AuditEnvelope 一致性
    let auth = ClientAuthContext {
        transport: TransportKind::Cli,
        cert: None,
        bearer_token: Some("valid-tok".to_string()),
        method: "GET".to_string(),
        path: "/api/v1/obs/metrics".to_string(),
        is_loopback: true,
        is_read_only: true,
    };
    let cli_res = rt
        .handle_cli(
            &["obs".to_string(), "metrics".to_string()],
            Some(serde_json::json!({"format": "prometheus"})),
            &auth,
        )
        .unwrap();
    let metrics_txt = cli_res.data["metrics_text"].as_str().unwrap();
    assert_prometheus_openmetrics_valid(metrics_txt);
    assert!(metrics_txt.contains("outcome=\"denied\"} 1"));

    let audits = captured.lock().unwrap().clone();
    assert!(audits
        .iter()
        .any(|a| a.trace_id == cli_res.trace_id && a.target == "body_obs_metrics"));
    assert!(audits
        .iter()
        .any(|a| a.trace_id == unauth_trace && a.outcome == "auth_denied"));

    // 3. MCP 调用 body_obs_diagnose 校验 OBS-01 与慢操作 trace_id 追踪
    let mcp_resp = rt.handle_mcp(
        &serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": { "name": "body_obs_diagnose", "arguments": { "limit": 10 } }
        }),
        &auth,
    );
    let diag = &mcp_resp["result"]["structuredContent"];
    assert_eq!(diag["health_grade"], "degraded");
    assert!(diag["anomalies"]
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a["rule_id"] == "OBS-01"));

    // 4. 校验 W3C traceparent 与 JSONL 格式
    let tp = format_w3c_traceparent(&cli_res.trace_id, true);
    let parts: Vec<&str> = tp.split('-').collect();
    assert_eq!(parts.len(), 4);
    assert_eq!(parts[0], "00");
    assert_eq!(parts[1].len(), 32);
    assert_eq!(parts[2].len(), 16);
    assert_eq!(parts[3], "01");

    let jsonl = format_jsonl_obs_event(
        "obs-unit",
        "node-unit",
        &SlowOpRecord {
            trace_id: cli_res.trace_id.clone(),
            kind: "proc".to_string(),
            target: "body_obs_metrics".to_string(),
            caller_id: "tester".to_string(),
            outcome: "ok".to_string(),
            elapsed_us: 1200,
            detail: None,
            recorded_at: String::new(),
        },
    );
    let parsed_jsonl: serde_json::Value = serde_json::from_str(&jsonl).unwrap();
    assert_eq!(parsed_jsonl["trace_id"], cli_res.trace_id);
    assert_eq!(parsed_jsonl["traceparent"], tp);
}
