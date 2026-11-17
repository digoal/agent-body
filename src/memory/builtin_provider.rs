use anyhow::Result;
use std::collections::HashMap;
use crate::storage::Storage;
use crate::storage::models::MemoryRecord;
use crate::memory::spi::{MemoryProvider, MemoryItem, MemoryFilter, SessionContext, EvictCriteria};

/// 基于本地内嵌 SQLite FTS5 的开箱即用轻量记忆 Provider
pub struct BuiltinMemoryProvider {
    storage: Storage,
}

impl BuiltinMemoryProvider {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// 将内部底层 MemoryRecord 转化为通用的 MemoryItem
    fn record_to_item(rec: MemoryRecord, score: Option<f64>) -> MemoryItem {
        let mut extra = HashMap::new();
        extra.insert("weight".to_string(), rec.weight.to_string());
        extra.insert("pinned".to_string(), rec.pinned.to_string());
        extra.insert("status".to_string(), rec.status.clone());

        MemoryItem {
            id: rec.id,
            memory_type: rec.memory_type,
            content: rec.content,
            summary: rec.summary,
            tags: rec.tags,
            source_agent: rec.source_agent,
            source_session: rec.source_session,
            created_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            score,
            extra,
        }
    }

    /// 将通用 MemoryItem 转换为内部持久化使用的 MemoryRecord
    fn item_to_record(item: &MemoryItem) -> MemoryRecord {
        let weight = item.extra.get("weight")
            .and_then(|w| w.parse::<f32>().ok())
            .unwrap_or(1.0f32);
        let pinned = item.extra.get("pinned")
            .and_then(|p| p.parse::<bool>().ok())
            .unwrap_or(false);
        let status = item.extra.get("status")
            .cloned()
            .unwrap_or_else(|| "active".to_string());

        MemoryRecord {
            id: if item.id.is_empty() {
                format!("mem-{}", uuid::Uuid::new_v4())
            } else {
                item.id.clone()
            },
            memory_type: item.memory_type.clone(),
            content: item.content.clone(),
            summary: item.summary.clone(),
            source_agent: item.source_agent.clone(),
            source_session: item.source_session.clone(),
            tags: item.tags.clone(),
            weight,
            pinned,
            status,
            embedding: vec![],
        }
    }
}

impl MemoryProvider for BuiltinMemoryProvider {
    fn provider_name(&self) -> &'static str {
        "builtin"
    }

    fn search(
        &self,
        query: &str,
        limit: usize,
        filter: Option<&MemoryFilter>,
    ) -> Result<Vec<MemoryItem>> {
        let q_trimmed = query.trim();
        let records = if q_trimmed.is_empty() {
            self.storage.list_all_active_memories(limit * 2)?
        } else {
            self.storage.search_memories_fts(q_trimmed, limit * 2)?
        };
        let mut items = Vec::new();

        for rec in records {
            // 过滤状态
            if rec.status == "archived" || rec.status == "deleted" {
                continue;
            }

            // 应用过滤条件
            if let Some(f) = filter {
                if let Some(ref req_type) = f.memory_type {
                    if &rec.memory_type != req_type {
                        continue;
                    }
                }
                if let Some(ref req_sess) = f.source_session {
                    if rec.source_session.as_deref() != Some(req_sess.as_str()) {
                        continue;
                    }
                }
                if !f.tags.is_empty() {
                    let has_any_tag = f.tags.iter().any(|t| rec.tags.contains(t));
                    if !has_any_tag {
                        continue;
                    }
                }
            }

            // 构造通用实体 (默认打分 1.0)
            items.push(Self::record_to_item(rec, Some(1.0)));

            if items.len() >= limit {
                break;
            }
        }

        Ok(items)
    }

    fn store(&self, item: &MemoryItem) -> Result<String> {
        let record = Self::item_to_record(item);
        let id = record.id.clone();
        self.storage.insert_memory(&record)?;
        Ok(id)
    }

    fn get_context(&self, session_id: &str) -> Result<SessionContext> {
        // 全量搜索该会话相关的记录 (空 query 或通过 session 过滤)
        let filter = MemoryFilter {
            source_session: Some(session_id.to_string()),
            ..Default::default()
        };

        // 查询该会话相关条目
        let all_memories = self.search("", 100, Some(&filter)).unwrap_or_default();

        let mut key_decisions = Vec::new();
        let mut active_tasks = Vec::new();
        let mut relevant_experiences = Vec::new();

        for m in all_memories {
            match m.memory_type.as_str() {
                "decision" => key_decisions.push(m),
                "task" => active_tasks.push(m),
                "experience" => relevant_experiences.push(m),
                _ => {}
            }
        }

        Ok(SessionContext {
            session_id: session_id.to_string(),
            key_decisions,
            active_tasks,
            relevant_experiences,
        })
    }

    fn evict(&self, criteria: &EvictCriteria) -> Result<usize> {
        // 利用内嵌 lifecycle 管理器审计并归档
        let lifecycle = crate::memory::lifecycle::MemoryLifecycleManager::new(&self.storage);
        let audit = lifecycle.audit_memories()?;
        let mut count = 0;

        for cand in audit.decay_candidates {
            if criteria.archive_instead_of_delete {
                if lifecycle.archive_memory(&cand.memory_id).is_ok() {
                    count += 1;
                }
            }
        }

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_memory_provider() -> Result<()> {
        let storage = Storage::open_in_memory()?;
        let provider = BuiltinMemoryProvider::new(storage);

        assert_eq!(provider.provider_name(), "builtin");

        // 1. 存储记忆
        let item = MemoryItem {
            id: "mem-test-1".to_string(),
            memory_type: "decision".to_string(),
            content: "经测试验证，PostgreSQL 采用 shared_buffers=8GB 配合 huge_pages 获得最高 TPS".to_string(),
            summary: Some("PG 内存优化决策".to_string()),
            tags: vec!["postgresql".to_string(), "tuning".to_string()],
            source_agent: Some("agent-steward".to_string()),
            source_session: Some("sess-20261117".to_string()),
            created_at: "2026-11-17 18:00:00".to_string(),
            score: None,
            extra: HashMap::new(),
        };

        let id = provider.store(&item)?;
        assert_eq!(id, "mem-test-1");

        // 2. 全文检索
        let results = provider.search("shared_buffers", 5, None)?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "mem-test-1");
        assert_eq!(results[0].memory_type, "decision");

        // 3. 过滤检索
        let filter = MemoryFilter {
            memory_type: Some("task".to_string()),
            ..Default::default()
        };
        let empty_results = provider.search("shared_buffers", 5, Some(&filter))?;
        assert_eq!(empty_results.len(), 0);

        // 4. 会话上下文
        let ctx = provider.get_context("sess-20261117")?;
        assert_eq!(ctx.key_decisions.len(), 1);

        Ok(())
    }
}
