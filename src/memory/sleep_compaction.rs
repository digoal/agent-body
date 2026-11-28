//! `L1_AUTONOMIC` 空闲期睡眠整理编排器 (`SleepCompactionOrchestrator` · Phase 77 M2)
//!
//! - 空闲窗口探测（CPU 负载 / `/proc/loadavg` + 当前无阻塞排他锁争用）
//! - `pause` / `resume` 持久化开关（`{data_dir}/sleep_compaction.paused`，`0600` 权限，零新表零 SPI）
//! - 写前强制经过 `SecretRedactor` + `PromptInjectionScanner`
//! - 优先调用 `PgTxEngine::run_sleep_compaction`，无 PG 时无损降级本地 SQLite 聚类摘要（源条目置 `'archived'`）
//! - 全量记入 `OUT_OF_AGENT` 双域审计与 `EffectivenessLedger` (`PR-15`)

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use ext_mem_pg_tx::{
    compact_episodic_texts, derive_cluster_topic, estimate_tokens_deterministic,
    CompactionClusterSummary, CompactionJobRecord, SleepCompactionConfig, SleepCompactionReport,
};

use crate::audit::{AuditEventInput, AuditLogger};
use crate::benchmark::ledger::{EffectivenessLedger, LedgerEventInput};
use crate::benchmark::provenance::ProvenanceRule;
use crate::body::envelope::{AutonomyEnvelope, AutonomyLevel};
use crate::body::manifest::compute_body_id;
use crate::config::Config;
use crate::memory::lifecycle::MemoryLifecycleManager;
use crate::memory::SalienceTier;
use crate::scanner::metadata::{PromptInjectionScanner, SecretRedactor};
use crate::storage::models::MemoryRecord;
use crate::storage::Storage;

const PAUSE_MARKER_FILE: &str = "sleep_compaction.paused";

/// 宿主机空闲窗口评估结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdleWindowStatus {
    pub is_idle: bool,
    pub estimated_cpu_pct: f32,
    pub threshold_pct: f32,
    pub active_exclusive_locks: usize,
    pub reason: String,
}

/// 睡眠整理全景观测状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SleepCompactionStatus {
    pub enabled: bool,
    pub paused: bool,
    pub backend: String,
    pub idle_window: IdleWindowStatus,
    pub active_summary_count: usize,
    pub recent_jobs: Vec<CompactionJobRecord>,
}

pub struct SleepCompactionOrchestrator;

impl SleepCompactionOrchestrator {
    pub fn pause_marker_path(data_dir: &Path) -> PathBuf {
        data_dir.join(PAUSE_MARKER_FILE)
    }

    pub fn is_paused(data_dir: &Path) -> bool {
        Self::pause_marker_path(data_dir).exists()
    }

    pub fn set_paused(data_dir: &Path, paused: bool) -> Result<bool> {
        let marker = Self::pause_marker_path(data_dir);
        if paused {
            if let Some(parent) = marker.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&marker, b"paused=1\n")?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Err(e) = std::fs::set_permissions(&marker, std::fs::Permissions::from_mode(0o600)) {
                    eprintln!("[SleepCompaction][WARN] 设置暂停开关文件权限失败: {}", e);
                }
            }
            Ok(true)
        } else {
            if marker.exists() {
                std::fs::remove_file(&marker)?;
            }
            Ok(false)
        }
    }

    /// 评估当前宿主机是否处于适合执行睡眠整理的空闲窗口
    pub fn probe_idle_window(storage: &Storage, max_cpu_pct: f32) -> IdleWindowStatus {
        let estimated_cpu_pct = sample_host_cpu_pct();
        let active_locks = storage.list_coordination_locks(None).unwrap_or_default();
        let active_exclusive_locks = active_locks
            .iter()
            .filter(|l| l.scope.eq_ignore_ascii_case("exclusive"))
            .count();

        let is_idle = estimated_cpu_pct <= max_cpu_pct && active_exclusive_locks == 0;
        let reason = if is_idle {
            format!(
                "宿主机处于空闲窗口 (load_pct={:.1}% <= {:.1}%, 排他锁={})",
                estimated_cpu_pct, max_cpu_pct, active_exclusive_locks
            )
        } else {
            format!(
                "暂缓睡眠整理以保护在线热路径 (load_pct={:.1}% vs 阈值 {:.1}%, 排他锁={})",
                estimated_cpu_pct, max_cpu_pct, active_exclusive_locks
            )
        };

        IdleWindowStatus {
            is_idle,
            estimated_cpu_pct,
            threshold_pct: max_cpu_pct,
            active_exclusive_locks,
            reason,
        }
    }

    /// 获取睡眠整理运行状态、空闲窗口与最近作业队列
    pub fn status(storage: &Storage, cfg: &Config) -> SleepCompactionStatus {
        let paused = Self::is_paused(&cfg.data_dir);
        let idle_window = Self::probe_idle_window(storage, cfg.memory.sleep_idle_cpu_pct);

        if cfg.memory.provider.trim().eq_ignore_ascii_case("pg_tx") {
            if let Ok(pg_cfg) = Self::resolve_pg_config(cfg) {
                if let Ok(engine) = ext_mem_pg_tx::PgTxEngine::connect(pg_cfg) {
                    let summaries = engine
                        .list_sleep_summary_texts(&cfg.memory.pg_tenant_scope, 50)
                        .unwrap_or_default();
                    let recent_jobs = engine
                        .list_compaction_jobs(&cfg.memory.pg_tenant_scope, 10)
                        .unwrap_or_default();
                    return SleepCompactionStatus {
                        enabled: cfg.memory.sleep_compaction_enabled,
                        paused,
                        backend: "pg_tx".to_string(),
                        idle_window,
                        active_summary_count: summaries.len(),
                        recent_jobs,
                    };
                }
            }
        }

        let local_summaries = storage
            .list_all_active_memories(500)
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.tags.iter().any(|t| t == "sleep_compacted" || t == "sleep_summary"))
            .count();

        SleepCompactionStatus {
            enabled: cfg.memory.sleep_compaction_enabled,
            paused,
            backend: "sqlite_fallback".to_string(),
            idle_window,
            active_summary_count: local_summaries,
            recent_jobs: vec![],
        }
    }

    /// 守护进程周期调度入口（仅当启用、未暂停、空闲窗口满足且包络放行时运行）
    pub fn maybe_run_idle_cycle(storage: &Storage, cfg: &Config) -> Option<SleepCompactionReport> {
        if !cfg.memory.sleep_compaction_enabled || Self::is_paused(&cfg.data_dir) {
            return None;
        }
        let idle = Self::probe_idle_window(storage, cfg.memory.sleep_idle_cpu_pct);
        if !idle.is_idle {
            return None;
        }
        match Self::execute(storage, cfg, false, false) {
            Ok(rep) => Some(rep),
            Err(e) => {
                eprintln!("[SleepCompaction][WARN] 空闲睡眠整理周期异常: {}", e);
                None
            }
        }
    }

    /// 执行一次睡眠整理（支持 `dry_run` 与 `force` 强制立即触发）
    pub fn execute(
        storage: &Storage,
        cfg: &Config,
        dry_run: bool,
        force: bool,
    ) -> Result<SleepCompactionReport> {
        let env_decision = AutonomyEnvelope::evaluate("sleep_compaction", Some("memories"), dry_run);
        if !env_decision.allowed || !matches!(env_decision.level, AutonomyLevel::L0Observe | AutonomyLevel::L1Autonomic) {
            anyhow::bail!("自治权限包络拦截睡眠整理: {}", env_decision.reason);
        }

        if !dry_run && !force {
            if !cfg.memory.sleep_compaction_enabled {
                return Ok(empty_report("disabled", dry_run, "config_disabled"));
            }
            if Self::is_paused(&cfg.data_dir) {
                return Ok(empty_report("paused", dry_run, "paused_by_operator"));
            }
            let idle = Self::probe_idle_window(storage, cfg.memory.sleep_idle_cpu_pct);
            if !idle.is_idle {
                return Ok(empty_report("busy_deferred", dry_run, &idle.reason));
            }
        }

        let trace_id = format!("sleep-{}", uuid::Uuid::new_v4());
        let sc_cfg = SleepCompactionConfig {
            min_events: cfg.memory.sleep_min_events.max(2),
            max_batch: cfg.memory.sleep_max_batch.max(cfg.memory.sleep_min_events),
            salience_ceiling: 0.72,
            dry_run,
            audit_trace_id: trace_id.clone(),
        };

        let mut report = if cfg.memory.provider.trim().eq_ignore_ascii_case("pg_tx") {
            match Self::resolve_pg_config(cfg).and_then(ext_mem_pg_tx::PgTxEngine::connect) {
                Ok(engine) => {
                    let body_id = compute_body_id();
                    engine.run_sleep_compaction(
                        &body_id,
                        &cfg.memory.pg_tenant_scope,
                        &sc_cfg,
                        sanitize_sleep_summary_text,
                    )?
                }
                Err(e) if cfg.memory.fallback_to_sqlite => {
                    eprintln!("[SleepCompaction][WARN] PG 不可达，降级 SQLite 本地整理: {}", e);
                    Self::compact_sqlite_memories(storage, &sc_cfg)?
                }
                Err(e) => return Err(e),
            }
        } else {
            Self::compact_sqlite_memories(storage, &sc_cfg)?
        };

        if !dry_run && report.events_compacted > 0 {
            let delta = serde_json::json!({
                "backend": report.backend,
                "job_id": report.job_id,
                "events_scanned": report.events_scanned,
                "events_compacted": report.events_compacted,
                "clusters_created": report.clusters_created,
                "raw_tokens": report.raw_tokens,
                "compacted_tokens": report.compacted_tokens,
                "tokens_saved": report.tokens_saved,
                "compression_ratio": report.compression_ratio,
                "preserved_fact_count": report.preserved_fact_count,
                "state": report.state,
            });
            AuditLogger::record_best_effort(
                storage,
                AuditEventInput::out_of_agent(
                    "daemon_scheduler",
                    "sleep_compactor",
                    "memory_op",
                    "sleep_compaction",
                )
                .with_trace(trace_id.clone())
                .with_target("memory_store", &report.backend)
                .with_concurrency_mode("L1_AUTONOMIC")
                .with_outcome("success")
                .with_elapsed_us(report.elapsed_us as i64)
                .with_delta(delta),
            );
            EffectivenessLedger::record(
                storage.provider().as_ref(),
                LedgerEventInput {
                    session_id: "daemon-sleep".to_string(),
                    source_channel: "daemon".to_string(),
                    tool_or_action: "sleep_compaction".to_string(),
                    rule: ProvenanceRule::Pr15SleepCompactionSavings,
                    baseline_tokens: report.raw_tokens,
                    body_tokens: report.compacted_tokens,
                    baseline_rounds: report.events_compacted as i64,
                    body_rounds: report.clusters_created.max(1) as i64,
                    hazard_intercepted: report.preserved_fact_count as i64,
                    noise_blocked: report.events_compacted.saturating_sub(report.clusters_created) as i64,
                    elapsed_us: report.elapsed_us as i64,
                    evidence_summary: format!(
                        "PR-15: 空闲睡眠整理 {} 条 -> {} 组摘要 ({} -> {} tok, {:.2}x 压缩, trace={})",
                        report.events_compacted,
                        report.clusters_created,
                        report.raw_tokens,
                        report.compacted_tokens,
                        report.compression_ratio,
                        report.audit_trace_id
                    ),
                },
            );
            report.audit_trace_id = trace_id;
        }

        Ok(report)
    }

    pub(crate) fn resolve_pg_config(cfg: &Config) -> Result<ext_mem_pg_tx::PgTxConfig> {
        let resolved_url = cfg
            .memory
            .pg_url
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| cfg.storage.pg_url.clone().filter(|s| !s.trim().is_empty()))
            .or_else(|| {
                std::env::var("AGENT_BODY_MEMORY_PG_URL")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            })
            .unwrap_or_default();
        if resolved_url.trim().is_empty() {
            anyhow::bail!("未配置 pg_url");
        }
        let mut pg_cfg = ext_mem_pg_tx::PgTxConfig::new(resolved_url);
        pg_cfg.pool_size = cfg.memory.pg_pool_size.clamp(1, 64);
        pg_cfg.tenant_scope = if cfg.memory.pg_tenant_scope.trim().is_empty() {
            "default".to_string()
        } else {
            cfg.memory.pg_tenant_scope.trim().to_string()
        };
        pg_cfg.vector_backend =
            ext_mem_pg_tx::VectorBackendKind::parse_str(&cfg.memory.vector_backend)?;
        Ok(pg_cfg)
    }

    /// SQLite 本地降级睡眠整理（遵循 `memories.status CHECK('active','archived','outdated')`）
    fn compact_sqlite_memories(
        storage: &Storage,
        sc_cfg: &SleepCompactionConfig,
    ) -> Result<SleepCompactionReport> {
        let t0 = Instant::now();
        let active = storage.list_all_active_memories(sc_cfg.max_batch.max(32))?;
        let events_scanned = active.len();
        let all_texts: Vec<&str> = active.iter().map(|m| m.content.as_str()).collect();

        let mut by_topic: BTreeMap<String, Vec<MemoryRecord>> = BTreeMap::new();
        for (idx, m) in active.iter().enumerate() {
            if m.pinned || m.tags.iter().any(|t| t == "sleep_compacted" || t == "sleep_summary") {
                continue;
            }
            let mut peers = Vec::with_capacity(all_texts.len().saturating_sub(1));
            for (j, text) in all_texts.iter().enumerate() {
                if j != idx {
                    peers.push(*text);
                }
            }
            let sal = MemoryLifecycleManager::score_memory_record(m, &peers);
            let is_long = m.content.len() > 240;
            if !matches!(sal.tier, SalienceTier::High) || is_long {
                let topic = derive_cluster_topic(&m.id, &m.tags);
                by_topic.entry(topic).or_default().push(m.clone());
            }
        }

        let mut clusters = Vec::new();
        let mut writes: Vec<(String, String, Vec<String>, Vec<MemoryRecord>)> = Vec::new();
        let mut total_raw_tokens = 0i64;
        let mut total_compacted_tokens = 0i64;
        let mut total_events_compacted = 0usize;
        let mut total_facts = 0usize;

        for (topic, group) in by_topic {
            if group.len() < sc_cfg.min_events.max(2) {
                continue;
            }
            let pairs: Vec<(String, String)> = group
                .iter()
                .map(|m| (m.id.clone(), m.content.clone()))
                .collect();
            let raw_tok: i64 = group
                .iter()
                .map(|m| estimate_tokens_deterministic(&m.content))
                .sum();
            let (raw_sum, fact_cnt) = compact_episodic_texts(&topic, &pairs);
            let clean_sum = sanitize_sleep_summary_text(&raw_sum);
            let comp_tok = estimate_tokens_deterministic(&clean_sum);
            if comp_tok >= raw_tok {
                continue;
            }

            let mut tags = BTreeSet::new();
            tags.insert("sleep_compacted".to_string());
            tags.insert("sleep_summary".to_string());
            tags.insert(topic.clone());
            let tag_vec: Vec<String> = tags.into_iter().collect();

            total_raw_tokens += raw_tok;
            total_compacted_tokens += comp_tok;
            total_events_compacted += group.len();
            total_facts += fact_cnt;

            let summary_id = format!("sleep-sum-{}", topic);
            clusters.push(CompactionClusterSummary {
                topic: topic.clone(),
                source_event_ids: (1..=group.len() as i64).collect(),
                raw_tokens: raw_tok,
                compacted_tokens: comp_tok,
                preserved_fact_count: fact_cnt,
                compacted_fact_key: summary_id.clone(),
                compacted_preview: clean_sum.chars().take(180).collect(),
            });
            writes.push((topic, clean_sum, tag_vec, group));
        }

        if !sc_cfg.dry_run && !writes.is_empty() {
            for (topic, clean_sum, tag_vec, group) in writes {
                for src in &group {
                    storage.update_memory_status(&src.id, "archived")?;
                }
                let rec = MemoryRecord {
                    id: format!("sleep-sum-{}", topic),
                    memory_type: "experience".to_string(),
                    content: clean_sum,
                    summary: Some(format!("Sleep compacted summary ({})", topic)),
                    source_agent: Some("sleep_time_compactor".to_string()),
                    source_session: Some(sc_cfg.audit_trace_id.clone()),
                    tags: tag_vec,
                    weight: 0.85,
                    pinned: false,
                    status: "active".to_string(),
                    embedding: vec![],
                };
                storage.insert_memory(&rec)?;
            }
        }

        let saved = total_raw_tokens.saturating_sub(total_compacted_tokens);
        let ratio = if total_compacted_tokens > 0 {
            ((total_raw_tokens as f64 / total_compacted_tokens as f64) * 100.0).round() / 100.0
        } else {
            1.0
        };
        let state = if sc_cfg.dry_run {
            "dry_run"
        } else if total_events_compacted > 0 {
            "committed"
        } else {
            "skipped_insufficient_events"
        };

        Ok(SleepCompactionReport {
            job_id: None,
            backend: "sqlite_fallback".to_string(),
            tenant_scope: "default".to_string(),
            dry_run: sc_cfg.dry_run,
            recovered_aborted_jobs: 0,
            events_scanned,
            events_compacted: total_events_compacted,
            clusters_created: clusters.len(),
            raw_tokens: total_raw_tokens,
            compacted_tokens: total_compacted_tokens,
            tokens_saved: saved,
            compression_ratio: ratio,
            preserved_fact_count: total_facts,
            state: state.to_string(),
            audit_trace_id: sc_cfg.audit_trace_id.clone(),
            elapsed_us: t0.elapsed().as_micros(),
            clusters,
        })
    }
}

pub fn sanitize_sleep_summary_text(raw: &str) -> String {
    let redacted = SecretRedactor::redact_preserving_newlines(raw);
    PromptInjectionScanner::scan_and_sanitize(&redacted).sanitized
}

fn empty_report(state: &str, dry_run: bool, trace_note: &str) -> SleepCompactionReport {
    SleepCompactionReport {
        job_id: None,
        backend: "none".to_string(),
        tenant_scope: "default".to_string(),
        dry_run,
        recovered_aborted_jobs: 0,
        events_scanned: 0,
        events_compacted: 0,
        clusters_created: 0,
        raw_tokens: 0,
        compacted_tokens: 0,
        tokens_saved: 0,
        compression_ratio: 1.0,
        preserved_fact_count: 0,
        state: state.to_string(),
        audit_trace_id: trace_note.to_string(),
        elapsed_us: 0,
        clusters: vec![],
    }
}

fn sample_host_cpu_pct() -> f32 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(raw) = std::fs::read_to_string("/proc/loadavg") {
            if let Some(load1) = raw
                .split_whitespace()
                .next()
                .and_then(|s| s.parse::<f32>().ok())
            {
                let cpus = std::thread::available_parallelism()
                    .map(|n| n.get() as f32)
                    .unwrap_or(1.0)
                    .max(1.0);
                return ((load1 / cpus) * 100.0).clamp(0.0, 100.0);
            }
        }
    }
    10.0
}
