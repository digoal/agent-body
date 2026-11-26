//! 本体等待事件 (`WaitEventRegistry`)、`/proc/self` 内核探针、AWR 快照基线差分与 ADDM 自优化引擎
//!
//! 对应设计规格书: `docs/design_doc/18-wait-events-and-awr-perf-diagnostics-spec.md` (Phase 50 · ISSUE #32)
//! 对齐规约: #2 (递弱代偿), #8 (L1_AUTONOMIC 自调优), #9 (<100ns 极速探针 & <90MB 内存), #16 (黑白屏 1:1:1), #26 (双域审计), #27 (本体稳态)

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};

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
        rows.sort_by_key(|a| std::cmp::Reverse(a.total_us));
        rows
    }
}

pub(super) fn estimate_percentile_from_buckets(buckets: &[u64; 8], quantile: f64, max_us: u64) -> u64 {
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
