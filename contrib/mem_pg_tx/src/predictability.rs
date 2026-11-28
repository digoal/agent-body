//! 查询可预测性门禁与自动旁路器 (`QueryPredictabilityGate` · Phase 77 M2.3)
//!
//! 规避 Sleep-Time Compute 在探索型/低可预测查询下因过度预压缩导致的细节失真：
//! - 高可预测、高摘要覆盖度的常规主题查询 -> 命中高密度 `sleep_compacted` 摘要（省 Token 3x~5x）；
//! - 探索型、罕见诊断标识符、多符号栈帧或历史时态查询 -> 自动旁路预整理摘要，直查原始活跃事件流，确保 `ΔRecall >= 0`。

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// 查询可预测性与旁路判定结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictabilityDecision {
    pub predictability_score: f32,
    pub bypass_compacted: bool,
    pub reason: String,
    pub lexical_overlap: f32,
    pub exploratory_signals: Vec<String>,
}

pub struct QueryPredictabilityGate;

impl QueryPredictabilityGate {
    /// 评估查询是否属于高可预测常规主题，或应自动旁路预整理摘要直查原始细节
    pub fn evaluate(
        query: &str,
        summary_corpus: &[&str],
        as_of_epoch: Option<i64>,
        include_history: bool,
    ) -> PredictabilityDecision {
        if as_of_epoch.is_some() || include_history {
            return PredictabilityDecision {
                predictability_score: 0.10,
                bypass_compacted: true,
                reason: "显式请求双时态时间旅行或全历史版本，自动旁路预整理摘要".to_string(),
                lexical_overlap: 0.0,
                exploratory_signals: vec!["temporal_history_request".to_string()],
            };
        }

        let q = query.trim();
        if q.is_empty() {
            return PredictabilityDecision {
                predictability_score: 0.90,
                bypass_compacted: false,
                reason: "空查询/默认上下文加载，优先使用高密度睡眠整理摘要".to_string(),
                lexical_overlap: 1.0,
                exploratory_signals: vec![],
            };
        }

        let mut signals = Vec::new();
        let q_lower = q.to_ascii_lowercase();
        if q.contains("::") {
            signals.push("rust_symbol_path".to_string());
        }
        if q_lower.contains("0x") {
            signals.push("hex_address_or_code".to_string());
        }
        if q_lower.contains("sqlstate")
            || q_lower.contains("err_")
            || q_lower.contains("panic")
            || q_lower.contains("errno")
        {
            signals.push("low_level_diagnostic_code".to_string());
        }
        if q.contains('/') && q.contains('.') {
            signals.push("exact_file_or_line_ref".to_string());
        }
        if has_rare_identifier_pattern(q) {
            signals.push("rare_specific_identifier".to_string());
        }

        let q_terms = extract_terms(q);
        let overlap = if summary_corpus.is_empty() || q_terms.is_empty() {
            0.0
        } else {
            let mut summary_vocab = HashSet::new();
            for s in summary_corpus {
                for term in extract_terms(s) {
                    summary_vocab.insert(term);
                }
            }
            let hit = q_terms.iter().filter(|t| summary_vocab.contains(*t)).count();
            hit as f32 / q_terms.len() as f32
        };

        if summary_corpus.is_empty() {
            return PredictabilityDecision {
                predictability_score: 0.35,
                bypass_compacted: true,
                reason: "尚无已整理的睡眠摘要，直查原始记忆事件流".to_string(),
                lexical_overlap: 0.0,
                exploratory_signals: signals,
            };
        }

        let signal_penalty = (signals.len() as f32 * 0.25).min(0.75);
        let predictability_score = (0.30 + 0.70 * overlap - signal_penalty).clamp(0.0, 1.0);
        let bypass_compacted = !signals.is_empty() && overlap < 0.75 || predictability_score < 0.45;

        let reason = if bypass_compacted {
            format!(
                "检测到不可预测/细粒度探索查询 (overlap={:.2}, signals={:?})，自动旁路预整理摘要直查原始事件",
                overlap, signals
            )
        } else {
            format!(
                "高可预测常规主题查询 (overlap={:.2}, score={:.2})，命中高密度睡眠摘要省 Token",
                overlap, predictability_score
            )
        };

        PredictabilityDecision {
            predictability_score,
            bypass_compacted,
            reason,
            lexical_overlap: overlap,
            exploratory_signals: signals,
        }
    }
}

fn has_rare_identifier_pattern(q: &str) -> bool {
    for tok in q.split_whitespace() {
        let clean = tok.trim_matches(|c: char| !c.is_alphanumeric() && c != '_');
        if clean.len() >= 14 && clean.contains('_') {
            return true;
        }
        let upper_internal = clean.chars().skip(1).filter(|c| c.is_ascii_uppercase()).count();
        if clean.len() >= 10 && upper_internal >= 2 {
            return true;
        }
    }
    false
}

fn extract_terms(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c <= '\u{7f}')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| s.len() >= 2)
        .collect()
}
