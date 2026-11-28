//! `ext-graph-relate`: 基于 `librelate_ffi` v2 的嵌入式 openCypher 图谱周边扩展
//!
//! 严守脑体分离与微内核防腐边界：
//! - 仅单向依赖 `agent-body-sdk`，零业务代码回流 `kernel/`；
//! - 通过 `dlopen` 动态加载 `librelate_ffi.so`，缺失时编译零阻断且运行时可平滑降级；
//! - 注册 `idx:graph_relate` (`IndexAmRoutine`) 与 3 个只读诊断 `ProcRoutine`。

pub mod engine;
pub mod ffi;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agent_body_sdk::{
    body_module_magic, AutonomyLevel, BodyExtension, EvidenceTuple, ExtensionKind, ExtensionMeta,
    ExtensionRegistrar, FunctionCallInfo, IndexAmRoutine, IndexCostEstimate, IndexHit,
    IndexScanDesc, KernelServices, MigrationScript, OpStrategy, ProcRoutine, ProcSpec,
    ProcVolatility, TableAmRoutine, SDK_API_VERSION,
};
use serde_json::{Map, Value};

pub use engine::{
    ensure_readonly_cypher, GraphEntityNode, GraphRelationEdge, RelateEngineStatus,
    RelateGraphConfig, RelateGraphEngine, RelateImpactHit, RelateQueryOutput, RelateSyncStats,
};
pub use ffi::{resolve_library_candidates, RelateFfiLib};

body_module_magic!();

const MIGRATION_0_1_0: &str = include_str!("../sql/graph_relate--0.1.0.sql");

/// 微内核 `BodyExtension` 注册入口
#[derive(Clone)]
pub struct GraphRelateExtension {
    pub config: RelateGraphConfig,
    index_am: Arc<GraphRelateIndexAm>,
}

impl Default for GraphRelateExtension {
    fn default() -> Self {
        Self::new(RelateGraphConfig::default())
    }
}

impl GraphRelateExtension {
    pub fn new(config: RelateGraphConfig) -> Self {
        let index_am = Arc::new(GraphRelateIndexAm::new(config.clone()));
        Self { config, index_am }
    }

    pub fn with_db_path(db_path: impl Into<String>) -> Self {
        Self::new(RelateGraphConfig {
            db_path: db_path.into(),
            ..Default::default()
        })
    }

    pub fn with_engine(engine: Arc<Mutex<Option<RelateGraphEngine>>>, cfg: RelateGraphConfig) -> Self {
        Self {
            config: cfg.clone(),
            index_am: Arc::new(GraphRelateIndexAm::with_shared_engine(cfg, engine)),
        }
    }

    pub fn index_am(&self) -> Arc<GraphRelateIndexAm> {
        Arc::clone(&self.index_am)
    }
}

impl BodyExtension for GraphRelateExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "graph_relate".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: SDK_API_VERSION.to_string(),
            table_prefix: "ext_graph_relate__".to_string(),
            requires: vec![],
        }
    }

    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: MIGRATION_0_1_0.to_string(),
        });
        reg.register_index_am(Arc::clone(&self.index_am) as Arc<dyn IndexAmRoutine>);
        reg.register_proc(Arc::new(GraphRelateStatusProc {
            ext: self.clone(),
        }));
        reg.register_proc(Arc::new(GraphRelateQueryProc {
            ext: self.clone(),
        }));
        reg.register_proc(Arc::new(GraphRelateImpactProc {
            ext: self.clone(),
        }));
        Ok(())
    }
}

/// 基于 Relate 引擎的图索引访问方法 (`idx:graph_relate`)
pub struct GraphRelateIndexAm {
    cfg: RelateGraphConfig,
    engine: Arc<Mutex<Option<RelateGraphEngine>>>,
}

impl GraphRelateIndexAm {
    pub fn new(cfg: RelateGraphConfig) -> Self {
        let opened = RelateGraphEngine::open(cfg.clone()).ok();
        if let Some(ref eng) = opened {
            let _ = eng.ensure_schema();
        }
        Self {
            cfg,
            engine: Arc::new(Mutex::new(opened)),
        }
    }

    pub fn with_shared_engine(
        cfg: RelateGraphConfig,
        engine: Arc<Mutex<Option<RelateGraphEngine>>>,
    ) -> Self {
        Self { cfg, engine }
    }

    pub fn is_ready(&self) -> bool {
        self.engine
            .lock()
            .map(|g| g.is_some())
            .unwrap_or(false)
    }

    fn ensure_engine(&self) -> anyhow::Result<()> {
        let mut guard = self
            .engine
            .lock()
            .map_err(|_| anyhow::anyhow!("idx:graph_relate 引擎锁已中毒"))?;
        if guard.is_none() {
            let eng = RelateGraphEngine::open(self.cfg.clone())?;
            let _ = eng.ensure_schema();
            *guard = Some(eng);
        }
        Ok(())
    }

    pub fn link_entities(&self, src_id: &str, tgt_id: &str, relation: &str, weight: f32) -> anyhow::Result<()> {
        self.ensure_engine()?;
        let guard = self
            .engine
            .lock()
            .map_err(|_| anyhow::anyhow!("idx:graph_relate 引擎锁已中毒"))?;
        let eng = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Relate 引擎未初始化"))?;
        let edge = GraphRelationEdge {
            source_id: src_id.to_string(),
            source_type: "entity".to_string(),
            target_id: tgt_id.to_string(),
            relation: relation.to_string(),
            weight,
            evidence_tier: "L1_EXPLICIT".to_string(),
            line_number: None,
            snippet: None,
            provenance: Some("index_am".to_string()),
        };
        eng.sync_graph(&[], &[edge])?;
        Ok(())
    }
}

impl IndexAmRoutine for GraphRelateIndexAm {
    fn am_id(&self) -> &'static str {
        "idx:graph_relate"
    }

    fn default_weight(&self) -> f64 {
        1.2
    }

    fn amcostestimate(&self, scan: &IndexScanDesc) -> IndexCostEstimate {
        let ready = self.is_ready();
        if !ready || scan.query_text.trim().is_empty() {
            return IndexCostEstimate {
                available: false,
                est_latency_us: 1000,
                est_selectivity: 1.0,
            };
        }
        if scan.strategy_num == OpStrategy::GraphHop.strategy_num() {
            IndexCostEstimate {
                available: true,
                est_latency_us: 80,
                est_selectivity: 0.15,
            }
        } else {
            IndexCostEstimate {
                available: true,
                est_latency_us: 180,
                est_selectivity: 0.30,
            }
        }
    }

    fn amgetmulti(
        &self,
        scan: &IndexScanDesc,
        _am: &dyn TableAmRoutine,
    ) -> anyhow::Result<Vec<IndexHit>> {
        if scan.query_text.trim().is_empty() {
            return Ok(Vec::new());
        }
        let guard = self
            .engine
            .lock()
            .map_err(|_| anyhow::anyhow!("idx:graph_relate 引擎锁已中毒"))?;
        let Some(eng) = guard.as_ref() else {
            return Ok(Vec::new());
        };
        let limit = if scan.limit == 0 { 20 } else { scan.limit };

        if scan.strategy_num == OpStrategy::GraphHop.strategy_num() {
            let mut params = Map::new();
            params.insert("q".to_string(), Value::String(scan.query_text.clone()));
            params.insert("lim".to_string(), Value::Number((limit as u64).into()));
            let out = eng.query_readonly(
                "MATCH (a:Entity {id: $q})-[r:RELATES*1..2]-(b:Entity) \
                 WHERE b.id <> $q \
                 RETURN b.id AS id, coalesce(b.node_type, 'entity') AS node_type, \
                        size(r) AS hops \
                 LIMIT $lim",
                &params,
            )?;
            let hits = out
                .rows
                .into_iter()
                .filter_map(|row| {
                    let eid = row.get("id")?.as_str()?.to_string();
                    let etype = row
                        .get("node_type")
                        .and_then(Value::as_str)
                        .unwrap_or("entity")
                        .to_string();
                    let hops = row.get("hops").and_then(Value::as_u64).unwrap_or(1) as f64;
                    Some(IndexHit {
                        entity_id: eid,
                        entity_type: etype,
                        score: (1.0 / hops.max(1.0)),
                        channel: self.am_id().to_string(),
                        reasoning_path: Some(format!("relate:{}-hop", hops)),
                        tuple: None,
                    })
                })
                .collect();
            return Ok(hits);
        }

        let mut params = Map::new();
        params.insert("q".to_string(), Value::String(scan.query_text.clone()));
        params.insert("lim".to_string(), Value::Number((limit as u64).into()));
        let out = eng.query_readonly(
            "MATCH (n:Entity) \
             WHERE n.id CONTAINS $q OR n.name CONTAINS $q OR n.label CONTAINS $q \
             RETURN n.id AS id, coalesce(n.node_type, 'entity') AS node_type \
             LIMIT $lim",
            &params,
        )?;
        let hits = out
            .rows
            .into_iter()
            .filter_map(|row| {
                let eid = row.get("id")?.as_str()?.to_string();
                let etype = row
                    .get("node_type")
                    .and_then(Value::as_str)
                    .unwrap_or("entity")
                    .to_string();
                Some(IndexHit {
                    entity_id: eid,
                    entity_type: etype,
                    score: 0.75,
                    channel: self.am_id().to_string(),
                    reasoning_path: Some("relate:entity_match".to_string()),
                    tuple: None,
                })
            })
            .collect();
        Ok(hits)
    }

    fn aminsert(&self, entity: &EvidenceTuple) -> anyhow::Result<()> {
        self.ensure_engine()?;
        let guard = self
            .engine
            .lock()
            .map_err(|_| anyhow::anyhow!("idx:graph_relate 引擎锁已中毒"))?;
        let eng = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Relate 引擎未初始化"))?;
        let label = entity
            .payload
            .get("label")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| entity.entity_id.clone());
        let node = GraphEntityNode {
            id: entity.entity_id.clone(),
            node_type: entity.entity_type.clone(),
            name: entity.source_ext.clone(),
            label,
            properties_json: serde_json::to_string(&entity.payload)?,
        };
        eng.sync_graph(&[node], &[])?;
        Ok(())
    }

    fn amdelete_by_source(&self, source_key: &str) -> anyhow::Result<usize> {
        let guard = self
            .engine
            .lock()
            .map_err(|_| anyhow::anyhow!("idx:graph_relate 引擎锁已中毒"))?;
        let Some(eng) = guard.as_ref() else {
            return Ok(0);
        };
        let mut params = Map::new();
        params.insert("src".to_string(), Value::String(source_key.to_string()));
        let out = eng.query(
            "MATCH (n:Entity) WHERE n.id = $src OR n.name = $src DETACH DELETE n",
            &params,
        )?;
        let deleted = out
            .stats
            .as_ref()
            .and_then(|s| s.get("nodes_deleted"))
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        Ok(deleted)
    }

    fn amvacuum(&self) -> anyhow::Result<usize> {
        let guard = self
            .engine
            .lock()
            .map_err(|_| anyhow::anyhow!("idx:graph_relate 引擎锁已中毒"))?;
        if let Some(eng) = guard.as_ref() {
            let _ = eng.checkpoint();
        }
        Ok(0)
    }
}

fn resolve_proc_config(ext_cfg: &RelateGraphConfig, args: &Value) -> RelateGraphConfig {
    let mut cfg = ext_cfg.clone();
    if let Some(db) = args
        .get("db_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cfg.db_path = db.to_string();
    }
    if let Some(lib) = args
        .get("lib_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cfg.lib_path = Some(PathBuf::from(lib));
    }
    if let Some(max_rows) = args.get("max_rows").and_then(Value::as_u64) {
        if max_rows > 0 {
            cfg.max_rows = max_rows as usize;
        }
    }
    cfg
}

struct GraphRelateStatusProc {
    ext: GraphRelateExtension,
}

impl ProcRoutine for GraphRelateStatusProc {
    fn spec(&self) -> ProcSpec {
        ProcSpec {
            proc_name: "ext_graph_relate_status",
            cli_path: &["graph", "relate", "status"],
            rest_route: ("GET", "/api/v1/graph/relate/status"),
            profiles: &["full", "ops"],
            volatility: ProcVolatility::Volatile,
            autonomy_level: AutonomyLevel::L0Observe,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "db_path": { "type": "string" },
                    "lib_path": { "type": "string" }
                }
            }),
            provenance_rule: Some("PR-GRAPH-RELATE"),
        }
    }

    fn invoke(
        &self,
        fcinfo: FunctionCallInfo,
        ctx: &KernelServices,
    ) -> anyhow::Result<serde_json::Value> {
        let cfg = resolve_proc_config(&self.ext.config, &fcinfo.args);
        let payload = match RelateGraphEngine::open(cfg.clone()) {
            Ok(eng) => serde_json::to_value(eng.status()?)?,
            Err(_) => {
                let probe = RelateGraphEngine::probe_library(cfg.lib_path.as_deref());
                serde_json::to_value(probe)?
            }
        };
        let _ = ctx
            .table_am()
            .kv_put("graph_relate", "last_status", &payload);
        Ok(payload)
    }
}

struct GraphRelateQueryProc {
    ext: GraphRelateExtension,
}

impl ProcRoutine for GraphRelateQueryProc {
    fn spec(&self) -> ProcSpec {
        ProcSpec {
            proc_name: "ext_graph_relate_query",
            cli_path: &["graph", "relate", "query"],
            rest_route: ("POST", "/api/v1/graph/relate/query"),
            profiles: &["full", "ops"],
            volatility: ProcVolatility::Stable,
            autonomy_level: AutonomyLevel::L0Observe,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "cypher": { "type": "string" },
                    "params": { "type": "object" },
                    "db_path": { "type": "string" },
                    "lib_path": { "type": "string" },
                    "max_rows": { "type": "integer" }
                },
                "required": ["cypher"]
            }),
            provenance_rule: Some("PR-GRAPH-RELATE"),
        }
    }

    fn invoke(
        &self,
        fcinfo: FunctionCallInfo,
        _ctx: &KernelServices,
    ) -> anyhow::Result<serde_json::Value> {
        let cypher = fcinfo
            .args
            .get("cypher")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("缺少必填参数 cypher"))?;
        let params = fcinfo
            .args
            .get("params")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();
        let cfg = resolve_proc_config(&self.ext.config, &fcinfo.args);
        let eng = RelateGraphEngine::open(cfg)?;
        let out = eng.query_readonly(cypher, &params)?;
        Ok(serde_json::to_value(out)?)
    }
}

struct GraphRelateImpactProc {
    ext: GraphRelateExtension,
}

impl ProcRoutine for GraphRelateImpactProc {
    fn spec(&self) -> ProcSpec {
        ProcSpec {
            proc_name: "ext_graph_relate_impact",
            cli_path: &["graph", "relate", "impact"],
            rest_route: ("POST", "/api/v1/graph/relate/impact"),
            profiles: &["full", "ops"],
            volatility: ProcVolatility::Stable,
            autonomy_level: AutonomyLevel::L0Observe,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "target": { "type": "string" },
                    "max_depth": { "type": "integer" },
                    "min_weight": { "type": "number" },
                    "include_semantic": { "type": "boolean" },
                    "db_path": { "type": "string" },
                    "lib_path": { "type": "string" }
                },
                "required": ["target"]
            }),
            provenance_rule: Some("PR-GRAPH-RELATE"),
        }
    }

    fn invoke(
        &self,
        fcinfo: FunctionCallInfo,
        _ctx: &KernelServices,
    ) -> anyhow::Result<serde_json::Value> {
        let target = fcinfo
            .args
            .get("target")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("缺少必填参数 target"))?;
        let max_depth = fcinfo
            .args
            .get("max_depth")
            .and_then(Value::as_u64)
            .unwrap_or(3) as usize;
        let min_weight = fcinfo
            .args
            .get("min_weight")
            .and_then(Value::as_f64)
            .unwrap_or(0.50) as f32;
        let include_semantic = fcinfo
            .args
            .get("include_semantic")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let cfg = resolve_proc_config(&self.ext.config, &fcinfo.args);
        let eng = RelateGraphEngine::open(cfg)?;
        let hits = eng.analyze_impact(target, max_depth, min_weight, include_semantic)?;
        Ok(serde_json::to_value(hits)?)
    }
}
