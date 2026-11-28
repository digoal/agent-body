use std::collections::HashMap;
use anyhow::Result;
use serde_json::{Map, Value};

use ext_graph_relate::{
    GraphEntityNode, GraphRelationEdge, RelateGraphConfig, RelateGraphEngine, RelateSyncStats,
};

use crate::storage::Storage;

/// `agent-body` 主存储与 `ext-graph-relate` 嵌入式 openCypher 图引擎之间的桥接器
pub struct RelateGraphBridge;

impl RelateGraphBridge {
    /// 将 `Storage` 中的资产、显式依赖边 (`links`) 与属性图谱 (`fact_nodes` / `fact_edges`) 同步至 Relate 图引擎
    pub fn sync_from_storage(
        storage: &Storage,
        engine: &RelateGraphEngine,
    ) -> Result<RelateSyncStats> {
        let mut node_map: HashMap<String, GraphEntityNode> = HashMap::new();

        // 1. 属性图谱原生节点
        for fn_node in storage.list_all_fact_nodes(50_000).unwrap_or_default() {
            node_map.insert(
                fn_node.id.clone(),
                GraphEntityNode {
                    id: fn_node.id,
                    node_type: fn_node.node_type,
                    name: fn_node.name,
                    label: fn_node.label,
                    properties_json: fn_node.properties.to_string(),
                },
            );
        }

        // 2. 资产表节点（未出现在 fact_nodes 中时补充）
        for asset in storage.list_assets(50_000).unwrap_or_default() {
            if asset.status != "active" {
                continue;
            }
            node_map
                .entry(asset.path.clone())
                .or_insert_with(|| GraphEntityNode {
                    id: asset.path.clone(),
                    node_type: asset.asset_type,
                    name: asset.path.clone(),
                    label: asset
                        .description
                        .clone()
                        .unwrap_or_else(|| asset.path.clone()),
                    properties_json: asset.desc_kv.to_string(),
                });
        }

        // 3. 统一关系边 (links + fact_edges)
        let mut edges: Vec<GraphRelationEdge> = Vec::new();
        for l in storage.list_all_links(50_000).unwrap_or_default() {
            edges.push(GraphRelationEdge {
                source_id: l.source_id,
                source_type: l.source_type,
                target_id: l.target_id,
                relation: l.relation,
                weight: l.confidence,
                evidence_tier: "L1_EXPLICIT".to_string(),
                line_number: l.line_number,
                snippet: l.snippet,
                provenance: l.provenance,
            });
        }

        for fe in storage.list_all_fact_edges(50_000).unwrap_or_default() {
            let src_type = fe
                .source_id
                .split(':')
                .next()
                .filter(|s| !s.is_empty() && fe.source_id.contains(':'))
                .unwrap_or("fact")
                .to_string();
            let snippet = fe
                .properties
                .get("reason")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            edges.push(GraphRelationEdge {
                source_id: fe.source_id,
                source_type: src_type,
                target_id: fe.target_id,
                relation: fe.relation,
                weight: fe.weight,
                evidence_tier: fe.evidence_tier,
                line_number: None,
                snippet,
                provenance: Some("property_graph".to_string()),
            });
        }

        let nodes: Vec<GraphEntityNode> = node_map.into_values().collect();
        engine.sync_graph(&nodes, &edges)
    }

    /// 以只读护栏执行 openCypher 查询并返回 JSON 结构
    pub fn query_readonly_json(
        engine: &RelateGraphEngine,
        cypher: &str,
        max_rows: Option<usize>,
    ) -> Result<Value> {
        let mut out = engine.query_readonly(cypher, &Map::new())?;
        if let Some(lim) = max_rows {
            if lim > 0 && out.rows.len() > lim {
                out.rows.truncate(lim);
                out.row_count = out.rows.len();
                out.truncated = true;
            }
        }
        Ok(serde_json::to_value(out)?)
    }

    /// 根据显式覆盖参数打开 Relate 引擎
    pub fn open_with_overrides(
        base_cfg: &crate::config::Config,
        db_override: Option<&str>,
        lib_override: Option<&str>,
        max_rows_override: Option<usize>,
    ) -> Result<RelateGraphEngine> {
        let mut cfg: RelateGraphConfig = base_cfg.resolve_relate_config();
        if let Some(db) = db_override.map(str::trim).filter(|s| !s.is_empty()) {
            cfg.db_path = db.to_string();
        }
        if let Some(lib) = lib_override.map(str::trim).filter(|s| !s.is_empty()) {
            cfg.lib_path = Some(std::path::PathBuf::from(lib));
        }
        if let Some(mr) = max_rows_override {
            if mr > 0 {
                cfg.max_rows = mr;
            }
        }
        RelateGraphEngine::open(cfg)
    }
}
