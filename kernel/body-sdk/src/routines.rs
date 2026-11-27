//! 微内核 6 大开放 Routine Trait 及配套上下文化门面 (`KernelServices`)

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::observability::{KernelObservabilitySnapshot, ObservabilityProvider};
use crate::types::{
    AutonomyLevel, CallerIdentity, EntityScanFilter, EventQueryFilter, EvidenceTuple, KernelEdge,
    KernelEventRecord, KernelLease, KernelSubGraph, LeaseAcquireReq, LeaseOutcome, ProcVolatility,
    RowFilterPredicate,
};

/// 检索操作符策略号（1~5，对标 PostgreSQL Operator Strategy Number）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u16)]
pub enum OpStrategy {
    Exact = 1,
    Prefix = 2,
    Fuzzy = 3,
    Vector = 4,
    GraphHop = 5,
}

impl OpStrategy {
    pub const fn strategy_num(self) -> u16 {
        self as u16
    }

    pub fn from_u16(num: u16) -> Option<Self> {
        match num {
            1 => Some(Self::Exact),
            2 => Some(Self::Prefix),
            3 => Some(Self::Fuzzy),
            4 => Some(Self::Vector),
            5 => Some(Self::GraphHop),
            _ => None,
        }
    }
}

/// 1A. 开放实体/关系类型定义原语
pub trait TypeRoutine: Send + Sync {
    fn type_id(&self) -> &'static str;
    fn validate_in(&self, raw: &serde_json::Value) -> anyhow::Result<serde_json::Value>;
    fn render_compact_summary(&self, val: &serde_json::Value) -> String;
}

/// 1B. 操作符算子原语
pub trait OperatorRoutine: Send + Sync {
    fn op_id(&self) -> &'static str;
    fn strategy_num(&self) -> u16;
    fn evaluate(&self, lhs: &serde_json::Value, rhs: &serde_json::Value) -> anyhow::Result<bool>;
}

/// 2. 表存储访问方法原语 (对标 PostgreSQL `TableAmRoutine`)
pub trait TableAmRoutine: Send + Sync {
    fn am_id(&self) -> &'static str;
    fn init_catalog_and_base_tables(&self) -> anyhow::Result<()>;

    // 通用实体与关系图原语（支持 RLS 行级过滤谓词下推）
    fn entity_upsert(&self, tuple: &EvidenceTuple) -> anyhow::Result<()>;
    fn entity_get(
        &self,
        entity_type: &str,
        entity_id: &str,
        rls: Option<&RowFilterPredicate>,
    ) -> anyhow::Result<Option<EvidenceTuple>>;
    fn entity_scan(
        &self,
        filter: &EntityScanFilter,
        rls: Option<&RowFilterPredicate>,
    ) -> anyhow::Result<Vec<EvidenceTuple>>;
    fn entity_delete_by_source(&self, source_ext: &str, path_or_key: &str) -> anyhow::Result<usize>;

    fn edge_upsert(&self, edge: &KernelEdge) -> anyhow::Result<()>;
    fn edge_traverse(
        &self,
        root_id: &str,
        max_hops: usize,
        min_weight: f32,
        rls: Option<&RowFilterPredicate>,
    ) -> anyhow::Result<KernelSubGraph>;

    // Append-Only 事件/审计流原语
    fn event_append(&self, stream: &str, event: &KernelEventRecord) -> anyhow::Result<()>;
    fn event_query(
        &self,
        filter: &EventQueryFilter,
        rls: Option<&RowFilterPredicate>,
    ) -> anyhow::Result<Vec<KernelEventRecord>>;
    fn event_prune_before(&self, stream: &str, before_iso: &str) -> anyhow::Result<usize>;

    // 协作租约 CAS 原语
    fn lease_try_acquire(&self, req: &LeaseAcquireReq) -> anyhow::Result<LeaseOutcome>;
    fn lease_release(&self, lock_id: &str) -> anyhow::Result<bool>;
    fn lease_list(&self, resource_type: Option<&str>) -> anyhow::Result<Vec<KernelLease>>;
    fn lease_gc_expired(&self) -> anyhow::Result<usize>;

    // 胶囊/游标 KV 与扩展私有 DDL 沙盒执行
    fn kv_put(&self, namespace: &str, key: &str, val: &serde_json::Value) -> anyhow::Result<()>;
    fn kv_get(&self, namespace: &str, key: &str) -> anyhow::Result<Option<serde_json::Value>>;
    fn exec_scoped_migration(
        &self,
        ext_id: &str,
        table_prefix: &str,
        version: &str,
        sql: &str,
    ) -> anyhow::Result<()>;
}

/// 索引代价估算结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexCostEstimate {
    pub available: bool,
    pub est_latency_us: u64,
    pub est_selectivity: f64,
}

/// 索引扫描描述符
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexScanDesc {
    pub query_text: String,
    pub strategy_num: u16,
    pub scalar_filters: BTreeMap<String, String>,
    pub rls_filter: Option<RowFilterPredicate>,
    pub limit: usize,
}

/// 索引检索命中结果
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexHit {
    pub entity_id: String,
    pub entity_type: String,
    pub score: f64,
    pub channel: String,
    pub reasoning_path: Option<String>,
    pub tuple: Option<EvidenceTuple>,
}

/// 3. 索引访问方法原语 (对标 PostgreSQL `IndexAmRoutine`)
pub trait IndexAmRoutine: Send + Sync {
    fn am_id(&self) -> &'static str;
    fn default_weight(&self) -> f64;
    fn amcostestimate(&self, scan: &IndexScanDesc) -> IndexCostEstimate;
    fn amgetmulti(
        &self,
        scan: &IndexScanDesc,
        am: &dyn TableAmRoutine,
    ) -> anyhow::Result<Vec<IndexHit>>;
    fn aminsert(&self, entity: &EvidenceTuple) -> anyhow::Result<()>;
    fn amdelete_by_source(&self, source_key: &str) -> anyhow::Result<usize>;
    fn amvacuum(&self) -> anyhow::Result<usize>;
}

/// 外部数据探针执行上下文
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeContext {
    pub caller: CallerIdentity,
    pub target_host: Option<String>,
    pub timeout_ms: u64,
    pub rls_filter: Option<RowFilterPredicate>,
    pub params: BTreeMap<String, String>,
}

/// 外部数据源健康与权限预检结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FdwSourceHealth {
    pub healthy: bool,
    pub degraded_reason: Option<String>,
    pub missing_deps: Vec<String>,
}

/// 外部数据扫描会话状态
pub trait FdwScanState: Send {
    fn iterate_scan(&mut self, batch_limit: usize) -> anyhow::Result<Vec<EvidenceTuple>>;
    fn end_scan(&mut self) -> anyhow::Result<()>;
}

/// 4. 外部数据与探针包装器原语 (对标 PostgreSQL `FdwRoutine`)
pub trait FdwRoutine: Send + Sync {
    fn fdw_id(&self) -> &'static str;
    fn supported_types(&self) -> &'static [&'static str];
    fn explain_source(&self, ctx: &ProbeContext) -> FdwSourceHealth;
    fn begin_scan(
        &self,
        target: Option<&str>,
        ctx: &ProbeContext,
    ) -> anyhow::Result<Box<dyn FdwScanState>>;
    fn poll_drift(
        &self,
        cursor: Option<&str>,
        ctx: &ProbeContext,
    ) -> anyhow::Result<(Vec<EvidenceTuple>, Option<String>)>;
}

/// 算子静态规格声明（单点注册同时驱动 MCP/CLI/HTTP/Parity）
#[derive(Debug, Clone)]
pub struct ProcSpec {
    pub proc_name: &'static str,
    pub cli_path: &'static [&'static str],
    pub rest_route: (&'static str, &'static str),
    pub profiles: &'static [&'static str],
    pub volatility: ProcVolatility,
    pub autonomy_level: AutonomyLevel,
    pub input_schema: serde_json::Value,
    pub provenance_rule: Option<&'static str>,
}

/// 算子运行时调用上下文
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FunctionCallInfo {
    pub proc_name: String,
    pub args: serde_json::Value,
    pub caller: CallerIdentity,
    pub trace_id: String,
}

/// 5. 函数/工具算子原语 (对标 PostgreSQL `pg_proc`)
pub trait ProcRoutine: Send + Sync {
    fn spec(&self) -> ProcSpec;
    fn invoke(
        &self,
        fcinfo: FunctionCallInfo,
        ctx: &KernelServices,
    ) -> anyhow::Result<serde_json::Value>;
}

/// 后台自治协程触发方式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerTrigger {
    Interval(Duration),
    EventTopics(&'static [&'static str]),
}

/// 后台自治协程单次 Tick 执行结果
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerTickOutcome {
    pub processed_count: usize,
    pub emitted_events: usize,
    pub note: Option<String>,
}

/// 轻量级无锁协作取消令牌
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// 6. 后台自治工作协程原语 (对标 PostgreSQL `BackgroundWorker`)
pub trait BgWorkerRoutine: Send + Sync {
    fn worker_id(&self) -> &'static str;
    fn trigger(&self) -> WorkerTrigger;
    fn autonomy_level(&self) -> AutonomyLevel;
    fn run_tick(
        &self,
        ctx: &KernelServices,
        cancel: &CancellationToken,
    ) -> anyhow::Result<WorkerTickOutcome>;
}

/// 内核向扩展算子与后台协程暴露的受控服务门面
#[derive(Clone)]
pub struct KernelServices {
    pub caller: CallerIdentity,
    pub trace_id: String,
    pub table_am: Arc<dyn TableAmRoutine>,
    pub observability: Option<Arc<dyn ObservabilityProvider>>,
}

impl KernelServices {
    pub fn new(
        caller: CallerIdentity,
        trace_id: impl Into<String>,
        table_am: Arc<dyn TableAmRoutine>,
    ) -> Self {
        Self {
            caller,
            trace_id: trace_id.into(),
            table_am,
            observability: None,
        }
    }

    /// 挂载只读内核可观测快照提供器（保持与 `KernelServices::new` 向后兼容）
    pub fn with_observability(mut self, provider: Arc<dyn ObservabilityProvider>) -> Self {
        self.observability = Some(provider);
        self
    }

    pub fn table_am(&self) -> &dyn TableAmRoutine {
        self.table_am.as_ref()
    }

    /// 获取当前内核实时可观测快照（若运行时未挂载则返回 `None`）
    pub fn observability_snapshot(&self) -> Option<KernelObservabilitySnapshot> {
        self.observability.as_ref().map(|p| p.snapshot())
    }
}
