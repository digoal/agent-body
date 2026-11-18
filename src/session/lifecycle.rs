use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Local};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::storage::Storage;
use crate::session::{SessionManager, SessionCloseResult};

/// 会话动态状态追踪记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub session_id: String,
    pub created_at: String,
    pub last_active_at: String,
    pub tool_call_count: usize,
    pub artifacts_produced: Vec<String>,
    pub status: String, // "active", "closed"
}

/// 会话生命周期自动绑定管理器 (Phase 14.4)
pub struct SessionLifecycleManager {
    sessions: Arc<RwLock<HashMap<String, SessionState>>>,
    session_manager: Arc<SessionManager>,
    #[allow(dead_code)]
    storage: Arc<Storage>,
}

impl SessionLifecycleManager {
    pub fn new(storage: Arc<Storage>) -> Self {
        let session_manager = Arc::new(SessionManager::new(storage.clone()));
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_manager,
            storage,
        }
    }

    /// 会话交互触发 (Touch)：首次调用自动注册，已存在则刷新心跳与产出记录
    pub fn touch_session(
        &self,
        session_id: &str,
        _tool_name: Option<&str>,
        artifact_path: Option<&str>,
    ) -> SessionState {
        let now = Local::now().to_rfc3339();
        let mut map = self.sessions.write().unwrap();

        if let Some(state) = map.get_mut(session_id) {
            state.last_active_at = now;
            state.tool_call_count += 1;
            state.status = "active".to_string();
            if let Some(path) = artifact_path {
                if !state.artifacts_produced.contains(&path.to_string()) {
                    state.artifacts_produced.push(path.to_string());
                }
            }
            state.clone()
        } else {
            let mut artifacts = Vec::new();
            if let Some(path) = artifact_path {
                artifacts.push(path.to_string());
            }
            let new_state = SessionState {
                session_id: session_id.to_string(),
                created_at: now.clone(),
                last_active_at: now,
                tool_call_count: 1,
                artifacts_produced: artifacts,
                status: "active".to_string(),
            };
            map.insert(session_id.to_string(), new_state.clone());
            new_state
        }
    }

    /// 查询指定 Session 状态
    pub fn get_session(&self, session_id: &str) -> Option<SessionState> {
        let map = self.sessions.read().unwrap();
        map.get(session_id).cloned()
    }

    /// 获取所有活跃中的 Session
    pub fn list_active_sessions(&self) -> Vec<SessionState> {
        let map = self.sessions.read().unwrap();
        map.values()
            .filter(|s| s.status == "active")
            .cloned()
            .collect()
    }

    /// 手动结算关闭指定会话
    pub fn close_session(
        &self,
        session_id: &str,
        clean_temp: bool,
        dry_run: bool,
    ) -> Result<SessionCloseResult> {
        {
            let mut map = self.sessions.write().unwrap();
            if let Some(state) = map.get_mut(session_id) {
                state.status = "closed".to_string();
                state.last_active_at = Local::now().to_rfc3339();
            }
        }
        self.session_manager
            .close_session_with_dry_run(session_id, clean_temp, dry_run)
    }

    /// 扫描超时会话并自动触发安全归档结清 (规约 #8 默认 dry-run 保护)
    pub fn check_timeout_sessions(
        &self,
        timeout_secs: u64,
        dry_run: bool,
    ) -> Vec<SessionCloseResult> {
        let now = Local::now();
        let timeout_candidates: Vec<String> = {
            let map = self.sessions.read().unwrap();
            map.values()
                .filter(|s| s.status == "active")
                .filter_map(|s| {
                    if let Ok(last_time) = DateTime::parse_from_rfc3339(&s.last_active_at) {
                        let duration = now.signed_duration_since(last_time);
                        if duration.num_seconds() >= timeout_secs as i64 {
                            return Some(s.session_id.clone());
                        }
                    }
                    None
                })
                .collect()
        };

        let mut results = Vec::new();
        for sid in timeout_candidates {
            if let Ok(res) = self.close_session(&sid, true, dry_run) {
                results.push(res);
            }
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_touch_and_auto_registration() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let lifecycle = SessionLifecycleManager::new(storage);

        // 1. 首次 touch 自动建档
        let s1 = lifecycle.touch_session("session-alpha", Some("write_file"), Some("/root/new/work/demo.rs"));
        assert_eq!(s1.session_id, "session-alpha");
        assert_eq!(s1.tool_call_count, 1);
        assert_eq!(s1.artifacts_produced.len(), 1);
        assert_eq!(s1.status, "active");

        // 2. 二次 touch 累加与去重
        let s2 = lifecycle.touch_session("session-alpha", Some("edit_file"), Some("/root/new/work/demo.rs"));
        assert_eq!(s2.tool_call_count, 2);
        assert_eq!(s2.artifacts_produced.len(), 1); // 路径去重

        let s3 = lifecycle.touch_session("session-alpha", Some("write_file"), Some("/root/new/work/another.rs"));
        assert_eq!(s3.artifacts_produced.len(), 2);

        // 3. 列出活跃会话
        let actives = lifecycle.list_active_sessions();
        assert_eq!(actives.len(), 1);
        assert_eq!(actives[0].session_id, "session-alpha");
    }

    #[test]
    fn test_lifecycle_timeout_auto_close_dry_run_protection() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let lifecycle = SessionLifecycleManager::new(storage);

        lifecycle.touch_session("session-beta", None, None);

        // 设置超短超时 (0 秒) 模拟超时触发
        let closed = lifecycle.check_timeout_sessions(0, true);
        assert_eq!(closed.len(), 1);
        assert_eq!(closed[0].session_id, "session-beta");
        assert!(closed[0].is_dry_run, "自动回收必须遵循规约 #8 默认 dry-run 保护");

        // 验证状态已变为 closed
        let s = lifecycle.get_session("session-beta").unwrap();
        assert_eq!(s.status, "closed");
        assert!(lifecycle.list_active_sessions().is_empty());
    }
}
