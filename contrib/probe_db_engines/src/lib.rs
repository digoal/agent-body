//! `ext-probe-db-engines`: 跨平台 PostgreSQL / 达梦 / 人大金仓配置探针 FDW 扩展

pub mod parser;

use std::path::PathBuf;
use std::sync::Arc;

use agent_body_sdk::{
    body_module_magic, BodyExtension, EvidenceTuple, ExtensionKind, ExtensionMeta,
    ExtensionRegistrar, FdwRoutine, FdwScanState, FdwSourceHealth, MigrationScript, ProbeContext,
    TypeRoutine, SDK_API_VERSION,
};
use anyhow::bail;

use parser::probe_roots_to_tuples;

body_module_magic!();

const MIGRATION_0_1_0: &str = include_str!("../sql/probe_db_engines--0.1.0.sql");

/// 多引擎数据库配置探针扩展入口
#[derive(Debug, Clone, Default)]
pub struct ProbeDbEnginesExtension {
    default_roots: Vec<PathBuf>,
}

impl ProbeDbEnginesExtension {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_roots(roots: Vec<PathBuf>) -> Self {
        Self {
            default_roots: roots,
        }
    }

    pub fn fdw(&self) -> DbEnginesFdw {
        DbEnginesFdw::new(self.default_roots.clone())
    }
}

impl BodyExtension for ProbeDbEnginesExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "probe_db_engines".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: SDK_API_VERSION.to_string(),
            table_prefix: "ext_probe_db_engines__".to_string(),
            requires: vec![],
        }
    }

    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: MIGRATION_0_1_0.to_string(),
        });
        reg.register_type(Arc::new(DbInstanceType));
        reg.register_fdw(Arc::new(DbEnginesFdw::new(self.default_roots.clone())));
        Ok(())
    }
}

/// `entity:db_instance` 开放实体类型契约
pub struct DbInstanceType;

impl TypeRoutine for DbInstanceType {
    fn type_id(&self) -> &'static str {
        "entity:db_instance"
    }

    fn validate_in(&self, raw: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let Some(obj) = raw.as_object() else {
            bail!("entity:db_instance 必须为 JSON 对象");
        };
        let has_engine = obj
            .get("engine")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.trim().is_empty());
        let has_cfg = obj
            .get("config_path")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.trim().is_empty());
        if !has_engine || !has_cfg {
            bail!("entity:db_instance 必须包含非空 'engine' 与 'config_path' 字段");
        }
        Ok(raw.clone())
    }

    fn render_compact_summary(&self, val: &serde_json::Value) -> String {
        let engine = val
            .get("engine")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let port = val
            .get("port")
            .and_then(|v| v.as_u64())
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        let data_dir = val
            .get("data_dir")
            .and_then(|v| v.as_str())
            .unwrap_or("-");
        format!("[{engine}] port={port} data_dir={data_dir}")
    }
}

/// `fdw:db_engines` 外部配置探针包装器
#[derive(Debug, Clone, Default)]
pub struct DbEnginesFdw {
    default_roots: Vec<PathBuf>,
}

impl DbEnginesFdw {
    pub fn new(default_roots: Vec<PathBuf>) -> Self {
        Self { default_roots }
    }

    fn resolve_roots(&self, target: Option<&str>, ctx: &ProbeContext) -> Vec<PathBuf> {
        if let Some(t) = target {
            let parsed: Vec<PathBuf> = std::env::split_paths(t)
                .filter(|p| !p.as_os_str().is_empty())
                .collect();
            if !parsed.is_empty() {
                return parsed;
            }
        }
        if let Some(raw_roots) = ctx.params.get("scan_roots") {
            let trimmed = raw_roots.trim();
            if trimmed.starts_with('[') {
                if let Ok(arr) = serde_json::from_str::<Vec<String>>(trimmed) {
                    let parsed: Vec<PathBuf> = arr
                        .into_iter()
                        .filter(|s| !s.trim().is_empty())
                        .map(PathBuf::from)
                        .collect();
                    if !parsed.is_empty() {
                        return parsed;
                    }
                }
            }
            let parsed: Vec<PathBuf> = std::env::split_paths(trimmed)
                .filter(|p| !p.as_os_str().is_empty())
                .collect();
            if !parsed.is_empty() {
                return parsed;
            }
        }
        self.default_roots.clone()
    }
}

impl FdwRoutine for DbEnginesFdw {
    fn fdw_id(&self) -> &'static str {
        "fdw:db_engines"
    }

    fn supported_types(&self) -> &'static [&'static str] {
        &["entity:db_instance"]
    }

    fn explain_source(&self, ctx: &ProbeContext) -> FdwSourceHealth {
        let roots = self.resolve_roots(None, ctx);
        if roots.is_empty() {
            return FdwSourceHealth {
                healthy: false,
                degraded_reason: Some("未配置扫描根目录 (scan_roots)".to_string()),
                missing_deps: vec!["scan_roots".to_string()],
            };
        }
        let existing = roots.iter().filter(|r| r.exists()).count();
        if existing == 0 {
            return FdwSourceHealth {
                healthy: false,
                degraded_reason: Some("全部扫描根目录均不存在".to_string()),
                missing_deps: Vec::new(),
            };
        }
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
        let roots = self.resolve_roots(target, ctx);
        let host_id = ctx
            .target_host
            .as_deref()
            .or_else(|| ctx.params.get("host_id").map(String::as_str))
            .unwrap_or("localhost");
        let tuples = probe_roots_to_tuples(&roots, host_id, ctx.rls_filter.as_ref())?;
        Ok(Box::new(DbEnginesScanState {
            tuples,
            cursor: 0,
            closed: false,
        }))
    }

    fn poll_drift(
        &self,
        cursor: Option<&str>,
        ctx: &ProbeContext,
    ) -> anyhow::Result<(Vec<EvidenceTuple>, Option<String>)> {
        let roots = self.resolve_roots(None, ctx);
        let host_id = ctx
            .target_host
            .as_deref()
            .or_else(|| ctx.params.get("host_id").map(String::as_str))
            .unwrap_or("localhost");
        let tuples = probe_roots_to_tuples(&roots, host_id, ctx.rls_filter.as_ref())?;
        let fingerprint = tuples
            .iter()
            .map(|t| t.evidence_sha256.as_str())
            .collect::<Vec<_>>()
            .join(",");
        if cursor == Some(fingerprint.as_str()) {
            Ok((Vec::new(), Some(fingerprint)))
        } else {
            Ok((tuples, Some(fingerprint)))
        }
    }
}

struct DbEnginesScanState {
    tuples: Vec<EvidenceTuple>,
    cursor: usize,
    closed: bool,
}

impl FdwScanState for DbEnginesScanState {
    fn iterate_scan(&mut self, batch_limit: usize) -> anyhow::Result<Vec<EvidenceTuple>> {
        if self.closed || self.cursor >= self.tuples.len() {
            return Ok(Vec::new());
        }
        let step = if batch_limit == 0 {
            self.tuples.len() - self.cursor
        } else {
            batch_limit.min(self.tuples.len() - self.cursor)
        };
        let end = self.cursor + step;
        let slice = self.tuples[self.cursor..end].to_vec();
        self.cursor = end;
        Ok(slice)
    }

    fn end_scan(&mut self) -> anyhow::Result<()> {
        self.closed = true;
        Ok(())
    }
}
