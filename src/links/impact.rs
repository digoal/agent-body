use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::storage::Storage;
use crate::links::codegraph_bridge::CodeGraphBridge;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependentNode {
    pub node_type: String, // "asset" | "service" | "memory" | "cron"
    pub node_id: String,
    pub relation: String,
    pub depth: usize,
    pub confidence: f32,
    pub line_number: Option<i32>,
    pub snippet: Option<String>,
    pub provenance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactAnalysisResult {
    pub target: String,
    pub risk_level: String, // "LOW" | "MEDIUM" | "HIGH" | "CRITICAL"
    pub direct_dependents: Vec<DependentNode>,
    pub indirect_dependents: Vec<DependentNode>,
    pub total_dependents_count: usize,
    pub has_codegraph: bool,
    pub codegraph_db: Option<String>,
    pub summary: String,
}

pub struct ImpactAnalyzer {
    storage: Arc<Storage>,
}

impl ImpactAnalyzer {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    /// 分析指定目标对象（资产路径、服务或记忆）被谁所依赖（逆向爆炸半径推演，含精准出处与 CodeGraph 探测）
    pub fn analyze_impact(&self, target_id: &str, max_depth: usize) -> Result<ImpactAnalysisResult> {
        let all_links = self.storage.list_all_links(1000)?;

        let mut direct_dependents = Vec::new();
        let mut indirect_dependents = Vec::new();
        let mut visited = HashSet::new();

        // 探测目标是否处于某个拥有 CodeGraph 代码图的项目中
        let target_path = Path::new(target_id);
        let codegraph_db = CodeGraphBridge::find_codegraph_db(target_path)
            .map(|p| p.to_string_lossy().to_string());
        let has_codegraph = codegraph_db.is_some();

        // BFS 队列: (current_id, current_depth)
        let mut queue = VecDeque::new();
        visited.insert(target_id.to_string());
        queue.push_back((target_id.to_string(), 0));

        while let Some((curr_id, depth)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            // 在所有 links 中查找哪些 source 指向了当前 target:
            // 即: link.target_id == curr_id，说明 link.source 依赖于 curr_id
            for link in &all_links {
                if link.target_id == curr_id && !visited.contains(&link.source_id) {
                    visited.insert(link.source_id.clone());
                    let next_depth = depth + 1;

                    let node = DependentNode {
                        node_type: link.source_type.clone(),
                        node_id: link.source_id.clone(),
                        relation: link.relation.clone(),
                        depth: next_depth,
                        confidence: link.confidence,
                        line_number: link.line_number,
                        snippet: link.snippet.clone(),
                        provenance: link.provenance.clone(),
                    };

                    if next_depth == 1 {
                        direct_dependents.push(node.clone());
                    } else {
                        indirect_dependents.push(node.clone());
                    }

                    queue.push_back((link.source_id.clone(), next_depth));
                }
            }
        }

        let total_count = direct_dependents.len() + indirect_dependents.len();

        // 评估风险等级
        let is_shared_or_core = target_id.contains("shared_proj") || target_id.contains("postgresql") || target_id.contains("config");
        let risk_level = if total_count >= 5 || (total_count >= 2 && is_shared_or_core) {
            "CRITICAL"
        } else if total_count >= 3 || (total_count >= 1 && is_shared_or_core) {
            "HIGH"
        } else if total_count >= 1 {
            "MEDIUM"
        } else {
            "LOW"
        };

        let codegraph_tip = if has_codegraph {
            " [支持 CodeGraph 微观函数级下钻]"
        } else {
            ""
        };

        let summary = if total_count == 0 {
            format!("✅ 未检测到任何上下游资产或服务依赖 '{}'。改动或删除此对象爆炸半径极小 (Risk: LOW)。{}", target_id, codegraph_tip)
        } else {
            format!(
                "⚠️ 发现 {} 个直接依赖与 {} 个多跳间接下游依赖于 '{}'。综合风险评级: {}！{}改动需确保接口向后兼容或提前迁移调用方。",
                direct_dependents.len(), indirect_dependents.len(), target_id, risk_level, codegraph_tip
            )
        };

        Ok(ImpactAnalysisResult {
            target: target_id.to_string(),
            risk_level: risk_level.to_string(),
            direct_dependents,
            indirect_dependents,
            total_dependents_count: total_count,
            has_codegraph,
            codegraph_db,
            summary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::links::LinksEngine;

    #[test]
    fn test_impact_analysis_multi_depth() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let engine = LinksEngine::new(storage.clone());
        let analyzer = ImpactAnalyzer::new(storage);

        // 构建依赖链路:
        // AppA (script) -> depends_on -> CoreUtil (shared_proj)
        // CronTask (other) -> depends_on -> AppA (script)
        engine.create_link("asset", "/root/new/work/app_a.py", "asset", "/root/new/shared_proj/core_util.py", "depends_on", 1.0).unwrap();
        engine.create_link("asset", "/root/new/cron/daily_sync.py", "asset", "/root/new/work/app_a.py", "depends_on", 0.9).unwrap();

        // 评估 CoreUtil 的爆炸半径
        let res = analyzer.analyze_impact("/root/new/shared_proj/core_util.py", 3).unwrap();
        assert_eq!(res.direct_dependents.len(), 1, "AppA 应当为直接依赖");
        assert_eq!(res.direct_dependents[0].node_id, "/root/new/work/app_a.py");

        assert_eq!(res.indirect_dependents.len(), 1, "CronTask 应当为间接依赖");
        assert_eq!(res.indirect_dependents[0].node_id, "/root/new/cron/daily_sync.py");

        assert_eq!(res.total_dependents_count, 2);
        assert_eq!(res.risk_level, "CRITICAL"); // 属于 shared_proj 且依赖数 >= 2
    }
}
