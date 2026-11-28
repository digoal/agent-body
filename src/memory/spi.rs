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

// ─── Phase 76 (ISSUE #55): 事务级海马体 SPI 契约（方案 A → 方案 B 前后向兼容，零重量依赖）───

/// 三层证据分级（对齐规约 #5 与 #23）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum TxEvidenceTier {
    #[serde(rename = "L1_EXPLICIT")]
    L1Explicit,
    #[serde(rename = "L2_STRUCTURAL")]
    L2Structural,
    #[serde(rename = "L3_SEMANTIC")]
    L3Semantic,
}

impl TxEvidenceTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::L1Explicit => "L1_EXPLICIT",
            Self::L2Structural => "L2_STRUCTURAL",
            Self::L3Semantic => "L3_SEMANTIC",
        }
    }

    pub fn parse_str(s: &str) -> Self {
        match s.trim().to_ascii_uppercase().as_str() {
            "L1_EXPLICIT" | "L1" => Self::L1Explicit,
            "L2_STRUCTURAL" | "L2" => Self::L2Structural,
            _ => Self::L3Semantic,
        }
    }
}

/// 事务级双时态记忆写入请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxMemoryWriteInput {
    pub body_id: String,
    pub tenant_scope: String,
    pub fact_key: String,
    pub memory_type: String,
    pub content: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub evidence_tier: TxEvidenceTier,
    /// STC 显著性评分（0.0 ~ 1.0，方案 B 预留）
    pub salience_score: f32,
    /// 可选 CAS 期望纪元：若提供且与当前活跃行 `tx_epoch` 不一致，则拒绝覆盖返回冲突
    pub expected_epoch: Option<i64>,
    pub source_agent: Option<String>,
    pub source_session: Option<String>,
    /// 可选解耦向量句柄（如外部 LSM / pgvector 段引用，绝不把大权重塞进堆表）
    pub embedding_ref: Option<String>,
}

/// 不可变追加 + 双时态记忆事件记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxMemoryEvent {
    pub event_id: i64,
    pub body_id: String,
    pub tenant_scope: String,
    pub fact_key: String,
    pub memory_type: String,
    pub content: String,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub evidence_tier: String,
    pub evidence_sha256: String,
    pub valid_from: String,
    pub valid_to: Option<String>,
    pub tx_epoch: i64,
    pub embedding_status: String,
    pub salience_score: f32,
    pub consolidation_st: String,
    pub created_at: String,
    pub score: Option<f64>,
}

/// 事务写穿提交结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxWriteOutcome {
    pub event_id: i64,
    pub tx_epoch: i64,
    pub superseded_event_id: Option<i64>,
    pub retries: u32,
    pub evidence_sha256: String,
}

/// 双时态快照检索选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxSearchOptions {
    pub tenant_scope: String,
    /// 可选逻辑纪元时间点回溯（`tx_epoch <= as_of_epoch`）
    pub as_of_epoch: Option<i64>,
    /// 是否包含历史已关闭版本
    pub include_history: bool,
    pub limit: usize,
}

impl Default for TxSearchOptions {
    fn default() -> Self {
        Self {
            tenant_scope: "default".to_string(),
            as_of_epoch: None,
            include_history: false,
            limit: 15,
        }
    }
}

/// 事务级快照检索响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxMemorySnapshot {
    pub snapshot_epoch: i64,
    pub backend: String,
    pub fallback_active: bool,
    pub hits: Vec<TxMemoryEvent>,
}

/// 事务记忆后端运行健康态快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxProviderHealth {
    pub provider: String,
    pub connected: bool,
    pub fallback_active: bool,
    pub current_epoch: i64,
    pub active_events: i64,
    pub tombstoned_events: i64,
    pub active_segments: i64,
    pub deleted_segments: i64,
    pub pool_size: usize,
    pub vector_backend: String,
    pub server_version: Option<String>,
    pub detail: String,
}

/// 扩展事务级海马体 SPI（非破坏性扩展既有 `MemoryProvider`）
pub trait TxMemoryProvider: MemoryProvider + Send + Sync {
    /// 单事务写穿提交（截断旧 `valid_range` + 标旧段 `is_deleted` + 追加新 `tx_epoch`）
    fn append_tx_event(&self, input: &TxMemoryWriteInput) -> Result<TxWriteOutcome>;

    /// 合规/物理删除墓碑化（关闭有效窗 + 标 `tombstoned` + 清理检索段，确保 0% 僵尸召回）
    fn tombstone_fact(
        &self,
        body_id: &str,
        tenant_scope: &str,
        fact_key: &str,
        reason: &str,
    ) -> Result<u64>;

    /// 支持 `as_of_epoch` 与 `include_history` 的双时态只读快照检索
    fn search_tx(&self, query: &str, opts: &TxSearchOptions) -> Result<TxMemorySnapshot>;

    /// 查询单个 `fact_key` 的完整双时态版本演进链
    fn fact_history(
        &self,
        body_id: &str,
        tenant_scope: &str,
        fact_key: &str,
        limit: usize,
    ) -> Result<Vec<TxMemoryEvent>>;

    /// O(1) 获取当前全局逻辑纪元号
    fn current_tx_epoch(&self) -> Result<i64>;

    /// 获取连接池、向量后端、事件/段统计与降级状态
    fn tx_health(&self) -> Result<TxProviderHealth>;
}

/// 方案 B 预留：旁路快权重快照元数据（绝不承载 GB 级权重 BLOB，仅管血缘/URI/纪元锚点）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightSnapshotMeta {
    pub snapshot_id: String,
    pub body_id: String,
    pub tenant_scope: String,
    pub base_model_id: String,
    pub parent_snapshot: Option<String>,
    pub anchored_epoch: i64,
    pub adapter_kind: String,
    pub artifact_uri: String,
    pub norm_ratio: f32,
    pub status: String,
    pub created_at: String,
}

/// 方案 B 预留控制面 SPI（Phase 78 激活对接旁路 Model Runtime）
pub trait FastWeightCheckpointSpi: Send + Sync {
    fn register_snapshot(&self, meta: &FastWeightSnapshotMeta) -> Result<()>;
    fn active_snapshot(&self, body_id: &str, tenant_scope: &str) -> Result<Option<FastWeightSnapshotMeta>>;
    fn rollback_to_epoch(&self, body_id: &str, tenant_scope: &str, target_epoch: i64) -> Result<Option<FastWeightSnapshotMeta>>;
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
            source_agent: Some("agent-body".to_string()),
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
