pub mod dq;
pub mod ledger;
pub mod micro;
pub mod provenance;
pub mod report;
pub mod workloads;

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;

use crate::benchmark::dq::run_data_quality_checks;
use crate::benchmark::micro::{run_all_micro_benchmarks, MicroBenchResult};
use crate::benchmark::report::{BenchmarkReportPayload, DataQualityResult};
use crate::benchmark::workloads::{run_all_workloads, run_workload, WorkloadResult};
use crate::storage::models::BenchmarkRunRecord;
use crate::storage::StorageProvider;

pub const BENCHMARK_SPEC_VERSION: &str = "2.0";

pub fn compute_geomean_speedup(workloads: &[WorkloadResult]) -> f64 {
    if workloads.is_empty() {
        return 1.0;
    }
    let log_sum: f64 = workloads
        .iter()
        .map(|w| w.speedup_factor.max(0.01).ln())
        .sum();
    let gm = (log_sum / workloads.len() as f64).exp();
    (gm * 100.0).round() / 100.0
}

pub fn compute_query_guard_speedup(workloads: &[WorkloadResult]) -> f64 {
    let subset: Vec<&WorkloadResult> = workloads
        .iter()
        .filter(|w| matches!(w.id.as_str(), "W1" | "W2" | "W4" | "W6" | "W7" | "W8" | "W9"))
        .collect();
    if subset.is_empty() {
        return 1.0;
    }
    let log_sum: f64 = subset
        .iter()
        .map(|w| w.speedup_factor.max(0.01).ln())
        .sum();
    let gm = (log_sum / subset.len() as f64).exp();
    (gm * 100.0).round() / 100.0
}

pub struct BenchmarkEngine;

impl BenchmarkEngine {
    /// 采集宿主机物理与数据库规模指纹，保证每次评测可复现比对
    pub fn collect_host_fingerprint(storage: &dyn StorageProvider) -> serde_json::Value {
        let assets_count = storage.list_assets(50000).map(|v| v.len()).unwrap_or(0);
        let skills_count = storage.list_skills_catalog().map(|v| v.len()).unwrap_or(0);
        let mcp_count = storage.list_mcp_catalog().map(|v| v.len()).unwrap_or(0);
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);

        serde_json::json!({
            "os": crate::BUILD_OS,
            "arch": crate::BUILD_ARCH,
            "cpu_cores": cpus,
            "assets_count": assets_count,
            "skills_count": skills_count,
            "mcp_count": mcp_count,
            "provider": storage.provider_name(),
        })
    }

    /// 运行基准评测套件: suite = "full" | "macro" | "micro" | "W1".."W8"
    pub fn run_suite(
        storage: Arc<dyn StorageProvider>,
        suite: &str,
        save: bool,
        run_id_prefix: Option<&str>,
    ) -> Result<BenchmarkReportPayload> {
        let t0 = Instant::now();
        let suite_norm = suite.trim();
        let (workloads, micro_benchmarks, data_quality): (
            Vec<WorkloadResult>,
            Vec<MicroBenchResult>,
            Vec<DataQualityResult>,
        ) = if suite_norm.eq_ignore_ascii_case("full") || suite_norm.is_empty() {
            (
                run_all_workloads(storage.clone())?,
                run_all_micro_benchmarks()?,
                run_data_quality_checks(storage.as_ref()),
            )
        } else if suite_norm.eq_ignore_ascii_case("macro") {
            (run_all_workloads(storage.clone())?, Vec::new(), Vec::new())
        } else if suite_norm.eq_ignore_ascii_case("micro") {
            (Vec::new(), run_all_micro_benchmarks()?, Vec::new())
        } else if suite_norm.eq_ignore_ascii_case("dq") {
            (Vec::new(), Vec::new(), run_data_quality_checks(storage.as_ref()))
        } else {
            (
                vec![run_workload(storage.clone(), suite_norm)?],
                Vec::new(),
                Vec::new(),
            )
        };

        let total_baseline_tokens: i64 = workloads.iter().map(|w| w.baseline_tokens).sum();
        let total_steward_tokens: i64 = workloads.iter().map(|w| w.steward_tokens).sum();
        let overall_token_savings_pct = if total_baseline_tokens > 0 {
            let saved = total_baseline_tokens.saturating_sub(total_steward_tokens);
            ((saved as f64 / total_baseline_tokens as f64) * 100.0 * 100.0).round() / 100.0
        } else {
            0.0
        };

        let total_base_us: u128 = workloads.iter().map(|w| w.baseline_elapsed_us).sum();
        let total_stew_us: u128 = workloads.iter().map(|w| w.steward_elapsed_us).sum();
        let overall_speedup_factor = if total_stew_us > 0 && total_base_us > 0 {
            ((total_base_us as f64 / total_stew_us as f64) * 100.0).round() / 100.0
        } else {
            1.0
        };

        let geometric_mean_speedup = compute_geomean_speedup(&workloads);
        let query_guard_speedup = compute_query_guard_speedup(&workloads);
        let mut d6_samples: Vec<f64> = workloads
            .iter()
            .filter_map(|w| w.quality_score)
            .collect();
        for dq in &data_quality {
            d6_samples.push(dq.measured_pct);
        }
        let d6_quality_score = if d6_samples.is_empty() {
            100.0
        } else {
            let avg = d6_samples.iter().sum::<f64>() / d6_samples.len() as f64;
            (avg * 100.0).round() / 100.0
        };

        let total_hazards_intercepted: i64 = workloads.iter().map(|w| w.hazards_intercepted).sum();
        let total_noise_blocked: i64 = workloads.iter().map(|w| w.blocked_noise as i64).sum();
        let sla_passed = workloads.iter().all(|w| w.sla_passed)
            && micro_benchmarks.iter().all(|m| m.passed)
            && data_quality.iter().all(|d| d.passed);

        let prefix = run_id_prefix.unwrap_or("run");
        let run_id = format!("{}-{}", prefix, uuid::Uuid::new_v4());
        let git_commit = crate::BUILD_GIT_HASH.to_string();
        let app_version = crate::VERSION.to_string();
        let host_fingerprint = Self::collect_host_fingerprint(storage.as_ref());
        let report_sha256 = BenchmarkReportPayload::compute_canonical_sha256(
            &run_id,
            suite_norm,
            &git_commit,
            &workloads,
            &micro_benchmarks,
        );
        let duration_ms = t0.elapsed().as_millis() as i64;
        let created_at = chrono::Utc::now().to_rfc3339();

        let payload = BenchmarkReportPayload {
            spec_version: BENCHMARK_SPEC_VERSION.to_string(),
            run_id: run_id.clone(),
            suite_name: suite_norm.to_string(),
            git_commit: git_commit.clone(),
            app_version: app_version.clone(),
            host_fingerprint: host_fingerprint.clone(),
            workloads: workloads.clone(),
            micro_benchmarks: micro_benchmarks.clone(),
            data_quality: data_quality.clone(),
            total_baseline_tokens,
            total_steward_tokens,
            overall_token_savings_pct,
            overall_speedup_factor,
            geometric_mean_speedup,
            query_guard_speedup,
            d6_quality_score,
            total_hazards_intercepted,
            total_noise_blocked,
            sla_passed,
            duration_ms,
            report_sha256: report_sha256.clone(),
            created_at: created_at.clone(),
        };

        if save {
            let rec = BenchmarkRunRecord {
                run_id,
                suite_name: suite_norm.to_string(),
                git_commit,
                app_version,
                host_fingerprint,
                workload_count: workloads.len(),
                total_baseline_tokens,
                total_steward_tokens,
                overall_token_savings_pct,
                overall_speedup_factor: geometric_mean_speedup,
                total_hazards_intercepted,
                total_noise_blocked,
                sla_passed,
                results_json: serde_json::json!({
                    "spec_version": BENCHMARK_SPEC_VERSION,
                    "geometric_mean_speedup": geometric_mean_speedup,
                    "query_guard_speedup": query_guard_speedup,
                    "d6_quality_score": d6_quality_score,
                    "workloads": workloads,
                    "micro_benchmarks": micro_benchmarks,
                    "data_quality": data_quality,
                }),
                report_sha256,
                duration_ms,
                created_at,
            };
            storage.insert_benchmark_run(&rec)?;
        }

        Ok(payload)
    }

    pub fn reset_sandbox(storage: &dyn StorageProvider) -> Result<usize> {
        storage.delete_bench_sandbox_records()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SqliteStorageProvider;

    #[test]
    fn test_benchmark_engine_full_suite_and_sandbox_reset() {
        let provider = Arc::new(SqliteStorageProvider::open_in_memory().expect("sqlite"));
        let report = BenchmarkEngine::run_suite(provider.clone(), "full", true, Some("bench:test"))
            .expect("run full suite");

        assert_eq!(report.workloads.len(), 10);
        assert_eq!(report.micro_benchmarks.len(), 5);
        assert_eq!(report.data_quality.len(), 3);
        assert_eq!(report.spec_version, "2.0");
        assert_eq!(report.report_sha256.len(), 64);
        assert!(
            report.overall_token_savings_pct > 50.0,
            "总体 Token 节省率应 >50%，实测: {:.2}%",
            report.overall_token_savings_pct
        );
        assert!(report.total_hazards_intercepted >= 3);
        assert!(report.total_noise_blocked >= 200);
        for w in &report.workloads {
            println!(
                "[W] {} ({}): passed={}, base_tok={}, stew_tok={}, base_us={}, stew_us={}, haz={}, noise={}, detail={}",
                w.id, w.name, w.sla_passed, w.baseline_tokens, w.steward_tokens,
                w.baseline_elapsed_us, w.steward_elapsed_us, w.hazards_intercepted,
                w.blocked_noise, w.evidence_detail
            );
        }
        for m in &report.micro_benchmarks {
            println!(
                "[M] {} ({}): passed={}, avg_us={}, limit_us={}",
                m.id, m.name, m.passed, m.avg_us, m.sla_limit_us
            );
        }
        assert!(report.sla_passed, "8 大负载 + 5 大微基准 SLA 应全部通过");

        let fetched = provider
            .get_benchmark_run(&report.run_id)
            .expect("get run")
            .expect("saved run");
        assert_eq!(fetched.report_sha256, report.report_sha256);

        let cleaned = BenchmarkEngine::reset_sandbox(provider.as_ref()).expect("reset");
        assert!(cleaned >= 1);
    }
}
