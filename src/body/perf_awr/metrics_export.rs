//! 旗舰版 Prometheus / OpenMetrics 0.0.4 文本指标导出器（零外部重型依赖）

use std::collections::BTreeMap;
use std::fmt::{Arguments, Write as _};

use super::wait_events::{
    KernelSelfTelemetry, WaitEventRegistry, LATENCY_BUCKET_BOUNDS_US,
};
use crate::body::vitals::HomeostasisVitalsEngine;
use crate::storage::StorageProvider;

fn escape_label(raw: &str) -> String {
    raw.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

#[inline]
fn append_line(out: &mut String, args: Arguments<'_>) {
    if out.write_fmt(args).is_err() {
        out.clear();
    }
    out.push('\n');
}

/// 聚合 18 种 `WaitEventId` 直方图、进程资源画像、活跃协作锁与 24h 审计汇总并渲染 OpenMetrics 0.0.4 文本
pub fn render_flagship_prometheus_metrics(storage: &dyn StorageProvider, uptime_secs: f64) -> String {
    let mut out = String::with_capacity(8192);

    append_line(&mut out, format_args!("# HELP agent_body_uptime_seconds Agent Body Web/Daemon uptime in seconds."));
    append_line(&mut out, format_args!("# TYPE agent_body_uptime_seconds gauge"));
    append_line(&mut out, format_args!("agent_body_uptime_seconds {:.6}", uptime_secs.max(0.0)));

    let telemetry = KernelSelfTelemetry::collect(None);
    append_line(&mut out, format_args!("# HELP agent_body_process_rss_bytes Resident set size of agent-body process in bytes."));
    append_line(&mut out, format_args!("# TYPE agent_body_process_rss_bytes gauge"));
    append_line(&mut out, format_args!("agent_body_process_rss_bytes {}", telemetry.rss_kb.saturating_mul(1024)));

    append_line(&mut out, format_args!("# HELP agent_body_process_hwm_bytes Peak resident set size high-water mark in bytes."));
    append_line(&mut out, format_args!("# TYPE agent_body_process_hwm_bytes gauge"));
    append_line(&mut out, format_args!("agent_body_process_hwm_bytes {}", telemetry.hwm_kb.saturating_mul(1024)));

    append_line(&mut out, format_args!("# HELP agent_body_process_open_fds Open file descriptors of current process."));
    append_line(&mut out, format_args!("# TYPE agent_body_process_open_fds gauge"));
    append_line(&mut out, format_args!("agent_body_process_open_fds {}", telemetry.open_fds));

    append_line(&mut out, format_args!("# HELP agent_body_sqlite_db_bytes Primary SQLite database file size in bytes."));
    append_line(&mut out, format_args!("# TYPE agent_body_sqlite_db_bytes gauge"));
    append_line(&mut out, format_args!("agent_body_sqlite_db_bytes {}", telemetry.sqlite_db_bytes));

    append_line(&mut out, format_args!("# HELP agent_body_sqlite_wal_bytes Primary SQLite WAL file size in bytes."));
    append_line(&mut out, format_args!("# TYPE agent_body_sqlite_wal_bytes gauge"));
    append_line(&mut out, format_args!("agent_body_sqlite_wal_bytes {}", telemetry.sqlite_wal_bytes));

    let active_locks = storage.list_coordination_locks(None).map(|v| v.len()).unwrap_or(0);
    append_line(&mut out, format_args!("# HELP agent_body_coordination_locks_active Currently held multi-agent coordination locks."));
    append_line(&mut out, format_args!("# TYPE agent_body_coordination_locks_active gauge"));
    append_line(&mut out, format_args!("agent_body_coordination_locks_active {active_locks}"));

    let vitals = HomeostasisVitalsEngine::assess_provider(storage);
    append_line(&mut out, format_args!("# HELP agent_body_homeostasis_score Five-dimension body homeostasis score (0-100)."));
    append_line(&mut out, format_args!("# TYPE agent_body_homeostasis_score gauge"));
    append_line(&mut out, format_args!("agent_body_homeostasis_score {:.2}", vitals.overall_score));

    let audit_sum = storage.audit_domain_summary("24h").unwrap_or_default();
    let mut by_domain_outcome: BTreeMap<(String, String), usize> = BTreeMap::new();
    for item in &audit_sum.breakdown {
        let key = (item.domain.clone(), item.outcome.clone());
        let entry = by_domain_outcome.entry(key).or_insert(0);
        *entry = entry.saturating_add(item.event_count);
    }
    append_line(&mut out, format_args!("# HELP agent_body_audit_events_24h Audit events aggregated over trailing 24 hours."));
    append_line(&mut out, format_args!("# TYPE agent_body_audit_events_24h gauge"));
    if by_domain_outcome.is_empty() {
        append_line(
            &mut out,
            format_args!(
                "agent_body_audit_events_24h{{domain=\"ALL\",outcome=\"total\"}} {}",
                audit_sum.total_events
            ),
        );
    } else {
        for ((domain, outcome), count) in by_domain_outcome {
            append_line(
                &mut out,
                format_args!(
                    "agent_body_audit_events_24h{{domain=\"{}\",outcome=\"{}\"}} {}",
                    escape_label(&domain),
                    escape_label(&outcome),
                    count
                ),
            );
        }
    }

    let waits = WaitEventRegistry::snapshot_all();
    append_line(&mut out, format_args!("# HELP agent_body_wait_event_duration_seconds Cumulative wait event duration histogram across 18 WaitEventId slots."));
    append_line(&mut out, format_args!("# TYPE agent_body_wait_event_duration_seconds histogram"));
    for w in waits {
        let wc = escape_label(&w.wait_class);
        let ev = escape_label(&w.event_name);
        let mut cumulative = 0u64;
        for (idx, count) in w.histogram_buckets.iter().enumerate() {
            cumulative = cumulative.saturating_add(*count);
            let le = match LATENCY_BUCKET_BOUNDS_US[idx] {
                100 => "0.0001".to_string(),
                500 => "0.0005".to_string(),
                2_000 => "0.002".to_string(),
                10_000 => "0.01".to_string(),
                50_000 => "0.05".to_string(),
                250_000 => "0.25".to_string(),
                1_000_000 => "1".to_string(),
                u64::MAX => "+Inf".to_string(),
                other => format!("{:.6}", other as f64 / 1_000_000.0),
            };
            append_line(
                &mut out,
                format_args!(
                    "agent_body_wait_event_duration_seconds_bucket{{wait_class=\"{wc}\",event=\"{ev}\",le=\"{le}\"}} {cumulative}"
                ),
            );
        }
        append_line(
            &mut out,
            format_args!(
                "agent_body_wait_event_duration_seconds_sum{{wait_class=\"{wc}\",event=\"{ev}\"}} {:.6}",
                w.total_us as f64 / 1_000_000.0
            ),
        );
        append_line(
            &mut out,
            format_args!(
                "agent_body_wait_event_duration_seconds_count{{wait_class=\"{wc}\",event=\"{ev}\"}} {}",
                w.calls
            ),
        );
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::wait_events::{WaitEventId, WAIT_EVENT_COUNT};
    use crate::storage::{SqliteStorageProvider, StorageProvider};

    #[test]
    fn flagship_prometheus_exports_18_wait_events_with_monotonic_buckets() {
        WaitEventRegistry::record_us(WaitEventId::IoSqliteRead, 45);
        WaitEventRegistry::record_us(WaitEventId::IoSqliteRead, 1_500);
        WaitEventRegistry::record_us(WaitEventId::CpuBriefAssemble, 80_000);

        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite mem");
        storage.init().expect("init");

        let txt = render_flagship_prometheus_metrics(&storage, 12.5);
        assert!(txt.contains("# TYPE agent_body_wait_event_duration_seconds histogram"));
        assert!(txt.contains("agent_body_uptime_seconds 12.500000"));
        assert!(txt.contains("agent_body_homeostasis_score "));

        for ev in WaitEventId::ALL {
            assert!(
                txt.contains(&format!("event=\"{}\"", ev.event_name())),
                "missing wait event {}",
                ev.event_name()
            );
        }

        let mut bucket_series: std::collections::BTreeMap<String, Vec<(String, u64)>> =
            std::collections::BTreeMap::new();
        let mut count_series: std::collections::BTreeMap<String, u64> =
            std::collections::BTreeMap::new();
        for line in txt.lines() {
            if let Some(rest) = line.strip_prefix("agent_body_wait_event_duration_seconds_bucket{") {
                let (labels, val_raw) = rest.rsplit_once("} ").expect("bucket line");
                let val: u64 = val_raw.trim().parse().expect("u64");
                let (key, le) = labels
                    .rsplit_once(",le=\"")
                    .map(|(k, l)| (k.to_string(), l.trim_end_matches('"').to_string()))
                    .expect("le");
                bucket_series.entry(key).or_default().push((le, val));
            } else if let Some(rest) =
                line.strip_prefix("agent_body_wait_event_duration_seconds_count{")
            {
                let (labels, val_raw) = rest.rsplit_once("} ").expect("count line");
                let val: u64 = val_raw.trim().parse().expect("u64");
                count_series.insert(labels.to_string(), val);
            }
        }
        assert_eq!(bucket_series.len(), WAIT_EVENT_COUNT);
        for (series, rows) in &bucket_series {
            assert_eq!(rows.len(), 8);
            let mut prev = 0u64;
            for (_, v) in rows {
                assert!(*v >= prev, "non-monotonic bucket for {series}");
                prev = *v;
            }
            assert_eq!(rows.last().map(|r| (r.0.as_str(), r.1)), Some(("+Inf", count_series[series])));
        }
    }
}
