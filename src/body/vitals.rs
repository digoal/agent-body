//! 五维本体生命体征 (`HomeostasisVitals`) 与 `L1_AUTONOMIC` 反射稳态自愈

use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use sysinfo::Disks;

use crate::audit::{AuditEventInput, AuditEvolutionAnalyzer, AuditLogger};
use crate::body::envelope::AutonomyEnvelope;
use crate::body::manifest::compute_body_id;
use crate::coordination::CoordinationEngine;
use crate::feedback::probes::FreshnessReconciler;
use crate::scanner::metadata::SecretRedactor;
use crate::storage::models::AuditQueryFilter;
use crate::storage::{Storage, StorageProvider};

/// 单维度生命体征
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitalDimension {
    pub key: String,
    pub label: String,
    pub weight_pct: u8,
    pub score: f64,
    pub status: String,
    pub detail: String,
}

/// 五维综合生命体征评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeostasisVitals {
    pub body_id: String,
    pub overall_score: f64,
    pub status: String,
    pub dimensions: Vec<VitalDimension>,
    pub recommended_l1_actions: Vec<String>,
    pub assessed_at: String,
}

/// L1 稳态自愈执行报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomeostasisHealReport {
    pub body_id: String,
    pub before_score: f64,
    pub after_score: f64,
    pub delta_score: f64,
    pub expired_locks_cleaned: usize,
    pub downgraded_leases: usize,
    pub reconciled_missing_assets: usize,
    pub pruned_ghost_nodes: usize,
    pub actions_executed: Vec<String>,
    pub envelope_level: String,
}

pub struct HomeostasisVitalsEngine;

impl HomeostasisVitalsEngine {
    fn status_from_score(score: f64) -> String {
        if score >= 85.0 {
            "healthy".to_string()
        } else if score >= 65.0 {
            "degraded".to_string()
        } else {
            "critical".to_string()
        }
    }

    /// 评估宿主机本体的五维稳态生命体征 (0..=100)
    pub fn assess(storage: &Storage) -> HomeostasisVitals {
        Self::assess_provider(storage.provider().as_ref())
    }

    /// 基于 `&dyn StorageProvider` 直接评估五维稳态生命体征 (供 DQ8 等借用路径免包装调用)
    pub fn assess_provider(storage: &dyn StorageProvider) -> HomeostasisVitals {
        let body_id = compute_body_id();
        let assessed_at = Local::now().format("%Y-%m-%dT%H:%M:%S+08:00").to_string();
        let mut recommended_l1_actions = Vec::new();

        // 1. 感官清晰度 (25%): 资产描述 + 向量回填 + 事实图谱项目连通度
        let assets = storage.list_assets(2000).unwrap_or_default();
        let fact_nodes = storage.list_all_fact_nodes(2000).unwrap_or_default();
        let fact_edges = storage.list_all_fact_edges(4000).unwrap_or_default();
        let sensory_score = if assets.is_empty() && fact_nodes.is_empty() {
            100.0
        } else {
            let desc_pct = if assets.is_empty() {
                100.0
            } else {
                let with_desc = assets
                    .iter()
                    .filter(|a| a.description.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false))
                    .count();
                (with_desc as f64 / assets.len() as f64) * 100.0
            };
            let proj_nodes: Vec<&str> = fact_nodes
                .iter()
                .filter(|n| n.node_type == "project")
                .map(|n| n.id.as_str())
                .collect();
            let conn_pct = if proj_nodes.is_empty() {
                100.0
            } else {
                let connected = proj_nodes
                    .iter()
                    .filter(|pid| fact_edges.iter().any(|e| e.source_id == **pid || e.target_id == **pid))
                    .count();
                (connected as f64 / proj_nodes.len() as f64) * 100.0
            };
            ((desc_pct * 0.6 + conn_pct * 0.4) * 100.0).round() / 100.0
        };
        if sensory_score < 75.0 {
            recommended_l1_actions.push("触发增量向量回填与 L2_STRUCTURAL 图谱上卷".to_string());
        }

        // 2. 反射灵敏度 (20%): 24h AOPCC 租约健康度与碰壁降级比例
        let audits_24h = storage
            .query_audit_events(&AuditQueryFilter {
                domain: Some("IN_AGENT".to_string()),
                window: Some("24h".to_string()),
                limit: 300,
                ..Default::default()
            })
            .unwrap_or_default();
        let lease_hits = audits_24h.iter().filter(|a| a.outcome == "success").count();
        let lease_misses = audits_24h
            .iter()
            .filter(|a| a.category == "hook_post_recovery" || a.outcome == "failed")
            .count();
        let reflex_samples = lease_hits + lease_misses;
        let reflex_score = if reflex_samples == 0 {
            100.0
        } else {
            (((lease_hits as f64 / reflex_samples as f64) * 100.0) * 100.0).round() / 100.0
        };
        if reflex_score < 75.0 {
            recommended_l1_actions.push("执行 lease_calibrate 收紧高频碰壁乐观租约".to_string());
        }

        // 3. 海马体纯净度 (20%): 记忆无明文凭据 + 接力胶囊无泄露 + 记忆矛盾状态
        let memories = storage.list_all_active_memories(500).unwrap_or_default();
        let checkpoints = storage.list_body_checkpoints(Some("active"), 50).unwrap_or_default();
        let total_mem_items = memories.len() + checkpoints.len();
        let memory_score = if total_mem_items == 0 {
            100.0
        } else {
            let mut clean = 0usize;
            for m in &memories {
                if SecretRedactor::redact_preserving_newlines(&m.content) == m.content {
                    clean += 1;
                }
            }
            for cp in &checkpoints {
                let raw = format!("{} {}", cp.goal_summary, cp.next_step_hint);
                if SecretRedactor::redact_single_line(&raw) == raw {
                    clean += 1;
                }
            }
            (((clean as f64 / total_mem_items as f64) * 100.0) * 100.0).round() / 100.0
        };
        if memory_score < 80.0 {
            recommended_l1_actions.push("执行 freshness_reconcile 与记忆冲突归档".to_string());
        }

        // 4. 代谢健康度 (20%): 磁盘余量 + 过期协作锁清理健康度
        let disks = Disks::new_with_refreshed_list();
        let max_used_pct = disks
            .list()
            .iter()
            .filter(|d| d.total_space() > 0)
            .map(|d| {
                let used = d.total_space().saturating_sub(d.available_space());
                (used as f64 / d.total_space() as f64) * 100.0
            })
            .fold(0.0_f64, f64::max);
        let disk_score = if max_used_pct >= 95.0 {
            50.0
        } else if max_used_pct >= 90.0 {
            70.0
        } else {
            100.0
        };
        let locks = storage.list_coordination_locks(None).unwrap_or_default();
        let now_iso = chrono::Utc::now().to_rfc3339();
        let expired_locks = locks.iter().filter(|l| l.expires_at < now_iso).count();
        let lock_penalty = (expired_locks as f64 * 5.0).min(30.0);
        let metabolic_score = ((disk_score - lock_penalty).clamp(0.0, 100.0) * 100.0).round() / 100.0;
        if metabolic_score < 75.0 {
            recommended_l1_actions.push("执行 expired_lock_gc 回收过期锁并检查 HostGC dry-run".to_string());
        }

        // 5. 免疫可溯度 (15%): 双域审计 SHA-256 完整率与脱敏卫生
        let audits = storage
            .query_audit_events(&AuditQueryFilter {
                limit: 200,
                ..Default::default()
            })
            .unwrap_or_default();
        let immune_score = if audits.is_empty() {
            100.0
        } else {
            let valid = audits
                .iter()
                .filter(|a| {
                    (a.domain == "IN_AGENT" || a.domain == "OUT_OF_AGENT")
                        && a.evidence_sha256.len() == 64
                        && SecretRedactor::redact_single_line(&a.target_id) == a.target_id
                })
                .count();
            (((valid as f64 / audits.len() as f64) * 100.0) * 100.0).round() / 100.0
        };
        if immune_score < 85.0 {
            recommended_l1_actions.push("审计免疫可溯度偏低，检查双域脱敏与 SHA-256 留痕".to_string());
        }

        let overall_raw = sensory_score * 0.25
            + reflex_score * 0.20
            + memory_score * 0.20
            + metabolic_score * 0.20
            + immune_score * 0.15;
        let overall_score = (overall_raw * 100.0).round() / 100.0;

        let dimensions = vec![
            VitalDimension {
                key: "sensory_clarity".to_string(),
                label: "感官清晰度".to_string(),
                weight_pct: 25,
                score: sensory_score,
                status: Self::status_from_score(sensory_score),
                detail: format!("资产 {} 条 · 节点 {} · 边 {}", assets.len(), fact_nodes.len(), fact_edges.len()),
            },
            VitalDimension {
                key: "reflex_accuracy".to_string(),
                label: "反射灵敏度".to_string(),
                weight_pct: 20,
                score: reflex_score,
                status: Self::status_from_score(reflex_score),
                detail: format!("24h 反射样本 {} 条 (命中 {} / 碰壁 {})", reflex_samples, lease_hits, lease_misses),
            },
            VitalDimension {
                key: "memory_purity".to_string(),
                label: "海马体纯净度".to_string(),
                weight_pct: 20,
                score: memory_score,
                status: Self::status_from_score(memory_score),
                detail: format!("记忆 {} 条 · 活跃胶囊 {} 个", memories.len(), checkpoints.len()),
            },
            VitalDimension {
                key: "metabolic_health".to_string(),
                label: "代谢健康度".to_string(),
                weight_pct: 20,
                score: metabolic_score,
                status: Self::status_from_score(metabolic_score),
                detail: format!("最高磁盘使用率 {:.1}% · 过期锁 {} 个", max_used_pct, expired_locks),
            },
            VitalDimension {
                key: "immune_traceability".to_string(),
                label: "免疫可溯度".to_string(),
                weight_pct: 15,
                score: immune_score,
                status: Self::status_from_score(immune_score),
                detail: format!("抽检最近 {} 条双域审计流水", audits.len()),
            },
        ];

        HomeostasisVitals {
            body_id,
            overall_score,
            status: Self::status_from_score(overall_score),
            dimensions,
            recommended_l1_actions,
            assessed_at,
        }
    }

    /// 严格在 `L1_AUTONOMIC` 包络内执行可逆稳态自愈反射（绝不物理删文件或改写代码）
    pub fn run_l1_autonomic_heal(storage: &Storage) -> Result<HomeostasisHealReport> {
        let decision = AutonomyEnvelope::evaluate("homeostasis_heal", None, false);
        if !decision.allowed {
            anyhow::bail!("自治包络拒绝稳态自愈: {}", decision.reason);
        }

        let before = Self::assess(storage);
        let mut actions_executed = Vec::new();

        // 1. 回收过期协作锁
        let coord = CoordinationEngine::new(storage.provider().clone());
        let expired_locks_cleaned = coord.gc_expired_locks().unwrap_or(0);
        if expired_locks_cleaned > 0 {
            actions_executed.push(format!("expired_lock_gc: 清理 {} 个过期锁", expired_locks_cleaned));
        }

        // 2. 审计驱动 AOPCC 租约校准
        let evo = AuditEvolutionAnalyzer::run_evolution_cycle(storage, None, false, 3)?;
        let downgraded_leases = evo.downgraded_targets.len();
        if downgraded_leases > 0 || !evo.rehabilitated_targets.is_empty() {
            actions_executed.push(format!(
                "lease_calibrate: 降级 {} / 康复 {} 个租约目标",
                downgraded_leases,
                evo.rehabilitated_targets.len()
            ));
        }

        // 3. 新鲜度现实对账（写穿失效、剪枝幽灵节点、归档重复记忆）
        let rec = FreshnessReconciler::reconcile(storage)?;
        if rec.missing_assets_marked > 0
            || rec.ghost_fact_nodes_pruned > 0
            || rec.duplicate_memories_archived > 0
        {
            actions_executed.push(format!(
                "freshness_reconcile: missing={}, ghosts={}, dup_mems={}",
                rec.missing_assets_marked,
                rec.ghost_fact_nodes_pruned,
                rec.duplicate_memories_archived
            ));
        }

        let after = Self::assess(storage);
        let delta_score = ((after.overall_score - before.overall_score) * 100.0).round() / 100.0;

        AuditLogger::record_best_effort(
            storage,
            AuditEventInput::out_of_agent(
                "governance_engine",
                "body",
                "homeostasis",
                "l1_autonomic_heal",
            )
            .with_target("body", &before.body_id)
            .with_concurrency_mode("L1_AUTONOMIC")
            .with_outcome("success")
            .with_delta(serde_json::json!({
                "before_score": before.overall_score,
                "after_score": after.overall_score,
                "delta_score": delta_score,
                "expired_locks_cleaned": expired_locks_cleaned,
                "downgraded_leases": downgraded_leases,
                "ghost_fact_nodes_pruned": rec.ghost_fact_nodes_pruned,
                "actions": actions_executed,
            })),
        );

        Ok(HomeostasisHealReport {
            body_id: before.body_id,
            before_score: before.overall_score,
            after_score: after.overall_score,
            delta_score,
            expired_locks_cleaned,
            downgraded_leases,
            reconciled_missing_assets: rec.missing_assets_marked,
            pruned_ghost_nodes: rec.ghost_fact_nodes_pruned,
            actions_executed,
            envelope_level: decision.level_code,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::CoordinationLock;

    #[test]
    fn test_vitals_assess_and_l1_heal_recovers_expired_locks() {
        let storage = Storage::open_in_memory().expect("open");
        // 注入一条已过期协作锁拉低代谢分
        let expired_at = (chrono::Utc::now() - chrono::Duration::seconds(300)).to_rfc3339();
        let acquired_at = (chrono::Utc::now() - chrono::Duration::seconds(600)).to_rfc3339();
        storage
            .insert_coordination_lock(&CoordinationLock {
                lock_id: "lock-expired-body-test".to_string(),
                resource_type: "port".to_string(),
                resource_id: "9999".to_string(),
                scope: "exclusive".to_string(),
                holder_agent: "dead-brain".to_string(),
                holder_pid: None,
                acquired_at,
                expires_at: expired_at,
                metadata: None,
            })
            .expect("insert expired lock");

        let before = HomeostasisVitalsEngine::assess(&storage);
        assert_eq!(before.dimensions.len(), 5);
        assert!(before.overall_score <= 100.0);

        let heal = HomeostasisVitalsEngine::run_l1_autonomic_heal(&storage).expect("heal");
        assert_eq!(heal.envelope_level, "L1_AUTONOMIC");
        assert_eq!(heal.expired_locks_cleaned, 1);
        assert!(heal.after_score >= heal.before_score);
    }
}
