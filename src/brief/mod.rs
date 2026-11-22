//! 情境简报模块 (`src/brief/`)
//!
//! 负责将宿主机事实图谱、工程偏好规约与记忆经验三大模块按优先级矩阵
//! 自适应组装为 Agent 开工情境简报。

pub mod assembler;
pub mod optimistic;

pub use assembler::{BriefAssembler, BriefOutput, BriefQuality, BriefRequest, estimate_tokens};
pub use optimistic::{OptimisticLease, OptimisticLeaseEngine, OptimisticLeaseReport};
