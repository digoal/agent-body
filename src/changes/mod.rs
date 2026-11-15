use std::sync::Arc;
use crate::storage::{Storage, models::ChangeRecord};
use anyhow::Result;

pub struct ChangesEngine {
    storage: Arc<Storage>,
}

impl ChangesEngine {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    /// 查询自某个时间戳 (ISO 8601，如 "2026-11-15T12:00:00") 以来的所有变更
    pub fn get_changes_since(&self, since: Option<&str>, limit: usize) -> Result<Vec<ChangeRecord>> {
        let all_changes = self.storage.list_changes(limit * 2)?;
        if let Some(since_ts) = since {
            let filtered = all_changes
                .into_iter()
                .filter(|c| {
                    if let Some(ref created_at) = c.created_at {
                        created_at.as_str() >= since_ts
                    } else {
                        false
                    }
                })
                .take(limit)
                .collect();
            Ok(filtered)
        } else {
            Ok(all_changes.into_iter().take(limit).collect())
        }
    }

    /// 为 Agent 生成开工环境简报 (Markdown 格式)
    pub fn generate_summary_markdown(&self, since: Option<&str>, limit: usize) -> Result<String> {
        let changes = self.get_changes_since(since, limit)?;
        let mut md = String::new();

        md.push_str("### 宿主环境变更回溯 (Changes Summary)\n\n");
        if let Some(since_ts) = since {
            md.push_str(&format!("> 统计起始点: `{}` | 变更记录数: `{}`\n\n", since_ts, changes.len()));
        } else {
            md.push_str(&format!("> 最近记录数: `{}`\n\n", changes.len()));
        }

        if changes.is_empty() {
            md.push_str("*(期间环境平稳，未检测到任何资产或服务变动)*\n");
            return Ok(md);
        }

        md.push_str("| 时间 | 类型 | 目标对象 | 来源 | 简要说明 |\n");
        md.push_str("|---|---|---|---|---|\n");

        for c in changes {
            let ts = c.created_at.as_deref().unwrap_or("-");
            let ctype = &c.change_type;
            let target = &c.target;
            let src = &c.source;
            let desc = c.description.as_deref().unwrap_or("-");
            md.push_str(&format!("| {} | `{}` | `{}` | `{}` | {} |\n", ts, ctype, target, src, desc));
        }

        Ok(md)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_changes_engine_summary() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let engine = ChangesEngine::new(storage.clone());

        storage.record_change(&ChangeRecord {
            id: None,
            change_type: "file_created".to_string(),
            target: "/root/new/work/demo.rs".to_string(),
            description: Some("新建演示文件".to_string()),
            details: serde_json::json!({}),
            source: "watcher".to_string(),
            created_at: Some("2026-11-15T14:00:00+08:00".to_string()),
        }).unwrap();

        let md = engine.generate_summary_markdown(None, 10).unwrap();
        assert!(md.contains("宿主环境变更回溯"));
        assert!(md.contains("/root/new/work/demo.rs"));
        assert!(md.contains("file_created"));
    }
}
