//! `agent-body-dba` (`distributions/dba-sentinel`) 运行时装配与自检

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_body_kernel::KernelRuntime;
use agent_body_sdk::{
    BodyExtension, ClientAuthContext, EvidenceTuple, ProbeContext, TransportKind,
};
use anyhow::{bail, Context};
use ext_hook_audit_siem::{HookAuditSiemExtension, SiemAuditBuffer, SiemFormat, SiemSinkConfig};
use ext_hook_gov_compliance::{GovComplianceConfig, HookGovComplianceExtension};
use ext_idx_fts_fuzzy::{FtsFuzzyIndexAm, IdxFtsFuzzyExtension};
use ext_probe_db_engines::ProbeDbEnginesExtension;
use ext_probe_linux_ops::ProbeLinuxOpsExtension;

/// DBA 哨兵发行版配置
#[derive(Debug, Clone)]
pub struct DbaSentinelConfig {
    pub db_roots: Vec<PathBuf>,
    pub os_root: PathBuf,
    pub host_id: String,
    pub host_labels: BTreeMap<String, String>,
}

impl Default for DbaSentinelConfig {
    fn default() -> Self {
        Self {
            db_roots: vec![PathBuf::from("/")],
            os_root: PathBuf::from("/"),
            host_id: "dba-node-01".to_string(),
            host_labels: BTreeMap::new(),
        }
    }
}

/// 装配完成的 `dba-sentinel` 运行态上下文
pub struct DbaSentinelRuntime {
    pub runtime: KernelRuntime,
    pub index_am: Arc<FtsFuzzyIndexAm>,
    pub audit_buffer: Arc<SiemAuditBuffer>,
}

/// 构建 `dba-sentinel` 微内核 + 5 个数据库诊断 `contrib` 插件运行时
pub fn build_dba_runtime(
    cfg: &DbaSentinelConfig,
    db_path: Option<&Path>,
) -> anyhow::Result<DbaSentinelRuntime> {
    let mut runtime = match db_path {
        Some(p) => KernelRuntime::open_sqlite(p, None)?,
        None => KernelRuntime::open_sqlite_in_memory(None)?,
    };

    let idx_ext = IdxFtsFuzzyExtension::new();
    let index_am = idx_ext.index_am();
    let siem_ext = HookAuditSiemExtension::new(SiemSinkConfig {
        app_name: "agent-body-dba".to_string(),
        host_name: cfg.host_id.clone(),
        ring_capacity: 2048,
        format: SiemFormat::Rfc5424Syslog,
    });
    let audit_buffer = siem_ext.buffer();

    let exts: Vec<Box<dyn BodyExtension>> = vec![
        Box::new(idx_ext),
        Box::new(HookGovComplianceExtension::new(GovComplianceConfig {
            host_labels: cfg.host_labels.clone(),
            custom_patterns: vec![],
            enable_pii_masking: true,
        })),
        Box::new(siem_ext),
        Box::new(ProbeLinuxOpsExtension::new(&cfg.os_root)),
        Box::new(ProbeDbEnginesExtension::with_roots(cfg.db_roots.clone())),
    ];
    runtime.install_extensions(&exts)?;

    Ok(DbaSentinelRuntime {
        runtime,
        index_am,
        audit_buffer,
    })
}

/// 写入并索引探针证据元组
pub fn ingest_tuples(
    dba_rt: &DbaSentinelRuntime,
    tuples: &[EvidenceTuple],
) -> anyhow::Result<usize> {
    use agent_body_sdk::IndexAmRoutine;
    for t in tuples {
        dba_rt.runtime.table_am().entity_upsert(t)?;
        dba_rt.index_am.aminsert(t)?;
    }
    Ok(tuples.len())
}

fn write_sample(p: &Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(p, content)?;
    Ok(())
}

/// 执行 `agent-body-dba self-check` 离线自检
pub fn run_self_check() -> anyhow::Result<()> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!(
        "agent_body_dba_check_{}_{}",
        std::process::id(),
        nanos
    ));
    std::fs::create_dir_all(&tmp)
        .with_context(|| format!("创建自检临时目录失败: {}", tmp.display()))?;
    let res = run_self_check_inner(&tmp);
    let rm_res = std::fs::remove_dir_all(&tmp);
    if rm_res.is_err() && res.is_ok() {
        return rm_res.map_err(Into::into);
    }
    res
}

fn run_self_check_inner(tmp: &Path) -> anyhow::Result<()> {
    let db_root = tmp.join("db_confs");
    let os_root = tmp.join("os_root");
    write_sample(
        &db_root.join("pg/postgresql.conf"),
        "port = 5432\nmax_connections = 500\nshared_buffers = '8GB'\n# dsn: postgresql://postgres:PgSecret#1@127.0.0.1:5432/app\n",
    )?;
    write_sample(
        &db_root.join("dm/dm.ini"),
        "INSTANCE_NAME = DMGOV01\nPORT_NUM = 5236\nMAX_SESSIONS = 1500\n",
    )?;
    write_sample(
        &db_root.join("kingbase/kingbase.conf"),
        "port = 54321\nmax_connections = 600\n",
    )?;
    write_sample(
        &os_root.join("proc/5001/cmdline"),
        "postgres\0-D\0/var/lib/pgsql/data\0",
    )?;

    let cfg = DbaSentinelConfig {
        db_roots: vec![db_root],
        os_root,
        host_id: "dba-selfcheck".to_string(),
        host_labels: BTreeMap::new(),
    };
    let dba_rt = build_dba_runtime(&cfg, Some(&tmp.join("dba.db")))?;
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
        target_host: Some("dba-selfcheck".to_string()),
        timeout_ms: 3000,
        ..Default::default()
    };
    let db_tuples = dba_rt
        .runtime
        .scan_fdw("fdw:db_engines", None, &mut ctx, &auth)?;
    if db_tuples.len() < 3 {
        bail!("DBA 自检应识别 PG/DM/Kingbase 至少 3 个实例，实际 {}", db_tuples.len());
    }
    let dumped = serde_json::to_string(&db_tuples)?;
    if dumped.contains("PgSecret#1") {
        bail!("检测到数据库配置 URI 密码未脱敏");
    }
    let ingested = ingest_tuples(&dba_rt, &db_tuples)?;

    println!(
        "✅ dba-sentinel self-check passed (ext={}, db_instances={ingested}, audit={})",
        dba_rt.runtime.installed_metas().len(),
        dba_rt.audit_buffer.len()
    );
    Ok(())
}
