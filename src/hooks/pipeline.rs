use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use super::{HookAction, HookFilter, StewardEvent};
use crate::storage::Storage;
use crate::storage::models::ChangeRecord;

/// Pipeline 失败处理策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnFailureStrategy {
    Abort,
    Continue,
}

/// Pipeline 执行单步定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookStep {
    pub name: String,
    pub action: HookAction,
    pub filter: Option<HookFilter>,
    #[serde(default = "default_on_failure")]
    pub on_failure: OnFailureStrategy,
    #[serde(default = "default_step_timeout_ms")]
    pub timeout_ms: u64,
}

fn default_on_failure() -> OnFailureStrategy {
    OnFailureStrategy::Abort
}

fn default_step_timeout_ms() -> u64 {
    3000
}

/// Hook 链式编排 Pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookPipeline {
    pub id: String,
    pub name: String,
    pub event_pattern: String, // "on_file_created", "on_memory_added", "*"
    pub steps: Vec<HookStep>,
    #[serde(default = "default_debounce_ms")]
    pub debounce_ms: u64,
}

fn default_debounce_ms() -> u64 {
    500
}

/// Pipeline 执行状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PipelineExecutionStatus {
    Success,
    Failed,
    SkippedDebounced,
    Aborted,
}

/// Pipeline 审计执行记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookAuditRecord {
    pub id: String,
    pub pipeline_id: String,
    pub event_type: String,
    pub status: PipelineExecutionStatus,
    pub duration_ms: u64,
    pub steps_executed: usize,
    pub total_steps: usize,
    pub error_message: Option<String>,
    pub created_at: String,
}

/// Hook Pipeline 编排执行引擎 (Phase 14.6)
pub struct HookPipelineEngine {
    pipelines: Vec<HookPipeline>,
    recent_hashes: Arc<RwLock<HashMap<u64, Instant>>>,
    audit_history: Arc<RwLock<Vec<HookAuditRecord>>>,
    storage: Option<Storage>,
}

impl HookPipelineEngine {
    pub fn new(pipelines: Vec<HookPipeline>, storage: Option<Storage>) -> Self {
        Self {
            pipelines,
            recent_hashes: Arc::new(RwLock::new(HashMap::new())),
            audit_history: Arc::new(RwLock::new(Vec::new())),
            storage,
        }
    }

    /// 计算事件哈希以进行幂等防抖
    fn compute_event_hash(event: &StewardEvent) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        event.event_type().hash(&mut hasher);
        if let Some(target) = event.target_path() {
            target.hash(&mut hasher);
        }
        hasher.finish()
    }

    /// 评估是否命中防抖去重窗口
    fn is_debounced(&self, hash: u64, debounce_ms: u64) -> bool {
        let mut map = self.recent_hashes.write().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if let Some(last_time) = map.get(&hash) {
            if now.duration_since(*last_time) < Duration::from_millis(debounce_ms) {
                return true;
            }
        }
        map.insert(hash, now);
        false
    }

    /// 执行匹配的所有编排链路
    pub async fn execute(&self, event: Arc<StewardEvent>) -> Vec<HookAuditRecord> {
        let mut audits = Vec::new();
        let event_type_str = event.event_type();
        let event_hash = Self::compute_event_hash(&event);

        for pipeline in &self.pipelines {
            if pipeline.event_pattern != "*" && pipeline.event_pattern != event_type_str {
                continue;
            }

            // 1. 幂等去重防抖检查
            if pipeline.debounce_ms > 0 && self.is_debounced(event_hash, pipeline.debounce_ms) {
                let rec = HookAuditRecord {
                    id: format!("audit-{}", uuid::Uuid::new_v4()),
                    pipeline_id: pipeline.id.clone(),
                    event_type: event_type_str.to_string(),
                    status: PipelineExecutionStatus::SkippedDebounced,
                    duration_ms: 0,
                    steps_executed: 0,
                    total_steps: pipeline.steps.len(),
                    error_message: Some("命中防抖时间窗口，幂等去重跳过".to_string()),
                    created_at: chrono::Local::now().to_rfc3339(),
                };
                self.record_audit(rec.clone());
                audits.push(rec);
                continue;
            }

            // 2. 串行执行各步骤
            let start = Instant::now();
            let mut steps_executed = 0;
            let mut pipeline_status = PipelineExecutionStatus::Success;
            let mut error_msg = None;

            for step in &pipeline.steps {
                // 条件过滤
                if let Some(ref filter) = step.filter {
                    if !filter.matches(&event) {
                        continue;
                    }
                }

                steps_executed += 1;
                let step_res = self.execute_step(step, &event).await;

                if let Err(e) = step_res {
                    error_msg = Some(format!("Step `{}` 失败: {}", step.name, e));
                    if step.on_failure == OnFailureStrategy::Abort {
                        pipeline_status = PipelineExecutionStatus::Aborted;
                        break;
                    } else {
                        pipeline_status = PipelineExecutionStatus::Failed;
                    }
                }
            }

            let duration_ms = start.elapsed().as_millis() as u64;
            let audit_rec = HookAuditRecord {
                id: format!("audit-{}", uuid::Uuid::new_v4()),
                pipeline_id: pipeline.id.clone(),
                event_type: event_type_str.to_string(),
                status: pipeline_status,
                duration_ms,
                steps_executed,
                total_steps: pipeline.steps.len(),
                error_message: error_msg,
                created_at: chrono::Local::now().to_rfc3339(),
            };

            self.record_audit(audit_rec.clone());
            audits.push(audit_rec);
        }

        audits
    }

    /// 执行单个 Hook 步骤
    async fn execute_step(&self, step: &HookStep, event: &StewardEvent) -> Result<()> {
        match &step.action {
            HookAction::Exec { command } => {
                let cmd_interpolated = super::dispatcher::interpolate_hook_command(command, event)?;
                let out = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd_interpolated)
                    .output()
                    .await?;

                if !out.status.success() {
                    let err = String::from_utf8_lossy(&out.stderr);
                    anyhow::bail!("命令执行非零退出 (code: {:?}): {}", out.status.code(), err);
                }
            }
            HookAction::Webhook { url, timeout_ms } => {
                let pusher = super::push::WebhookPusher::default();
                let rep = pusher.send_event(url, event, *timeout_ms).await;
                if !rep.success {
                    anyhow::bail!("Webhook 投递失败: {:?}", rep.error_message);
                }
            }
        }
        Ok(())
    }

    /// 记录审计流水到内存与 Storage 持久化
    fn record_audit(&self, record: HookAuditRecord) {
        {
            let mut hist = self.audit_history.write().unwrap_or_else(|e| e.into_inner());
            hist.push(record.clone());
            if hist.len() > 100 {
                hist.remove(0);
            }
        }

        if let Some(ref storage) = self.storage {
            let change = ChangeRecord {
                id: None,
                change_type: "hook_pipeline:audit".to_string(),
                target: record.pipeline_id.clone(),
                description: Some(format!(
                    "Hook Pipeline [{}] 执行完成 (状态: {:?}, 耗时: {}ms)",
                    record.pipeline_id, record.status, record.duration_ms
                )),
                details: serde_json::to_value(&record).unwrap_or_default(),
                source: "pipeline_engine".to_string(),
                created_at: None,
            };
            let _ = storage.record_change(&change);
        }
    }

    /// 查询最近的审计日志
    pub fn list_recent_audits(&self, limit: usize) -> Vec<HookAuditRecord> {
        let hist = self.audit_history.read().unwrap_or_else(|e| e.into_inner());
        hist.iter().rev().take(limit).cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pipeline_engine_full_lifecycle() {
        let pipeline = HookPipeline {
            id: "pipe-test-01".to_string(),
            name: "测试链式编排".to_string(),
            event_pattern: "on_file_created".to_string(),
            steps: vec![
                HookStep {
                    name: "step-1-echo".to_string(),
                    action: HookAction::Exec { command: "echo 'Step 1 success'".to_string() },
                    filter: None,
                    on_failure: OnFailureStrategy::Abort,
                    timeout_ms: 1000,
                },
                HookStep {
                    name: "step-2-echo".to_string(),
                    action: HookAction::Exec { command: "echo 'Step 2 success'".to_string() },
                    filter: None,
                    on_failure: OnFailureStrategy::Abort,
                    timeout_ms: 1000,
                },
            ],
            debounce_ms: 200,
        };

        let storage = Storage::open_in_memory().expect("open_in_memory");
        let engine = HookPipelineEngine::new(vec![pipeline], Some(storage.clone()));

        let event = Arc::new(StewardEvent::FileCreated {
            path: "/root/new/work/demo.rs".to_string(),
            source: "test".to_string(),
            timestamp: "2026-11-18T22:00:00Z".to_string(),
        });

        // 1. 首次触发执行全部步骤
        let audits = engine.execute(event.clone()).await;
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].status, PipelineExecutionStatus::Success);
        assert_eq!(audits[0].steps_executed, 2);

        // 2. 紧接着第二次触发命中防抖去重
        let audits_debounced = engine.execute(event).await;
        assert_eq!(audits_debounced.len(), 1);
        assert_eq!(audits_debounced[0].status, PipelineExecutionStatus::SkippedDebounced);

        // 3. 验证审计日志记录
        let recs = engine.list_recent_audits(5);
        assert_eq!(recs.len(), 2);

        // 4. 验证 Storage 持久化已同步
        let changes = storage.list_changes(5).unwrap();
        assert!(!changes.is_empty());
        assert_eq!(changes[0].change_type, "hook_pipeline:audit");
    }

    #[tokio::test]
    async fn test_pipeline_abort_on_step_failure() {
        let pipeline = HookPipeline {
            id: "pipe-test-abort".to_string(),
            name: "测试中止策略".to_string(),
            event_pattern: "*".to_string(),
            steps: vec![
                HookStep {
                    name: "step-fail".to_string(),
                    action: HookAction::Exec { command: "exit 1".to_string() },
                    filter: None,
                    on_failure: OnFailureStrategy::Abort,
                    timeout_ms: 1000,
                },
                HookStep {
                    name: "step-never-run".to_string(),
                    action: HookAction::Exec { command: "echo 'should not run'".to_string() },
                    filter: None,
                    on_failure: OnFailureStrategy::Continue,
                    timeout_ms: 1000,
                },
            ],
            debounce_ms: 0,
        };

        let engine = HookPipelineEngine::new(vec![pipeline], None);
        let event = Arc::new(StewardEvent::LockAcquired {
            resource_type: "port".to_string(),
            resource_id: "8080".to_string(),
            holder_agent: "agent-1".to_string(),
            ttl_seconds: 300,
            timestamp: "2026-11-18T22:00:00Z".to_string(),
        });

        let audits = engine.execute(event).await;
        assert_eq!(audits.len(), 1);
        assert_eq!(audits[0].status, PipelineExecutionStatus::Aborted);
        assert_eq!(audits[0].steps_executed, 1);
        assert!(audits[0].error_message.is_some());
    }
}
