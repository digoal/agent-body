//! 审计驱动自进化引擎 (`AuditEvolutionAnalyzer`) — Phase 43.4
//!
//! 职责：
//! 1. **AOPCC 租约滑动窗口动态校准**：依据近 24h `audit_events` 中 `hook_post_recovery` 碰壁与
//!    后续成功/康复序列，计算经验有效率 `p_valid` 并驱动 `OptimisticLeaseEngine` 自动降级与康复回升；
//! 2. **摩擦热点与错放目录聚类**：识别高频 `failed`/`blocked`/`warn` 动作自动去重上报 `user_feedback.md`，
//!    并从文件变更审计中聚类未覆盖高频目录输出 `PlacementLearner` 规则候选。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::audit::{AuditEventInput, AuditLogger};
use crate::feedback::FeedbackEngine;
use crate::placement::learner::{LearnedRuleCandidate, PlacementLearner};
use crate::storage::models::AuditQueryFilter;
use crate::storage::Storage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseHealthEval {
    pub target: String,
    pub total_samples: usize,
    pub success_count: usize,
    pub miss_count: usize,
    pub success_since_last_miss: usize,
    pub empirical_p_valid: f32,
    pub should_downgrade: bool,
    pub rehabilitated: bool,
    pub recommended_confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrictionHotspot {
    pub category: String,
    pub action: String,
    pub target_id: String,
    pub hit_count: usize,
    pub last_outcome: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvolutionReport {
    pub evaluated_events: usize,
    pub lease_evaluations: Vec<LeaseHealthEval>,
    pub downgraded_targets: Vec<String>,
    pub rehabilitated_targets: Vec<String>,
    pub friction_hotspots: Vec<FrictionHotspot>,
    pub feedback_items_created: Vec<String>,
    pub placement_candidates: Vec<LearnedRuleCandidate>,
}

pub struct AuditEvolutionAnalyzer;

impl AuditEvolutionAnalyzer {
    /// 评估单个目标（如 `port:5432`、`tool:wkhtmltopdf`）在时间窗内的经验有效率与升降级建议
    pub fn evaluate_lease_health(
        storage: &Storage,
        target: &str,
        window: &str,
    ) -> LeaseHealthEval {
        let rows = storage
            .query_audit_events(&AuditQueryFilter {
                domain: Some("IN_AGENT".to_string()),
                target_contains: Some(target.to_string()),
                window: Some(window.to_string()),
                limit: 200,
                ..Default::default()
            })
            .unwrap_or_default();

        evaluate_target_rows(target, &rows)
    }

    /// 批量收集近 24h `audit_events` 中存在碰壁/恢复记录的租约调整映射
    pub fn collect_audit_lease_adjustments(storage: &Storage) -> HashMap<String, LeaseHealthEval> {
        let rows = storage
            .query_audit_events(&AuditQueryFilter {
                domain: Some("IN_AGENT".to_string()),
                window: Some("24h".to_string()),
                limit: 500,
                ..Default::default()
            })
            .unwrap_or_default();

        let mut grouped: HashMap<String, Vec<crate::storage::models::AuditEventRecord>> =
            HashMap::new();
        for r in rows {
            if r.target_id.trim().is_empty() {
                continue;
            }
            if r.category == "hook_post_recovery"
                || r.category == "hook_post_exec"
                || r.category == "hook_pre_guard"
                || r.category == "tool_call"
            {
                grouped.entry(r.target_id.clone()).or_default().push(r);
            }
        }

        let mut out = HashMap::new();
        for (target, t_rows) in grouped {
            let eval = evaluate_target_rows(&target, &t_rows);
            if eval.should_downgrade || eval.rehabilitated {
                out.insert(target, eval);
            }
        }
        out
    }

    /// 执行一次全量审计驱动自进化周期：租约滑窗校准 + 摩擦热点沉淀 + 目录聚类规则挖掘
    pub fn run_evolution_cycle(
        storage: &Storage,
        fb_path: Option<&Path>,
        auto_feedback: bool,
        min_hotspot_hits: usize,
    ) -> Result<AuditEvolutionReport> {
        let t0 = std::time::Instant::now();
        let rows = storage.query_audit_events(&AuditQueryFilter {
            window: Some("7d".to_string()),
            limit: 2000,
            ..Default::default()
        })?;
        let evaluated_events = rows.len();

        // 1. 按 target 聚合 AOPCC 租约健康度
        let mut by_target: HashMap<String, Vec<crate::storage::models::AuditEventRecord>> =
            HashMap::new();
        // 2. 摩擦热点聚合 (category, action, target_id) -> (count, last_outcome)
        let mut hotspot_counts: HashMap<(String, String, String), (usize, String)> = HashMap::new();
        // 3. 常规工作区文件写目录聚合，用于未覆盖放置规则聚类
        let mut dir_counts: HashMap<(String, String), Vec<String>> = HashMap::new();

        for r in &rows {
            if r.domain == "IN_AGENT" && !r.target_id.trim().is_empty() {
                by_target
                    .entry(r.target_id.clone())
                    .or_default()
                    .push(r.clone());
            }

            if r.outcome == "failed" || r.outcome == "blocked" || r.outcome == "warn" {
                let key = (r.category.clone(), r.action.clone(), r.target_id.clone());
                let entry = hotspot_counts.entry(key).or_insert((0, r.outcome.clone()));
                entry.0 += 1;
            }

            if (r.category == "hook_post_exec" || r.category == "fs_watch_sync")
                && r.target_type == "file"
                && r.target_id.starts_with('/')
                && !r.target_id.starts_with("/root/new/tmp")
                && !r.target_id.starts_with("/tmp")
            {
                let p = Path::new(&r.target_id);
                if let (Some(parent), Some(ext)) = (
                    p.parent().and_then(|d| d.to_str()),
                    p.extension().and_then(|e| e.to_str()),
                ) {
                    dir_counts
                        .entry((parent.trim_end_matches('/').to_string(), ext.to_lowercase()))
                        .or_default()
                        .push(r.target_id.clone());
                }
            }
        }

        let mut lease_evaluations = Vec::new();
        let mut downgraded_targets = Vec::new();
        let mut rehabilitated_targets = Vec::new();
        for (target, t_rows) in by_target {
            let eval = evaluate_target_rows(&target, &t_rows);
            if eval.miss_count > 0 || eval.success_count >= 3 {
                if eval.should_downgrade {
                    downgraded_targets.push(target.clone());
                }
                if eval.rehabilitated {
                    rehabilitated_targets.push(target.clone());
                }
                lease_evaluations.push(eval);
            }
        }

        let mut friction_hotspots: Vec<FrictionHotspot> = hotspot_counts
            .into_iter()
            .filter(|(_, (cnt, _))| *cnt >= min_hotspot_hits.max(1))
            .map(|((cat, act, tgt), (cnt, last_out))| FrictionHotspot {
                category: cat,
                action: act,
                target_id: tgt,
                hit_count: cnt,
                last_outcome: last_out,
            })
            .collect();
        friction_hotspots.sort_by(|a, b| b.hit_count.cmp(&a.hit_count));

        let mut feedback_items_created = Vec::new();
        if auto_feedback {
            if let Some(fb) = fb_path {
                for hs in friction_hotspots.iter().take(3) {
                    let title = format!(
                        "[AuditHotspot:{}] `{}` 高频{} ({}次)",
                        hs.category, hs.action, hs.last_outcome, hs.hit_count
                    );
                    let evidence = format!(
                        "近 7 天 audit_events 捕获 target=`{}` 累计 {} 次 `{}`",
                        hs.target_id, hs.hit_count, hs.last_outcome
                    );
                    let rec = "核查前置约束或通过 `agent-body audit evolve` 调整租约/规则".to_string();
                    if let Ok((item, created)) =
                        FeedbackEngine::add_item_deduped(fb, "CON", "P1", &title, &evidence, &rec)
                    {
                        if created {
                            feedback_items_created.push(item.id);
                        }
                    }
                }
            }
        }

        // 结合 PlacementLearner 过滤已有规则目录，生成新目录候选
        let existing_dirs: HashSet<String> = storage
            .get_placement_rules()
            .unwrap_or_default()
            .into_iter()
            .map(|r| r.target_base_dir.trim_end_matches('/').to_string())
            .collect();
        let mut placement_candidates = Vec::new();
        for ((parent_dir, ext), samples) in dir_counts {
            if samples.len() < 3 || existing_dirs.iter().any(|d| parent_dir.starts_with(d)) {
                continue;
            }
            let dir_name = Path::new(&parent_dir)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("custom");
            let conf = (0.75 + samples.len() as f64 * 0.04).min(0.96);
            placement_candidates.push(LearnedRuleCandidate {
                candidate_id: format!("aud_cand_{}_{}", dir_name, ext),
                rule_name: format!("audit_learned_{}_{}", dir_name, ext),
                pattern_type: "extension".to_string(),
                target_base_dir: format!("{}/", parent_dir),
                naming_pattern: format!("{{name}}.{}", ext),
                description: format!(
                    "基于审计流聚类: `{}/` 高频产出 .{} 文件 ({} 次)",
                    parent_dir,
                    ext,
                    samples.len()
                ),
                sample_count: samples.len(),
                confidence: conf,
                sample_paths: samples.into_iter().take(5).collect(),
            });
        }
        let _ = PlacementLearner::new(storage); // 保持与 PlacementLearner 类型体系对齐

        let report = AuditEvolutionReport {
            evaluated_events,
            lease_evaluations,
            downgraded_targets,
            rehabilitated_targets,
            friction_hotspots,
            feedback_items_created,
            placement_candidates,
        };

        AuditLogger::record_best_effort(
            storage,
            AuditEventInput::out_of_agent(
                "governance_engine",
                "audit_evolution",
                "audit_evolution",
                "run_evolution_cycle",
            )
            .with_target("table", "audit_events")
            .with_outcome("success")
            .with_elapsed_us(t0.elapsed().as_micros() as i64)
            .with_delta(serde_json::json!({
                "evaluated_events": report.evaluated_events,
                "downgraded": report.downgraded_targets.len(),
                "rehabilitated": report.rehabilitated_targets.len(),
                "hotspots": report.friction_hotspots.len(),
                "placement_candidates": report.placement_candidates.len(),
            })),
        );

        Ok(report)
    }
}

/// 基于按时间倒序 (`created_at DESC`) 的事件序列计算经验有效率与升降级结论
fn evaluate_target_rows(
    target: &str,
    rows_desc: &[crate::storage::models::AuditEventRecord],
) -> LeaseHealthEval {
    let total_samples = rows_desc.len();
    let mut success_count = 0usize;
    let mut miss_count = 0usize;
    let mut success_since_last_miss = 0usize;
    let mut seen_miss = false;

    for r in rows_desc {
        let is_miss = r.category == "hook_post_recovery"
            || r.outcome == "failed"
            || r.outcome == "blocked";
        if is_miss {
            miss_count += 1;
            seen_miss = true;
        } else if r.outcome == "success" {
            success_count += 1;
            if !seen_miss {
                success_since_last_miss += 1;
            }
        }
    }

    let empirical_p_valid = (success_count as f32 + 1.0) / (total_samples as f32 + 2.0);
    let should_downgrade = miss_count >= 1 && success_since_last_miss < 2;
    let rehabilitated = miss_count >= 1 && success_since_last_miss >= 2;
    let recommended_confidence = if should_downgrade {
        Some(0.55)
    } else if rehabilitated {
        Some(0.90)
    } else if miss_count == 0 && success_count >= 3 {
        Some((0.92 + empirical_p_valid * 0.06).min(0.99))
    } else {
        None
    };

    LeaseHealthEval {
        target: target.to_string(),
        total_samples,
        success_count,
        miss_count,
        success_since_last_miss,
        empirical_p_valid,
        should_downgrade,
        rehabilitated,
        recommended_confidence,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_evolution_downgrades_and_rehabilitates_lease() {
        let storage = Storage::open_in_memory().expect("open_in_memory");

        // 1. 写入一次碰壁自愈事件 -> 应判定降级
        AuditLogger::record(
            &storage,
            AuditEventInput::in_agent("hook", "hook_post_recovery", "run_command")
                .with_target("tool_target", "port:5432")
                .with_outcome("recovered"),
        )
        .unwrap();

        let eval1 = AuditEvolutionAnalyzer::evaluate_lease_health(&storage, "port:5432", "24h");
        assert!(eval1.should_downgrade, "首次碰壁且未连续成功应降级");
        assert!(!eval1.rehabilitated);

        // 2. 连续写入两次成功直连 -> 应判定康复回升
        for _ in 0..2 {
            AuditLogger::record(
                &storage,
                AuditEventInput::in_agent("hook", "hook_post_exec", "run_command")
                    .with_target("tool_target", "port:5432")
                    .with_outcome("success"),
            )
            .unwrap();
        }

        let eval2 = AuditEvolutionAnalyzer::evaluate_lease_health(&storage, "port:5432", "24h");
        assert!(!eval2.should_downgrade);
        assert!(eval2.rehabilitated, "连续两次成功后应判定康复");
        assert_eq!(eval2.recommended_confidence, Some(0.90));
    }

    #[test]
    fn test_audit_evolution_cycle_hotspots_and_feedback_dedup() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let dir = tempfile::tempdir().expect("tempdir");
        let fb = dir.path().join("user_feedback.md");
        std::fs::write(&fb, "# 反馈总账\n\n## 七、第五部分：后续新增反馈标准模板\n").unwrap();

        for _ in 0..3 {
            AuditLogger::record(
                &storage,
                AuditEventInput::in_agent("hook", "hook_pre_guard", "write_file")
                    .with_target("tool_arg", "/etc/sudoers")
                    .with_outcome("blocked"),
            )
            .unwrap();
        }

        let rep = AuditEvolutionAnalyzer::run_evolution_cycle(&storage, Some(&fb), true, 3)
            .expect("evolution cycle");
        assert!(!rep.friction_hotspots.is_empty());
        assert_eq!(rep.friction_hotspots[0].hit_count, 3);
        assert_eq!(rep.feedback_items_created.len(), 1);

        // 再次运行应去重不再重复生成
        let rep2 = AuditEvolutionAnalyzer::run_evolution_cycle(&storage, Some(&fb), true, 3)
            .expect("evolution cycle 2");
        assert!(rep2.feedback_items_created.is_empty());
    }
}
