//! 内核纯无锁原子耗时直方图 (`KernelWaitRegistry`)，仅使用 `std::sync::atomic`

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

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

/// 微内核机制层标准等待事件枚举（领域与扩展无关）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(usize)]
pub enum KernelWaitEventId {
    CatalogRead = 0,
    CatalogWrite = 1,
    TableAmRead = 2,
    TableAmWrite = 3,
    IndexScan = 4,
    FdwProbe = 5,
    HookDispatch = 6,
    LeaseCas = 7,
    AuditEmit = 8,
    BgWorkerTick = 9,
    SandboxIpc = 10,
}

pub const KERNEL_WAIT_EVENT_COUNT: usize = 11;

impl KernelWaitEventId {
    pub const ALL: [KernelWaitEventId; KERNEL_WAIT_EVENT_COUNT] = [
        Self::CatalogRead,
        Self::CatalogWrite,
        Self::TableAmRead,
        Self::TableAmWrite,
        Self::IndexScan,
        Self::FdwProbe,
        Self::HookDispatch,
        Self::LeaseCas,
        Self::AuditEmit,
        Self::BgWorkerTick,
        Self::SandboxIpc,
    ];

    #[inline]
    pub const fn event_name(&self) -> &'static str {
        match self {
            Self::CatalogRead => "Kernel::CatalogRead",
            Self::CatalogWrite => "Kernel::CatalogWrite",
            Self::TableAmRead => "Kernel::TableAmRead",
            Self::TableAmWrite => "Kernel::TableAmWrite",
            Self::IndexScan => "Kernel::IndexScan",
            Self::FdwProbe => "Kernel::FdwProbe",
            Self::HookDispatch => "Kernel::HookDispatch",
            Self::LeaseCas => "Kernel::LeaseCas",
            Self::AuditEmit => "Kernel::AuditEmit",
            Self::BgWorkerTick => "Kernel::BgWorkerTick",
            Self::SandboxIpc => "Kernel::SandboxIpc",
        }
    }
}

struct AtomicLatencySlot {
    calls: AtomicU64,
    total_us: AtomicU64,
    max_us: AtomicU64,
    buckets: [AtomicU64; 8],
}

impl AtomicLatencySlot {
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
        let b_idx = bucket_index(elapsed_us);
        self.buckets[b_idx].fetch_add(1, Ordering::Relaxed);
    }

    fn reset(&self) {
        self.calls.store(0, Ordering::Relaxed);
        self.total_us.store(0, Ordering::Relaxed);
        self.max_us.store(0, Ordering::Relaxed);
        for b in &self.buckets {
            b.store(0, Ordering::Relaxed);
        }
    }
}

#[inline]
const fn bucket_index(elapsed_us: u64) -> usize {
    if elapsed_us < 100 {
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
    }
}

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

/// 纯无锁原子耗时直方图注册表
pub struct KernelWaitRegistry {
    slots: [AtomicLatencySlot; KERNEL_WAIT_EVENT_COUNT],
}

static GLOBAL_KERNEL_WAITS: KernelWaitRegistry = KernelWaitRegistry::new();

impl KernelWaitRegistry {
    pub const fn new() -> Self {
        #[allow(clippy::declare_interior_mutable_const)]
        const EMPTY_SLOT: AtomicLatencySlot = AtomicLatencySlot::new();
        Self {
            slots: [EMPTY_SLOT; KERNEL_WAIT_EVENT_COUNT],
        }
    }

    pub fn global() -> &'static Self {
        &GLOBAL_KERNEL_WAITS
    }

    #[inline]
    pub fn record_us(&self, event: KernelWaitEventId, elapsed_us: u64) {
        self.slots[event as usize].record(elapsed_us);
    }

    pub fn enter(&self, event: KernelWaitEventId) -> KernelWaitGuard<'_> {
        KernelWaitGuard {
            registry: self,
            event,
            start: Instant::now(),
        }
    }

    pub fn reset_all(&self) {
        for slot in &self.slots {
            slot.reset();
        }
    }

    pub fn snapshot_all(&self) -> Vec<KernelWaitSnapshotRow> {
        let mut rows = Vec::with_capacity(KERNEL_WAIT_EVENT_COUNT);
        for ev in KernelWaitEventId::ALL {
            let slot = &self.slots[ev as usize];
            let calls = slot.calls.load(Ordering::Relaxed);
            let total_us = slot.total_us.load(Ordering::Relaxed);
            let max_us = slot.max_us.load(Ordering::Relaxed);
            let mut buckets = [0u64; 8];
            for (i, b) in slot.buckets.iter().enumerate() {
                buckets[i] = b.load(Ordering::Relaxed);
            }
            let avg_us = if calls == 0 {
                0.0
            } else {
                ((total_us as f64 / calls as f64) * 100.0).round() / 100.0
            };
            let est_p95_us = estimate_p95(&buckets, max_us);
            rows.push(KernelWaitSnapshotRow {
                event_name: ev.event_name().to_string(),
                calls,
                total_us,
                avg_us,
                max_us,
                est_p95_us,
                histogram_buckets: buckets,
            });
        }
        rows
    }
}

impl Default for KernelWaitRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// RAII 无锁等待事件计时守卫
pub struct KernelWaitGuard<'a> {
    registry: &'a KernelWaitRegistry,
    event: KernelWaitEventId,
    start: Instant,
}

impl<'a> Drop for KernelWaitGuard<'a> {
    #[inline]
    fn drop(&mut self) {
        let elapsed_us = self.start.elapsed().as_micros() as u64;
        self.registry.record_us(self.event, elapsed_us);
    }
}

fn estimate_p95(buckets: &[u64; 8], max_us: u64) -> u64 {
    let total: u64 = buckets.iter().sum();
    if total == 0 {
        return 0;
    }
    let threshold = ((total as f64) * 0.95).ceil() as u64;
    let mut cumulative = 0u64;
    for (idx, count) in buckets.iter().enumerate() {
        cumulative = cumulative.saturating_add(*count);
        if cumulative >= threshold {
            let bound = KERNEL_LATENCY_BUCKET_BOUNDS_US[idx];
            return if bound == u64::MAX {
                max_us
            } else {
                bound.min(max_us)
            };
        }
    }
    max_us
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_registry_records_buckets_and_stats_lock_free() {
        let reg = KernelWaitRegistry::new();
        reg.record_us(KernelWaitEventId::CatalogRead, 50);
        reg.record_us(KernelWaitEventId::CatalogRead, 300);
        reg.record_us(KernelWaitEventId::CatalogRead, 15_000);
        {
            let _guard = reg.enter(KernelWaitEventId::HookDispatch);
        }

        let snap = reg.snapshot_all();
        let cat_row = snap
            .iter()
            .find(|r| r.event_name == "Kernel::CatalogRead")
            .unwrap();
        assert_eq!(cat_row.calls, 3);
        assert_eq!(cat_row.total_us, 15_350);
        assert_eq!(cat_row.max_us, 15_000);
        assert_eq!(cat_row.histogram_buckets[0], 1);
        assert_eq!(cat_row.histogram_buckets[1], 1);
        assert_eq!(cat_row.histogram_buckets[4], 1);
        assert!(cat_row.est_p95_us > 0);

        let hook_row = snap
            .iter()
            .find(|r| r.event_name == "Kernel::HookDispatch")
            .unwrap();
        assert_eq!(hook_row.calls, 1);

        reg.reset_all();
        let after = reg.snapshot_all();
        assert!(after.iter().all(|r| r.calls == 0));
    }
}
