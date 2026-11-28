//! Relate 嵌入式图谱引擎高层封装
//!
//! 提供：
//! - 库探测、进程内连接管理、只读护栏 (`query_readonly`)
//! - 显式事务与 `checkpoint`
//! - 实体/关系图同步 (`sync_graph`)
//! - 多跳影响分析 (`analyze_impact`) 与最短路径 (`shortest_path`)

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::ffi::{RelateConnHandle, RelateDbHandle, RelateFfiLib};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelateGraphConfig {
    /// 支持 `:memory:`、目录路径或 `.rl` 单文件存储
    pub db_path: String,
    /// 可选显式指定 `librelate_ffi.so` 路径
    pub lib_path: Option<PathBuf>,
    pub read_only: bool,
    pub timeout_ms: u64,
    pub max_rows: usize,
    pub namespace: String,
}

impl Default for RelateGraphConfig {
    fn default() -> Self {
        Self {
            db_path: ":memory:".to_string(),
            lib_path: None,
            read_only: false,
            timeout_ms: 5000,
            max_rows: 200,
            namespace: "neo4j".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelateQueryOutput {
    pub columns: Vec<String>,
    pub rows: Vec<Map<String, Value>>,
    pub row_count: usize,
    pub truncated: bool,
    pub elapsed_ms: f64,
    pub bookmark: Option<String>,
    pub stats: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelateEngineStatus {
    pub available: bool,
    pub abi_version: Option<i32>,
    pub lib_path: Option<String>,
    pub db_path: String,
    pub node_count: u64,
    pub edge_count: u64,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphEntityNode {
    pub id: String,
    pub node_type: String,
    pub name: String,
    pub label: String,
    pub properties_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphRelationEdge {
    pub source_id: String,
    pub source_type: String,
    pub target_id: String,
    pub relation: String,
    pub weight: f32,
    pub evidence_tier: String,
    pub line_number: Option<i32>,
    pub snippet: Option<String>,
    pub provenance: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelateSyncStats {
    pub nodes_upserted: usize,
    pub edges_upserted: usize,
    pub bookmark: Option<String>,
    pub elapsed_ms: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelateImpactHit {
    pub node_id: String,
    pub node_type: String,
    pub relation: String,
    pub depth: usize,
    pub weight: f32,
    pub effective_score: f32,
    pub evidence_tier: String,
    pub line_number: Option<i32>,
    pub snippet: Option<String>,
    pub provenance: Option<String>,
}

pub struct RelateGraphEngine {
    cfg: RelateGraphConfig,
    db: Arc<RelateDbHandle>,
    conn: RelateConnHandle,
}

impl RelateGraphEngine {
    /// 只探测宿主机 `librelate_ffi.so` 是否可用及 ABI 版本（不打开数据库）
    pub fn probe_library(lib_path: Option<&Path>) -> RelateEngineStatus {
        match RelateFfiLib::load_or_cached(lib_path) {
            Ok(lib) => RelateEngineStatus {
                available: true,
                abi_version: Some(lib.abi_version()),
                lib_path: Some(lib.resolved_path().to_string()),
                db_path: String::new(),
                node_count: 0,
                edge_count: 0,
                detail: format!(
                    "librelate_ffi 就绪 (ABI v{}, {})",
                    lib.abi_version(),
                    lib.resolved_path()
                ),
            },
            Err(e) => RelateEngineStatus {
                available: false,
                abi_version: None,
                lib_path: None,
                db_path: String::new(),
                node_count: 0,
                edge_count: 0,
                detail: e.to_string(),
            },
        }
    }

    pub fn open(cfg: RelateGraphConfig) -> Result<Self> {
        let lib = RelateFfiLib::load_or_cached(cfg.lib_path.as_deref())?;
        if cfg.db_path != ":memory:" && !cfg.read_only {
            let p = Path::new(&cfg.db_path);
            if let Some(parent) = p.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent).with_context(|| {
                        format!("创建 Relate 数据库父目录失败: {:?}", parent)
                    })?;
                }
            }
        }
        let db = RelateDbHandle::open(lib, &cfg.db_path, cfg.read_only)?;
        let conn = RelateConnHandle::connect(Arc::clone(&db), Some(&cfg.namespace))?;
        if cfg.timeout_ms > 0 {
            conn.set_statement_timeout(cfg.timeout_ms)?;
        }
        Ok(Self { cfg, db, conn })
    }

    pub fn open_in_memory(lib_path: Option<PathBuf>) -> Result<Self> {
        Self::open(RelateGraphConfig {
            db_path: ":memory:".to_string(),
            lib_path,
            ..Default::default()
        })
    }

    pub fn config(&self) -> &RelateGraphConfig {
        &self.cfg
    }

    pub fn query(&self, cypher: &str, params: &Map<String, Value>) -> Result<RelateQueryOutput> {
        let raw = self.conn.query(cypher, params)?;
        let limit = self.cfg.max_rows;
        let total = raw.rows.len();
        let (rows, truncated) = if limit > 0 && total > limit {
            (raw.rows.into_iter().take(limit).collect(), true)
        } else {
            (raw.rows, false)
        };
        let row_count = rows.len();
        Ok(RelateQueryOutput {
            columns: raw.columns,
            rows,
            row_count,
            truncated,
            elapsed_ms: raw.elapsed_ms,
            bookmark: raw.bookmark,
            stats: raw.stats,
        })
    }

    /// 带静态只读护栏的 Cypher 查询（拒绝任何图修改子句）
    pub fn query_readonly(
        &self,
        cypher: &str,
        params: &Map<String, Value>,
    ) -> Result<RelateQueryOutput> {
        ensure_readonly_cypher(cypher)?;
        self.query(cypher, params)
    }

    /// 在单个显式事务中顺序执行多条 Cypher 语句并提交
    pub fn execute_tx(&self, statements: &[(&str, Map<String, Value>)]) -> Result<Option<String>> {
        self.conn.begin()?;
        for (stmt, params) in statements {
            if let Err(e) = self.conn.query(stmt, params) {
                let _ = self.conn.rollback();
                return Err(e);
            }
        }
        self.conn.commit()
    }

    pub fn begin_tx(&self) -> Result<()> {
        self.conn.begin()
    }

    pub fn commit_tx(&self) -> Result<Option<String>> {
        self.conn.commit()
    }

    pub fn rollback_tx(&self) -> Result<()> {
        self.conn.rollback()
    }

    pub fn checkpoint(&self) -> Result<()> {
        self.db.checkpoint()
    }

    /// 初始化 `:Entity(id)` 索引（幂等）
    pub fn ensure_schema(&self) -> Result<()> {
        let _ = self.conn.query(
            "CREATE INDEX entity_id_idx IF NOT EXISTS FOR (n:Entity) ON (n.id)",
            &Map::new(),
        );
        Ok(())
    }

    /// 在显式事务内增量/幂等同步实体节点与关联边
    pub fn sync_graph(
        &self,
        nodes: &[GraphEntityNode],
        edges: &[GraphRelationEdge],
    ) -> Result<RelateSyncStats> {
        let t0 = std::time::Instant::now();
        self.ensure_schema()?;

        // 去重补齐孤立端点，确保 MERGE 边前所有节点存在
        let mut node_map: HashMap<String, GraphEntityNode> = HashMap::with_capacity(nodes.len());
        for n in nodes {
            node_map.insert(n.id.clone(), n.clone());
        }
        for e in edges {
            node_map
                .entry(e.source_id.clone())
                .or_insert_with(|| GraphEntityNode {
                    id: e.source_id.clone(),
                    node_type: if e.source_type.trim().is_empty() {
                        "entity".to_string()
                    } else {
                        e.source_type.clone()
                    },
                    name: e.source_id.clone(),
                    label: e.source_id.clone(),
                    properties_json: "{}".to_string(),
                });
            node_map
                .entry(e.target_id.clone())
                .or_insert_with(|| GraphEntityNode {
                    id: e.target_id.clone(),
                    node_type: e
                        .target_id
                        .split(':')
                        .next()
                        .filter(|s| !s.is_empty() && e.target_id.contains(':'))
                        .unwrap_or("entity")
                        .to_string(),
                    name: e.target_id.clone(),
                    label: e.target_id.clone(),
                    properties_json: "{}".to_string(),
                });
        }

        self.conn.begin()?;
        let res: Result<Option<String>> = (|| {
            for n in node_map.values() {
                let mut p = Map::new();
                p.insert("id".to_string(), Value::String(n.id.clone()));
                p.insert("node_type".to_string(), Value::String(n.node_type.clone()));
                p.insert("name".to_string(), Value::String(n.name.clone()));
                p.insert("label".to_string(), Value::String(n.label.clone()));
                p.insert(
                    "properties_json".to_string(),
                    Value::String(n.properties_json.clone()),
                );
                self.conn.query(
                    "MERGE (n:Entity {id: $id}) \
                     SET n.node_type = $node_type, n.name = $name, n.label = $label, n.properties_json = $properties_json",
                    &p,
                )?;
            }

            for e in edges {
                let mut p = Map::new();
                p.insert("src".to_string(), Value::String(e.source_id.clone()));
                p.insert("tgt".to_string(), Value::String(e.target_id.clone()));
                p.insert("relation".to_string(), Value::String(e.relation.clone()));
                p.insert(
                    "weight".to_string(),
                    serde_json::Number::from_f64(e.weight as f64)
                        .map(Value::Number)
                        .unwrap_or(Value::Null),
                );
                p.insert(
                    "evidence_tier".to_string(),
                    Value::String(e.evidence_tier.clone()),
                );
                p.insert(
                    "line_number".to_string(),
                    e.line_number
                        .map(|v| Value::Number(serde_json::Number::from(v)))
                        .unwrap_or(Value::Null),
                );
                p.insert(
                    "snippet".to_string(),
                    e.snippet.clone().map(Value::String).unwrap_or(Value::Null),
                );
                p.insert(
                    "provenance".to_string(),
                    e.provenance
                        .clone()
                        .map(Value::String)
                        .unwrap_or(Value::Null),
                );
                self.conn.query(
                    "MATCH (a:Entity {id: $src}), (b:Entity {id: $tgt}) \
                     MERGE (a)-[r:RELATES {relation: $relation}]->(b) \
                     SET r.weight = $weight, r.evidence_tier = $evidence_tier, \
                         r.line_number = $line_number, r.snippet = $snippet, r.provenance = $provenance",
                    &p,
                )?;
            }

            self.conn.commit()
        })();

        let bookmark = match res {
            Ok(bm) => bm,
            Err(e) => {
                let _ = self.conn.rollback();
                return Err(e);
            }
        };

        if self.cfg.db_path != ":memory:" && !self.cfg.read_only {
            let _ = self.checkpoint();
        }

        Ok(RelateSyncStats {
            nodes_upserted: node_map.len(),
            edges_upserted: edges.len(),
            bookmark,
            elapsed_ms: t0.elapsed().as_secs_f64() * 1000.0,
        })
    }

    /// 通过 openCypher 变长路径查询多跳上下游影响半径
    pub fn analyze_impact(
        &self,
        target_id: &str,
        max_depth: usize,
        min_weight: f32,
        include_semantic: bool,
    ) -> Result<Vec<RelateImpactHit>> {
        let depth = max_depth.clamp(1, 10);
        let alt_target = target_id
            .strip_prefix("file:")
            .or_else(|| target_id.strip_prefix("project:"))
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("file:{}", target_id));

        let mut params = Map::new();
        params.insert("target".to_string(), Value::String(target_id.to_string()));
        params.insert("target_alt".to_string(), Value::String(alt_target));
        params.insert(
            "min_weight".to_string(),
            serde_json::Number::from_f64(min_weight as f64)
                .map(Value::Number)
                .unwrap_or(Value::Null),
        );
        params.insert(
            "include_semantic".to_string(),
            Value::Bool(include_semantic),
        );

        let cypher = format!(
            "MATCH (src:Entity)-[rels:RELATES*1..{depth}]->(tgt:Entity) \
             WHERE (tgt.id = $target OR tgt.id = $target_alt) \
               AND ALL(r IN rels WHERE coalesce(r.weight, 1.0) >= $min_weight \
                       AND ($include_semantic OR coalesce(r.evidence_tier, 'L1_EXPLICIT') <> 'L3_SEMANTIC')) \
             RETURN src.id AS src_id, \
                    coalesce(src.node_type, 'entity') AS src_type, \
                    [r IN rels | coalesce(r.relation, 'related_to')] AS rel_names, \
                    [r IN rels | coalesce(r.weight, 1.0)] AS rel_weights, \
                    [r IN rels | coalesce(r.evidence_tier, 'L1_EXPLICIT')] AS rel_tiers, \
                    [r IN rels | r.line_number] AS rel_lines, \
                    [r IN rels | r.snippet] AS rel_snippets, \
                    [r IN rels | r.provenance] AS rel_provs, \
                    size(rels) AS hop_len",
            depth = depth
        );

        let raw = self.conn.query(&cypher, &params)?;

        // 兼容 semantically_related 双向关系（当 include_semantic=true 时补充反向 1 跳语义邻居）
        let mut rows = raw.rows;
        if include_semantic {
            let sym_cypher = "MATCH (tgt:Entity)-[r:RELATES]->(src:Entity) \
                WHERE (tgt.id = $target OR tgt.id = $target_alt) \
                  AND r.relation = 'semantically_related' \
                  AND coalesce(r.weight, 1.0) >= $min_weight \
                RETURN src.id AS src_id, \
                       coalesce(src.node_type, 'entity') AS src_type, \
                       [coalesce(r.relation, 'semantically_related')] AS rel_names, \
                       [coalesce(r.weight, 1.0)] AS rel_weights, \
                       [coalesce(r.evidence_tier, 'L3_SEMANTIC')] AS rel_tiers, \
                       [r.line_number] AS rel_lines, \
                       [r.snippet] AS rel_snippets, \
                       [r.provenance] AS rel_provs, \
                       1 AS hop_len";
            if let Ok(sym_raw) = self.conn.query(sym_cypher, &params) {
                rows.extend(sym_raw.rows);
            }
        }

        const HOP_DECAY: f32 = 0.80;
        let mut best_by_node: HashMap<String, RelateImpactHit> = HashMap::new();
        let mut excluded: HashSet<String> = HashSet::new();
        excluded.insert(target_id.to_string());

        for row in rows {
            let Some(src_id) = row.get("src_id").and_then(Value::as_str).map(str::to_string) else {
                continue;
            };
            if excluded.contains(&src_id) {
                continue;
            }
            let src_type = row
                .get("src_type")
                .and_then(Value::as_str)
                .unwrap_or("entity")
                .to_string();
            let hop_len = row
                .get("hop_len")
                .and_then(Value::as_u64)
                .unwrap_or(1)
                .max(1) as usize;

            let rel_names = row
                .get("rel_names")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let rel_weights = row
                .get("rel_weights")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let rel_tiers = row
                .get("rel_tiers")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let rel_lines = row
                .get("rel_lines")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let rel_snippets = row
                .get("rel_snippets")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let rel_provs = row
                .get("rel_provs")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();

            let first_rel = rel_names
                .first()
                .and_then(Value::as_str)
                .unwrap_or("related_to")
                .to_string();
            let first_weight = rel_weights
                .first()
                .and_then(Value::as_f64)
                .unwrap_or(1.0) as f32;
            let first_tier = rel_tiers
                .first()
                .and_then(Value::as_str)
                .unwrap_or("L1_EXPLICIT")
                .to_string();
            let first_line = rel_lines
                .first()
                .and_then(Value::as_i64)
                .map(|v| v as i32);
            let first_snippet = rel_snippets
                .first()
                .and_then(Value::as_str)
                .map(str::to_string);
            let first_prov = rel_provs
                .first()
                .and_then(Value::as_str)
                .map(str::to_string);

            // 与 ImpactAnalyzer::analyze_impact_weighted 保持一致的跳数逐级衰减
            let mut eff = 1.0_f32;
            for (idx, w_val) in rel_weights.iter().rev().enumerate() {
                let w = w_val.as_f64().unwrap_or(1.0) as f32;
                eff = (eff * w * HOP_DECAY.powi(idx as i32)).clamp(0.0, 1.0);
            }
            let effective_score = (eff * 1000.0).round() / 1000.0;

            let hit = RelateImpactHit {
                node_id: src_id.clone(),
                node_type: src_type,
                relation: first_rel,
                depth: hop_len,
                weight: first_weight,
                effective_score,
                evidence_tier: first_tier,
                line_number: first_line,
                snippet: first_snippet,
                provenance: first_prov,
            };

            match best_by_node.get(&src_id) {
                Some(prev)
                    if prev.depth < hit.depth
                        || (prev.depth == hit.depth
                            && prev.effective_score >= hit.effective_score) => {}
                _ => {
                    best_by_node.insert(src_id, hit);
                }
            }
        }

        let mut hits: Vec<RelateImpactHit> = best_by_node.into_values().collect();
        hits.sort_by(|a, b| {
            a.depth
                .cmp(&b.depth)
                .then_with(|| b.effective_score.total_cmp(&a.effective_score))
                .then_with(|| a.node_id.cmp(&b.node_id))
        });
        Ok(hits)
    }

    /// 基于 Relate `SHORTEST 1` 计算两实体间最短路径
    pub fn shortest_path(
        &self,
        from_id: &str,
        to_id: &str,
        max_hops: usize,
    ) -> Result<Option<Value>> {
        let hops = max_hops.clamp(1, 10);
        let mut params = Map::new();
        params.insert("from".to_string(), Value::String(from_id.to_string()));
        params.insert("to".to_string(), Value::String(to_id.to_string()));

        let cypher = format!(
            "MATCH p = SHORTEST 1 (a:Entity {{id: $from}})-[:RELATES*1..{hops}]-(b:Entity {{id: $to}}) \
             RETURN [n IN nodes(p) | n.id] AS node_ids, \
                    [r IN relationships(p) | r.relation] AS relations, \
                    length(p) AS hop_count",
            hops = hops
        );
        let res = self.conn.query(&cypher, &params)?;
        Ok(res.rows.into_iter().next().map(Value::Object))
    }

    pub fn status(&self) -> Result<RelateEngineStatus> {
        let n_res = self.conn.query(
            "MATCH (n:Entity) RETURN count(n) AS cnt",
            &Map::new(),
        )?;
        let e_res = self.conn.query(
            "MATCH ()-[r:RELATES]->() RETURN count(r) AS cnt",
            &Map::new(),
        )?;
        let node_count = n_res
            .rows
            .first()
            .and_then(|r| r.get("cnt"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let edge_count = e_res
            .rows
            .first()
            .and_then(|r| r.get("cnt"))
            .and_then(Value::as_u64)
            .unwrap_or(0);

        Ok(RelateEngineStatus {
            available: true,
            abi_version: Some(self.db.lib().abi_version()),
            lib_path: Some(self.db.lib().resolved_path().to_string()),
            db_path: self.cfg.db_path.clone(),
            node_count,
            edge_count,
            detail: format!(
                "Relate 图引擎在线 (ABI v{}, nodes={}, edges={})",
                self.db.lib().abi_version(),
                node_count,
                edge_count
            ),
        })
    }
}

/// 静态只读 Cypher 护栏：剔除注释和字符串字面量后检查是否包含写操作关键字
pub fn ensure_readonly_cypher(cypher: &str) -> Result<()> {
    let stripped = strip_cypher_strings_and_comments(cypher);
    let upper = stripped.to_ascii_uppercase();
    let tokens: Vec<&str> = upper
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|t| !t.is_empty())
        .collect();

    const WRITE_KEYWORDS: &[&str] = &[
        "CREATE", "MERGE", "SET", "DELETE", "DETACH", "REMOVE", "DROP", "FOREACH",
    ];
    for kw in WRITE_KEYWORDS {
        if tokens.contains(kw) {
            bail!("只读 Cypher 护栏拒绝写操作关键字: {}", kw);
        }
    }
    for win in tokens.windows(2) {
        if win == ["LOAD", "CSV"] {
            bail!("只读 Cypher 护栏拒绝 LOAD CSV 语句");
        }
    }
    Ok(())
}

fn strip_cypher_strings_and_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let ch = chars[i];
        // 行注释 // ...
        if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '/' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            out.push(' ');
            continue;
        }
        // 块注释 /* ... */
        if ch == '/' && i + 1 < chars.len() && chars[i + 1] == '*' {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(chars.len());
            out.push(' ');
            continue;
        }
        // 单/双引号及反引号字面量
        if ch == '\'' || ch == '"' || ch == '`' {
            let quote = ch;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' && i + 1 < chars.len() {
                    i += 2;
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push(' ');
            continue;
        }
        out.push(ch);
        i += 1;
    }
    out
}
