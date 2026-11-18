use std::sync::Arc;
use std::time::Instant;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::storage::duckdb_provider::DuckDbStorageProvider;
use crate::storage::spi::StorageProvider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub assets_synced: usize,
    pub changes_synced: usize,
    pub memories_synced: usize,
    pub fact_nodes_synced: usize,
    pub fact_edges_synced: usize,
    pub duration_ms: u128,
}

/// 主存储 (SQLite / PG) → DuckDB 分析副本的增量数据同步管道
pub struct SyncPipeline {
    source: Arc<dyn StorageProvider>,
    target: Arc<DuckDbStorageProvider>,
}

impl SyncPipeline {
    pub fn new(source: Arc<dyn StorageProvider>, target: Arc<DuckDbStorageProvider>) -> Self {
        Self { source, target }
    }

    /// 执行全量或增量同步
    pub fn sync_all(&self) -> Result<SyncReport> {
        let start = Instant::now();

        // 1. 同步资产
        let assets = self.source.list_assets(500_000)?;
        let mut assets_synced = 0;
        for a in &assets {
            self.target.upsert_asset(a)?;
            assets_synced += 1;
        }

        // 2. 同步变更日志
        let changes = self.source.list_changes(500_000)?;
        let mut changes_synced = 0;
        for c in &changes {
            self.target.record_change(c)?;
            changes_synced += 1;
        }

        // 3. 同步记忆
        let memories = self.source.list_all_active_memories(500_000)?;
        let mut memories_synced = 0;
        for m in &memories {
            self.target.insert_memory(m)?;
            memories_synced += 1;
        }

        // 4. 同步事实图谱节点
        let nodes = self.source.list_all_fact_nodes(500_000)?;
        let mut nodes_synced = 0;
        for n in &nodes {
            self.target.upsert_fact_node(n)?;
            nodes_synced += 1;
        }

        // 5. 同步事实图谱边
        let edges = self.source.list_all_fact_edges(500_000)?;
        let mut edges_synced = 0;
        for e in &edges {
            self.target.upsert_fact_edge(e)?;
            edges_synced += 1;
        }

        let duration_ms = start.elapsed().as_millis();
        Ok(SyncReport {
            assets_synced,
            changes_synced,
            memories_synced,
            fact_nodes_synced: nodes_synced,
            fact_edges_synced: edges_synced,
            duration_ms,
        })
    }
}
