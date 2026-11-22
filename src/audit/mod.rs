//! 双域全链路行为审计引擎 (`AuditLogger`) 与 `Watcher` 60s 折叠防洪门禁 (Phase 43)
//!
//! 对齐规约 #4, #8, #18, #22, #26：
//! - `IN_AGENT` 域：Brief 租约、Pre/Post Hook 裁决与自愈、41 原生 MCP 工具、CLI、Web 写端点、Mesh 协同
//! - `OUT_OF_AGENT` 域：Daemon 30m 巡检、Watcher 60s 折叠防洪、HostGC/LinkRepair/Reconcile/RuleLearn/锁 GC
//! - 全链路强制 `SecretRedactor` 脱敏、`before_after_delta <= 4KB` 截断与 `evidence_sha256` 存证指纹

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub mod evolution;
pub use evolution::{AuditEvolutionAnalyzer, AuditEvolutionReport, FrictionHotspot, LeaseHealthEval};

use anyhow::Result;
use serde_json::Value;

use crate::benchmark::provenance::sha256_hex;
use crate::scanner::metadata::SecretRedactor;
use crate::storage::models::AuditEventRecord;
use crate::storage::Storage;

const MAX_DELTA_BYTES: usize = 4096;
const WATCHER_FOLD_WINDOW: Duration = Duration::from_secs(60);

/// 审计事件构建入参
#[derive(Debug, Clone)]
pub struct AuditEventInput {
    pub trace_id: Option<String>,
    pub session_id: String,
    /// `"IN_AGENT"` | `"OUT_OF_AGENT"`
    pub domain: String,
    pub actor_kind: String,
    pub actor_id: String,
    pub source_channel: String,
    pub category: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub concurrency_mode: Option<String>,
    /// `"success"` | `"warn"` | `"blocked"` | `"failed"` | `"recovered"` | `"dry_run"`
    pub outcome: String,
    pub before_after_delta: Value,
    pub elapsed_us: i64,
}

impl AuditEventInput {
    pub fn in_agent(
        channel: impl Into<String>,
        category: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        let ch = channel.into();
        let actor_kind = match ch.as_str() {
            "mcp" => "mcp_client",
            "cli" => "cli_user",
            "web" => "web_user",
            "mesh" => "mesh_peer",
            _ => "agent_session",
        };
        Self {
            trace_id: None,
            session_id: format!("{}-live", ch),
            domain: "IN_AGENT".to_string(),
            actor_kind: actor_kind.to_string(),
            actor_id: "agent".to_string(),
            source_channel: ch,
            category: category.into(),
            action: action.into(),
            target_type: "none".to_string(),
            target_id: String::new(),
            concurrency_mode: None,
            outcome: "success".to_string(),
            before_after_delta: serde_json::json!({}),
            elapsed_us: 0,
        }
    }

    pub fn out_of_agent(
        actor_kind: impl Into<String>,
        source_channel: impl Into<String>,
        category: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            trace_id: None,
            session_id: "system".to_string(),
            domain: "OUT_OF_AGENT".to_string(),
            actor_kind: actor_kind.into(),
            actor_id: "steward".to_string(),
            source_channel: source_channel.into(),
            category: category.into(),
            action: action.into(),
            target_type: "none".to_string(),
            target_id: String::new(),
            concurrency_mode: None,
            outcome: "success".to_string(),
            before_after_delta: serde_json::json!({}),
            elapsed_us: 0,
        }
    }

    pub fn with_trace(mut self, trace_id: impl Into<String>) -> Self {
        self.trace_id = Some(trace_id.into());
        self
    }

    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = session_id.into();
        self
    }

    pub fn with_actor(mut self, actor_id: impl Into<String>) -> Self {
        self.actor_id = actor_id.into();
        self
    }

    pub fn with_target(
        mut self,
        target_type: impl Into<String>,
        target_id: impl Into<String>,
    ) -> Self {
        self.target_type = target_type.into();
        self.target_id = target_id.into();
        self
    }

    pub fn with_concurrency_mode(mut self, mode: impl Into<String>) -> Self {
        self.concurrency_mode = Some(mode.into());
        self
    }

    pub fn with_outcome(mut self, outcome: impl Into<String>) -> Self {
        self.outcome = outcome.into();
        self
    }

    pub fn with_delta(mut self, delta: Value) -> Self {
        self.before_after_delta = delta;
        self
    }

    pub fn with_elapsed_us(mut self, elapsed_us: i64) -> Self {
        self.elapsed_us = elapsed_us;
        self
    }
}

struct WatcherFoldBucket {
    window_start: Instant,
    suppressed_count: usize,
}

static WATCHER_FOLD_STATE: OnceLock<Mutex<HashMap<String, WatcherFoldBucket>>> = OnceLock::new();

fn watcher_fold_map() -> &'static Mutex<HashMap<String, WatcherFoldBucket>> {
    WATCHER_FOLD_STATE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FoldDecision {
    EmitNow { suppressed_prev: usize },
    FoldedSkip,
}

fn evaluate_watcher_fold(key: &str, now: Instant, window: Duration) -> FoldDecision {
    let mut guard = watcher_fold_map()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if guard.len() > 2048 {
        guard.retain(|_, b| now.duration_since(b.window_start) < window);
    }
    match guard.get_mut(key) {
        Some(bucket) if now.duration_since(bucket.window_start) < window => {
            bucket.suppressed_count += 1;
            FoldDecision::FoldedSkip
        }
        Some(bucket) => {
            let prev = bucket.suppressed_count;
            bucket.window_start = now;
            bucket.suppressed_count = 0;
            FoldDecision::EmitNow {
                suppressed_prev: prev,
            }
        }
        None => {
            guard.insert(
                key.to_string(),
                WatcherFoldBucket {
                    window_start: now,
                    suppressed_count: 0,
                },
            );
            FoldDecision::EmitNow { suppressed_prev: 0 }
        }
    }
}

/// 清空 Watcher 折叠状态（主要用于测试隔离）
pub fn reset_watcher_fold_cache() {
    let mut guard = watcher_fold_map()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    guard.clear();
}

pub struct AuditLogger;

impl AuditLogger {
    /// 标准化、脱敏并写入一条双域审计记录；对 `watcher_inotify` 实施 60s 滑动窗防洪折叠
    pub fn record(storage: &Storage, input: AuditEventInput) -> Result<Option<AuditEventRecord>> {
        let mut input = input;

        if input.actor_kind == "watcher_inotify" {
            let fold_key = format!("{}::{}", input.action, input.target_id);
            match evaluate_watcher_fold(&fold_key, Instant::now(), WATCHER_FOLD_WINDOW) {
                FoldDecision::FoldedSkip => return Ok(None),
                FoldDecision::EmitNow { suppressed_prev } => {
                    if suppressed_prev > 0 {
                        if let Some(obj) = input.before_after_delta.as_object_mut() {
                            obj.insert(
                                "folded_suppressed_prev_window".to_string(),
                                serde_json::json!(suppressed_prev),
                            );
                        } else {
                            input.before_after_delta = serde_json::json!({
                                "raw": input.before_after_delta,
                                "folded_suppressed_prev_window": suppressed_prev,
                            });
                        }
                    }
                }
            }
        }

        let rec = Self::build_sanitized_record(input);
        storage.insert_audit_event(&rec)?;
        Ok(Some(rec))
    }

    /// 非阻塞容错写：失败仅记 stderr，绝不中断主业务流程
    pub fn record_best_effort(storage: &Storage, input: AuditEventInput) {
        if let Err(e) = Self::record(storage, input) {
            eprintln!("[AuditLogger] 审计写入失败(已降级忽略): {}", e);
        }
    }

    /// 构造经 SecretRedactor 脱敏、<=4KB 截断且附带 SHA-256 证据指纹的审计记录
    pub fn build_sanitized_record(input: AuditEventInput) -> AuditEventRecord {
        let trace_id = input
            .trace_id
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| format!("tr-{}", uuid::Uuid::new_v4()));
        let session_id = SecretRedactor::redact_single_line(if input.session_id.trim().is_empty() {
            "system"
        } else {
            &input.session_id
        });
        let domain = if input.domain == "OUT_OF_AGENT" {
            "OUT_OF_AGENT".to_string()
        } else {
            "IN_AGENT".to_string()
        };
        let actor_id = SecretRedactor::redact_single_line(&input.actor_id);
        let target_id = SecretRedactor::redact_single_line(&input.target_id);
        let outcome = match input.outcome.as_str() {
            "success" | "warn" | "blocked" | "failed" | "recovered" | "dry_run" => input.outcome,
            _ => "success".to_string(),
        };

        let redacted_delta = SecretRedactor::redact_json_value(&input.before_after_delta);
        let bounded_delta = truncate_delta_json(redacted_delta, MAX_DELTA_BYTES);
        let delta_str = serde_json::to_string(&bounded_delta).unwrap_or_else(|_| "{}".to_string());
        let created_at = chrono::Utc::now().to_rfc3339();

        let canonical = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            trace_id,
            session_id,
            domain,
            input.actor_kind,
            input.source_channel,
            input.category,
            input.action,
            input.target_type,
            target_id,
            outcome,
            delta_str,
            created_at
        );
        let evidence_sha256 = sha256_hex(&canonical);

        AuditEventRecord {
            event_id: format!("aud-{}", uuid::Uuid::new_v4()),
            trace_id,
            session_id,
            domain,
            actor_kind: input.actor_kind,
            actor_id,
            source_channel: input.source_channel,
            category: input.category,
            action: input.action,
            target_type: input.target_type,
            target_id,
            concurrency_mode: input.concurrency_mode,
            outcome,
            before_after_delta: bounded_delta,
            elapsed_us: input.elapsed_us,
            evidence_sha256,
            created_at,
        }
    }
}

fn truncate_delta_json(val: Value, max_bytes: usize) -> Value {
    let serialized = serde_json::to_string(&val).unwrap_or_else(|_| "{}".to_string());
    if serialized.len() <= max_bytes {
        return val;
    }
    let preview_limit = max_bytes.saturating_sub(256).min(3072);
    let mut end = preview_limit.min(serialized.len());
    while end > 0 && !serialized.is_char_boundary(end) {
        end -= 1;
    }
    serde_json::json!({
        "_truncated": true,
        "original_bytes": serialized.len(),
        "preview": &serialized[..end],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::AuditQueryFilter;

    #[test]
    fn test_audit_logger_redacts_secrets_truncates_4kb_and_computes_sha256() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let huge_payload = "x".repeat(6000);
        let input = AuditEventInput::in_agent("mcp", "tool_call", "body_memory_add")
            .with_session("sess-sec-1")
            .with_target(
                "memory",
                "postgres://admin:SuperSecret99@127.0.0.1:5432/prod",
            )
            .with_concurrency_mode("CAUTIOUS_CAS")
            .with_outcome("success")
            .with_delta(serde_json::json!({
                "token": "sk-abc1234567890abcdef",
                "huge": huge_payload,
            }))
            .with_elapsed_us(145);

        let rec = AuditLogger::record(&storage, input)
            .expect("record")
            .expect("should emit");
        assert_eq!(rec.domain, "IN_AGENT");
        assert_eq!(rec.evidence_sha256.len(), 64);
        assert!(!rec.target_id.contains("SuperSecret99"));
        let delta_str = serde_json::to_string(&rec.before_after_delta).unwrap();
        assert!(!delta_str.contains("sk-abc1234567890abcdef"));
        assert!(delta_str.len() <= MAX_DELTA_BYTES);
        assert_eq!(rec.before_after_delta["_truncated"], serde_json::json!(true));

        let rows = storage
            .query_audit_events(&AuditQueryFilter {
                session_id: Some("sess-sec-1".to_string()),
                limit: 10,
                ..Default::default()
            })
            .expect("query");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].event_id, rec.event_id);
    }

    #[test]
    fn test_audit_logger_watcher_60s_fold_deduplication() {
        reset_watcher_fold_cache();
        let storage = Storage::open_in_memory().expect("open_in_memory");

        for i in 0..5 {
            let input = AuditEventInput::out_of_agent(
                "watcher_inotify",
                "watcher",
                "fs_watch_sync",
                "file_modified",
            )
            .with_target("file", "/root/new/work/hot_saved.rs")
            .with_delta(serde_json::json!({"seq": i}));
            let _ = AuditLogger::record(&storage, input).expect("record");
        }

        let rows = storage
            .query_audit_events(&AuditQueryFilter {
                actor_kind: Some("watcher_inotify".to_string()),
                limit: 20,
                ..Default::default()
            })
            .expect("query");
        assert_eq!(rows.len(), 1, "60s 窗口内同路径多次保存应折叠为 1 条");
    }

    #[test]
    fn test_audit_events_sqlite_summary_and_prune() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        AuditLogger::record(
            &storage,
            AuditEventInput::in_agent("cli", "hook_pre_guard", "run_command")
                .with_outcome("blocked")
                .with_elapsed_us(90),
        )
        .unwrap();
        AuditLogger::record(
            &storage,
            AuditEventInput::out_of_agent("governance_engine", "gc", "host_gc", "cleanup_tmp")
                .with_target("dir", "/root/new/tmp")
                .with_outcome("dry_run")
                .with_delta(serde_json::json!({"candidates": 3}))
                .with_elapsed_us(310),
        )
        .unwrap();

        let summary = storage.audit_domain_summary("7d").expect("summary");
        assert_eq!(summary.total_events, 2);
        assert_eq!(summary.in_agent_count, 1);
        assert_eq!(summary.out_of_agent_count, 1);
        assert_eq!(summary.failed_or_blocked_count, 1);

        let pruned = storage
            .prune_old_audit_events("2099-01-01T00:00:00Z")
            .expect("prune");
        assert_eq!(pruned, 2);
    }

    #[test]
    fn test_step43_3_in_agent_and_out_of_agent_wiring() {
        use std::sync::Arc;
        let storage = Storage::open_in_memory().expect("open_in_memory");

        // 1. IN_AGENT: BriefAssembler + Pre/Post Hook + MCP (无 ROI 规则的工具 body_env_show)
        let assembler = crate::brief::BriefAssembler::new(storage.clone(), None);
        let _ = assembler
            .assemble(&crate::brief::BriefRequest {
                query: "audit check".to_string(),
                session_id: "sess-wire-43".to_string(),
                budget_tokens: 800,
            })
            .expect("brief");

        let hook_engine = crate::api::hooks::AgentHookEngine::new(storage.clone());
        let _ = hook_engine.pre_tool_call(
            "run_command",
            &serde_json::json!({"command": "psql -h 127.0.0.1 -p 5432 -c 'select 1'"}),
        );
        let _ = hook_engine.post_tool_call_with_error(
            "run_command",
            &serde_json::json!({"command": "psql -h 127.0.0.1 -p 59998"}),
            false,
            Some("Connection refused on port 59998"),
        );

        let registry = crate::api::mcp_tools::McpToolRegistry::new(storage.clone());
        let _ = registry
            .call_tool("body_env_show", serde_json::json!({}))
            .expect("mcp env show");

        // 2. OUT_OF_AGENT: HostGC audited + LinkRepairer + PlacementLearner + MemoryLifecycle + Reconciler
        let dir = tempfile::tempdir().expect("tempdir");
        let tmp_sub = dir.path().join("tmp_audit_area");
        std::fs::create_dir_all(&tmp_sub).unwrap();
        let _ = crate::env::gc::HostGarbageCollector::run_gc_audited(&storage, &tmp_sub, 24, true)
            .expect("gc audited");

        let repairer = crate::links::repair::LinkRepairer::new(Arc::new(storage.clone()));
        let _ = repairer.repair_links(false).expect("repair dry run");

        let learner = crate::placement::learner::PlacementLearner::new(&storage);
        let _ = learner
            .apply_candidate(&crate::placement::learner::LearnedRuleCandidate {
                candidate_id: "cand-1".into(),
                rule_name: "learned_audit_test".into(),
                pattern_type: "extension".into(),
                target_base_dir: "/root/new/tmp/".into(),
                naming_pattern: "{name}.log".into(),
                description: "test".into(),
                sample_count: 3,
                confidence: 0.9,
                sample_paths: vec![],
            })
            .expect("apply candidate");

        let mem_mgr = crate::memory::lifecycle::MemoryLifecycleManager::new(&storage);
        let _ = mem_mgr.audit_memories().expect("audit memories");

        let _ = crate::feedback::probes::FreshnessReconciler::reconcile(&storage).expect("reconcile");

        let sum = storage.audit_domain_summary("7d").expect("summary");
        assert!(sum.in_agent_count >= 4, "IN_AGENT 应至少记录 4 条，实际 {}", sum.in_agent_count);
        assert!(
            sum.out_of_agent_count >= 5,
            "OUT_OF_AGENT 应至少记录 5 条，实际 {}",
            sum.out_of_agent_count
        );
        assert!(sum.recovered_count >= 1, "碰壁自愈应记为 recovered");
    }
}
