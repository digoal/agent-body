//! `contrib/probe_jvm_middleware` — JVM/Tomcat/东方通 & Nginx 中间件拓扑探针

pub mod parsers;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use agent_body_sdk::{
    body_module_magic, BodyExtension, EvidenceTuple, ExtensionKind, ExtensionMeta,
    ExtensionRegistrar, FdwRoutine, FdwScanState, FdwSourceHealth, MigrationScript, ProbeContext,
    TypeRoutine,
};
use serde_json::Value;

use parsers::{parse_jvm_cmdline, parse_nginx_hierarchy};

const MIGRATION_SQL: &str = include_str!("../sql/probe_jvm_middleware--0.1.0.sql");

/// JVM 与 Nginx 中间件探针扩展
pub struct ProbeJvmMiddlewareExtension {
    root_path: PathBuf,
}

impl Default for ProbeJvmMiddlewareExtension {
    fn default() -> Self {
        Self::new("/")
    }
}

impl ProbeJvmMiddlewareExtension {
    pub fn new(root_path: impl Into<PathBuf>) -> Self {
        Self {
            root_path: root_path.into(),
        }
    }
}

impl BodyExtension for ProbeJvmMiddlewareExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "probe_jvm_middleware".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: agent_body_sdk::SDK_API_VERSION.to_string(),
            table_prefix: "ext_probe_jvm_middleware__".to_string(),
            requires: vec!["probe_linux_ops".to_string()],
        }
    }

    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: MIGRATION_SQL.to_string(),
        });
        for tid in ["entity:jvm_app", "entity:nginx_upstream", "edge:proxies_to"] {
            reg.register_type(Arc::new(MiddlewareType(tid)));
        }
        reg.register_fdw(Arc::new(JvmMiddlewareFdw {
            root_path: self.root_path.clone(),
        }));
        Ok(())
    }
}

body_module_magic!();

struct MiddlewareType(&'static str);

impl TypeRoutine for MiddlewareType {
    fn type_id(&self) -> &'static str {
        self.0
    }

    fn validate_in(&self, raw: &Value) -> anyhow::Result<Value> {
        Ok(raw.clone())
    }

    fn render_compact_summary(&self, val: &Value) -> String {
        val.get("jar_or_main")
            .or_else(|| val.get("upstream_name"))
            .and_then(|v| v.as_str())
            .unwrap_or(self.0)
            .to_string()
    }
}

pub struct JvmMiddlewareFdw {
    pub root_path: PathBuf,
}

impl JvmMiddlewareFdw {
    pub fn collect_tuples(
        &self,
        _target: Option<&str>,
        ctx: &ProbeContext,
    ) -> anyhow::Result<Vec<EvidenceTuple>> {
        let host_id = ctx
            .target_host
            .clone()
            .unwrap_or_else(|| "local-host".to_string());
        let now = agent_body_sdk::types::rfc3339_beijing_now();
        let mut tuples = Vec::new();

        // 1. 扫描 /proc/[pid]/cmdline 提取 JVM/Tomcat/东方通实例
        let proc_dir = self.root_path.join("proc");
        if proc_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&proc_dir) {
                let mut pids: Vec<(u32, PathBuf)> = entries
                    .flatten()
                    .filter_map(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .parse::<u32>()
                            .ok()
                            .map(|pid| (pid, e.path()))
                    })
                    .collect();
                pids.sort_by_key(|(pid, _)| *pid);
                for (pid, pdir) in pids {
                    if let Ok(bytes) = std::fs::read(pdir.join("cmdline")) {
                        let cmd = bytes
                            .split(|&b| b == 0)
                            .filter(|s| !s.is_empty())
                            .map(|s| String::from_utf8_lossy(s).to_string())
                            .collect::<Vec<_>>()
                            .join(" ");
                        if let Some(jvm) = parse_jvm_cmdline(pid, &cmd) {
                            tuples.push(
                                EvidenceTuple {
                                    evidence_id: format!("ev-jvm-{pid}"),
                                    host_id: host_id.clone(),
                                    host_labels: BTreeMap::new(),
                                    entity_type: "entity:jvm_app".to_string(),
                                    entity_id: format!("jvm:{pid}"),
                                    source_ext: "probe_jvm_middleware".to_string(),
                                    evidence_tier: "L1_EXPLICIT".to_string(),
                                    collected_at: now.clone(),
                                    ttl_sec: 30,
                                    payload: serde_json::to_value(&jvm)?,
                                    redacted: false,
                                    evidence_sha256: String::new(),
                                }
                                .with_computed_sha256(),
                            );
                        }
                    }
                }
            }
        }

        // 2. 扫描 Nginx 主配置与 include 拓扑
        let nginx_candidates = [
            self.root_path.join("etc/nginx/nginx.conf"),
            self.root_path.join("usr/local/nginx/conf/nginx.conf"),
        ];
        for conf in nginx_candidates {
            if conf.is_file() {
                let topo = parse_nginx_hierarchy(&conf, &self.root_path);
                for up in &topo.upstreams {
                    tuples.push(
                        EvidenceTuple {
                            evidence_id: format!("ev-nginx-up-{}", up.upstream_name),
                            host_id: host_id.clone(),
                            host_labels: BTreeMap::new(),
                            entity_type: "entity:nginx_upstream".to_string(),
                            entity_id: format!("nginx_upstream:{}", up.upstream_name),
                            source_ext: "probe_jvm_middleware".to_string(),
                            evidence_tier: "L1_EXPLICIT".to_string(),
                            collected_at: now.clone(),
                            ttl_sec: 60,
                            payload: serde_json::to_value(up)?,
                            redacted: false,
                            evidence_sha256: String::new(),
                        }
                        .with_computed_sha256(),
                    );
                }
                for (idx, route) in topo.proxy_routes.iter().enumerate() {
                    tuples.push(
                        EvidenceTuple {
                            evidence_id: format!("ev-nginx-route-{idx}"),
                            host_id: host_id.clone(),
                            host_labels: BTreeMap::new(),
                            entity_type: "entity:nginx_upstream".to_string(),
                            entity_id: format!("nginx_route:{idx}:{}", route.location),
                            source_ext: "probe_jvm_middleware".to_string(),
                            evidence_tier: "L1_EXPLICIT".to_string(),
                            collected_at: now.clone(),
                            ttl_sec: 60,
                            payload: serde_json::to_value(route)?,
                            redacted: false,
                            evidence_sha256: String::new(),
                        }
                        .with_computed_sha256(),
                    );
                }
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

impl FdwRoutine for JvmMiddlewareFdw {
    fn fdw_id(&self) -> &'static str {
        "fdw:jvm_middleware"
    }

    fn supported_types(&self) -> &'static [&'static str] {
        &["entity:jvm_app", "entity:nginx_upstream"]
    }

    fn explain_source(&self, _ctx: &ProbeContext) -> FdwSourceHealth {
        FdwSourceHealth {
            healthy: true,
            degraded_reason: None,
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
        _cursor: Option<&str>,
        ctx: &ProbeContext,
    ) -> anyhow::Result<(Vec<EvidenceTuple>, Option<String>)> {
        let items = self.collect_tuples(None, ctx)?;
        Ok((items, None))
    }
}
