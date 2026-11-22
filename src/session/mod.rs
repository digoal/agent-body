use std::fs;
use std::path::Path;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::storage::{Storage, models::MemoryRecord};
use crate::changes::ChangesEngine;

pub mod lifecycle;
pub use lifecycle::{SessionLifecycleManager, SessionState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCloseResult {
    pub session_id: String,
    pub cleaned_files_count: usize,
    pub freed_bytes: u64,
    pub cleaned_paths: Vec<String>,
    pub changes_summary: String,
    pub saved_memory_id: Option<String>,
    #[serde(default)]
    pub is_dry_run: bool,
    #[serde(default)]
    pub saved_checkpoint_id: Option<String>,
}

pub struct SessionManager {
    storage: Arc<Storage>,
}

impl SessionManager {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    /// 会话完工结算与环境治理 (默认直接物理删除临时文件)
    pub fn close_session(&self, session_id: &str, clean_temp: bool) -> Result<SessionCloseResult> {
        self.close_session_with_dry_run(session_id, clean_temp, false)
    }

    /// 支持 dry-run 保护的会话结算与治理 (规约 #8)
    pub fn close_session_with_dry_run(
        &self,
        session_id: &str,
        clean_temp: bool,
        dry_run: bool,
    ) -> Result<SessionCloseResult> {
        self.close_session_with_handoff(session_id, clean_temp, dry_run, None, None, None)
    }

    /// 支持 dry-run 与可选跨大脑接力胶囊沉淀的会话结算 (Phase 44)
    pub fn close_session_with_handoff(
        &self,
        session_id: &str,
        clean_temp: bool,
        dry_run: bool,
        handoff_goal: Option<&str>,
        from_brain: Option<&str>,
        next_step_hint: Option<&str>,
    ) -> Result<SessionCloseResult> {
        let mut cleaned_files_count = 0;
        let mut freed_bytes = 0;
        let mut cleaned_paths = Vec::new();

        if clean_temp {
            let tmp_dir = Path::new("/root/new/tmp");
            if tmp_dir.exists() && tmp_dir.is_dir() {
                if let Ok(entries) = fs::read_dir(tmp_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
                            // 匹配属于该 session 或测试遗留的临时文件
                            if file_name.contains(session_id) || file_name.starts_with("test_") || file_name.starts_with("tmp_") {
                                if let Ok(meta) = entry.metadata() {
                                    freed_bytes += meta.len();
                                }
                                cleaned_files_count += 1;
                                cleaned_paths.push(path.to_string_lossy().to_string());
                                if !dry_run {
                                    let _ = fs::remove_file(&path);
                                }
                            }
                        }
                    }
                }
            }
        }

        // 提取变更摘要
        let changes_engine = ChangesEngine::new(self.storage.clone());
        let changes_summary = changes_engine.generate_summary_markdown(None, 10)?;

        // 沉淀进长期记忆
        let mem_content = format!(
            "会话 [{}] 完工环境总结 (dry_run: {}):\n- 清理/待清理临时文件数: {}\n- 涉及空间: {} 字节\n- 环境变更:\n{}",
            session_id, dry_run, cleaned_files_count, freed_bytes, changes_summary
        );

        let mem_id = format!("mem-session-{}", uuid::Uuid::new_v4());
        let memory_rec = MemoryRecord {
            id: mem_id.clone(),
            memory_type: "task".to_string(),
            content: mem_content,
            summary: Some(format!("会话 [{}] 完工环境归档结算", session_id)),
            source_agent: Some("session_close".to_string()),
            source_session: Some(session_id.to_string()),
            tags: vec!["session".to_string(), "close".to_string(), session_id.to_string()],
            weight: 0.8,
            pinned: false,
            status: "active".to_string(),
            embedding: vec![],
        };

        let _ = self.storage.insert_memory(&memory_rec);

        let saved_checkpoint_id = if let Some(goal) = handoff_goal.filter(|s| !s.trim().is_empty()) {
            crate::body::ContinuityManager::save_capsule(
                &self.storage,
                crate::body::ContinuityInput {
                    from_brain: from_brain.unwrap_or("session_close").to_string(),
                    session_id: session_id.to_string(),
                    goal_summary: goal.to_string(),
                    active_files: cleaned_paths.iter().take(5).cloned().collect(),
                    active_locks: vec![],
                    verified_facts: vec![format!("会话 {} 已完成归档结算", session_id)],
                    open_blockers: vec![],
                    next_step_hint: next_step_hint.unwrap_or("").to_string(),
                    token_budget: Some(300),
                },
            )
            .ok()
            .map(|c| c.checkpoint_id)
        } else {
            None
        };

        crate::audit::AuditLogger::record_best_effort(
            &self.storage,
            crate::audit::AuditEventInput::in_agent("session", "session_close", "close_session")
                .with_session(session_id)
                .with_target("session", session_id)
                .with_outcome(if dry_run { "dry_run" } else { "success" })
                .with_delta(serde_json::json!({
                    "cleaned_files_count": cleaned_files_count,
                    "freed_bytes": freed_bytes,
                    "cleaned_paths": cleaned_paths,
                    "saved_memory_id": mem_id,
                    "saved_checkpoint_id": saved_checkpoint_id,
                })),
        );

        Ok(SessionCloseResult {
            session_id: session_id.to_string(),
            cleaned_files_count,
            freed_bytes,
            cleaned_paths,
            changes_summary,
            saved_memory_id: Some(mem_id),
            is_dry_run: dry_run,
            saved_checkpoint_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_close_and_archive() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let sm = SessionManager::new(storage.clone());

        // 测试会话结算与归档
        let res = sm.close_session("session-test-01", false).unwrap();
        assert_eq!(res.session_id, "session-test-01");
        assert!(res.saved_memory_id.is_some());

        // 检索沉淀的记忆
        let mems = storage.search_memories_fts("\"session-test-01\"", 5).unwrap();
        assert_eq!(mems.len(), 1);
        assert!(mems[0].content.contains("完工环境总结"));
    }
}
