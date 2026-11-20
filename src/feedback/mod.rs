use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::memory::MemoryManager;
use crate::storage::Storage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackItem {
    pub id: String,
    pub category: String,
    pub priority: String,
    pub title: String,
    pub status: String,
    pub summary: String,
}

pub struct FeedbackEngine;

impl FeedbackEngine {
    /// 定位当前工作区或宿主规范下的 user_feedback.md
    pub fn default_feedback_path() -> PathBuf {
        let cwd_path = PathBuf::from("user_feedback.md");
        if cwd_path.exists() {
            return cwd_path;
        }
        let repo_path = PathBuf::from("/root/new/work/agent-steward/user_feedback.md");
        if repo_path.exists() {
            return repo_path;
        }
        cwd_path
    }

    /// 解析 user_feedback.md 中的 PRO / CON / BENCH / REQ 条目
    pub fn list_items(path: &Path) -> Result<Vec<FeedbackItem>> {
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(path)?;
        let mut items = Vec::new();
        let mut current: Option<FeedbackItem> = None;

        for line in content.lines() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("### `") {
                if let Some(prev) = current.take() {
                    items.push(prev);
                }
                if let Some(end_id) = rest.find('`') {
                    let id = rest[..end_id].trim().to_string();
                    let after_id = rest[end_id + 1..].trim();
                    let category = id.split('-').next().unwrap_or("OTHER").to_string();
                    let (priority, title) = parse_priority_and_title(after_id);
                    current = Some(FeedbackItem {
                        id,
                        category,
                        priority,
                        title,
                        status: "📥 Open".to_string(),
                        summary: String::new(),
                    });
                }
            } else if let Some(ref mut item) = current {
                if trimmed.starts_with("- **状态**:") {
                    if let Some(s_start) = trimmed.find('`') {
                        if let Some(s_end) = trimmed[s_start + 1..].find('`') {
                            item.status = trimmed[s_start + 1..s_start + 1 + s_end].to_string();
                        }
                    }
                } else if !trimmed.is_empty()
                    && !trimmed.starts_with("---")
                    && item.summary.len() < 240
                {
                    if !item.summary.is_empty() {
                        item.summary.push(' ');
                    }
                    item.summary.push_str(trimmed.trim_start_matches("- "));
                }
            }
        }
        if let Some(last) = current.take() {
            items.push(last);
        }
        Ok(items)
    }

    /// 结构化追加一条新反馈到 user_feedback.md（自动分配递增编号）
    pub fn add_item(
        path: &Path,
        category: &str,
        priority: &str,
        title: &str,
        evidence: &str,
        target: &str,
    ) -> Result<FeedbackItem> {
        let cat_upper = category.trim().to_uppercase();
        if !matches!(cat_upper.as_str(), "PRO" | "CON" | "BENCH" | "REQ") {
            bail!("不支持的反馈类别 '{}'（仅支持 PRO / CON / BENCH / REQ）", category);
        }
        if title.trim().is_empty() {
            bail!("反馈标题不能为空");
        }

        let existing = Self::list_items(path)?;
        let next_num = existing
            .iter()
            .filter(|it| it.category == cat_upper)
            .filter_map(|it| it.id.split('-').nth(1)?.parse::<u32>().ok())
            .max()
            .unwrap_or(0)
            + 1;
        let id = format!("{}-{:02}", cat_upper, next_num);
        let prio = if priority.trim().is_empty() {
            "P1"
        } else {
            priority.trim()
        };
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let status = if cat_upper == "PRO" {
            "✅ Verified"
        } else {
            "📥 Open"
        };

        let block = format!(
            "\n### `{id}` `[{prio} · {cat_upper}]` {title}\n\
             - **状态**: `{status}` · **记录日期**: {today}\n\
             - **现象与复现证据**: {evidence}\n\
             - **预期目标与 Benchmark 联动**: {target}\n",
            id = id,
            prio = prio,
            cat_upper = cat_upper,
            title = title.trim(),
            status = status,
            today = today,
            evidence = if evidence.trim().is_empty() {
                "来自实时交互反馈"
            } else {
                evidence.trim()
            },
            target = if target.trim().is_empty() {
                "待评估排期并纳入回归测试"
            } else {
                target.trim()
            },
        );

        let mut content = if path.exists() {
            fs::read_to_string(path)?
        } else {
            "# Agent Host Steward — 实际使用反馈总账 (`user_feedback.md`)\n\n".to_string()
        };

        let marker = "## 七、第五部分：后续新增反馈标准模板";
        if let Some(pos) = content.find(marker) {
            content.insert_str(pos, &format!("{}\n---\n\n", block));
        } else {
            content.push_str(&block);
        }
        fs::write(path, content)?;

        Ok(FeedbackItem {
            id,
            category: cat_upper,
            priority: prio.to_string(),
            title: title.trim().to_string(),
            status: status.to_string(),
            summary: evidence.trim().to_string(),
        })
    }

    /// 将 user_feedback.md 中已验证（✅ Verified）的条目沉淀入长期记忆库（去重幂等）
    pub fn sync_verified_to_memories(path: &Path, storage: &Storage) -> Result<usize> {
        let items = Self::list_items(path)?;
        let mm = MemoryManager::new(storage.clone());
        let existing_all = storage.list_all_active_memories(1000).unwrap_or_default();
        let mut added = 0usize;

        for item in items {
            if !item.status.contains("Verified") {
                continue;
            }
            let marker = format!("[feedback:{}]", item.id);
            if existing_all.iter().any(|m| m.content.contains(&marker)) {
                continue;
            }
            let content = format!(
                "{} {} ({}) - {}",
                marker,
                item.title,
                item.status,
                item.summary.chars().take(180).collect::<String>()
            );
            let _ = mm.add(
                "experience",
                &content,
                Some(&format!("使用反馈沉淀 {}", item.id)),
                Some("user_feedback.md"),
                vec!["feedback".to_string(), item.id.clone()],
            )?;
            added += 1;
        }
        Ok(added)
    }
}

fn parse_priority_and_title(raw: &str) -> (String, String) {
    let trimmed = raw.trim();
    if let Some(after_bt) = trimmed.strip_prefix('`') {
        if let Some(end_bt) = after_bt.find('`') {
            let tag = after_bt[..end_bt].trim().to_string();
            let title = after_bt[end_bt + 1..].trim().to_string();
            return (tag, title);
        }
    }
    ("P1".to_string(), trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_add_list_and_sync_memories() {
        let dir = tempfile::tempdir().expect("tempdir");
        let md = dir.path().join("user_feedback.md");
        fs::write(
            &md,
            "# 反馈总账\n\n### `PRO-01` `[P1]` 检索极快\n- **状态**: `✅ Verified`\n- **实测表现**: P50 < 300us\n\n## 七、第五部分：后续新增反馈标准模板\n",
        )
        .expect("write");

        let added = FeedbackEngine::add_item(
            &md,
            "CON",
            "P0",
            "冷启动慢",
            "W8 耗时 474ms",
            "优化到 < 10ms",
        )
        .expect("add CON");
        assert_eq!(added.id, "CON-01");

        let list = FeedbackEngine::list_items(&md).expect("list");
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "PRO-01");
        assert_eq!(list[1].id, "CON-01");

        let storage = Storage::open_in_memory().expect("sqlite");
        let synced1 = FeedbackEngine::sync_verified_to_memories(&md, &storage).expect("sync 1");
        assert_eq!(synced1, 1);
        let synced2 = FeedbackEngine::sync_verified_to_memories(&md, &storage).expect("sync 2");
        assert_eq!(synced2, 0, "重复同步应幂等去重");
    }
}
