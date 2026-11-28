//! PostgreSQL 事务级海马体适配器 (`PgTxMemoryAdapter`)
//!
//! 桥接 `contrib/mem_pg_tx`（`ext-mem-pg-tx`）与旗舰 `MemoryProvider` / `TxMemoryProvider` SPI：
//! - 写前强制经 `SecretRedactor` 脱敏与 `PromptInjectionScanner` 清洗；
//! - PostgreSQL 不可达时依据 `fallback_to_sqlite` 自动无损降级至本地 `BuiltinMemoryProvider` 并记审计告警；
//! - 提供 SQLite ⇄ PostgreSQL 双向热同步能力，保障离线零依赖与多 Agent 事务底座自由切换。

use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;
use ext_mem_pg_tx::{PgTxConfig, PgTxEngine, PgTxEventRow, PgTxWriteInput, VectorBackendKind};

use crate::audit::{AuditEventInput, AuditLogger};
use crate::body::manifest::compute_body_id;
use crate::memory::builtin_provider::BuiltinMemoryProvider;
use crate::memory::spi::{
    EvictCriteria, FastWeightCheckpointSpi, FastWeightSnapshotMeta, MemoryFilter, MemoryItem,
    MemoryProvider, SessionContext, TxEvidenceTier, TxMemoryEvent, TxMemoryProvider,
    TxMemorySnapshot, TxMemoryWriteInput, TxProviderHealth, TxSearchOptions, TxWriteOutcome,
};
use crate::scanner::metadata::{PromptInjectionScanner, SecretRedactor};
use crate::storage::models::MemoryRecord;
use crate::storage::Storage;

/// `PgTxMemoryAdapter` 连接与降级配置
#[derive(Debug, Clone)]
pub struct PgTxAdapterConfig {
    pub pg_url: String,
    pub pool_size: usize,
    pub tenant_scope: String,
    pub vector_backend: String,
    pub fallback_to_sqlite: bool,
    pub mirror_writes_to_sqlite: bool,
}

impl Default for PgTxAdapterConfig {
    fn default() -> Self {
        Self {
            pg_url: String::new(),
            pool_size: 8,
            tenant_scope: "default".to_string(),
            vector_backend: "none".to_string(),
            fallback_to_sqlite: true,
            mirror_writes_to_sqlite: true,
        }
    }
}

/// 事务级 PostgreSQL 记忆适配器（支持自动无损降级为本地 SQLite）
pub struct PgTxMemoryAdapter {
    engine: Option<PgTxEngine>,
    fallback: BuiltinMemoryProvider,
    storage: Storage,
    config: PgTxAdapterConfig,
    fallback_reason: Option<String>,
}

impl PgTxMemoryAdapter {
    /// 连接 PostgreSQL 事务记忆引擎；若不可达且允许降级则无缝切回本地 SQLite
    pub fn connect_or_fallback(config: PgTxAdapterConfig, storage: Storage) -> Result<Self> {
        let fallback = BuiltinMemoryProvider::new(storage.clone());
        let resolved_url = if !config.pg_url.trim().is_empty() {
            config.pg_url.trim().to_string()
        } else {
            std::env::var("AGENT_BODY_MEMORY_PG_URL")
                .ok()
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };

        if resolved_url.is_empty() {
            if config.fallback_to_sqlite {
                return Ok(Self {
                    engine: None,
                    fallback,
                    storage,
                    config,
                    fallback_reason: Some("未配置 memory.pg_url，已自动降级至本地 SQLite 引擎".to_string()),
                });
            }
            anyhow::bail!("memory.provider='pg_tx' 未配置 pg_url 且禁止降级");
        }

        let vec_kind = VectorBackendKind::parse_str(&config.vector_backend)?;
        let mut pg_cfg = PgTxConfig::new(resolved_url);
        pg_cfg.pool_size = config.pool_size.clamp(1, 64);
        pg_cfg.tenant_scope = if config.tenant_scope.trim().is_empty() {
            "default".to_string()
        } else {
            config.tenant_scope.trim().to_string()
        };
        pg_cfg.vector_backend = vec_kind;

        match PgTxEngine::connect(pg_cfg) {
            Ok(engine) => Ok(Self {
                engine: Some(engine),
                fallback,
                storage,
                config,
                fallback_reason: None,
            }),
            Err(err) if config.fallback_to_sqlite => {
                let reason = format!("PostgreSQL 事务记忆不可达，已自动降级 SQLite: {}", err);
                eprintln!("[PgTx][WARN] {}", reason);
                AuditLogger::record_best_effort(
                    &storage,
                    AuditEventInput::out_of_agent(
                        "memory_adapter",
                        "pg_tx_memory",
                        "memory_op",
                        "fallback_to_sqlite",
                    )
                    .with_target("memory_provider", "pg_tx")
                    .with_outcome("warn")
                    .with_delta(serde_json::json!({ "reason": reason })),
                );
                Ok(Self {
                    engine: None,
                    fallback,
                    storage,
                    config,
                    fallback_reason: Some(reason),
                })
            }
            Err(err) => Err(err),
        }
    }

    pub fn is_pg_active(&self) -> bool {
        self.engine.is_some()
    }

    pub fn engine(&self) -> Option<&PgTxEngine> {
        self.engine.as_ref()
    }

    pub fn fallback_reason(&self) -> Option<&str> {
        self.fallback_reason.as_deref()
    }

    /// 将本地 SQLite `memories` 中的活跃记忆无损同步入 PostgreSQL 双时态事件表
    pub fn sync_sqlite_to_pg(&self, limit: usize) -> Result<usize> {
        let Some(engine) = &self.engine else {
            return Ok(0);
        };
        let body_id = compute_body_id();
        let memories = self.storage.list_all_active_memories(limit.clamp(1, 5000))?;
        let mut synced = 0usize;
        for m in memories {
            let input = TxMemoryWriteInput {
                body_id: body_id.clone(),
                tenant_scope: self.config.tenant_scope.clone(),
                fact_key: format!("sqlite_mem:{}", m.id),
                memory_type: m.memory_type,
                content: m.content,
                summary: m.summary,
                tags: m.tags,
                evidence_tier: TxEvidenceTier::L2Structural,
                salience_score: m.weight.clamp(0.0, 1.0),
                expected_epoch: None,
                source_agent: m.source_agent,
                source_session: m.source_session,
                embedding_ref: None,
            };
            let _outcome = self.append_with_engine(engine, &input)?;
            synced += 1;
        }
        Ok(synced)
    }

    /// 将 PostgreSQL 当前活跃双时态记忆镜像回本地 SQLite（保障离线降级时零数据断层）
    pub fn sync_pg_to_sqlite(&self, limit: usize) -> Result<usize> {
        let Some(engine) = &self.engine else {
            return Ok(0);
        };
        let snap = engine.search("", &self.config.tenant_scope, limit.clamp(1, 5000), None, false)?;
        let mut synced = 0usize;
        for row in snap.hits {
            let rec = MemoryRecord {
                id: format!("pgtx-{}", row.fact_key.replace(':', "_")),
                memory_type: crate::memory::MemoryManager::normalize_memory_type(&row.memory_type)
                    .unwrap_or("fact")
                    .to_string(),
                content: row.content,
                summary: row.summary,
                source_agent: Some("pg_tx_sync".to_string()),
                source_session: None,
                tags: row.tags,
                weight: row.salience_score.clamp(0.1, 1.0),
                pinned: false,
                status: "active".to_string(),
                embedding: vec![],
            };
            self.storage.insert_memory(&rec)?;
            synced += 1;
        }
        Ok(synced)
    }

    fn sanitize_input(input: &TxMemoryWriteInput) -> PgTxWriteInput {
        let redacted_content = SecretRedactor::redact_preserving_newlines(&input.content);
        let scan_content = PromptInjectionScanner::scan_and_sanitize(&redacted_content);
        let clean_summary = input.summary.as_deref().map(|s| {
            let r = SecretRedactor::redact_single_line(s);
            PromptInjectionScanner::scan_and_sanitize(&r).sanitized
        });
        let body_id = if input.body_id.trim().is_empty() {
            compute_body_id()
        } else {
            input.body_id.trim().to_string()
        };
        let canonical_type = crate::memory::MemoryManager::normalize_memory_type(&input.memory_type)
            .unwrap_or("fact")
            .to_string();

        PgTxWriteInput {
            body_id,
            tenant_scope: input.tenant_scope.clone(),
            fact_key: input.fact_key.clone(),
            memory_type: canonical_type,
            evidence_tier: input.evidence_tier.as_str().to_string(),
            content: scan_content.sanitized,
            summary: clean_summary,
            tags: input.tags.clone(),
            salience_score: input.salience_score.clamp(0.0, 1.0),
            expected_epoch: input.expected_epoch,
            source_agent: input.source_agent.clone(),
            source_session: input.source_session.clone(),
            embedding_ref: input.embedding_ref.clone(),
        }
    }

    fn append_with_engine(
        &self,
        engine: &PgTxEngine,
        input: &TxMemoryWriteInput,
    ) -> Result<TxWriteOutcome> {
        let sanitized = Self::sanitize_input(input);
        let res = engine.append_event(&sanitized)?;

        if self.config.mirror_writes_to_sqlite {
            let mirror_rec = MemoryRecord {
                id: format!("pgtx-{}", sanitized.fact_key.replace(':', "_")),
                memory_type: sanitized.memory_type.clone(),
                content: sanitized.content.clone(),
                summary: sanitized.summary.clone(),
                source_agent: sanitized.source_agent.clone(),
                source_session: sanitized.source_session.clone(),
                tags: sanitized.tags.clone(),
                weight: sanitized.salience_score.clamp(0.1, 1.0),
                pinned: false,
                status: "active".to_string(),
                embedding: vec![],
            };
            if let Err(e) = self.storage.insert_memory(&mirror_rec) {
                eprintln!("[PgTx][WARN] SQLite 本地镜像写入失败（不影响 PG 主事务）: {}", e);
            }
        }

        AuditLogger::record_best_effort(
            &self.storage,
            AuditEventInput::in_agent("memory", "memory_op", "pg_tx_append")
                .with_target("fact_key", &sanitized.fact_key)
                .with_concurrency_mode("OPTIMISTIC_MVCC")
                .with_outcome("success")
                .with_delta(serde_json::json!({
                    "event_id": res.event_id,
                    "tx_epoch": res.tx_epoch,
                    "superseded_event_id": res.superseded_event_id,
                    "retries": res.retries,
                    "evidence_tier": sanitized.evidence_tier,
                })),
        );

        Ok(TxWriteOutcome {
            event_id: res.event_id,
            tx_epoch: res.tx_epoch,
            superseded_event_id: res.superseded_event_id,
            retries: res.retries,
            evidence_sha256: res.evidence_sha256,
        })
    }
}

fn convert_row(row: PgTxEventRow) -> TxMemoryEvent {
    TxMemoryEvent {
        event_id: row.event_id,
        body_id: row.body_id,
        tenant_scope: row.tenant_scope,
        fact_key: row.fact_key,
        memory_type: row.memory_type,
        content: row.content,
        summary: row.summary,
        tags: row.tags,
        evidence_tier: row.evidence_tier,
        evidence_sha256: row.evidence_sha256,
        valid_from: row.valid_from,
        valid_to: row.valid_to,
        tx_epoch: row.tx_epoch,
        embedding_status: row.embedding_status,
        salience_score: row.salience_score,
        consolidation_st: row.consolidation_st,
        created_at: row.created_at,
        score: row.score,
    }
}

fn event_to_item(ev: TxMemoryEvent) -> MemoryItem {
    let mut extra = HashMap::new();
    extra.insert("fact_key".to_string(), ev.fact_key);
    extra.insert("tx_epoch".to_string(), ev.tx_epoch.to_string());
    extra.insert("evidence_tier".to_string(), ev.evidence_tier);
    extra.insert("evidence_sha256".to_string(), ev.evidence_sha256);
    extra.insert("valid_from".to_string(), ev.valid_from);
    if let Some(vt) = ev.valid_to {
        extra.insert("valid_to".to_string(), vt);
    }
    extra.insert("embedding_status".to_string(), ev.embedding_status);
    MemoryItem {
        id: format!("pgtx-ev-{}", ev.event_id),
        memory_type: ev.memory_type,
        content: ev.content,
        summary: ev.summary,
        tags: ev.tags,
        source_agent: Some("pg_tx".to_string()),
        source_session: None,
        created_at: ev.created_at,
        score: ev.score,
        extra,
    }
}

impl MemoryProvider for PgTxMemoryAdapter {
    fn provider_name(&self) -> &'static str {
        if self.engine.is_some() {
            "pg_tx"
        } else {
            "pg_tx_fallback_sqlite"
        }
    }

    fn search(
        &self,
        query: &str,
        limit: usize,
        filter: Option<&MemoryFilter>,
    ) -> Result<Vec<MemoryItem>> {
        let Some(engine) = &self.engine else {
            return self.fallback.search(query, limit, filter);
        };
        let snap = engine.search(query, &self.config.tenant_scope, limit, None, false)?;
        let mut items: Vec<MemoryItem> = snap
            .hits
            .into_iter()
            .map(|r| event_to_item(convert_row(r)))
            .collect();
        if let Some(f) = filter {
            if let Some(mt) = f.memory_type.as_deref() {
                items.retain(|i| i.memory_type == mt);
            }
            if !f.tags.is_empty() {
                items.retain(|i| f.tags.iter().all(|t| i.tags.contains(t)));
            }
            if let Some(min_s) = f.min_score {
                items.retain(|i| i.score.unwrap_or(0.0) >= min_s);
            }
        }
        Ok(items)
    }

    fn store(&self, item: &MemoryItem) -> Result<String> {
        let Some(engine) = &self.engine else {
            return self.fallback.store(item);
        };
        let fact_key = item
            .extra
            .get("fact_key")
            .cloned()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| {
                if item.id.trim().is_empty() {
                    format!("mem:{}", uuid::Uuid::new_v4())
                } else {
                    item.id.clone()
                }
            });
        let tier = item
            .extra
            .get("evidence_tier")
            .map(|s| TxEvidenceTier::parse_str(s))
            .unwrap_or(TxEvidenceTier::L3Semantic);
        let input = TxMemoryWriteInput {
            body_id: compute_body_id(),
            tenant_scope: self.config.tenant_scope.clone(),
            fact_key,
            memory_type: item.memory_type.clone(),
            content: item.content.clone(),
            summary: item.summary.clone(),
            tags: item.tags.clone(),
            evidence_tier: tier,
            salience_score: 0.7,
            expected_epoch: None,
            source_agent: item.source_agent.clone(),
            source_session: item.source_session.clone(),
            embedding_ref: item.extra.get("embedding_ref").cloned(),
        };
        let out = self.append_with_engine(engine, &input)?;
        Ok(format!("pgtx-ev-{}@epoch:{}", out.event_id, out.tx_epoch))
    }

    fn get_context(&self, session_id: &str) -> Result<SessionContext> {
        let Some(_) = &self.engine else {
            return self.fallback.get_context(session_id);
        };
        let all = self.search("", 30, None)?;
        let mut key_decisions = Vec::new();
        let mut active_tasks = Vec::new();
        let mut relevant_experiences = Vec::new();
        for item in all {
            match item.memory_type.as_str() {
                "decision" => key_decisions.push(item),
                "task" => active_tasks.push(item),
                _ => relevant_experiences.push(item),
            }
        }
        Ok(SessionContext {
            session_id: session_id.to_string(),
            key_decisions,
            active_tasks,
            relevant_experiences,
        })
    }

    fn evict(&self, criteria: &EvictCriteria) -> Result<usize> {
        // 事务底座只允许显式墓碑化或通过睡眠整理队列归档；本地镜像交由 fallback 处理
        self.fallback.evict(criteria)
    }
}

impl TxMemoryProvider for PgTxMemoryAdapter {
    fn append_tx_event(&self, input: &TxMemoryWriteInput) -> Result<TxWriteOutcome> {
        if let Some(engine) = &self.engine {
            return self.append_with_engine(engine, input);
        }
        // SQLite 降级写入路径
        let rec = MemoryRecord {
            id: format!("fallback-{}", input.fact_key.replace(':', "_")),
            memory_type: crate::memory::MemoryManager::normalize_memory_type(&input.memory_type)
                .unwrap_or("fact")
                .to_string(),
            content: SecretRedactor::redact_preserving_newlines(&input.content),
            summary: input
                .summary
                .as_deref()
                .map(SecretRedactor::redact_single_line),
            source_agent: input.source_agent.clone(),
            source_session: input.source_session.clone(),
            tags: input.tags.clone(),
            weight: input.salience_score.clamp(0.1, 1.0),
            pinned: false,
            status: "active".to_string(),
            embedding: vec![],
        };
        self.storage.insert_memory(&rec)?;
        Ok(TxWriteOutcome {
            event_id: 0,
            tx_epoch: 0,
            superseded_event_id: None,
            retries: 0,
            evidence_sha256: crate::benchmark::provenance::sha256_hex(&rec.content),
        })
    }

    fn tombstone_fact(
        &self,
        body_id: &str,
        tenant_scope: &str,
        fact_key: &str,
        reason: &str,
    ) -> Result<u64> {
        let count = if let Some(engine) = &self.engine {
            let resolved_body = if body_id.trim().is_empty() {
                compute_body_id()
            } else {
                body_id.trim().to_string()
            };
            engine.tombstone_fact(&resolved_body, tenant_scope, fact_key, reason)?
        } else {
            0
        };
        // 同步归档本地 SQLite 镜像记录
        let mirror_id = format!("pgtx-{}", fact_key.replace(':', "_"));
        if let Err(e) = self.storage.update_memory_status(&mirror_id, "archived") {
            eprintln!("[PgTx][WARN] 归档 SQLite 镜像记录 '{}' 失败: {}", mirror_id, e);
        }
        AuditLogger::record_best_effort(
            &self.storage,
            AuditEventInput::out_of_agent(
                "memory_adapter",
                "pg_tx_memory",
                "memory_op",
                "tombstone_fact",
            )
            .with_target("fact_key", fact_key)
            .with_outcome("success")
            .with_delta(serde_json::json!({
                "tombstoned_events": count,
                "reason": reason,
            })),
        );
        Ok(count)
    }

    fn search_tx(&self, query: &str, opts: &TxSearchOptions) -> Result<TxMemorySnapshot> {
        if let Some(engine) = &self.engine {
            let scope = if opts.tenant_scope.trim().is_empty() {
                self.config.tenant_scope.as_str()
            } else {
                opts.tenant_scope.trim()
            };
            // Phase 77 M2.3: 查询可预测性评估，若遇到探索型或低摘要覆盖查询且非时间旅行/历史请求，自动旁路预整理摘要直查原始事件
            let should_bypass = if opts.as_of_epoch.is_none() && !opts.include_history && !query.trim().is_empty() {
                let summaries = engine.list_sleep_summary_texts(scope, 40).unwrap_or_default();
                if summaries.is_empty() {
                    false
                } else {
                    let s_refs: Vec<&str> = summaries.iter().map(|s| s.as_str()).collect();
                    let dec = ext_mem_pg_tx::QueryPredictabilityGate::evaluate(
                        query,
                        &s_refs,
                        opts.as_of_epoch,
                        opts.include_history,
                    );
                    dec.bypass_compacted
                }
            } else {
                false
            };

            let snap = if should_bypass {
                engine.search_raw_bypass(query, scope, opts.limit)?
            } else {
                let regular = engine.search(
                    query,
                    scope,
                    opts.limit,
                    opts.as_of_epoch,
                    opts.include_history,
                )?;
                // 兜底防漏召回：若常规解耦段未命中且未开启时间旅行，尝试旁路直查未墓碑原始事件
                if regular.hits.is_empty()
                    && opts.as_of_epoch.is_none()
                    && !opts.include_history
                    && !query.trim().is_empty()
                {
                    engine.search_raw_bypass(query, scope, opts.limit)?
                } else {
                    regular
                }
            };
            return Ok(TxMemorySnapshot {
                snapshot_epoch: snap.snapshot_epoch,
                backend: "pg_tx".to_string(),
                fallback_active: false,
                hits: snap.hits.into_iter().map(convert_row).collect(),
            });
        }
        // SQLite 降级快照检索
        let local = if query.trim().is_empty() {
            self.storage.list_all_active_memories(opts.limit)?
        } else {
            self.storage.search_memories_fts(query, opts.limit)?
        };
        let now = Utc::now().to_rfc3339();
        let hits = local
            .into_iter()
            .enumerate()
            .map(|(idx, m)| TxMemoryEvent {
                event_id: (idx as i64) + 1,
                body_id: compute_body_id(),
                tenant_scope: opts.tenant_scope.clone(),
                fact_key: m.id,
                memory_type: m.memory_type,
                content: m.content,
                summary: m.summary,
                tags: m.tags,
                evidence_tier: "L3_SEMANTIC".to_string(),
                evidence_sha256: String::new(),
                valid_from: now.clone(),
                valid_to: None,
                tx_epoch: 0,
                embedding_status: "fresh".to_string(),
                salience_score: m.weight,
                consolidation_st: "raw".to_string(),
                created_at: now.clone(),
                score: Some(m.weight as f64),
            })
            .collect();
        Ok(TxMemorySnapshot {
            snapshot_epoch: 0,
            backend: "sqlite_fallback".to_string(),
            fallback_active: true,
            hits,
        })
    }

    fn fact_history(
        &self,
        body_id: &str,
        tenant_scope: &str,
        fact_key: &str,
        limit: usize,
    ) -> Result<Vec<TxMemoryEvent>> {
        let Some(engine) = &self.engine else {
            return Ok(vec![]);
        };
        let rows = engine.fact_history(body_id, tenant_scope, fact_key, limit)?;
        Ok(rows.into_iter().map(convert_row).collect())
    }

    fn current_tx_epoch(&self) -> Result<i64> {
        match &self.engine {
            Some(engine) => engine.current_epoch(),
            None => Ok(0),
        }
    }

    fn tx_health(&self) -> Result<TxProviderHealth> {
        if let Some(engine) = &self.engine {
            let st = engine.status()?;
            return Ok(TxProviderHealth {
                provider: "pg_tx".to_string(),
                connected: st.connected,
                fallback_active: false,
                current_epoch: st.current_epoch,
                active_events: st.active_events,
                tombstoned_events: st.tombstoned_events,
                active_segments: st.active_segments,
                deleted_segments: st.deleted_segments,
                pool_size: st.pool_max_size,
                vector_backend: st.vector_backend,
                server_version: Some(st.server_version),
                detail: format!(
                    "PG 事务海马体运行中 (pool active={}/idle={}/max={}, extensions={:?})",
                    st.pool_active, st.pool_idle, st.pool_max_size, st.detected_extensions
                ),
            });
        }
        Ok(TxProviderHealth {
            provider: "pg_tx_fallback_sqlite".to_string(),
            connected: false,
            fallback_active: true,
            current_epoch: 0,
            active_events: 0,
            tombstoned_events: 0,
            active_segments: 0,
            deleted_segments: 0,
            pool_size: self.config.pool_size,
            vector_backend: self.config.vector_backend.clone(),
            server_version: None,
            detail: self
                .fallback_reason
                .clone()
                .unwrap_or_else(|| "使用本地 SQLite 降级模式".to_string()),
        })
    }
}

fn pg_snapshot_to_spi(s: ext_mem_pg_tx::PgFastWeightSnapshot) -> FastWeightSnapshotMeta {
    FastWeightSnapshotMeta {
        snapshot_id: s.snapshot_id,
        body_id: s.body_id,
        tenant_scope: s.tenant_scope,
        base_model_id: s.base_model_id,
        parent_snapshot: s.parent_snapshot,
        anchored_epoch: s.anchored_epoch,
        adapter_kind: s.adapter_kind,
        artifact_uri: s.artifact_uri,
        norm_ratio: s.norm_ratio,
        status: s.status,
        created_at: s.created_at,
    }
}

impl FastWeightCheckpointSpi for PgTxMemoryAdapter {
    fn register_snapshot(&self, meta: &FastWeightSnapshotMeta) -> Result<()> {
        let Some(engine) = &self.engine else {
            anyhow::bail!("PostgreSQL 事务底座未激活（当前 SQLite 降级模式），无法写入 fast_weight_snapshots");
        };
        let body_id = if meta.body_id.trim().is_empty() {
            compute_body_id()
        } else {
            meta.body_id.trim().to_string()
        };
        let scope = if meta.tenant_scope.trim().is_empty() {
            self.config.tenant_scope.as_str()
        } else {
            meta.tenant_scope.trim()
        };
        let policy = ext_mem_pg_tx::FastWeightGatePolicy::default();
        let snap_opt = Some(meta.snapshot_id.as_str()).filter(|s| !s.trim().is_empty());
        engine.register_snapshot_meta(
            snap_opt,
            &body_id,
            scope,
            &meta.base_model_id,
            meta.parent_snapshot.as_deref(),
            meta.anchored_epoch,
            &meta.adapter_kind,
            &meta.artifact_uri,
            meta.norm_ratio,
            &meta.status,
            &policy,
        )?;
        Ok(())
    }

    fn active_snapshot(
        &self,
        body_id: &str,
        tenant_scope: &str,
    ) -> Result<Option<FastWeightSnapshotMeta>> {
        let Some(engine) = &self.engine else {
            return Ok(None);
        };
        let body = if body_id.trim().is_empty() {
            compute_body_id()
        } else {
            body_id.trim().to_string()
        };
        Ok(engine
            .active_fast_weight_snapshot(&body, tenant_scope)?
            .map(pg_snapshot_to_spi))
    }

    fn rollback_to_epoch(
        &self,
        body_id: &str,
        tenant_scope: &str,
        target_epoch: i64,
    ) -> Result<Option<FastWeightSnapshotMeta>> {
        let Some(engine) = &self.engine else {
            return Ok(None);
        };
        let body = if body_id.trim().is_empty() {
            compute_body_id()
        } else {
            body_id.trim().to_string()
        };
        let trace_id = format!("spi-rb-{}", uuid::Uuid::new_v4());
        let outcome = engine.rollback_fast_weight_to_epoch(&body, tenant_scope, target_epoch, None, &trace_id)?;
        Ok(outcome.restored_snapshot.map(pg_snapshot_to_spi))
    }
}
