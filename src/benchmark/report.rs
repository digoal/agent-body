use std::fmt::Write as _;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::benchmark::competitors::{CompetitorCapabilityRow, CompetitorComparisonResult};
use crate::benchmark::micro::MicroBenchResult;
use crate::benchmark::provenance::sha256_hex;
use crate::benchmark::workloads::WorkloadResult;
use crate::storage::models::BenchmarkRunRecord;

fn default_spec_version() -> String {
    "1.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataQualityResult {
    pub id: String,
    pub name: String,
    pub title: String,
    pub measured_pct: f64,
    pub target_pct: f64,
    pub passed: bool,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReportPayload {
    #[serde(default = "default_spec_version")]
    pub spec_version: String,
    pub run_id: String,
    pub suite_name: String,
    pub git_commit: String,
    pub app_version: String,
    pub host_fingerprint: serde_json::Value,
    pub workloads: Vec<WorkloadResult>,
    pub micro_benchmarks: Vec<MicroBenchResult>,
    #[serde(default)]
    pub data_quality: Vec<DataQualityResult>,
    #[serde(default)]
    pub competitors: Vec<CompetitorComparisonResult>,
    #[serde(default)]
    pub competitor_matrix: Vec<CompetitorCapabilityRow>,
    pub total_baseline_tokens: i64,
    pub total_steward_tokens: i64,
    pub overall_token_savings_pct: f64,
    pub overall_speedup_factor: f64,
    #[serde(default)]
    pub geometric_mean_speedup: f64,
    #[serde(default)]
    pub query_guard_speedup: f64,
    #[serde(default)]
    pub d6_quality_score: f64,
    #[serde(default)]
    pub overall_baseline_quality_score: f64,
    #[serde(default)]
    pub overall_quality_delta: f64,
    #[serde(default)]
    pub overall_info_density_gain: f64,
    #[serde(default)]
    pub quality_non_degraded: bool,
    pub total_hazards_intercepted: i64,
    pub total_noise_blocked: i64,
    pub sla_passed: bool,
    pub duration_ms: i64,
    pub report_sha256: String,
    pub created_at: String,
}

impl BenchmarkReportPayload {
    /// 基于规范化 payload 摘要生成防篡改 SHA-256 指纹
    pub fn compute_canonical_sha256(
        run_id: &str,
        suite_name: &str,
        git_commit: &str,
        workloads: &[WorkloadResult],
        micro: &[MicroBenchResult],
    ) -> String {
        let canonical = serde_json::json!({
            "run_id": run_id,
            "suite_name": suite_name,
            "git_commit": git_commit,
            "workloads": workloads,
            "micro_benchmarks": micro,
        });
        let raw = serde_json::to_string(&canonical).unwrap_or_default();
        sha256_hex(&raw)
    }

    pub fn from_record(rec: &BenchmarkRunRecord) -> Self {
        let spec_version = rec
            .results_json
            .get("spec_version")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0")
            .to_string();
        let workloads: Vec<WorkloadResult> = rec
            .results_json
            .get("workloads")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let micro_benchmarks: Vec<MicroBenchResult> = rec
            .results_json
            .get("micro_benchmarks")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let data_quality: Vec<DataQualityResult> = rec
            .results_json
            .get("data_quality")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let competitors: Vec<CompetitorComparisonResult> = rec
            .results_json
            .get("competitors")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let competitor_matrix: Vec<CompetitorCapabilityRow> = rec
            .results_json
            .get("competitor_matrix")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let geometric_mean_speedup = rec
            .results_json
            .get("geometric_mean_speedup")
            .and_then(|v| v.as_f64())
            .unwrap_or_else(|| crate::benchmark::compute_geomean_speedup(&workloads));
        let query_guard_speedup = rec
            .results_json
            .get("query_guard_speedup")
            .and_then(|v| v.as_f64())
            .unwrap_or_else(|| crate::benchmark::compute_query_guard_speedup(&workloads));
        let d6_quality_score = rec
            .results_json
            .get("d6_quality_score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let overall_baseline_quality_score = rec
            .results_json
            .get("overall_baseline_quality_score")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let overall_quality_delta = rec
            .results_json
            .get("overall_quality_delta")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let overall_info_density_gain = rec
            .results_json
            .get("overall_info_density_gain")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        let quality_non_degraded = rec
            .results_json
            .get("quality_non_degraded")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        Self {
            spec_version,
            run_id: rec.run_id.clone(),
            suite_name: rec.suite_name.clone(),
            git_commit: rec.git_commit.clone(),
            app_version: rec.app_version.clone(),
            host_fingerprint: rec.host_fingerprint.clone(),
            workloads,
            micro_benchmarks,
            data_quality,
            competitors,
            competitor_matrix,
            total_baseline_tokens: rec.total_baseline_tokens,
            total_steward_tokens: rec.total_steward_tokens,
            overall_token_savings_pct: rec.overall_token_savings_pct,
            overall_speedup_factor: rec.overall_speedup_factor,
            geometric_mean_speedup,
            query_guard_speedup,
            d6_quality_score,
            overall_baseline_quality_score,
            overall_quality_delta,
            overall_info_density_gain,
            quality_non_degraded,
            total_hazards_intercepted: rec.total_hazards_intercepted,
            total_noise_blocked: rec.total_noise_blocked,
            sla_passed: rec.sla_passed,
            duration_ms: rec.duration_ms,
            report_sha256: rec.report_sha256.clone(),
            created_at: rec.created_at.clone(),
        }
    }
}

pub fn render_markdown_report(report: &BenchmarkReportPayload) -> Result<String> {
    let mut md = String::new();
    writeln!(md, "# Agent Host Steward 实证基准评测报告 ({})\n", report.run_id)?;
    writeln!(md, "- **版本 / Commit**: `v{}` (`{}`) · **Benchmark 规格**: `v{}`", report.app_version, report.git_commit, report.spec_version)?;
    writeln!(md, "- **评测套件**: `{}` · **运行时间**: `{}` · **总耗时**: `{} ms`", report.suite_name, report.created_at, report.duration_ms)?;
    writeln!(md, "- **报告 SHA-256 存证指纹**: `{}`", report.report_sha256)?;
    writeln!(md, "- **SLA 总检门禁**: {}\n", if report.sla_passed { "✅ PASS" } else { "❌ FAIL" })?;

    writeln!(md, "## 一、总体收益汇总 (Without Steward vs With Steward)\n")?;
    writeln!(md, "| 指标维度 | 基线 (Without Steward) | 管家赋能 (With Steward) | 净收益 |")?;
    writeln!(md, "|---|---:|---:|---:|")?;
    writeln!(
        md,
        "| **Prompt Token 消耗 (D1)** | {} tok | {} tok | **-{:.2}%** ({} tok saved) |",
        report.total_baseline_tokens,
        report.total_steward_tokens,
        report.overall_token_savings_pct,
        report.total_baseline_tokens.saturating_sub(report.total_steward_tokens)
    )?;
    writeln!(
        md,
        "| **几何平均加速比 (Geomean, D3 主口径)** | - | - | **{:.2}x** (Query/Guard: {:.2}x · 算术总比: {:.2}x) |",
        report.geometric_mean_speedup,
        report.query_guard_speedup,
        report.overall_speedup_factor
    )?;
    writeln!(
        md,
        "| **D6 上下文质量 (基线 → 管家)** | {:.1}/100 | **{:.1}/100** | **+{:.1} 分** (不降反升: {}) |",
        report.overall_baseline_quality_score,
        report.d6_quality_score,
        report.overall_quality_delta,
        if report.quality_non_degraded { "✅" } else { "❌" }
    )?;
    writeln!(
        md,
        "| **有效信息密度 (Facts/1k-tok)** | 1.00x | **{:.1}x** | **密度提升 {:.1} 倍** |",
        report.overall_info_density_gain,
        report.overall_info_density_gain
    )?;
    writeln!(
        md,
        "| **事前拦截风险数** | - | - | **{} 次** |",
        report.total_hazards_intercepted
    )?;
    writeln!(
        md,
        "| **阻断垃圾噪音文件** | - | - | **{} 个** |\n",
        report.total_noise_blocked
    )?;

    if !report.workloads.is_empty() {
        writeln!(md, "## 二、11 大标准宿主负载 A/B 实测明细（省 Token × 质量提升帕累托双门禁）\n")?;
        writeln!(md, "| ID | 场景 | Token (基线→管家) | 节省率 | 上下文质量 (基线→管家) | 事实命中 | 信息密度 | 加速比 | 状态 |")?;
        writeln!(md, "|:---:|---|---:|---:|---:|:---:|---:|---:|:---:|")?;
        for w in &report.workloads {
            writeln!(
                md,
                "| {} | {} | {} → {} | {:.1}% | {:.1} → **{:.1}** (+{:.1}) | {}/{} | {:.1}x | {:.2}x | {} |",
                w.id,
                w.title,
                w.baseline_tokens,
                w.steward_tokens,
                w.token_savings_pct,
                w.baseline_quality_score,
                w.steward_quality_score,
                w.quality_delta,
                w.steward_facts_hit,
                w.required_facts_total,
                w.info_density_gain,
                w.speedup_factor,
                if w.sla_passed { "✅" } else { "❌" }
            )?;
        }
        writeln!(md)?;
    }

    if !report.micro_benchmarks.is_empty() {
        writeln!(md, "## 三、5 大内核微基准 SLA (P50 / P95 / P99)\n")?;
        writeln!(md, "| ID | 基准项 | 迭代次数 | 平均耗时 (μs) | P50 (μs) | P95 (μs) | P99 (μs) | SLA 上限 (μs) | 状态 |")?;
        writeln!(md, "|:---:|---|---:|---:|---:|---:|---:|---:|:---:|")?;
        for m in &report.micro_benchmarks {
            writeln!(
                md,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                m.id,
                m.name,
                m.iterations,
                m.avg_us,
                m.p50_us,
                m.p95_us,
                m.p99_us,
                m.sla_limit_us,
                if m.passed { "✅" } else { "❌" }
            )?;
        }
        writeln!(md)?;
    }

    if !report.data_quality.is_empty() {
        writeln!(md, "## 四、宿主机数据质量与 D6 准确率门禁 (DQ1~DQ4)\n")?;
        writeln!(md, "| ID | 检查项 | 实测指标 | SLA 目标 | 状态 | 详情 |")?;
        writeln!(md, "|:---:|---|---:|---:|:---:|---|")?;
        for dq in &report.data_quality {
            writeln!(
                md,
                "| {} | {} | {:.1}% | >= {:.1}% | {} | {} |",
                dq.id,
                dq.title,
                dq.measured_pct,
                dq.target_pct,
                if dq.passed { "✅" } else { "❌" },
                dq.detail
            )?;
        }
        writeln!(md)?;
    }

    if !report.competitors.is_empty() {
        writeln!(md, "## 五、热门开源竞品横向对标实测 (`COMP-C1 ~ COMP-C4`, `LIVE_BENCHMARKED`)\n")?;
        writeln!(md, "| ID | 赛道 | 对标对象 | 竞品指标 | Steward 指标 | 上下文质量 (竞品→Steward) | 信息密度 | Token 节省 | 状态 |")?;
        writeln!(md, "|:---:|---|---|---|---|---:|---:|---:|:---:|")?;
        for c in &report.competitors {
            writeln!(
                md,
                "| {} | {} | {} | {} (`{} μs`) | {} (`{} μs`) | {:.1} → **{:.1}** (+{:.1}) | {:.1}x | {:.1}% | {} |",
                c.id,
                c.title,
                c.competitor_name,
                c.competitor_metric_value,
                c.competitor_elapsed_us,
                c.steward_metric_value,
                c.steward_elapsed_us,
                c.competitor_quality_score,
                c.steward_quality_score,
                c.quality_delta,
                c.info_density_gain,
                c.token_savings_pct,
                if c.passed { "✅" } else { "❌" }
            )?;
        }
        writeln!(md)?;
    }

    Ok(md)
}

pub fn render_csv_report(report: &BenchmarkReportPayload) -> Result<String> {
    let mut csv = String::from("kind,id,name,baseline_tokens,steward_tokens,token_savings_pct,baseline_elapsed_us,steward_elapsed_us,speedup_factor,hazards_intercepted,blocked_noise,passed\n");
    for w in &report.workloads {
        writeln!(
            csv,
            "workload,{},{},{},{},{:.2},{},{},{:.2},{},{},{}",
            w.id,
            w.name,
            w.baseline_tokens,
            w.steward_tokens,
            w.token_savings_pct,
            w.baseline_elapsed_us,
            w.steward_elapsed_us,
            w.speedup_factor,
            w.hazards_intercepted,
            w.blocked_noise,
            w.sla_passed
        )?;
    }
    for m in &report.micro_benchmarks {
        writeln!(
            csv,
            "micro,{},{},0,0,0.00,0,{},1.00,0,0,{}",
            m.id, m.name, m.avg_us, m.passed
        )?;
    }
    Ok(csv)
}
