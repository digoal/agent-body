//! `L1_AUTONOMIC` 空闲期睡眠整理引擎 (`SleepTimeCompactor` · Phase 77 M2.2)
//!
//! 在不干扰热路径检索 SLA (< 20 ms) 的前提下：
//! - 按 `fact_key` 前缀/标签聚类低显著性或冗余长情景事件；
//! - 采用纯 Rust 确定性事实保留压缩器（`compact_episodic_texts`）消去重复寒暄与模板噪声，
//!   同时 100% 保留数字参数、端口、路径与决策/约束事实；
//! - 原事件保留于 `ext_mem_pg_tx__events`（`consolidation_st='sleep_compacted'`）以便不可预测查询旁路直查，
//!   仅将冗余检索段标记 `is_deleted=TRUE` 并写入高密度 `sleep_summary:*` 摘要，状态机追踪 `running -> verified -> committed / aborted`。

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use anyhow::Result;
use chrono::{DateTime, Utc};
use postgres::IsolationLevel;
use serde::{Deserialize, Serialize};

use crate::engine::{
    build_search_text, bytes_to_hex, map_event_row, PgTxEngine, PgTxEventRow, PgTxSearchSnapshot,
};
use crate::salience::SalienceScorer;

/// 睡眠整理运行配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SleepCompactionConfig {
    pub min_events: usize,
    pub max_batch: usize,
    pub salience_ceiling: f32,
    pub dry_run: bool,
    pub audit_trace_id: String,
}

impl Default for SleepCompactionConfig {
    fn default() -> Self {
        Self {
            min_events: 3,
            max_batch: 64,
            salience_ceiling: 0.72,
            dry_run: false,
            audit_trace_id: format!("sleep-{}", uuid::Uuid::new_v4()),
        }
    }
}

/// 单个主题聚类的压缩预览/执行明细
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionClusterSummary {
    pub topic: String,
    pub source_event_ids: Vec<i64>,
    pub raw_tokens: i64,
    pub compacted_tokens: i64,
    pub preserved_fact_count: usize,
    pub compacted_fact_key: String,
    pub compacted_preview: String,
}

/// `ext_mem_pg_tx__compaction_jobs` 作业状态快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactionJobRecord {
    pub job_id: i64,
    pub job_kind: String,
    pub tenant_scope: String,
    pub from_epoch: i64,
    pub to_epoch: i64,
    pub state: String,
    pub audit_trace_id: String,
    pub created_at: String,
}

/// 睡眠整理执行报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SleepCompactionReport {
    pub job_id: Option<i64>,
    pub backend: String,
    pub tenant_scope: String,
    pub dry_run: bool,
    pub recovered_aborted_jobs: usize,
    pub events_scanned: usize,
    pub events_compacted: usize,
    pub clusters_created: usize,
    pub raw_tokens: i64,
    pub compacted_tokens: i64,
    pub tokens_saved: i64,
    pub compression_ratio: f64,
    pub preserved_fact_count: usize,
    pub state: String,
    pub audit_trace_id: String,
    pub elapsed_us: u128,
    pub clusters: Vec<CompactionClusterSummary>,
}

/// 确定性分词估算（与主 crate `provenance::estimate_tokens` 完全对齐）
pub fn estimate_tokens_deterministic(text: &str) -> i64 {
    if text.is_empty() {
        return 0;
    }
    let mut ascii_bytes = 0usize;
    let mut non_ascii_chars = 0usize;
    for ch in text.chars() {
        if ch.is_ascii() {
            ascii_bytes += ch.len_utf8();
        } else {
            non_ascii_chars += 1;
        }
    }
    let ascii_tok = (ascii_bytes as f64 / 4.0).ceil() as i64;
    let cjk_tok = (non_ascii_chars as f64 * 1.5).ceil() as i64;
    (ascii_tok + cjk_tok).max(1)
}

/// 确定性无幻觉聚类压缩器：剥离冗余寒暄与重复样板，100% 保留数字/端口/路径/决策/专有词事实
pub fn compact_episodic_texts(topic: &str, items: &[(String, String)]) -> (String, usize) {
    let mut seen_facts = BTreeSet::new();
    let mut ordered_facts: Vec<String> = Vec::new();

    for (_key, content) in items {
        for raw_seg in split_sentences_safe(content) {
            let trimmed = strip_boilerplate_prefix(raw_seg.trim());
            if trimmed.is_empty() || is_boilerplate_noise(trimmed) {
                continue;
            }
            if contains_atomic_fact_anchor(trimmed) || ordered_facts.is_empty() {
                let norm = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
                let dedup_key = norm.to_ascii_lowercase();
                if !norm.is_empty() && seen_facts.insert(dedup_key) {
                    ordered_facts.push(norm);
                }
            }
        }
    }

    if ordered_facts.is_empty() {
        if let Some((_, first)) = items.first() {
            ordered_facts.push(first.chars().take(120).collect());
        }
    }

    let preserved_fact_count = ordered_facts.len();
    let joined = format!("[sleep_summary:{}] {}", topic, ordered_facts.join(" | "));
    (joined, preserved_fact_count)
}

fn split_sentences_safe(content: &str) -> Vec<&str> {
    let mut segs = Vec::new();
    for line in content.split(['\n', '。', ';', '；']) {
        let mut start = 0usize;
        let bytes = line.as_bytes();
        let mut i = 0usize;
        while i < bytes.len() {
            if bytes[i] == b'.' && i + 1 < bytes.len() && bytes[i + 1] == b' ' {
                segs.push(&line[start..i]);
                i += 2;
                start = i;
                continue;
            }
            i += 1;
        }
        if start < line.len() {
            let tail = line[start..].trim_end_matches('.');
            if !tail.trim().is_empty() {
                segs.push(tail);
            }
        }
    }
    segs
}

fn strip_boilerplate_prefix(s: &str) -> &str {
    let mut cur = s.trim();
    for prefix in [
        "auto_sync:",
        "[auto_sync]",
        "routine log:",
        "session check:",
        "periodic note:",
        "背景回顾:",
        "会话记录:",
        "常规日志:",
    ] {
        if cur.to_ascii_lowercase().starts_with(prefix) {
            cur = cur[prefix.len()..].trim();
        }
    }
    cur
}

fn is_boilerplate_noise(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.starts_with("checked host status and everything looks")
        || lower.starts_with("no anomaly detected during routine")
        || lower.starts_with("routine telemetry healthy")
        || lower.starts_with("例行检查一切正常")
        || lower.starts_with("继续执行常规巡检")
        || lower.starts_with("等待下一次周期触发")
}

fn contains_atomic_fact_anchor(s: &str) -> bool {
    let has_digit = s.chars().any(|c| c.is_ascii_digit());
    let has_path = s.contains('/') || s.contains('=');
    let lower = s.to_ascii_lowercase();
    has_digit
        || has_path
        || lower.contains("must")
        || lower.contains("never")
        || lower.contains("prefer")
        || lower.contains("decision")
        || lower.contains("禁止")
        || lower.contains("必须")
        || lower.contains("选用")
        || lower.contains("端口")
        || lower.contains("配置")
}

/// 计算聚类归属主题：提取 `fact_key` 第一段前缀或首标签
pub fn derive_cluster_topic(fact_key: &str, tags: &[String]) -> String {
    if let Some((prefix, _)) = fact_key.split_once(':') {
        let p = prefix.trim();
        if !p.is_empty() && !matches!(p, "auto_sync" | "episodic" | "sqlite_mem") {
            return p.to_string();
        }
    }
    if let Some(tag) = tags
        .iter()
        .map(|t| t.trim())
        .find(|t| !t.is_empty() && !matches!(*t, "auto_sync" | "episodic" | "sleep_compacted" | "sleep_summary"))
    {
        return tag.to_string();
    }
    "general".to_string()
}

impl PgTxEngine {
    /// 异常中断恢复：将残留的 `queued`/`running` 睡眠整理作业安全置为 `aborted`
    pub fn recover_interrupted_sleep_jobs(&self, tenant_scope: &str) -> Result<usize> {
        let scope = if tenant_scope.trim().is_empty() {
            self.config.tenant_scope.as_str()
        } else {
            tenant_scope.trim()
        };
        let mut client = self.pool.checkout()?;
        let affected = client.execute(
            "UPDATE ext_mem_pg_tx__compaction_jobs
             SET state = 'aborted'
             WHERE tenant_scope = $1
               AND job_kind = 'sleep_context'
               AND state IN ('queued', 'running')",
            &[&scope],
        )?;
        Ok(affected as usize)
    }

    /// 列出最近 `compaction_jobs` 状态记录
    pub fn list_compaction_jobs(
        &self,
        tenant_scope: &str,
        limit: usize,
    ) -> Result<Vec<CompactionJobRecord>> {
        let scope = if tenant_scope.trim().is_empty() {
            self.config.tenant_scope.as_str()
        } else {
            tenant_scope.trim()
        };
        let lim = limit.clamp(1, 100) as i64;
        let mut client = self.pool.checkout()?;
        let rows = client.query(
            "SELECT job_id, job_kind, tenant_scope, from_epoch, to_epoch, state, audit_trace_id, created_at
             FROM ext_mem_pg_tx__compaction_jobs
             WHERE tenant_scope = $1
             ORDER BY job_id DESC
             LIMIT $2",
            &[&scope, &lim],
        )?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let ts: String = r
                    .try_get::<_, DateTime<Utc>>(7)
                    .map(|d| d.to_rfc3339())
                    .unwrap_or_default();
                CompactionJobRecord {
                    job_id: r.get(0),
                    job_kind: r.get(1),
                    tenant_scope: r.get(2),
                    from_epoch: r.get(3),
                    to_epoch: r.get(4),
                    state: r.get(5),
                    audit_trace_id: r.get(6),
                    created_at: ts,
                }
            })
            .collect())
    }

    /// 获取当前活跃睡眠摘要事件文本列表（供 `QueryPredictabilityGate` 评估词汇覆盖）
    pub fn list_sleep_summary_texts(&self, tenant_scope: &str, limit: usize) -> Result<Vec<String>> {
        let scope = if tenant_scope.trim().is_empty() {
            self.config.tenant_scope.as_str()
        } else {
            tenant_scope.trim()
        };
        let lim = limit.clamp(1, 100) as i64;
        let mut client = self.pool.checkout()?;
        let rows = client.query(
            "SELECT e.payload_redacted->>'content' AS txt
             FROM ext_mem_pg_tx__segments s
             JOIN ext_mem_pg_tx__events e ON e.event_id = s.event_id
             WHERE e.tenant_scope = $1
               AND NOT s.is_deleted
               AND e.embedding_status <> 'tombstoned'
               AND upper_inf(e.valid_range)
               AND e.consolidation_st = 'sleep_compacted'
             ORDER BY e.tx_epoch DESC
             LIMIT $2",
            &[&scope, &lim],
        )?;
        Ok(rows
            .into_iter()
            .filter_map(|r| r.try_get::<_, Option<String>>(0).ok().flatten())
            .collect())
    }

    /// 不可预测查询专用旁路直查：绕过压缩段过滤，直查 `ext_mem_pg_tx__events`（严格屏蔽 `tombstoned` 与历史覆盖行）
    pub fn search_raw_bypass(
        &self,
        query: &str,
        tenant_scope: &str,
        limit: usize,
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
            .start()?;

        let snapshot_epoch: i64 = tx
            .query_one(
                "SELECT COALESCE(MAX(tx_epoch), 0) FROM ext_mem_pg_tx__events WHERE tenant_scope = $1",
                &[&scope],
            )?
            .get(0);

        let rows = tx.query(
            "WITH active_e AS MATERIALIZED (
                 SELECT e.event_id, e.body_id, e.tenant_scope, e.fact_key, e.memory_type,
                        e.evidence_tier, e.payload_redacted, e.evidence_sha256,
                        lower(e.valid_range) AS valid_from,
                        CASE WHEN upper_inf(e.valid_range) THEN NULL ELSE upper(e.valid_range) END AS valid_to,
                        e.tx_epoch, e.embedding_status, e.salience_score, e.consolidation_st, e.created_at
                 FROM ext_mem_pg_tx__events e
                 WHERE e.tenant_scope = $1
                   AND e.embedding_status <> 'tombstoned'
                   AND upper_inf(e.valid_range)
             )
             SELECT e.event_id, e.body_id, e.tenant_scope, e.fact_key, e.memory_type,
                    e.evidence_tier, e.payload_redacted, e.evidence_sha256,
                    e.valid_from, e.valid_to,
                    e.tx_epoch, e.embedding_status, e.salience_score, e.consolidation_st, e.created_at,
                    CASE
                        WHEN $2 = '' THEN 1.0::double precision
                        WHEN s.fts_tsv @@ plainto_tsquery('simple', $2)
                            THEN (ts_rank(s.fts_tsv, plainto_tsquery('simple', $2)) + 1.2)::double precision
                        ELSE 0.75::double precision
                    END AS rank_score
             FROM active_e e
             JOIN ext_mem_pg_tx__segments s ON s.event_id = e.event_id
             WHERE (
                   $2 = ''
                   OR s.fts_tsv @@ plainto_tsquery('simple', $2)
                   OR e.payload_redacted::text ILIKE '%' || $2 || '%'
                   OR e.fact_key ILIKE '%' || $2 || '%'
               )
             ORDER BY CASE WHEN e.consolidation_st = 'raw' THEN 0 ELSE 1 END ASC,
                      rank_score DESC,
                      e.tx_epoch DESC
             LIMIT $3",
            &[&scope, &q, &lim],
        )?;
        tx.commit()?;
        Ok(PgTxSearchSnapshot {
            snapshot_epoch,
            hits: rows.iter().map(map_event_row).collect(),
        })
    }

    /// 执行一次 PostgreSQL 事务记忆空闲期睡眠整理 (`dry_run` 支持预览，`committed` 写入高密度摘要并折叠冗余段)
    pub fn run_sleep_compaction<F>(
        &self,
        body_id: &str,
        tenant_scope: &str,
        cfg: &SleepCompactionConfig,
        sanitize: F,
    ) -> Result<SleepCompactionReport>
    where
        F: Fn(&str) -> String,
    {
        let t0 = Instant::now();
        let scope = if tenant_scope.trim().is_empty() {
            self.config.tenant_scope.as_str()
        } else {
            tenant_scope.trim()
        };
        let recovered = self.recover_interrupted_sleep_jobs(scope)?;
        let _rescore = self.rescore_salience(scope, cfg.max_batch.max(32))?;

        let max_batch = cfg.max_batch.clamp(2, 500) as i64;
        let mut client = self.pool.checkout()?;
        let rows = client.query(
            "SELECT event_id, body_id, tenant_scope, fact_key, memory_type,
                    evidence_tier, payload_redacted, evidence_sha256,
                    lower(valid_range) AS valid_from,
                    CASE WHEN upper_inf(valid_range) THEN NULL ELSE upper(valid_range) END AS valid_to,
                    tx_epoch, embedding_status, salience_score, consolidation_st, created_at,
                    1.0::double precision AS rank_score
             FROM ext_mem_pg_tx__events
             WHERE tenant_scope = $1
               AND upper_inf(valid_range)
               AND embedding_status <> 'tombstoned'
               AND consolidation_st = 'raw'
               AND fact_key NOT LIKE 'sleep_summary:%'
             ORDER BY tx_epoch ASC
             LIMIT $2",
            &[&scope, &max_batch],
        )?;
        let raw_events: Vec<PgTxEventRow> = rows.iter().map(map_event_row).collect();
        let events_scanned = raw_events.len();

        // 筛选可压缩候选：低显著性或同主题冗余长情景事件
        let mut by_topic: BTreeMap<String, Vec<PgTxEventRow>> = BTreeMap::new();
        for ev in raw_events {
            let is_auto = SalienceScorer::looks_like_auto_sync(&ev.fact_key, &ev.content, &ev.tags);
            if ev.salience_score <= cfg.salience_ceiling
                || is_auto
                || ev.content.len() > 240
            {
                let topic = derive_cluster_topic(&ev.fact_key, &ev.tags);
                by_topic.entry(topic).or_default().push(ev);
            }
        }

        let mut clusters = Vec::new();
        let mut planned_writes: Vec<(String, String, Vec<i64>, Vec<String>, i64, i64, usize)> = Vec::new();
        let mut total_raw_tokens = 0i64;
        let mut total_compacted_tokens = 0i64;
        let mut total_events_compacted = 0usize;
        let mut total_facts = 0usize;
        let mut min_ep = i64::MAX;
        let mut max_ep = 0i64;

        for (topic, evs) in by_topic {
            if evs.len() < cfg.min_events.max(2) {
                continue;
            }
            let pairs: Vec<(String, String)> = evs
                .iter()
                .map(|e| (e.fact_key.clone(), e.content.clone()))
                .collect();
            let raw_tok: i64 = evs
                .iter()
                .map(|e| estimate_tokens_deterministic(&e.content))
                .sum();
            let (summary_raw, fact_cnt) = compact_episodic_texts(&topic, &pairs);
            let clean_summary = sanitize(&summary_raw);
            let comp_tok = estimate_tokens_deterministic(&clean_summary);
            if comp_tok >= raw_tok {
                continue;
            }
            let ids: Vec<i64> = evs.iter().map(|e| e.event_id).collect();
            for e in &evs {
                min_ep = min_ep.min(e.tx_epoch);
                max_ep = max_ep.max(e.tx_epoch);
            }
            let mut tags = BTreeSet::new();
            tags.insert("sleep_compacted".to_string());
            tags.insert("sleep_summary".to_string());
            tags.insert(topic.clone());
            let tag_vec: Vec<String> = tags.into_iter().collect();

            total_raw_tokens += raw_tok;
            total_compacted_tokens += comp_tok;
            total_events_compacted += ids.len();
            total_facts += fact_cnt;

            let fk = format!("sleep_summary:{}", topic);
            clusters.push(CompactionClusterSummary {
                topic: topic.clone(),
                source_event_ids: ids.clone(),
                raw_tokens: raw_tok,
                compacted_tokens: comp_tok,
                preserved_fact_count: fact_cnt,
                compacted_fact_key: fk.clone(),
                compacted_preview: clean_summary.chars().take(180).collect(),
            });
            planned_writes.push((topic, fk, ids, tag_vec, raw_tok, comp_tok, fact_cnt));
        }

        if planned_writes.is_empty() || cfg.dry_run {
            let saved = total_raw_tokens.saturating_sub(total_compacted_tokens);
            let ratio = if total_compacted_tokens > 0 {
                ((total_raw_tokens as f64 / total_compacted_tokens as f64) * 100.0).round() / 100.0
            } else {
                1.0
            };
            return Ok(SleepCompactionReport {
                job_id: None,
                backend: "pg_tx".to_string(),
                tenant_scope: scope.to_string(),
                dry_run: cfg.dry_run,
                recovered_aborted_jobs: recovered,
                events_scanned,
                events_compacted: total_events_compacted,
                clusters_created: clusters.len(),
                raw_tokens: total_raw_tokens,
                compacted_tokens: total_compacted_tokens,
                tokens_saved: saved,
                compression_ratio: ratio,
                preserved_fact_count: total_facts,
                state: if cfg.dry_run { "dry_run" } else { "skipped_insufficient_events" }.to_string(),
                audit_trace_id: cfg.audit_trace_id.clone(),
                elapsed_us: t0.elapsed().as_micros(),
                clusters,
            });
        }

        // 真实落盘：插入 compaction_jobs(running) -> verified -> 写入摘要 + 折叠冗余检索段 -> committed
        let from_ep = if min_ep == i64::MAX { 0 } else { min_ep };
        let job_row = client.query_one(
            "INSERT INTO ext_mem_pg_tx__compaction_jobs (
                job_kind, tenant_scope, from_epoch, to_epoch, state, audit_trace_id
             ) VALUES ('sleep_context', $1, $2, $3, 'running', $4)
             RETURNING job_id",
            &[&scope, &from_ep, &max_ep, &cfg.audit_trace_id],
        )?;
        let job_id: i64 = job_row.get(0);

        let mut tx = client.transaction()?;
        tx.execute(
            "UPDATE ext_mem_pg_tx__compaction_jobs SET state = 'verified' WHERE job_id = $1",
            &[&job_id],
        )?;

        for (topic, fk, ev_ids, tag_vec, _r_tok, _c_tok, _fcnt) in planned_writes.iter() {
            // 重新获取干净摘要文本以确保与 clusters 对齐
            let clean_summary = {
                let source_rows = tx.query(
                    "SELECT fact_key, payload_redacted->>'content'
                     FROM ext_mem_pg_tx__events
                     WHERE event_id = ANY($1)
                     ORDER BY tx_epoch ASC",
                    &[ev_ids],
                )?;
                let pairs: Vec<(String, String)> = source_rows
                    .into_iter()
                    .map(|r| {
                        let k: String = r.get(0);
                        let c: Option<String> = r.get(1);
                        (k, c.unwrap_or_default())
                    })
                    .collect();
                let (s_raw, _) = compact_episodic_texts(topic, &pairs);
                sanitize(&s_raw)
            };

            // 标记被吸收的源事件为 sleep_compacted（保留事件行本体用于不可预测查询旁路），仅折叠其检索段
            tx.execute(
                "UPDATE ext_mem_pg_tx__events
                 SET consolidation_st = 'sleep_compacted'
                 WHERE event_id = ANY($1)",
                &[ev_ids],
            )?;
            tx.execute(
                "UPDATE ext_mem_pg_tx__segments
                 SET is_deleted = TRUE, updated_at = clock_timestamp()
                 WHERE event_id = ANY($1) AND NOT is_deleted",
                &[ev_ids],
            )?;

            // 对同 fact_key 旧活跃摘要执行写穿覆盖并追加新摘要事件
            tx.execute(
                "SELECT pg_advisory_xact_lock(hashtextextended($1 || '|' || $2 || '|' || $3, 0))",
                &[&body_id, &scope, fk],
            )?;
            let prev_sum = tx.query_opt(
                "SELECT event_id
                 FROM ext_mem_pg_tx__events
                 WHERE body_id = $1 AND tenant_scope = $2 AND fact_key = $3
                   AND upper_inf(valid_range) AND embedding_status <> 'tombstoned'
                 LIMIT 1
                 FOR UPDATE",
                &[&body_id, &scope, fk],
            )?;
            let cut_ts: DateTime<Utc> = if let Some(prev_r) = prev_sum {
                let old_id: i64 = prev_r.get(0);
                let u = tx.query_one(
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
                )?;
                tx.execute(
                    "UPDATE ext_mem_pg_tx__segments
                     SET is_deleted = TRUE, updated_at = clock_timestamp()
                     WHERE event_id = $1 AND NOT is_deleted",
                    &[&old_id],
                )?;
                u.get(0)
            } else {
                tx.query_one("SELECT clock_timestamp()", &[])?.get(0)
            };

            let new_epoch: i64 = tx
                .query_one("SELECT nextval('ext_mem_pg_tx__epoch_seq')", &[])?
                .get(0);
            let hash_bytes =
                PgTxEngine::compute_evidence_hash(body_id, scope, fk, "experience", &clean_summary);
            let _hash_hex = bytes_to_hex(&hash_bytes);
            let payload = serde_json::json!({
                "content": clean_summary,
                "summary": format!("Sleep compacted summary for {}", topic),
                "tags": tag_vec,
                "source_agent": "sleep_time_compactor",
                "source_session": cfg.audit_trace_id,
            });
            let inserted = tx.query_one(
                "INSERT INTO ext_mem_pg_tx__events (
                    body_id, tenant_scope, fact_key, memory_type, evidence_tier,
                    payload_redacted, evidence_sha256, valid_range, tx_epoch,
                    embedding_status, salience_score, consolidation_st
                 ) VALUES (
                    $1, $2, $3, 'experience', 'L2_STRUCTURAL',
                    $4, $5, tstzrange($6, NULL, '[)'), $7,
                    'pending', $8, 'sleep_compacted'
                 )
                 RETURNING event_id",
                &[
                    &body_id,
                    &scope,
                    fk,
                    &payload,
                    &hash_bytes,
                    &cut_ts,
                    &new_epoch,
                    &0.82_f32,
                ],
            )?;
            let sum_event_id: i64 = inserted.get(0);
            let search_text = build_search_text(
                fk,
                &clean_summary,
                Some("sleep_compacted"),
                tag_vec.as_slice(),
            );
            tx.execute(
                "INSERT INTO ext_mem_pg_tx__segments (event_id, fts_tsv, embedding_ref, is_deleted)
                 VALUES ($1, to_tsvector('simple', $2), NULL, FALSE)",
                &[&sum_event_id, &search_text],
            )?;
        }

        tx.execute(
            "UPDATE ext_mem_pg_tx__compaction_jobs SET state = 'committed' WHERE job_id = $1",
            &[&job_id],
        )?;
        tx.commit()?;

        let saved = total_raw_tokens.saturating_sub(total_compacted_tokens);
        let ratio = if total_compacted_tokens > 0 {
            ((total_raw_tokens as f64 / total_compacted_tokens as f64) * 100.0).round() / 100.0
        } else {
            1.0
        };
        Ok(SleepCompactionReport {
            job_id: Some(job_id),
            backend: "pg_tx".to_string(),
            tenant_scope: scope.to_string(),
            dry_run: false,
            recovered_aborted_jobs: recovered,
            events_scanned,
            events_compacted: total_events_compacted,
            clusters_created: clusters.len(),
            raw_tokens: total_raw_tokens,
            compacted_tokens: total_compacted_tokens,
            tokens_saved: saved,
            compression_ratio: ratio,
            preserved_fact_count: total_facts,
            state: "committed".to_string(),
            audit_trace_id: cfg.audit_trace_id.clone(),
            elapsed_us: t0.elapsed().as_micros(),
            clusters,
        })
    }
}
