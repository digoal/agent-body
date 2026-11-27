//! 微内核 7 大生命周期拦截 Hook Trait 与异常兜底策略

use serde::{Deserialize, Serialize};

use crate::routines::{KernelServices, ProbeContext, ProcSpec};
use crate::types::{CallerIdentity, RowFilterPredicate};

/// Hook 异常兜底策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailPolicy {
    FailClosed,
    FailOpen,
}

impl FailPolicy {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::FailClosed => "fail_closed",
            Self::FailOpen => "fail_open",
        }
    }
}

/// 请求接入传输通道类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    StdioMcp,
    Cli,
    HttpRest,
    UnixSocket,
    Internal,
}

/// 客户端证书摘要信息
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientCertInfo {
    pub subject: String,
    pub issuer: String,
    pub fingerprint_sha256: String,
}

/// 1. ClientAuthHook 认证上下文（覆盖全量读/写端点）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientAuthContext {
    pub transport: TransportKind,
    pub cert: Option<ClientCertInfo>,
    pub bearer_token: Option<String>,
    pub method: String,
    pub path: String,
    pub is_loopback: bool,
    pub is_read_only: bool,
}

/// ClientAuthHook 裁决结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthVerdict {
    Allow(CallerIdentity),
    Next,
    Deny { status_code: u16, reason: String },
}

/// Hook 1: 客户端身份认证插槽（FailClosed）
pub trait ClientAuthHook: Send + Sync {
    fn hook_id(&self) -> &'static str;
    fn priority(&self) -> i32 {
        100
    }
    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailClosed
    }
    fn authenticate(&self, ctx: &ClientAuthContext) -> anyhow::Result<AuthVerdict>;
}

/// 对象访问操作类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessOp {
    Read,
    Write,
    Exec,
}

/// 2. ObjectAccessHook 授权上下文
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectAccessContext {
    pub caller: CallerIdentity,
    pub op: AccessOp,
    pub entity_type: String,
    pub proc_name: Option<String>,
}

/// ObjectAccessHook 裁决结果（支持签发 RLS 行级过滤谓词）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessVerdict {
    Allow,
    AllowWithRls(RowFilterPredicate),
    Deny { reason: String },
}

/// Hook 2: 对象访问控制与行级隔离插槽（FailClosed）
pub trait ObjectAccessHook: Send + Sync {
    fn hook_id(&self) -> &'static str;
    fn priority(&self) -> i32 {
        100
    }
    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailClosed
    }
    fn authorize(&self, ctx: &ObjectAccessContext) -> anyhow::Result<AccessVerdict>;
}

/// 3. PlannerRewriteHook 可改写的检索执行计划
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchPlan {
    pub query_text: String,
    pub channels: Vec<String>,
    pub disabled_channels: Vec<String>,
    pub rls_filter: Option<RowFilterPredicate>,
    pub timeout_ms: u64,
    pub limit: usize,
}

/// PlannerRewriteHook 裁决结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanRewriteVerdict {
    Proceed,
    Modified,
    Reject { reason: String },
}

/// Hook 3: 执行计划与探针上下文改写插槽（FailClosed）
pub trait PlannerRewriteHook: Send + Sync {
    fn hook_id(&self) -> &'static str;
    fn priority(&self) -> i32 {
        100
    }
    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailClosed
    }
    fn rewrite_search_plan(
        &self,
        plan: &mut SearchPlan,
        caller: &CallerIdentity,
    ) -> anyhow::Result<PlanRewriteVerdict>;

    fn rewrite_probe_context(
        &self,
        _ctx: &mut ProbeContext,
    ) -> anyhow::Result<PlanRewriteVerdict> {
        Ok(PlanRewriteVerdict::Proceed)
    }
}

/// 4. ExecutorStartHook 裁决结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardVerdict {
    Proceed,
    Block { reason: String },
    RequireConfirm { prompt: String },
}

/// Hook 4: 算子执行前自治门禁插槽（FailClosed）
pub trait ExecutorStartHook: Send + Sync {
    fn hook_id(&self) -> &'static str;
    fn priority(&self) -> i32 {
        100
    }
    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailClosed
    }
    fn pre_execute(
        &self,
        spec: &ProcSpec,
        args: &serde_json::Value,
        caller: &CallerIdentity,
    ) -> anyhow::Result<GuardVerdict>;
}

/// 5. ExecutorEndHook 自愈处方
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryPrescription {
    pub action: String,
    pub retry_suggested: bool,
    pub degraded_mode: Option<String>,
    pub detail: String,
}

/// Hook 5: 算子失败后恢复处方插槽（FailOpen）
pub trait ExecutorEndHook: Send + Sync {
    fn hook_id(&self) -> &'static str;
    fn priority(&self) -> i32 {
        100
    }
    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailOpen
    }
    fn post_execute_error(
        &self,
        spec: &ProcSpec,
        err_msg: &str,
        ctx: &KernelServices,
    ) -> anyhow::Result<Option<RecoveryPrescription>>;
}

/// 脱敏方向
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedactDirection {
    Inbound,
    Outbound,
}

/// 6. TupleRedactHook 字段路径感知上下文
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldPathContext {
    pub entity_type: String,
    pub field_path: String,
    pub direction: RedactDirection,
}

/// TupleRedactHook 裁决结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedactVerdict {
    Unchanged,
    Redacted,
    Blocked { reason: String },
}

/// Hook 6: 入库前与出站前元组脱敏插槽（FailClosed）
pub trait TupleRedactHook: Send + Sync {
    fn hook_id(&self) -> &'static str;
    fn priority(&self) -> i32 {
        100
    }
    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailClosed
    }
    fn redact(
        &self,
        value: &mut serde_json::Value,
        path_ctx: &FieldPathContext,
        caller: &CallerIdentity,
    ) -> anyhow::Result<RedactVerdict>;
}

/// 7. EmitAuditHook 标准化审计信封
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuditEnvelope {
    pub trace_id: String,
    pub caller: CallerIdentity,
    pub domain: String,
    pub operation: String,
    pub target: String,
    pub outcome: String,
    pub before_after_delta: Option<serde_json::Value>,
    pub evidence_sha256: String,
    pub recorded_at: String,
}

/// Hook 7: 读/写/后台变更统一异步审计扇出插槽（FailOpen）
pub trait EmitAuditHook: Send + Sync {
    fn hook_id(&self) -> &'static str;
    fn priority(&self) -> i32 {
        100
    }
    fn fail_policy(&self) -> FailPolicy {
        FailPolicy::FailOpen
    }
    fn emit(&self, envelope: &AuditEnvelope) -> anyhow::Result<()>;
}
