use std::fs;
use std::path::Path;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::storage::{Storage, models::MemoryRecord};
use crate::changes::ChangesEngine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCloseResult {
    pub session_id: String,
    pub cleaned_files_count: usize,
    pub freed_bytes: u64,
    pub cleaned_paths: Vec<String>,
    pub changes_summary: String,
    pub saved_memory_id: Option<String>,
}

pub struct SessionManager {
    storage: Arc<Storage>,
}

impl SessionManager {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    /// 会话完工结算与环境治理：
    /// 1. 扫描并安全清理 /root/new/tmp/ 下与该会话关联或孤儿遗留的临时测试脚本
    /// 2. 统计本次会话的 changes 变更摘要
    /// 3. 将会话成果环境摘要自动持久化沉淀到长期记忆 (memories)
    pub fn close_session(&self, session_id: &str, clean_temp: bool) -> Result<SessionCloseResult> {
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
                            // 匹配属于该 session 或测试遗留的临时文件 (例如含 session_id 或 test/tmp 前缀)
                            if file_name.contains(session_id) || file_name.starts_with("test_") || file_name.starts_with("tmp_") {
                                if let Ok(meta) = entry.metadata() {
                                    freed_bytes += meta.len();
                                }
                                if fs::remove_file(&path).is_ok() {
                                    cleaned_files_count += 1;
                                    cleaned_paths.push(path.to_string_lossy().to_string());
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
            "会话 [{}] 完工环境总结:\n- 清理临时文件数: {}\n- 释放空间: {} 字节\n- 环境变更:\n{}",
            session_id, cleaned_files_count, freed_bytes, changes_summary
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

        Ok(SessionCloseResult {
            session_id: session_id.to_string(),
            cleaned_files_count,
            freed_bytes,
            cleaned_paths,
            changes_summary,
            saved_memory_id: Some(mem_id),
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
