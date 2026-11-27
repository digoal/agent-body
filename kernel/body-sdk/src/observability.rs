//! 微内核纯机制只读观测快照契约 (`ObservabilityProvider`)
//!
//! 仅描述通用内核指标数据结构与只读访问抽象，不绑定任何外部监控协议或发行版实现。

use serde::{Deserialize, Serialize};

/// 8 档对数分位延迟桶边界（单位：微秒）
pub const KERNEL_LATENCY_BUCKET_BOUNDS_US: [u64; 8] = [
    100,
    500,
    2_000,
    10_000,
    50_000,
    250_000,
    1_000_000,
    u64::MAX,
];

/// 8 档延迟分桶展示标签
pub const KERNEL_LATENCY_BUCKET_LABELS: [&str; 8] = [
    "<100us",
    "100~500us",
    "0.5~2ms",
    "2~10ms",
    "10~50ms",
    "50~250ms",
    "250ms~1s",
    ">=1s",
];

/// 等待事件快照单行数据
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KernelWaitSnapshotRow {
    pub event_name: String,
    pub calls: u64,
    pub total_us: u64,
    pub avg_us: f64,
    pub max_us: u64,
    pub est_p95_us: u64,
    pub histogram_buckets: [u64; 8],
}

/// 算子/探针/Hook/协程执行聚合统计行（对标 PostgreSQL `pg_stat_statements`）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KernelOpStatsRow {
    pub kind: String,
    pub target: String,
    pub calls: u64,
    pub ok_count: u64,
    pub error_count: u64,
    pub denied_count: u64,
    pub timeout_count: u64,
    pub total_us: u64,
    pub max_us: u64,
    pub avg_us: f64,
}

/// 当前正在执行中的活跃操作快照（对标 PostgreSQL `pg_stat_activity`）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InFlightOpSnapshot {
    pub trace_id: String,
    pub kind: String,
    pub target: String,
    pub caller_id: String,
    pub started_at: String,
    pub elapsed_us: u64,
}

/// 慢操作 / 异常 / 鉴权拒绝 / 超时环形追踪记录（对标 `auto_explain`）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlowOpRecord {
    pub trace_id: String,
    pub kind: String,
    pub target: String,
    pub caller_id: String,
    pub outcome: String,
    pub elapsed_us: u64,
    pub detail: Option<String>,
    pub recorded_at: String,
}

/// 微内核全量可观测只读快照
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct KernelObservabilitySnapshot {
    pub captured_at: String,
    pub uptime_ms: u64,
    pub slow_threshold_us: u64,
    pub wait_events: Vec<KernelWaitSnapshotRow>,
    pub op_stats: Vec<KernelOpStatsRow>,
    pub in_flight: Vec<InFlightOpSnapshot>,
    pub slow_ops: Vec<SlowOpRecord>,
}

/// 供 `contrib/` 算子通过 `KernelServices` 只读读取内核可观测态的受控 Trait
pub trait ObservabilityProvider: Send + Sync {
    fn snapshot(&self) -> KernelObservabilitySnapshot;
}
