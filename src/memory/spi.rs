use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 跨生态通用的长期记忆实体 (Universal Memory Item)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryItem {
    /// 唯一标识 (如 mem-xxx 或 mem0-xxx)
    pub id: String,
    /// 类别 ("decision", "experience", "task", "fact", "preference", "note")
    pub memory_type: String,
    /// 记忆核心正文
    pub content: String,
    /// 一句话摘要（快速呈现）
    pub summary: Option<String>,
    /// 语义标签集合
    pub tags: Vec<String>,
    /// 来源元数据
    pub source_agent: Option<String>,
    pub source_session: Option<String>,
    pub created_at: String,
    /// 检索相关度打分 (0.0 ~ 1.0)
    pub score: Option<f64>,
    /// 扩展键值字典（用于承载 Mem0 / Letta 等特定引擎的元数据）
    #[serde(default)]
    pub extra: HashMap<String, String>,
}

/// 多维度记忆检索过滤条件
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MemoryFilter {
    pub memory_type: Option<String>,
    pub tags: Vec<String>,
    pub source_session: Option<String>,
    pub min_score: Option<f64>,
}

/// 会话情境上下文画像 (Session Context Snapshot)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub session_id: String,
    pub key_decisions: Vec<MemoryItem>,
    pub active_tasks: Vec<MemoryItem>,
    pub relevant_experiences: Vec<MemoryItem>,
}

/// 记忆生命周期淘汰标准
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvictCriteria {
    pub older_than_days: Option<u32>,
    pub min_weight: Option<f64>,
    pub archive_instead_of_delete: bool,
}

/// 通用可插拔记忆服务提供者接口 (Service Provider Interface)
pub trait MemoryProvider: Send + Sync {
    /// 记忆引擎唯一标识名 (如 "builtin", "mem0", "letta", "powercontext")
    fn provider_name(&self) -> &'static str;

    /// 跨会话搜索过往决策、经验与事实 (关键词/语义混合)
    fn search(
        &self,
        query: &str,
        limit: usize,
        filter: Option<&MemoryFilter>,
    ) -> Result<Vec<MemoryItem>>;

    /// 持久化沉淀一条新记忆或关键决策
    fn store(&self, item: &MemoryItem) -> Result<String>;

    /// 一键获取指定会话相关的历史决策与经验画像
    fn get_context(&self, session_id: &str) -> Result<SessionContext>;

    /// 老化淘汰、归档低价值过期噪音记忆
    fn evict(&self, criteria: &EvictCriteria) -> Result<usize>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockMemoryProvider {
        items: std::sync::Mutex<Vec<MemoryItem>>,
    }

    impl MockMemoryProvider {
        fn new() -> Self {
            Self {
                items: std::sync::Mutex::new(Vec::new()),
            }
        }
    }

    impl MemoryProvider for MockMemoryProvider {
        fn provider_name(&self) -> &'static str {
            "mock"
        }

        fn search(
            &self,
            query: &str,
            limit: usize,
            _filter: Option<&MemoryFilter>,
        ) -> Result<Vec<MemoryItem>> {
            let lock = self.items.lock().unwrap();
            let q = query.to_lowercase();
            let matched: Vec<MemoryItem> = lock
                .iter()
                .filter(|m| m.content.to_lowercase().contains(&q) || m.tags.iter().any(|t| t.to_lowercase().contains(&q)))
                .take(limit)
                .cloned()
                .collect();
            Ok(matched)
        }

        fn store(&self, item: &MemoryItem) -> Result<String> {
            let mut lock = self.items.lock().unwrap();
            lock.push(item.clone());
            Ok(item.id.clone())
        }

        fn get_context(&self, session_id: &str) -> Result<SessionContext> {
            let lock = self.items.lock().unwrap();
            let sess_items: Vec<MemoryItem> = lock
                .iter()
                .filter(|m| m.source_session.as_deref() == Some(session_id))
                .cloned()
                .collect();

            Ok(SessionContext {
                session_id: session_id.to_string(),
                key_decisions: sess_items.clone(),
                active_tasks: vec![],
                relevant_experiences: sess_items,
            })
        }

        fn evict(&self, _criteria: &EvictCriteria) -> Result<usize> {
            let mut lock = self.items.lock().unwrap();
            let len = lock.len();
            lock.clear();
            Ok(len)
        }
    }

    #[test]
    fn test_mock_memory_provider_spi_contract() -> Result<()> {
        let provider: Box<dyn MemoryProvider> = Box::new(MockMemoryProvider::new());
        assert_eq!(provider.provider_name(), "mock");

        let item = MemoryItem {
            id: "mem-1".to_string(),
            memory_type: "decision".to_string(),
            content: "决定使用 MemoryProvider SPI 实现多记忆生态插件化".to_string(),
            summary: Some("SPI 特征设计".to_string()),
            tags: vec!["spi".to_string(), "memory".to_string()],
            source_agent: Some("agent-steward".to_string()),
            source_session: Some("sess-001".to_string()),
            created_at: "2026-11-17 18:00:00".to_string(),
            score: Some(1.0),
            extra: HashMap::new(),
        };

        let stored_id = provider.store(&item)?;
        assert_eq!(stored_id, "mem-1");

        let search_res = provider.search("SPI", 5, None)?;
        assert_eq!(search_res.len(), 1);
        assert_eq!(search_res[0].id, "mem-1");

        let ctx = provider.get_context("sess-001")?;
        assert_eq!(ctx.key_decisions.len(), 1);

        let evicted = provider.evict(&EvictCriteria {
            older_than_days: Some(30),
            min_weight: None,
            archive_instead_of_delete: false,
        })?;
        assert_eq!(evicted, 1);

        Ok(())
    }
}
