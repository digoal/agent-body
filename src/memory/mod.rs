use anyhow::Result;
use crate::storage::Storage;
use crate::storage::models::MemoryRecord;

pub mod hybrid;

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
        let id = format!("mem-{}", uuid::Uuid::new_v4());
        let record = MemoryRecord {
            id: id.clone(),
            memory_type: memory_type.to_string(),
            content: content.to_string(),
            summary: summary.map(|s| s.to_string()),
            source_agent: source_agent.map(|s| s.to_string()),
            source_session: None,
            tags,
            weight: 1.0,
            pinned: false,
            status: "active".to_string(),
            embedding: vec![],
        };

        self.storage.insert_memory(&record)?;
        Ok(id)
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
}
