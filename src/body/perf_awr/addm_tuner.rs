use std::time::Instant;
use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};

use super::awr_engine::{BodyPerfEngine, META_PRAGMA_OPTIMIZE};
use super::wait_events::*;
use crate::audit::{AuditEventInput, AuditLogger};
use crate::body::manifest::compute_body_id;
use crate::storage::Storage;

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


impl BodyPerfEngine {
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
                let _trimmed = malloc_trim(0);
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
