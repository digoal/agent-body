use std::collections::HashMap;
use std::path::Path;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::storage::Storage;
use crate::storage::models::PlacementRule;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnedRuleCandidate {
    pub candidate_id: String,
    pub rule_name: String,
    pub pattern_type: String, // "extension" | "category"
    pub target_base_dir: String,
    pub naming_pattern: String,
    pub description: String,
    pub sample_count: usize,
    pub confidence: f64,
    pub sample_paths: Vec<String>,
}

/// 用户或 Agent 纠偏行为记录实体
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorrectionFeedback {
    /// 纠正前文件散落路径（如 /tmp/sec_crawler.py 或 ./test.py）
    pub original_path: String,
    /// 纠正后规范目标路径（如 /root/new/shared_proj/sec_crawler/main.py）
    pub corrected_path: String,
    /// 可选意图说明
    pub intent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LearnReport {
    pub total_analyzed_assets: usize,
    pub candidate_count: usize,
    pub candidates: Vec<LearnedRuleCandidate>,
}

pub struct PlacementLearner<'a> {
    storage: &'a Storage,
}

impl<'a> PlacementLearner<'a> {
    pub fn new(storage: &'a Storage) -> Self {
        Self { storage }
    }

    /// 从资产库与变更历史中自动聚类挖掘潜在的放置规范
    pub fn learn_rules(&self, min_samples: usize) -> Result<LearnReport> {
        let existing_rules = self.storage.get_placement_rules()?;
        let existing_dirs: Vec<String> = existing_rules
            .iter()
            .map(|r| r.target_base_dir.trim_end_matches('/').to_string())
            .collect();

        let assets = self.storage.list_assets(5000)?;
        let total_analyzed_assets = assets.len();

        // 统计 (parent_dir, extension) 频次
        let mut cluster_map: HashMap<(String, String), Vec<String>> = HashMap::new();

        for asset in &assets {
            let path = Path::new(&asset.path);
            let parent_dir = match path.parent().and_then(|p| p.to_str()) {
                Some(p) => p.trim_end_matches('/').to_string(),
                None => continue,
            };

            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            if ext.is_empty() {
                continue;
            }

            cluster_map
                .entry((parent_dir, ext))
                .or_default()
                .push(asset.path.clone());
        }

        let mut candidates = Vec::new();

        for ((parent_dir, ext), samples) in cluster_map {
            if samples.len() < min_samples {
                continue;
            }

            // 若已有规则完全覆盖该基准目录，则跳过或作为补充扩展名
            let is_already_covered = existing_dirs.contains(&parent_dir);
            if is_already_covered {
                continue;
            }

            // 计算置信度：样本数越多且扩展名高度收敛，置信度越高
            let count = samples.len();
            let confidence = (0.70 + (count as f64 * 0.05)).min(0.98);

            let dir_name = Path::new(&parent_dir)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("custom");

            let rule_name = format!("learned_{}_{}", dir_name, ext);
            let target_base_dir = format!("{}/", parent_dir);
            let naming_pattern = format!("{{name}}.{}", ext);
            let description = format!(
                "自动学习沉淀规则: 在 {} 集中存放 .{} 格式资产 (置信度: {:.2}, 样本量: {})",
                target_base_dir, ext, confidence, count
            );

            candidates.push(LearnedRuleCandidate {
                candidate_id: format!("cand_{}_{}", dir_name, ext),
                rule_name,
                pattern_type: "extension".to_string(),
                target_base_dir,
                naming_pattern,
                description,
                sample_count: count,
                confidence,
                sample_paths: samples.into_iter().take(5).collect(),
            });
        }

        // 按置信度与样本数降序排序
        candidates.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let candidate_count = candidates.len();
        Ok(LearnReport {
            total_analyzed_assets,
            candidate_count,
            candidates,
        })
    }

    /// 将挖掘出的候选规则持久化到 placement_rules 表
    pub fn apply_candidate(&self, candidate: &LearnedRuleCandidate) -> Result<PlacementRule> {
        let rule = PlacementRule {
            id: format!("rule_{}", candidate.rule_name),
            rule_name: candidate.rule_name.clone(),
            pattern_type: candidate.pattern_type.clone(),
            target_base_dir: candidate.target_base_dir.clone(),
            naming_pattern: Some(candidate.naming_pattern.clone()),
            description: Some(candidate.description.clone()),
            guideline: Some(format!(
                "自学习规范: 请统一存放至 {}，命名采用 kebab-case 规范",
                candidate.target_base_dir
            )),
            priority: 50, // 动态学习规则优先级
        };

        self.storage.insert_placement_rule(&rule)?;
        crate::audit::AuditLogger::record_best_effort(
            self.storage,
            crate::audit::AuditEventInput::out_of_agent(
                "governance_engine",
                "placement",
                "rule_apply",
                "apply_learned_rule",
            )
            .with_target("placement_rule", rule.rule_name.clone())
            .with_outcome("success")
            .with_delta(serde_json::json!({
                "rule_name": rule.rule_name,
                "target_base_dir": rule.target_base_dir,
                "sample_count": candidate.sample_count,
                "confidence": candidate.confidence,
            })),
        );
        Ok(rule)
    }

    /// 从用户人工纠偏记录中分析归纳规则 (Feedback-driven Rule Learning)
    pub fn learn_from_corrections(&self, corrections: &[CorrectionFeedback]) -> Vec<LearnedRuleCandidate> {
        let mut mapping_counts: HashMap<(String, String), Vec<String>> = HashMap::new();

        for corr in corrections {
            let orig = Path::new(&corr.original_path);
            let corr_p = Path::new(&corr.corrected_path);

            let ext = corr_p
                .extension()
                .or_else(|| orig.extension())
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            let target_parent = match corr_p.parent().and_then(|p| p.to_str()) {
                Some(p) => p.trim_end_matches('/').to_string(),
                None => continue,
            };

            mapping_counts
                .entry((target_parent, ext))
                .or_default()
                .push(corr.corrected_path.clone());
        }

        let mut candidates = Vec::new();
        for ((target_dir, ext), samples) in mapping_counts {
            let count = samples.len();
            // 人工纠正具有极高意图确定性，基准置信度直接给 0.90+
            let confidence = (0.90 + (count as f64 * 0.03)).min(0.99);

            let dir_name = Path::new(&target_dir)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or("custom");

            let ext_label = if ext.is_empty() { "file" } else { &ext };
            let rule_name = format!("correction_{}_{}", dir_name, ext_label);
            let target_base_dir = format!("{}/", target_dir);
            let naming_pattern = if ext.is_empty() { "{name}/".to_string() } else { format!("{{name}}.{}", ext) };

            candidates.push(LearnedRuleCandidate {
                candidate_id: format!("cand_corr_{}_{}", dir_name, ext_label),
                rule_name,
                pattern_type: "correction".to_string(),
                target_base_dir: target_base_dir.clone(),
                naming_pattern,
                description: format!(
                    "根据用户人工纠偏自演化: 用户偏好将此类资产统一存放于 {} (置信度: {:.2}, 纠偏次数: {})",
                    target_base_dir, confidence, count
                ),
                sample_count: count,
                confidence,
                sample_paths: samples.into_iter().take(5).collect(),
            });
        }

        candidates.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap_or(std::cmp::Ordering::Equal));
        candidates
    }

    /// 将规则候选集导出为标准 .agent-body-rules.toml 文件内容
    pub fn export_as_toml(candidates: &[LearnedRuleCandidate]) -> String {
        let mut out = String::new();
        out.push_str("# 自动演进生成的项目放置规范 (.agent-body-rules.toml)\n\n");
        for c in candidates {
            out.push_str("[[rules]]\n");
            out.push_str(&format!("object_type = \"{}\"\n", c.pattern_type));
            out.push_str(&format!("target_sub_dir = \"{}\"\n", c.target_base_dir.trim_matches('/')));
            out.push_str(&format!("naming_pattern = \"{}\"\n", c.naming_pattern));
            out.push_str(&format!("guideline = \"{}\"\n\n", c.description));
        }
        out
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use tempfile::tempdir;
    use crate::storage::models::AssetRecord;

    #[test]
    fn test_placement_learner_clustering_and_apply() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("test_learn.db");
        let storage = Storage::open(&db_path)?;

        // 插入 3 个新目录下的 parquet 文件资产
        let custom_dir = "/root/new/datasets";
        for i in 1..=3 {
            let asset = AssetRecord {
                id: format!("asset_ds_{}", i),
                path: format!("{}/dataset_{}.parquet", custom_dir, i),
                asset_type: "dataset".to_string(),
                owner: "tester".to_string(),
                security_level: "internal".to_string(),
                scan_tier: "deep".to_string(),
                description: Some("parquet dataset".to_string()),
                desc_kv: serde_json::json!({}),
                usage: None,
                size_bytes: Some(1024),
                file_count: Some(1),
                tags: vec![],
                status: "active".to_string(),
            };
            storage.upsert_asset(&asset)?;
        }

        let learner = PlacementLearner::new(&storage);

        // 1. 挖掘规则
        let report = learner.learn_rules(2)?;
        assert!(report.candidate_count >= 1);
        let cand = &report.candidates[0];
        assert_eq!(cand.target_base_dir, "/root/new/datasets/");
        assert_eq!(cand.sample_count, 3);
        assert!(cand.confidence >= 0.85);

        // 2. 采纳应用规则
        let rule = learner.apply_candidate(cand)?;
        assert_eq!(rule.rule_name, "learned_datasets_parquet");

        // 3. 验证规则已入库
        let rules = storage.get_placement_rules()?;
        let found = rules.iter().any(|r| r.rule_name == "learned_datasets_parquet");
        assert!(found);

        Ok(())
    }

    #[test]
    fn test_learn_from_corrections() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("test_corr.db");
        let storage = Storage::open(&db_path)?;
        let learner = PlacementLearner::new(&storage);

        let feedback = vec![
            CorrectionFeedback {
                original_path: "/tmp/script1.py".to_string(),
                corrected_path: "/root/new/shared_proj/tools/script1.py".to_string(),
                intent: Some("脚本移至 shared_proj".to_string()),
            },
            CorrectionFeedback {
                original_path: "./my_crawler.py".to_string(),
                corrected_path: "/root/new/shared_proj/tools/my_crawler.py".to_string(),
                intent: Some("抓取脚本整理".to_string()),
            },
        ];

        let candidates = learner.learn_from_corrections(&feedback);
        assert!(!candidates.is_empty());
        let top = &candidates[0];
        assert_eq!(top.target_base_dir, "/root/new/shared_proj/tools/");
        assert_eq!(top.sample_count, 2);
        assert!(top.confidence >= 0.90);

        let toml_str = PlacementLearner::export_as_toml(&candidates);
        assert!(toml_str.contains(".agent-body-rules.toml"));
        assert!(toml_str.contains("shared_proj/tools"));

        Ok(())
    }
}
