use std::collections::{HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::storage::Storage;
use crate::links::codegraph_bridge::CodeGraphBridge;

fn default_dep_weight() -> f32 {
    1.0
}

fn default_dep_tier() -> String {
    "L1_EXPLICIT".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependentNode {
    pub node_type: String, // "asset" | "service" | "memory" | "cron" | "project"
    pub node_id: String,
    pub relation: String,
    pub depth: usize,
    pub confidence: f32,
    #[serde(default = "default_dep_weight")]
    pub weight: f32,
    #[serde(default = "default_dep_weight")]
    pub effective_score: f32,
    #[serde(default = "default_dep_tier")]
    pub evidence_tier: String,
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

    /// 分析指定目标对象（资产路径、服务或记忆）被谁所依赖（默认仅计 L1/L2 物理与结构强依赖，min_weight=0.50）
    pub fn analyze_impact(&self, target_id: &str, max_depth: usize) -> Result<ImpactAnalysisResult> {
        self.analyze_impact_weighted(target_id, max_depth, 0.50, false)
    }

    /// 多维证据加权爆炸半径推演（融合 links 与 fact_edges，支持跳数衰减 0.8^(hop-1) 与 L3 隐式语义开关）
    pub fn analyze_impact_weighted(
        &self,
        target_id: &str,
        max_depth: usize,
        min_weight: f32,
        include_semantic: bool,
    ) -> Result<ImpactAnalysisResult> {
        let all_links = self.storage.list_all_links(2000)?;
        let all_fact_edges = self.storage.list_all_fact_edges(5000).unwrap_or_default();

        // 统一候选边模型: (source_id, source_type, target_id, relation, weight, tier, line_number, snippet, provenance)
        struct UnifiedEdge {
            src: String,
            src_type: String,
            tgt: String,
            relation: String,
            weight: f32,
            tier: String,
            line_number: Option<i32>,
            snippet: Option<String>,
            provenance: Option<String>,
        }

        let mut unified: Vec<UnifiedEdge> = Vec::with_capacity(all_links.len() + all_fact_edges.len());
        for l in all_links {
            if l.confidence >= min_weight {
                unified.push(UnifiedEdge {
                    src: l.source_id,
                    src_type: l.source_type,
                    tgt: l.target_id,
                    relation: l.relation,
                    weight: l.confidence,
                    tier: "L1_EXPLICIT".to_string(),
                    line_number: l.line_number,
                    snippet: l.snippet,
                    provenance: l.provenance,
                });
            }
        }
        for fe in all_fact_edges {
            if fe.weight < min_weight {
                continue;
            }
            if !include_semantic && fe.evidence_tier == "L3_SEMANTIC" {
                continue;
            }
            let src_type = fe.source_id.split(':').next().unwrap_or("fact").to_string();
            unified.push(UnifiedEdge {
                src: fe.source_id,
                src_type,
                tgt: fe.target_id,
                relation: fe.relation,
                weight: fe.weight,
                tier: fe.evidence_tier,
                line_number: None,
                snippet: fe.properties.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string()),
                provenance: Some("property_graph".to_string()),
            });
        }

        let mut direct_dependents = Vec::new();
        let mut indirect_dependents = Vec::new();
        let mut visited = HashSet::new();

        // 探测目标是否处于某个拥有 CodeGraph 代码图的项目中
        let target_path = Path::new(target_id);
        let codegraph_db = CodeGraphBridge::find_codegraph_db(target_path)
            .map(|p| p.to_string_lossy().to_string());
        let has_codegraph = codegraph_db.is_some();

        // 兼容带前缀(project:/file:/service:)与裸路径双向匹配
        let mut queue = VecDeque::new();
        visited.insert(target_id.to_string());
        queue.push_back((target_id.to_string(), 0usize, 1.0_f32));

        let matches_target = |candidate: &str, curr: &str| -> bool {
            if candidate == curr {
                return true;
            }
            if let Some(stripped) = curr.strip_prefix("file:").or_else(|| curr.strip_prefix("project:")) {
                if candidate == stripped {
                    return true;
                }
            }
            if let Some(stripped) = candidate.strip_prefix("file:").or_else(|| candidate.strip_prefix("project:")) {
                if stripped == curr {
                    return true;
                }
            }
            false
        };

        const HOP_DECAY: f32 = 0.80;
        while let Some((curr_id, depth, acc_score)) = queue.pop_front() {
            if depth >= max_depth {
                continue;
            }

            for edge in &unified {
                // 正向依赖: edge.tgt == curr_id => edge.src 受 curr_id 影响
                // 对称语义(semantically_related): 双向均可发现
                let hit_neighbor = if matches_target(&edge.tgt, &curr_id) {
                    Some((&edge.src, &edge.src_type))
                } else if edge.relation == "semantically_related" && matches_target(&edge.src, &curr_id) {
                    Some((&edge.tgt, &edge.src_type))
                } else {
                    None
                };

                if let Some((neighbor_id, neighbor_type)) = hit_neighbor {
                    if visited.contains(neighbor_id) {
                        continue;
                    }
                    visited.insert(neighbor_id.clone());
                    let next_depth = depth + 1;
                    let eff_score = (acc_score * edge.weight * HOP_DECAY.powi((next_depth as i32) - 1)).clamp(0.0, 1.0);

                    let node = DependentNode {
                        node_type: neighbor_type.clone(),
                        node_id: neighbor_id.clone(),
                        relation: edge.relation.clone(),
                        depth: next_depth,
                        confidence: edge.weight,
                        weight: edge.weight,
                        effective_score: (eff_score * 1000.0).round() / 1000.0,
                        evidence_tier: edge.tier.clone(),
                        line_number: edge.line_number,
                        snippet: edge.snippet.clone(),
                        provenance: edge.provenance.clone(),
                    };

                    if next_depth == 1 {
                        direct_dependents.push(node);
                    } else {
                        indirect_dependents.push(node);
                    }
                    queue.push_back((neighbor_id.clone(), next_depth, eff_score));
                }
            }
        }

        direct_dependents.sort_by(|a, b| b.effective_score.partial_cmp(&a.effective_score).unwrap_or(std::cmp::Ordering::Equal));
        indirect_dependents.sort_by(|a, b| b.effective_score.partial_cmp(&a.effective_score).unwrap_or(std::cmp::Ordering::Equal));

        let strong_count = direct_dependents.iter().chain(indirect_dependents.iter())
            .filter(|d| d.evidence_tier != "L3_SEMANTIC")
            .count();
        let total_count = direct_dependents.len() + indirect_dependents.len();

        // 评估风险等级（仅按强物理/结构依赖评估，避免语义弱关联虚高风险）
        let is_shared_or_core = target_id.contains("shared_proj") || target_id.contains("postgresql") || target_id.contains("config");
        let risk_level = if strong_count >= 5 || (strong_count >= 2 && is_shared_or_core) {
            "CRITICAL"
        } else if strong_count >= 3 || (strong_count >= 1 && is_shared_or_core) {
            "HIGH"
        } else if strong_count >= 1 {
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
            let top_score = direct_dependents.first().map(|d| d.effective_score).unwrap_or(0.0);
            format!(
                "⚠️ 发现 {} 个直接依赖与 {} 个多跳间接下游依赖于 '{}'（强依赖 {} 个，最高有效权重 {:.2}）。综合风险评级: {}！{}改动需确保接口向后兼容或提前迁移调用方。",
                direct_dependents.len(), indirect_dependents.len(), target_id, strong_count, top_score, risk_level, codegraph_tip
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

    #[test]
    fn test_weighted_impact_fuses_fact_edges_and_filters_semantic() {
        use crate::storage::models::{FactEdgeRecord, FactNodeRecord};

        let storage = Arc::new(Storage::open_in_memory().expect("mem"));
        for (id, ntype) in [
            ("project:core", "project"),
            ("project:downstream", "project"),
            ("project:semantic_peer", "project"),
        ] {
            storage
                .upsert_fact_node(&FactNodeRecord {
                    id: id.into(),
                    node_type: ntype.into(),
                    name: id.into(),
                    label: id.into(),
                    properties: serde_json::json!({}),
                    updated_at: None,
                })
                .expect("node");
        }

        storage
            .upsert_fact_edge(&FactEdgeRecord {
                id: None,
                source_id: "project:downstream".into(),
                target_id: "project:core".into(),
                relation: "depends_on".into(),
                properties: serde_json::json!({"reason": "L2 rollup"}),
                confidence: 0.75,
                evidence_tier: "L2_STRUCTURAL".into(),
                weight: 0.75,
                direction_kind: "forward_dependency".into(),
                updated_at: None,
            })
            .expect("l2");
        storage
            .upsert_fact_edge(&FactEdgeRecord {
                id: None,
                source_id: "project:semantic_peer".into(),
                target_id: "project:core".into(),
                relation: "semantically_related".into(),
                properties: serde_json::json!({"reason": "L3 semantic"}),
                confidence: 0.48,
                evidence_tier: "L3_SEMANTIC".into(),
                weight: 0.48,
                direction_kind: "symmetric_affinity".into(),
                updated_at: None,
            })
            .expect("l3");

        let analyzer = ImpactAnalyzer::new(storage);
        // 默认分析应排除 L3 语义弱关联
        let strict = analyzer.analyze_impact("project:core", 2).expect("strict");
        assert_eq!(strict.direct_dependents.len(), 1);
        assert_eq!(strict.direct_dependents[0].node_id, "project:downstream");

        // 显式放开 L3 且降低权重阈值时应全部命中，且强边排在前面
        let full = analyzer
            .analyze_impact_weighted("project:core", 2, 0.35, true)
            .expect("full");
        assert_eq!(full.direct_dependents.len(), 2);
        assert!(full.direct_dependents[0].effective_score > full.direct_dependents[1].effective_score);
    }
}
