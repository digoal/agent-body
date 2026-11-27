//! `KernelDispatcher` 全链路 Hook 执行流水线与 CLI 参数解析辅助

use std::sync::Arc;

use agent_body_sdk::{
    derive_aopcc_mode, AccessOp, AccessVerdict, AopccMode, AuditEnvelope, AuthVerdict,
    AutonomyLevel, CallerIdentity, ClientAuthContext, FieldPathContext, FunctionCallInfo,
    GuardVerdict, KernelServices, ObjectAccessContext, ProcRoutine, ProcVolatility,
    RedactDirection, RedactVerdict, TableAmRoutine,
};
use anyhow::Context;
use chrono::Utc;
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{DispatchError, DispatchSuccess};
use crate::hook_chain::HookChain;
use crate::wait_events::{KernelWaitEventId, KernelWaitRegistry};

pub(crate) fn execute_proc_pipeline(
    routine: &dyn ProcRoutine,
    args: serde_json::Value,
    auth_ctx: &ClientAuthContext,
    hooks: &HookChain,
    table_am: &Arc<dyn TableAmRoutine>,
    waits: &KernelWaitRegistry,
) -> Result<DispatchSuccess, DispatchError> {
    let spec = routine.spec();
    let trace_id = Uuid::new_v4().to_string();

    let caller = run_auth_gate(auth_ctx, hooks, waits)?;
    let op = if spec.autonomy_level == AutonomyLevel::L0Observe
        && spec.volatility != ProcVolatility::Volatile
    {
        AccessOp::Read
    } else {
        AccessOp::Exec
    };
    if let Err(err) = run_object_access_gate(
        &caller,
        op,
        "proc",
        Some(spec.proc_name),
        hooks,
        waits,
    ) {
        emit_kernel_audit(hooks, waits, &caller, "proc_exec", spec.proc_name, "access_denied", None, "");
        return Err(err);
    }

    let aopcc_mode = derive_aopcc_mode(spec.volatility, spec.autonomy_level);
    if aopcc_mode == AopccMode::Denied {
        let reason = format!(
            "proc '{}' blocked by AOPCC ({:?}, {:?})",
            spec.proc_name, spec.volatility, spec.autonomy_level
        );
        emit_kernel_audit(hooks, waits, &caller, "proc_exec", spec.proc_name, "aopcc_denied", None, "");
        return Err(DispatchError::Forbidden { reason });
    }

    {
        let _guard = waits.enter(KernelWaitEventId::HookDispatch);
        match hooks.run_executor_start(&spec, &args, &caller) {
            GuardVerdict::Proceed => {}
            GuardVerdict::Block { reason } => {
                emit_kernel_audit(hooks, waits, &caller, "proc_exec", spec.proc_name, "guard_blocked", None, "");
                return Err(DispatchError::Forbidden { reason });
            }
            GuardVerdict::RequireConfirm { prompt } => {
                emit_kernel_audit(hooks, waits, &caller, "proc_exec", spec.proc_name, "require_confirm", None, "");
                return Err(DispatchError::RequireConfirm { prompt });
            }
        }
    }

    let fcinfo = FunctionCallInfo {
        proc_name: spec.proc_name.to_string(),
        args,
        caller: caller.clone(),
        trace_id: trace_id.clone(),
    };
    let services = KernelServices::new(caller.clone(), trace_id.clone(), Arc::clone(table_am));

    match routine.invoke(fcinfo, &services) {
        Ok(mut output) => {
            let path_ctx = FieldPathContext {
                entity_type: format!("proc:{}", spec.proc_name),
                field_path: "result".to_string(),
                direction: RedactDirection::Outbound,
            };
            let redact_verdict = {
                let _guard = waits.enter(KernelWaitEventId::HookDispatch);
                hooks.run_tuple_redact(&mut output, &path_ctx, &caller)
            };
            if let RedactVerdict::Blocked { reason } = redact_verdict {
                emit_kernel_audit(hooks, waits, &caller, "proc_exec", spec.proc_name, "redact_blocked", None, "");
                return Err(DispatchError::Forbidden { reason });
            }
            let digest = sha256_json(&output);
            emit_kernel_audit(hooks, waits, &caller, "proc_exec", spec.proc_name, "ok", None, &digest);
            Ok(DispatchSuccess {
                proc_name: spec.proc_name.to_string(),
                trace_id,
                aopcc_mode,
                data: output,
            })
        }
        Err(err) => {
            let err_msg = err.to_string();
            let (recovery, hook_errors) = {
                let _guard = waits.enter(KernelWaitEventId::HookDispatch);
                hooks.run_executor_end(&spec, &err_msg, &services)
            };
            emit_kernel_audit(
                hooks,
                waits,
                &caller,
                "proc_exec",
                spec.proc_name,
                "error",
                Some(serde_json::json!({"error": err_msg, "recovery": recovery})),
                "",
            );
            Err(DispatchError::ExecutionFailed {
                message: err_msg,
                recovery: recovery.map(Box::new),
                hook_errors,
            })
        }
    }
}

pub(crate) fn run_auth_gate(
    auth_ctx: &ClientAuthContext,
    hooks: &HookChain,
    waits: &KernelWaitRegistry,
) -> Result<CallerIdentity, DispatchError> {
    let _guard = waits.enter(KernelWaitEventId::HookDispatch);
    match hooks.run_client_auth(auth_ctx) {
        AuthVerdict::Allow(identity) => Ok(identity),
        AuthVerdict::Deny { status_code, reason } => {
            Err(DispatchError::AuthDenied { status_code, reason })
        }
        AuthVerdict::Next => Err(DispatchError::AuthDenied {
            status_code: 401,
            reason: "no ClientAuthHook allowed request".to_string(),
        }),
    }
}

pub(crate) fn run_object_access_gate(
    caller: &CallerIdentity,
    op: AccessOp,
    entity_type: &str,
    proc_name: Option<&str>,
    hooks: &HookChain,
    waits: &KernelWaitRegistry,
) -> Result<(), DispatchError> {
    let _guard = waits.enter(KernelWaitEventId::HookDispatch);
    let ctx = ObjectAccessContext {
        caller: caller.clone(),
        op,
        entity_type: entity_type.to_string(),
        proc_name: proc_name.map(str::to_string),
    };
    match hooks.run_object_access(&ctx) {
        AccessVerdict::Allow | AccessVerdict::AllowWithRls(_) => Ok(()),
        AccessVerdict::Deny { reason } => Err(DispatchError::Forbidden { reason }),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_kernel_audit(
    hooks: &HookChain,
    waits: &KernelWaitRegistry,
    caller: &CallerIdentity,
    operation: &str,
    target: &str,
    outcome: &str,
    delta: Option<serde_json::Value>,
    sha256: &str,
) {
    let _guard = waits.enter(KernelWaitEventId::AuditEmit);
    let _errs = hooks.run_emit_audit(&AuditEnvelope {
        trace_id: Uuid::new_v4().to_string(),
        caller: caller.clone(),
        domain: "IN_AGENT".to_string(),
        operation: operation.to_string(),
        target: target.to_string(),
        outcome: outcome.to_string(),
        before_after_delta: delta,
        evidence_sha256: sha256.to_string(),
        recorded_at: Utc::now().to_rfc3339(),
    });
}

pub(crate) fn parse_cli_tail_args(tail: &[String]) -> anyhow::Result<serde_json::Value> {
    if tail.len() == 1 && tail[0].trim_start().starts_with('{') {
        return serde_json::from_str(&tail[0]).context("解析 CLI JSON 参数失败");
    }
    let mut map = serde_json::Map::new();
    let mut i = 0usize;
    while i < tail.len() {
        let tok = &tail[i];
        if let Some(flag) = tok.strip_prefix("--") {
            if let Some((k, v)) = flag.split_once('=') {
                map.insert(k.replace('-', "_"), parse_scalar_json(v));
                i += 1;
            } else if i + 1 < tail.len() && !tail[i + 1].starts_with("--") {
                map.insert(flag.replace('-', "_"), parse_scalar_json(&tail[i + 1]));
                i += 2;
            } else {
                map.insert(flag.replace('-', "_"), serde_json::Value::Bool(true));
                i += 1;
            }
        } else {
            anyhow::bail!("未识别的 CLI 参数 '{tok}'，请使用 --key value 或 JSON 对象");
        }
    }
    Ok(serde_json::Value::Object(map))
}

fn parse_scalar_json(raw: &str) -> serde_json::Value {
    if raw == "true" {
        serde_json::Value::Bool(true)
    } else if raw == "false" {
        serde_json::Value::Bool(false)
    } else if let Ok(n) = raw.parse::<i64>() {
        serde_json::Value::Number(n.into())
    } else {
        serde_json::Value::String(raw.to_string())
    }
}

pub(crate) fn query_catalog_extensions(
    catalog_conn: Option<&std::sync::Mutex<Connection>>,
) -> Vec<serde_json::Value> {
    let Some(mutex) = catalog_conn else {
        return Vec::new();
    };
    let Ok(conn) = mutex.lock() else {
        return Vec::new();
    };
    let Ok(mut stmt) = conn.prepare(
        "SELECT ext_id, version, kind, state, table_prefix FROM body_cat_extension ORDER BY ext_id ASC",
    ) else {
        return Vec::new();
    };
    let Ok(rows) = stmt.query_map([], |r| {
        Ok(serde_json::json!({
            "ext_id": r.get::<_, String>(0)?,
            "version": r.get::<_, String>(1)?,
            "kind": r.get::<_, String>(2)?,
            "state": r.get::<_, String>(3)?,
            "table_prefix": r.get::<_, String>(4)?,
        }))
    }) else {
        return Vec::new();
    };
    rows.filter_map(Result::ok).collect()
}

fn sha256_json(val: &serde_json::Value) -> String {
    let serialized = serde_json::to_string(val).unwrap_or_default();
    let digest = Sha256::digest(serialized.as_bytes());
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}
