use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 确定性分词估算器：ASCII 按 ceil(bytes / 4.0)，非 ASCII 字符（含中文）按 ceil(chars * 1.5)
pub fn estimate_tokens(text: &str) -> i64 {
    if text.is_empty() {
        return 0;
    }
    let mut ascii_bytes = 0usize;
    let mut non_ascii_chars = 0usize;
    for ch in text.chars() {
        if ch.is_ascii() {
            ascii_bytes += ch.len_utf8();
        } else {
            non_ascii_chars += 1;
        }
    }
    let ascii_tok = (ascii_bytes as f64 / 4.0).ceil() as i64;
    let cjk_tok = (non_ascii_chars as f64 * 1.5).ceil() as i64;
    (ascii_tok + cjk_tok).max(1)
}

/// 计算规范化 SHA-256 十六进制指纹
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 生产效能存证账本确定性推导规则 (PR-01 ~ PR-13)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProvenanceRule {
    Pr01SkillMcpRouting,
    Pr02ProgressiveAssetSearch,
    Pr03PreHookInterception,
    Pr04HostFactQuery,
    Pr05GraphImpactAndAudit,
    Pr06GranularityNoiseGate,
    Pr07LockDeadlockGuard,
    Pr08AdaptiveBriefColdStart,
    Pr09HybridSemanticSearch,
    Pr10FeedbackAndMemoryLoop,
    Pr11OptimisticLeaseAndRecovery,
    Pr12AuditDrivenSelfEvolution,
    Pr13CrossBrainContinuitySaved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvenanceRuleMeta {
    pub code: String,
    pub name: String,
    pub applies_to: String,
    pub formula: String,
}

impl ProvenanceRule {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Pr01SkillMcpRouting => "PR-01",
            Self::Pr02ProgressiveAssetSearch => "PR-02",
            Self::Pr03PreHookInterception => "PR-03",
            Self::Pr04HostFactQuery => "PR-04",
            Self::Pr05GraphImpactAndAudit => "PR-05",
            Self::Pr06GranularityNoiseGate => "PR-06",
            Self::Pr07LockDeadlockGuard => "PR-07",
            Self::Pr08AdaptiveBriefColdStart => "PR-08",
            Self::Pr09HybridSemanticSearch => "PR-09",
            Self::Pr10FeedbackAndMemoryLoop => "PR-10",
            Self::Pr11OptimisticLeaseAndRecovery => "PR-11",
            Self::Pr12AuditDrivenSelfEvolution => "PR-12",
            Self::Pr13CrossBrainContinuitySaved => "PR-13",
        }
    }

    pub fn title(&self) -> &'static str {
        match self {
            Self::Pr01SkillMcpRouting => "Skill/MCP 预算路由上下文压缩",
            Self::Pr02ProgressiveAssetSearch => "资产渐进式元数据披露",
            Self::Pr03PreHookInterception => "Pre-Hook 事前冲突拦截",
            Self::Pr04HostFactQuery => "宿主机物理事实免 Shell 探测",
            Self::Pr05GraphImpactAndAudit => "依赖爆炸半径与断链自愈",
            Self::Pr06GranularityNoiseGate => "黑白名单与噪音门禁阻断",
            Self::Pr07LockDeadlockGuard => "多 Agent 协作锁与 WFG 死锁防护",
            Self::Pr08AdaptiveBriefColdStart => "冷启动自适应态势简报组装",
            Self::Pr09HybridSemanticSearch => "语义向量 + FTS RRF 混合检索",
            Self::Pr10FeedbackAndMemoryLoop => "使用反馈与长期记忆自动沉淀",
            Self::Pr11OptimisticLeaseAndRecovery => "AOPCC 乐观直通租约与碰壁秒级自愈",
            Self::Pr12AuditDrivenSelfEvolution => "双域行为审计溯源与数据驱动自进化",
            Self::Pr13CrossBrainContinuitySaved => "跨大脑连续性接力与本体器官自省",
        }
    }

    pub fn formula_desc(&self) -> &'static str {
        match self {
            Self::Pr01SkillMcpRouting => {
                "baseline_tokens = sum(skills_catalog.est_tokens) + sum(mcp_catalog.est_tokens); steward_tokens = estimate_tokens(routed_response)"
            }
            Self::Pr02ProgressiveAssetSearch => {
                "baseline_tokens = sum(min(matched_asset.size_bytes, 65536) / 4); steward_tokens = estimate_tokens(response)"
            }
            Self::Pr03PreHookInterception => {
                "On BLOCK/WARN: hazard_intercepted = 1, rounds_saved = 2, baseline_tokens = 600"
            }
            Self::Pr04HostFactQuery => {
                "baseline_tokens = calibrated_w1_w2_tokens (or 2x floor), baseline_rounds = 4, steward_rounds = 1"
            }
            Self::Pr05GraphImpactAndAudit => {
                "baseline_tokens = min(managed_assets, 500) * 12, hazard_intercepted = dangling_links_found"
            }
            Self::Pr06GranularityNoiseGate => {
                "noise_blocked = shallow_blocked_files + noise_filtered_files; baseline_tokens = noise_blocked * 25"
            }
            Self::Pr07LockDeadlockGuard => {
                "On Denied/WaitQueued/Deadlock: hazard_intercepted = cycles_or_conflict, rounds_saved = 2"
            }
            Self::Pr08AdaptiveBriefColdStart => {
                "baseline_rounds = 6, steward_rounds = 1, baseline_tokens = calibrated_w8_tokens (or 1800 floor)"
            }
            Self::Pr09HybridSemanticSearch => {
                "baseline_tokens = max(steward_tokens * 4, 600), baseline_rounds = 3, steward_rounds = 1"
            }
            Self::Pr10FeedbackAndMemoryLoop => {
                "baseline_tokens = max(steward_tokens * 3, 450), baseline_rounds = 2, steward_rounds = 1"
            }
            Self::Pr11OptimisticLeaseAndRecovery => {
                "Optimistic lease hits save 1 turn/step (C_turn); on abort PostToolUse(success=false) heals in <2ms, rounds_saved = 2"
            }
            Self::Pr12AuditDrivenSelfEvolution => {
                "IN_AGENT + OUT_OF_AGENT audit coverage 100% (redacted & SHA-256), lease calibration saves repeated miss rounds, rounds_saved = 2"
            }
            Self::Pr13CrossBrainContinuitySaved => {
                "Cross-brain handoff capsule (<=300 tok, redacted & sanitized) + 6-organ BodyManifest saves cold re-discovery, rounds_saved = 3"
            }
        }
    }

    pub fn list_all() -> Vec<ProvenanceRuleMeta> {
        let all = [
            (Self::Pr01SkillMcpRouting, "steward_skill_route, steward_mcp_catalog, steward_service_contact"),
            (Self::Pr02ProgressiveAssetSearch, "steward_asset_search, steward_asset_register"),
            (Self::Pr03PreHookInterception, "steward_hook_pre"),
            (Self::Pr04HostFactQuery, "steward_fact_query, steward_port_check, steward_env_processes, steward_env_show"),
            (Self::Pr05GraphImpactAndAudit, "steward_links_impact, steward_links_audit, steward_links_repair, steward_codegraph_bridge"),
            (Self::Pr06GranularityNoiseGate, "steward_asset_scan, Scanner"),
            (Self::Pr07LockDeadlockGuard, "steward_lock_acquire, steward_lock_wait, steward_lock_deadlock_check"),
            (Self::Pr08AdaptiveBriefColdStart, "steward_brief"),
            (Self::Pr09HybridSemanticSearch, "steward_memory_search, memory search --hybrid"),
            (Self::Pr10FeedbackAndMemoryLoop, "steward_feedback, steward_memory_add, steward_session_close"),
            (Self::Pr11OptimisticLeaseAndRecovery, "steward_brief(leases), steward_hook_pre(mode), steward_hook_post(success=false)"),
            (Self::Pr12AuditDrivenSelfEvolution, "steward_audit_query, AuditLogger, AuditEvolutionAnalyzer"),
            (Self::Pr13CrossBrainContinuitySaved, "steward_body_state, BodyManifestEngine, ContinuityManager, HomeostasisVitalsEngine"),
        ];
        all.iter()
            .map(|(r, tools)| ProvenanceRuleMeta {
                code: r.code().to_string(),
                name: r.title().to_string(),
                applies_to: (*tools).to_string(),
                formula: r.formula_desc().to_string(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_and_sha256() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcdefgh"), 2);
        // 中文按 ceil(chars * 1.5)
        assert_eq!(estimate_tokens("你好世界"), 6);
        let h1 = sha256_hex("agent-steward");
        let h2 = sha256_hex("agent-steward");
        assert_eq!(h1.len(), 64);
        assert_eq!(h1, h2);
    }
}
