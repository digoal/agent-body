//! `agent-body-probe` (`distributions/aiops-probe`) 运行时装配与自检

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use agent_body_kernel::dispatcher::HttpRequest;
use agent_body_kernel::{KernelRuntime, OutOfProcessFdwAdapter};
use agent_body_sdk::{
    BodyExtension, ClientAuthContext, EvidenceTuple, ProbeContext, TransportKind,
};
use anyhow::{bail, Context};
use ext_fdw_cmdb_ticket::{
    build_track_b_sandbox_manifest, FdwCmdbTicketExtension, CMDB_TICKET_FDW_SCRIPT,
};
use ext_hook_audit_siem::{HookAuditSiemExtension, SiemAuditBuffer, SiemFormat, SiemSinkConfig};
use ext_hook_auth_mtls_rbac::{
    HookAuthMtlsRbacExtension, MtlsRbacConfig, PrincipalPolicy,
};
use ext_hook_gov_compliance::{GovComplianceConfig, HookGovComplianceExtension};
use ext_idx_fts_fuzzy::{FtsFuzzyIndexAm, IdxFtsFuzzyExtension};
use ext_obs_telemetry::{ObsTelemetryConfig, ObsTelemetryExtension};
use ext_probe_jvm_middleware::ProbeJvmMiddlewareExtension;
use ext_probe_linux_ops::ProbeLinuxOpsExtension;

/// AIOps 探针发行版装配配置
#[derive(Debug, Clone)]
pub struct AiopsProbeConfig {
    pub root_path: PathBuf,
    pub host_id: String,
    pub host_labels: BTreeMap<String, String>,
    pub tokens: BTreeMap<String, PrincipalPolicy>,
    pub cert_cn_policies: BTreeMap<String, PrincipalPolicy>,
    pub custom_redact_patterns: Vec<String>,
    pub track_b_script: Option<PathBuf>,
    pub track_b_fixture: Option<PathBuf>,
    pub track_b_timeout_ms: u64,
}

impl Default for AiopsProbeConfig {
    fn default() -> Self {
        let mut host_labels = BTreeMap::new();
        host_labels.insert("tenant_id".to_string(), "dept-default".to_string());
        host_labels.insert("env".to_string(), "prod".to_string());

        let mut tokens = BTreeMap::new();
        tokens.insert(
            "local-probe-token".to_string(),
            PrincipalPolicy {
                caller_id: "probe-operator".to_string(),
                tenant_id: Some("dept-default".to_string()),
                project_id: None,
                env: Some("prod".to_string()),
                roles: vec!["operator".to_string()],
                allowed_entity_types: vec!["*".to_string()],
            },
        );

        Self {
            root_path: PathBuf::from("/"),
            host_id: "gov-node-01".to_string(),
            host_labels,
            tokens,
            cert_cn_policies: BTreeMap::new(),
            custom_redact_patterns: Vec::new(),
            track_b_script: None,
            track_b_fixture: None,
            track_b_timeout_ms: 3000,
        }
    }
}

/// 装配完成的 `aiops-probe` 运行态上下文
pub struct AiopsProbeRuntime {
    pub runtime: KernelRuntime,
    pub index_am: Arc<FtsFuzzyIndexAm>,
    pub audit_buffer: Arc<SiemAuditBuffer>,
    pub host_name: String,
}

/// 组装微内核与 7 个 `contrib/` 扩展及可选轨 B CMDB 沙盒适配器
pub fn build_aiops_runtime(
    cfg: &AiopsProbeConfig,
    db_path: Option<&Path>,
) -> anyhow::Result<AiopsProbeRuntime> {
    let mut runtime = match db_path {
        Some(p) => KernelRuntime::open_sqlite(p, None)?,
        None => KernelRuntime::open_sqlite_in_memory(None)?,
    };

    let idx_ext = IdxFtsFuzzyExtension::new();
    let index_am = idx_ext.index_am();
    let siem_ext = HookAuditSiemExtension::new(SiemSinkConfig {
        app_name: "agent-body-probe".to_string(),
        host_name: cfg.host_id.clone(),
        ring_capacity: 4096,
        format: SiemFormat::Rfc5424Syslog,
    });
    let audit_buffer = siem_ext.buffer();

    let exts: Vec<Box<dyn BodyExtension>> = vec![
        Box::new(idx_ext),
        Box::new(HookAuthMtlsRbacExtension::new(MtlsRbacConfig {
            tokens: cfg.tokens.clone(),
            cert_cn_policies: cfg.cert_cn_policies.clone(),
        })),
        Box::new(HookGovComplianceExtension::new(GovComplianceConfig {
            host_labels: cfg.host_labels.clone(),
            custom_patterns: cfg.custom_redact_patterns.clone(),
            enable_pii_masking: true,
        })),
        Box::new(siem_ext),
        Box::new(ProbeLinuxOpsExtension::new(&cfg.root_path)),
        Box::new(ProbeJvmMiddlewareExtension::new(&cfg.root_path)),
        Box::new(FdwCmdbTicketExtension::new()),
        Box::new(ObsTelemetryExtension::new(ObsTelemetryConfig {
            service_name: "agent-body-probe".to_string(),
            host_id: cfg.host_id.clone(),
            ..Default::default()
        })),
    ];
    runtime.install_extensions(&exts)?;

    if let Some(ref script_path) = cfg.track_b_script {
        let manifest = build_track_b_sandbox_manifest(
            script_path,
            cfg.track_b_fixture.as_deref(),
            cfg.track_b_timeout_ms,
        )?;
        let adapter = OutOfProcessFdwAdapter::from_manifest(
            manifest,
            None,
            Some(Arc::clone(runtime.waits())),
        )?
        .with_observability(Arc::clone(runtime.observability()));
        runtime.register_fdw(Arc::new(adapter));
    }

    Ok(AiopsProbeRuntime {
        runtime,
        index_am,
        audit_buffer,
        host_name: cfg.host_id.clone(),
    })
}

/// 将探针扫描得到的 `EvidenceTuple` 写入基表并同步索引
pub fn ingest_tuples(
    probe_rt: &AiopsProbeRuntime,
    tuples: &[EvidenceTuple],
) -> anyhow::Result<usize> {
    for tuple in tuples {
        probe_rt.runtime.table_am().entity_upsert(tuple)?;
        probe_sdk_index(&probe_rt.index_am, tuple)?;
    }
    Ok(tuples.len())
}

fn probe_sdk_index(idx: &FtsFuzzyIndexAm, tuple: &EvidenceTuple) -> anyhow::Result<()> {
    use agent_body_sdk::IndexAmRoutine;
    idx.aminsert(tuple)
}

fn write_sample(path: &Path, content: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

/// 执行 `agent-body-probe self-check` 完整离线闭环验证（含模拟 `/proc` / Nginx / Cron / Systemd / 轨 B 沙盒）
pub fn run_self_check() -> anyhow::Result<()> {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!(
        "agent_body_probe_check_{}_{}",
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
    let root = tmp.join("rootfs");
    write_sample(
        &root.join("proc/3001/cmdline"),
        "java\0-Xms1g\0-Dspring.datasource.password=SelfCheckSecret!\0-jar\0/opt/biz/portal.jar\0--server.port=8080\0",
    )?;
    write_sample(
        &root.join("etc/nginx/nginx.conf"),
        "http {\n  include /etc/nginx/conf.d/*.conf;\n}\n",
    )?;
    write_sample(
        &root.join("etc/nginx/conf.d/portal.conf"),
        "upstream biz_pool {\n  server 127.0.0.1:8080;\n}\nserver {\n  listen 80;\n  location / { proxy_pass http://biz_pool; }\n}\n",
    )?;
    write_sample(
        &root.join("var/spool/cron/root"),
        "0 3 * * * PGPASSWORD=CronSecret9 /usr/bin/pg_dump -h 127.0.0.1\n",
    )?;
    write_sample(
        &root.join("usr/lib/systemd/system/portal.service"),
        "[Service]\nExecStart=/usr/bin/java -jar /opt/biz/portal.jar\n",
    )?;

    let script_path = tmp.join("cmdb_ticket_fdw.py");
    write_sample(&script_path, CMDB_TICKET_FDW_SCRIPT)?;

    let cfg = AiopsProbeConfig {
        root_path: root,
        host_id: "selfcheck-node".to_string(),
        track_b_script: Some(script_path),
        ..Default::default()
    };
    let db_path = tmp.join("probe.db");
    let probe_rt = build_aiops_runtime(&cfg, Some(&db_path))?;

    // 1. 强制读鉴权验证
    let unauth = probe_rt.runtime.handle_http(&HttpRequest {
        method: "GET".to_string(),
        path: "/api/v1/catalog/extensions".to_string(),
        bearer_token: None,
        is_loopback: true,
        body: serde_json::Value::Null,
    });
    if unauth.status_code != 401 {
        bail!("未认证请求应返回 401，实际为 {}", unauth.status_code);
    }

    // 2. 合法凭据采集三大 FDW（Linux + JVM/Nginx + Track-B Python3 CMDB/Ticket）
    let auth = ClientAuthContext {
        transport: TransportKind::Internal,
        cert: None,
        bearer_token: Some("local-probe-token".to_string()),
        method: "GET".to_string(),
        path: "/api/v1/fdw".to_string(),
        is_loopback: true,
        is_read_only: true,
    };
    let mut all_tuples = Vec::new();
    for fdw_id in ["fdw:linux_ops", "fdw:jvm_middleware", "fdw:cmdb_ticket"] {
        let mut pctx = ProbeContext {
            target_host: Some("selfcheck-node".to_string()),
            timeout_ms: 3000,
            ..Default::default()
        };
        let batch = probe_rt.runtime.scan_fdw(fdw_id, None, &mut pctx, &auth)?;
        all_tuples.extend(batch);
    }
    let ingested = ingest_tuples(&probe_rt, &all_tuples)?;
    if ingested < 5 {
        bail!("自检采集元组数量过少: {ingested}");
    }

    // 3. 脱敏校验：Java -D 与 Cron PGPASSWORD 必须已掩码
    let raw_dump = serde_json::to_string(&all_tuples)?;
    if raw_dump.contains("SelfCheckSecret!") || raw_dump.contains("CronSecret9") {
        bail!("检测到未脱敏凭据泄露");
    }

    // 4. 证据包算子派发校验 + 全链路 trace_id 一致性校验
    let pack = probe_rt.runtime.handle_http(&HttpRequest {
        method: "POST".to_string(),
        path: "/api/v1/aiops/evidence-pack".to_string(),
        bearer_token: Some("local-probe-token".to_string()),
        is_loopback: true,
        body: serde_json::json!({"host_id": "selfcheck-node"}),
    });
    if pack.status_code != 200 || pack.body["data"]["tuple_count"].as_u64().unwrap_or(0) < 5 {
        bail!("证据包算子返回异常: {:?}", pack);
    }
    let pack_trace = pack.body["trace_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("证据包响应缺少 trace_id"))?;
    let siem_snap = probe_rt.audit_buffer.snapshot();
    if !siem_snap.iter().any(|e| e.trace_id == pack_trace) {
        bail!("SIEM 审计缓冲未关联到相同的 trace_id ({pack_trace})");
    }

    // 5. 可观测指标与自诊断校验
    let metrics_resp = probe_rt.runtime.handle_http(&HttpRequest {
        method: "GET".to_string(),
        path: "/api/v1/obs/metrics".to_string(),
        bearer_token: Some("local-probe-token".to_string()),
        is_loopback: true,
        body: serde_json::json!({"format": "prometheus"}),
    });
    let metrics_text = metrics_resp.body["data"]["metrics_text"]
        .as_str()
        .unwrap_or("");
    if metrics_resp.status_code != 200
        || !metrics_text.contains("agent_body_kernel_wait_duration_seconds_bucket")
    {
        bail!("obs metrics 返回异常: {:?}", metrics_resp);
    }
    let diag_resp = probe_rt.runtime.handle_http(&HttpRequest {
        method: "POST".to_string(),
        path: "/api/v1/obs/diagnose".to_string(),
        bearer_token: Some("local-probe-token".to_string()),
        is_loopback: true,
        body: serde_json::json!({"limit": 10}),
    });
    if diag_resp.status_code != 200
        || !diag_resp.body["data"]["anomalies"]
            .as_array()
            .map(|arr| arr.iter().any(|a| a["rule_id"] == "OBS-01"))
            .unwrap_or(false)
    {
        bail!("obs diagnose 应检出前置 401 鉴权拒绝 (OBS-01): {:?}", diag_resp);
    }

    let audit_count = probe_rt.audit_buffer.len();
    println!(
        "✅ aiops-probe self-check passed (ext={}, tuples={}, audit={audit_count}, trace={pack_trace})",
        probe_rt.runtime.installed_metas().len(),
        ingested
    );
    Ok(())
}
