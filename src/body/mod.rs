//! Agent 本体中枢 (`src/body/` — 第 27 模块 · Phase 44)
//!
//! 对齐规约 #1, #2, #3, #8, #27：
//! - 脑体分离：LLM/客户端为瞬态大脑，本模块收敛宿主躯干的六大生理器官能力；
//! - `BodyManifest`：确定性 `body_id`、六器官自省与边界声明；
//! - `AutonomyEnvelope`：`L0_OBSERVE` / `L1_AUTONOMIC` / `L2_SUPERVISED` / `L3_FORBIDDEN` 四级自治包络；
//! - `ContinuityCapsule`：`<=300 tok` 脱敏抗投毒跨大脑接力胶囊 (`body_checkpoints`)；
//! - `HomeostasisVitals`：五维生命体征雷达与 `L1` 可逆稳态自愈反射。

pub mod continuity;
pub mod envelope;
pub mod manifest;
pub mod perf_awr;
pub mod vitals;

pub use continuity::{ContinuityInput, ContinuityManager};
pub use envelope::{AutonomyEnvelope, AutonomyLevel, EnvelopeDecision, EnvelopeRuleSpec};
pub use manifest::{compute_body_id, BodyManifest, BodyManifestEngine, OrganStatus};
pub use perf_awr::{
    AddmFinding, AwrDiffReport, AwrReportMeta, AwrSnapshot, AwrSnapshotMeta, BodyPerfEngine,
    BodyTimeModel, KernelSelfTelemetry, OperationStatRow, PerfTuneReport, WaitEventGuard,
    WaitEventId, WaitEventRegistry, WaitEventSnapshotRow,
};
pub use vitals::{HomeostasisHealReport, HomeostasisVitals, HomeostasisVitalsEngine, VitalDimension};
