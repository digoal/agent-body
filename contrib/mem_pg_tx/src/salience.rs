//! STC 突触显著性评分引擎 (`SalienceScorer` · Phase 77 M2.1)
//!
//! 受 Synaptic Tagging and Capture (STC) 理论启发，纯确定性评估记忆事件保持高保真还是纳入空闲睡眠整理：
//! - 综合证据层级 + 记忆类型基础分、语料词项新颖度、跨会话复用频次、人工/探针修正冲突权重与时间衰减；
//! - 严格区分核心架构决策/显式规则 (`>= 0.70` High) 与高频低值 `auto_sync`/噪声 (`< 0.35` Low)。

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::engine::{map_event_row, PgTxEngine};

/// 突触显著性分级阈值
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SalienceTier {
    High,
    Medium,
    Low,
}

impl SalienceTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }
}

/// 单条记忆的显著性打分输入要素
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalienceInput {
    pub content: String,
    pub memory_type: String,
    pub evidence_tier: String,
    pub cross_session_reuses: u32,
    pub conflict_corrections: u32,
    pub novelty_ratio: f32,
    pub age_hours: f32,
    pub is_auto_sync: bool,
    pub pinned: bool,
}

/// 分项评分明细（可解释、可审计）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalienceBreakdown {
    pub base_weight: f32,
    pub novelty_boost: f32,
    pub reuse_boost: f32,
    pub correction_boost: f32,
    pub decay_factor: f32,
    pub auto_sync_penalty: f32,
}

/// 单条显著性评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalienceScore {
    pub score: f32,
    pub tier: SalienceTier,
    pub components: SalienceBreakdown,
}

/// 批量重算 PG 活跃事件 `salience_score` 的回执
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalienceRescoreReport {
    pub tenant_scope: String,
    pub scanned: usize,
    pub updated: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
    pub avg_score: f32,
}

pub struct SalienceScorer;

impl SalienceScorer {
    /// 确定性计算 STC 突触显著性评分 (`0.05..=1.00`)
    pub fn score(input: &SalienceInput) -> SalienceScore {
        let mtype = input.memory_type.trim().to_ascii_lowercase();
        let tier_upper = input.evidence_tier.trim().to_ascii_uppercase();

        let type_base: f32 = match mtype.as_str() {
            "decision" => 0.36,
            "preference" => 0.34,
            "experience" => 0.28,
            "fact" => 0.24,
            "task" => 0.20,
            _ => 0.14,
        };
        let tier_bonus: f32 = match tier_upper.as_str() {
            "L1_EXPLICIT" | "L1" => 0.14,
            "L2_STRUCTURAL" | "L2" => 0.08,
            _ => 0.02,
        };
        let base_weight = type_base + tier_bonus;

        let novelty = input.novelty_ratio.clamp(0.0, 1.0);
        let novelty_boost = 0.22 * novelty;
        let reuse_boost = 0.20 * (1.0 - (-(input.cross_session_reuses as f32) / 3.0).exp());
        let corr_capped = (input.conflict_corrections.min(3) as f32) / 3.0;
        let correction_boost = 0.18 * corr_capped;

        let half_life_hours = if input.is_auto_sync {
            48.0
        } else if mtype == "decision" || tier_upper == "L1_EXPLICIT" || tier_upper == "L1" {
            336.0
        } else {
            168.0
        };
        let age = input.age_hours.max(0.0);
        let decay_factor = (0.5_f32).powf(age / half_life_hours).clamp(0.30, 1.0);
        let auto_sync_penalty = if input.is_auto_sync { 0.58 } else { 1.0 };

        let raw = (base_weight + novelty_boost + reuse_boost + correction_boost)
            * decay_factor
            * auto_sync_penalty;
        let mut final_score = (raw * 1000.0).round() / 1000.0;
        if input.pinned && final_score < 0.85 {
            final_score = 0.85;
        }
        final_score = final_score.clamp(0.05, 1.0);

        let tier = if final_score >= 0.70 {
            SalienceTier::High
        } else if final_score >= 0.35 {
            SalienceTier::Medium
        } else {
            SalienceTier::Low
        };

        SalienceScore {
            score: final_score,
            tier,
            components: SalienceBreakdown {
                base_weight,
                novelty_boost,
                reuse_boost,
                correction_boost,
                decay_factor,
                auto_sync_penalty,
            },
        }
    }

    /// 计算目标文本相对于同类语料的新颖度 (`1.0 - max_jaccard`)
    pub fn compute_corpus_novelty(target: &str, peers: &[&str]) -> f32 {
        let target_tokens = tokenize_terms(target);
        if target_tokens.is_empty() {
            return 0.0;
        }
        let mut max_sim: f32 = 0.0;
        for peer in peers {
            let p_tokens = tokenize_terms(peer);
            if p_tokens.is_empty() {
                continue;
            }
            let inter = target_tokens.intersection(&p_tokens).count();
            let union = target_tokens.union(&p_tokens).count();
            if union > 0 {
                let sim = inter as f32 / union as f32;
                if sim > max_sim {
                    max_sim = sim;
                }
            }
        }
        (1.0 - max_sim).clamp(0.05, 1.0)
    }

    /// 判断内容是否属于低价值自动流水/脚手架噪声
    pub fn looks_like_auto_sync(fact_key: &str, content: &str, tags: &[String]) -> bool {
        let fk_lower = fact_key.to_ascii_lowercase();
        let c_lower = content.to_ascii_lowercase();
        fk_lower.starts_with("auto_sync:")
            || fk_lower.starts_with("episodic:")
            || c_lower.contains("auto_sync")
            || c_lower.contains("产出文件 `")
            || tags.iter().any(|t| {
                let tl = t.to_ascii_lowercase();
                tl == "auto_sync" || tl == "episodic" || tl == "noise_log"
            })
    }
}

fn tokenize_terms(text: &str) -> HashSet<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c <= '\u{7f}' && c != ':')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| s.len() >= 2)
        .collect()
}

impl PgTxEngine {
    /// 批量评估并持久化 PG 活跃事件的 STC `salience_score`
    pub fn rescore_salience(&self, tenant_scope: &str, limit: usize) -> Result<SalienceRescoreReport> {
        let scope = if tenant_scope.trim().is_empty() {
            self.config.tenant_scope.as_str()
        } else {
            tenant_scope.trim()
        };
        let lim = limit.clamp(1, 2000) as i64;
        let mut client = self.pool.checkout()?;

        let rows = client.query(
            "SELECT event_id, body_id, tenant_scope, fact_key, memory_type,
                    evidence_tier, payload_redacted, evidence_sha256,
                    lower(valid_range) AS valid_from,
                    CASE WHEN upper_inf(valid_range) THEN NULL ELSE upper(valid_range) END AS valid_to,
                    tx_epoch, embedding_status, salience_score, consolidation_st, created_at,
                    1.0::double precision AS rank_score
             FROM ext_mem_pg_tx__events
             WHERE tenant_scope = $1
               AND upper_inf(valid_range)
               AND embedding_status <> 'tombstoned'
             ORDER BY tx_epoch DESC
             LIMIT $2",
            &[&scope, &lim],
        )?;
        let events: Vec<_> = rows.iter().map(map_event_row).collect();
        if events.is_empty() {
            return Ok(SalienceRescoreReport {
                tenant_scope: scope.to_string(),
                scanned: 0,
                updated: 0,
                high_count: 0,
                medium_count: 0,
                low_count: 0,
                avg_score: 0.0,
            });
        }

        // 统计同 fact_key 历史版本频次（用于衡量修正冲突/演化活跃度）与前缀复用频次
        let ver_rows = client.query(
            "SELECT fact_key, COUNT(*)::bigint
             FROM ext_mem_pg_tx__events
             WHERE tenant_scope = $1
             GROUP BY fact_key",
            &[&scope],
        )?;
        let mut version_counts: HashMap<String, u32> = HashMap::new();
        for vr in ver_rows {
            let fk: String = vr.get(0);
            let cnt: i64 = vr.get(1);
            version_counts.insert(fk, cnt.max(0) as u32);
        }

        let now = Utc::now();
        let all_contents: Vec<&str> = events.iter().map(|e| e.content.as_str()).collect();
        let mut high_count = 0usize;
        let mut medium_count = 0usize;
        let mut low_count = 0usize;
        let mut sum_score = 0.0f32;
        let mut updates: Vec<(i64, f32)> = Vec::with_capacity(events.len());

        for (idx, ev) in events.iter().enumerate() {
            let mut peers: Vec<&str> = Vec::with_capacity(all_contents.len().saturating_sub(1));
            for (j, text) in all_contents.iter().enumerate() {
                if j != idx {
                    peers.push(*text);
                }
            }
            let novelty_ratio = SalienceScorer::compute_corpus_novelty(&ev.content, &peers);
            let age_hours = DateTime::parse_from_rfc3339(&ev.created_at)
                .map(|dt| (now - dt.with_timezone(&Utc)).num_minutes().max(0) as f32 / 60.0)
                .unwrap_or(1.0);
            let versions = version_counts.get(&ev.fact_key).copied().unwrap_or(1);
            let corrections = versions.saturating_sub(1);
            let is_auto = SalienceScorer::looks_like_auto_sync(&ev.fact_key, &ev.content, &ev.tags);

            let assessed = SalienceScorer::score(&SalienceInput {
                content: ev.content.clone(),
                memory_type: ev.memory_type.clone(),
                evidence_tier: ev.evidence_tier.clone(),
                cross_session_reuses: if !is_auto && ev.evidence_tier == "L1_EXPLICIT" {
                    2
                } else {
                    0
                },
                conflict_corrections: corrections,
                novelty_ratio,
                age_hours,
                is_auto_sync: is_auto,
                pinned: false,
            });
            match assessed.tier {
                SalienceTier::High => high_count += 1,
                SalienceTier::Medium => medium_count += 1,
                SalienceTier::Low => low_count += 1,
            }
            sum_score += assessed.score;
            updates.push((ev.event_id, assessed.score));
        }

        let mut updated = 0usize;
        let mut tx = client.transaction()?;
        for (eid, new_score) in updates {
            let n = tx.execute(
                "UPDATE ext_mem_pg_tx__events SET salience_score = $1 WHERE event_id = $2",
                &[&new_score, &eid],
            )?;
            updated += n as usize;
        }
        tx.commit()?;

        let scanned = events.len();
        Ok(SalienceRescoreReport {
            tenant_scope: scope.to_string(),
            scanned,
            updated,
            high_count,
            medium_count,
            low_count,
            avg_score: ((sum_score / scanned.max(1) as f32) * 1000.0).round() / 1000.0,
        })
    }
}
