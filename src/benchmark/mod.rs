pub(crate) mod competitor_tracks;
pub mod competitors;
pub mod dq;
pub mod ledger;
pub mod micro;
pub mod provenance;
pub mod report;
pub mod workloads;

use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;

use crate::benchmark::competitors::{
    build_competitor_capability_matrix, run_all_competitor_benchmarks, run_competitor_track,
    CompetitorCapabilityRow, CompetitorComparisonResult,
};
use crate::benchmark::dq::run_data_quality_checks;
use crate::benchmark::micro::{run_all_micro_benchmarks, MicroBenchResult};
use crate::benchmark::report::{BenchmarkReportPayload, DataQualityResult};
use crate::benchmark::workloads::{run_all_workloads, run_workload, WorkloadResult};
use crate::storage::models::BenchmarkRunRecord;
use crate::storage::StorageProvider;

pub const BENCHMARK_SPEC_VERSION: &str = "4.1";

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
        .filter(|w| matches!(w.id.as_str(), "W1" | "W2" | "W4" | "W6" | "W7" | "W8" | "W9" | "W11" | "W15" | "W16" | "W17" | "W18" | "W19" | "W20" | "W21" | "W22"))
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
        type BenchmarkSuiteOutput = (
            Vec<WorkloadResult>,
            Vec<MicroBenchResult>,
            Vec<DataQualityResult>,
            Vec<CompetitorComparisonResult>,
            Vec<CompetitorCapabilityRow>,
        );
        let (workloads, micro_benchmarks, data_quality, competitors, competitor_matrix): BenchmarkSuiteOutput =
            if suite_norm.eq_ignore_ascii_case("full") || suite_norm.is_empty() {
            (
                run_all_workloads(storage.clone())?,
                run_all_micro_benchmarks()?,
                run_data_quality_checks(storage.as_ref()),
                run_all_competitor_benchmarks(storage.clone())?,
                build_competitor_capability_matrix(),
            )
        } else if suite_norm.eq_ignore_ascii_case("macro") {
            (run_all_workloads(storage.clone())?, Vec::new(), Vec::new(), Vec::new(), Vec::new())
        } else if suite_norm.eq_ignore_ascii_case("micro") {
            (Vec::new(), run_all_micro_benchmarks()?, Vec::new(), Vec::new(), Vec::new())
        } else if suite_norm.eq_ignore_ascii_case("dq") {
            (Vec::new(), Vec::new(), run_data_quality_checks(storage.as_ref()), Vec::new(), Vec::new())
        } else if suite_norm.eq_ignore_ascii_case("competitors")
            || suite_norm.to_ascii_uppercase().starts_with("COMP-")
        {
            (
                Vec::new(),
                Vec::new(),
                Vec::new(),
                run_competitor_track(storage.clone(), suite_norm)?,
                build_competitor_capability_matrix(),
            )
        } else {
            (
                vec![run_workload(storage.clone(), suite_norm)?],
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        };

        let total_baseline_tokens: i64 = workloads.iter().map(|w| w.baseline_tokens).sum();
        let total_body_tokens: i64 = workloads.iter().map(|w| w.body_tokens).sum();
        let overall_token_savings_pct = if total_baseline_tokens > 0 {
            let saved = total_baseline_tokens.saturating_sub(total_body_tokens);
            ((saved as f64 / total_baseline_tokens as f64) * 100.0 * 100.0).round() / 100.0
        } else {
            0.0
        };

        let total_base_us: u128 = workloads.iter().map(|w| w.baseline_elapsed_us).sum();
        let total_body_us: u128 = workloads.iter().map(|w| w.body_elapsed_us).sum();
        let arithmetic_speedup_factor = if total_body_us > 0 && total_base_us > 0 {
            ((total_base_us as f64 / total_body_us as f64) * 100.0).round() / 100.0
        } else {
            1.0
        };

        let geometric_mean_speedup = compute_geomean_speedup(&workloads);
        let overall_speedup_factor = geometric_mean_speedup;
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

        let base_q_samples: Vec<f64> = workloads
            .iter()
            .filter(|w| w.required_facts_total > 0)
            .map(|w| w.baseline_quality_score)
            .collect();
        let body_q_samples: Vec<f64> = workloads
            .iter()
            .filter(|w| w.required_facts_total > 0)
            .map(|w| w.body_quality_score)
            .collect();
        let overall_baseline_quality_score = if base_q_samples.is_empty() {
            0.0
        } else {
            ((base_q_samples.iter().sum::<f64>() / base_q_samples.len() as f64) * 100.0).round() / 100.0
        };
        let overall_body_quality_score = if body_q_samples.is_empty() {
            d6_quality_score
        } else {
            ((body_q_samples.iter().sum::<f64>() / body_q_samples.len() as f64) * 100.0).round() / 100.0
        };
        let overall_quality_delta = if base_q_samples.is_empty() {
            0.0
        } else {
            ((overall_body_quality_score - overall_baseline_quality_score) * 100.0).round() / 100.0
        };
        let density_samples: Vec<f64> = workloads
            .iter()
            .filter(|w| w.info_density_gain > 0.0)
            .map(|w| w.info_density_gain)
            .collect();
        let overall_info_density_gain = if density_samples.is_empty() {
            1.0
        } else {
            let log_sum: f64 = density_samples.iter().map(|v| v.max(0.01).ln()).sum();
            ((log_sum / density_samples.len() as f64).exp() * 100.0).round() / 100.0
        };
        let quality_non_degraded = workloads
            .iter()
            .filter(|w| w.required_facts_total > 0)
            .all(|w| w.body_quality_score >= w.baseline_quality_score && w.body_facts_hit == w.required_facts_total)
            && competitors
                .iter()
                .filter(|c| c.required_facts_total > 0)
                .all(|c| c.body_quality_score >= c.competitor_quality_score && c.body_facts_hit == c.required_facts_total);

        let total_hazards_intercepted: i64 = workloads.iter().map(|w| w.hazards_intercepted).sum();
        let total_noise_blocked: i64 = workloads.iter().map(|w| w.blocked_noise as i64).sum();
        let sla_passed = workloads.iter().all(|w| w.sla_passed)
            && micro_benchmarks.iter().all(|m| m.passed)
            && data_quality.iter().all(|d| d.passed)
            && competitors.iter().all(|c| c.passed);

        let prefix = run_id_prefix.unwrap_or("run");
        let run_id = format!("{}-{}", prefix, uuid::Uuid::new_v4());
        let git_commit = crate::BUILD_GIT_HASH.to_string();
        let app_version = crate::VERSION.to_string();
        let host_fingerprint = Self::collect_host_fingerprint(storage.as_ref());
        let report_sha256 = BenchmarkReportPayload::compute_canonical_sha256(
            BENCHMARK_SPEC_VERSION,
            &run_id,
            suite_norm,
            &git_commit,
            &workloads,
            &micro_benchmarks,
            &data_quality,
            &competitors,
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
            competitors: competitors.clone(),
            competitor_matrix: competitor_matrix.clone(),
            total_baseline_tokens,
            total_body_tokens,
            overall_token_savings_pct,
            overall_speedup_factor,
            geometric_mean_speedup,
            query_guard_speedup,
            d6_quality_score,
            overall_baseline_quality_score,
            overall_quality_delta,
            overall_info_density_gain,
            quality_non_degraded,
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
                total_body_tokens,
                overall_token_savings_pct,
                overall_speedup_factor: geometric_mean_speedup,
                total_hazards_intercepted,
                total_noise_blocked,
                sla_passed,
                results_json: serde_json::json!({
                    "spec_version": BENCHMARK_SPEC_VERSION,
                    "geometric_mean_speedup": geometric_mean_speedup,
                    "arithmetic_speedup_factor": arithmetic_speedup_factor,
                    "query_guard_speedup": query_guard_speedup,
                    "d6_quality_score": d6_quality_score,
                    "overall_baseline_quality_score": overall_baseline_quality_score,
                    "overall_quality_delta": overall_quality_delta,
                    "overall_info_density_gain": overall_info_density_gain,
                    "quality_non_degraded": quality_non_degraded,
                    "workloads": workloads,
                    "micro_benchmarks": micro_benchmarks,
                    "data_quality": data_quality,
                    "competitors": competitors,
                    "competitor_matrix": competitor_matrix,
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

        assert_eq!(report.workloads.len(), 22);
        assert_eq!(report.micro_benchmarks.len(), 6);
        assert_eq!(report.data_quality.len(), 12);
        assert_eq!(report.competitors.len(), 4);
        assert_eq!(report.spec_version, "4.1");
        let bad_w: Vec<String> = report
            .workloads
            .iter()
            .filter(|w| w.required_facts_total > 0 && (w.body_quality_score < w.baseline_quality_score || w.body_facts_hit != w.required_facts_total))
            .map(|w| format!("{}(hit={}/{}, q={:.1}<{:.1})", w.id, w.body_facts_hit, w.required_facts_total, w.body_quality_score, w.baseline_quality_score))
            .collect();
        let bad_c: Vec<String> = report
            .competitors
            .iter()
            .filter(|c| c.required_facts_total > 0 && (c.body_quality_score < c.competitor_quality_score || c.body_facts_hit != c.required_facts_total))
            .map(|c| format!("{}(hit={}/{}, q={:.1}<{:.1})", c.id, c.body_facts_hit, c.required_facts_total, c.body_quality_score, c.competitor_quality_score))
            .collect();
        assert!(
            report.quality_non_degraded,
            "所有负载上下文质量必须不降反升; bad_w={:?}, bad_c={:?}",
            bad_w, bad_c
        );
        assert!(report.overall_quality_delta > 20.0, "整体上下文质量净提升应 > 20 分，实测: {}", report.overall_quality_delta);
        assert!(report.overall_info_density_gain > 5.0, "有效信息密度应提升 > 5x，实测: {}", report.overall_info_density_gain);
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
                "[W] {} ({}): passed={}, base_tok={}, body_tok={}, base_us={}, body_us={}, haz={}, noise={}, detail={}",
                w.id, w.name, w.sla_passed, w.baseline_tokens, w.body_tokens,
                w.baseline_elapsed_us, w.body_elapsed_us, w.hazards_intercepted,
                w.blocked_noise, w.evidence_detail
            );
        }
        for m in &report.micro_benchmarks {
            println!(
                "[M] {} ({}): passed={}, avg_us={}, limit_us={}",
                m.id, m.name, m.passed, m.avg_us, m.sla_limit_us
            );
        }
        assert!(report.sla_passed, "22 大负载 + 6 大微基准 + 12 项 DQ SLA 应全部通过");

        let fetched = provider
            .get_benchmark_run(&report.run_id)
            .expect("get run")
            .expect("saved run");
        assert_eq!(fetched.report_sha256, report.report_sha256);
        assert_eq!(fetched.overall_speedup_factor, report.overall_speedup_factor);

        let cleaned = BenchmarkEngine::reset_sandbox(provider.as_ref()).expect("reset");
        assert!(cleaned >= 1);
    }
}
