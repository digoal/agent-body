//! `contrib/hook_audit_siem` — 统一只读/写入/鉴权拒绝审计与 SIEM 异步外发插件
//!
//! - `EmitAuditHook` (`FailOpen`)：有界环形缓冲（默认 4096）承接内核全链路 `AuditEnvelope`
//! - `BgWorkerRoutine` (`worker:siem_flush`)：定时将审计事件刷新至 `siem_outbox` 流并记录水线
//! - 原生格式化器：RFC 5424 Syslog 与 OpenTelemetry (OTLP JSON) LogRecord

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use agent_body_sdk::{
    body_module_magic, types::rfc3339_beijing_now, AuditEnvelope, AutonomyLevel, BgWorkerRoutine,
    BodyExtension, CancellationToken, EmitAuditHook, ExtensionKind, ExtensionMeta,
    ExtensionRegistrar, FailPolicy, HookRegistration, KernelEventRecord, KernelServices,
    MigrationScript, WorkerTickOutcome, WorkerTrigger, SDK_API_VERSION,
};
use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MIGRATION_SQL: &str = include_str!("../sql/hook_audit_siem--0.1.0.sql");

/// SIEM 外发格式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiemFormat {
    Rfc5424Syslog,
    OtlpJson,
}

/// SIEM 审计插件配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiemSinkConfig {
    pub app_name: String,
    pub host_name: String,
    pub ring_capacity: usize,
    pub format: SiemFormat,
}

impl Default for SiemSinkConfig {
    fn default() -> Self {
        Self {
            app_name: "agent-body".to_string(),
            host_name: "localhost".to_string(),
            ring_capacity: 4096,
            format: SiemFormat::Rfc5424Syslog,
        }
    }
}

/// 线程安全有界 SIEM 审计环形缓冲区
pub struct SiemAuditBuffer {
    capacity: usize,
    queue: Mutex<VecDeque<AuditEnvelope>>,
    total_received: AtomicU64,
    dropped_count: AtomicU64,
}

impl SiemAuditBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            queue: Mutex::new(VecDeque::with_capacity(capacity.clamp(1, 512))),
            total_received: AtomicU64::new(0),
            dropped_count: AtomicU64::new(0),
        }
    }

    pub fn push(&self, envelope: AuditEnvelope) -> anyhow::Result<()> {
        let mut guard = self
            .queue
            .lock()
            .map_err(|_| anyhow!("siem ring buffer lock poisoned"))?;
        self.total_received.fetch_add(1, Ordering::Relaxed);
        if guard.len() >= self.capacity {
            guard.pop_front();
            self.dropped_count.fetch_add(1, Ordering::Relaxed);
        }
        guard.push_back(envelope);
        Ok(())
    }

    pub fn snapshot(&self) -> Vec<AuditEnvelope> {
        match self.queue.lock() {
            Ok(guard) => guard.iter().cloned().collect(),
            Err(_) => Vec::new(),
        }
    }

    pub fn drain_all(&self) -> anyhow::Result<Vec<AuditEnvelope>> {
        let mut guard = self
            .queue
            .lock()
            .map_err(|_| anyhow!("siem ring buffer lock poisoned"))?;
        Ok(guard.drain(..).collect())
    }

    pub fn len(&self) -> usize {
        self.queue.lock().map(|g| g.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn total_received(&self) -> u64 {
        self.total_received.load(Ordering::Relaxed)
    }

    pub fn dropped_count(&self) -> u64 {
        self.dropped_count.load(Ordering::Relaxed)
    }
}

/// 格式化单条 RFC 5424 Syslog 记录
pub fn format_rfc5424(env: &AuditEnvelope, host: &str, app: &str) -> String {
    let tenant = env.caller.tenant_id.as_deref().unwrap_or("-");
    let clean_target = env.target.replace(['\r', '\n'], " ");
    format!(
        "<134>1 {} {} {} - AUDIT [agentbody@59999 trace_id=\"{}\" caller=\"{}\" tenant=\"{}\" domain=\"{}\" op=\"{}\" outcome=\"{}\" sha256=\"{}\"] {}",
        env.recorded_at,
        host,
        app,
        escape_sd_param(&env.trace_id),
        escape_sd_param(&env.caller.caller_id),
        escape_sd_param(tenant),
        escape_sd_param(&env.domain),
        escape_sd_param(&env.operation),
        escape_sd_param(&env.outcome),
        escape_sd_param(&env.evidence_sha256),
        clean_target
    )
}

fn escape_sd_param(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace(']', "\\]")
}

/// 格式化单条 OpenTelemetry (OTLP JSON) LogRecord
pub fn format_otlp_log_record(env: &AuditEnvelope, host: &str, app: &str) -> Value {
    serde_json::json!({
        "resourceLogs": [{
            "resource": {
                "attributes": [
                    { "key": "service.name", "value": { "stringValue": app } },
                    { "key": "host.name", "value": { "stringValue": host } }
                ]
            },
            "scopeLogs": [{
                "logRecords": [{
                    "timeUnixNano": env.recorded_at,
                    "severityText": if env.outcome == "ok" { "INFO" } else { "WARN" },
                    "body": { "stringValue": format!("{} {} -> {}", env.operation, env.target, env.outcome) },
                    "attributes": [
                        { "key": "trace_id", "value": { "stringValue": env.trace_id } },
                        { "key": "caller.id", "value": { "stringValue": env.caller.caller_id } },
                        { "key": "tenant.id", "value": { "stringValue": env.caller.tenant_id.clone().unwrap_or_default() } },
                        { "key": "domain", "value": { "stringValue": env.domain } },
                        { "key": "outcome", "value": { "stringValue": env.outcome } },
                        { "key": "evidence.sha256", "value": { "stringValue": env.evidence_sha256 } }
                    ]
                }]
            }]
        }]
    })
}

/// SIEM 审计插件主体
pub struct HookAuditSiemExtension {
    config: SiemSinkConfig,
    buffer: Arc<SiemAuditBuffer>,
}

impl HookAuditSiemExtension {
    pub fn new(config: SiemSinkConfig) -> Self {
        let cap = config.ring_capacity;
        Self {
            config,
            buffer: Arc::new(SiemAuditBuffer::new(cap)),
        }
    }

    pub fn buffer(&self) -> Arc<SiemAuditBuffer> {
        Arc::clone(&self.buffer)
    }

    pub fn format_buffered_rfc5424(&self) -> Vec<String> {
        self.buffer
            .snapshot()
            .iter()
            .map(|e| format_rfc5424(e, &self.config.host_name, &self.config.app_name))
            .collect()
    }
}

impl BodyExtension for HookAuditSiemExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "hook_audit_siem".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: SDK_API_VERSION.to_string(),
            table_prefix: "ext_hook_audit_siem__".to_string(),
            requires: vec![],
        }
    }

    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: MIGRATION_SQL.to_string(),
        });
        reg.register_hook(HookRegistration::EmitAudit(Arc::new(SiemAuditSinkHook {
            buffer: Arc::clone(&self.buffer),
        })));
        reg.register_bgworker(Arc::new(SiemFlushWorker {
            config: self.config.clone(),
            buffer: Arc::clone(&self.buffer),
        }));
        Ok(())
    }
}

body_module_magic!();

struct SiemAuditSinkHook {
    buffer: Arc<SiemAuditBuffer>,
}

impl EmitAuditHook for SiemAuditSinkHook {
    fn hook_id(&self) -> &'static str {
        "hook:audit_siem_sink"
    }

    fn priority(&self) -> i32 {
        10
    }

    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailOpen
    }

    fn emit(&self, envelope: &AuditEnvelope) -> anyhow::Result<()> {
        self.buffer.push(envelope.clone())
    }
}

struct SiemFlushWorker {
    config: SiemSinkConfig,
    buffer: Arc<SiemAuditBuffer>,
}

impl BgWorkerRoutine for SiemFlushWorker {
    fn worker_id(&self) -> &'static str {
        "worker:siem_flush"
    }

    fn trigger(&self) -> WorkerTrigger {
        WorkerTrigger::Interval(Duration::from_secs(10))
    }

    fn autonomy_level(&self) -> AutonomyLevel {
        AutonomyLevel::L1Autonomic
    }

    fn run_tick(
        &self,
        ctx: &KernelServices,
        cancel: &CancellationToken,
    ) -> anyhow::Result<WorkerTickOutcome> {
        if cancel.is_cancelled() {
            return Ok(WorkerTickOutcome::default());
        }
        let batch = self.buffer.drain_all()?;
        if batch.is_empty() {
            return Ok(WorkerTickOutcome::default());
        }

        let mut emitted = 0usize;
        for (idx, env) in batch.iter().enumerate() {
            if cancel.is_cancelled() {
                break;
            }
            let payload = match self.config.format {
                SiemFormat::Rfc5424Syslog => serde_json::json!({
                    "format": "rfc5424",
                    "line": format_rfc5424(env, &self.config.host_name, &self.config.app_name),
                    "envelope": env,
                }),
                SiemFormat::OtlpJson => serde_json::json!({
                    "format": "otlp_json",
                    "record": format_otlp_log_record(env, &self.config.host_name, &self.config.app_name),
                }),
            };
            ctx.table_am().event_append(
                "siem_outbox",
                &KernelEventRecord {
                    event_id: format!("siem-{}-{idx}-{}", env.trace_id, ctx.trace_id),
                    stream: "siem_outbox".to_string(),
                    event_type: "siem.audit.export".to_string(),
                    source_ext: "hook_audit_siem".to_string(),
                    entity_id: None,
                    occurred_at: env.recorded_at.clone(),
                    payload,
                    evidence_sha256: env.evidence_sha256.clone(),
                },
            )?;
            emitted += 1;
        }

        ctx.table_am().kv_put(
            "ext_hook_audit_siem",
            "last_flush",
            &serde_json::json!({
                "flushed_at": rfc3339_beijing_now(),
                "emitted": emitted,
                "dropped_total": self.buffer.dropped_count(),
            }),
        )?;

        Ok(WorkerTickOutcome {
            processed_count: batch.len(),
            emitted_events: emitted,
            note: Some(format!("flushed {emitted} audit records to siem_outbox")),
        })
    }
}
