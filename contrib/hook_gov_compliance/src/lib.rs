//! `contrib/hook_gov_compliance` — 政务云合规脱敏与宿主机维度标签注入插件
//!
//! - `TupleRedactHook` (`FailClosed`)：入库与出站前双相精度脱敏，保真保留 `-jar` / `--server.port`
//! - `PlannerRewriteHook` (`FailClosed`)：将配置中的 `host_labels` 注入 `ProbeContext.params`

pub mod redactor;

use std::sync::Arc;

use agent_body_sdk::{
    body_module_magic, BodyExtension, CallerIdentity, ExtensionKind, ExtensionMeta,
    ExtensionRegistrar, FailPolicy, FieldPathContext, HookRegistration, MigrationScript,
    PlanRewriteVerdict, PlannerRewriteHook, ProbeContext, RedactVerdict, SearchPlan,
    TupleRedactHook, SDK_API_VERSION,
};
use serde_json::Value;

pub use redactor::{GovComplianceConfig, GovComplianceRedactor};

const MIGRATION_SQL: &str = include_str!("../sql/hook_gov_compliance--0.1.0.sql");

/// 政务云合规脱敏与标签注入扩展入口
pub struct HookGovComplianceExtension {
    config: GovComplianceConfig,
}

impl HookGovComplianceExtension {
    pub fn new(config: GovComplianceConfig) -> Self {
        Self { config }
    }
}

impl BodyExtension for HookGovComplianceExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "hook_gov_compliance".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: SDK_API_VERSION.to_string(),
            table_prefix: "ext_hook_gov_compliance__".to_string(),
            requires: vec![],
        }
    }

    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: MIGRATION_SQL.to_string(),
        });
        let engine = Arc::new(GovComplianceRedactor::from_config(&self.config)?);
        reg.register_hook(HookRegistration::TupleRedact(Arc::new(
            GovTupleRedactHook { engine },
        )));
        reg.register_hook(HookRegistration::PlannerRewrite(Arc::new(
            HostLabelsInjectorHook {
                host_labels: self.config.host_labels.clone(),
            },
        )));
        Ok(())
    }
}

body_module_magic!();

struct GovTupleRedactHook {
    engine: Arc<GovComplianceRedactor>,
}

impl TupleRedactHook for GovTupleRedactHook {
    fn hook_id(&self) -> &'static str {
        "hook:gov_compliance_redact"
    }

    fn priority(&self) -> i32 {
        10
    }

    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailClosed
    }

    fn redact(
        &self,
        value: &mut Value,
        _path_ctx: &FieldPathContext,
        _caller: &CallerIdentity,
    ) -> anyhow::Result<RedactVerdict> {
        if self.engine.redact_json(value) {
            Ok(RedactVerdict::Redacted)
        } else {
            Ok(RedactVerdict::Unchanged)
        }
    }
}

struct HostLabelsInjectorHook {
    host_labels: std::collections::BTreeMap<String, String>,
}

impl PlannerRewriteHook for HostLabelsInjectorHook {
    fn hook_id(&self) -> &'static str {
        "hook:gov_host_labels"
    }

    fn priority(&self) -> i32 {
        20
    }

    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailClosed
    }

    fn rewrite_search_plan(
        &self,
        _plan: &mut SearchPlan,
        _caller: &CallerIdentity,
    ) -> anyhow::Result<PlanRewriteVerdict> {
        Ok(PlanRewriteVerdict::Proceed)
    }

    fn rewrite_probe_context(&self, ctx: &mut ProbeContext) -> anyhow::Result<PlanRewriteVerdict> {
        if self.host_labels.is_empty() {
            return Ok(PlanRewriteVerdict::Proceed);
        }
        let mut modified = false;
        for (k, v) in &self.host_labels {
            let param_key = format!("host_label:{k}");
            if ctx.params.get(&param_key) != Some(v) {
                ctx.params.insert(param_key, v.clone());
                modified = true;
            }
        }
        if modified {
            Ok(PlanRewriteVerdict::Modified)
        } else {
            Ok(PlanRewriteVerdict::Proceed)
        }
    }
}
