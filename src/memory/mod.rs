use anyhow::Result;
use crate::storage::Storage;
use crate::storage::models::MemoryRecord;

pub mod hybrid;
pub mod lifecycle;
pub mod spi;
pub mod builtin_provider;
pub mod adapters;
pub mod intake;
pub mod embedding;

pub use spi::{
    EvictCriteria, FastWeightCheckpointSpi, FastWeightSnapshotMeta, MemoryFilter, MemoryItem,
    MemoryProvider, SessionContext, TxEvidenceTier, TxMemoryEvent, TxMemoryProvider,
    TxMemorySnapshot, TxMemoryWriteInput, TxProviderHealth, TxSearchOptions, TxWriteOutcome,
};
pub use builtin_provider::BuiltinMemoryProvider;
pub use adapters::{AdapterConfig, MemoryFactory, PgTxAdapterConfig, PgTxMemoryAdapter};
pub use intake::{MemoryIntakeFilter, MemoryIntakeEvaluation};
pub use embedding::{EmbeddingClient, inject_embedding_meta, inject_pending_embedding_meta};

pub struct MemoryManager {
    storage: Storage,
}

impl MemoryManager {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// 规范化记忆类型，兼容常见语义别名（如 troubleshooting -> experience），拒绝非法值避免触碰数据库 CHECK 约束
    pub fn normalize_memory_type(raw: &str) -> Result<&'static str> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "decision" => Ok("decision"),
            "task" | "todo" | "plan" => Ok("task"),
            "experience" | "troubleshooting" | "debug" | "lesson" | "incident" | "bugfix" => {
                Ok("experience")
            }
            "preference" | "style" | "rule" => Ok("preference"),
            "fact" | "knowledge" => Ok("fact"),
            "note" => Ok("note"),
            other => anyhow::bail!(
                "不支持的记忆类型 '{}'，仅支持: decision, task, experience, preference, fact, note",
                other
            ),
        }
    }

    /// 写入一条新记忆（入库前强制经 SecretRedactor 全链路脱敏）
    pub fn add(
        &self,
        memory_type: &str,
        content: &str,
        summary: Option<&str>,
        source_agent: Option<&str>,
        tags: Vec<String>,
    ) -> Result<String> {
        let canonical_type = Self::normalize_memory_type(memory_type)?;
        let sanitized_content = crate::scanner::metadata::SecretRedactor::redact_preserving_newlines(content);
        let sanitized_summary = summary.map(crate::scanner::metadata::SecretRedactor::redact_single_line);
        let filter = MemoryIntakeFilter::default();
        let eval = filter.evaluate(canonical_type, &sanitized_content);
        let weight = if eval.accepted { eval.calculated_weight } else { 0.5 };
        self.insert_sanitized_record(
            canonical_type,
            sanitized_content,
            sanitized_summary,
            source_agent,
            tags,
            weight,
        )
    }

    /// 智能门禁入库：经过脱敏、重要性评估与噪声过滤后写入，返回 (Option<id>, 评估报告)
    pub fn add_filtered(
        &self,
        memory_type: &str,
        content: &str,
        summary: Option<&str>,
        source_agent: Option<&str>,
        tags: Vec<String>,
    ) -> Result<(Option<String>, MemoryIntakeEvaluation)> {
        let canonical_type = Self::normalize_memory_type(memory_type)?;
        let sanitized_content = crate::scanner::metadata::SecretRedactor::redact_preserving_newlines(content);
        let sanitized_summary = summary.map(crate::scanner::metadata::SecretRedactor::redact_single_line);
        let filter = MemoryIntakeFilter::default();
        let eval = filter.evaluate(canonical_type, &sanitized_content);
        if !eval.accepted {
            return Ok((None, eval));
        }

        let id = self.insert_sanitized_record(
            canonical_type,
            sanitized_content,
            sanitized_summary,
            source_agent,
            tags,
            eval.calculated_weight,
        )?;
        Ok((Some(id), eval))
    }

    fn insert_sanitized_record(
        &self,
        memory_type: &str,
        content: String,
        summary: Option<String>,
        source_agent: Option<&str>,
        tags: Vec<String>,
        weight: f32,
    ) -> Result<String> {
        let id = format!("mem-{}", uuid::Uuid::new_v4());
        let emb = Self::try_embed_memory(&content, summary.as_deref());
        let record = MemoryRecord {
            id: id.clone(),
            memory_type: memory_type.to_string(),
            content,
            summary,
            source_agent: source_agent.map(|s| s.to_string()),
            source_session: None,
            tags,
            weight,
            pinned: false,
            status: "active".to_string(),
            embedding: emb,
        };

        self.storage.insert_memory(&record)?;
        Ok(id)
    }

    /// 全文检索记忆
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        self.storage.search_memories_fts(query, limit)
    }

    fn try_embed_memory(content: &str, summary: Option<&str>) -> Vec<f32> {
        if cfg!(test) {
            return Vec::new();
        }
        let client = EmbeddingClient::default();
        if !client.is_available() {
            return Vec::new();
        }
        let text = match summary {
            Some(s) if !s.trim().is_empty() => format!("{} — {}", s.trim(), content.trim()),
            _ => content.trim().to_string(),
        };
        client.embed_text(&text).unwrap_or_default()
    }

    /// 批量补全历史记忆缺失的 Embedding 向量
    pub fn backfill_missing_embeddings(&self, limit: usize) -> Result<usize> {
        let client = EmbeddingClient::default();
        if !client.is_available() {
            return Ok(0);
        }
        let all = self.storage.list_all_active_memories(limit.max(100))?;
        let mut updated = 0usize;
        for mut m in all {
            if !m.embedding.is_empty() {
                continue;
            }
            let text = match m.summary.as_deref() {
                Some(s) if !s.trim().is_empty() => format!("{} — {}", s.trim(), m.content.trim()),
                _ => m.content.trim().to_string(),
            };
            if let Some(vec) = client.embed_text(&text) {
                m.embedding = vec;
                self.storage.insert_memory(&m)?;
                updated += 1;
                if updated >= limit {
                    break;
                }
            }
        }
        Ok(updated)
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
            "选型决定使用 Rust 编写 Agent Body 核心守护进程",
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

    #[test]
    fn test_memory_add_redacts_secrets() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let mm = MemoryManager::new(storage.clone());

        let id = mm.add(
            "experience",
            "连接 PG postgres://agent_body:MyPgSecret77@127.0.0.1:5432/db 与 OPENAI_API_KEY=sk-1234567890abcdef 排障",
            Some("TOKEN=ghp_1234567890abcdef 备注"),
            Some("agent:sec"),
            vec!["security".to_string()],
        ).expect("add secret memory");

        let list = storage.list_all_active_memories(10).expect("list");
        let rec = list.iter().find(|m| m.id == id).expect("found");
        assert!(!rec.content.contains("MyPgSecret77"));
        assert!(!rec.content.contains("sk-1234567890abcdef"));
        assert!(!rec.summary.as_deref().unwrap_or("").contains("ghp_1234567890abcdef"));
    }
}
