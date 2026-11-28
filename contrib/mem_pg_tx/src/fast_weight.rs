//! 方案 B（Phase 78 / M3）：PG 快权重快照血缘账本、定向遗忘管道、三道硬门禁原子切换与按 `tx_epoch` 秒级回滚
//!
//! 严守架构底线：
//! - 表 `ext_mem_pg_tx__fast_weight_snapshots` 与 `ext_mem_pg_tx__compaction_jobs` 仅存 URI / 血缘 / 纪元 / 门禁指标，
//!   绝不存储 GB 级权重矩阵 BLOB；
//! - 绝不把 PG 事务锁横跨在外部 Model Runtime JSON-RPC 耗时链路上：先落盘 `running` 状态并释放事务，
//!   拿到评测指标并校验三道硬门禁后，再在单 PG 短事务内原子切换 `active`；
//! - 任一门禁不通过（`norm_ratio` 超标 / `eval_glue_delta < -0.03` / `unlearn_leak_rate >= 0.001`）立即熔断并无损回退方案 A。

use std::time::Instant;

use agent_body_sdk::{
    evaluate_fast_weight_gates, validate_artifact_uri, AdapterKind, FastWeightCompactRequest,
    FastWeightGateMetrics, FastWeightGatePolicy, FastWeightGateVerdict, FastWeightRuntimeClient,
    FastWeightSnapshotStatus,
};
use anyhow::{Context, Result};
use postgres::IsolationLevel;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::engine::PgTxEngine;

/// 映射到 `ext_mem_pg_tx__fast_weight_snapshots` 的快权重快照血缘记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PgFastWeightSnapshot {
    pub snapshot_id: String,
    pub body_id: String,
    pub tenant_scope: String,
    pub base_model_id: String,
    pub parent_snapshot: Option<String>,
    pub anchored_epoch: i64,
    pub adapter_kind: String,
    pub artifact_uri: String,
    pub norm_ratio: f32,
    pub status: String,
    pub created_at: String,
}

/// 神经 Compaction（`fast_to_warm_lora`）与三道硬门禁原子切换结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightCompactionResult {
    pub job_id: i64,
    pub job_state: String,
    pub gate_verdict: FastWeightGateVerdict,
    pub metrics: FastWeightGateMetrics,
    pub from_epoch: i64,
    pub to_epoch: i64,
    pub merged_events: usize,
    pub snapshot: Option<PgFastWeightSnapshot>,
    pub fallback_to_plan_a: bool,
    pub elapsed_ms: u64,
}

/// 定向遗忘墓碑管道（`selective_unlearn`）执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightUnlearnResult {
    pub processed_job_ids: Vec<i64>,
    pub job_state: String,
    pub purged_event_ids: Vec<i64>,
    pub gate_verdict: Option<FastWeightGateVerdict>,
    pub metrics: Option<FastWeightGateMetrics>,
    pub snapshot: Option<PgFastWeightSnapshot>,
    pub fallback_to_plan_a: bool,
    pub elapsed_ms: u64,
}

/// 按 `tx_epoch` 秒级回滚执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightRollbackOutcome {
    pub target_epoch: i64,
    pub rolled_back_snapshot_ids: Vec<String>,
    pub restored_snapshot: Option<PgFastWeightSnapshot>,
    pub fallback_to_plan_a: bool,
    pub runtime_acknowledged: bool,
    pub elapsed_ms: u64,
}

/// 快权重血缘与遗忘队列健康快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightHealthSummary {
    pub body_id: String,
    pub tenant_scope: String,
    pub active_snapshot: Option<PgFastWeightSnapshot>,
    pub total_snapshots: i64,
    pub rolled_back_snapshots: i64,
    pub queued_unlearn_jobs: i64,
    pub aborted_jobs: i64,
    pub committed_jobs: i64,
}

pub(crate) fn map_snapshot_row(r: &postgres::Row) -> PgFastWeightSnapshot {
    PgFastWeightSnapshot {
        snapshot_id: r.get(0),
        body_id: r.get(1),
        tenant_scope: r.get(2),
        base_model_id: r.get(3),
        parent_snapshot: r.get(4),
        anchored_epoch: r.get(5),
        adapter_kind: r.get(6),
        artifact_uri: r.get(7),
        norm_ratio: r.get(8),
        status: r.get(9),
        created_at: r.get(10),
    }
}

impl PgTxEngine {
    pub(crate) fn resolve_scope(&self, tenant_scope: &str) -> String {
        if tenant_scope.trim().is_empty() {
            self.config.tenant_scope.clone()
        } else {
            tenant_scope.trim().to_string()
        }
    }

    /// 登记 `warming` 态快权重快照并校验 `artifact_uri` 与 `norm_ratio` 防雪崩阈值
    pub fn register_warming_snapshot(
        &self,
        body_id: &str,
        tenant_scope: &str,
        base_model_id: &str,
        parent_snapshot: Option<&str>,
        anchored_epoch: i64,
        adapter_kind: AdapterKind,
        artifact_uri: &str,
        norm_ratio: f32,
        policy: &FastWeightGatePolicy,
    ) -> Result<PgFastWeightSnapshot> {
        validate_artifact_uri(artifact_uri).map_err(anyhow::Error::msg)?;
        if !norm_ratio.is_finite() || norm_ratio <= 0.0 || norm_ratio > policy.max_norm_ratio {
            anyhow::bail!(
                "norm_ratio={} 触发范数雪崩熔断 (允许范围 (0.0, {}])，拒绝登记",
                norm_ratio,
                policy.max_norm_ratio
            );
        }
        let scope = self.resolve_scope(tenant_scope);
        let snap_id = Uuid::new_v4().to_string();
        let model = if base_model_id.trim().is_empty() {
            "default-base-model"
        } else {
            base_model_id.trim()
        };
        let parent = parent_snapshot.map(str::trim).filter(|s| !s.is_empty());

        let mut client = self.pool.checkout()?;
        let row = client
            .query_one(
                "INSERT INTO ext_mem_pg_tx__fast_weight_snapshots (
                    snapshot_id, body_id, tenant_scope, base_model_id,
                    parent_snapshot, anchored_epoch, adapter_kind, artifact_uri,
                    norm_ratio, status
                 ) VALUES (
                    $1::text::uuid, $2, $3, $4,
                    NULLIF($5, '')::uuid, $6, $7, $8,
                    $9, 'warming'
                 )
                 RETURNING snapshot_id::text, body_id, tenant_scope, base_model_id,
                           parent_snapshot::text, anchored_epoch, adapter_kind, artifact_uri,
                           norm_ratio, status,
                           to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
                &[
                    &snap_id,
                    &body_id,
                    &scope,
                    &model,
                    &parent.unwrap_or(""),
                    &anchored_epoch,
                    &adapter_kind.as_str(),
                    &artifact_uri.trim(),
                    &norm_ratio,
                ],
            )
            .context("写入 warming 快权重快照失败")?;
        Ok(map_snapshot_row(&row))
    }

    /// 按 SPI 元数据直接登记快照（含 `norm_ratio` 熔断及旧 `active` 自动折叠）
    pub fn register_snapshot_meta(
        &self,
        snapshot_id: Option<&str>,
        body_id: &str,
        tenant_scope: &str,
        base_model_id: &str,
        parent_snapshot: Option<&str>,
        anchored_epoch: i64,
        adapter_kind: &str,
        artifact_uri: &str,
        norm_ratio: f32,
        requested_status: &str,
        policy: &FastWeightGatePolicy,
    ) -> Result<PgFastWeightSnapshot> {
        validate_artifact_uri(artifact_uri).map_err(anyhow::Error::msg)?;
        let kind = AdapterKind::parse_str(adapter_kind)?;
        let st = FastWeightSnapshotStatus::parse_str(requested_status)?;
        if !norm_ratio.is_finite() || norm_ratio <= 0.0 || norm_ratio > policy.max_norm_ratio {
            anyhow::bail!(
                "norm_ratio={} 超出范数防雪崩上限 {}",
                norm_ratio,
                policy.max_norm_ratio
            );
        }

        let scope = self.resolve_scope(tenant_scope);
        let snap_id = snapshot_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let parent = parent_snapshot.map(str::trim).filter(|s| !s.is_empty());

        let mut client = self.pool.checkout()?;
        let mut tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        tx.execute(
            "SELECT pg_advisory_xact_lock(hashtextextended($1 || '|' || $2 || '|fw', 0))",
            &[&body_id, &scope],
        )?;
        if st == FastWeightSnapshotStatus::Active {
            tx.execute(
                "UPDATE ext_mem_pg_tx__fast_weight_snapshots
                 SET status = 'compacted'
                 WHERE body_id = $1 AND tenant_scope = $2 AND status = 'active'",
                &[&body_id, &scope],
            )?;
        }
        let row = tx.query_one(
            "INSERT INTO ext_mem_pg_tx__fast_weight_snapshots (
                snapshot_id, body_id, tenant_scope, base_model_id,
                parent_snapshot, anchored_epoch, adapter_kind, artifact_uri,
                norm_ratio, status
             ) VALUES (
                $1::text::uuid, $2, $3, $4,
                NULLIF($5, '')::uuid, $6, $7, $8,
                $9, $10
             )
             ON CONFLICT (snapshot_id) DO UPDATE SET
                anchored_epoch = EXCLUDED.anchored_epoch,
                artifact_uri = EXCLUDED.artifact_uri,
                norm_ratio = EXCLUDED.norm_ratio,
                status = EXCLUDED.status
             RETURNING snapshot_id::text, body_id, tenant_scope, base_model_id,
                       parent_snapshot::text, anchored_epoch, adapter_kind, artifact_uri,
                       norm_ratio, status,
                       to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
            &[
                &snap_id,
                &body_id,
                &scope,
                &base_model_id.trim(),
                &parent.unwrap_or(""),
                &anchored_epoch,
                &kind.as_str(),
                &artifact_uri.trim(),
                &norm_ratio,
                &st.as_str(),
            ],
        )?;
        tx.commit()?;
        Ok(map_snapshot_row(&row))
    }

    /// O(1) 查询当前活跃快权重快照（利用 `idx_ext_mem_pg_tx_fw_active` 部分索引）
    pub fn active_fast_weight_snapshot(
        &self,
        body_id: &str,
        tenant_scope: &str,
    ) -> Result<Option<PgFastWeightSnapshot>> {
        let scope = self.resolve_scope(tenant_scope);
        let mut client = self.pool.checkout()?;
        let row = client.query_opt(
            "SELECT snapshot_id::text, body_id, tenant_scope, base_model_id,
                    parent_snapshot::text, anchored_epoch, adapter_kind, artifact_uri,
                    norm_ratio, status,
                    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')
             FROM ext_mem_pg_tx__fast_weight_snapshots
             WHERE body_id = $1 AND tenant_scope = $2 AND status = 'active'
             ORDER BY anchored_epoch DESC, created_at DESC
             LIMIT 1",
            &[&body_id, &scope],
        )?;
        Ok(row.map(|r| map_snapshot_row(&r)))
    }

    /// 列出最近快权重快照台账
    pub fn list_fast_weight_snapshots(
        &self,
        body_id: &str,
        tenant_scope: &str,
        limit: usize,
    ) -> Result<Vec<PgFastWeightSnapshot>> {
        let scope = self.resolve_scope(tenant_scope);
        let lim = limit.clamp(1, 200) as i64;
        let mut client = self.pool.checkout()?;
        let rows = client.query(
            "SELECT snapshot_id::text, body_id, tenant_scope, base_model_id,
                    parent_snapshot::text, anchored_epoch, adapter_kind, artifact_uri,
                    norm_ratio, status,
                    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')
             FROM ext_mem_pg_tx__fast_weight_snapshots
             WHERE body_id = $1 AND tenant_scope = $2
             ORDER BY created_at DESC, anchored_epoch DESC
             LIMIT $3",
            &[&body_id, &scope, &lim],
        )?;
        Ok(rows.iter().map(map_snapshot_row).collect())
    }

    /// 使用 PostgreSQL `WITH RECURSIVE` CTE 追溯指定（或当前活跃）快照的父子血缘链
    pub fn get_snapshot_lineage(
        &self,
        body_id: &str,
        tenant_scope: &str,
        snapshot_id_opt: Option<&str>,
        max_depth: usize,
    ) -> Result<Vec<PgFastWeightSnapshot>> {
        let scope = self.resolve_scope(tenant_scope);
        let seed_id = match snapshot_id_opt.map(str::trim).filter(|s| !s.is_empty()) {
            Some(id) => id.to_string(),
            None => match self.active_fast_weight_snapshot(body_id, &scope)? {
                Some(active) => active.snapshot_id,
                None => {
                    let recent = self.list_fast_weight_snapshots(body_id, &scope, 1)?;
                    match recent.into_iter().next() {
                        Some(s) => s.snapshot_id,
                        None => return Ok(Vec::new()),
                    }
                }
            },
        };

        let depth_lim = max_depth.clamp(1, 128) as i32;
        let mut client = self.pool.checkout()?;
        let rows = client.query(
            "WITH RECURSIVE chain AS (
                SELECT snapshot_id, body_id, tenant_scope, base_model_id,
                       parent_snapshot, anchored_epoch, adapter_kind, artifact_uri,
                       norm_ratio, status, created_at, 0 AS depth
                FROM ext_mem_pg_tx__fast_weight_snapshots
                WHERE snapshot_id = $1::text::uuid AND body_id = $2 AND tenant_scope = $3
                UNION ALL
                SELECT p.snapshot_id, p.body_id, p.tenant_scope, p.base_model_id,
                       p.parent_snapshot, p.anchored_epoch, p.adapter_kind, p.artifact_uri,
                       p.norm_ratio, p.status, p.created_at, c.depth + 1
                FROM ext_mem_pg_tx__fast_weight_snapshots p
                JOIN chain c ON c.parent_snapshot = p.snapshot_id
                WHERE c.depth < $4
             )
             SELECT snapshot_id::text, body_id, tenant_scope, base_model_id,
                    parent_snapshot::text, anchored_epoch, adapter_kind, artifact_uri,
                    norm_ratio, status,
                    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')
             FROM chain
             ORDER BY depth ASC",
            &[&seed_id, &body_id, &scope, &depth_lim],
        )?;
        Ok(rows.iter().map(map_snapshot_row).collect())
    }

    /// 执行神经 Compaction (`fast_to_warm_lora`) 并以三道硬门禁在单事务内原子切换 `status='active'`
    pub fn run_neural_compaction_and_switch(
        &self,
        body_id: &str,
        tenant_scope: &str,
        base_model_id: &str,
        adapter_kind: AdapterKind,
        runtime: &dyn FastWeightRuntimeClient,
        policy: &FastWeightGatePolicy,
        audit_trace_id: &str,
    ) -> Result<FastWeightCompactionResult> {
        let t0 = Instant::now();
        let scope = self.resolve_scope(tenant_scope);
        let model = if base_model_id.trim().is_empty() {
            "default-base-model".to_string()
        } else {
            base_model_id.trim().to_string()
        };

        // 1. 短读收集待合并活跃事件与待擦除墓碑（不持写锁）
        let (event_ids, payloads, from_epoch, to_epoch, parent_snap, pending_tombstones, job_id) = {
            let mut client = self.pool.checkout()?;
            let mut tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::ReadCommitted)
                .start()?;

            let ev_rows = tx.query(
                "SELECT event_id, tx_epoch, payload_redacted::text
                 FROM ext_mem_pg_tx__events
                 WHERE body_id = $1 AND tenant_scope = $2
                   AND upper_inf(valid_range)
                   AND embedding_status <> 'tombstoned'
                   AND consolidation_st IN ('raw', 'sleep_compacted')
                 ORDER BY tx_epoch ASC
                 LIMIT 256",
                &[&body_id, &scope],
            )?;

            let mut ids = Vec::with_capacity(ev_rows.len());
            let mut texts = Vec::with_capacity(ev_rows.len());
            let mut min_ep = i64::MAX;
            let mut max_ep = 0i64;
            for r in &ev_rows {
                let eid: i64 = r.get(0);
                let ep: i64 = r.get(1);
                let txt: String = r.get(2);
                ids.push(eid);
                texts.push(txt);
                min_ep = min_ep.min(ep);
                max_ep = max_ep.max(ep);
            }
            if ids.is_empty() {
                let cur_ep = self.current_epoch().unwrap_or(0);
                min_ep = cur_ep;
                max_ep = cur_ep;
            }

            let parent: Option<String> = tx
                .query_opt(
                    "SELECT snapshot_id::text
                     FROM ext_mem_pg_tx__fast_weight_snapshots
                     WHERE body_id = $1 AND tenant_scope = $2 AND status = 'active'
                     ORDER BY anchored_epoch DESC LIMIT 1",
                    &[&body_id, &scope],
                )?
                .map(|r| r.get(0));

            let tomb_rows = tx.query(
                "SELECT DISTINCT unnest(tombstone_ids)
                 FROM ext_mem_pg_tx__compaction_jobs
                 WHERE tenant_scope = $1 AND job_kind = 'selective_unlearn' AND state = 'queued'",
                &[&scope],
            )?;
            let tomb_ids: Vec<i64> = tomb_rows.iter().map(|r| r.get(0)).collect();

            let jid: i64 = tx
                .query_one(
                    "INSERT INTO ext_mem_pg_tx__compaction_jobs (
                        job_kind, tenant_scope, from_epoch, to_epoch, tombstone_ids, state, audit_trace_id
                     ) VALUES (
                        'fast_to_warm_lora', $1, $2, $3, $4, 'running', $5
                     ) RETURNING job_id",
                    &[&scope, &min_ep, &max_ep, &tomb_ids, &audit_trace_id],
                )?
                .get(0);
            tx.commit()?;
            (ids, texts, min_ep, max_ep, parent, tomb_ids, jid)
        };

        // 2. 脱离 PG 事务锁调用旁路 Model Runtime
        let req = FastWeightCompactRequest {
            body_id: body_id.to_string(),
            tenant_scope: scope.clone(),
            base_model_id: model.clone(),
            adapter_kind,
            parent_snapshot_id: parent_snap.clone(),
            from_epoch,
            to_epoch,
            episode_payloads: payloads,
            tombstone_ids: pending_tombstones.clone(),
            audit_trace_id: audit_trace_id.to_string(),
        };
        let resp = match runtime.submit_compaction(&req) {
            Ok(r) => r,
            Err(e) => {
                if let Ok(mut client) = self.pool.checkout() {
                    if let Err(u_err) = client.execute(
                        "UPDATE ext_mem_pg_tx__compaction_jobs SET state = 'aborted' WHERE job_id = $1",
                        &[&job_id],
                    ) {
                        eprintln!("[FastWeight][WARN] 标记 compaction job {} aborted 失败: {}", job_id, u_err);
                    }
                }
                return Err(e);
            }
        };

        let metrics = FastWeightGateMetrics {
            norm_ratio: resp.norm_ratio,
            eval_glue_delta: resp.eval_glue_delta,
            unlearn_leak_rate: resp.unlearn_leak_rate,
        };
        let mut verdict = evaluate_fast_weight_gates(&metrics, policy);
        if let Err(uri_err) = validate_artifact_uri(&resp.artifact_uri) {
            verdict.passed = false;
            verdict.reasons.push(uri_err);
        }
        let snap_uuid = if Uuid::parse_str(resp.snapshot_id.trim()).is_ok() {
            resp.snapshot_id.trim().to_string()
        } else {
            Uuid::new_v4().to_string()
        };

        // 3. 单 PG 事务内执行三道硬门禁判定与原子切换或无损回退方案 A
        let mut client = self.pool.checkout()?;
        let mut tx = client
            .build_transaction()
            .isolation_level(IsolationLevel::ReadCommitted)
            .start()?;
        tx.execute(
            "SELECT pg_advisory_xact_lock(hashtextextended($1 || '|' || $2 || '|fw', 0))",
            &[&body_id, &scope],
        )?;

        if verdict.passed {
            tx.execute(
                "UPDATE ext_mem_pg_tx__fast_weight_snapshots
                 SET status = 'compacted'
                 WHERE body_id = $1 AND tenant_scope = $2 AND status = 'active'",
                &[&body_id, &scope],
            )?;

            let snap_row = tx.query_one(
                "INSERT INTO ext_mem_pg_tx__fast_weight_snapshots (
                    snapshot_id, body_id, tenant_scope, base_model_id,
                    parent_snapshot, anchored_epoch, adapter_kind, artifact_uri,
                    norm_ratio, status
                 ) VALUES (
                    $1::text::uuid, $2, $3, $4,
                    NULLIF($5, '')::uuid, $6, $7, $8,
                    $9, 'active'
                 )
                 RETURNING snapshot_id::text, body_id, tenant_scope, base_model_id,
                           parent_snapshot::text, anchored_epoch, adapter_kind, artifact_uri,
                           norm_ratio, status,
                           to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
                &[
                    &snap_uuid,
                    &body_id,
                    &scope,
                    &model,
                    &parent_snap.as_deref().unwrap_or(""),
                    &to_epoch,
                    &adapter_kind.as_str(),
                    &resp.artifact_uri.trim(),
                    &metrics.norm_ratio,
                ],
            )?;

            if !event_ids.is_empty() {
                tx.execute(
                    "UPDATE ext_mem_pg_tx__events
                     SET consolidation_st = 'weight_merged'
                     WHERE event_id = ANY($1) AND embedding_status <> 'tombstoned'",
                    &[&event_ids],
                )?;
            }

            tx.execute(
                "UPDATE ext_mem_pg_tx__compaction_jobs
                 SET state = 'committed', eval_glue_delta = $2, unlearn_leak_rate = $3
                 WHERE job_id = $1",
                &[&job_id, &metrics.eval_glue_delta, &metrics.unlearn_leak_rate],
            )?;
            tx.commit()?;

            Ok(FastWeightCompactionResult {
                job_id,
                job_state: "committed".to_string(),
                gate_verdict: verdict,
                metrics,
                from_epoch,
                to_epoch,
                merged_events: event_ids.len(),
                snapshot: Some(map_snapshot_row(&snap_row)),
                fallback_to_plan_a: false,
                elapsed_ms: t0.elapsed().as_millis().max(1) as u64,
            })
        } else {
            let safe_uri = if validate_artifact_uri(&resp.artifact_uri).is_ok() {
                resp.artifact_uri.trim().to_string()
            } else {
                format!("mock://rejected/{}", snap_uuid)
            };
            let snap_row = tx.query_one(
                "INSERT INTO ext_mem_pg_tx__fast_weight_snapshots (
                    snapshot_id, body_id, tenant_scope, base_model_id,
                    parent_snapshot, anchored_epoch, adapter_kind, artifact_uri,
                    norm_ratio, status
                 ) VALUES (
                    $1::text::uuid, $2, $3, $4,
                    NULLIF($5, '')::uuid, $6, $7, $8,
                    $9, 'rolled_back'
                 )
                 RETURNING snapshot_id::text, body_id, tenant_scope, base_model_id,
                           parent_snapshot::text, anchored_epoch, adapter_kind, artifact_uri,
                           norm_ratio, status,
                           to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
                &[
                    &snap_uuid,
                    &body_id,
                    &scope,
                    &model,
                    &parent_snap.as_deref().unwrap_or(""),
                    &to_epoch,
                    &adapter_kind.as_str(),
                    &safe_uri,
                    &metrics.norm_ratio,
                ],
            )?;
            tx.execute(
                "UPDATE ext_mem_pg_tx__compaction_jobs
                 SET state = 'aborted', eval_glue_delta = $2, unlearn_leak_rate = $3
                 WHERE job_id = $1",
                &[&job_id, &metrics.eval_glue_delta, &metrics.unlearn_leak_rate],
            )?;
            tx.commit()?;

            Ok(FastWeightCompactionResult {
                job_id,
                job_state: "aborted".to_string(),
                gate_verdict: verdict,
                metrics,
                from_epoch,
                to_epoch,
                merged_events: 0,
                snapshot: Some(map_snapshot_row(&snap_row)),
                fallback_to_plan_a: true,
                elapsed_ms: t0.elapsed().as_millis().max(1) as u64,
            })
        }
    }

    /// 汇总快权重血缘、遗忘队列与门禁统计
    pub fn fast_weight_health(
        &self,
        body_id: &str,
        tenant_scope: &str,
    ) -> Result<FastWeightHealthSummary> {
        let scope = self.resolve_scope(tenant_scope);
        let active_snapshot = self.active_fast_weight_snapshot(body_id, &scope)?;
        let mut client = self.pool.checkout()?;
        let row = client.query_one(
            "SELECT
                (SELECT COUNT(*) FROM ext_mem_pg_tx__fast_weight_snapshots WHERE body_id = $1 AND tenant_scope = $2),
                (SELECT COUNT(*) FROM ext_mem_pg_tx__fast_weight_snapshots WHERE body_id = $1 AND tenant_scope = $2 AND status = 'rolled_back'),
                (SELECT COUNT(*) FROM ext_mem_pg_tx__compaction_jobs WHERE tenant_scope = $2 AND job_kind = 'selective_unlearn' AND state = 'queued'),
                (SELECT COUNT(*) FROM ext_mem_pg_tx__compaction_jobs WHERE tenant_scope = $2 AND job_kind IN ('fast_to_warm_lora','selective_unlearn') AND state = 'aborted'),
                (SELECT COUNT(*) FROM ext_mem_pg_tx__compaction_jobs WHERE tenant_scope = $2 AND job_kind IN ('fast_to_warm_lora','selective_unlearn') AND state = 'committed')",
            &[&body_id, &scope],
        )?;
        Ok(FastWeightHealthSummary {
            body_id: body_id.to_string(),
            tenant_scope: scope,
            active_snapshot,
            total_snapshots: row.get(0),
            rolled_back_snapshots: row.get(1),
            queued_unlearn_jobs: row.get(2),
            aborted_jobs: row.get(3),
            committed_jobs: row.get(4),
        })
    }
}
