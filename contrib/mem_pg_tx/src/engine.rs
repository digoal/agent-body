//! PostgreSQL 事务级双时态海马体核心引擎（方案 A M1 · MVCC + 写穿失效 + 解耦检索段）

use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use postgres::error::SqlState;
use postgres::IsolationLevel;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::pool::PgConnPool;
use crate::schema::PG_TX_SCHEMA_SQL;

/// 解耦向量索引后端选项
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VectorBackendKind {
    None,
    Pgvector,
    PgVectorsearch,
    ExternalLsm,
}

impl VectorBackendKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Pgvector => "pgvector",
            Self::PgVectorsearch => "pg_vectorsearch",
            Self::ExternalLsm => "external_lsm",
        }
    }

    pub fn parse_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" => Ok(Self::None),
            "pgvector" | "vector" => Ok(Self::Pgvector),
            "pg_vectorsearch" | "vectorsearch" => Ok(Self::PgVectorsearch),
            "external_lsm" | "lsm" => Ok(Self::ExternalLsm),
            other => anyhow::bail!(
                "不支持的 vector_backend '{}'，允许值: none | pgvector | pg_vectorsearch | external_lsm",
                other
            ),
        }
    }
}

/// PostgreSQL 事务级记忆配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgTxConfig {
    pub pg_url: String,
    pub pool_size: usize,
    pub tenant_scope: String,
    pub vector_backend: VectorBackendKind,
    pub max_occ_retries: u32,
}

impl PgTxConfig {
    pub fn new(pg_url: impl Into<String>) -> Self {
        Self {
            pg_url: pg_url.into(),
            pool_size: 8,
            tenant_scope: "default".to_string(),
            vector_backend: VectorBackendKind::None,
            max_occ_retries: 16,
        }
    }
}

/// 写入一条双时态记忆事件的输入载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgTxWriteInput {
    pub body_id: String,
    pub tenant_scope: String,
    pub fact_key: String,
    pub memory_type: String,
    pub evidence_tier: String,
    pub content: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub salience_score: f32,
    pub expected_epoch: Option<i64>,
    pub source_agent: Option<String>,
    pub source_session: Option<String>,
    pub embedding_ref: Option<String>,
}

/// 写穿事务提交结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgTxWriteResult {
    pub event_id: i64,
    pub tx_epoch: i64,
    pub superseded_event_id: Option<i64>,
    pub retries: u32,
    pub evidence_sha256: String,
}

/// 双时态记忆事件视图
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgTxEventRow {
    pub event_id: i64,
    pub body_id: String,
    pub tenant_scope: String,
    pub fact_key: String,
    pub memory_type: String,
    pub evidence_tier: String,
    pub content: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub evidence_sha256: String,
    pub valid_from: String,
    pub valid_to: Option<String>,
    pub tx_epoch: i64,
    pub embedding_status: String,
    pub salience_score: f32,
    pub consolidation_st: String,
    pub created_at: String,
    pub score: Option<f64>,
}

/// 快照检索结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgTxSearchSnapshot {
    pub snapshot_epoch: i64,
    pub hits: Vec<PgTxEventRow>,
}

/// 事务记忆库健康态统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgTxStatus {
    pub connected: bool,
    pub current_epoch: i64,
    pub active_events: i64,
    pub tombstoned_events: i64,
    pub active_segments: i64,
    pub deleted_segments: i64,
    pub pool_max_size: usize,
    pub pool_active: usize,
    pub pool_idle: usize,
    pub vector_backend: String,
    pub detected_extensions: Vec<String>,
    pub server_version: String,
}

/// PostgreSQL 事务级双时态记忆核心引擎
#[derive(Clone)]
pub struct PgTxEngine {
    pool: Arc<PgConnPool>,
    config: PgTxConfig,
}

impl PgTxEngine {
    /// 建立有界连接池并幂等初始化四表 Schema
    pub fn connect(config: PgTxConfig) -> Result<Self> {
        let pool = PgConnPool::new(config.pg_url.clone(), config.pool_size)?;
        let engine = Self { pool, config };
        engine.init_schema()?;
        Ok(engine)
    }

    pub fn config(&self) -> &PgTxConfig {
        &self.config
    }

    /// 幂等建表与索引初始化
    pub fn init_schema(&self) -> Result<()> {
        let mut client = self.pool.checkout()?;
        client
            .batch_execute(PG_TX_SCHEMA_SQL)
            .context("初始化 PostgreSQL ext_mem_pg_tx__* 四表 Schema 失败")?;
        Ok(())
    }

    /// 计算规范化 SHA-256 存证字节
    pub fn compute_evidence_hash(
        body_id: &str,
        tenant_scope: &str,
        fact_key: &str,
        memory_type: &str,
        content: &str,
    ) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.update(body_id.as_bytes());
        hasher.update(b"|");
        hasher.update(tenant_scope.as_bytes());
        hasher.update(b"|");
        hasher.update(fact_key.as_bytes());
        hasher.update(b"|");
        hasher.update(memory_type.as_bytes());
        hasher.update(b"|");
        hasher.update(content.as_bytes());
        hasher.finalize().to_vec()
    }

    /// 单事务 `REPEATABLE READ` 写穿提交 + OCC 自动重试（防多 Agent 并发丢写，严禁跨推理持 2PL 长锁）
    pub fn append_event(&self, input: &PgTxWriteInput) -> Result<PgTxWriteResult> {
        if input.fact_key.trim().is_empty() {
            anyhow::bail!("fact_key 不能为空");
        }
        if input.content.trim().is_empty() {
            anyhow::bail!("content 不能为空");
        }

        let tier = normalize_tier(&input.evidence_tier);
        let scope = if input.tenant_scope.trim().is_empty() {
            self.config.tenant_scope.as_str()
        } else {
            input.tenant_scope.trim()
        };
        let hash_bytes = Self::compute_evidence_hash(
            input.body_id.trim(),
            scope,
            input.fact_key.trim(),
            input.memory_type.trim(),
            input.content.trim(),
        );
        let hash_hex = bytes_to_hex(&hash_bytes);

        let payload = serde_json::json!({
            "content": input.content,
            "summary": input.summary,
            "tags": input.tags,
            "source_agent": input.source_agent,
            "source_session": input.source_session,
        });
        let search_text = build_search_text(
            input.fact_key.trim(),
            &input.content,
            input.summary.as_deref(),
            &input.tags,
        );
        let max_retries = self.config.max_occ_retries.clamp(1, 64);

        for attempt in 0..=max_retries {
            match self.try_append_once(
                input,
                scope,
                tier,
                &payload,
                &hash_bytes,
                &search_text,
            ) {
                Ok((event_id, tx_epoch, superseded_event_id)) => {
                    return Ok(PgTxWriteResult {
                        event_id,
                        tx_epoch,
                        superseded_event_id,
                        retries: attempt,
                        evidence_sha256: hash_hex,
                    });
                }
                Err(AttemptErr::Retryable(msg)) if attempt < max_retries => {
                    // 确定性微抖动退避（1~20ms），避免 16+ 并发热点活锁
                    let backoff_ms = ((attempt as u64 + 1) * 2).min(20);
                    let jitter_us = (u64::from(hash_bytes[0]) + (attempt as u64 * 37)) % 900;
                    std::thread::sleep(Duration::from_micros(backoff_ms * 1000 + jitter_us));
                    if attempt + 1 == max_retries {
                        eprintln!("[PgTx][WARN] OCC 并发写接近重试上限: {}", msg);
                    }
                }
                Err(AttemptErr::Retryable(msg)) => {
                    anyhow::bail!("OCC 并发冲突重试 {} 次后仍失败: {}", max_retries, msg);
                }
                Err(AttemptErr::Fatal(e)) => return Err(e),
            }
        }

        Err(anyhow!("OCC 写穿循环异常退出"))
    }

    fn try_append_once(
        &self,
        input: &PgTxWriteInput,
        scope: &str,
        tier: &str,
        payload: &serde_json::Value,
        hash_bytes: &[u8],
        search_text: &str,
    ) -> std::result::Result<(i64, i64, Option<i64>), AttemptErr> {
        let mut client = self.pool.checkout().map_err(AttemptErr::Fatal)?;
        let mut tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()
            .map_err(classify_pg_err)?;

        // 0. 同 fact_key 亚毫秒级提交期微序列化（绝不跨推理持锁，不同 fact_key 100% 并行，消除 16+ 线程热点 40001 惊群）
        tx.execute(
            "SELECT pg_advisory_xact_lock(hashtextextended($1 || '|' || $2 || '|' || $3, 0))",
            &[&input.body_id, &scope, &input.fact_key],
        )
        .map_err(classify_pg_err)?;

        // 1. 读取当前有效行（若有）并锁定单行避免并发截断交错
        let prev = tx
            .query_opt(
                "SELECT event_id, tx_epoch
                 FROM ext_mem_pg_tx__events
                 WHERE body_id = $1 AND tenant_scope = $2 AND fact_key = $3
                   AND upper_inf(valid_range) AND embedding_status <> 'tombstoned'
                 LIMIT 1
                 FOR UPDATE",
                &[&input.body_id, &scope, &input.fact_key],
            )
            .map_err(classify_pg_err)?;

        let (superseded_id, prev_epoch) = match prev {
            Some(row) => (Some(row.get::<_, i64>(0)), Some(row.get::<_, i64>(1))),
            None => (None, None),
        };

        // 2. 可选严格 CAS 期望纪元校验（用于 AOPCC 租约严格写防护）
        if let Some(expected) = input.expected_epoch {
            if prev_epoch != Some(expected) {
                return Err(AttemptErr::Fatal(anyhow!(
                    "MVCC_EPOCH_CONFLICT: fact_key='{}' 期望 tx_epoch={}，当前库内实际={:?}",
                    input.fact_key,
                    expected,
                    prev_epoch
                )));
            }
        }

        // 3. 写穿截断旧有效窗 + 标记旧检索段为 is_deleted（保证 0% 僵尸召回）
        let cut_ts: DateTime<Utc> = if let Some(old_id) = superseded_id {
            let row = tx
                .query_one(
                    "UPDATE ext_mem_pg_tx__events
                     SET valid_range = tstzrange(
                            lower(valid_range),
                            GREATEST(clock_timestamp(), lower(valid_range) + interval '1 microsecond'),
                            '[)'
                         ),
                         embedding_status = 'stale'
                     WHERE event_id = $1
                     RETURNING upper(valid_range)",
                    &[&old_id],
                )
                .map_err(classify_pg_err)?;
            tx.execute(
                "UPDATE ext_mem_pg_tx__segments
                 SET is_deleted = TRUE, updated_at = clock_timestamp()
                 WHERE event_id = $1 AND NOT is_deleted",
                &[&old_id],
            )
            .map_err(classify_pg_err)?;
            row.get(0)
        } else {
            tx.query_one("SELECT clock_timestamp()", &[])
                .map_err(classify_pg_err)?
                .get(0)
        };

        // 4. 分配单调逻辑纪元并插入新事件 + 新解耦检索段
        let new_epoch: i64 = tx
            .query_one("SELECT nextval('ext_mem_pg_tx__epoch_seq')", &[])
            .map_err(classify_pg_err)?
            .get(0);

        let emb_status = if input.embedding_ref.as_deref().is_some_and(|s| !s.is_empty()) {
            "fresh"
        } else {
            "pending"
        };

        let inserted = tx
            .query_one(
                "INSERT INTO ext_mem_pg_tx__events (
                    body_id, tenant_scope, fact_key, memory_type, evidence_tier,
                    payload_redacted, evidence_sha256, valid_range, tx_epoch,
                    embedding_status, salience_score, consolidation_st
                 ) VALUES (
                    $1, $2, $3, $4, $5,
                    $6, $7, tstzrange($8, NULL, '[)'), $9,
                    $10, $11, 'raw'
                 )
                 RETURNING event_id",
                &[
                    &input.body_id,
                    &scope,
                    &input.fact_key,
                    &input.memory_type,
                    &tier,
                    payload,
                    &hash_bytes,
                    &cut_ts,
                    &new_epoch,
                    &emb_status,
                    &input.salience_score,
                ],
            )
            .map_err(classify_pg_err)?;
        let new_event_id: i64 = inserted.get(0);

        tx.execute(
            "INSERT INTO ext_mem_pg_tx__segments (event_id, fts_tsv, embedding_ref, is_deleted)
             VALUES ($1, to_tsvector('simple', $2), $3, FALSE)",
            &[&new_event_id, &search_text, &input.embedding_ref],
        )
        .map_err(classify_pg_err)?;

        tx.commit().map_err(classify_pg_err)?;
        Ok((new_event_id, new_epoch, superseded_id))
    }

    /// 物理删除/合规擦除墓碑化（关闭有效窗 + 置 tombstoned + 检索段全部标删，确保 0% 僵尸召回）
    pub fn tombstone_fact(
        &self,
        body_id: &str,
        tenant_scope: &str,
        fact_key: &str,
        _reason: &str,
    ) -> Result<u64> {
        let scope = if tenant_scope.trim().is_empty() {
            self.config.tenant_scope.as_str()
        } else {
            tenant_scope.trim()
        };
        let max_retries = self.config.max_occ_retries.clamp(1, 32);

        for attempt in 0..=max_retries {
            let mut client = self.pool.checkout()?;
            let mut tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::ReadCommitted)
                .start()?;

            tx.execute(
                "SELECT pg_advisory_xact_lock(hashtextextended($1 || '|' || $2 || '|' || $3, 0))",
                &[&body_id, &scope, &fact_key],
            )?;

            let updated_res = tx.query(
                "UPDATE ext_mem_pg_tx__events
                 SET valid_range = CASE
                        WHEN upper_inf(valid_range) THEN tstzrange(
                            lower(valid_range),
                            GREATEST(clock_timestamp(), lower(valid_range) + interval '1 microsecond'),
                            '[)'
                        )
                        ELSE valid_range
                     END,
                     embedding_status = 'tombstoned',
                     consolidation_st = 'purged'
                 WHERE body_id = $1 AND tenant_scope = $2 AND fact_key = $3
                   AND embedding_status <> 'tombstoned'
                 RETURNING event_id",
                &[&body_id, &scope, &fact_key],
            );

            let rows = match updated_res.map_err(classify_pg_err) {
                Ok(r) => r,
                Err(AttemptErr::Retryable(_)) if attempt < max_retries => {
                    std::thread::sleep(Duration::from_millis((attempt as u64 + 1) * 2));
                    continue;
                }
                Err(AttemptErr::Retryable(m)) => anyhow::bail!("墓碑事务重试耗尽: {}", m),
                Err(AttemptErr::Fatal(e)) => return Err(e),
            };

            let ids: Vec<i64> = rows.iter().map(|r| r.get(0)).collect();
            if !ids.is_empty() {
                if let Err(err) = tx
                    .execute(
                        "UPDATE ext_mem_pg_tx__segments
                         SET is_deleted = TRUE, updated_at = clock_timestamp()
                         WHERE event_id = ANY($1)",
                        &[&ids],
                    )
                    .map_err(classify_pg_err)
                {
                    match err {
                        AttemptErr::Retryable(_) if attempt < max_retries => continue,
                        AttemptErr::Retryable(m) => anyhow::bail!("段墓碑标记重试耗尽: {}", m),
                        AttemptErr::Fatal(e) => return Err(e),
                    }
                }
            }

            match tx.commit().map_err(classify_pg_err) {
                Ok(()) => return Ok(ids.len() as u64),
                Err(AttemptErr::Retryable(_)) if attempt < max_retries => continue,
                Err(AttemptErr::Retryable(m)) => anyhow::bail!("墓碑提交重试耗尽: {}", m),
                Err(AttemptErr::Fatal(e)) => return Err(e),
            }
        }
        Ok(0)
    }

    /// 双时态只读快照检索（默认严格过滤 `NOT is_deleted AND embedding_status <> 'tombstoned'`）
    pub fn search(
        &self,
        query: &str,
        tenant_scope: &str,
        limit: usize,
        as_of_epoch: Option<i64>,
        include_history: bool,
    ) -> Result<PgTxSearchSnapshot> {
        let scope = if tenant_scope.trim().is_empty() {
            self.config.tenant_scope.as_str()
        } else {
            tenant_scope.trim()
        };
        let lim = limit.clamp(1, 200) as i64;
        let q = query.trim();

        let mut client = self.pool.checkout()?;
        let mut tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::RepeatableRead)
            .read_only(true)
            .start()
            .context("开启只读快照事务失败")?;

        let snapshot_epoch: i64 = tx
            .query_one(
                "SELECT COALESCE(MAX(tx_epoch), 0) FROM ext_mem_pg_tx__events WHERE tenant_scope = $1",
                &[&scope],
            )?
            .get(0);

        let rows = if let Some(target_epoch) = as_of_epoch {
            // 逻辑纪元时间旅行快照：每个 fact_key 仅取 tx_epoch <= target_epoch 的最新版本
            tx.query(
                "WITH as_of AS (
                    SELECT DISTINCT ON (e.body_id, e.tenant_scope, e.fact_key)
                           e.event_id, e.body_id, e.tenant_scope, e.fact_key, e.memory_type,
                           e.evidence_tier, e.payload_redacted, e.evidence_sha256,
                           lower(e.valid_range) AS valid_from,
                           CASE WHEN upper_inf(e.valid_range) THEN NULL ELSE upper(e.valid_range) END AS valid_to,
                           e.tx_epoch, e.embedding_status, e.salience_score, e.consolidation_st, e.created_at,
                           s.fts_tsv
                    FROM ext_mem_pg_tx__events e
                    JOIN ext_mem_pg_tx__segments s ON s.event_id = e.event_id
                    WHERE e.tenant_scope = $1 AND e.tx_epoch <= $2
                    ORDER BY e.body_id, e.tenant_scope, e.fact_key, e.tx_epoch DESC
                 )
                 SELECT event_id, body_id, tenant_scope, fact_key, memory_type,
                        evidence_tier, payload_redacted, evidence_sha256,
                        valid_from, valid_to, tx_epoch, embedding_status,
                        salience_score, consolidation_st, created_at,
                        CASE
                            WHEN $3 = '' THEN 1.0::double precision
                            WHEN fts_tsv @@ plainto_tsquery('simple', $3)
                                THEN (ts_rank(fts_tsv, plainto_tsquery('simple', $3)) + 1.0)::double precision
                            ELSE 0.5::double precision
                        END AS rank_score
                 FROM as_of
                 WHERE embedding_status <> 'tombstoned'
                   AND (
                       $3 = ''
                       OR fts_tsv @@ plainto_tsquery('simple', $3)
                       OR payload_redacted::text ILIKE '%' || $3 || '%'
                       OR fact_key ILIKE '%' || $3 || '%'
                   )
                 ORDER BY rank_score DESC, tx_epoch DESC
                 LIMIT $4",
                &[&scope, &target_epoch, &q, &lim],
            )?
        } else if include_history {
            tx.query(
                "SELECT e.event_id, e.body_id, e.tenant_scope, e.fact_key, e.memory_type,
                        e.evidence_tier, e.payload_redacted, e.evidence_sha256,
                        lower(e.valid_range) AS valid_from,
                        CASE WHEN upper_inf(e.valid_range) THEN NULL ELSE upper(e.valid_range) END AS valid_to,
                        e.tx_epoch, e.embedding_status, e.salience_score, e.consolidation_st, e.created_at,
                        CASE
                            WHEN $2 = '' THEN 1.0::double precision
                            WHEN s.fts_tsv @@ plainto_tsquery('simple', $2)
                                THEN (ts_rank(s.fts_tsv, plainto_tsquery('simple', $2)) + 1.0)::double precision
                            ELSE 0.5::double precision
                        END AS rank_score
                 FROM ext_mem_pg_tx__events e
                 JOIN ext_mem_pg_tx__segments s ON s.event_id = e.event_id
                 WHERE e.tenant_scope = $1
                   AND (
                       $2 = ''
                       OR s.fts_tsv @@ plainto_tsquery('simple', $2)
                       OR e.payload_redacted::text ILIKE '%' || $2 || '%'
                       OR e.fact_key ILIKE '%' || $2 || '%'
                   )
                 ORDER BY e.tx_epoch DESC
                 LIMIT $3",
                &[&scope, &q, &lim],
            )?
        } else {
            // 热路径默认视图：仅扫描未删解耦段 + 当前有效行（0% 僵尸向量/墓碑召回）
            tx.query(
                "SELECT e.event_id, e.body_id, e.tenant_scope, e.fact_key, e.memory_type,
                        e.evidence_tier, e.payload_redacted, e.evidence_sha256,
                        lower(e.valid_range) AS valid_from,
                        CASE WHEN upper_inf(e.valid_range) THEN NULL ELSE upper(e.valid_range) END AS valid_to,
                        e.tx_epoch, e.embedding_status, e.salience_score, e.consolidation_st, e.created_at,
                        CASE
                            WHEN $2 = '' THEN 1.0::double precision
                            WHEN s.fts_tsv @@ plainto_tsquery('simple', $2)
                                THEN (ts_rank(s.fts_tsv, plainto_tsquery('simple', $2)) + 1.0)::double precision
                            ELSE 0.5::double precision
                        END AS rank_score
                 FROM ext_mem_pg_tx__segments s
                 JOIN ext_mem_pg_tx__events e ON e.event_id = s.event_id
                 WHERE e.tenant_scope = $1
                   AND NOT s.is_deleted
                   AND e.embedding_status <> 'tombstoned'
                   AND upper_inf(e.valid_range)
                   AND (
                       $2 = ''
                       OR s.fts_tsv @@ plainto_tsquery('simple', $2)
                       OR e.payload_redacted::text ILIKE '%' || $2 || '%'
                       OR e.fact_key ILIKE '%' || $2 || '%'
                   )
                 ORDER BY rank_score DESC, e.tx_epoch DESC
                 LIMIT $3",
                &[&scope, &q, &lim],
            )?
        };

        tx.commit()?;
        let hits = rows.iter().map(map_event_row).collect();
        Ok(PgTxSearchSnapshot {
            snapshot_epoch,
            hits,
        })
    }

    /// 获取单个 `fact_key` 的完整双时态版本历史
    pub fn fact_history(
        &self,
        body_id: &str,
        tenant_scope: &str,
        fact_key: &str,
        limit: usize,
    ) -> Result<Vec<PgTxEventRow>> {
        let scope = if tenant_scope.trim().is_empty() {
            self.config.tenant_scope.as_str()
        } else {
            tenant_scope.trim()
        };
        let lim = limit.clamp(1, 200) as i64;
        let mut client = self.pool.checkout()?;
        let rows = client.query(
            "SELECT event_id, body_id, tenant_scope, fact_key, memory_type,
                    evidence_tier, payload_redacted, evidence_sha256,
                    lower(valid_range) AS valid_from,
                    CASE WHEN upper_inf(valid_range) THEN NULL ELSE upper(valid_range) END AS valid_to,
                    tx_epoch, embedding_status, salience_score, consolidation_st, created_at,
                    1.0::double precision AS rank_score
             FROM ext_mem_pg_tx__events
             WHERE ($1 = '' OR body_id = $1) AND tenant_scope = $2 AND fact_key = $3
             ORDER BY tx_epoch DESC
             LIMIT $4",
            &[&body_id, &scope, &fact_key, &lim],
        )?;
        Ok(rows.iter().map(map_event_row).collect())
    }

    /// O(1) 读取当前全局单调 `tx_epoch`
    pub fn current_epoch(&self) -> Result<i64> {
        let mut client = self.pool.checkout()?;
        let row = client.query_one(
            "SELECT COALESCE(MAX(tx_epoch), 0) FROM ext_mem_pg_tx__events",
            &[],
        )?;
        Ok(row.get(0))
    }

    /// 获取连接池、向量后端与表健康指标
    pub fn status(&self) -> Result<PgTxStatus> {
        let mut client = self.pool.checkout()?;
        let counts = client.query_one(
            "SELECT
                COALESCE(MAX(tx_epoch), 0) AS cur_epoch,
                COUNT(*) FILTER (WHERE upper_inf(valid_range) AND embedding_status <> 'tombstoned') AS active_ev,
                COUNT(*) FILTER (WHERE embedding_status = 'tombstoned') AS tomb_ev
             FROM ext_mem_pg_tx__events",
            &[],
        )?;
        let seg_counts = client.query_one(
            "SELECT
                COUNT(*) FILTER (WHERE NOT is_deleted) AS active_seg,
                COUNT(*) FILTER (WHERE is_deleted) AS del_seg
             FROM ext_mem_pg_tx__segments",
            &[],
        )?;
        let ver_row = client.query_one("SHOW server_version", &[])?;
        let server_version: String = ver_row.get(0);

        let ext_rows = client.query(
            "SELECT name FROM pg_available_extensions
             WHERE name IN ('btree_gist', 'vector', 'pg_vectorsearch', 'pg_trgm')
             ORDER BY name",
            &[],
        )?;
        let detected_extensions: Vec<String> = ext_rows.into_iter().map(|r| r.get(0)).collect();
        let (pool_active, pool_idle, pool_max_size) = self.pool.stats();

        Ok(PgTxStatus {
            connected: true,
            current_epoch: counts.get(0),
            active_events: counts.get(1),
            tombstoned_events: counts.get(2),
            active_segments: seg_counts.get(0),
            deleted_segments: seg_counts.get(1),
            pool_max_size,
            pool_active,
            pool_idle,
            vector_backend: self.config.vector_backend.as_str().to_string(),
            detected_extensions,
            server_version,
        })
    }
}

enum AttemptErr {
    Retryable(String),
    Fatal(anyhow::Error),
}

fn classify_pg_err(err: postgres::Error) -> AttemptErr {
    if let Some(code) = err.code() {
        if *code == SqlState::T_R_SERIALIZATION_FAILURE
            || *code == SqlState::UNIQUE_VIOLATION
            || *code == SqlState::T_R_DEADLOCK_DETECTED
        {
            return AttemptErr::Retryable(format!("PG SQLSTATE {}: {}", code.code(), err));
        }
    }
    AttemptErr::Fatal(err.into())
}

fn normalize_tier(raw: &str) -> &'static str {
    match raw.trim().to_ascii_uppercase().as_str() {
        "L1_EXPLICIT" | "L1" => "L1_EXPLICIT",
        "L2_STRUCTURAL" | "L2" => "L2_STRUCTURAL",
        _ => "L3_SEMANTIC",
    }
}

fn build_search_text(
    fact_key: &str,
    content: &str,
    summary: Option<&str>,
    tags: &[String],
) -> String {
    let mut parts = Vec::with_capacity(4);
    parts.push(fact_key.to_string());
    parts.push(content.to_string());
    if let Some(s) = summary.filter(|v| !v.trim().is_empty()) {
        parts.push(s.to_string());
    }
    if !tags.is_empty() {
        parts.push(tags.join(" "));
    }
    parts.join(" ")
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn map_event_row(row: &postgres::Row) -> PgTxEventRow {
    let payload: serde_json::Value = row.get(6);
    let content = payload
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let summary = payload
        .get("summary")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let tags = payload
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    let hash_bytes: Vec<u8> = row.try_get(7).unwrap_or_default();
    let valid_from: String = row
        .try_get::<_, DateTime<Utc>>(8)
        .map(|t| t.to_rfc3339())
        .unwrap_or_default();
    let valid_to: Option<String> = row
        .try_get::<_, Option<DateTime<Utc>>>(9)
        .ok()
        .flatten()
        .map(|t| t.to_rfc3339());
    let created_at: String = row
        .try_get::<_, DateTime<Utc>>(14)
        .map(|t| t.to_rfc3339())
        .unwrap_or_default();
    let score: f64 = row.try_get(15).unwrap_or(0.0);

    PgTxEventRow {
        event_id: row.try_get(0).unwrap_or(0),
        body_id: row.try_get(1).unwrap_or_default(),
        tenant_scope: row.try_get(2).unwrap_or_default(),
        fact_key: row.try_get(3).unwrap_or_default(),
        memory_type: row.try_get(4).unwrap_or_default(),
        evidence_tier: row.try_get(5).unwrap_or_default(),
        content,
        summary,
        tags,
        evidence_sha256: bytes_to_hex(&hash_bytes),
        valid_from,
        valid_to,
        tx_epoch: row.try_get(10).unwrap_or(0),
        embedding_status: row.try_get(11).unwrap_or_default(),
        salience_score: row.try_get(12).unwrap_or(0.0),
        consolidation_st: row.try_get(13).unwrap_or_default(),
        created_at,
        score: Some(score),
    }
}
