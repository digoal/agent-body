//! Phase 78（方案 B / M3）：旁路快权重血缘控制面与神经 Compaction 编排器 (`FastWeightOrchestrator`)
//!
//! 职责：
//! - 将 `agent_body_sdk::FastWeightRuntimeClient`、`ext_mem_pg_tx::PgTxEngine` 与 `Config` 策略串接；
//! - 每次神经 Compaction / 定向遗忘 / 按 `tx_epoch` 回滚均经 `AutonomyEnvelope` 评估、`SecretRedactor` 脱敏，
//!   以 `OUT_OF_AGENT` 写入 `audit_events` 并沉淀效能账本；
//! - 绝不在本体持有任何模型权重文件或训练循环。

use std::sync::Arc;

use ext_mem_pg_tx::{AdapterKind, FastWeightGatePolicy, FastWeightRuntimeClient};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::audit::{AuditEventInput, AuditLogger};
use crate::benchmark::ledger::{EffectivenessLedger, LedgerEventInput};
use crate::benchmark::provenance::ProvenanceRule;
use crate::body::envelope::{AutonomyEnvelope, AutonomyLevel};
use crate::body::manifest::compute_body_id;
use crate::config::Config;
use crate::memory::sleep_compaction::SleepCompactionOrchestrator;
use crate::scanner::metadata::SecretRedactor;
use crate::storage::Storage;

pub use ext_mem_pg_tx::{
    FastWeightCompactionResult, FastWeightHealthSummary, FastWeightRollbackOutcome,
    FastWeightUnlearnResult, HttpFastWeightRuntime, MockFastWeightRuntime, MockRuntimeFault,
    PgFastWeightSnapshot, PgTxEngine,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightStatusReport {
    pub fast_weight_enabled: bool,
    pub runtime_configured: bool,
    pub backend: String,
    pub fallback_active: bool,
    pub health: Option<FastWeightHealthSummary>,
    pub recent_snapshots: Vec<PgFastWeightSnapshot>,
}

pub struct FastWeightOrchestrator;

impl FastWeightOrchestrator {
    pub fn gate_policy(cfg: &Config) -> FastWeightGatePolicy {
        FastWeightGatePolicy {
            max_norm_ratio: cfg.memory.fast_weight_max_norm_ratio,
            min_eval_glue_delta: cfg.memory.fast_weight_min_glue_delta,
            max_unlearn_leak_rate: cfg.memory.fast_weight_max_unlearn_leak,
        }
    }

    fn try_connect_engine(cfg: &Config) -> Option<PgTxEngine> {
        if !cfg.memory.provider.trim().eq_ignore_ascii_case("pg_tx") {
            return None;
        }
        SleepCompactionOrchestrator::resolve_pg_config(cfg)
            .and_then(PgTxEngine::connect)
            .ok()
    }

    fn require_engine(cfg: &Config) -> Result<PgTxEngine> {
        if !cfg.memory.provider.trim().eq_ignore_ascii_case("pg_tx") {
            anyhow::bail!("memory.provider 当前为 '{}'，仅 pg_tx 支持 FastWeight 控制面", cfg.memory.provider);
        }
        SleepCompactionOrchestrator::resolve_pg_config(cfg).and_then(PgTxEngine::connect)
    }

    pub fn resolve_runtime(
        cfg: &Config,
        explicit: Option<Arc<dyn FastWeightRuntimeClient>>,
    ) -> Result<Option<Arc<dyn FastWeightRuntimeClient>>> {
        if let Some(rt) = explicit {
            return Ok(Some(rt));
        }
        let url = cfg.memory.fast_weight_runtime_url.trim();
        if url.is_empty() {
            return Ok(None);
        }
        let tok = std::env::var("AGENT_BODY_FAST_WEIGHT_TOKEN").ok();
        let client = HttpFastWeightRuntime::new(url, tok, 30_000)?;
        Ok(Some(Arc::new(client)))
    }

    pub fn status(cfg: &Config) -> Result<FastWeightStatusReport> {
        let runtime_configured = !cfg.memory.fast_weight_runtime_url.trim().is_empty();
        let Some(engine) = Self::try_connect_engine(cfg) else {
            return Ok(FastWeightStatusReport {
                fast_weight_enabled: cfg.memory.fast_weight_enabled,
                runtime_configured,
                backend: "sqlite_fallback".to_string(),
                fallback_active: true,
                health: None,
                recent_snapshots: Vec::new(),
            });
        };
        let body_id = compute_body_id();
        let scope = cfg.memory.pg_tenant_scope.as_str();
        let health = engine.fast_weight_health(&body_id, scope)?;
        let recent = engine.list_fast_weight_snapshots(&body_id, scope, 10)?;
        Ok(FastWeightStatusReport {
            fast_weight_enabled: cfg.memory.fast_weight_enabled,
            runtime_configured,
            backend: "pg_tx".to_string(),
            fallback_active: false,
            health: Some(health),
            recent_snapshots: recent,
        })
    }

    pub fn lineage(
        cfg: &Config,
        snapshot_id: Option<&str>,
        max_depth: usize,
    ) -> Result<serde_json::Value> {
        if let Some(engine) = Self::try_connect_engine(cfg) {
            let body_id = compute_body_id();
            let chain = engine.get_snapshot_lineage(
                &body_id,
                cfg.memory.pg_tenant_scope.as_str(),
                snapshot_id,
                max_depth,
            )?;
            return Ok(serde_json::json!({
                "backend": "pg_tx",
                "fallback_active": false,
                "count": chain.len(),
                "lineage": chain,
            }));
        }
        Ok(serde_json::json!({
            "backend": "sqlite_fallback",
            "fallback_active": true,
            "count": 0,
            "lineage": [],
        }))
    }

    pub fn run_compaction(
        storage: &Storage,
        cfg: &Config,
        base_model_id: Option<&str>,
        adapter_kind: Option<&str>,
        explicit_runtime: Option<Arc<dyn FastWeightRuntimeClient>>,
    ) -> Result<FastWeightCompactionResult> {
        let env_dec = AutonomyEnvelope::evaluate("fast_weight_compaction", base_model_id, false);
        if !env_dec.allowed
            || !matches!(env_dec.level, AutonomyLevel::L0Observe | AutonomyLevel::L1Autonomic)
        {
            anyhow::bail!("AutonomyEnvelope 拒绝执行 fast_weight_compaction: {}", env_dec.reason);
        }
        let runtime = Self::resolve_runtime(cfg, explicit_runtime)?.ok_or_else(|| {
            anyhow::anyhow!(
                "未配置 memory.fast_weight_runtime_url，无法触发外部神经 Compaction（保持纯方案 A 非参数记忆）"
            )
        })?;
        let engine = Self::require_engine(cfg)?;

        let body_id = compute_body_id();
        let model = base_model_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("qwen3-coder-next");
        let kind = adapter_kind
            .map(AdapterKind::parse_str)
            .transpose()?
            .unwrap_or(AdapterKind::ScolLora);
        let trace_id = format!("fwc-{}", Uuid::new_v4());
        let policy = Self::gate_policy(cfg);

        let res = engine.run_neural_compaction_and_switch(
            &body_id,
            cfg.memory.pg_tenant_scope.as_str(),
            model,
            kind,
            runtime.as_ref(),
            &policy,
            &trace_id,
        )?;

        let outcome_str = if res.gate_verdict.passed { "success" } else { "blocked" };
        let sanitized_reasons: Vec<String> = res
            .gate_verdict
            .reasons
            .iter()
            .map(|r| SecretRedactor::redact_single_line(r))
            .collect();
        AuditLogger::record_best_effort(
            storage,
            AuditEventInput::out_of_agent(
                "fast_weight",
                "fast_weight_compactor",
                "memory_op",
                "fast_weight_compaction",
            )
            .with_trace(trace_id.clone())
            .with_target("fast_weight_job", &res.job_id.to_string())
            .with_concurrency_mode("L1_AUTONOMIC")
            .with_outcome(outcome_str)
            .with_elapsed_us((res.elapsed_ms as i64).saturating_mul(1000))
            .with_delta(serde_json::json!({
                "audit_trace_id": trace_id,
                "job_state": res.job_state,
                "gate_passed": res.gate_verdict.passed,
                "fallback_to_plan_a": res.fallback_to_plan_a,
                "norm_ratio": res.metrics.norm_ratio,
                "eval_glue_delta": res.metrics.eval_glue_delta,
                "unlearn_leak_rate": res.metrics.unlearn_leak_rate,
                "merged_events": res.merged_events,
                "reasons": sanitized_reasons,
            })),
        );

        if res.gate_verdict.passed {
            let base_tok = ((res.merged_events.max(1)) as i64).saturating_mul(64);
            let body_tok = 24i64.min(base_tok.saturating_sub(1)).max(1);
            EffectivenessLedger::record(
                storage.provider().as_ref(),
                LedgerEventInput {
                    session_id: "fast_weight:m3".to_string(),
                    source_channel: "daemon".to_string(),
                    tool_or_action: "fast_weight_compaction".to_string(),
                    rule: ProvenanceRule::Pr16FastWeightGateVerified,
                    baseline_tokens: base_tok,
                    body_tokens: body_tok,
                    baseline_rounds: 3,
                    body_rounds: 1,
                    hazard_intercepted: 0,
                    noise_blocked: res.merged_events as i64,
                    elapsed_us: (res.elapsed_ms as i64).saturating_mul(1000),
                    evidence_summary: format!(
                        "PR-16: 神经 Compaction {} 条过门禁激活 (norm={:.2}, glue={:+.3}, leak={:.4})",
                        res.merged_events,
                        res.metrics.norm_ratio,
                        res.metrics.eval_glue_delta,
                        res.metrics.unlearn_leak_rate
                    ),
                },
            );
        }

        Ok(res)
    }

    pub fn run_selective_unlearn(
        storage: &Storage,
        cfg: &Config,
        base_model_id: Option<&str>,
        adapter_kind: Option<&str>,
        job_id: Option<i64>,
        explicit_runtime: Option<Arc<dyn FastWeightRuntimeClient>>,
    ) -> Result<FastWeightUnlearnResult> {
        let env_dec = AutonomyEnvelope::evaluate("selective_unlearn", base_model_id, false);
        if !env_dec.allowed
            || !matches!(env_dec.level, AutonomyLevel::L0Observe | AutonomyLevel::L1Autonomic)
        {
            anyhow::bail!("AutonomyEnvelope 拒绝执行 selective_unlearn: {}", env_dec.reason);
        }
        let runtime = Self::resolve_runtime(cfg, explicit_runtime)?.ok_or_else(|| {
            anyhow::anyhow!("未配置 memory.fast_weight_runtime_url，无法执行旁路 selective_unlearn")
        })?;
        let engine = Self::require_engine(cfg)?;

        let body_id = compute_body_id();
        let model = base_model_id
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("qwen3-coder-next");
        let kind = adapter_kind
            .map(AdapterKind::parse_str)
            .transpose()?
            .unwrap_or(AdapterKind::ScolLora);
        let trace_id = format!("fwu-{}", Uuid::new_v4());
        let policy = Self::gate_policy(cfg);

        let res = engine.execute_selective_unlearn(
            &body_id,
            cfg.memory.pg_tenant_scope.as_str(),
            model,
            kind,
            job_id,
            runtime.as_ref(),
            &policy,
            &trace_id,
        )?;

        if !res.processed_job_ids.is_empty() {
            let passed = res.gate_verdict.as_ref().map(|v| v.passed).unwrap_or(false);
            AuditLogger::record_best_effort(
                storage,
                AuditEventInput::out_of_agent(
                    "fast_weight",
                    "fast_weight_unlearner",
                    "memory_op",
                    "selective_unlearn",
                )
                .with_trace(trace_id.clone())
                .with_target("unlearn_jobs", &format!("{:?}", res.processed_job_ids))
                .with_concurrency_mode("L1_AUTONOMIC")
                .with_outcome(if passed { "success" } else { "blocked" })
                .with_elapsed_us((res.elapsed_ms as i64).saturating_mul(1000))
                .with_delta(serde_json::json!({
                    "audit_trace_id": trace_id,
                    "job_state": res.job_state,
                    "purged_count": res.purged_event_ids.len(),
                    "fallback_to_plan_a": res.fallback_to_plan_a,
                })),
            );
            if passed {
                EffectivenessLedger::record(
                    storage.provider().as_ref(),
                    LedgerEventInput {
                        session_id: "fast_weight:m3".to_string(),
                        source_channel: "daemon".to_string(),
                        tool_or_action: "selective_unlearn".to_string(),
                        rule: ProvenanceRule::Pr16FastWeightGateVerified,
                        baseline_tokens: 120,
                        body_tokens: 20,
                        baseline_rounds: 2,
                        body_rounds: 1,
                        hazard_intercepted: res.purged_event_ids.len() as i64,
                        noise_blocked: 0,
                        elapsed_us: (res.elapsed_ms as i64).saturating_mul(1000),
                        evidence_summary: format!(
                            "PR-16: 定向遗忘擦除 {} 条墓碑事件 (trace={})",
                            res.purged_event_ids.len(),
                            trace_id
                        ),
                    },
                );
            }
        }
        Ok(res)
    }

    pub fn rollback_to_epoch(
        storage: &Storage,
        cfg: &Config,
        target_epoch: i64,
        explicit_runtime: Option<Arc<dyn FastWeightRuntimeClient>>,
    ) -> Result<FastWeightRollbackOutcome> {
        let env_dec = AutonomyEnvelope::evaluate("fast_weight_rollback", None, false);
        if !env_dec.allowed
            || !matches!(env_dec.level, AutonomyLevel::L0Observe | AutonomyLevel::L1Autonomic)
        {
            anyhow::bail!("AutonomyEnvelope 拒绝执行 fast_weight_rollback: {}", env_dec.reason);
        }
        let runtime = Self::resolve_runtime(cfg, explicit_runtime)?;
        let engine = Self::require_engine(cfg)?;

        let body_id = compute_body_id();
        let trace_id = format!("fwr-{}", Uuid::new_v4());
        let outcome = engine.rollback_fast_weight_to_epoch(
            &body_id,
            cfg.memory.pg_tenant_scope.as_str(),
            target_epoch,
            runtime.as_deref(),
            &trace_id,
        )?;

        AuditLogger::record_best_effort(
            storage,
            AuditEventInput::out_of_agent(
                "fast_weight",
                "fast_weight_rollback",
                "memory_op",
                "fast_weight_rollback",
            )
            .with_trace(trace_id.clone())
            .with_target("tx_epoch", &target_epoch.to_string())
            .with_concurrency_mode("L1_AUTONOMIC")
            .with_outcome("success")
            .with_elapsed_us((outcome.elapsed_ms as i64).saturating_mul(1000))
            .with_delta(serde_json::json!({
                "audit_trace_id": trace_id,
                "rolled_back_count": outcome.rolled_back_snapshot_ids.len(),
                "restored_snapshot_id": outcome.restored_snapshot.as_ref().map(|s| s.snapshot_id.as_str()),
                "fallback_to_plan_a": outcome.fallback_to_plan_a,
                "elapsed_ms": outcome.elapsed_ms,
            })),
        );
        Ok(outcome)
    }

    /// 供 MCP / CLI / Web 1:1:1 复用的统一快权重动作入口
    pub fn execute_action(
        storage: &Storage,
        cfg: &Config,
        action: &str,
        snapshot_id: Option<&str>,
        target_epoch: Option<i64>,
        base_model_id: Option<&str>,
        adapter_kind: Option<&str>,
        explicit_runtime: Option<Arc<dyn FastWeightRuntimeClient>>,
    ) -> Result<serde_json::Value> {
        match action.trim().to_ascii_lowercase().as_str() {
            "status" => Ok(serde_json::to_value(Self::status(cfg)?)?),
            "lineage" => Self::lineage(cfg, snapshot_id, 32),
            "compact" => Ok(serde_json::to_value(Self::run_compaction(
                storage,
                cfg,
                base_model_id,
                adapter_kind,
                explicit_runtime,
            )?)?),
            "unlearn" => Ok(serde_json::to_value(Self::run_selective_unlearn(
                storage,
                cfg,
                base_model_id,
                adapter_kind,
                None,
                explicit_runtime,
            )?)?),
            "rollback" => {
                let ep = target_epoch.ok_or_else(|| {
                    anyhow::anyhow!("fast_weight_action='rollback' 必须提供 target_epoch")
                })?;
                Ok(serde_json::to_value(Self::rollback_to_epoch(
                    storage,
                    cfg,
                    ep,
                    explicit_runtime,
                )?)?)
            }
            other => anyhow::bail!(
                "未知 fast_weight_action '{}'，允许值: status | lineage | compact | unlearn | rollback",
                other
            ),
        }
    }
}
