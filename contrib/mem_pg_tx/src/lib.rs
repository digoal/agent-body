//! `ext-mem-pg-tx`: PostgreSQL 事务级双时态海马体周边扩展（Phase 76 · 方案 A → 方案 B）
//!
//! 严守脑体分离与微内核防腐边界：
//! - 仅单向依赖 `agent-body-sdk` 与 `postgres`，零业务代码回流 `kernel/`；
//! - 管理秩序（ACID/MVCC）、版本（`tx_epoch`）、解耦检索段与墓碑擦除；
//! - 绝不内置 LLM 推理/训练循环，绝不把 GB 级模型权重作为裸 BLOB 存入堆表。

pub mod engine;
pub mod fast_weight;
mod fast_weight_ops;
pub mod fast_weight_runtime;
pub mod pool;
pub mod predictability;
pub mod salience;
pub mod schema;
pub mod sleep_compactor;

use std::sync::Arc;

use agent_body_sdk::{
    body_module_magic, AutonomyLevel, BodyExtension, ExtensionKind, ExtensionMeta,
    ExtensionRegistrar, FunctionCallInfo, KernelServices, MigrationScript, ProcRoutine, ProcSpec,
    ProcVolatility, SDK_API_VERSION,
};

pub use agent_body_sdk::{
    evaluate_fast_weight_gates, validate_artifact_uri, AdapterKind, FastWeightCompactRequest,
    FastWeightCompactResponse, FastWeightGateMetrics, FastWeightGatePolicy, FastWeightGateVerdict,
    FastWeightRollbackRequest, FastWeightRollbackResponse, FastWeightRuntimeClient,
    FastWeightRuntimeHealth, FastWeightSnapshotStatus, FastWeightUnlearnRequest,
    FastWeightUnlearnResponse,
};

pub use engine::{
    PgTxConfig, PgTxEngine, PgTxEventRow, PgTxSearchSnapshot, PgTxStatus, PgTxWriteInput,
    PgTxWriteResult, VectorBackendKind,
};
pub use fast_weight::{
    FastWeightCompactionResult, FastWeightHealthSummary, FastWeightRollbackOutcome,
    FastWeightUnlearnResult, PgFastWeightSnapshot,
};
pub use fast_weight_runtime::{HttpFastWeightRuntime, MockFastWeightRuntime, MockRuntimeFault};
pub use pool::{PgConnPool, PooledClient};
pub use predictability::{PredictabilityDecision, QueryPredictabilityGate};
pub use salience::{
    SalienceBreakdown, SalienceInput, SalienceRescoreReport, SalienceScore, SalienceScorer,
    SalienceTier,
};
pub use schema::PG_TX_SCHEMA_SQL;
pub use sleep_compactor::{
    compact_episodic_texts, derive_cluster_topic, estimate_tokens_deterministic,
    CompactionClusterSummary, CompactionJobRecord, SleepCompactionConfig, SleepCompactionReport,
};

body_module_magic!();

const MIGRATION_0_1_0: &str = include_str!("../sql/mem_pg_tx--0.1.0.sql");

/// 微内核 `BodyExtension` 注册入口（标准五件套）
#[derive(Debug, Clone, Default)]
pub struct MemPgTxExtension {
    pub pg_url: Option<String>,
    pub pool_size: usize,
    pub tenant_scope: String,
    pub vector_backend: String,
}

impl MemPgTxExtension {
    pub fn with_pg_url(pg_url: impl Into<String>) -> Self {
        Self {
            pg_url: Some(pg_url.into()),
            pool_size: 8,
            tenant_scope: "default".to_string(),
            vector_backend: "none".to_string(),
        }
    }
}

impl BodyExtension for MemPgTxExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "mem_pg_tx".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: SDK_API_VERSION.to_string(),
            table_prefix: "ext_mem_pg_tx__".to_string(),
            requires: vec![],
        }
    }

    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: MIGRATION_0_1_0.to_string(),
        });
        reg.register_proc(Arc::new(MemPgTxStatusProc {
            ext: self.clone(),
        }));
        reg.register_proc(Arc::new(MemPgTxQueryProc {
            ext: self.clone(),
        }));
        Ok(())
    }
}

struct MemPgTxStatusProc {
    ext: MemPgTxExtension,
}

impl ProcRoutine for MemPgTxStatusProc {
    fn spec(&self) -> ProcSpec {
        ProcSpec {
            proc_name: "ext_mem_pg_tx_status",
            cli_path: &["mem", "pg-tx", "status"],
            rest_route: ("GET", "/api/v1/mem/pg-tx/status"),
            profiles: &["full", "ops"],
            volatility: ProcVolatility::Volatile,
            autonomy_level: AutonomyLevel::L0Observe,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "pg_url": { "type": "string", "description": "可选覆盖 PostgreSQL 连接串" }
                }
            }),
            provenance_rule: Some("PR-MEM-PG-TX"),
        }
    }

    fn invoke(
        &self,
        fcinfo: FunctionCallInfo,
        ctx: &KernelServices,
    ) -> anyhow::Result<serde_json::Value> {
        let url_opt = fcinfo
            .args
            .get("pg_url")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| self.ext.pg_url.clone())
            .or_else(|| {
                std::env::var("AGENT_BODY_MEMORY_PG_URL")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            });

        let payload = match url_opt {
            Some(url) => {
                let mut cfg = PgTxConfig::new(url);
                if self.ext.pool_size > 0 {
                    cfg.pool_size = self.ext.pool_size;
                }
                if !self.ext.tenant_scope.trim().is_empty() {
                    cfg.tenant_scope = self.ext.tenant_scope.clone();
                }
                cfg.vector_backend =
                    VectorBackendKind::parse_str(&self.ext.vector_backend).unwrap_or(VectorBackendKind::None);
                match PgTxEngine::connect(cfg).and_then(|e| e.status()) {
                    Ok(st) => serde_json::to_value(st)?,
                    Err(e) => serde_json::json!({
                        "connected": false,
                        "fallback_active": true,
                        "detail": format!("PostgreSQL 不可达，已降级为本地模式: {}", e),
                    }),
                }
            }
            None => serde_json::json!({
                "connected": false,
                "fallback_active": true,
                "detail": "未配置 pg_url（可设置 [memory].pg_url 或 AGENT_BODY_MEMORY_PG_URL）",
            }),
        };

        ctx.table_am()
            .kv_put("mem_pg_tx", "last_status", &payload)?;
        Ok(payload)
    }
}

struct MemPgTxQueryProc {
    ext: MemPgTxExtension,
}

impl ProcRoutine for MemPgTxQueryProc {
    fn spec(&self) -> ProcSpec {
        ProcSpec {
            proc_name: "ext_mem_pg_tx_query",
            cli_path: &["mem", "pg-tx", "query"],
            rest_route: ("POST", "/api/v1/mem/pg-tx/query"),
            profiles: &["full", "ops"],
            volatility: ProcVolatility::Stable,
            autonomy_level: AutonomyLevel::L0Observe,
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string" },
                    "tenant_scope": { "type": "string" },
                    "as_of_epoch": { "type": "integer" },
                    "include_history": { "type": "boolean" },
                    "limit": { "type": "integer" },
                    "pg_url": { "type": "string" }
                }
            }),
            provenance_rule: Some("PR-MEM-PG-TX"),
        }
    }

    fn invoke(
        &self,
        fcinfo: FunctionCallInfo,
        _ctx: &KernelServices,
    ) -> anyhow::Result<serde_json::Value> {
        let url = fcinfo
            .args
            .get("pg_url")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .or_else(|| self.ext.pg_url.clone())
            .or_else(|| {
                std::env::var("AGENT_BODY_MEMORY_PG_URL")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            })
            .ok_or_else(|| anyhow::anyhow!("未配置 pg_url，无法查询 PostgreSQL 事务记忆"))?;

        let query = fcinfo
            .args
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let tenant_scope = fcinfo
            .args
            .get("tenant_scope")
            .and_then(|v| v.as_str())
            .unwrap_or(if self.ext.tenant_scope.is_empty() {
                "default"
            } else {
                &self.ext.tenant_scope
            });
        let as_of_epoch = fcinfo.args.get("as_of_epoch").and_then(|v| v.as_i64());
        let include_history = fcinfo
            .args
            .get("include_history")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let limit = fcinfo
            .args
            .get("limit")
            .and_then(|v| v.as_u64())
            .unwrap_or(15) as usize;

        let engine = PgTxEngine::connect(PgTxConfig::new(url))?;
        let snap = engine.search(query, tenant_scope, limit, as_of_epoch, include_history)?;
        Ok(serde_json::to_value(snap)?)
    }
}
