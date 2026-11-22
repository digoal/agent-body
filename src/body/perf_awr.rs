//! 本体等待事件 (`WaitEventRegistry`)、`/proc/self` 内核探针、AWR 快照基线差分与 ADDM 自优化引擎
//!
//! 对应设计规格书: `docs/design_doc/18-wait-events-and-awr-perf-diagnostics-spec.md` (Phase 50 · ISSUE #32)
//! 对齐规约: #2 (递弱代偿), #8 (L1_AUTONOMIC 自调优), #9 (<100ns 极速探针 & <90MB 内存), #16 (黑白屏 1:1:1), #26 (双域审计), #27 (本体稳态)

use std::fmt::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::audit::{AuditEventInput, AuditLogger};
use crate::body::manifest::compute_body_id;
use crate::body::vitals::HomeostasisVitalsEngine;
use crate::storage::{Storage, StorageProvider};

/// 8 档对数分位延迟桶边界（单位：微秒）
pub const LATENCY_BUCKET_BOUNDS_US: [u64; 8] = [
    100,       // B0: < 100 μs
    500,       // B1: 100 ~ 500 μs
    2_000,     // B2: 500 μs ~ 2 ms
    10_000,    // B3: 2 ~ 10 ms
    50_000,    // B4: 10 ~ 50 ms
    250_000,   // B5: 50 ~ 250 ms
    1_000_000, // B6: 250 ms ~ 1 s
    u64::MAX,  // B7: >= 1 s
];

pub const LATENCY_BUCKET_LABELS: [&str; 8] = [
    "<100us",
    "100~500us",
    "0.5~2ms",
    "2~10ms",
    "10~50ms",
    "50~250ms",
    "250ms~1s",
    ">=1s",
];

/// 6 大类 18 种本体标准等待/执行事件（对标 PostgreSQL pg_stat_activity.wait_event）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(usize)]
pub enum WaitEventId {
    LockSqliteMutex = 0,
    LockDuckDbSync = 1,
    LockCoordination = 2,
    IoSqliteRead = 3,
    IoSqliteWrite = 4,
    IoDiskWalk = 5,
    IoProcFs = 6,
    IoInotifyPoll = 7,
    NetOllamaEmbed = 8,
    NetOllamaEnrich = 9,
    NetMeshRpc = 10,
    CpuLinkRegex = 11,
    CpuVectorCosine = 12,
    CpuSecretRedact = 13,
    CpuBriefAssemble = 14,
    IpcEventBus = 15,
    IpcHookSubprocess = 16,
    IdleDaemonLoop = 17,
}

pub const WAIT_EVENT_COUNT: usize = 18;

impl WaitEventId {
    pub const ALL: [WaitEventId; WAIT_EVENT_COUNT] = [
        Self::LockSqliteMutex,
        Self::LockDuckDbSync,
        Self::LockCoordination,
        Self::IoSqliteRead,
        Self::IoSqliteWrite,
        Self::IoDiskWalk,
        Self::IoProcFs,
        Self::IoInotifyPoll,
        Self::NetOllamaEmbed,
        Self::NetOllamaEnrich,
        Self::NetMeshRpc,
        Self::CpuLinkRegex,
        Self::CpuVectorCosine,
        Self::CpuSecretRedact,
        Self::CpuBriefAssemble,
        Self::IpcEventBus,
        Self::IpcHookSubprocess,
        Self::IdleDaemonLoop,
    ];

    #[inline]
    pub const fn wait_class(&self) -> &'static str {
        match self {
            Self::LockSqliteMutex | Self::LockDuckDbSync | Self::LockCoordination => "Lock",
            Self::IoSqliteRead
            | Self::IoSqliteWrite
            | Self::IoDiskWalk
            | Self::IoProcFs
            | Self::IoInotifyPoll => "IO",
            Self::NetOllamaEmbed | Self::NetOllamaEnrich | Self::NetMeshRpc => "Net",
            Self::CpuLinkRegex
            | Self::CpuVectorCosine
            | Self::CpuSecretRedact
            | Self::CpuBriefAssemble => "CPU",
            Self::IpcEventBus | Self::IpcHookSubprocess => "IPC",
            Self::IdleDaemonLoop => "Idle",
        }
    }

    #[inline]
    pub const fn event_name(&self) -> &'static str {
        match self {
            Self::LockSqliteMutex => "Lock::SqliteMutex",
            Self::LockDuckDbSync => "Lock::DuckDbSync",
            Self::LockCoordination => "Lock::Coordination",
            Self::IoSqliteRead => "IO::SqliteRead",
            Self::IoSqliteWrite => "IO::SqliteWrite",
            Self::IoDiskWalk => "IO::DiskWalk",
            Self::IoProcFs => "IO::ProcFs",
            Self::IoInotifyPoll => "IO::InotifyPoll",
            Self::NetOllamaEmbed => "Net::OllamaEmbed",
            Self::NetOllamaEnrich => "Net::OllamaEnrich",
            Self::NetMeshRpc => "Net::MeshRpc",
            Self::CpuLinkRegex => "CPU::LinkRegex",
            Self::CpuVectorCosine => "CPU::VectorCosine",
            Self::CpuSecretRedact => "CPU::SecretRedact",
            Self::CpuBriefAssemble => "CPU::BriefAssemble",
            Self::IpcEventBus => "IPC::EventBus",
            Self::IpcHookSubprocess => "IPC::HookSubprocess",
            Self::IdleDaemonLoop => "Idle::DaemonLoop",
        }
    }

    #[inline]
    pub const fn description(&self) -> &'static str {
        match self {
            Self::LockSqliteMutex => "SQLite 主连接互斥锁获取等待",
            Self::LockDuckDbSync => "DuckDB OLAP 同步管道单飞锁等待",
            Self::LockCoordination => "多 Agent 资源协作锁排队与冲突仲裁",
            Self::IoSqliteRead => "SQLite 查询、FTS5 检索与属性图遍历读取",
            Self::IoSqliteWrite => "SQLite 事务落盘与索引更新写入",
            Self::IoDiskWalk => "工作区目录遍历、元数据提取与新鲜度对账磁盘 I/O",
            Self::IoProcFs => "Linux /proc 进程、Socket 端口与 Systemd 采集",
            Self::IoInotifyPoll => "inotify 文件变更事件轮询与防抖合并",
            Self::NetOllamaEmbed => "本地 Ollama Embedding 向量 HTTP 推理等待",
            Self::NetOllamaEnrich => "本地 Ollama L2 元数据精炼 HTTP 推理等待",
            Self::NetMeshRpc => "跨机联邦 MeshEnvelope v1.0 远程协同 RPC",
            Self::CpuLinkRegex => "多语言脚本依赖正则与语法模式抽取计算",
            Self::CpuVectorCosine => "高维向量余弦相似度与 L3 隐式图谱挖掘计算",
            Self::CpuSecretRedact => "SecretRedactor 凭据脱敏与抗提示词投毒扫描",
            Self::CpuBriefAssemble => "BriefAssembler 自适应 Token 裁剪与租约计算",
            Self::IpcEventBus => "内部 EventBus 与 Web SSE 实时事件分发",
            Self::IpcHookSubprocess => "外部 Hook 子进程命令派发执行",
            Self::IdleDaemonLoop => "守护进程事件循环空闲等待",
        }
    }
}

struct AtomicWaitSlot {
    calls: AtomicU64,
    total_us: AtomicU64,
    max_us: AtomicU64,
    buckets: [AtomicU64; 8],
}

impl AtomicWaitSlot {
    const fn new() -> Self {
        Self {
            calls: AtomicU64::new(0),
            total_us: AtomicU64::new(0),
            max_us: AtomicU64::new(0),
            buckets: [
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
                AtomicU64::new(0),
            ],
        }
    }

    #[inline]
    fn record(&self, elapsed_us: u64) {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.total_us.fetch_add(elapsed_us, Ordering::Relaxed);
        let mut prev_max = self.max_us.load(Ordering::Relaxed);
        while elapsed_us > prev_max {
            match self.max_us.compare_exchange_weak(
                prev_max,
                elapsed_us,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => prev_max = actual,
            }
        }
        let b_idx = if elapsed_us < 100 {
            0
        } else if elapsed_us < 500 {
            1
        } else if elapsed_us < 2_000 {
            2
        } else if elapsed_us < 10_000 {
            3
        } else if elapsed_us < 50_000 {
            4
        } else if elapsed_us < 250_000 {
            5
        } else if elapsed_us < 1_000_000 {
            6
        } else {
            7
        };
        self.buckets[b_idx].fetch_add(1, Ordering::Relaxed);
    }
}

#[allow(clippy::declare_interior_mutable_const)]
const INIT_SLOT: AtomicWaitSlot = AtomicWaitSlot::new();
static GLOBAL_WAIT_SLOTS: [AtomicWaitSlot; WAIT_EVENT_COUNT] = [INIT_SLOT; WAIT_EVENT_COUNT];

/// RAII 零分配等待事件计时守卫（Drop 时自动原子入账，单次开销 < 50ns）
pub struct WaitEventGuard {
    event: WaitEventId,
    start: Instant,
}

impl WaitEventGuard {
    #[inline]
    pub fn enter(event: WaitEventId) -> Self {
        Self {
            event,
            start: Instant::now(),
        }
    }
}

impl Drop for WaitEventGuard {
    #[inline]
    fn drop(&mut self) {
        let us = self.start.elapsed().as_micros() as u64;
        GLOBAL_WAIT_SLOTS[self.event as usize].record(us);
    }
}

/// 单个等待事件的快照采样结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitEventSnapshotRow {
    pub event_name: String,
    pub wait_class: String,
    pub description: String,
    pub calls: u64,
    pub total_us: u64,
    pub avg_us: f64,
    pub max_us: u64,
    pub est_p95_us: u64,
    pub pct_of_body_time: f64,
    pub histogram_buckets: [u64; 8],
}

/// 全局无锁等待事件注册表门面
pub struct WaitEventRegistry;

impl WaitEventRegistry {
    /// 直接手动记录一次已测得耗时的等待事件（适用于已有 Instant 计时器的切面）
    #[inline]
    pub fn record_us(event: WaitEventId, elapsed_us: u64) {
        GLOBAL_WAIT_SLOTS[event as usize].record(elapsed_us);
    }

    /// 采集当前全部 18 种等待事件的原子快照
    pub fn snapshot_all() -> Vec<WaitEventSnapshotRow> {
        let mut raw = Vec::with_capacity(WAIT_EVENT_COUNT);
        let mut active_total_us = 0u64;

        for ev in WaitEventId::ALL {
            let slot = &GLOBAL_WAIT_SLOTS[ev as usize];
            let calls = slot.calls.load(Ordering::Relaxed);
            let total_us = slot.total_us.load(Ordering::Relaxed);
            let max_us = slot.max_us.load(Ordering::Relaxed);
            let mut buckets = [0u64; 8];
            for (i, b) in slot.buckets.iter().enumerate() {
                buckets[i] = b.load(Ordering::Relaxed);
            }
            if ev != WaitEventId::IdleDaemonLoop {
                active_total_us = active_total_us.saturating_add(total_us);
            }
            raw.push((ev, calls, total_us, max_us, buckets));
        }

        let denom = active_total_us.max(1) as f64;
        let mut rows = Vec::with_capacity(WAIT_EVENT_COUNT);
        for (ev, calls, total_us, max_us, buckets) in raw {
            let avg_us = if calls == 0 {
                0.0
            } else {
                ((total_us as f64 / calls as f64) * 100.0).round() / 100.0
            };
            let est_p95_us = estimate_percentile_from_buckets(&buckets, 0.95, max_us);
            let pct = if ev == WaitEventId::IdleDaemonLoop || active_total_us == 0 {
                0.0
            } else {
                (((total_us as f64 / denom) * 100.0) * 100.0).round() / 100.0
            };
            rows.push(WaitEventSnapshotRow {
                event_name: ev.event_name().to_string(),
                wait_class: ev.wait_class().to_string(),
                description: ev.description().to_string(),
                calls,
                total_us,
                avg_us,
                max_us,
                est_p95_us,
                pct_of_body_time: pct,
                histogram_buckets: buckets,
            });
        }
        rows.sort_by(|a, b| b.total_us.cmp(&a.total_us));
        rows
    }
}

fn estimate_percentile_from_buckets(buckets: &[u64; 8], quantile: f64, max_us: u64) -> u64 {
    let total: u64 = buckets.iter().sum();
    if total == 0 {
        return 0;
    }
    let target = ((total as f64) * quantile).ceil() as u64;
    let mut acc = 0u64;
    let bucket_midpoints: [u64; 8] = [50, 300, 1_200, 6_000, 30_000, 150_000, 600_000, 1_500_000];
    for (i, &cnt) in buckets.iter().enumerate() {
        acc += cnt;
        if acc >= target {
            return bucket_midpoints[i].min(max_us);
        }
    }
    max_us
}

/// 零特权 `/proc/self` 内核资源遥测画像（轻量 eBPF 等价层）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KernelSelfTelemetry {
    pub pid: u32,
    pub rss_kb: u64,
    pub hwm_kb: u64,
    pub vm_size_kb: u64,
    pub threads: u64,
    pub open_fds: usize,
    pub voluntary_ctxt_switches: u64,
    pub nonvoluntary_ctxt_switches: u64,
    pub io_read_bytes: u64,
    pub io_write_bytes: u64,
    pub io_syscr: u64,
    pub io_syscw: u64,
    pub sqlite_db_bytes: u64,
    pub sqlite_wal_bytes: u64,
}

impl KernelSelfTelemetry {
    /// 采集当前进程 `/proc/self` 内核状态与 SQLite 文件物理尺寸
    pub fn collect(db_path_opt: Option<&Path>) -> Self {
        let mut t = Self {
            pid: std::process::id(),
            ..Default::default()
        };

        if let Ok(status_txt) = std::fs::read_to_string("/proc/self/status") {
            for line in status_txt.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let num = v
                        .split_whitespace()
                        .next()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(0);
                    match k.trim() {
                        "VmRSS" => t.rss_kb = num,
                        "VmHWM" => t.hwm_kb = num,
                        "VmSize" => t.vm_size_kb = num,
                        "Threads" => t.threads = num,
                        "voluntary_ctxt_switches" => t.voluntary_ctxt_switches = num,
                        "nonvoluntary_ctxt_switches" => t.nonvoluntary_ctxt_switches = num,
                        _ => {}
                    }
                }
            }
        }

        if let Ok(io_txt) = std::fs::read_to_string("/proc/self/io") {
            for line in io_txt.lines() {
                if let Some((k, v)) = line.split_once(':') {
                    let num = v.trim().parse::<u64>().unwrap_or(0);
                    match k.trim() {
                        "read_bytes" => t.io_read_bytes = num,
                        "write_bytes" => t.io_write_bytes = num,
                        "syscr" => t.io_syscr = num,
                        "syscw" => t.io_syscw = num,
                        _ => {}
                    }
                }
            }
        }

        if let Ok(entries) = std::fs::read_dir("/proc/self/fd") {
            t.open_fds = entries.count();
        }

        // 非 Linux（macOS / Windows）或受限无 /proc 容器场景回退到 sysinfo 采集 RSS/VM/IO
        if t.rss_kb == 0 {
            let sys_t = Self::collect_via_sysinfo_raw();
            if t.rss_kb == 0 {
                t.rss_kb = sys_t.rss_kb;
            }
            if t.hwm_kb == 0 {
                t.hwm_kb = sys_t.hwm_kb;
            }
            if t.vm_size_kb == 0 {
                t.vm_size_kb = sys_t.vm_size_kb;
            }
            if t.io_read_bytes == 0 {
                t.io_read_bytes = sys_t.io_read_bytes;
            }
            if t.io_write_bytes == 0 {
                t.io_write_bytes = sys_t.io_write_bytes;
            }
        }

        let resolved_db = db_path_opt
            .map(|p| p.to_path_buf())
            .unwrap_or_else(crate::config::default_db_path);
        if let Ok(m) = std::fs::metadata(&resolved_db) {
            t.sqlite_db_bytes = m.len();
        }
        let wal_path_str = format!("{}-wal", resolved_db.to_string_lossy());
        if let Ok(m) = std::fs::metadata(&wal_path_str) {
            t.sqlite_wal_bytes = m.len();
        }

        t
    }

    /// 跨平台 `sysinfo` 内核状态回退采集（供 macOS / Windows 或无 `/proc/self` 容器环境复用）
    pub fn collect_via_sysinfo_raw() -> Self {
        let mut t = Self {
            pid: std::process::id(),
            ..Default::default()
        };
        if let Ok(pid) = sysinfo::get_current_pid() {
            let mut sys = sysinfo::System::new();
            sys.refresh_process(pid);
            if let Some(proc_info) = sys.process(pid) {
                t.rss_kb = proc_info.memory() / 1024;
                t.hwm_kb = t.rss_kb;
                t.vm_size_kb = proc_info.virtual_memory() / 1024;
                let disk = proc_info.disk_usage();
                t.io_read_bytes = disk.total_read_bytes;
                t.io_write_bytes = disk.total_written_bytes;
            }
        }
        t
    }
}

/// 操作级性能画像行（对标 PostgreSQL `pg_stat_statements`）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationStatRow {
    pub operation: String,
    pub category: String,
    pub calls: usize,
    pub total_us: i64,
    pub avg_us: f64,
    pub max_us: i64,
    pub p95_us: i64,
    pub fail_count: usize,
}

/// Body Time 分解汇总模型
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BodyTimeModel {
    pub total_body_time_us: u64,
    pub cpu_time_us: u64,
    pub lock_wait_us: u64,
    pub io_wait_us: u64,
    pub net_wait_us: u64,
    pub ipc_wait_us: u64,
    pub cpu_pct: f64,
    pub lock_pct: f64,
    pub io_pct: f64,
    pub net_pct: f64,
    pub ipc_pct: f64,
}

impl BodyTimeModel {
    pub fn from_wait_rows(rows: &[WaitEventSnapshotRow]) -> Self {
        let mut m = Self::default();
        for r in rows {
            match r.wait_class.as_str() {
                "CPU" => m.cpu_time_us = m.cpu_time_us.saturating_add(r.total_us),
                "Lock" => m.lock_wait_us = m.lock_wait_us.saturating_add(r.total_us),
                "IO" => m.io_wait_us = m.io_wait_us.saturating_add(r.total_us),
                "Net" => m.net_wait_us = m.net_wait_us.saturating_add(r.total_us),
                "IPC" => m.ipc_wait_us = m.ipc_wait_us.saturating_add(r.total_us),
                _ => {}
            }
        }
        m.total_body_time_us = m
            .cpu_time_us
            .saturating_add(m.lock_wait_us)
            .saturating_add(m.io_wait_us)
            .saturating_add(m.net_wait_us)
            .saturating_add(m.ipc_wait_us);
        let d = m.total_body_time_us.max(1) as f64;
        let round2 = |v: u64| (((v as f64 / d) * 100.0) * 100.0).round() / 100.0;
        if m.total_body_time_us > 0 {
            m.cpu_pct = round2(m.cpu_time_us);
            m.lock_pct = round2(m.lock_wait_us);
            m.io_pct = round2(m.io_wait_us);
            m.net_pct = round2(m.net_wait_us);
            m.ipc_pct = round2(m.ipc_wait_us);
        }
        m
    }
}

/// AWR 性能快照完整载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwrSnapshot {
    pub snapshot_id: String,
    pub body_id: String,
    pub label: String,
    pub is_baseline: bool,
    pub captured_at: String,
    pub homeostasis_score: f64,
    pub time_model: BodyTimeModel,
    pub wait_events: Vec<WaitEventSnapshotRow>,
    pub top_operations: Vec<OperationStatRow>,
    pub kernel_telemetry: KernelSelfTelemetry,
}

/// AWR 快照轻量摘要（供前端/CLI 快照下拉选择器展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwrSnapshotMeta {
    pub snapshot_id: String,
    pub label: String,
    pub is_baseline: bool,
    pub captured_at: String,
    pub rss_kb: u64,
    pub hwm_kb: u64,
    pub open_fds: usize,
    pub total_body_time_us: u64,
    pub homeostasis_score: f64,
}

impl AwrSnapshot {
    pub fn to_meta(&self) -> AwrSnapshotMeta {
        AwrSnapshotMeta {
            snapshot_id: self.snapshot_id.clone(),
            label: self.label.clone(),
            is_baseline: self.is_baseline,
            captured_at: self.captured_at.clone(),
            rss_kb: self.kernel_telemetry.rss_kb,
            hwm_kb: self.kernel_telemetry.hwm_kb,
            open_fds: self.kernel_telemetry.open_fds,
            total_body_time_us: self.time_model.total_body_time_us,
            homeostasis_score: self.homeostasis_score,
        }
    }
}

/// ADDM 自动性能瓶颈诊断发现项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddmFinding {
    pub rule_id: String,
    pub severity: String, // "INFO" | "WARN" | "CRITICAL"
    pub bottleneck_class: String,
    pub summary: String,
    pub evidence: String,
    pub recommendation: String,
    pub auto_tunable_l1: bool,
}

/// 已保存的 AWR 诊断报告轻量摘要（供历史报告下拉选择器展示）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwrReportMeta {
    pub report_id: String,
    pub mode: String,
    pub base_snapshot_id: String,
    pub base_label: String,
    pub target_snapshot_id: String,
    pub target_label: String,
    pub generated_at: String,
    pub regression_detected: bool,
    pub rss_drift_pct: f64,
    pub total_body_time_us: u64,
}

/// AWR 快照差分对比与 ADDM 诊断报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwrDiffReport {
    pub report_id: String,
    pub body_id: String,
    pub mode: String, // "baseline_vs_current" | "snap_a_vs_snap_b" | "single_snapshot"
    pub base_snapshot_id: String,
    #[serde(default)]
    pub base_label: String,
    #[serde(default)]
    pub base_captured_at: String,
    pub target_snapshot_id: String,
    #[serde(default)]
    pub target_label: String,
    #[serde(default)]
    pub target_captured_at: String,
    pub generated_at: String,
    pub regression_detected: bool,
    pub rss_delta_kb: i64,
    pub rss_drift_pct: f64,
    pub fd_delta: i64,
    pub voluntary_cs_delta: i64,
    pub nonvoluntary_cs_delta: i64,
    pub io_write_bytes_delta: i64,
    pub delta_time_model: BodyTimeModel,
    pub top_wait_events: Vec<WaitEventSnapshotRow>,
    pub top_slow_operations: Vec<OperationStatRow>,
    pub addm_findings: Vec<AddmFinding>,
    #[serde(default)]
    pub base_kernel: KernelSelfTelemetry,
    pub target_kernel: KernelSelfTelemetry,
    pub markdown_summary: String,
}

impl AwrDiffReport {
    pub fn to_meta(&self) -> AwrReportMeta {
        AwrReportMeta {
            report_id: self.report_id.clone(),
            mode: self.mode.clone(),
            base_snapshot_id: self.base_snapshot_id.clone(),
            base_label: self.base_label.clone(),
            target_snapshot_id: self.target_snapshot_id.clone(),
            target_label: self.target_label.clone(),
            generated_at: self.generated_at.clone(),
            regression_detected: self.regression_detected,
            rss_drift_pct: self.rss_drift_pct,
            total_body_time_us: self.delta_time_model.total_body_time_us,
        }
    }
}

/// L1 自主性能调优报告 (`perf_tune`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfTuneReport {
    pub body_id: String,
    pub tuned_at: String,
    pub before_rss_kb: u64,
    pub after_rss_kb: u64,
    pub rss_freed_kb: i64,
    pub before_wal_bytes: u64,
    pub after_wal_bytes: u64,
    pub reconciled_assets_checked: usize,
    pub reconciled_missing_marked: usize,
    pub expired_locks_cleaned: usize,
    pub actions_executed: Vec<String>,
    pub elapsed_us: u64,
}

const META_SNAP_INDEX: &str = "awr:index";
const META_REPORT_INDEX: &str = "awr:report_index";
const META_BASELINE_KEY: &str = "awr:baseline";
const META_LATEST_SNAP_KEY: &str = "awr:latest";
const META_PRAGMA_OPTIMIZE: &str = "awr:pragma_optimize";
const MAX_RETAINED_SNAPSHOTS: usize = 32;
const MAX_RETAINED_REPORTS: usize = 32;

pub struct BodyPerfEngine;

impl BodyPerfEngine {
    fn save_meta_json(provider: &dyn StorageProvider, key: &str, val: serde_json::Value) -> Result<()> {
        let now = Local::now().format("%Y-%m-%dT%H:%M:%S+08:00").to_string();
        let rec = crate::storage::models::BenchmarkRunRecord {
            run_id: key.to_string(),
            suite_name: "awr_internal".to_string(),
            git_commit: crate::BUILD_GIT_HASH.to_string(),
            app_version: crate::VERSION.to_string(),
            host_fingerprint: serde_json::json!({}),
            workload_count: 0,
            total_baseline_tokens: 0,
            total_steward_tokens: 0,
            overall_token_savings_pct: 0.0,
            overall_speedup_factor: 1.0,
            total_hazards_intercepted: 0,
            total_noise_blocked: 0,
            sla_passed: true,
            results_json: val,
            report_sha256: "awr".to_string(),
            duration_ms: 0,
            created_at: now,
        };
        provider.insert_benchmark_run(&rec)
    }

    fn load_meta_json(provider: &dyn StorageProvider, key: &str) -> Option<serde_json::Value> {
        provider
            .get_benchmark_run(key)
            .ok()
            .flatten()
            .map(|r| r.results_json)
    }
    /// 从 `audit_events` 聚合 Top-N 操作级统计（对标 `pg_stat_statements`）
    pub fn collect_operation_stats(storage: &dyn StorageProvider, limit: usize) -> Vec<OperationStatRow> {
        let events = storage
            .query_audit_events(&crate::storage::models::AuditQueryFilter {
                window: Some("24h".to_string()),
                limit: 1000,
                ..Default::default()
            })
            .unwrap_or_default();

        let mut groups: std::collections::HashMap<(String, String), Vec<(i64, bool)>> =
            std::collections::HashMap::new();
        for ev in events {
            let key = (ev.action.clone(), ev.category.clone());
            let failed = ev.outcome == "failed" || ev.outcome == "blocked";
            groups.entry(key).or_default().push((ev.elapsed_us.max(0), failed));
        }

        let mut rows = Vec::with_capacity(groups.len());
        for ((op, cat), mut samples) in groups {
            let calls = samples.len();
            if calls == 0 {
                continue;
            }
            samples.sort_by_key(|x| x.0);
            let total_us: i64 = samples.iter().map(|x| x.0).sum();
            let max_us = samples.last().map(|x| x.0).unwrap_or(0);
            let p95_idx = ((calls as f64 * 0.95).ceil() as usize).min(calls).saturating_sub(1);
            let p95_us = samples[p95_idx].0;
            let fail_count = samples.iter().filter(|x| x.1).count();
            let avg_us = ((total_us as f64 / calls as f64) * 100.0).round() / 100.0;
            rows.push(OperationStatRow {
                operation: op,
                category: cat,
                calls,
                total_us,
                avg_us,
                max_us,
                p95_us,
                fail_count,
            });
        }
        rows.sort_by(|a, b| b.total_us.cmp(&a.total_us));
        rows.truncate(limit);
        rows
    }

    /// 抓取当前本体 AWR 性能快照并持久化至 `system_meta`（可选设为黄金性能基准）
    pub fn capture_snapshot(
        storage: &Storage,
        label: Option<&str>,
        set_baseline: bool,
    ) -> Result<AwrSnapshot> {
        let _guard = WaitEventGuard::enter(WaitEventId::IoSqliteRead);
        let provider = storage.provider();
        let snap_id = format!("snap-{}", Local::now().format("%Y%m%d-%H%M%S-%3f"));
        let wait_events = WaitEventRegistry::snapshot_all();
        let time_model = BodyTimeModel::from_wait_rows(&wait_events);
        let top_operations = Self::collect_operation_stats(provider.as_ref(), 10);
        let kernel_telemetry = KernelSelfTelemetry::collect(None);
        let vitals = HomeostasisVitalsEngine::assess_provider(provider.as_ref());

        let snap = AwrSnapshot {
            snapshot_id: snap_id.clone(),
            body_id: compute_body_id(),
            label: label.unwrap_or("manual").to_string(),
            is_baseline: set_baseline,
            captured_at: Local::now().format("%Y-%m-%dT%H:%M:%S+08:00").to_string(),
            homeostasis_score: vitals.overall_score,
            time_model,
            wait_events,
            top_operations,
            kernel_telemetry,
        };

        let snap_val = serde_json::to_value(&snap)?;
        let snap_key = format!("awr:snap:{}", snap_id);
        Self::save_meta_json(provider.as_ref(), &snap_key, snap_val.clone())?;
        Self::save_meta_json(
            provider.as_ref(),
            META_LATEST_SNAP_KEY,
            serde_json::json!({ "latest_id": snap_id }),
        )?;
        if set_baseline {
            Self::save_meta_json(provider.as_ref(), META_BASELINE_KEY, snap_val)?;
        }

        // 维护最近 32 份滚动索引，同时物理清理被淘汰的孤儿快照行（黄金基线豁免）
        let baseline_snap_id = Self::load_snapshot(provider.as_ref(), "baseline").map(|s| s.snapshot_id);
        let mut idx_list: Vec<String> = Self::load_meta_json(provider.as_ref(), META_SNAP_INDEX)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        idx_list.push(snap_id.clone());
        while idx_list.len() > MAX_RETAINED_SNAPSHOTS {
            let oldest = idx_list.remove(0);
            if baseline_snap_id.as_deref() != Some(oldest.as_str()) {
                if let Err(e) = provider.delete_benchmark_run(&format!("awr:snap:{}", oldest)) {
                    eprintln!("[PerfAWR][WARN] 清理淘汰快照 awr:snap:{} 失败: {}", oldest, e);
                }
            }
        }
        if let Ok(idx_val) = serde_json::to_value(&idx_list) {
            Self::save_meta_json(provider.as_ref(), META_SNAP_INDEX, idx_val)?;
        }

        AuditLogger::record_best_effort(
            storage,
            AuditEventInput::in_agent("body", "perf_awr", "perf_snapshot")
                .with_target("awr_snapshot", &snap_id)
                .with_outcome("success")
                .with_delta(serde_json::json!({
                    "label": snap.label,
                    "is_baseline": set_baseline,
                    "rss_kb": snap.kernel_telemetry.rss_kb,
                    "open_fds": snap.kernel_telemetry.open_fds,
                    "total_body_time_us": snap.time_model.total_body_time_us,
                })),
        );

        Ok(snap)
    }

    /// 读取指定快照或黄金基线
    pub fn load_snapshot(storage: &dyn StorageProvider, id_or_tag: &str) -> Option<AwrSnapshot> {
        let key = if id_or_tag == "baseline" {
            META_BASELINE_KEY.to_string()
        } else if id_or_tag == "latest" {
            let v = Self::load_meta_json(storage, META_LATEST_SNAP_KEY)?;
            let latest_id = v.get("latest_id")?.as_str()?;
            format!("awr:snap:{}", latest_id)
        } else if id_or_tag.starts_with("awr:snap:") {
            id_or_tag.to_string()
        } else {
            format!("awr:snap:{}", id_or_tag)
        };
        let val = Self::load_meta_json(storage, &key)?;
        serde_json::from_value(val).ok()
    }

    /// 构造内存瞬时快照（不写入快照历史列表，避免查看报告时污染用户快照库）
    pub fn sample_transient_snapshot(storage: &Storage, label: &str) -> AwrSnapshot {
        let provider = storage.provider();
        let snap_id = format!("live-{}", Local::now().format("%H%M%S"));
        let wait_events = WaitEventRegistry::snapshot_all();
        let time_model = BodyTimeModel::from_wait_rows(&wait_events);
        let top_operations = Self::collect_operation_stats(provider.as_ref(), 10);
        let kernel_telemetry = KernelSelfTelemetry::collect(None);
        let vitals = HomeostasisVitalsEngine::assess_provider(provider.as_ref());

        AwrSnapshot {
            snapshot_id: snap_id,
            body_id: compute_body_id(),
            label: label.to_string(),
            is_baseline: false,
            captured_at: Local::now().format("%Y-%m-%dT%H:%M:%S+08:00").to_string(),
            homeostasis_score: vitals.overall_score,
            time_model,
            wait_events,
            top_operations,
            kernel_telemetry,
        }
    }

    /// 列出已保存的快照 ID 列表与当前是否已设定黄金基线
    pub fn list_snapshot_ids(storage: &dyn StorageProvider) -> (Vec<String>, Option<String>) {
        let idx_list: Vec<String> = Self::load_meta_json(storage, META_SNAP_INDEX)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let baseline_id = Self::load_snapshot(storage, "baseline").map(|s| s.snapshot_id);
        (idx_list, baseline_id)
    }

    /// 列出全部已保存快照的详细元数据（按时间倒序，过滤旧版 awr-probe 临时项）及黄金基线元数据
    pub fn list_snapshots_meta(
        storage: &dyn StorageProvider,
    ) -> (Vec<AwrSnapshotMeta>, Option<AwrSnapshotMeta>) {
        let baseline_snap = Self::load_snapshot(storage, "baseline");
        let baseline_id = baseline_snap.as_ref().map(|s| s.snapshot_id.clone());
        let baseline_meta = baseline_snap.map(|s| s.to_meta());

        let idx_list: Vec<String> = Self::load_meta_json(storage, META_SNAP_INDEX)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();

        let mut metas = Vec::new();
        for id in idx_list.iter().rev() {
            if let Some(mut snap) = Self::load_snapshot(storage, id) {
                if snap.label == "awr-probe" {
                    continue;
                }
                if baseline_id.as_deref() == Some(snap.snapshot_id.as_str()) {
                    snap.is_baseline = true;
                }
                metas.push(snap.to_meta());
            }
        }
        // 确保黄金基线也在列表中可选
        if let Some(ref bm) = baseline_meta {
            if !metas.iter().any(|m| m.snapshot_id == bm.snapshot_id) {
                metas.push(bm.clone());
            }
        }
        (metas, baseline_meta)
    }

    /// 读取已保存的历史 AWR 差分报告
    pub fn load_awr_report(storage: &dyn StorageProvider, report_id: &str) -> Option<AwrDiffReport> {
        let key = if report_id.starts_with("awr:report:") {
            report_id.to_string()
        } else {
            format!("awr:report:{}", report_id)
        };
        let val = Self::load_meta_json(storage, &key)?;
        serde_json::from_value(val).ok()
    }

    /// 列出已保存的历史 AWR 诊断报告摘要列表（最新在前）
    pub fn list_awr_reports_meta(storage: &dyn StorageProvider) -> Vec<AwrReportMeta> {
        let list: Vec<AwrReportMeta> = Self::load_meta_json(storage, META_REPORT_INDEX)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        list
    }

    /// 删除指定 AWR 性能快照（同步维护索引、`awr:latest` 与黄金基线引用）
    pub fn delete_snapshot(storage: &Storage, snapshot_id: &str) -> Result<bool> {
        let raw = snapshot_id.trim();
        if raw.is_empty() {
            anyhow::bail!("snapshot_id 不能为空");
        }
        let provider = storage.provider();
        let baseline_snap = Self::load_snapshot(provider.as_ref(), "baseline");
        let resolved_id = if raw == "baseline" {
            match baseline_snap.as_ref() {
                Some(b) => b.snapshot_id.clone(),
                None => return Ok(false),
            }
        } else {
            raw.strip_prefix("awr:snap:").unwrap_or(raw).to_string()
        };

        let mut deleted = provider.delete_benchmark_run(&format!("awr:snap:{}", resolved_id))?;
        if raw == "baseline"
            || baseline_snap
                .as_ref()
                .map(|b| b.snapshot_id == resolved_id)
                .unwrap_or(false)
        {
            if provider.delete_benchmark_run(META_BASELINE_KEY)? {
                deleted = true;
            }
        }

        let mut idx_list: Vec<String> = Self::load_meta_json(provider.as_ref(), META_SNAP_INDEX)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        let before_len = idx_list.len();
        idx_list.retain(|id| id != &resolved_id);
        if idx_list.len() != before_len {
            deleted = true;
            if let Ok(idx_val) = serde_json::to_value(&idx_list) {
                Self::save_meta_json(provider.as_ref(), META_SNAP_INDEX, idx_val)?;
            }
        }

        let latest_pointed = Self::load_meta_json(provider.as_ref(), META_LATEST_SNAP_KEY)
            .and_then(|v| v.get("latest_id").and_then(|x| x.as_str()).map(str::to_string));
        if latest_pointed.as_deref() == Some(resolved_id.as_str()) {
            if let Some(new_latest) = idx_list.last() {
                Self::save_meta_json(
                    provider.as_ref(),
                    META_LATEST_SNAP_KEY,
                    serde_json::json!({ "latest_id": new_latest }),
                )?;
            } else {
                let _deleted_latest = provider.delete_benchmark_run(META_LATEST_SNAP_KEY)?;
            }
        }

        AuditLogger::record_best_effort(
            storage,
            AuditEventInput::in_agent("body", "perf_awr", "perf_delete")
                .with_target("awr_snapshot", &resolved_id)
                .with_outcome(if deleted { "success" } else { "warn" })
                .with_delta(serde_json::json!({ "kind": "snapshot", "deleted": deleted })),
        );

        Ok(deleted)
    }

    /// 删除指定已保存的历史 AWR 诊断报告（同步维护 `awr:report_index`）
    pub fn delete_awr_report(storage: &Storage, report_id: &str) -> Result<bool> {
        let raw = report_id.trim();
        if raw.is_empty() {
            anyhow::bail!("report_id 不能为空");
        }
        let resolved_id = raw.strip_prefix("awr:report:").unwrap_or(raw).to_string();
        let provider = storage.provider();
        let mut deleted = provider.delete_benchmark_run(&format!("awr:report:{}", resolved_id))?;

        let mut list = Self::list_awr_reports_meta(provider.as_ref());
        let before_len = list.len();
        list.retain(|r| r.report_id != resolved_id);
        if list.len() != before_len {
            deleted = true;
            let idx_val = serde_json::to_value(&list)?;
            Self::save_meta_json(provider.as_ref(), META_REPORT_INDEX, idx_val)?;
        }

        AuditLogger::record_best_effort(
            storage,
            AuditEventInput::in_agent("body", "perf_awr", "perf_delete")
                .with_target("awr_report", &resolved_id)
                .with_outcome(if deleted { "success" } else { "warn" })
                .with_delta(serde_json::json!({ "kind": "report", "deleted": deleted })),
        );

        Ok(deleted)
    }

    /// 统一删除入口（支持 `kind = snapshot | report | auto`）
    pub fn delete_perf_artifact(
        storage: &Storage,
        kind: &str,
        target_id: &str,
    ) -> Result<serde_json::Value> {
        let tid = target_id.trim();
        if tid.is_empty() {
            anyhow::bail!("待删除的快照或报告 ID 不能为空");
        }
        let k_norm = kind.trim().to_ascii_lowercase();
        let resolved_kind = match k_norm.as_str() {
            "snapshot" | "snap" => "snapshot",
            "report" | "awr" => "report",
            "auto" | "" => {
                if tid.starts_with("awr-") || tid.starts_with("awr:report:") {
                    "report"
                } else if tid.starts_with("snap-")
                    || tid.starts_with("awr:snap:")
                    || tid == "baseline"
                {
                    "snapshot"
                } else {
                    "report"
                }
            }
            other => anyhow::bail!("不支持的 perf_delete 类型: {} (可选 auto|snapshot|report)", other),
        };

        let deleted = if resolved_kind == "snapshot" {
            Self::delete_snapshot(storage, tid)?
        } else {
            Self::delete_awr_report(storage, tid)?
        };

        Ok(serde_json::json!({
            "perf_deleted": true,
            "kind": resolved_kind,
            "target_id": tid,
            "deleted": deleted,
        }))
    }

    fn save_awr_report(provider: &dyn StorageProvider, report: &AwrDiffReport) -> Result<()> {
        let key = format!("awr:report:{}", report.report_id);
        let val = serde_json::to_value(report)?;
        Self::save_meta_json(provider, &key, val)?;

        let mut list = Self::list_awr_reports_meta(provider);
        list.retain(|r| r.report_id != report.report_id);
        list.insert(0, report.to_meta());
        while list.len() > MAX_RETAINED_REPORTS {
            if let Some(evicted) = list.pop() {
                if let Err(e) = provider.delete_benchmark_run(&format!("awr:report:{}", evicted.report_id)) {
                    eprintln!("[PerfAWR][WARN] 清理淘汰报告 awr:report:{} 失败: {}", evicted.report_id, e);
                }
            }
        }
        let idx_val = serde_json::to_value(&list)?;
        Self::save_meta_json(provider, META_REPORT_INDEX, idx_val)?;
        Ok(())
    }

    /// 生成 AWR 快照差分报告与 ADDM 瓶颈诊断处方（自动持久化保存至历史报告库）
    pub fn generate_awr_report(
        storage: &Storage,
        snap_a_opt: Option<&str>,
        snap_b_opt: Option<&str>,
        vs_baseline: bool,
    ) -> Result<AwrDiffReport> {
        Self::generate_awr_report_with_options(storage, snap_a_opt, snap_b_opt, vs_baseline, true)
    }

    /// 生成 AWR 快照差分报告（`persist=false` 用于只读 GET 实时预览，不落库污染报告库）
    pub fn generate_awr_report_with_options(
        storage: &Storage,
        snap_a_opt: Option<&str>,
        snap_b_opt: Option<&str>,
        vs_baseline: bool,
        persist: bool,
    ) -> Result<AwrDiffReport> {
        let provider = storage.provider();
        let current_live = Self::sample_transient_snapshot(storage, "实时当前态");

        let a_clean = snap_a_opt.map(str::trim).filter(|s| !s.is_empty());
        let b_clean = snap_b_opt.map(str::trim).filter(|s| !s.is_empty());

        let (mode, base_snap, target_snap) = match (a_clean, b_clean) {
            (Some(a), Some(b)) => {
                let sa = if a == "live" {
                    current_live.clone()
                } else {
                    Self::load_snapshot(provider.as_ref(), a).unwrap_or_else(|| current_live.clone())
                };
                let sb = if b == "live" {
                    current_live.clone()
                } else {
                    Self::load_snapshot(provider.as_ref(), b).unwrap_or_else(|| current_live.clone())
                };
                ("snap_a_vs_snap_b".to_string(), sa, sb)
            }
            (Some(a), None) => {
                let sa = Self::load_snapshot(provider.as_ref(), a).unwrap_or_else(|| current_live.clone());
                ("snap_a_vs_snap_b".to_string(), sa, current_live)
            }
            (None, Some(b)) => {
                let base = Self::load_snapshot(provider.as_ref(), "baseline")
                    .unwrap_or_else(|| current_live.clone());
                let sb = if b == "live" {
                    current_live.clone()
                } else {
                    Self::load_snapshot(provider.as_ref(), b).unwrap_or(current_live)
                };
                ("baseline_vs_target".to_string(), base, sb)
            }
            (None, None) => {
                if vs_baseline {
                    if let Some(base) = Self::load_snapshot(provider.as_ref(), "baseline") {
                        ("baseline_vs_current".to_string(), base, current_live)
                    } else {
                        ("single_snapshot".to_string(), current_live.clone(), current_live)
                    }
                } else {
                    ("single_snapshot".to_string(), current_live.clone(), current_live)
                }
            }
        };

        // 计算等待事件差分（若同一进程周期内 target >= base 则取增量，否则取 target 绝对值）
        let mut delta_waits = Vec::with_capacity(target_snap.wait_events.len());
        let base_map: std::collections::HashMap<&str, &WaitEventSnapshotRow> = base_snap
            .wait_events
            .iter()
            .map(|r| (r.event_name.as_str(), r))
            .collect();

        let is_same_snap = base_snap.snapshot_id == target_snap.snapshot_id;
        for tr in &target_snap.wait_events {
            if is_same_snap {
                delta_waits.push(tr.clone());
                continue;
            }
            if let Some(br) = base_map.get(tr.event_name.as_str()) {
                let d_calls = if tr.calls >= br.calls {
                    tr.calls - br.calls
                } else {
                    tr.calls
                };
                let d_total = if tr.total_us >= br.total_us {
                    tr.total_us - br.total_us
                } else {
                    tr.total_us
                };
                let d_avg = if d_calls == 0 {
                    0.0
                } else {
                    ((d_total as f64 / d_calls as f64) * 100.0).round() / 100.0
                };
                delta_waits.push(WaitEventSnapshotRow {
                    event_name: tr.event_name.clone(),
                    wait_class: tr.wait_class.clone(),
                    description: tr.description.clone(),
                    calls: d_calls,
                    total_us: d_total,
                    avg_us: d_avg,
                    max_us: tr.max_us,
                    est_p95_us: tr.est_p95_us,
                    pct_of_body_time: 0.0,
                    histogram_buckets: tr.histogram_buckets,
                });
            } else {
                delta_waits.push(tr.clone());
            }
        }

        let delta_time_model = if is_same_snap || BodyTimeModel::from_wait_rows(&delta_waits).total_body_time_us == 0 {
            // 若两个快照紧邻导致 delta 为 0，使用 target 全量时间模型便于诊断
            delta_waits = target_snap.wait_events.clone();
            target_snap.time_model.clone()
        } else {
            let tm = BodyTimeModel::from_wait_rows(&delta_waits);
            let denom = tm.total_body_time_us.max(1) as f64;
            for r in &mut delta_waits {
                if r.wait_class != "Idle" {
                    r.pct_of_body_time = (((r.total_us as f64 / denom) * 100.0) * 100.0).round() / 100.0;
                }
            }
            tm
        };
        delta_waits.sort_by(|a, b| b.total_us.cmp(&a.total_us));

        let rss_delta_kb = target_snap.kernel_telemetry.rss_kb as i64 - base_snap.kernel_telemetry.rss_kb as i64;
        let rss_drift_pct = if base_snap.kernel_telemetry.rss_kb == 0 {
            0.0
        } else {
            (((rss_delta_kb as f64 / base_snap.kernel_telemetry.rss_kb as f64) * 100.0) * 100.0).round() / 100.0
        };
        let fd_delta = target_snap.kernel_telemetry.open_fds as i64 - base_snap.kernel_telemetry.open_fds as i64;
        let voluntary_cs_delta = target_snap.kernel_telemetry.voluntary_ctxt_switches as i64
            - base_snap.kernel_telemetry.voluntary_ctxt_switches as i64;
        let nonvoluntary_cs_delta = target_snap.kernel_telemetry.nonvoluntary_ctxt_switches as i64
            - base_snap.kernel_telemetry.nonvoluntary_ctxt_switches as i64;
        let io_write_bytes_delta = target_snap.kernel_telemetry.io_write_bytes as i64
            - base_snap.kernel_telemetry.io_write_bytes as i64;

        let addm_findings = Self::evaluate_addm_rules(&delta_time_model, &delta_waits, &target_snap.kernel_telemetry);
        let regression_detected = rss_drift_pct > 25.0
            || target_snap.kernel_telemetry.rss_kb > 90_000
            || addm_findings.iter().any(|f| f.severity == "CRITICAL");

        let top_wait_events: Vec<WaitEventSnapshotRow> = delta_waits
            .into_iter()
            .filter(|w| w.wait_class != "Idle")
            .take(6)
            .collect();
        let top_slow_operations: Vec<OperationStatRow> =
            target_snap.top_operations.into_iter().take(6).collect();

        let report_id = format!("awr-{}", Local::now().format("%Y%m%d-%H%M%S-%3f"));
        let generated_at = Local::now().format("%Y-%m-%dT%H:%M:%S+08:00").to_string();

        let mut md = String::new();
        writeln!(md, "# 📈 Agent Body 内生 AWR 性能诊断报告 ({})\n", report_id)?;
        writeln!(
            md,
            "- **起始快照 (Begin Snap A)**: `{}` (`{}` · `{}`)",
            base_snap.snapshot_id, base_snap.label, base_snap.captured_at
        )?;
        writeln!(
            md,
            "- **结束快照 (End Snap B)**: `{}` (`{}` · `{}`)",
            target_snap.snapshot_id, target_snap.label, target_snap.captured_at
        )?;
        writeln!(
            md,
            "- **对比模式**: `{}` · **退化门禁**: {}",
            mode,
            if regression_detected { "⚠️ 检测到性能/资源水位退化" } else { "✅ 健康稳态 (0 退化)" }
        )?;
        writeln!(
            md,
            "- **内核 `/proc/self` 差分**: VmRSS `{} → {} KB` ({:+} KB / {:+.1}%) · VmHWM `{} → {} KB` · 句柄 `{} → {} FDs` ({:+}) · 主动切换 `{:+}` · 被动抢占 `{:+}` · SQLite WAL `{} KB`",
            base_snap.kernel_telemetry.rss_kb,
            target_snap.kernel_telemetry.rss_kb,
            rss_delta_kb,
            rss_drift_pct,
            base_snap.kernel_telemetry.hwm_kb,
            target_snap.kernel_telemetry.hwm_kb,
            base_snap.kernel_telemetry.open_fds,
            target_snap.kernel_telemetry.open_fds,
            fd_delta,
            voluntary_cs_delta,
            nonvoluntary_cs_delta,
            target_snap.kernel_telemetry.sqlite_wal_bytes / 1024
        )?;
        writeln!(
            md,
            "- **Body Time 分解 (`{} μs`)**: CPU `{:.1}%` | Lock `{:.1}%` | IO `{:.1}%` | Net `{:.1}%` | IPC `{:.1}%`\n",
            delta_time_model.total_body_time_us,
            delta_time_model.cpu_pct,
            delta_time_model.lock_pct,
            delta_time_model.io_pct,
            delta_time_model.net_pct,
            delta_time_model.ipc_pct
        )?;

        writeln!(md, "## 一、Top 等待与执行事件 (Wait Events)\n")?;
        writeln!(md, "| 事件 | 类别 | 调用次数 | 总耗时 (μs) | 平均 (μs) | 估算 P95 (μs) | 峰值 (μs) | 占比 |")?;
        writeln!(md, "|---|---|---:|---:|---:|---:|---:|---:|")?;
        for w in &top_wait_events {
            writeln!(
                md,
                "| `{}` | {} | {} | {} | {:.1} | {} | {} | **{:.1}%** |",
                w.event_name,
                w.wait_class,
                w.calls,
                w.total_us,
                w.avg_us,
                w.est_p95_us,
                w.max_us,
                w.pct_of_body_time
            )?;
        }

        writeln!(md, "\n## 二、ADDM 自动根因诊断与调优处方\n")?;
        for f in &addm_findings {
            writeln!(
                md,
                "- **[{} | {} | {}]** {} — *证据*: {} → **处方**: {}",
                f.rule_id, f.severity, f.bottleneck_class, f.summary, f.evidence, f.recommendation
            )?;
        }

        let report = AwrDiffReport {
            report_id,
            body_id: compute_body_id(),
            mode,
            base_snapshot_id: base_snap.snapshot_id,
            base_label: base_snap.label,
            base_captured_at: base_snap.captured_at,
            target_snapshot_id: target_snap.snapshot_id,
            target_label: target_snap.label,
            target_captured_at: target_snap.captured_at,
            generated_at,
            regression_detected,
            rss_delta_kb,
            rss_drift_pct,
            fd_delta,
            voluntary_cs_delta,
            nonvoluntary_cs_delta,
            io_write_bytes_delta,
            delta_time_model,
            top_wait_events,
            top_slow_operations,
            addm_findings,
            base_kernel: base_snap.kernel_telemetry,
            target_kernel: target_snap.kernel_telemetry,
            markdown_summary: md,
        };
        if persist {
            if let Err(e) = Self::save_awr_report(provider.as_ref(), &report) {
                eprintln!("[AWR][WARN] 保存历史 AWR 报告失败: {}", e);
            }
        }

        Ok(report)
    }

    /// ADDM 5 大确定性瓶颈诊断规则引擎
    pub fn evaluate_addm_rules(
        tm: &BodyTimeModel,
        waits: &[WaitEventSnapshotRow],
        kernel: &KernelSelfTelemetry,
    ) -> Vec<AddmFinding> {
        let mut findings = Vec::new();

        // ADDM-01: Lock 争用
        let sqlite_lock = waits.iter().find(|w| w.event_name == "Lock::SqliteMutex");
        let lock_max = sqlite_lock.map(|w| w.max_us).unwrap_or(0);
        if tm.lock_pct > 20.0 || lock_max > 50_000 {
            findings.push(AddmFinding {
                rule_id: "ADDM-01".to_string(),
                severity: if tm.lock_pct > 40.0 { "CRITICAL" } else { "WARN" }.to_string(),
                bottleneck_class: "Lock Contention".to_string(),
                summary: "SQLite/DuckDB 互斥锁等待占比偏高".to_string(),
                evidence: format!("Lock 等待占 Body Time {:.1}%，单次最大锁等待 {} μs", tm.lock_pct, lock_max),
                recommendation: "执行 `perf_tune` 触发 WAL 检查点，并将高频写入收敛为单事务批处理".to_string(),
                auto_tunable_l1: true,
            });
        }

        // ADDM-02: 进程 RSS 内存水位
        if kernel.rss_kb > 75_000 {
            findings.push(AddmFinding {
                rule_id: "ADDM-02".to_string(),
                severity: if kernel.rss_kb > 90_000 { "WARN" } else { "INFO" }.to_string(),
                bottleneck_class: "Memory Watermark".to_string(),
                summary: "进程驻留内存 (VmRSS) 接近或高于 75MB 警戒线".to_string(),
                evidence: format!("当前 VmRSS = {} KB ({:.1} MB), VmHWM = {} KB", kernel.rss_kb, kernel.rss_kb as f64 / 1024.0, kernel.hwm_kb),
                recommendation: "执行 `perf_tune` 触发 SQLite `PRAGMA shrink_memory`、流式分批对账与 `malloc_trim(0)` 释放空闲堆页".to_string(),
                auto_tunable_l1: true,
            });
        }

        // ADDM-03: SQLite WAL 体积或读取延迟
        let sqlite_read_p95 = waits
            .iter()
            .find(|w| w.event_name == "IO::SqliteRead")
            .map(|w| w.est_p95_us)
            .unwrap_or(0);
        if kernel.sqlite_wal_bytes > 16 * 1024 * 1024 || sqlite_read_p95 > 10_000 {
            findings.push(AddmFinding {
                rule_id: "ADDM-03".to_string(),
                severity: "WARN".to_string(),
                bottleneck_class: "Storage IO / WAL".to_string(),
                summary: "SQLite WAL 日志累积或查询 P95 偏高".to_string(),
                evidence: format!(
                    "WAL 体积 = {} KB, IO::SqliteRead P95 = {} μs",
                    kernel.sqlite_wal_bytes / 1024,
                    sqlite_read_p95
                ),
                recommendation: "执行 `perf_tune` 运行 `PRAGMA optimize` 与 `PRAGMA wal_checkpoint(PASSIVE)`".to_string(),
                auto_tunable_l1: true,
            });
        }

        // ADDM-04: FD / inotify 句柄水位
        if kernel.open_fds > 1024 {
            findings.push(AddmFinding {
                rule_id: "ADDM-04".to_string(),
                severity: "WARN".to_string(),
                bottleneck_class: "File Descriptors".to_string(),
                summary: "进程打开文件/inotify 句柄数偏高".to_string(),
                evidence: format!("当前 open_fds = {} (阈值 1024)", kernel.open_fds),
                recommendation: "检查 FileWatcher 监听目录黑名单剪枝规则，避免递归监听构建缓存目录".to_string(),
                auto_tunable_l1: false,
            });
        }

        // ADDM-05: 本地 Ollama 网络推理延迟
        if tm.net_pct > 40.0 {
            findings.push(AddmFinding {
                rule_id: "ADDM-05".to_string(),
                severity: "INFO".to_string(),
                bottleneck_class: "Local LLM Network IO".to_string(),
                summary: "本地 Ollama 向量/标注 HTTP 推理占主导耗时".to_string(),
                evidence: format!("Net 等待占 Body Time {:.1}%", tm.net_pct),
                recommendation: "保持后台小批量异步回填，前台检索优先走 SQLite FTS5 + 已缓存向量 RRF".to_string(),
                auto_tunable_l1: false,
            });
        }

        if findings.is_empty() {
            findings.push(AddmFinding {
                rule_id: "ADDM-00".to_string(),
                severity: "INFO".to_string(),
                bottleneck_class: "Healthy".to_string(),
                summary: "全部等待事件、锁争用、FD 句柄与内存水位均处于黄金 SLA 包络内".to_string(),
                evidence: format!(
                    "VmRSS={} KB, open_fds={}, Lock={:.1}%, IO={:.1}%, CPU={:.1}%",
                    kernel.rss_kb, kernel.open_fds, tm.lock_pct, tm.io_pct, tm.cpu_pct
                ),
                recommendation: "无需干预，建议将当前稳态快照设为 Golden Baseline (`set_baseline=true`)".to_string(),
                auto_tunable_l1: false,
            });
        }

        findings
    }

    /// 在 `L1_AUTONOMIC` 权限包络内执行一键性能自优化 (`perf_tune`)
    pub fn run_l1_auto_tune(storage: &Storage) -> Result<PerfTuneReport> {
        let env_chk = crate::body::AutonomyEnvelope::evaluate("perf_tune", None, false);
        if !env_chk.allowed {
            anyhow::bail!("AutonomyEnvelope 拒绝执行 perf_tune: {}", env_chk.reason);
        }
        let t0 = Instant::now();
        let before_kernel = KernelSelfTelemetry::collect(None);
        let mut actions = Vec::new();
        let provider_name = storage.provider().provider_name();

        // 1. SQLite 优化器统计刷新 + WAL 检查点 + 连接缓存释放（非 SQLite 后端明确跳过 PRAGMA）
        if provider_name == "sqlite" {
            if let Err(e) = storage.provider().get_benchmark_run(META_PRAGMA_OPTIMIZE) {
                eprintln!("[PerfTune][WARN] 执行 SQLite PRAGMA 调优失败: {}", e);
            }
            actions.push("SQLite 执行 PRAGMA optimize + PRAGMA wal_checkpoint(PASSIVE) + PRAGMA shrink_memory".to_string());
        } else {
            actions.push(format!("当前存储后端为 `{}`，跳过 SQLite 专属 PRAGMA 调优", provider_name));
        }

        // 2. 过期协作锁回收
        let coord = crate::coordination::CoordinationEngine::new(storage.provider().clone());
        let expired_locks_cleaned = coord.gc_expired_locks().unwrap_or(0);
        actions.push(format!("回收过期协作锁 {} 把", expired_locks_cleaned));

        // 3. 流式新鲜度对账（清理失活资产与幽灵节点）
        let rec_rep = crate::feedback::probes::FreshnessReconciler::reconcile(storage).unwrap_or_default();
        actions.push(format!(
            "流式新鲜度对账: 检查 {} 资产, 标记失活 {}, 剪枝幽灵节点 {}",
            rec_rep.checked_assets, rec_rep.missing_assets_marked, rec_rep.ghost_fact_nodes_pruned
        ));

        // 4. SQLite 再次收缩缓存；Linux glibc 调用 malloc_trim(0) 归还物理内存页给 OS
        if provider_name == "sqlite" {
            if let Err(e) = storage.provider().get_benchmark_run(META_PRAGMA_OPTIMIZE) {
                eprintln!("[PerfTune][WARN] 二次收缩 SQLite 内存失败: {}", e);
            }
        }
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            unsafe extern "C" {
                fn malloc_trim(pad: usize) -> i32;
            }
            unsafe {
                let _ = malloc_trim(0);
            }
            actions.push("调用 Linux glibc malloc_trim(0) 向内核归还空闲堆内存页".to_string());
        }
        #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
        {
            actions.push("当前平台非 Linux glibc，跳过 malloc_trim(0)".to_string());
        }

        let after_kernel = KernelSelfTelemetry::collect(None);
        let rss_freed_kb = before_kernel.rss_kb as i64 - after_kernel.rss_kb as i64;
        let elapsed_us = t0.elapsed().as_micros() as u64;

        let report = PerfTuneReport {
            body_id: compute_body_id(),
            tuned_at: Local::now().format("%Y-%m-%dT%H:%M:%S+08:00").to_string(),
            before_rss_kb: before_kernel.rss_kb,
            after_rss_kb: after_kernel.rss_kb,
            rss_freed_kb,
            before_wal_bytes: before_kernel.sqlite_wal_bytes,
            after_wal_bytes: after_kernel.sqlite_wal_bytes,
            reconciled_assets_checked: rec_rep.checked_assets,
            reconciled_missing_marked: rec_rep.missing_assets_marked,
            expired_locks_cleaned,
            actions_executed: actions,
            elapsed_us,
        };

        AuditLogger::record_best_effort(
            storage,
            AuditEventInput::out_of_agent(
                "body_perf_tuner",
                "body",
                "l1_autonomic_perf_tune",
                "perf_tune",
            )
            .with_target("body", &report.body_id)
            .with_outcome("success")
            .with_elapsed_us(elapsed_us as i64)
            .with_delta(serde_json::json!({
                "before_rss_kb": report.before_rss_kb,
                "after_rss_kb": report.after_rss_kb,
                "rss_freed_kb": report.rss_freed_kb,
                "before_wal_bytes": report.before_wal_bytes,
                "after_wal_bytes": report.after_wal_bytes,
            })),
        );

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wait_event_guard_sub_100ns_overhead_and_histogram() {
        let iters = 1000u64;
        let t0 = Instant::now();
        for _ in 0..iters {
            let _g = WaitEventGuard::enter(WaitEventId::CpuBriefAssemble);
        }
        let elapsed_ns = t0.elapsed().as_nanos() as u64;
        let per_call_ns = elapsed_ns / iters;
        assert!(
            per_call_ns < 2_000,
            "WaitEventGuard 单次开销应极低，实测 {} ns",
            per_call_ns
        );

        WaitEventRegistry::record_us(WaitEventId::LockSqliteMutex, 120);
        WaitEventRegistry::record_us(WaitEventId::IoSqliteRead, 45);
        let rows = WaitEventRegistry::snapshot_all();
        assert_eq!(rows.len(), WAIT_EVENT_COUNT);
        let lock_row = rows
            .iter()
            .find(|r| r.event_name == "Lock::SqliteMutex")
            .expect("lock row");
        assert!(lock_row.calls >= 1);
        assert!(lock_row.total_us >= 120);
    }

    #[test]
    fn test_awr_snapshot_baseline_diff_and_addm_tune() {
        let storage = Storage::open_in_memory().expect("in_memory");
        WaitEventRegistry::record_us(WaitEventId::IoSqliteRead, 300);

        let snap_base = BodyPerfEngine::capture_snapshot(&storage, Some("golden-v1"), true)
            .expect("capture base");
        assert!(snap_base.is_baseline);
        assert!(snap_base.kernel_telemetry.pid > 0);

        WaitEventRegistry::record_us(WaitEventId::CpuLinkRegex, 850);
        let snap_curr = BodyPerfEngine::capture_snapshot(&storage, Some("after-load"), false)
            .expect("capture curr");

        let diff = BodyPerfEngine::generate_awr_report(
            &storage,
            Some(&snap_base.snapshot_id),
            Some(&snap_curr.snapshot_id),
            true,
        )
        .expect("awr diff");
        assert_eq!(diff.base_snapshot_id, snap_base.snapshot_id);
        assert_eq!(diff.target_snapshot_id, snap_curr.snapshot_id);
        assert!(!diff.addm_findings.is_empty());
        assert!(diff.markdown_summary.contains("Agent Body 内生 AWR 性能诊断报告"));

        let tune = BodyPerfEngine::run_l1_auto_tune(&storage).expect("auto tune");
        assert!(!tune.actions_executed.is_empty());
    }

    #[test]
    fn test_awr_snapshot_and_report_eviction_deletes_orphan_rows() {
        let storage = Storage::open_in_memory().expect("in_memory");
        let base_snap = BodyPerfEngine::capture_snapshot(&storage, Some("golden-evict"), true)
            .expect("capture base");
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second_snap = BodyPerfEngine::capture_snapshot(&storage, Some("evict-candidate"), false)
            .expect("capture second");

        for i in 0..MAX_RETAINED_SNAPSHOTS {
            std::thread::sleep(std::time::Duration::from_millis(2));
            let label = format!("roll-{}", i);
            BodyPerfEngine::capture_snapshot(&storage, Some(&label), false).expect("roll snap");
        }

        // 黄金基线虽移出 32 滚动窗口但受豁免保留，第 2 份普通淘汰快照应被物理删除
        assert!(
            BodyPerfEngine::load_snapshot(storage.provider().as_ref(), "baseline").is_some(),
            "黄金基线不应被淘汰清理"
        );
        assert!(
            storage
                .get_benchmark_run(&format!("awr:snap:{}", base_snap.snapshot_id))
                .expect("query base")
                .is_some()
        );
        assert!(
            storage
                .get_benchmark_run(&format!("awr:snap:{}", second_snap.snapshot_id))
                .expect("query second")
                .is_none(),
            "超窗淘汰快照 awr:snap:* 必须从 benchmark_runs 删除"
        );

        // 验证报告 20 份滚动淘汰同步物理清理 awr:report:*
        let mut generated_ids = Vec::new();
        for _ in 0..(MAX_RETAINED_REPORTS + 2) {
            std::thread::sleep(std::time::Duration::from_millis(2));
            let rep = BodyPerfEngine::generate_awr_report(&storage, None, None, false).expect("gen");
            generated_ids.push(rep.report_id);
        }
        assert!(
            storage
                .get_benchmark_run(&format!("awr:report:{}", generated_ids[0]))
                .expect("query evicted rep 0")
                .is_none(),
            "最早生成的淘汰 AWR 报告必须已删除"
        );
        assert!(
            storage
                .get_benchmark_run(&format!("awr:report:{}", generated_ids[1]))
                .expect("query evicted rep 1")
                .is_none(),
            "第 2 早生成的淘汰 AWR 报告必须已删除"
        );
        assert!(
            storage
                .get_benchmark_run(&format!(
                    "awr:report:{}",
                    generated_ids[generated_ids.len() - 1]
                ))
                .expect("query latest rep")
                .is_some()
        );
    }

    #[test]
    fn test_percentile_never_exceeds_max_addm05_and_delete_apis() {
        // 1. Tiny max_us=5 must clamp est_p95_us <= max_us
        let mut buckets = [0u64; 8];
        buckets[0] = 100;
        let est = estimate_percentile_from_buckets(&buckets, 0.95, 5);
        assert!(est <= 5, "est_p95_us ({}) must be <= max_us (5)", est);

        // 2. ADDM-05 triggers cleanly when net_pct > 40%
        let tm = BodyTimeModel {
            total_body_time_us: 100_000,
            net_wait_us: 55_000,
            net_pct: 55.0,
            ..Default::default()
        };
        let findings = BodyPerfEngine::evaluate_addm_rules(&tm, &[], &KernelSelfTelemetry::default());
        assert!(findings.iter().any(|f| f.rule_id == "ADDM-05"));

        // 3. delete_snapshot & delete_awr_report remove both rows and indices
        let storage = Storage::open_in_memory().expect("in_memory");
        let snap = BodyPerfEngine::capture_snapshot(&storage, Some("del-snap"), false).expect("snap");
        let rep = BodyPerfEngine::generate_awr_report(&storage, None, Some(&snap.snapshot_id), false)
            .expect("rep");
        assert!(BodyPerfEngine::load_awr_report(storage.provider().as_ref(), &rep.report_id).is_some());
        assert!(BodyPerfEngine::delete_awr_report(&storage, &rep.report_id).expect("del rep"));
        assert!(BodyPerfEngine::load_awr_report(storage.provider().as_ref(), &rep.report_id).is_none());
        assert!(BodyPerfEngine::list_awr_reports_meta(storage.provider().as_ref())
            .iter()
            .all(|r| r.report_id != rep.report_id));

        assert!(BodyPerfEngine::delete_snapshot(&storage, &snap.snapshot_id).expect("del snap"));
        assert!(BodyPerfEngine::load_snapshot(storage.provider().as_ref(), &snap.snapshot_id).is_none());
        assert!(BodyPerfEngine::list_snapshot_ids(storage.provider().as_ref())
            .0
            .iter()
            .all(|id| id != &snap.snapshot_id));
    }

    #[test]
    fn test_kernel_self_telemetry_sysinfo_fallback() {
        let t = KernelSelfTelemetry::collect_via_sysinfo_raw();
        assert_eq!(t.pid, std::process::id());
        assert!(t.rss_kb > 0, "sysinfo 回退必须采集到当前进程 RSS > 0");
    }
}
