//! Agent Body Microkernel SDK (`agent-body-sdk`)
//!
//! 定义微内核与所有周边扩展模块之间的唯一稳定契约：
//! - 通用数据类型、证据元组与二维正交 AOPCC 矩阵 (`types`)
//! - 6 大开放 Routine Trait (`routines`)
//! - 7 大生命周期拦截 Hook 插槽 (`hooks`)
//! - 扩展元数据、注册器与 `BodyExtension` Trait (`extension`)

pub mod extension;
pub mod hooks;
pub mod observability;
pub mod routines;
pub mod types;

/// 编译期 SDK 主版本号指纹（对标 PostgreSQL `PG_CATALOG_VERSION` / `PG_MODULE_MAGIC`）
pub const SDK_API_VERSION: &str = "2.0";

/// 编译期模块魔数结构体，由 `body_module_magic!()` 宏在扩展 Crate 根导出
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BodyModuleMagic {
    pub sdk_api_version: &'static str,
    pub crate_name: &'static str,
    pub crate_version: &'static str,
}

/// 在每个扩展 Crate 根调用，注入编译期 SDK API 版本指纹（对标 `PG_MODULE_MAGIC`）
#[macro_export]
macro_rules! body_module_magic {
    () => {
        #[doc(hidden)]
        pub const BODY_MODULE_MAGIC: $crate::BodyModuleMagic = $crate::BodyModuleMagic {
            sdk_api_version: $crate::SDK_API_VERSION,
            crate_name: env!("CARGO_PKG_NAME"),
            crate_version: env!("CARGO_PKG_VERSION"),
        };

        #[doc(hidden)]
        pub fn body_module_magic() -> $crate::BodyModuleMagic {
            BODY_MODULE_MAGIC
        }
    };
}

pub use extension::{BodyExtension, ExtensionKind, ExtensionMeta, ExtensionRegistrar, HookRegistration, MigrationScript};
pub use hooks::{
    AccessOp, AccessVerdict, AuditEnvelope, AuthVerdict, ClientAuthContext, ClientAuthHook,
    ClientCertInfo, EmitAuditHook, ExecutorEndHook, ExecutorStartHook, FailPolicy,
    FieldPathContext, GuardVerdict, ObjectAccessContext, ObjectAccessHook, PlanRewriteVerdict,
    PlannerRewriteHook, RecoveryPrescription, RedactDirection, RedactVerdict, SearchPlan,
    TransportKind, TupleRedactHook,
};
pub use observability::{
    InFlightOpSnapshot, KernelObservabilitySnapshot, KernelOpStatsRow, KernelWaitSnapshotRow,
    ObservabilityProvider, SlowOpRecord, KERNEL_LATENCY_BUCKET_BOUNDS_US,
    KERNEL_LATENCY_BUCKET_LABELS,
};
pub use routines::{
    BgWorkerRoutine, CancellationToken, FdwRoutine, FdwScanState, FdwSourceHealth,
    FunctionCallInfo, IndexAmRoutine, IndexCostEstimate, IndexHit, IndexScanDesc, KernelServices,
    OpStrategy, OperatorRoutine, ProbeContext, ProcRoutine, ProcSpec, TableAmRoutine, TypeRoutine,
    WorkerTickOutcome, WorkerTrigger,
};
pub use types::{
    derive_aopcc_mode, AopccMode, AutonomyLevel, CallerIdentity, EntityScanFilter, EventQueryFilter,
    EvidenceTuple, KernelEdge, KernelEventRecord, KernelLease, KernelSubGraph, LeaseAcquireReq,
    LeaseMode, LeaseOutcome, ProcVolatility, RowFilterPredicate,
};
