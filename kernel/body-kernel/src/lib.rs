//! Agent Body 微内核核心 (`agent-body-kernel`)
//!
//! 纯微内核契约、存储、融合、派发与沙盒执行层（严禁混入任何周边业务策略）：
//! - `catalog`: 8 张 `body_cat_*` 系统目录表、依赖 DAG 拓扑排序与扩展生命周期状态机
//! - `hook_chain`: 7 大生命周期 Hook 优先级调度链与 FailClosed/FailOpen 兜底
//! - `wait_events`: 无锁原子耗时直方图与等待事件快照
//! - `scoped_sql`: 扩展私有物理表沙盒 SQL 解析与前缀隔离执行器
//! - `sqlite_am`: SQLite 5 张通用基表 + FTS5 + RLS 下推 `TableAmRoutine`
//! - `mem_am`: 纯内存无盘 `TableAmRoutine`
//! - `rrf_executor`: 多通道 `IndexAmRoutine` RRF 融合器（$k=60$、动态重归一化、RLS 与脱敏）
//! - `dispatcher`: 单点四端自动派发外壳（MCP / CLI / HTTP / Parity）
//! - `engine`: 统一微内核运行时 (`KernelRuntime`)
//! - `oop_sandbox`: 轨 B 进程外沙盒适配器 (`OutOfProcessFdwAdapter`)

pub mod catalog;
pub mod dispatcher;
pub mod engine;
mod engine_fdw;
pub mod hook_chain;
pub mod mem_am;
pub mod oop_sandbox;
pub mod rrf_executor;
pub mod scoped_sql;
pub mod sqlite_am;
pub mod wait_events;

pub use catalog::{
    init_catalog_schema, topo_sort_extensions, CatalogStore, ExtensionRecord, ExtensionState,
};
pub use dispatcher::{
    DispatchError, DispatchSuccess, HttpRequest, HttpResponse, KernelDispatcher, ParityEntry,
};
pub use engine::KernelRuntime;
pub use hook_chain::{HookChain, LoopbackOrTokenAuthHook};
pub use mem_am::MemoryTableAm;
pub use oop_sandbox::{sanitize_prompt_injection, OutOfProcessFdwAdapter};
pub use rrf_executor::{
    FusedHit, RrfMergeExecutor, RrfSearchResult, SearchQuery, DEFAULT_RRF_K,
};
pub use scoped_sql::ScopedSqlExecutor;
pub use sqlite_am::{SqliteCoreTableAm, BASE_TABLES_SQL};
pub use wait_events::{
    KernelWaitEventId, KernelWaitGuard, KernelWaitRegistry, KernelWaitSnapshotRow,
    KERNEL_LATENCY_BUCKET_BOUNDS_US, KERNEL_LATENCY_BUCKET_LABELS, KERNEL_WAIT_EVENT_COUNT,
};
