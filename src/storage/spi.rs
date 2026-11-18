use anyhow::Result;
use crate::storage::models::*;

/// 存储提供者核心 SPI 契约 (Storage Provider Service Provider Interface)
/// 支撑多后端适配（SQLite / PostgreSQL / DuckDB），统一规范全文、标量、图与任务流存储接口
pub trait StorageProvider: Send + Sync {
    // ─── 生命周期与元数据 ───────────────────────────────────────────────
    fn init(&self) -> Result<()>;
    fn provider_name(&self) -> &'static str;

    // ─── Placement Rules ─────────────────────────────────────────────
    fn get_placement_rules(&self) -> Result<Vec<PlacementRule>>;
    fn insert_placement_rule(&self, rule: &PlacementRule) -> Result<()>;

    // ─── Scan Policies ───────────────────────────────────────────────
    fn get_scan_policies(&self) -> Result<Vec<ScanPolicy>>;

    // ─── Assets ──────────────────────────────────────────────────────
    fn upsert_asset(&self, asset: &AssetRecord) -> Result<()>;
    fn get_asset_by_path(&self, path: &str) -> Result<Option<AssetRecord>>;
    fn search_assets_fts(&self, query: &str, limit: usize) -> Result<Vec<AssetRecord>>;
    fn list_assets(&self, limit: usize) -> Result<Vec<AssetRecord>>;

    // ─── Active Processes ────────────────────────────────────────────
    fn replace_active_processes(&self, procs: &[ActiveProcessRecord]) -> Result<()>;
    fn list_active_processes(&self) -> Result<Vec<ActiveProcessRecord>>;

    // ─── Memories ────────────────────────────────────────────────────
    fn insert_memory(&self, mem: &MemoryRecord) -> Result<()>;
    fn search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>>;
    fn list_all_active_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>>;
    fn update_memory_status(&self, memory_id: &str, status: &str) -> Result<usize>;

    // ─── Changes ─────────────────────────────────────────────────────
    fn record_change(&self, change: &ChangeRecord) -> Result<()>;
    fn list_changes(&self, limit: usize) -> Result<Vec<ChangeRecord>>;

    // ─── Links & Dependencies ────────────────────────────────────────
    fn add_link(&self, link: &LinkRecord) -> Result<()>;
    fn delete_link(&self, id: i64) -> Result<()>;
    fn delete_links_for_target(&self, target_id: &str) -> Result<usize>;
    fn update_link_target(&self, source_id: &str, old_target: &str, new_target: &str) -> Result<usize>;
    fn query_links_for_node(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>>;
    fn list_all_links(&self, limit: usize) -> Result<Vec<LinkRecord>>;

    // ─── Scan Jobs (断点续扫状态机) ──────────────────────────────────
    fn create_scan_job(&self, job: &ScanJobRecord) -> Result<()>;
    fn update_scan_job(&self, job: &ScanJobRecord) -> Result<()>;
    fn get_scan_job(&self, id: &str) -> Result<Option<ScanJobRecord>>;
    fn list_scan_jobs(&self, limit: usize) -> Result<Vec<ScanJobRecord>>;
    fn get_active_scan_job(&self) -> Result<Option<ScanJobRecord>>;

    // ─── Fact Property Graph (宿主机实体属性图) ──────────────────────
    fn upsert_fact_node(&self, node: &FactNodeRecord) -> Result<()>;
    fn upsert_fact_edge(&self, edge: &FactEdgeRecord) -> Result<()>;
    fn get_fact_node(&self, id: &str) -> Result<Option<FactNodeRecord>>;
    fn find_fact_nodes_by_name(&self, name: &str) -> Result<Vec<FactNodeRecord>>;
    fn traverse_fact_graph(&self, root_id: &str, max_hops: usize) -> Result<SubGraph>;
    fn list_all_fact_nodes(&self, limit: usize) -> Result<Vec<FactNodeRecord>>;
    fn list_all_fact_edges(&self, limit: usize) -> Result<Vec<FactEdgeRecord>>;
}
