use std::fmt::Write as _;

use anyhow::Result;
use chrono::Local;

use super::wait_events::*;
use crate::audit::{AuditEventInput, AuditLogger};
use crate::body::manifest::compute_body_id;
use crate::body::vitals::HomeostasisVitalsEngine;
use crate::storage::{Storage, StorageProvider};


pub use super::addm_tuner::*;

const META_SNAP_INDEX: &str = "awr:index";
const META_REPORT_INDEX: &str = "awr:report_index";
const META_BASELINE_KEY: &str = "awr:baseline";
const META_LATEST_SNAP_KEY: &str = "awr:latest";
pub(super) const META_PRAGMA_OPTIMIZE: &str = "awr:pragma_optimize";
pub(super) const MAX_RETAINED_SNAPSHOTS: usize = 32;
pub(super) const MAX_RETAINED_REPORTS: usize = 32;

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
            total_body_tokens: 0,
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
        rows.sort_by_key(|a| std::cmp::Reverse(a.total_us));
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
        if (raw == "baseline"
            || baseline_snap
                .as_ref()
                .map(|b| b.snapshot_id == resolved_id)
                .unwrap_or(false))
            && provider.delete_benchmark_run(META_BASELINE_KEY)? {
                deleted = true;
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
        delta_waits.sort_by_key(|a| std::cmp::Reverse(a.total_us));

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

}
