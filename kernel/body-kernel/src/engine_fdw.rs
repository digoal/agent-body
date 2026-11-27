//! 微内核通用 FDW 外部探针托管执行流水线（全链路鉴权 / RLS / 重写 / 脱敏 / 审计）

use std::collections::BTreeMap;
use std::sync::Arc;

use agent_body_sdk::{
    AccessOp, AccessVerdict, AuditEnvelope, AuthVerdict, CallerIdentity, ClientAuthContext,
    EvidenceTuple, FdwRoutine, FdwSourceHealth, FieldPathContext, ObjectAccessContext,
    PlanRewriteVerdict, ProbeContext, RedactDirection, RedactVerdict,
};
use anyhow::{anyhow, bail};
use chrono::Utc;
use uuid::Uuid;

use crate::hook_chain::{intersect_rls, HookChain};
use crate::wait_events::{KernelWaitEventId, KernelWaitRegistry};

/// 校验 FDW 健康度与运行时前提
pub(crate) fn explain_fdw_inner(
    fdws: &BTreeMap<String, Arc<dyn FdwRoutine>>,
    fdw_id: &str,
    ctx: &ProbeContext,
    waits: &KernelWaitRegistry,
) -> anyhow::Result<FdwSourceHealth> {
    let fdw = fdws
        .get(fdw_id)
        .ok_or_else(|| anyhow!("未找到已注册的 FDW '{fdw_id}'"))?;
    let _guard = waits.enter(KernelWaitEventId::FdwProbe);
    Ok(fdw.explain_source(ctx))
}

/// 托管执行 FDW 扫描：`ClientAuth` -> `ObjectAccess(RLS)` -> `PlannerRewrite` -> `FdwScan` -> `TupleRedact` -> `EmitAudit`
pub(crate) fn scan_fdw_inner(
    fdws: &BTreeMap<String, Arc<dyn FdwRoutine>>,
    hooks: &HookChain,
    waits: &KernelWaitRegistry,
    fdw_id: &str,
    target: Option<&str>,
    ctx: &mut ProbeContext,
    auth_ctx: &ClientAuthContext,
) -> anyhow::Result<Vec<EvidenceTuple>> {
    let trace_id = Uuid::new_v4().to_string();
    let fdw = fdws
        .get(fdw_id)
        .ok_or_else(|| anyhow!("未找到已注册的 FDW '{fdw_id}'"))?;

    let caller = match hooks.run_client_auth(auth_ctx) {
        AuthVerdict::Allow(id) => id,
        AuthVerdict::Next => {
            emit_fdw_audit(
                hooks,
                waits,
                &trace_id,
                &CallerIdentity::default(),
                fdw_id,
                "denied",
                serde_json::json!({"status": 401, "reason": "no auth hook allowed"}),
                "",
            );
            bail!("AUTH_DENIED (401): no auth hook allowed")
        }
        AuthVerdict::Deny { status_code, reason } => {
            emit_fdw_audit(
                hooks,
                waits,
                &trace_id,
                &CallerIdentity::default(),
                fdw_id,
                "denied",
                serde_json::json!({"status": status_code, "reason": reason}),
                "",
            );
            bail!("AUTH_DENIED ({status_code}): {reason}")
        }
    };
    ctx.caller = caller.clone();

    let primary_entity = fdw
        .supported_types()
        .first()
        .copied()
        .unwrap_or("entity:fdw");
    match hooks.run_object_access(&ObjectAccessContext {
        caller: caller.clone(),
        op: AccessOp::Read,
        entity_type: primary_entity.to_string(),
        proc_name: Some(fdw_id.to_string()),
    }) {
        AccessVerdict::Allow => {}
        AccessVerdict::AllowWithRls(pred) => {
            ctx.rls_filter = Some(match ctx.rls_filter.take() {
                None => pred,
                Some(existing) => intersect_rls(existing, pred),
            });
        }
        AccessVerdict::Deny { reason } => {
            emit_fdw_audit(
                hooks,
                waits,
                &trace_id,
                &caller,
                fdw_id,
                "denied",
                serde_json::json!({"reason": reason}),
                "",
            );
            bail!("ACCESS_DENIED: {reason}");
        }
    }

    if let PlanRewriteVerdict::Reject { reason } = hooks.run_planner_rewrite_probe(ctx) {
        emit_fdw_audit(
            hooks,
            waits,
            &trace_id,
            &caller,
            fdw_id,
            "rejected",
            serde_json::json!({"reason": reason}),
            "",
        );
        bail!("PLAN_REJECTED: {reason}");
    }

    let raw_tuples = {
        let _guard = waits.enter(KernelWaitEventId::FdwProbe);
        let mut scan_state = fdw.begin_scan(target, ctx)?;
        let mut collected = Vec::new();
        loop {
            let batch = scan_state.iterate_scan(128)?;
            if batch.is_empty() {
                break;
            }
            let batch_len = batch.len();
            collected.extend(batch);
            if batch_len < 128 {
                break;
            }
        }
        scan_state.end_scan()?;
        collected
    };

    let mut out = Vec::with_capacity(raw_tuples.len());
    for mut tuple in raw_tuples {
        for (k, v) in &ctx.params {
            if let Some(label_key) = k.strip_prefix("host_label:") {
                tuple
                    .host_labels
                    .entry(label_key.to_string())
                    .or_insert_with(|| v.clone());
            }
        }

        let path_ctx = FieldPathContext {
            entity_type: tuple.entity_type.clone(),
            field_path: "payload".to_string(),
            direction: RedactDirection::Outbound,
        };
        match hooks.run_tuple_redact(&mut tuple.payload, &path_ctx, &caller) {
            RedactVerdict::Unchanged => {
                if tuple.evidence_sha256.is_empty() {
                    tuple.evidence_sha256 = tuple.compute_sha256();
                }
            }
            RedactVerdict::Redacted => {
                tuple.redacted = true;
                tuple.evidence_sha256 = tuple.compute_sha256();
            }
            RedactVerdict::Blocked { reason } => {
                emit_fdw_audit(
                    hooks,
                    waits,
                    &trace_id,
                    &caller,
                    fdw_id,
                    "blocked",
                    serde_json::json!({"reason": reason, "entity_id": tuple.entity_id}),
                    "",
                );
                bail!("REDACT_BLOCKED: {reason}");
            }
        }

        if let Some(ref rls) = ctx.rls_filter {
            if !rls.matches_tuple(&tuple) {
                continue;
            }
        }
        out.push(tuple);
    }

    let first_sha = out
        .first()
        .map(|t| t.evidence_sha256.as_str())
        .unwrap_or("");
    emit_fdw_audit(
        hooks,
        waits,
        &trace_id,
        &caller,
        fdw_id,
        "ok",
        serde_json::json!({
            "target": target,
            "returned": out.len(),
        }),
        first_sha,
    );
    Ok(out)
}

#[allow(clippy::too_many_arguments)]
fn emit_fdw_audit(
    hooks: &HookChain,
    waits: &KernelWaitRegistry,
    trace_id: &str,
    caller: &CallerIdentity,
    fdw_id: &str,
    outcome: &str,
    delta: serde_json::Value,
    evidence_sha256: &str,
) {
    let _guard = waits.enter(KernelWaitEventId::AuditEmit);
    let _errs = hooks.run_emit_audit(&AuditEnvelope {
        trace_id: trace_id.to_string(),
        caller: caller.clone(),
        domain: "IN_AGENT".to_string(),
        operation: "fdw_scan".to_string(),
        target: fdw_id.to_string(),
        outcome: outcome.to_string(),
        before_after_delta: Some(delta),
        evidence_sha256: evidence_sha256.to_string(),
        recorded_at: Utc::now().to_rfc3339(),
    });
}
