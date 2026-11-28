//! 方案 B（Phase 78 / M3）：定向遗忘墓碑管道 (`selective_unlearn`) 与按 `tx_epoch` 秒级原子回滚

use std::time::Instant;

use agent_body_sdk::{
    evaluate_fast_weight_gates, validate_artifact_uri, AdapterKind, FastWeightGateMetrics,
    FastWeightGatePolicy, FastWeightRollbackRequest, FastWeightRuntimeClient,
    FastWeightUnlearnRequest,
};
use anyhow::Result;
use postgres::IsolationLevel;
use uuid::Uuid;

use crate::engine::PgTxEngine;
use crate::fast_weight::{
    map_snapshot_row, FastWeightRollbackOutcome, FastWeightUnlearnResult,
};

impl PgTxEngine {
    /// 消费并执行 `selective_unlearn` 定向遗忘作业，下发旁路擦除并在三道硬门禁通过后原子切换净化快照
    pub fn execute_selective_unlearn(
        &self,
        body_id: &str,
        tenant_scope: &str,
        base_model_id: &str,
        adapter_kind: AdapterKind,
        job_id_opt: Option<i64>,
        runtime: &dyn FastWeightRuntimeClient,
        policy: &FastWeightGatePolicy,
        audit_trace_id: &str,
    ) -> Result<FastWeightUnlearnResult> {
        let t0 = Instant::now();
        let scope = self.resolve_scope(tenant_scope);
        let model = if base_model_id.trim().is_empty() {
            "default-base-model".to_string()
        } else {
            base_model_id.trim().to_string()
        };

        let (job_ids, tomb_ids, fact_keys, active_snap, anchored_ep) = {
            let mut client = self.pool.checkout()?;
            let mut tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::ReadCommitted)
                .start()?;

            let rows = if let Some(jid) = job_id_opt {
                tx.query(
                    "UPDATE ext_mem_pg_tx__compaction_jobs
                     SET state = 'running'
                     WHERE job_id = $1 AND tenant_scope = $2 AND job_kind = 'selective_unlearn'
                       AND state IN ('queued', 'running')
                     RETURNING job_id, tombstone_ids, to_epoch",
                    &[&jid, &scope],
                )?
            } else {
                tx.query(
                    "UPDATE ext_mem_pg_tx__compaction_jobs
                     SET state = 'running'
                     WHERE tenant_scope = $1 AND job_kind = 'selective_unlearn' AND state = 'queued'
                     RETURNING job_id, tombstone_ids, to_epoch",
                    &[&scope],
                )?
            };

            if rows.is_empty() {
                tx.commit()?;
                return Ok(FastWeightUnlearnResult {
                    processed_job_ids: Vec::new(),
                    job_state: "noop".to_string(),
                    purged_event_ids: Vec::new(),
                    gate_verdict: None,
                    metrics: None,
                    snapshot: None,
                    fallback_to_plan_a: false,
                    elapsed_ms: t0.elapsed().as_millis().max(1) as u64,
                });
            }

            let mut jids = Vec::with_capacity(rows.len());
            let mut all_tombs = Vec::new();
            let mut max_ep = 0i64;
            for r in &rows {
                let id: i64 = r.get(0);
                let tids: Vec<i64> = r.get(1);
                let ep: i64 = r.get(2);
                jids.push(id);
                all_tombs.extend(tids);
                max_ep = max_ep.max(ep);
            }
            all_tombs.sort_unstable();
            all_tombs.dedup();

            let fk_rows = if all_tombs.is_empty() {
                Vec::new()
            } else {
                tx.query(
                    "SELECT DISTINCT fact_key FROM ext_mem_pg_tx__events WHERE event_id = ANY($1)",
                    &[&all_tombs],
                )?
            };
            let fks: Vec<String> = fk_rows.iter().map(|r| r.get(0)).collect();

            let active_row: Option<(String, i64)> = tx
                .query_opt(
                    "SELECT snapshot_id::text, anchored_epoch
                     FROM ext_mem_pg_tx__fast_weight_snapshots
                     WHERE body_id = $1 AND tenant_scope = $2 AND status = 'active'
                     ORDER BY anchored_epoch DESC LIMIT 1",
                    &[&body_id, &scope],
                )?
                .map(|r| (r.get(0), r.get(1)));
            let latest_ev_ep: i64 = tx
                .query_one(
                    "SELECT COALESCE(MAX(tx_epoch), 0)
                     FROM ext_mem_pg_tx__events
                     WHERE body_id = $1 AND tenant_scope = $2",
                    &[&body_id, &scope],
                )?
                .get(0);
            let active_ep = active_row.as_ref().map(|(_, ep)| *ep).unwrap_or(0);
            let active_snap = active_row.map(|(id, _)| id);
            let resolved_ep = max_ep.max(latest_ev_ep).max(active_ep);

            tx.commit()?;
            (jids, all_tombs, fks, active_snap, resolved_ep)
        };

        let req = FastWeightUnlearnRequest {
            body_id: body_id.to_string(),
            tenant_scope: scope.clone(),
            base_model_id: model.clone(),
            adapter_kind,
            active_snapshot_id: active_snap.clone(),
            anchored_epoch: anchored_ep,
            tombstone_ids: tomb_ids.clone(),
            redacted_fact_keys: fact_keys,
            audit_trace_id: audit_trace_id.to_string(),
        };

        let resp = match runtime.submit_selective_unlearn(&req) {
            Ok(r) => r,
            Err(e) => {
                if let Ok(mut client) = self.pool.checkout() {
                    if let Err(u_err) = client.execute(
                        "UPDATE ext_mem_pg_tx__compaction_jobs SET state = 'aborted' WHERE job_id = ANY($1)",
                        &[&job_ids],
                    ) {
                        eprintln!("[FastWeight][WARN] 标记 selective_unlearn aborted 失败: {}", u_err);
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
        let snap_uuid = if Uuid::parse_str(resp.purged_snapshot_id.trim()).is_ok() {
            resp.purged_snapshot_id.trim().to_string()
        } else {
            Uuid::new_v4().to_string()
        };

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
                    &active_snap.as_deref().unwrap_or(""),
                    &anchored_ep,
                    &adapter_kind.as_str(),
                    &resp.artifact_uri.trim(),
                    &metrics.norm_ratio,
                ],
            )?;

            if !tomb_ids.is_empty() {
                tx.execute(
                    "UPDATE ext_mem_pg_tx__events
                     SET consolidation_st = 'purged', embedding_status = 'tombstoned'
                     WHERE event_id = ANY($1)",
                    &[&tomb_ids],
                )?;
                tx.execute(
                    "UPDATE ext_mem_pg_tx__segments
                     SET is_deleted = TRUE, updated_at = clock_timestamp()
                     WHERE event_id = ANY($1)",
                    &[&tomb_ids],
                )?;
            }
            tx.execute(
                "UPDATE ext_mem_pg_tx__compaction_jobs
                 SET state = 'committed', eval_glue_delta = $2, unlearn_leak_rate = $3
                 WHERE job_id = ANY($1)",
                &[&job_ids, &metrics.eval_glue_delta, &metrics.unlearn_leak_rate],
            )?;
            tx.commit()?;

            Ok(FastWeightUnlearnResult {
                processed_job_ids: job_ids,
                job_state: "committed".to_string(),
                purged_event_ids: tomb_ids,
                gate_verdict: Some(verdict),
                metrics: Some(metrics),
                snapshot: Some(map_snapshot_row(&snap_row)),
                fallback_to_plan_a: false,
                elapsed_ms: t0.elapsed().as_millis().max(1) as u64,
            })
        } else {
            tx.execute(
                "UPDATE ext_mem_pg_tx__compaction_jobs
                 SET state = 'aborted', eval_glue_delta = $2, unlearn_leak_rate = $3
                 WHERE job_id = ANY($1)",
                &[&job_ids, &metrics.eval_glue_delta, &metrics.unlearn_leak_rate],
            )?;
            tx.commit()?;

            Ok(FastWeightUnlearnResult {
                processed_job_ids: job_ids,
                job_state: "aborted".to_string(),
                purged_event_ids: tomb_ids,
                gate_verdict: Some(verdict),
                metrics: Some(metrics),
                snapshot: None,
                fallback_to_plan_a: true,
                elapsed_ms: t0.elapsed().as_millis().max(1) as u64,
            })
        }
    }

    /// 按 `tx_epoch` 秒级原子回滚快权重快照（`> target_epoch` 全置 `rolled_back`，`<= target_epoch` 最新有效快照恢复 `active`）
    pub fn rollback_fast_weight_to_epoch(
        &self,
        body_id: &str,
        tenant_scope: &str,
        target_epoch: i64,
        runtime_opt: Option<&dyn FastWeightRuntimeClient>,
        audit_trace_id: &str,
    ) -> Result<FastWeightRollbackOutcome> {
        let t0 = Instant::now();
        let scope = self.resolve_scope(tenant_scope);

        let (rolled_back_ids, restored) = {
            let mut client = self.pool.checkout()?;
            let mut tx = client
                .build_transaction()
                .isolation_level(IsolationLevel::ReadCommitted)
                .start()?;
            tx.execute(
                "SELECT pg_advisory_xact_lock(hashtextextended($1 || '|' || $2 || '|fw', 0))",
                &[&body_id, &scope],
            )?;

            let rb_rows = tx.query(
                "UPDATE ext_mem_pg_tx__fast_weight_snapshots
                 SET status = 'rolled_back'
                 WHERE body_id = $1 AND tenant_scope = $2
                   AND anchored_epoch > $3
                   AND status IN ('active', 'warming', 'compacted')
                 RETURNING snapshot_id::text",
                &[&body_id, &scope, &target_epoch],
            )?;
            let rb_ids: Vec<String> = rb_rows.iter().map(|r| r.get(0)).collect();

            tx.execute(
                "UPDATE ext_mem_pg_tx__fast_weight_snapshots
                 SET status = 'compacted'
                 WHERE body_id = $1 AND tenant_scope = $2 AND status = 'active'",
                &[&body_id, &scope],
            )?;

            let target_id: Option<String> = tx
                .query_opt(
                    "SELECT snapshot_id::text
                     FROM ext_mem_pg_tx__fast_weight_snapshots
                     WHERE body_id = $1 AND tenant_scope = $2
                       AND anchored_epoch <= $3
                       AND status IN ('compacted', 'active')
                     ORDER BY anchored_epoch DESC, created_at DESC
                     LIMIT 1",
                    &[&body_id, &scope, &target_epoch],
                )?
                .map(|r| r.get(0));

            let restored_snap = if let Some(tid) = target_id {
                let row = tx.query_one(
                    "UPDATE ext_mem_pg_tx__fast_weight_snapshots
                     SET status = 'active'
                     WHERE snapshot_id = $1::text::uuid
                     RETURNING snapshot_id::text, body_id, tenant_scope, base_model_id,
                               parent_snapshot::text, anchored_epoch, adapter_kind, artifact_uri,
                               norm_ratio, status,
                               to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')",
                    &[&tid],
                )?;
                Some(map_snapshot_row(&row))
            } else {
                None
            };
            tx.commit()?;
            (rb_ids, restored_snap)
        };

        let runtime_acked = if let Some(rt) = runtime_opt {
            let req = FastWeightRollbackRequest {
                body_id: body_id.to_string(),
                tenant_scope: scope,
                target_epoch,
                restored_snapshot_id: restored.as_ref().map(|s| s.snapshot_id.clone()),
                rolled_back_snapshot_ids: rolled_back_ids.clone(),
                audit_trace_id: audit_trace_id.to_string(),
            };
            match rt.notify_rollback(&req) {
                Ok(r) => r.acknowledged,
                Err(e) => {
                    eprintln!("[FastWeight][WARN] 通知旁路 Runtime 回滚失败: {}", e);
                    false
                }
            }
        } else {
            false
        };

        let fallback_to_plan_a = restored.is_none();
        Ok(FastWeightRollbackOutcome {
            target_epoch,
            rolled_back_snapshot_ids: rolled_back_ids,
            restored_snapshot: restored,
            fallback_to_plan_a,
            runtime_acknowledged: runtime_acked,
            elapsed_ms: t0.elapsed().as_millis().max(1) as u64,
        })
    }
}
