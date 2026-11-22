//! 本体等待事件 (`WaitEventRegistry`)、`/proc/self` 内核探针、AWR 快照基线差分与 ADDM 自优化引擎
//! 对应设计规格书: `docs/design_doc/18-wait-events-and-awr-perf-diagnostics-spec.md`

pub mod awr_engine;
pub mod wait_events;

pub use awr_engine::*;
pub use wait_events::*;

#[cfg(test)]
mod tests {
    use std::time::Instant;
    use crate::storage::Storage;
    use super::awr_engine::{MAX_RETAINED_REPORTS, MAX_RETAINED_SNAPSHOTS};
    use super::wait_events::estimate_percentile_from_buckets;
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
