//! `contrib/hook_auth_mtls_rbac` — mTLS / Token 全量读写强制鉴权与多租户 RLS 隔离

use std::collections::BTreeMap;
use std::sync::Arc;

use agent_body_sdk::{
    body_module_magic, AccessVerdict, AuthVerdict, BodyExtension, CallerIdentity,
    ClientAuthContext, ClientAuthHook, ExtensionKind, ExtensionMeta, ExtensionRegistrar,
    FailPolicy, HookRegistration, MigrationScript, ObjectAccessContext, ObjectAccessHook,
    PlanRewriteVerdict, PlannerRewriteHook, ProbeContext, RowFilterPredicate, SearchPlan,
};
use serde::{Deserialize, Serialize};

const MIGRATION_SQL: &str = include_str!("../sql/hook_auth_mtls_rbac--0.1.0.sql");

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrincipalPolicy {
    pub caller_id: String,
    pub tenant_id: Option<String>,
    pub project_id: Option<String>,
    pub env: Option<String>,
    pub roles: Vec<String>,
    pub allowed_entity_types: Vec<String>,
}

impl PrincipalPolicy {
    pub fn to_caller_identity(&self, auth_method: &str, is_loopback: bool) -> CallerIdentity {
        CallerIdentity {
            caller_id: self.caller_id.clone(),
            tenant_id: self.tenant_id.clone(),
            project_id: self.project_id.clone(),
            env: self.env.clone(),
            roles: self.roles.clone(),
            auth_method: auth_method.to_string(),
            is_loopback,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MtlsRbacConfig {
    pub tokens: BTreeMap<String, PrincipalPolicy>,
    pub cert_cn_policies: BTreeMap<String, PrincipalPolicy>,
}

pub struct HookAuthMtlsRbacExtension {
    config: Arc<MtlsRbacConfig>,
}

impl Default for HookAuthMtlsRbacExtension {
    fn default() -> Self {
        Self::new(MtlsRbacConfig::default())
    }
}

impl HookAuthMtlsRbacExtension {
    pub fn new(config: MtlsRbacConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }
}

impl BodyExtension for HookAuthMtlsRbacExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "hook_auth_mtls_rbac".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: agent_body_sdk::SDK_API_VERSION.to_string(),
            table_prefix: "ext_hook_auth_mtls_rbac__".to_string(),
            requires: Vec::new(),
        }
    }

    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: MIGRATION_SQL.to_string(),
        });
        reg.register_hook(HookRegistration::ClientAuth(Arc::new(MtlsRbacAuthHook {
            config: Arc::clone(&self.config),
        })));
        reg.register_hook(HookRegistration::ObjectAccess(Arc::new(
            TenantRlsObjectHook {
                config: Arc::clone(&self.config),
            },
        )));
        reg.register_hook(HookRegistration::PlannerRewrite(Arc::new(
            TenantPlannerHook,
        )));
        Ok(())
    }
}

body_module_magic!();

/// 提取 X.509 Subject 中的 `CN=` 值
pub fn extract_cert_cn(subject: &str) -> Option<String> {
    for part in subject.split(',') {
        let trimmed = part.trim();
        if let Some(cn) = trimmed
            .strip_prefix("CN=")
            .or_else(|| trimmed.strip_prefix("cn="))
        {
            let val = cn.trim();
            if !val.is_empty() {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in ab.iter().zip(bb.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

pub struct MtlsRbacAuthHook {
    pub config: Arc<MtlsRbacConfig>,
}

impl ClientAuthHook for MtlsRbacAuthHook {
    fn hook_id(&self) -> &'static str {
        "ext:hook_auth_mtls_rbac:client_auth"
    }

    fn priority(&self) -> i32 {
        10
    }

    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailClosed
    }

    fn authenticate(&self, ctx: &ClientAuthContext) -> anyhow::Result<AuthVerdict> {
        if let Some(ref cert) = ctx.cert {
            if let Some(cn) = extract_cert_cn(&cert.subject) {
                if let Some(policy) = self.config.cert_cn_policies.get(&cn) {
                    return Ok(AuthVerdict::Allow(
                        policy.to_caller_identity("mtls_cert", ctx.is_loopback),
                    ));
                }
                return Ok(AuthVerdict::Deny {
                    status_code: 403,
                    reason: format!("mTLS certificate CN '{cn}' is not authorized"),
                });
            }
        }

        if let Some(ref token) = ctx.bearer_token {
            for (expected, policy) in &self.config.tokens {
                if constant_time_eq(expected, token) {
                    return Ok(AuthVerdict::Allow(
                        policy.to_caller_identity("rbac_token", ctx.is_loopback),
                    ));
                }
            }
            return Ok(AuthVerdict::Deny {
                status_code: 403,
                reason: "invalid RBAC bearer token".to_string(),
            });
        }

        Ok(AuthVerdict::Deny {
            status_code: 401,
            reason: "authentication required for all read and write endpoints".to_string(),
        })
    }
}

pub struct TenantRlsObjectHook {
    pub config: Arc<MtlsRbacConfig>,
}

impl ObjectAccessHook for TenantRlsObjectHook {
    fn hook_id(&self) -> &'static str {
        "ext:hook_auth_mtls_rbac:object_access"
    }

    fn priority(&self) -> i32 {
        10
    }

    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailClosed
    }

    fn authorize(&self, ctx: &ObjectAccessContext) -> anyhow::Result<AccessVerdict> {
        if ctx.caller.roles.iter().any(|r| r == "super_admin") {
            return Ok(AccessVerdict::Allow);
        }
        let policy = self
            .config
            .tokens
            .values()
            .chain(self.config.cert_cn_policies.values())
            .find(|p| p.caller_id == ctx.caller.caller_id);

        if let Some(pol) = policy {
            if !pol.allowed_entity_types.is_empty()
                && !pol
                    .allowed_entity_types
                    .iter()
                    .any(|t| t == &ctx.entity_type || t == "*")
            {
                return Ok(AccessVerdict::Deny {
                    reason: format!(
                        "caller '{}' lacks access to entity_type '{}'",
                        ctx.caller.caller_id, ctx.entity_type
                    ),
                });
            }
        }

        if ctx.caller.tenant_id.is_none() && ctx.caller.env.is_none() {
            return Ok(AccessVerdict::Deny {
                reason: "non-admin caller must carry tenant_id or env scope".to_string(),
            });
        }

        Ok(AccessVerdict::AllowWithRls(RowFilterPredicate {
            tenant_id: ctx.caller.tenant_id.clone(),
            project_id: ctx.caller.project_id.clone(),
            env: ctx.caller.env.clone(),
            ..Default::default()
        }))
    }
}

pub struct TenantPlannerHook;

impl PlannerRewriteHook for TenantPlannerHook {
    fn hook_id(&self) -> &'static str {
        "ext:hook_auth_mtls_rbac:planner_rewrite"
    }

    fn priority(&self) -> i32 {
        10
    }

    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailClosed
    }

    fn rewrite_search_plan(
        &self,
        plan: &mut SearchPlan,
        caller: &CallerIdentity,
    ) -> anyhow::Result<PlanRewriteVerdict> {
        if caller.roles.iter().any(|r| r == "super_admin") {
            return Ok(PlanRewriteVerdict::Proceed);
        }
        plan.rls_filter = Some(RowFilterPredicate {
            tenant_id: caller.tenant_id.clone(),
            project_id: caller.project_id.clone(),
            env: caller.env.clone(),
            ..Default::default()
        });
        Ok(PlanRewriteVerdict::Modified)
    }

    fn rewrite_probe_context(&self, ctx: &mut ProbeContext) -> anyhow::Result<PlanRewriteVerdict> {
        if ctx.caller.roles.iter().any(|r| r == "super_admin") {
            return Ok(PlanRewriteVerdict::Proceed);
        }
        ctx.rls_filter = Some(RowFilterPredicate {
            tenant_id: ctx.caller.tenant_id.clone(),
            project_id: ctx.caller.project_id.clone(),
            env: ctx.caller.env.clone(),
            ..Default::default()
        });
        Ok(PlanRewriteVerdict::Modified)
    }
}
