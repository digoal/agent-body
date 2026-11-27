//! 7 大生命周期 Hook 优先级调度链与默认回环/令牌认证钩子

use std::sync::Arc;

use agent_body_sdk::{
    AccessVerdict, AuditEnvelope, AuthVerdict, CallerIdentity, ClientAuthContext, ClientAuthHook,
    EmitAuditHook, ExecutorEndHook, ExecutorStartHook, FailPolicy, FieldPathContext, GuardVerdict,
    HookRegistration, KernelServices, ObjectAccessContext, ObjectAccessHook, PlanRewriteVerdict,
    PlannerRewriteHook, ProbeContext, ProcSpec, RecoveryPrescription, RedactVerdict,
    RowFilterPredicate, SearchPlan, TupleRedactHook,
};

/// 7 大生命周期 Hook 调度链（每个插槽按 `priority` 升序排列，数值越小越先执行）
#[derive(Default, Clone)]
pub struct HookChain {
    client_auth: Vec<Arc<dyn ClientAuthHook>>,
    object_access: Vec<Arc<dyn ObjectAccessHook>>,
    planner_rewrite: Vec<Arc<dyn PlannerRewriteHook>>,
    executor_start: Vec<Arc<dyn ExecutorStartHook>>,
    executor_end: Vec<Arc<dyn ExecutorEndHook>>,
    tuple_redact: Vec<Arc<dyn TupleRedactHook>>,
    emit_audit: Vec<Arc<dyn EmitAuditHook>>,
}

impl HookChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, reg: HookRegistration) {
        match reg {
            HookRegistration::ClientAuth(h) => {
                self.client_auth.push(h);
                self.client_auth.sort_by_key(|item| item.priority());
            }
            HookRegistration::ObjectAccess(h) => {
                self.object_access.push(h);
                self.object_access.sort_by_key(|item| item.priority());
            }
            HookRegistration::PlannerRewrite(h) => {
                self.planner_rewrite.push(h);
                self.planner_rewrite.sort_by_key(|item| item.priority());
            }
            HookRegistration::ExecutorStart(h) => {
                self.executor_start.push(h);
                self.executor_start.sort_by_key(|item| item.priority());
            }
            HookRegistration::ExecutorEnd(h) => {
                self.executor_end.push(h);
                self.executor_end.sort_by_key(|item| item.priority());
            }
            HookRegistration::TupleRedact(h) => {
                self.tuple_redact.push(h);
                self.tuple_redact.sort_by_key(|item| item.priority());
            }
            HookRegistration::EmitAudit(h) => {
                self.emit_audit.push(h);
                self.emit_audit.sort_by_key(|item| item.priority());
            }
        }
    }

    /// Hook 1: ClientAuthHook（严格 FailClosed：任意异常或无一放行即拒绝）
    pub fn run_client_auth(&self, ctx: &ClientAuthContext) -> AuthVerdict {
        if self.client_auth.is_empty() {
            return AuthVerdict::Deny {
                status_code: 401,
                reason: "no ClientAuthHook registered (fail_closed)".to_string(),
            };
        }
        for hook in &self.client_auth {
            match hook.authenticate(ctx) {
                Ok(AuthVerdict::Allow(identity)) => return AuthVerdict::Allow(identity),
                Ok(AuthVerdict::Deny { status_code, reason }) => {
                    return AuthVerdict::Deny { status_code, reason };
                }
                Ok(AuthVerdict::Next) => continue,
                Err(err) => {
                    if hook.fail_policy() == FailPolicy::FailClosed {
                        return AuthVerdict::Deny {
                            status_code: 403,
                            reason: format!(
                                "ClientAuthHook '{}' failed under fail_closed: {err}",
                                hook.hook_id()
                            ),
                        };
                    }
                }
            }
        }
        AuthVerdict::Deny {
            status_code: 401,
            reason: "no ClientAuthHook allowed request (fail_closed)".to_string(),
        }
    }

    /// Hook 2: ObjectAccessHook（严格 FailClosed，合并多条 RLS 行级谓词）
    pub fn run_object_access(&self, ctx: &ObjectAccessContext) -> AccessVerdict {
        let mut merged_rls: Option<RowFilterPredicate> = None;
        for hook in &self.object_access {
            match hook.authorize(ctx) {
                Ok(AccessVerdict::Allow) => {}
                Ok(AccessVerdict::AllowWithRls(pred)) => {
                    merged_rls = Some(match merged_rls {
                        None => pred,
                        Some(existing) => intersect_rls(existing, pred),
                    });
                }
                Ok(AccessVerdict::Deny { reason }) => return AccessVerdict::Deny { reason },
                Err(err) => {
                    if hook.fail_policy() == FailPolicy::FailClosed {
                        return AccessVerdict::Deny {
                            reason: format!(
                                "ObjectAccessHook '{}' failed under fail_closed: {err}",
                                hook.hook_id()
                            ),
                        };
                    }
                }
            }
        }
        match merged_rls {
            Some(pred) => AccessVerdict::AllowWithRls(pred),
            None => AccessVerdict::Allow,
        }
    }

    /// Hook 3: PlannerRewriteHook（严格 FailClosed）
    pub fn run_planner_rewrite(
        &self,
        plan: &mut SearchPlan,
        caller: &CallerIdentity,
    ) -> PlanRewriteVerdict {
        let mut modified = false;
        for hook in &self.planner_rewrite {
            match hook.rewrite_search_plan(plan, caller) {
                Ok(PlanRewriteVerdict::Proceed) => {}
                Ok(PlanRewriteVerdict::Modified) => modified = true,
                Ok(PlanRewriteVerdict::Reject { reason }) => {
                    return PlanRewriteVerdict::Reject { reason };
                }
                Err(err) => {
                    if hook.fail_policy() == FailPolicy::FailClosed {
                        return PlanRewriteVerdict::Reject {
                            reason: format!(
                                "PlannerRewriteHook '{}' failed under fail_closed: {err}",
                                hook.hook_id()
                            ),
                        };
                    }
                }
            }
        }
        if modified {
            PlanRewriteVerdict::Modified
        } else {
            PlanRewriteVerdict::Proceed
        }
    }

    /// Hook 3b: PlannerRewriteHook 对外部探针上下文改写（严格 FailClosed）
    pub fn run_planner_rewrite_probe(&self, ctx: &mut ProbeContext) -> PlanRewriteVerdict {
        let mut modified = false;
        for hook in &self.planner_rewrite {
            match hook.rewrite_probe_context(ctx) {
                Ok(PlanRewriteVerdict::Proceed) => {}
                Ok(PlanRewriteVerdict::Modified) => modified = true,
                Ok(PlanRewriteVerdict::Reject { reason }) => {
                    return PlanRewriteVerdict::Reject { reason };
                }
                Err(err) => {
                    if hook.fail_policy() == FailPolicy::FailClosed {
                        return PlanRewriteVerdict::Reject {
                            reason: format!(
                                "PlannerRewriteHook '{}' failed under fail_closed: {err}",
                                hook.hook_id()
                            ),
                        };
                    }
                }
            }
        }
        if modified {
            PlanRewriteVerdict::Modified
        } else {
            PlanRewriteVerdict::Proceed
        }
    }

    /// Hook 4: ExecutorStartHook（严格 FailClosed）
    pub fn run_executor_start(
        &self,
        spec: &ProcSpec,
        args: &serde_json::Value,
        caller: &CallerIdentity,
    ) -> GuardVerdict {
        for hook in &self.executor_start {
            match hook.pre_execute(spec, args, caller) {
                Ok(GuardVerdict::Proceed) => {}
                Ok(verdict) => return verdict,
                Err(err) => {
                    if hook.fail_policy() == FailPolicy::FailClosed {
                        return GuardVerdict::Block {
                            reason: format!(
                                "ExecutorStartHook '{}' failed under fail_closed: {err}",
                                hook.hook_id()
                            ),
                        };
                    }
                }
            }
        }
        GuardVerdict::Proceed
    }

    /// Hook 5: ExecutorEndHook（FailOpen：异常只记诊断不掩盖主错）
    pub fn run_executor_end(
        &self,
        spec: &ProcSpec,
        err_msg: &str,
        ctx: &KernelServices,
    ) -> (Option<RecoveryPrescription>, Vec<String>) {
        let mut first_rx: Option<RecoveryPrescription> = None;
        let mut hook_errors: Vec<String> = Vec::new();
        for hook in &self.executor_end {
            match hook.post_execute_error(spec, err_msg, ctx) {
                Ok(Some(rx)) => {
                    if first_rx.is_none() {
                        first_rx = Some(rx);
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    hook_errors.push(format!("ExecutorEndHook '{}': {err}", hook.hook_id()));
                }
            }
        }
        (first_rx, hook_errors)
    }

    /// Hook 6: TupleRedactHook（严格 FailClosed：异常即阻断输出防泄密）
    pub fn run_tuple_redact(
        &self,
        value: &mut serde_json::Value,
        path_ctx: &FieldPathContext,
        caller: &CallerIdentity,
    ) -> RedactVerdict {
        let mut any_redacted = false;
        for hook in &self.tuple_redact {
            match hook.redact(value, path_ctx, caller) {
                Ok(RedactVerdict::Unchanged) => {}
                Ok(RedactVerdict::Redacted) => any_redacted = true,
                Ok(RedactVerdict::Blocked { reason }) => {
                    return RedactVerdict::Blocked { reason };
                }
                Err(err) => {
                    if hook.fail_policy() == FailPolicy::FailClosed {
                        return RedactVerdict::Blocked {
                            reason: format!(
                                "TupleRedactHook '{}' failed under fail_closed: {err}",
                                hook.hook_id()
                            ),
                        };
                    }
                }
            }
        }
        if any_redacted {
            RedactVerdict::Redacted
        } else {
            RedactVerdict::Unchanged
        }
    }

    /// Hook 7: EmitAuditHook（FailOpen：异步降级记录异常不阻塞主业务）
    pub fn run_emit_audit(&self, envelope: &AuditEnvelope) -> Vec<String> {
        let mut errs = Vec::new();
        for hook in &self.emit_audit {
            if let Err(err) = hook.emit(envelope) {
                errs.push(format!("EmitAuditHook '{}': {err}", hook.hook_id()));
            }
        }
        errs
    }
}

pub(crate) fn intersect_rls(
    mut base: RowFilterPredicate,
    extra: RowFilterPredicate,
) -> RowFilterPredicate {
    if base.tenant_id.is_none() {
        base.tenant_id = extra.tenant_id;
    }
    if base.project_id.is_none() {
        base.project_id = extra.project_id;
    }
    if base.env.is_none() {
        base.env = extra.env;
    }
    if let Some(extra_hosts) = extra.allowed_host_ids {
        base.allowed_host_ids = Some(match base.allowed_host_ids {
            Some(existing) => existing
                .into_iter()
                .filter(|h| extra_hosts.contains(h))
                .collect(),
            None => extra_hosts,
        });
    }
    for (k, v) in extra.custom_labels {
        base.custom_labels.entry(k).or_insert(v);
    }
    base
}

/// 内核默认最小回环 / Bearer Token 认证钩子
#[derive(Debug, Clone)]
pub struct LoopbackOrTokenAuthHook {
    pub require_auth_for_reads: bool,
    pub expected_token: Option<String>,
}

impl LoopbackOrTokenAuthHook {
    pub fn new(require_auth_for_reads: bool, expected_token: Option<String>) -> Self {
        Self {
            require_auth_for_reads,
            expected_token,
        }
    }
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

impl ClientAuthHook for LoopbackOrTokenAuthHook {
    fn hook_id(&self) -> &'static str {
        "kernel:loopback_or_token_auth"
    }

    fn priority(&self) -> i32 {
        100
    }

    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailClosed
    }

    fn authenticate(&self, ctx: &ClientAuthContext) -> anyhow::Result<AuthVerdict> {
        if let (Some(expected), Some(provided)) = (&self.expected_token, &ctx.bearer_token) {
            if constant_time_eq(expected, provided) {
                return Ok(AuthVerdict::Allow(CallerIdentity {
                    caller_id: "bearer-token-client".to_string(),
                    tenant_id: None,
                    project_id: None,
                    env: None,
                    roles: vec!["authenticated".to_string()],
                    auth_method: "bearer_token".to_string(),
                    is_loopback: ctx.is_loopback,
                }));
            }
            return Ok(AuthVerdict::Deny {
                status_code: 403,
                reason: "invalid bearer token".to_string(),
            });
        }

        if ctx.is_loopback {
            if ctx.is_read_only && self.require_auth_for_reads {
                return Ok(AuthVerdict::Deny {
                    status_code: 401,
                    reason: "read endpoint requires authentication (require_auth_for_reads=true)"
                        .to_string(),
                });
            }
            if !ctx.is_read_only && self.expected_token.is_some() {
                return Ok(AuthVerdict::Deny {
                    status_code: 401,
                    reason: "write endpoint requires bearer token".to_string(),
                });
            }
            return Ok(AuthVerdict::Allow(CallerIdentity {
                caller_id: "loopback-local".to_string(),
                tenant_id: None,
                project_id: None,
                env: None,
                roles: vec!["local_loopback".to_string()],
                auth_method: "loopback".to_string(),
                is_loopback: true,
            }));
        }

        Ok(AuthVerdict::Deny {
            status_code: 401,
            reason: "non-loopback request requires valid authentication".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use agent_body_sdk::{AccessOp, RedactDirection, TransportKind};
    use anyhow::bail;

    struct OrderedRedactHook {
        id: &'static str,
        prio: i32,
        log: Arc<Mutex<Vec<&'static str>>>,
        fail: bool,
    }

    impl TupleRedactHook for OrderedRedactHook {
        fn hook_id(&self) -> &'static str {
            self.id
        }
        fn priority(&self) -> i32 {
            self.prio
        }
        fn redact(
            &self,
            _value: &mut serde_json::Value,
            _path_ctx: &FieldPathContext,
            _caller: &CallerIdentity,
        ) -> anyhow::Result<RedactVerdict> {
            self.log.lock().unwrap().push(self.id);
            if self.fail {
                bail!("redactor crashed");
            }
            Ok(RedactVerdict::Redacted)
        }
    }

    struct FailingAuditHook;
    impl EmitAuditHook for FailingAuditHook {
        fn hook_id(&self) -> &'static str {
            "audit:failing"
        }
        fn emit(&self, _envelope: &AuditEnvelope) -> anyhow::Result<()> {
            bail!("sink unavailable");
        }
    }

    fn auth_ctx(is_loopback: bool, is_read_only: bool, token: Option<&str>) -> ClientAuthContext {
        ClientAuthContext {
            transport: TransportKind::HttpRest,
            cert: None,
            bearer_token: token.map(str::to_string),
            method: if is_read_only { "GET" } else { "POST" }.to_string(),
            path: "/api/v1/facts".to_string(),
            is_loopback,
            is_read_only,
        }
    }

    #[test]
    fn hook_chain_sorts_by_priority_and_enforces_fail_closed_vs_fail_open() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let mut chain = HookChain::new();
        chain.register(HookRegistration::TupleRedact(Arc::new(OrderedRedactHook {
            id: "second",
            prio: 200,
            log: log.clone(),
            fail: true,
        })));
        chain.register(HookRegistration::TupleRedact(Arc::new(OrderedRedactHook {
            id: "first",
            prio: 50,
            log: log.clone(),
            fail: false,
        })));
        chain.register(HookRegistration::EmitAudit(Arc::new(FailingAuditHook)));

        let mut val = serde_json::json!({"secret": "abc"});
        let path_ctx = FieldPathContext {
            entity_type: "entity:process".to_string(),
            field_path: "args".to_string(),
            direction: RedactDirection::Outbound,
        };
        let verdict = chain.run_tuple_redact(&mut val, &path_ctx, &CallerIdentity::default());
        assert_eq!(*log.lock().unwrap(), vec!["first", "second"]);
        assert!(matches!(verdict, RedactVerdict::Blocked { .. }));

        // Empty ObjectAccess defaults to Allow; empty ClientAuth fails closed
        assert!(matches!(
            chain.run_client_auth(&auth_ctx(true, true, None)),
            AuthVerdict::Deny { status_code: 401, .. }
        ));
        assert_eq!(
            chain.run_object_access(&ObjectAccessContext {
                caller: CallerIdentity::default(),
                op: AccessOp::Read,
                entity_type: "entity:host".to_string(),
                proc_name: None,
            }),
            AccessVerdict::Allow
        );

        // EmitAuditHook is FailOpen: error is captured, does not abort
        let audit_errs = chain.run_emit_audit(&AuditEnvelope {
            trace_id: "t-1".to_string(),
            caller: CallerIdentity::default(),
            domain: "IN_AGENT".to_string(),
            operation: "read".to_string(),
            target: "entity:host".to_string(),
            outcome: "ok".to_string(),
            before_after_delta: None,
            evidence_sha256: "0".repeat(64),
            recorded_at: "2026-11-27T16:00:00+08:00".to_string(),
        });
        assert_eq!(audit_errs.len(), 1);
    }

    #[test]
    fn loopback_or_token_auth_respects_require_auth_for_reads() {
        let mut chain_relaxed = HookChain::new();
        chain_relaxed.register(HookRegistration::ClientAuth(Arc::new(
            LoopbackOrTokenAuthHook::new(false, Some("secret-tok".to_string())),
        )));
        assert!(matches!(
            chain_relaxed.run_client_auth(&auth_ctx(true, true, None)),
            AuthVerdict::Allow(_)
        ));
        assert!(matches!(
            chain_relaxed.run_client_auth(&auth_ctx(true, false, None)),
            AuthVerdict::Deny { status_code: 401, .. }
        ));

        let mut chain_strict = HookChain::new();
        chain_strict.register(HookRegistration::ClientAuth(Arc::new(
            LoopbackOrTokenAuthHook::new(true, Some("secret-tok".to_string())),
        )));
        assert!(matches!(
            chain_strict.run_client_auth(&auth_ctx(true, true, None)),
            AuthVerdict::Deny { status_code: 401, .. }
        ));
        assert!(matches!(
            chain_strict.run_client_auth(&auth_ctx(false, true, Some("secret-tok"))),
            AuthVerdict::Allow(_)
        ));
        assert!(matches!(
            chain_strict.run_client_auth(&auth_ctx(false, true, Some("wrong"))),
            AuthVerdict::Deny { status_code: 403, .. }
        ));
    }
}
