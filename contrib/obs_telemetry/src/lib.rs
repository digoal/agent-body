//! `contrib/obs_telemetry` — 标准可观测导出与自诊断扩展（第 10 个 `contrib/` 五件套）
//!
//! - `body_obs_metrics` (`GET /api/v1/obs/metrics` / `obs metrics` / MCP): Prometheus 0.0.4 文本或 JSON 指标
//! - `body_obs_diagnose` (`POST /api/v1/obs/diagnose` / `obs diagnose` / MCP): 类 PG 活跃/Top 算子/慢调用与 5 条异常规则
//! - 严格单向依赖 `agent-body-sdk`，通过 `ctx.observability_snapshot()` 只读获取微内核遥测态

pub mod diagnose;
pub mod prometheus;

use std::sync::Arc;

use agent_body_sdk::{
    body_module_magic, AutonomyLevel, BodyExtension, ExtensionKind, ExtensionMeta,
    ExtensionRegistrar, FunctionCallInfo, KernelObservabilitySnapshot, KernelServices,
    MigrationScript, ProcRoutine, ProcSpec, ProcVolatility, SDK_API_VERSION,
};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_json::json;

pub use diagnose::{evaluate_diagnostics, ObsDiagnoseThresholds};
pub use prometheus::{
    format_jsonl_obs_event, format_w3c_traceparent, read_proc_self_rss_and_fds,
    render_prometheus_text,
};

const MIGRATION_SQL: &str = include_str!("../sql/obs_telemetry--0.1.0.sql");

/// 可观测扩展配置
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObsTelemetryConfig {
    pub service_name: String,
    pub host_id: String,
    pub thresholds: ObsDiagnoseThresholds,
}

impl Default for ObsTelemetryConfig {
    fn default() -> Self {
        Self {
            service_name: "agent-body".to_string(),
            host_id: "localhost".to_string(),
            thresholds: ObsDiagnoseThresholds::default(),
        }
    }
}

/// 第 10 个周边扩展：统一 Prometheus / W3C Trace / JSONL 与异常自诊断
pub struct ObsTelemetryExtension {
    config: ObsTelemetryConfig,
}

impl ObsTelemetryExtension {
    pub fn new(config: ObsTelemetryConfig) -> Self {
        Self { config }
    }
}

impl Default for ObsTelemetryExtension {
    fn default() -> Self {
        Self::new(ObsTelemetryConfig::default())
    }
}

impl BodyExtension for ObsTelemetryExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "obs_telemetry".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: SDK_API_VERSION.to_string(),
            table_prefix: "ext_obs_telemetry__".to_string(),
            requires: vec![],
        }
    }

    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: MIGRATION_SQL.to_string(),
        });
        reg.register_proc(Arc::new(BodyObsMetricsProc {
            config: self.config.clone(),
        }));
        reg.register_proc(Arc::new(BodyObsDiagnoseProc {
            config: self.config.clone(),
        }));
        Ok(())
    }
}

body_module_magic!();

fn read_siem_dropped_from_kv(ctx: &KernelServices) -> u64 {
    ctx.table_am()
        .kv_get("ext_hook_audit_siem", "last_flush")
        .ok()
        .flatten()
        .and_then(|v| v.get("dropped_total").and_then(|d| d.as_u64()))
        .unwrap_or(0)
}

fn require_obs_snapshot(ctx: &KernelServices) -> anyhow::Result<KernelObservabilitySnapshot> {
    ctx.observability_snapshot()
        .ok_or_else(|| anyhow!("当前运行时未挂载 ObservabilityProvider"))
}

struct BodyObsMetricsProc {
    config: ObsTelemetryConfig,
}

impl ProcRoutine for BodyObsMetricsProc {
    fn spec(&self) -> ProcSpec {
        ProcSpec {
            proc_name: "body_obs_metrics",
            cli_path: &["obs", "metrics"],
            rest_route: ("GET", "/api/v1/obs/metrics"),
            profiles: &["aiops", "dba", "core"],
            volatility: ProcVolatility::Stable,
            autonomy_level: AutonomyLevel::L0Observe,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "format": {
                        "type": "string",
                        "enum": ["prometheus", "json"],
                        "default": "prometheus"
                    }
                }
            }),
            provenance_rule: None,
        }
    }

    fn invoke(
        &self,
        fcinfo: FunctionCallInfo,
        ctx: &KernelServices,
    ) -> anyhow::Result<serde_json::Value> {
        let snap = require_obs_snapshot(ctx)?;
        let siem_dropped = read_siem_dropped_from_kv(ctx);
        let fmt = fcinfo
            .args
            .get("format")
            .and_then(|v| v.as_str())
            .unwrap_or("prometheus");

        if fmt.eq_ignore_ascii_case("json") {
            let (rss_bytes, open_fds) = read_proc_self_rss_and_fds();
            return Ok(json!({
                "format": "json",
                "service": self.config.service_name,
                "host_id": self.config.host_id,
                "trace_id": fcinfo.trace_id,
                "traceparent": format_w3c_traceparent(&fcinfo.trace_id, true),
                "siem_dropped": siem_dropped,
                "process_rss_bytes": rss_bytes,
                "process_open_fds": open_fds,
                "snapshot": snap,
            }));
        }

        let metrics_text = render_prometheus_text(
            &snap,
            &self.config.service_name,
            &self.config.host_id,
            Some(siem_dropped),
        );
        Ok(json!({
            "format": "prometheus",
            "content_type": "text/plain; version=0.0.4; charset=utf-8",
            "trace_id": fcinfo.trace_id,
            "traceparent": format_w3c_traceparent(&fcinfo.trace_id, true),
            "metrics_text": metrics_text,
        }))
    }
}

struct BodyObsDiagnoseProc {
    config: ObsTelemetryConfig,
}

impl ProcRoutine for BodyObsDiagnoseProc {
    fn spec(&self) -> ProcSpec {
        ProcSpec {
            proc_name: "body_obs_diagnose",
            cli_path: &["obs", "diagnose"],
            rest_route: ("POST", "/api/v1/obs/diagnose"),
            profiles: &["aiops", "dba", "core"],
            volatility: ProcVolatility::Stable,
            autonomy_level: AutonomyLevel::L0Observe,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 }
                }
            }),
            provenance_rule: None,
        }
    }

    fn invoke(
        &self,
        fcinfo: FunctionCallInfo,
        ctx: &KernelServices,
    ) -> anyhow::Result<serde_json::Value> {
        let snap = require_obs_snapshot(ctx)?;
        let siem_dropped = read_siem_dropped_from_kv(ctx);
        let limit = fcinfo
            .args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(20) as usize;
        let mut report =
            evaluate_diagnostics(&snap, siem_dropped, limit, &self.config.thresholds);
        if let Some(obj) = report.as_object_mut() {
            obj.insert("service".to_string(), json!(self.config.service_name));
            obj.insert("host_id".to_string(), json!(self.config.host_id));
            obj.insert("trace_id".to_string(), json!(fcinfo.trace_id));
            obj.insert(
                "traceparent".to_string(),
                json!(format_w3c_traceparent(&fcinfo.trace_id, true)),
            );
        }
        Ok(report)
    }
}
