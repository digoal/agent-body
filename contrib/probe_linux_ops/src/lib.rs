//! `contrib/probe_linux_ops` — Linux/信创主机深度现场探针与漂移后台协程

pub mod automation_parser;
pub mod proc_parser;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use agent_body_sdk::{
    body_module_magic, AutonomyLevel, BgWorkerRoutine, BodyExtension, CancellationToken,
    EvidenceTuple, ExtensionKind, ExtensionMeta, ExtensionRegistrar, FdwRoutine, FdwScanState,
    FdwSourceHealth, KernelEventRecord, KernelServices, MigrationScript, ProbeContext, TypeRoutine,
    WorkerTickOutcome, WorkerTrigger,
};
use serde_json::Value;

use automation_parser::{scan_cron_all, scan_systemd_units};
use proc_parser::{scan_proc_root, DEFAULT_CMDLINE_MAX_LEN};

const MIGRATION_SQL: &str = include_str!("../sql/probe_linux_ops--0.1.0.sql");

/// Linux 深度探针扩展入口（支持注入可测试根目录 `/`）
pub struct ProbeLinuxOpsExtension {
    root_path: PathBuf,
    max_cmd_len: usize,
}

impl Default for ProbeLinuxOpsExtension {
    fn default() -> Self {
        Self::new("/")
    }
}

impl ProbeLinuxOpsExtension {
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
            max_cmd_len: DEFAULT_CMDLINE_MAX_LEN,
        }
    }

    pub fn with_max_cmd_len(mut self, max_cmd_len: usize) -> Self {
        self.max_cmd_len = max_cmd_len;
        self
    }
}

impl BodyExtension for ProbeLinuxOpsExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "probe_linux_ops".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: agent_body_sdk::SDK_API_VERSION.to_string(),
            table_prefix: "ext_probe_linux_ops__".to_string(),
            requires: Vec::new(),
        }
    }

    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: MIGRATION_SQL.to_string(),
        });
        for tid in [
            "entity:linux_process",
            "entity:linux_socket",
            "entity:linux_unit",
            "entity:linux_cron",
        ] {
            reg.register_type(Arc::new(LinuxOpsType(tid)));
        }
        reg.register_fdw(Arc::new(LinuxOpsFdw {
            root_path: self.root_path.clone(),
            max_cmd_len: self.max_cmd_len,
        }));
        reg.register_bgworker(Arc::new(ProcPortDriftWorker {
            root_path: self.root_path.clone(),
            max_cmd_len: self.max_cmd_len,
        }));
        Ok(())
    }
}

body_module_magic!();

struct LinuxOpsType(&'static str);

impl TypeRoutine for LinuxOpsType {
    fn type_id(&self) -> &'static str {
        self.0
    }

    fn validate_in(&self, raw: &Value) -> anyhow::Result<Value> {
        Ok(raw.clone())
    }

    fn render_compact_summary(&self, val: &Value) -> String {
        val.get("service_name")
            .or_else(|| val.get("comm"))
            .or_else(|| val.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or(self.0)
            .to_string()
    }
}

pub struct LinuxOpsFdw {
    pub root_path: PathBuf,
    pub max_cmd_len: usize,
}

impl LinuxOpsFdw {
    pub fn collect_tuples(
        &self,
        target: Option<&str>,
        ctx: &ProbeContext,
    ) -> anyhow::Result<Vec<EvidenceTuple>> {
        let host_id = ctx
            .target_host
            .clone()
            .unwrap_or_else(|| "local-host".to_string());
        let now = agent_body_sdk::types::rfc3339_beijing_now();
        let snap = scan_proc_root(&self.root_path, self.max_cmd_len);
        let units = scan_systemd_units(&self.root_path);
        let crons = scan_cron_all(&self.root_path);

        let mut tuples = Vec::new();
        for p in &snap.processes {
            let port_match = target
                .and_then(|t| t.parse::<u16>().ok())
                .map(|port| p.listening_sockets.iter().any(|s| s.local_port == port))
                .unwrap_or(true);
            if !port_match {
                continue;
            }
            let payload = serde_json::to_value(p)?;
            tuples.push(
                EvidenceTuple {
                    evidence_id: format!("ev-proc-{}", p.pid),
                    host_id: host_id.clone(),
                    host_labels: BTreeMap::new(),
                    entity_type: "entity:linux_process".to_string(),
                    entity_id: format!("proc:{}", p.pid),
                    source_ext: "probe_linux_ops".to_string(),
                    evidence_tier: "L1_EXPLICIT".to_string(),
                    collected_at: now.clone(),
                    ttl_sec: 15,
                    payload,
                    redacted: false,
                    evidence_sha256: String::new(),
                }
                .with_computed_sha256(),
            );
        }

        if target.is_none() || target == Some("units") || target == Some("all") {
            for u in &units {
                tuples.push(
                    EvidenceTuple {
                        evidence_id: format!("ev-unit-{}", u.service_name),
                        host_id: host_id.clone(),
                        host_labels: BTreeMap::new(),
                        entity_type: "entity:linux_unit".to_string(),
                        entity_id: format!("unit:{}", u.service_name),
                        source_ext: "probe_linux_ops".to_string(),
                        evidence_tier: "L1_EXPLICIT".to_string(),
                        collected_at: now.clone(),
                        ttl_sec: 30,
                        payload: serde_json::to_value(u)?,
                        redacted: false,
                        evidence_sha256: String::new(),
                    }
                    .with_computed_sha256(),
                );
            }
        }

        if target.is_none() || target == Some("crons") || target == Some("all") {
            for (idx, c) in crons.iter().enumerate() {
                tuples.push(
                    EvidenceTuple {
                        evidence_id: format!("ev-cron-{idx}"),
                        host_id: host_id.clone(),
                        host_labels: BTreeMap::new(),
                        entity_type: "entity:linux_cron".to_string(),
                        entity_id: format!("cron:{}:{idx}", c.user),
                        source_ext: "probe_linux_ops".to_string(),
                        evidence_tier: "L1_EXPLICIT".to_string(),
                        collected_at: now.clone(),
                        ttl_sec: 60,
                        payload: serde_json::to_value(c)?,
                        redacted: false,
                        evidence_sha256: String::new(),
                    }
                    .with_computed_sha256(),
                );
            }
        }

        Ok(tuples)
    }
}

struct VecScanState {
    items: Vec<EvidenceTuple>,
    cursor: usize,
}

impl FdwScanState for VecScanState {
    fn iterate_scan(&mut self, batch_limit: usize) -> anyhow::Result<Vec<EvidenceTuple>> {
        if self.cursor >= self.items.len() {
            return Ok(Vec::new());
        }
        let step = batch_limit.max(1);
        let end = (self.cursor + step).min(self.items.len());
        let batch = self.items[self.cursor..end].to_vec();
        self.cursor = end;
        Ok(batch)
    }

    fn end_scan(&mut self) -> anyhow::Result<()> {
        self.items.clear();
        Ok(())
    }
}

impl FdwRoutine for LinuxOpsFdw {
    fn fdw_id(&self) -> &'static str {
        "fdw:linux_ops"
    }

    fn supported_types(&self) -> &'static [&'static str] {
        &[
            "entity:linux_process",
            "entity:linux_socket",
            "entity:linux_unit",
            "entity:linux_cron",
        ]
    }

    fn explain_source(&self, _ctx: &ProbeContext) -> FdwSourceHealth {
        let status_path = self.root_path.join("proc/self/status");
        if let Ok(txt) = std::fs::read_to_string(&status_path) {
            let cap = proc_parser::check_capabilities_from_status(&txt);
            return FdwSourceHealth {
                healthy: cap.degraded_reason.is_none(),
                degraded_reason: cap.degraded_reason,
                missing_deps: Vec::new(),
            };
        }
        FdwSourceHealth {
            healthy: self.root_path.join("proc").is_dir(),
            degraded_reason: if self.root_path.join("proc").is_dir() {
                None
            } else {
                Some("procfs directory not present on host".to_string())
            },
            missing_deps: Vec::new(),
        }
    }

    fn begin_scan(
        &self,
        target: Option<&str>,
        ctx: &ProbeContext,
    ) -> anyhow::Result<Box<dyn FdwScanState>> {
        let items = self.collect_tuples(target, ctx)?;
        Ok(Box::new(VecScanState { items, cursor: 0 }))
    }

    fn poll_drift(
        &self,
        cursor: Option<&str>,
        ctx: &ProbeContext,
    ) -> anyhow::Result<(Vec<EvidenceTuple>, Option<String>)> {
        let tuples = self.collect_tuples(None, ctx)?;
        let sigs: BTreeSet<String> = tuples
            .iter()
            .map(|t| format!("{}:{}", t.entity_id, t.evidence_sha256))
            .collect();
        let next_cursor = serde_json::to_string(&sigs)?;
        if cursor == Some(next_cursor.as_str()) {
            return Ok((Vec::new(), Some(next_cursor)));
        }
        Ok((tuples, Some(next_cursor)))
    }
}

/// 进程与端口漂移探测后台协程
pub struct ProcPortDriftWorker {
    pub root_path: PathBuf,
    pub max_cmd_len: usize,
}

impl BgWorkerRoutine for ProcPortDriftWorker {
    fn worker_id(&self) -> &'static str {
        "worker:proc_port_drift"
    }

    fn trigger(&self) -> WorkerTrigger {
        WorkerTrigger::Interval(Duration::from_secs(30))
    }

    fn autonomy_level(&self) -> AutonomyLevel {
        AutonomyLevel::L0Observe
    }

    fn run_tick(
        &self,
        ctx: &KernelServices,
        cancel: &CancellationToken,
    ) -> anyhow::Result<WorkerTickOutcome> {
        if cancel.is_cancelled() {
            return Ok(WorkerTickOutcome::default());
        }
        let snap = scan_proc_root(&self.root_path, self.max_cmd_len);
        let curr_keys: BTreeSet<String> = snap
            .processes
            .iter()
            .flat_map(|p| {
                p.listening_sockets
                    .iter()
                    .map(move |s| format!("pid:{}:{}:{}", p.pid, s.local_ip, s.local_port))
            })
            .collect();

        let prev_keys: BTreeSet<String> = ctx
            .table_am()
            .kv_get("probe_linux_ops", "last_port_bindings")?
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        let added: Vec<String> = curr_keys.difference(&prev_keys).cloned().collect();
        let removed: Vec<String> = prev_keys.difference(&curr_keys).cloned().collect();
        let mut emitted = 0usize;

        if !added.is_empty() || !removed.is_empty() {
            let now = agent_body_sdk::types::rfc3339_beijing_now();
            let event = KernelEventRecord {
                event_id: format!("drift-{}", snap.processes.len()),
                stream: "host_drift".to_string(),
                event_type: "proc_port_drift".to_string(),
                source_ext: "probe_linux_ops".to_string(),
                entity_id: None,
                occurred_at: now.clone(),
                payload: serde_json::json!({
                    "added": added,
                    "removed": removed,
                }),
                evidence_sha256: String::new(),
            };
            ctx.table_am().event_append("host_drift", &event)?;
            emitted = 1;
        }

        ctx.table_am().kv_put(
            "probe_linux_ops",
            "last_port_bindings",
            &serde_json::to_value(&curr_keys)?,
        )?;

        Ok(WorkerTickOutcome {
            processed_count: snap.processes.len(),
            emitted_events: emitted,
            note: Some(format!("active_listeners={}", curr_keys.len())),
        })
    }
}
