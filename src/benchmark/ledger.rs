use serde::{Deserialize, Serialize};

use crate::benchmark::provenance::{estimate_tokens, ProvenanceRule};
use crate::benchmark::workloads::WorkloadResult;
use crate::storage::models::{BenchmarkRunRecord, EffectivenessLedgerRecord};
use crate::storage::StorageProvider;

/// 生产运行态效能记账入参
pub struct LedgerEventInput {
    pub session_id: String,
    pub source_channel: String,
    pub tool_or_action: String,
    pub rule: ProvenanceRule,
    pub baseline_tokens: i64,
    pub steward_tokens: i64,
    pub baseline_rounds: i64,
    pub steward_rounds: i64,
    pub hazard_intercepted: i64,
    pub noise_blocked: i64,
    pub elapsed_us: i64,
    pub evidence_summary: String,
}

/// 跨批次 Benchmark 回归对比结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadDelta {
    pub id: String,
    pub name: String,
    pub run_a_elapsed_us: u128,
    pub run_b_elapsed_us: u128,
    pub elapsed_delta_pct: f64,
    pub run_a_savings_pct: f64,
    pub run_b_savings_pct: f64,
    pub savings_delta_pct: f64,
    pub regression_flag: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunComparison {
    pub run_a_id: String,
    pub run_b_id: String,
    pub run_a_version: String,
    pub run_b_version: String,
    #[serde(default)]
    pub run_a_spec_version: String,
    #[serde(default)]
    pub run_b_spec_version: String,
    #[serde(default)]
    pub spec_version_changed: bool,
    pub speedup_a: f64,
    pub speedup_b: f64,
    pub token_savings_a_pct: f64,
    pub token_savings_b_pct: f64,
    pub workloads_diff: Vec<WorkloadDelta>,
    pub has_regression: bool,
}

pub struct EffectivenessLedger;

impl EffectivenessLedger {
    /// 非阻塞静默容错记账：即使写入失败也不会中断主业务调用链
    pub fn record(storage: &dyn StorageProvider, input: LedgerEventInput) {
        let tokens_saved = input.baseline_tokens.saturating_sub(input.steward_tokens);
        let rounds_saved = input.baseline_rounds.saturating_sub(input.steward_rounds);
        let rec = EffectivenessLedgerRecord {
            event_id: format!("ev-{}", uuid::Uuid::new_v4()),
            session_id: input.session_id,
            source_channel: input.source_channel,
            tool_or_action: input.tool_or_action,
            provenance_rule: input.rule.code().to_string(),
            baseline_tokens: input.baseline_tokens,
            steward_tokens: input.steward_tokens,
            tokens_saved,
            baseline_rounds: input.baseline_rounds,
            steward_rounds: input.steward_rounds,
            rounds_saved,
            hazard_intercepted: input.hazard_intercepted,
            noise_blocked: input.noise_blocked,
            elapsed_us: input.elapsed_us,
            evidence_summary: input.evidence_summary,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        if let Err(e) = storage.insert_effectiveness_event(&rec) {
            eprintln!("[BenchmarkLedger] 记账写入失败(已降级忽略): {}", e);
        }
    }

    /// 根据 MCP 工具名与实际输出自动按 PR-01~PR-10 推导并记入生产效能账本
    pub fn record_mcp_call(
        storage: &dyn StorageProvider,
        tool_name: &str,
        output_text: &str,
        elapsed_us: i64,
    ) {
        Self::record_channel_call(storage, "mcp", tool_name, output_text, elapsed_us);
    }

    /// 全渠道统一轻量记账入口：支持 mcp / cli / web / hook，单次调用耗时 < 100 μs
    pub fn record_channel_call(
        storage: &dyn StorageProvider,
        channel: &str,
        tool_or_action: &str,
        output_text: &str,
        elapsed_us: i64,
    ) {
        if tool_or_action == "steward_benchmark"
            || tool_or_action == "steward_demo_scenario"
            || tool_or_action == "bench"
        {
            return;
        }
        let session_id = format!("{}-live", channel);
        let stew_tokens = estimate_tokens(output_text);
        let input = match tool_or_action {
            "steward_skill_route" | "steward_mcp_catalog" | "steward_service_contact" => {
                let skills_tok: i64 = storage
                    .list_skills_catalog()
                    .map(|v| v.iter().map(|s| s.est_tokens as i64).sum())
                    .unwrap_or(0);
                let mcps_tok: i64 = storage
                    .list_mcp_catalog()
                    .map(|v| v.iter().map(|m| m.est_total_tokens as i64).sum())
                    .unwrap_or(0);
                let base = (skills_tok + mcps_tok).max(stew_tokens);
                LedgerEventInput {
                    session_id,
                    source_channel: channel.into(),
                    tool_or_action: tool_or_action.into(),
                    rule: ProvenanceRule::Pr01SkillMcpRouting,
                    baseline_tokens: base,
                    steward_tokens: stew_tokens,
                    baseline_rounds: 3,
                    steward_rounds: 1,
                    hazard_intercepted: 0,
                    noise_blocked: 0,
                    elapsed_us,
                    evidence_summary: format!("PR-01: 库内全量技能+MCP {} tok -> 路由输出 {} tok", base, stew_tokens),
                }
            }
            "steward_asset_search" | "steward_asset_register" => LedgerEventInput {
                session_id,
                source_channel: channel.into(),
                tool_or_action: tool_or_action.into(),
                rule: ProvenanceRule::Pr02ProgressiveAssetSearch,
                baseline_tokens: stew_tokens.saturating_mul(4).max(400),
                steward_tokens: stew_tokens,
                baseline_rounds: 4,
                steward_rounds: 1,
                hazard_intercepted: 0,
                noise_blocked: 0,
                elapsed_us,
                evidence_summary: format!("PR-02: 渐进式元数据披露 {} tok（免全量读源码）", stew_tokens),
            },
            "steward_hook_pre" => {
                let blocked = output_text.contains("BLOCK")
                    || output_text.contains("WARN")
                    || output_text.contains("occupied")
                    || output_text.contains("冲突")
                    || output_text.contains("警告");
                if !blocked {
                    return;
                }
                LedgerEventInput {
                    session_id,
                    source_channel: "hook".into(),
                    tool_or_action: tool_or_action.into(),
                    rule: ProvenanceRule::Pr03PreHookInterception,
                    baseline_tokens: 600,
                    steward_tokens: stew_tokens,
                    baseline_rounds: 3,
                    steward_rounds: 1,
                    hazard_intercepted: 1,
                    noise_blocked: 0,
                    elapsed_us,
                    evidence_summary: "PR-03: Pre-Hook 执行前静态拦截冲突/违规".into(),
                }
            }
            "steward_fact_query" | "steward_port_check" | "steward_env_processes" | "steward_env_show"
            | "steward_placement_suggest" | "steward_changes_since" => {
                let occupied = tool_or_action == "steward_port_check"
                    && (output_text.contains("occupied") || output_text.contains("已被占用"));
                LedgerEventInput {
                    session_id,
                    source_channel: channel.into(),
                    tool_or_action: tool_or_action.into(),
                    rule: ProvenanceRule::Pr04HostFactQuery,
                    baseline_tokens: stew_tokens.saturating_mul(3).max(300),
                    steward_tokens: stew_tokens,
                    baseline_rounds: 4,
                    steward_rounds: 1,
                    hazard_intercepted: if occupied { 1 } else { 0 },
                    noise_blocked: 0,
                    elapsed_us,
                    evidence_summary: format!("PR-04: 宿主机只读物理事实/放置查询 ({} tok, {} μs)", stew_tokens, elapsed_us),
                }
            }
            "steward_links_impact" | "steward_links_audit" | "steward_links_repair" | "steward_codegraph_bridge" => {
                LedgerEventInput {
                    session_id,
                    source_channel: channel.into(),
                    tool_or_action: tool_or_action.into(),
                    rule: ProvenanceRule::Pr05GraphImpactAndAudit,
                    baseline_tokens: stew_tokens.saturating_mul(4).max(500),
                    steward_tokens: stew_tokens,
                    baseline_rounds: 4,
                    steward_rounds: 1,
                    hazard_intercepted: 0,
                    noise_blocked: 0,
                    elapsed_us,
                    evidence_summary: format!("PR-05: 宏观拓扑/宏微联邦图分析 ({} tok)", stew_tokens),
                }
            }
            "steward_lock_acquire" | "steward_lock_wait" | "steward_lock_deadlock_check" => {
                let intercepted = output_text.contains("Denied")
                    || output_text.contains("deadlock")
                    || output_text.contains("true");
                if !intercepted {
                    return;
                }
                LedgerEventInput {
                    session_id,
                    source_channel: channel.into(),
                    tool_or_action: tool_or_action.into(),
                    rule: ProvenanceRule::Pr07LockDeadlockGuard,
                    baseline_tokens: 400,
                    steward_tokens: stew_tokens,
                    baseline_rounds: 3,
                    steward_rounds: 1,
                    hazard_intercepted: 1,
                    noise_blocked: 0,
                    elapsed_us,
                    evidence_summary: "PR-07: 多 Agent 协作锁冲突或死锁环路预警".into(),
                }
            }
            "steward_brief" => LedgerEventInput {
                session_id,
                source_channel: channel.into(),
                tool_or_action: tool_or_action.into(),
                rule: ProvenanceRule::Pr08AdaptiveBriefColdStart,
                baseline_tokens: stew_tokens.saturating_mul(3).max(1800),
                steward_tokens: stew_tokens,
                baseline_rounds: 6,
                steward_rounds: 1,
                hazard_intercepted: 0,
                noise_blocked: 0,
                elapsed_us,
                evidence_summary: format!("PR-08: 冷启动自适应简报组装 ({} tok, 免 5 轮探查)", stew_tokens),
            },
            "steward_memory_search" => LedgerEventInput {
                session_id,
                source_channel: channel.into(),
                tool_or_action: tool_or_action.into(),
                rule: ProvenanceRule::Pr09HybridSemanticSearch,
                baseline_tokens: stew_tokens.saturating_mul(4).max(600),
                steward_tokens: stew_tokens,
                baseline_rounds: 3,
                steward_rounds: 1,
                hazard_intercepted: 0,
                noise_blocked: 0,
                elapsed_us,
                evidence_summary: format!("PR-09: 语义向量+FTS 混合检索 ({} tok)", stew_tokens),
            },
            "steward_memory_add" | "steward_session_close" | "steward_feedback" => LedgerEventInput {
                session_id,
                source_channel: channel.into(),
                tool_or_action: tool_or_action.into(),
                rule: ProvenanceRule::Pr10FeedbackAndMemoryLoop,
                baseline_tokens: stew_tokens.saturating_mul(3).max(450),
                steward_tokens: stew_tokens,
                baseline_rounds: 2,
                steward_rounds: 1,
                hazard_intercepted: 0,
                noise_blocked: 0,
                elapsed_us,
                evidence_summary: format!("PR-10: 实际反馈与长期记忆沉淀 ({} tok)", stew_tokens),
            },
            "steward_hook_post" => {
                let recovered = output_text.contains("recovery_prescription")
                    || output_text.contains("AOPCC 碰壁秒级自愈");
                if !recovered {
                    return;
                }
                LedgerEventInput {
                    session_id,
                    source_channel: "hook".into(),
                    tool_or_action: tool_or_action.into(),
                    rule: ProvenanceRule::Pr11OptimisticLeaseAndRecovery,
                    baseline_tokens: stew_tokens.saturating_mul(3).max(600),
                    steward_tokens: stew_tokens,
                    baseline_rounds: 3,
                    steward_rounds: 1,
                    hazard_intercepted: 1,
                    noise_blocked: 0,
                    elapsed_us,
                    evidence_summary: "PR-11: AOPCC 乐观直行碰壁 <2ms 自愈处方注入".into(),
                }
            }
            _ => return,
        };
        Self::record(storage, input);
    }

    /// 跨版本 / 跨批次基准回归对比
    pub fn compare_runs(a: &BenchmarkRunRecord, b: &BenchmarkRunRecord) -> RunComparison {
        let map_a: Vec<WorkloadResult> = a
            .results_json
            .get("workloads")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let map_b: Vec<WorkloadResult> = b
            .results_json
            .get("workloads")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let mut diffs = Vec::new();
        let mut has_regression = false;

        for wb in &map_b {
            if let Some(wa) = map_a.iter().find(|x| x.id == wb.id) {
                let elapsed_delta_pct = if wa.steward_elapsed_us > 0 {
                    ((wb.steward_elapsed_us as f64 - wa.steward_elapsed_us as f64)
                        / wa.steward_elapsed_us as f64)
                        * 100.0
                } else {
                    0.0
                };
                let savings_delta_pct = wb.token_savings_pct - wa.token_savings_pct;
                // 若延迟暴涨 >50% 且超过 1ms 或 Token 节省率显著退化 >5%，标记回归告警
                let regression = (elapsed_delta_pct > 50.0 && wb.steward_elapsed_us > 1_000)
                    || savings_delta_pct < -5.0;
                if regression {
                    has_regression = true;
                }
                diffs.push(WorkloadDelta {
                    id: wb.id.clone(),
                    name: wb.name.clone(),
                    run_a_elapsed_us: wa.steward_elapsed_us,
                    run_b_elapsed_us: wb.steward_elapsed_us,
                    elapsed_delta_pct: (elapsed_delta_pct * 100.0).round() / 100.0,
                    run_a_savings_pct: wa.token_savings_pct,
                    run_b_savings_pct: wb.token_savings_pct,
                    savings_delta_pct: (savings_delta_pct * 100.0).round() / 100.0,
                    regression_flag: regression,
                });
            }
        }

        let spec_a = a
            .results_json
            .get("spec_version")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0")
            .to_string();
        let spec_b = b
            .results_json
            .get("spec_version")
            .and_then(|v| v.as_str())
            .unwrap_or("1.0")
            .to_string();
        let spec_version_changed = spec_a != spec_b;

        RunComparison {
            run_a_id: a.run_id.clone(),
            run_b_id: b.run_id.clone(),
            run_a_version: a.app_version.clone(),
            run_b_version: b.app_version.clone(),
            run_a_spec_version: spec_a,
            run_b_spec_version: spec_b,
            spec_version_changed,
            speedup_a: a.overall_speedup_factor,
            speedup_b: b.overall_speedup_factor,
            token_savings_a_pct: a.overall_token_savings_pct,
            token_savings_b_pct: b.overall_token_savings_pct,
            workloads_diff: diffs,
            has_regression,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SqliteStorageProvider;

    #[test]
    fn test_record_channel_call_cli_web_pr09_pr10() {
        let provider = SqliteStorageProvider::open_in_memory().expect("sqlite");
        EffectivenessLedger::record_channel_call(
            &provider,
            "cli",
            "steward_memory_search",
            "命中 3 条混合语义记忆",
            180,
        );
        EffectivenessLedger::record_channel_call(
            &provider,
            "web",
            "steward_feedback",
            "已记录 CON-01 并沉淀记忆",
            95,
        );
        let summary = provider
            .query_effectiveness_ledger("7d", None, 10)
            .expect("query ledger");
        assert_eq!(summary.total_events, 2);
        assert!(summary.by_rule.iter().any(|r| r.provenance_rule == "PR-09"));
        assert!(summary.by_rule.iter().any(|r| r.provenance_rule == "PR-10"));
    }
}
