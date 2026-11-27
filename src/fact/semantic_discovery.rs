//! 多维证据分层关系挖掘器 (Phase 40.3 ~ 40.4 / 规约 #23)
//!
//! 职责：
//! 1. `L2_STRUCTURAL` 结构聚合派生：
//!    - 文件级 `links` 上卷至项目级有向加权边（对数饱和模型，防止单一日志刷爆权重）；
//!    - Systemd unit 与运行时进程 `runs_as` 同体桥接 & `exec_start` 工程归属边；
//!    - 工程/服务/定时任务对宿主机 CLI 工具的 `uses_tool` 依赖边；
//!    - 源码工程与专栏/文档工程的 `documented_by` 衍生关系边。
//! 2. `L3_SEMANTIC` 隐式语义发现：
//!    - 利用 `fact_nodes` 本地 Ollama Embedding 余弦相似度 (>= 0.78) 与标签/域名 Jaccard 混合打分；
//!    - 每节点严格 `Top-K <= 3` 稀疏剪枝，严禁退化为全连接稠密网。

use std::collections::{HashMap, HashSet};
use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::storage::models::{FactEdgeRecord, FactNodeRecord};
use crate::storage::Storage;

/// 单次关系挖掘报告
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MiningReport {
    pub l2_rollup_edges: usize,
    pub l2_service_bridge_edges: usize,
    pub l2_tool_usage_edges: usize,
    pub l2_derivation_edges: usize,
    pub l3_semantic_edges: usize,
}

impl MiningReport {
    pub fn total_edges(&self) -> usize {
        self.l2_rollup_edges
            + self.l2_service_bridge_edges
            + self.l2_tool_usage_edges
            + self.l2_derivation_edges
            + self.l3_semantic_edges
    }
}

/// 宿主机图谱多维关系挖掘器
pub struct GraphRelationshipMiner<'a> {
    storage: &'a Storage,
}

impl<'a> GraphRelationshipMiner<'a> {
    pub fn new(storage: &'a Storage) -> Self {
        Self { storage }
    }

    /// 执行 L2 结构上卷与（可选）L3 隐式语义边挖掘，幂等清理旧派生边后重建
    pub fn mine_all(&self, include_semantic: bool) -> Result<MiningReport> {
        let mut report = MiningReport::default();

        // 幂等清理既有 L2 边并重建
        if let Err(e) = self.storage.delete_fact_edges_by_tier("L2_STRUCTURAL") {
            eprintln!("[SemanticDiscovery][WARN] 清理旧 L2_STRUCTURAL 边失败: {}", e);
        }

        let nodes = self.storage.list_all_fact_nodes(2000)?;
        let existing_edges = self.storage.list_all_fact_edges(5000)?;

        let project_prefixes = collect_project_prefixes(&nodes);
        let mut existing_pairs: HashSet<(String, String)> = existing_edges
            .iter()
            .filter(|e| e.evidence_tier == "L1_EXPLICIT")
            .map(|e| (e.source_id.clone(), e.target_id.clone()))
            .collect();

        report.l2_rollup_edges = self.mine_project_rollup_links(&project_prefixes, &mut existing_pairs)?;
        report.l2_service_bridge_edges =
            self.mine_service_bridges(&nodes, &project_prefixes, &mut existing_pairs)?;
        report.l2_tool_usage_edges = self.mine_tool_usage(&nodes, &mut existing_pairs)?;
        report.l2_derivation_edges = self.mine_derivation_edges(&nodes, &mut existing_pairs)?;

        if include_semantic {
            if let Err(e) = self.storage.delete_fact_edges_by_tier("L3_SEMANTIC") {
                eprintln!("[SemanticDiscovery][WARN] 清理旧 L3_SEMANTIC 边失败: {}", e);
            }
            report.l3_semantic_edges = self.mine_semantic_edges(&nodes, &existing_pairs, 3)?;
        }

        Ok(report)
    }

    /// 1. 将微观文件级 `links` 按目录归属上卷聚合成项目级 `L2_STRUCTURAL` 有向加权边
    fn mine_project_rollup_links(
        &self,
        project_prefixes: &[(String, String)],
        existing_pairs: &mut HashSet<(String, String)>,
    ) -> Result<usize> {
        if project_prefixes.is_empty() {
            return Ok(0);
        }
        let links = self.storage.list_all_links(15_000)?;
        // (src_proj, dst_proj) -> (effective_votes, raw_count, sample_paths)
        let mut agg: HashMap<(String, String), (f32, usize, Vec<String>)> = HashMap::new();

        for l in &links {
            let Some(src_proj) = match_project_id(&l.source_id, project_prefixes) else {
                continue;
            };
            let Some(dst_proj) = match_project_id(&l.target_id, project_prefixes) else {
                continue;
            };
            if src_proj == dst_proj {
                continue;
            }
            let vote = match l.provenance.as_deref() {
                Some("ast_import") => 2.0_f32,
                Some("port_detect") => 1.5_f32,
                _ => 1.0_f32,
            };
            let entry = agg
                .entry((src_proj, dst_proj))
                .or_insert_with(|| (0.0, 0, Vec::new()));
            entry.0 += vote;
            entry.1 += 1;
            if entry.2.len() < 3 && !entry.2.contains(&l.target_id) {
                entry.2.push(l.target_id.clone());
            }
        }

        let mut count = 0usize;
        for ((src, dst), (eff_votes, raw_cnt, samples)) in agg {
            // 对数饱和模型: w = min(0.84, 0.55 + 0.08 * ln(1 + N_eff))
            let weight = (0.55_f32 + 0.08_f32 * (1.0_f32 + eff_votes).ln()).min(0.84_f32);
            let edge = FactEdgeRecord {
                id: None,
                source_id: src.clone(),
                target_id: dst.clone(),
                relation: "depends_on".to_string(),
                properties: serde_json::json!({
                    "rollup_links_count": raw_cnt,
                    "effective_votes": eff_votes,
                    "sample_targets": samples,
                    "reason": format!("由 {} 条跨工程文件引用上卷聚合", raw_cnt),
                }),
                confidence: weight,
                evidence_tier: "L2_STRUCTURAL".to_string(),
                weight,
                direction_kind: "forward_dependency".to_string(),
                updated_at: None,
            };
            if self.storage.upsert_fact_edge(&edge).is_ok() {
                existing_pairs.insert((src, dst));
                count += 1;
            }
        }
        Ok(count)
    }

    /// 2. 消解 Systemd Unit 与运行时进程同体分裂 (`runs_as`)，并从执行路径反推工程依赖
    fn mine_service_bridges(
        &self,
        nodes: &[FactNodeRecord],
        project_prefixes: &[(String, String)],
        existing_pairs: &mut HashSet<(String, String)>,
    ) -> Result<usize> {
        let mut count = 0usize;
        let node_ids: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

        for n in nodes.iter().filter(|n| n.node_type == "service") {
            // A. Systemd unit -> runtime service 同体桥接
            if let Some(base) = n.id.strip_prefix("service:").and_then(|s| s.strip_suffix(".service")) {
                let runtime_id = format!("service:{}", base);
                if node_ids.contains(runtime_id.as_str()) && runtime_id != n.id {
                    let edge = FactEdgeRecord {
                        id: None,
                        source_id: n.id.clone(),
                        target_id: runtime_id.clone(),
                        relation: "runs_as".to_string(),
                        properties: serde_json::json!({
                            "reason": format!("Systemd unit {} 对应运行时进程 {}", n.name, base)
                        }),
                        confidence: 0.82,
                        evidence_tier: "L2_STRUCTURAL".to_string(),
                        weight: 0.82,
                        direction_kind: "derivation_flow".to_string(),
                        updated_at: None,
                    };
                    if self.storage.upsert_fact_edge(&edge).is_ok() {
                        existing_pairs.insert((n.id.clone(), runtime_id));
                        count += 1;
                    }
                }
            }

            // B. 从 exec_start / command / working_directory / canonical_path 提取关联工程
            let mut candidate_texts = Vec::new();
            for key in ["exec_start", "command", "working_directory", "canonical_path", "associated_project"] {
                if let Some(s) = n.properties.get(key).and_then(|v| v.as_str()) {
                    candidate_texts.push(s.to_string());
                }
            }
            for text in &candidate_texts {
                for (prefix, proj_id) in project_prefixes {
                    if text.contains(prefix) && !existing_pairs.contains(&(n.id.clone(), proj_id.clone())) {
                        let edge = FactEdgeRecord {
                            id: None,
                            source_id: n.id.clone(),
                            target_id: proj_id.clone(),
                            relation: "depends_on".to_string(),
                            properties: serde_json::json!({
                                "matched_prefix": prefix,
                                "reason": format!("服务 {} 启动/工作路径命中工程目录 {}", n.name, prefix)
                            }),
                            confidence: 0.80,
                            evidence_tier: "L2_STRUCTURAL".to_string(),
                            weight: 0.80,
                            direction_kind: "forward_dependency".to_string(),
                            updated_at: None,
                        };
                        if self.storage.upsert_fact_edge(&edge).is_ok() {
                            existing_pairs.insert((n.id.clone(), proj_id.clone()));
                            count += 1;
                        }
                    }
                }
            }
        }
        Ok(count)
    }

    /// 3. 抽取项目/服务/Cron 对宿主机 CLI 工具的 `uses_tool` 依赖边
    fn mine_tool_usage(
        &self,
        nodes: &[FactNodeRecord],
        existing_pairs: &mut HashSet<(String, String)>,
    ) -> Result<usize> {
        let tools: Vec<(&FactNodeRecord, String)> = nodes
            .iter()
            .filter(|n| n.node_type == "tool" && n.name.len() >= 2)
            .map(|n| (n, n.name.to_lowercase()))
            .collect();
        if tools.is_empty() {
            return Ok(0);
        }

        let mut count = 0usize;
        for n in nodes {
            if !matches!(n.node_type.as_str(), "service" | "cron" | "project") {
                continue;
            }
            let text_blob = collect_searchable_text(n).to_lowercase();
            if text_blob.is_empty() {
                continue;
            }

            for (tool_node, tool_lower) in &tools {
                if contains_word_boundary(&text_blob, tool_lower) {
                    let weight = if n.node_type == "project" { 0.68 } else { 0.76 };
                    let edge = FactEdgeRecord {
                        id: None,
                        source_id: n.id.clone(),
                        target_id: tool_node.id.clone(),
                        relation: "uses_tool".to_string(),
                        properties: serde_json::json!({
                            "tool": tool_node.name,
                            "reason": format!("{} 文本/配置特征引用宿主机工具 {}", n.id, tool_node.name)
                        }),
                        confidence: weight,
                        evidence_tier: "L2_STRUCTURAL".to_string(),
                        weight,
                        direction_kind: "forward_dependency".to_string(),
                        updated_at: None,
                    };
                    if self.storage.upsert_fact_edge(&edge).is_ok() {
                        existing_pairs.insert((n.id.clone(), tool_node.id.clone()));
                        count += 1;
                    }
                }
            }
        }
        Ok(count)
    }

    /// 4. 识别 `<name>_articles` / `<name>_docs` 与 `<name>` 的衍生文档流边 (`documented_by`)
    fn mine_derivation_edges(
        &self,
        nodes: &[FactNodeRecord],
        existing_pairs: &mut HashSet<(String, String)>,
    ) -> Result<usize> {
        let mut by_lower_name: HashMap<String, &FactNodeRecord> = HashMap::new();
        for n in nodes.iter().filter(|n| n.node_type == "project") {
            by_lower_name.insert(n.name.to_lowercase(), n);
        }

        let suffixes = ["_articles", "-articles", "_docs", "-docs", "_notes", "-notes"];
        let mut count = 0usize;
        for (lower_name, doc_node) in &by_lower_name {
            for suf in &suffixes {
                if let Some(stem) = lower_name.strip_suffix(suf) {
                    if let Some(src_node) = by_lower_name.get(stem) {
                        if src_node.id != doc_node.id {
                            let edge = FactEdgeRecord {
                                id: None,
                                source_id: src_node.id.clone(),
                                target_id: doc_node.id.clone(),
                                relation: "documented_by".to_string(),
                                properties: serde_json::json!({
                                    "reason": format!("工程 {} 与衍生分析工程 {} 同源命名匹配", src_node.name, doc_node.name)
                                }),
                                confidence: 0.80,
                                evidence_tier: "L2_STRUCTURAL".to_string(),
                                weight: 0.80,
                                direction_kind: "derivation_flow".to_string(),
                                updated_at: None,
                            };
                            if self.storage.upsert_fact_edge(&edge).is_ok() {
                                existing_pairs.insert((src_node.id.clone(), doc_node.id.clone()));
                                existing_pairs.insert((doc_node.id.clone(), src_node.id.clone()));
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        Ok(count)
    }

    /// 5. L3 隐式语义边发现（向量余弦 >= 0.78 + 标签 Jaccard 混合打分，Top-K 稀疏剪枝）
    fn mine_semantic_edges(
        &self,
        nodes: &[FactNodeRecord],
        existing_pairs: &HashSet<(String, String)>,
        top_k: usize,
    ) -> Result<usize> {
        // 仅针对 project / service / tool 做跨节点语义连接
        let candidates: Vec<(&FactNodeRecord, Vec<f32>, HashSet<String>)> = nodes
            .iter()
            .filter(|n| matches!(n.node_type.as_str(), "project" | "service" | "tool"))
            .filter_map(|n| {
                let vec = self.storage.get_fact_node_embedding(&n.id).ok()?;
                if vec.is_empty() {
                    return None;
                }
                let tokens = extract_semantic_tokens(n);
                Some((n, vec, tokens))
            })
            .collect();

        if candidates.len() < 2 {
            return Ok(0);
        }

        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::CpuVectorCosine);
        let mut count = 0usize;
        let mut emitted_undirected: HashSet<(String, String)> = HashSet::new();

        for (i, (src_node, src_vec, src_toks)) in candidates.iter().enumerate() {
            let mut scored: Vec<(usize, f32, f32, f32, f32)> = Vec::new();
            for (j, (dst_node, dst_vec, dst_toks)) in candidates.iter().enumerate() {
                if i == j || src_node.id == dst_node.id {
                    continue;
                }
                if existing_pairs.contains(&(src_node.id.clone(), dst_node.id.clone()))
                    || existing_pairs.contains(&(dst_node.id.clone(), src_node.id.clone()))
                {
                    continue;
                }

                let cos = cosine_similarity(src_vec, dst_vec).max(0.0);
                if cos < 0.78 {
                    continue;
                }
                let jacc = jaccard_similarity(src_toks, dst_toks);
                let score = 0.75 * cos + 0.25 * jacc;
                if score < 0.62 {
                    continue;
                }
                let weight = (0.35_f32 + 0.30_f32 * ((score - 0.62) / 0.38)).min(0.65_f32);
                scored.push((j, weight, cos, jacc, score));
            }

            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            for (j, weight, cos, jacc, score) in scored.into_iter().take(top_k) {
                let dst_node = candidates[j].0;
                let pair_key = if src_node.id <= dst_node.id {
                    (src_node.id.clone(), dst_node.id.clone())
                } else {
                    (dst_node.id.clone(), src_node.id.clone())
                };
                if !emitted_undirected.insert(pair_key) {
                    continue;
                }

                let edge = FactEdgeRecord {
                    id: None,
                    source_id: src_node.id.clone(),
                    target_id: dst_node.id.clone(),
                    relation: "semantically_related".to_string(),
                    properties: serde_json::json!({
                        "cosine": (cos * 1000.0).round() / 1000.0,
                        "tag_jaccard": (jacc * 1000.0).round() / 1000.0,
                        "hybrid_score": (score * 1000.0).round() / 1000.0,
                        "reason": format!("向量语义余弦 {:.3} + 标签重合 {:.3} 跨孤岛隐式发现", cos, jacc),
                    }),
                    confidence: weight,
                    evidence_tier: "L3_SEMANTIC".to_string(),
                    weight,
                    direction_kind: "symmetric_affinity".to_string(),
                    updated_at: None,
                };
                if self.storage.upsert_fact_edge(&edge).is_ok() {
                    count += 1;
                }
            }
        }
        Ok(count)
    }
}

// ─────────────────────────── 辅助纯函数 ───────────────────────────

fn collect_project_prefixes(nodes: &[FactNodeRecord]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for n in nodes.iter().filter(|n| n.node_type == "project") {
        if let Some(p) = n
            .properties
            .get("abs_path")
            .or_else(|| n.properties.get("canonical_path"))
            .and_then(|v| v.as_str())
        {
            let norm = p.trim_end_matches('/');
            if !norm.is_empty() {
                out.push((norm.to_string(), n.id.clone()));
            }
        }
    }
    // 最长路径优先，避免子目录误匹配父前缀
    out.sort_by_key(|a| std::cmp::Reverse(a.0.len()));
    out
}

fn match_project_id(path: &str, prefixes: &[(String, String)]) -> Option<String> {
    let norm = path.trim();
    for (prefix, proj_id) in prefixes {
        if norm == prefix || norm.starts_with(&format!("{}/", prefix)) {
            return Some(proj_id.clone());
        }
    }
    None
}

fn collect_searchable_text(n: &FactNodeRecord) -> String {
    let mut parts = Vec::new();
    for key in ["description", "exec_start", "command", "usage", "primary_lang"] {
        if let Some(s) = n.properties.get(key).and_then(|v| v.as_str()) {
            parts.push(s.to_string());
        }
    }
    if let Some(arr) = n.properties.get("top_level_entries").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(s) = item.as_str() {
                parts.push(s.to_string());
            }
        }
    }
    parts.join(" ")
}

fn contains_word_boundary(haystack: &str, needle: &str) -> bool {
    let mut start = 0usize;
    while let Some(pos) = haystack[start..].find(needle) {
        let abs = start + pos;
        let before_ok = abs == 0
            || !haystack[..abs]
                .chars()
                .next_back()
                .map(|c| c.is_ascii_alphanumeric() || c == '_')
                .unwrap_or(false);
        let after_idx = abs + needle.len();
        let after_ok = after_idx >= haystack.len()
            || !haystack[after_idx..]
                .chars()
                .next()
                .map(|c| c.is_ascii_alphanumeric() || c == '_')
                .unwrap_or(false);
        if before_ok && after_ok {
            return true;
        }
        start = abs + needle.len();
    }
    false
}

fn extract_semantic_tokens(n: &FactNodeRecord) -> HashSet<String> {
    let mut set = HashSet::new();
    for raw in [
        n.name.as_str(),
        n.properties.get("domain").and_then(|v| v.as_str()).unwrap_or(""),
        n.properties.get("primary_lang").and_then(|v| v.as_str()).unwrap_or(""),
        n.properties.get("category").and_then(|v| v.as_str()).unwrap_or(""),
    ] {
        for tok in raw.split(|c: char| !c.is_alphanumeric()) {
            let t = tok.trim().to_lowercase();
            if t.len() >= 2 {
                set.insert(t);
            }
        }
    }
    set
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0_f64;
    let mut na = 0.0_f64;
    let mut nb = 0.0_f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let xf = *x as f64;
        let yf = *y as f64;
        dot += xf * yf;
        na += xf * xf;
        nb += yf * yf;
    }
    if na <= 0.0 || nb <= 0.0 {
        0.0
    } else {
        (dot / (na.sqrt() * nb.sqrt())) as f32
    }
}

fn jaccard_similarity(a: &HashSet<String>, b: &HashSet<String>) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let inter = a.intersection(b).count() as f32;
    let union = a.union(b).count() as f32;
    if union <= 0.0 { 0.0 } else { inter / union }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::LinkRecord;

    #[test]
    fn test_l2_rollup_bridge_tool_and_derivation_mining() {
        let storage = Storage::open_in_memory().expect("mem");

        // 创建两个项目 + 衍生文章项目 + 工具 + systemd/runtime 服务
        for node in [
            FactNodeRecord {
                id: "project:agentfs".into(),
                node_type: "project".into(),
                name: "agentfs".into(),
                label: "工程: agentfs".into(),
                properties: serde_json::json!({
                    "abs_path": "/root/new/src/agentfs",
                    "description": "Agent filesystem in Rust using sqlite3 and cargo"
                }),
                updated_at: None,
            },
            FactNodeRecord {
                id: "project:agentfs_articles".into(),
                node_type: "project".into(),
                name: "agentfs_articles".into(),
                label: "工程: agentfs_articles".into(),
                properties: serde_json::json!({
                    "abs_path": "/root/new/work/agentfs_articles",
                    "description": "AgentFS 源码解读"
                }),
                updated_at: None,
            },
            FactNodeRecord {
                id: "tool:sqlite3".into(),
                node_type: "tool".into(),
                name: "sqlite3".into(),
                label: "CLI: sqlite3".into(),
                properties: serde_json::json!({}),
                updated_at: None,
            },
            FactNodeRecord {
                id: "service:agent-body.service".into(),
                node_type: "service".into(),
                name: "agent-body.service".into(),
                label: "Systemd: agent-body".into(),
                properties: serde_json::json!({
                    "exec_start": "/root/new/src/agentfs/bin/agent-body daemon"
                }),
                updated_at: None,
            },
            FactNodeRecord {
                id: "service:agent-body".into(),
                node_type: "service".into(),
                name: "agent-body".into(),
                label: "进程: agent-body".into(),
                properties: serde_json::json!({}),
                updated_at: None,
            },
        ] {
            storage.upsert_fact_node(&node).expect("upsert node");
        }

        // 添加跨项目文件级 links
        for i in 0..3 {
            storage
                .add_link(&LinkRecord {
                    id: None,
                    source_type: "asset".into(),
                    source_id: format!("/root/new/work/agentfs_articles/ch{}.md", i),
                    target_type: "asset".into(),
                    target_id: format!("/root/new/src/agentfs/src/lib{}.rs", i),
                    relation: "references".into(),
                    confidence: 0.9,
                    auto_discovered: true,
                    line_number: Some(10),
                    snippet: None,
                    provenance: Some("path_regex".into()),
                    created_at: None,
                })
                .expect("add link");
        }

        let miner = GraphRelationshipMiner::new(&storage);
        let rep = miner.mine_all(false).expect("mine L2");
        assert_eq!(rep.l2_rollup_edges, 1, "应上卷 1 条跨项目 L2 边");
        assert!(rep.l2_service_bridge_edges >= 2, "应生成 runs_as 与 service->project 边");
        assert_eq!(rep.l2_tool_usage_edges, 1, "应抽取 agentfs -> sqlite3 工具边");
        assert_eq!(rep.l2_derivation_edges, 1, "应生成 agentfs -> agentfs_articles 衍生边");

        let edges = storage.list_all_fact_edges(100).expect("list edges");
        assert!(edges.iter().all(|e| e.evidence_tier == "L2_STRUCTURAL"));
        assert!(edges.iter().any(|e| e.relation == "runs_as"));
        assert!(edges.iter().any(|e| e.relation == "uses_tool"));
        assert!(edges.iter().any(|e| e.relation == "documented_by"));
    }

    #[test]
    fn test_l3_semantic_discovery_topk_and_threshold() {
        let storage = Storage::open_in_memory().expect("mem");
        for (id, name, vec) in [
            ("project:pg_core", "pg_core", vec![1.0, 0.0, 0.0]),
            ("project:pg_vector", "pg_vector", vec![0.99, 0.14, 0.0]), // cos ~ 0.99
            ("project:pg_backup", "pg_backup", vec![0.98, 0.19, 0.0]), // cos ~ 0.98
            ("project:unrelated", "unrelated", vec![0.0, 0.0, 1.0]),  // cos = 0.0
        ] {
            storage
                .upsert_fact_node(&FactNodeRecord {
                    id: id.into(),
                    node_type: "project".into(),
                    name: name.into(),
                    label: name.into(),
                    properties: serde_json::json!({ "domain": "database" }),
                    updated_at: None,
                })
                .expect("node");
            storage.update_fact_node_embedding(id, &vec).expect("emb");
        }

        let miner = GraphRelationshipMiner::new(&storage);
        let rep = miner.mine_all(true).expect("mine L3");
        assert!(rep.l3_semantic_edges >= 1, "相似向量节点应生成 L3 语义边");

        let edges = storage.list_all_fact_edges(100).expect("list");
        assert!(
            edges.iter().all(|e| e.source_id != "project:unrelated" && e.target_id != "project:unrelated"),
            "正交不相关节点不得产生语义边"
        );
        assert!(edges.iter().all(|e| e.evidence_tier == "L3_SEMANTIC" && e.weight <= 0.65));
    }
}
