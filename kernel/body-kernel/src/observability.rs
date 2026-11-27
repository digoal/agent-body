//! 微内核纯机制可观测原语 (`KernelStatsRegistry` / `InFlightRegistry` / `SlowOpRing` / `KernelObservability`)
//!
//! 严守零外部监控协议与零业务词依赖，仅使用标准库原子/互斥容器向 `ObservabilityProvider` 暴露只读态。

use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use agent_body_sdk::{
    types::rfc3339_beijing_now, InFlightOpSnapshot, KernelObservabilitySnapshot, KernelOpStatsRow,
    ObservabilityProvider, SlowOpRecord,
};

use crate::wait_events::KernelWaitRegistry;

pub const DEFAULT_SLOW_OP_CAPACITY: usize = 256;
pub const DEFAULT_SLOW_THRESHOLD_US: u64 = 50_000;

#[derive(Debug, Clone, Default)]
struct OpStatAccum {
    calls: u64,
    ok_count: u64,
    error_count: u64,
    denied_count: u64,
    timeout_count: u64,
    total_us: u64,
    max_us: u64,
}

/// 按 `(kind, target)` 分组的操作计数与延迟统计注册表（对标 `pg_stat_statements`）
#[derive(Default)]
pub struct KernelStatsRegistry {
    rows: Mutex<BTreeMap<(String, String), OpStatAccum>>,
}

impl KernelStatsRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, kind: &str, target: &str, outcome: &str, elapsed_us: u64) {
        let Ok(mut guard) = self.rows.lock() else {
            return;
        };
        let entry = guard
            .entry((kind.to_string(), target.to_string()))
            .or_default();
        entry.calls = entry.calls.saturating_add(1);
        entry.total_us = entry.total_us.saturating_add(elapsed_us);
        if elapsed_us > entry.max_us {
            entry.max_us = elapsed_us;
        }
        if is_ok_outcome(outcome) {
            entry.ok_count = entry.ok_count.saturating_add(1);
        } else if is_timeout_outcome(outcome) {
            entry.timeout_count = entry.timeout_count.saturating_add(1);
        } else if is_denied_outcome(outcome) {
            entry.denied_count = entry.denied_count.saturating_add(1);
        } else {
            entry.error_count = entry.error_count.saturating_add(1);
        }
    }

    pub fn snapshot(&self) -> Vec<KernelOpStatsRow> {
        let Ok(guard) = self.rows.lock() else {
            return Vec::new();
        };
        let mut out: Vec<KernelOpStatsRow> = guard
            .iter()
            .map(|((kind, target), acc)| {
                let avg_us = if acc.calls == 0 {
                    0.0
                } else {
                    ((acc.total_us as f64 / acc.calls as f64) * 100.0).round() / 100.0
                };
                KernelOpStatsRow {
                    kind: kind.clone(),
                    target: target.clone(),
                    calls: acc.calls,
                    ok_count: acc.ok_count,
                    error_count: acc.error_count,
                    denied_count: acc.denied_count,
                    timeout_count: acc.timeout_count,
                    total_us: acc.total_us,
                    max_us: acc.max_us,
                    avg_us,
                }
            })
            .collect();
        out.sort_by_key(|r| std::cmp::Reverse(r.total_us));
        out
    }

    pub fn reset(&self) {
        if let Ok(mut guard) = self.rows.lock() {
            guard.clear();
        }
    }
}

struct InFlightEntry {
    trace_id: String,
    kind: String,
    target: String,
    caller_id: String,
    started_at: String,
    started_instant: Instant,
}

/// 活跃执行中请求注册表（对标 `pg_stat_activity`）
#[derive(Default)]
pub struct InFlightRegistry {
    next_id: AtomicU64,
    active: Mutex<BTreeMap<u64, InFlightEntry>>,
}

impl InFlightRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enter(
        &self,
        trace_id: impl Into<String>,
        kind: impl Into<String>,
        target: impl Into<String>,
        caller_id: impl Into<String>,
    ) -> InFlightGuard<'_> {
        let slot_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut guard) = self.active.lock() {
            guard.insert(
                slot_id,
                InFlightEntry {
                    trace_id: trace_id.into(),
                    kind: kind.into(),
                    target: target.into(),
                    caller_id: caller_id.into(),
                    started_at: rfc3339_beijing_now(),
                    started_instant: Instant::now(),
                },
            );
        }
        InFlightGuard {
            registry: self,
            slot_id,
        }
    }

    pub fn snapshot(&self) -> Vec<InFlightOpSnapshot> {
        let Ok(guard) = self.active.lock() else {
            return Vec::new();
        };
        guard
            .values()
            .map(|e| InFlightOpSnapshot {
                trace_id: e.trace_id.clone(),
                kind: e.kind.clone(),
                target: e.target.clone(),
                caller_id: e.caller_id.clone(),
                started_at: e.started_at.clone(),
                elapsed_us: e.started_instant.elapsed().as_micros() as u64,
            })
            .collect()
    }

    fn remove(&self, slot_id: u64) {
        if let Ok(mut guard) = self.active.lock() {
            guard.remove(&slot_id);
        }
    }
}

/// RAII 活跃操作守卫：`Drop` 时自动注销，防止异常分支泄漏
pub struct InFlightGuard<'a> {
    registry: &'a InFlightRegistry,
    slot_id: u64,
}

impl<'a> Drop for InFlightGuard<'a> {
    fn drop(&mut self) {
        self.registry.remove(self.slot_id);
    }
}

/// 有界慢操作/异常环形追踪环（对标 `auto_explain`）
pub struct SlowOpRing {
    capacity: usize,
    threshold_us: AtomicU64,
    ring: Mutex<VecDeque<SlowOpRecord>>,
}

impl SlowOpRing {
    pub fn new(capacity: usize, threshold_us: u64) -> Self {
        Self {
            capacity: capacity.max(1),
            threshold_us: AtomicU64::new(threshold_us),
            ring: Mutex::new(VecDeque::with_capacity(capacity.clamp(1, 256))),
        }
    }

    pub fn threshold_us(&self) -> u64 {
        self.threshold_us.load(Ordering::Relaxed)
    }

    pub fn set_threshold_us(&self, us: u64) {
        self.threshold_us.store(us, Ordering::Relaxed);
    }

    pub fn record(&self, entry: SlowOpRecord) {
        let threshold = self.threshold_us.load(Ordering::Relaxed);
        if is_ok_outcome(&entry.outcome) && entry.elapsed_us < threshold {
            return;
        }
        let Ok(mut guard) = self.ring.lock() else {
            return;
        };
        if guard.len() >= self.capacity {
            guard.pop_front();
        }
        guard.push_back(entry);
    }

    pub fn snapshot(&self) -> Vec<SlowOpRecord> {
        let Ok(guard) = self.ring.lock() else {
            return Vec::new();
        };
        guard.iter().cloned().collect()
    }

    pub fn reset(&self) {
        if let Ok(mut guard) = self.ring.lock() {
            guard.clear();
        }
    }
}

impl Default for SlowOpRing {
    fn default() -> Self {
        Self::new(DEFAULT_SLOW_OP_CAPACITY, DEFAULT_SLOW_THRESHOLD_US)
    }
}

/// 微内核统一可观测注册表，实现 `agent_body_sdk::ObservabilityProvider`
pub struct KernelObservability {
    started_at: Instant,
    waits: Arc<KernelWaitRegistry>,
    stats: KernelStatsRegistry,
    in_flight: InFlightRegistry,
    slow_ops: SlowOpRing,
}

impl KernelObservability {
    pub fn new(waits: Arc<KernelWaitRegistry>) -> Self {
        Self {
            started_at: Instant::now(),
            waits,
            stats: KernelStatsRegistry::new(),
            in_flight: InFlightRegistry::new(),
            slow_ops: SlowOpRing::default(),
        }
    }

    pub fn enter_op(
        &self,
        trace_id: &str,
        kind: &str,
        target: &str,
        caller_id: &str,
    ) -> InFlightGuard<'_> {
        self.in_flight.enter(trace_id, kind, target, caller_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_outcome(
        &self,
        trace_id: &str,
        kind: &str,
        target: &str,
        caller_id: &str,
        outcome: &str,
        elapsed_us: u64,
        detail: Option<String>,
    ) {
        self.stats.record(kind, target, outcome, elapsed_us);
        self.slow_ops.record(SlowOpRecord {
            trace_id: trace_id.to_string(),
            kind: kind.to_string(),
            target: target.to_string(),
            caller_id: caller_id.to_string(),
            outcome: outcome.to_string(),
            elapsed_us,
            detail,
            recorded_at: rfc3339_beijing_now(),
        });
    }

    pub fn slow_threshold_us(&self) -> u64 {
        self.slow_ops.threshold_us()
    }

    pub fn set_slow_threshold_us(&self, us: u64) {
        self.slow_ops.set_threshold_us(us);
    }

    pub fn snapshot(&self) -> KernelObservabilitySnapshot {
        KernelObservabilitySnapshot {
            captured_at: rfc3339_beijing_now(),
            uptime_ms: self.started_at.elapsed().as_millis() as u64,
            slow_threshold_us: self.slow_ops.threshold_us(),
            wait_events: self.waits.snapshot_all(),
            op_stats: self.stats.snapshot(),
            in_flight: self.in_flight.snapshot(),
            slow_ops: self.slow_ops.snapshot(),
        }
    }

    pub fn reset(&self) {
        self.waits.reset_all();
        self.stats.reset();
        self.slow_ops.reset();
    }
}

impl ObservabilityProvider for KernelObservability {
    fn snapshot(&self) -> KernelObservabilitySnapshot {
        KernelObservability::snapshot(self)
    }
}

fn is_ok_outcome(outcome: &str) -> bool {
    outcome == "ok"
}

fn is_timeout_outcome(outcome: &str) -> bool {
    outcome == "timeout"
}

fn is_denied_outcome(outcome: &str) -> bool {
    matches!(
        outcome,
        "denied"
            | "auth_denied"
            | "access_denied"
            | "aopcc_denied"
            | "guard_blocked"
            | "redact_blocked"
            | "blocked"
            | "rejected"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_flight_guard_auto_cleans_on_drop() {
        let reg = InFlightRegistry::new();
        {
            let _g1 = reg.enter("trace-1", "proc", "ping", "alice");
            let _g2 = reg.enter("trace-2", "fdw", "linux_ops", "bob");
            assert_eq!(reg.snapshot().len(), 2);
        }
        assert!(reg.snapshot().is_empty());
    }

    #[test]
    fn slow_ring_evicts_oldest_and_keeps_errors() {
        let ring = SlowOpRing::new(3, 10_000);
        ring.record(SlowOpRecord {
            trace_id: "fast-ok".to_string(),
            kind: "proc".to_string(),
            target: "ping".to_string(),
            caller_id: "c1".to_string(),
            outcome: "ok".to_string(),
            elapsed_us: 200,
            detail: None,
            recorded_at: "t0".to_string(),
        });
        assert!(ring.snapshot().is_empty(), "fast ok call below threshold must be skipped");

        for idx in 1..=4 {
            ring.record(SlowOpRecord {
                trace_id: format!("trace-{idx}"),
                kind: "fdw".to_string(),
                target: "probe".to_string(),
                caller_id: "c1".to_string(),
                outcome: if idx == 4 { "timeout".to_string() } else { "ok".to_string() },
                elapsed_us: if idx == 4 { 500 } else { 20_000 },
                detail: Some(format!("detail-{idx}")),
                recorded_at: format!("t{idx}"),
            });
        }
        let snap = ring.snapshot();
        assert_eq!(snap.len(), 3);
        let ids: Vec<&str> = snap.iter().map(|r| r.trace_id.as_str()).collect();
        assert_eq!(ids, vec!["trace-2", "trace-3", "trace-4"]);
    }

    #[test]
    fn stats_registry_tracks_ok_error_denied_timeout() {
        let waits = Arc::new(KernelWaitRegistry::new());
        let obs = KernelObservability::new(waits);
        obs.record_outcome("t1", "proc", "a", "u", "ok", 100, None);
        obs.record_outcome("t2", "proc", "a", "u", "auth_denied", 50, Some("401".into()));
        obs.record_outcome("t3", "proc", "a", "u", "timeout", 80_000, Some("slow".into()));
        obs.record_outcome("t4", "proc", "a", "u", "error", 200, Some("boom".into()));

        let snap = obs.snapshot();
        let row = snap.op_stats.iter().find(|r| r.target == "a").unwrap();
        assert_eq!(row.calls, 4);
        assert_eq!(row.ok_count, 1);
        assert_eq!(row.denied_count, 1);
        assert_eq!(row.timeout_count, 1);
        assert_eq!(row.error_count, 1);
        assert_eq!(row.total_us, 80_350);
        assert_eq!(row.max_us, 80_000);
        assert_eq!(snap.slow_ops.len(), 3);
    }
}
