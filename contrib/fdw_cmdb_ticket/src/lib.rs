//! `contrib/fdw_cmdb_ticket` — CMDB 资产关联、变更工单轨 B 沙盒契约与 AIOps 取证交接包

use std::path::Path;
use std::sync::Arc;

use agent_body_protocol::{
    CapabilitiesSection, ExtensionControlManifest, ExtensionSection, SandboxSection, SchemaSection,
};
use agent_body_sdk::{
    body_module_magic, types::rfc3339_beijing_now, AutonomyLevel, BodyExtension, EntityScanFilter,
    EvidenceTuple, ExtensionKind, ExtensionMeta, ExtensionRegistrar, FunctionCallInfo,
    KernelServices, MigrationScript, ProcRoutine, ProcSpec, ProcVolatility, RowFilterPredicate,
    TypeRoutine, SDK_API_VERSION,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const MIGRATION_SQL: &str = include_str!("../sql/fdw_cmdb_ticket--0.1.0.sql");

/// 内嵌零依赖 Python 3 轨 B 进程外适配器源码
pub const CMDB_TICKET_FDW_SCRIPT: &str = include_str!("../scripts/cmdb_ticket_fdw.py");

/// 构建经过严格校验的轨 B 进程外沙盒 `.control.toml` 结构清单
pub fn build_track_b_sandbox_manifest(
    script_path: &Path,
    fixture_path: Option<&Path>,
    timeout_ms: u64,
) -> anyhow::Result<ExtensionControlManifest> {
    let mut args = vec![script_path.to_string_lossy().to_string()];
    if let Some(fix) = fixture_path {
        args.push(fix.to_string_lossy().to_string());
    }
    let manifest = ExtensionControlManifest {
        extension: ExtensionSection {
            id: "fdw_cmdb_ticket".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::OutOfProcess,
            min_sdk_version: SDK_API_VERSION.to_string(),
            requires: vec![],
            relocatable: true,
        },
        schema: SchemaSection {
            migration_dir: Some("sql".to_string()),
            table_prefix: "ext_fdw_cmdb_ticket__".to_string(),
            default_autonomy: AutonomyLevel::L0Observe,
        },
        capabilities: CapabilitiesSection {
            provides_types: vec![
                "entity:cmdb_ci".to_string(),
                "entity:change_ticket".to_string(),
            ],
            provides_fdws: vec!["fdw:cmdb_ticket".to_string()],
            provides_workers: vec![],
            provides_hooks: vec![],
            provides_procs: vec![],
        },
        sandbox: Some(SandboxSection {
            transport: "stdio_jsonrpc".to_string(),
            command: "python3".to_string(),
            args,
            timeout_ms,
            max_output_bytes: 262_144,
            env_whitelist: vec![],
        }),
    };
    manifest.validate()?;
    Ok(manifest)
}

/// CMDB 与变更工单 + AIOps 证据包扩展入口
pub struct FdwCmdbTicketExtension;

impl FdwCmdbTicketExtension {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FdwCmdbTicketExtension {
    fn default() -> Self {
        Self::new()
    }
}

impl BodyExtension for FdwCmdbTicketExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "fdw_cmdb_ticket".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: SDK_API_VERSION.to_string(),
            table_prefix: "ext_fdw_cmdb_ticket__".to_string(),
            requires: vec![],
        }
    }

    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: MIGRATION_SQL.to_string(),
        });
        reg.register_type(Arc::new(CmdbEntityType("entity:cmdb_ci")));
        reg.register_type(Arc::new(CmdbEntityType("entity:change_ticket")));
        reg.register_proc(Arc::new(BodyAiopsEvidencePackProc));
        Ok(())
    }
}

body_module_magic!();

struct CmdbEntityType(&'static str);

impl TypeRoutine for CmdbEntityType {
    fn type_id(&self) -> &'static str {
        self.0
    }

    fn validate_in(&self, raw: &Value) -> anyhow::Result<Value> {
        Ok(raw.clone())
    }

    fn render_compact_summary(&self, val: &Value) -> String {
        val.get("ticket_id")
            .or_else(|| val.get("ci_code"))
            .and_then(|v| v.as_str())
            .unwrap_or(self.0)
            .to_string()
    }
}

struct BodyAiopsEvidencePackProc;

impl ProcRoutine for BodyAiopsEvidencePackProc {
    fn spec(&self) -> ProcSpec {
        ProcSpec {
            proc_name: "body_aiops_evidence_pack",
            cli_path: &["aiops", "evidence-pack"],
            rest_route: ("POST", "/api/v1/aiops/evidence-pack"),
            profiles: &["aiops", "dba", "full"],
            volatility: ProcVolatility::Stable,
            autonomy_level: AutonomyLevel::L0Observe,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "host_id": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 500 }
                }
            }),
            provenance_rule: Some(
                "仅汇总已采集并带 SHA-256 指纹的客观证据与时间线，未验证推断必须显式标注于 unverified_hypotheses",
            ),
        }
    }

    fn invoke(&self, fcinfo: FunctionCallInfo, ctx: &KernelServices) -> anyhow::Result<Value> {
        let host_filter = fcinfo
            .args
            .get("host_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string);
        let limit = fcinfo
            .args
            .get("limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize)
            .unwrap_or(200);

        let rls = if fcinfo.caller.tenant_id.is_some() || fcinfo.caller.env.is_some() {
            Some(RowFilterPredicate {
                tenant_id: fcinfo.caller.tenant_id.clone(),
                project_id: fcinfo.caller.project_id.clone(),
                env: fcinfo.caller.env.clone(),
                allowed_host_ids: None,
                custom_labels: Default::default(),
            })
        } else {
            None
        };

        let tuples = ctx.table_am().entity_scan(
            &EntityScanFilter {
                host_id: host_filter.clone(),
                limit,
                ..Default::default()
            },
            rls.as_ref(),
        )?;

        Ok(assemble_evidence_pack(
            host_filter.as_deref().unwrap_or("all"),
            &tuples,
        ))
    }
}

fn assemble_evidence_pack(host_scope: &str, tuples: &[EvidenceTuple]) -> Value {
    let mut evidence_hashes = Vec::with_capacity(tuples.len());
    let mut timeline = Vec::new();
    let mut verified_facts = Vec::new();
    let mut has_ticket = false;
    let mut has_jvm_or_proc = false;
    let mut haser = Sha256::new();

    for t in tuples {
        haser.update(t.entity_id.as_bytes());
        haser.update(b":");
        haser.update(t.evidence_sha256.as_bytes());
        haser.update(b"\n");

        evidence_hashes.push(json!({
            "evidence_id": t.evidence_id,
            "entity_id": t.entity_id,
            "entity_type": t.entity_type,
            "source_ext": t.source_ext,
            "host_id": t.host_id,
            "collected_at": t.collected_at,
            "redacted": t.redacted,
            "evidence_sha256": t.evidence_sha256,
        }));

        if t.entity_type == "entity:change_ticket" {
            has_ticket = true;
            let ts = t
                .payload
                .get("change_time")
                .and_then(|v| v.as_str())
                .unwrap_or(&t.collected_at);
            timeline.push(json!({
                "timestamp": ts,
                "kind": "change_ticket",
                "entity_id": t.entity_id,
                "summary": t.payload.get("title").and_then(|v| v.as_str()).unwrap_or(&t.entity_id),
                "evidence_sha256": t.evidence_sha256,
            }));
            verified_facts.push(json!({
                "fact": format!("发现变更工单 {} (timestamp={ts})", t.entity_id),
                "evidence_sha256": t.evidence_sha256,
            }));
        }

        if t.entity_type == "entity:linux_process" || t.entity_type == "entity:jvm_app" {
            has_jvm_or_proc = true;
            if let Some(started_at) = t.payload.get("started_at").and_then(|v| v.as_str()) {
                timeline.push(json!({
                    "timestamp": started_at,
                    "kind": t.entity_type,
                    "entity_id": t.entity_id,
                    "summary": t.payload.get("comm").or_else(|| t.payload.get("jar_or_main")).cloned().unwrap_or(Value::String(t.entity_id.clone())),
                    "evidence_sha256": t.evidence_sha256,
                }));
                verified_facts.push(json!({
                    "fact": format!("进程/JVM 实例 {} 启动时间 {started_at}", t.entity_id),
                    "evidence_sha256": t.evidence_sha256,
                }));
            }
        }
    }

    timeline.sort_by(|a, b| {
        a["timestamp"]
            .as_str()
            .unwrap_or("")
            .cmp(b["timestamp"].as_str().unwrap_or(""))
    });

    let mut unverified_hypotheses = vec![
        "当前证据包仅包含宿主机配置、进程快照与变更工单事实，根因判定仍需关联 APM / 监控时序指标与业务错误日志后确认".to_string(),
    ];
    if has_ticket && has_jvm_or_proc {
        unverified_hypotheses.push(
            "待核实假设：工单变更窗口与进程重启时间相近，需进一步对比变更前后配置文件差异及接口 5xx 曲线验证因果关系".to_string(),
        );
    }

    let digest = haser.finalize();
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut pack_sha256 = String::with_capacity(64);
    for b in digest {
        pack_sha256.push(HEX[(b >> 4) as usize] as char);
        pack_sha256.push(HEX[(b & 0x0f) as usize] as char);
    }

    json!({
        "host_scope": host_scope,
        "generated_at": rfc3339_beijing_now(),
        "tuple_count": tuples.len(),
        "pack_sha256": pack_sha256,
        "evidence_hashes": evidence_hashes,
        "change_timeline": timeline,
        "verified_facts": verified_facts,
        "unverified_hypotheses": unverified_hypotheses,
    })
}
