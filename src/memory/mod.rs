use anyhow::Result;
use crate::storage::Storage;
use crate::storage::models::MemoryRecord;

pub mod hybrid;
pub mod lifecycle;
pub mod spi;
pub mod builtin_provider;
pub mod adapters;
pub mod intake;

pub use spi::{MemoryProvider, MemoryItem, MemoryFilter, SessionContext, EvictCriteria};
pub use builtin_provider::BuiltinMemoryProvider;
pub use adapters::{MemoryFactory, AdapterConfig};
pub use intake::{MemoryIntakeFilter, MemoryIntakeEvaluation};

pub struct MemoryManager {
    storage: Storage,
}

impl MemoryManager {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// 写入一条新记忆
    pub fn add(
        &self,
        memory_type: &str,
        content: &str,
        summary: Option<&str>,
        source_agent: Option<&str>,
        tags: Vec<String>,
    ) -> Result<String> {
        let filter = MemoryIntakeFilter::default();
        let eval = filter.evaluate(memory_type, content);
        let weight = if eval.accepted { eval.calculated_weight } else { 0.5 };

        let id = format!("mem-{}", uuid::Uuid::new_v4());
        let record = MemoryRecord {
            id: id.clone(),
            memory_type: memory_type.to_string(),
            content: content.to_string(),
            summary: summary.map(|s| s.to_string()),
            source_agent: source_agent.map(|s| s.to_string()),
            source_session: None,
            tags,
            weight,
            pinned: false,
            status: "active".to_string(),
            embedding: vec![],
        };

        self.storage.insert_memory(&record)?;
        Ok(id)
    }

    /// 智能门禁入库：经过重要性评估与噪声过滤后写入，返回 (Option<id>, 评估报告)
    pub fn add_filtered(
        &self,
        memory_type: &str,
        content: &str,
        summary: Option<&str>,
        source_agent: Option<&str>,
        tags: Vec<String>,
    ) -> Result<(Option<String>, MemoryIntakeEvaluation)> {
        let filter = MemoryIntakeFilter::default();
        let eval = filter.evaluate(memory_type, content);
        if !eval.accepted {
            return Ok((None, eval));
        }

        let id = format!("mem-{}", uuid::Uuid::new_v4());
        let record = MemoryRecord {
            id: id.clone(),
            memory_type: memory_type.to_string(),
            content: content.to_string(),
            summary: summary.map(|s| s.to_string()),
            source_agent: source_agent.map(|s| s.to_string()),
            source_session: None,
            tags,
            weight: eval.calculated_weight,
            pinned: false,
            status: "active".to_string(),
            embedding: vec![],
        };

        self.storage.insert_memory(&record)?;
        Ok((Some(id), eval))
    }

    /// 全文检索记忆
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        self.storage.search_memories_fts(query, limit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_add_and_search() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let mm = MemoryManager::new(storage);

        let id = mm.add(
            "decision",
            "选型决定使用 Rust 编写 Agent Host Steward 核心守护进程",
            Some("选用 Rust 做核心"),
            Some("agent:main"),
            vec!["rust".to_string(), "decision".to_string()],
        ).expect("add memory");

        assert!(!id.is_empty());

        let results = mm.search("Rust", 10).expect("search");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
    }

    #[test]
    fn test_memory_add_filtered_noise_rejection() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let mm = MemoryManager::new(storage);

        // 1. 无效口头禅应该被过滤拒绝
        let (id_rej, eval_rej) = mm.add_filtered("note", "好的", None, None, vec![]).expect("call");
        assert!(id_rej.is_none());
        assert!(!eval_rej.accepted);

        // 2. 有效架构决断应该被接收入库并给予高权重
        let (id_ok, eval_ok) = mm.add_filtered(
            "decision",
            "确定三位一体架构：宿主机事实图谱作为底层只读体系",
            None,
            Some("agent:main"),
            vec!["arch".to_string()],
        ).expect("call");
        assert!(id_ok.is_some());
        assert!(eval_ok.accepted);
        assert!(eval_ok.calculated_weight >= 0.85);
    }
}
