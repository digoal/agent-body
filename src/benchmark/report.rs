use std::fmt::Write as _;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::benchmark::micro::MicroBenchResult;
use crate::benchmark::provenance::sha256_hex;
use crate::benchmark::workloads::WorkloadResult;
use crate::storage::models::BenchmarkRunRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkReportPayload {
    pub run_id: String,
    pub suite_name: String,
    pub git_commit: String,
    pub app_version: String,
    pub host_fingerprint: serde_json::Value,
    pub workloads: Vec<WorkloadResult>,
    pub micro_benchmarks: Vec<MicroBenchResult>,
    pub total_baseline_tokens: i64,
    pub total_steward_tokens: i64,
    pub overall_token_savings_pct: f64,
    pub overall_speedup_factor: f64,
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
        Self {
            run_id: rec.run_id.clone(),
            suite_name: rec.suite_name.clone(),
            git_commit: rec.git_commit.clone(),
            app_version: rec.app_version.clone(),
            host_fingerprint: rec.host_fingerprint.clone(),
            workloads,
            micro_benchmarks,
            total_baseline_tokens: rec.total_baseline_tokens,
            total_steward_tokens: rec.total_steward_tokens,
            overall_token_savings_pct: rec.overall_token_savings_pct,
            overall_speedup_factor: rec.overall_speedup_factor,
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
    writeln!(md, "- **版本 / Commit**: `v{}` (`{}`)", report.app_version, report.git_commit)?;
    writeln!(md, "- **评测套件**: `{}` · **运行时间**: `{}` · **总耗时**: `{} ms`", report.suite_name, report.created_at, report.duration_ms)?;
    writeln!(md, "- **报告 SHA-256 存证指纹**: `{}`", report.report_sha256)?;
    writeln!(md, "- **SLA 总检门禁**: {}\n", if report.sla_passed { "✅ PASS" } else { "❌ FAIL" })?;

    writeln!(md, "## 一、总体收益汇总 (Without Steward vs With Steward)\n")?;
    writeln!(md, "| 指标维度 | 基线 (Without Steward) | 管家赋能 (With Steward) | 净收益 |")?;
    writeln!(md, "|---|---:|---:|---:|")?;
    writeln!(
        md,
        "| **Prompt Token 消耗** | {} tok | {} tok | **-{:.2}%** ({} tok saved) |",
        report.total_baseline_tokens,
        report.total_steward_tokens,
        report.overall_token_savings_pct,
        report.total_baseline_tokens.saturating_sub(report.total_steward_tokens)
    )?;
    writeln!(
        md,
        "| **综合执行加速比** | - | - | **{:.2}x** |",
        report.overall_speedup_factor
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
        writeln!(md, "## 二、8 大标准宿主负载 A/B 实测明细\n")?;
        writeln!(md, "| ID | 场景 | 基线 Token | 管家 Token | 节省率 | 基线耗时 (μs) | 管家耗时 (μs) | 加速比 | 风险拦截 | 噪音阻断 | 状态 |")?;
        writeln!(md, "|:---:|---|---:|---:|---:|---:|---:|---:|---:|---:|:---:|")?;
        for w in &report.workloads {
            writeln!(
                md,
                "| {} | {} | {} | {} | {:.1}% | {} | {} | {:.2}x | {} | {} | {} |",
                w.id,
                w.title,
                w.baseline_tokens,
                w.steward_tokens,
                w.token_savings_pct,
                w.baseline_elapsed_us,
                w.steward_elapsed_us,
                w.speedup_factor,
                w.hazards_intercepted,
                w.blocked_noise,
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
