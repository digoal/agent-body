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
            let is_already_covered = existing_dirs.iter().any(|d| *d == parent_dir);
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
        Ok(rule)
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
}
