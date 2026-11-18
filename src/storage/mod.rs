pub mod models;
pub mod spi;
pub mod sqlite_provider;
pub mod pg_provider;
pub mod duckdb_provider;
pub mod duckdb_analytics;
pub mod sync_pipeline;
pub mod migration;

use std::path::Path;
use std::sync::Arc;
use anyhow::Result;
pub use models::*;
pub use spi::StorageProvider;
pub use sqlite_provider::{SqliteStorageProvider, SCHEMA_SQL};
pub use pg_provider::{PgStorageProvider, PG_SCHEMA_SQL};
pub use duckdb_provider::{DuckDbStorageProvider, DUCKDB_SCHEMA_SQL};
pub use duckdb_analytics::{AnalyticsProvider, DuckDbAnalyticsEngine, HeatmapCell, TrendPoint, ChangeFrequency, DecayPoint, GraphStats, ExportSummary};
pub use sync_pipeline::{SyncPipeline, SyncReport};
pub use migration::{StorageMigrator, MigrationReport};

/// 统一存储中枢门面 (Storage Facade)
/// 屏蔽底层具体的存储引擎实现（SQLite / PostgreSQL / DuckDB），向上层各业务模块提供一致接口
#[derive(Clone)]
pub struct Storage {
    provider: Arc<dyn StorageProvider>,
}

impl Storage {
    /// 基于自定义 StorageProvider 构建存储门面
    pub fn new(provider: Arc<dyn StorageProvider>) -> Self {
        Self { provider }
    }

    /// 获取底层存储 Provider 的动态引用
    pub fn provider(&self) -> &Arc<dyn StorageProvider> {
        &self.provider
    }

    /// 获取底层 Provider 的技术栈标识名称（如 "sqlite", "postgres", "duckdb"）
    pub fn provider_name(&self) -> &'static str {
        self.provider.provider_name()
    }

    /// 打开或创建基于内置 SQLite 的存储实现（默认生产模式）
    pub fn open(path: &Path) -> Result<Self> {
        let provider = SqliteStorageProvider::open(path)?;
        Ok(Self {
            provider: Arc::new(provider),
        })
    }

    /// 打开 SQLite 内存数据库（用于单元测试快速隔离）
    pub fn open_in_memory() -> Result<Self> {
        let provider = SqliteStorageProvider::open_in_memory()?;
        Ok(Self {
            provider: Arc::new(provider),
        })
    }

    /// 根据系统统一配置动态构建存储实例（支持 SQLite / PostgreSQL / DuckDB）
    pub fn from_config(config: &crate::config::Config) -> Result<Self> {
        match config.storage.provider.to_lowercase().as_str() {
            "postgres" | "pg" | "postgresql" => {
                let url = config.storage.pg_url.as_deref().unwrap_or(
                    "postgresql://postgres@127.0.0.1:5432/agent_steward"
                );
                let provider = PgStorageProvider::connect(url)?;
                Ok(Self {
                    provider: Arc::new(provider),
                })
            }
            "duckdb" => {
                let path = config.storage.duckdb_path.clone().unwrap_or_else(|| {
                    config.data_dir.join("steward.duckdb")
                });
                let provider = DuckDbStorageProvider::open(&path)?;
                Ok(Self {
                    provider: Arc::new(provider),
                })
            }
            _ => {
                let provider = SqliteStorageProvider::open(&config.db_path)?;
                Ok(Self {
                    provider: Arc::new(provider),
                })
            }
        }
    }

    /// 手工触发底层 Schema 初始化与热迁移
    pub fn init_schema(&self) -> Result<()> {
        self.provider.init()
    }

    // ─── Placement Rules ─────────────────────────────────────────────

    pub fn get_placement_rules(&self) -> Result<Vec<PlacementRule>> {
        self.provider.get_placement_rules()
    }

    pub fn insert_placement_rule(&self, rule: &PlacementRule) -> Result<()> {
        self.provider.insert_placement_rule(rule)
    }

    // ─── Scan Policies ───────────────────────────────────────────────

    pub fn get_scan_policies(&self) -> Result<Vec<ScanPolicy>> {
        self.provider.get_scan_policies()
    }

    // ─── Assets ──────────────────────────────────────────────────────

    pub fn upsert_asset(&self, asset: &AssetRecord) -> Result<()> {
        self.provider.upsert_asset(asset)
    }

    pub fn get_asset_by_path(&self, path: &str) -> Result<Option<AssetRecord>> {
        self.provider.get_asset_by_path(path)
    }

    pub fn search_assets_fts(&self, query: &str, limit: usize) -> Result<Vec<AssetRecord>> {
        self.provider.search_assets_fts(query, limit)
    }

    pub fn list_assets(&self, limit: usize) -> Result<Vec<AssetRecord>> {
        self.provider.list_assets(limit)
    }

    // ─── Active Processes ────────────────────────────────────────────

    pub fn replace_active_processes(&self, procs: &[ActiveProcessRecord]) -> Result<()> {
        self.provider.replace_active_processes(procs)
    }

    pub fn list_active_processes(&self) -> Result<Vec<ActiveProcessRecord>> {
        self.provider.list_active_processes()
    }

    // ─── Memories ────────────────────────────────────────────────────

    pub fn insert_memory(&self, mem: &MemoryRecord) -> Result<()> {
        self.provider.insert_memory(mem)
    }

    pub fn search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        self.provider.search_memories_fts(query, limit)
    }

    pub fn list_all_active_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        self.provider.list_all_active_memories(limit)
    }

    pub fn update_memory_status(&self, memory_id: &str, status: &str) -> Result<usize> {
        self.provider.update_memory_status(memory_id, status)
    }

    // ─── Changes ─────────────────────────────────────────────────────

    pub fn record_change(&self, change: &ChangeRecord) -> Result<()> {
        self.provider.record_change(change)
    }

    pub fn list_changes(&self, limit: usize) -> Result<Vec<ChangeRecord>> {
        self.provider.list_changes(limit)
    }

    // ─── Links & Dependencies ────────────────────────────────────────

    pub fn add_link(&self, link: &LinkRecord) -> Result<()> {
        self.provider.add_link(link)
    }

    pub fn delete_link(&self, id: i64) -> Result<()> {
        self.provider.delete_link(id)
    }

    pub fn delete_links_for_target(&self, target_id: &str) -> Result<usize> {
        self.provider.delete_links_for_target(target_id)
    }

    pub fn update_link_target(&self, source_id: &str, old_target: &str, new_target: &str) -> Result<usize> {
        self.provider.update_link_target(source_id, old_target, new_target)
    }

    pub fn query_links_for_node(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        self.provider.query_links_for_node(node_type, node_id)
    }

    pub fn list_all_links(&self, limit: usize) -> Result<Vec<LinkRecord>> {
        self.provider.list_all_links(limit)
    }

    // ─── Scan Jobs ───────────────────────────────────────────────────

    pub fn create_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        self.provider.create_scan_job(job)
    }

    pub fn update_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        self.provider.update_scan_job(job)
    }

    pub fn get_scan_job(&self, id: &str) -> Result<Option<ScanJobRecord>> {
        self.provider.get_scan_job(id)
    }

    pub fn list_scan_jobs(&self, limit: usize) -> Result<Vec<ScanJobRecord>> {
        self.provider.list_scan_jobs(limit)
    }

    pub fn get_active_scan_job(&self) -> Result<Option<ScanJobRecord>> {
        self.provider.get_active_scan_job()
    }

    // ─── Fact Property Graph ──────────────────────────────────────────

    pub fn upsert_fact_node(&self, node: &FactNodeRecord) -> Result<()> {
        self.provider.upsert_fact_node(node)
    }

    pub fn upsert_fact_edge(&self, edge: &FactEdgeRecord) -> Result<()> {
        self.provider.upsert_fact_edge(edge)
    }

    pub fn get_fact_node(&self, id: &str) -> Result<Option<FactNodeRecord>> {
        self.provider.get_fact_node(id)
    }

    pub fn find_fact_nodes_by_name(&self, name: &str) -> Result<Vec<FactNodeRecord>> {
        self.provider.find_fact_nodes_by_name(name)
    }

    pub fn traverse_fact_graph(&self, root_id: &str, max_hops: usize) -> Result<SubGraph> {
        self.provider.traverse_fact_graph(root_id, max_hops)
    }

    pub fn list_all_fact_nodes(&self, limit: usize) -> Result<Vec<FactNodeRecord>> {
        self.provider.list_all_fact_nodes(limit)
    }

    pub fn list_all_fact_edges(&self, limit: usize) -> Result<Vec<FactEdgeRecord>> {
        self.provider.list_all_fact_edges(limit)
    }

    // ─── Multi-Agent Coordination Locks ──────────────────────────────

    pub fn insert_coordination_lock(&self, lock: &CoordinationLock) -> Result<bool> {
        self.provider.insert_coordination_lock(lock)
    }

    pub fn delete_coordination_lock(&self, lock_id: &str) -> Result<bool> {
        self.provider.delete_coordination_lock(lock_id)
    }

    pub fn get_coordination_lock(&self, resource_type: &str, resource_id: &str) -> Result<Option<CoordinationLock>> {
        self.provider.get_coordination_lock(resource_type, resource_id)
    }

    pub fn list_coordination_locks(&self, resource_type: Option<&str>) -> Result<Vec<CoordinationLock>> {
        self.provider.list_coordination_locks(resource_type)
    }

    pub fn gc_expired_coordination_locks(&self) -> Result<usize> {
        self.provider.gc_expired_coordination_locks()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_init_and_defaults() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        assert_eq!(storage.provider_name(), "sqlite");
        let rules = storage.get_placement_rules().expect("get_placement_rules");
        assert_eq!(rules.len(), 8, "预置放置规则应有 8 条");

        let policies = storage.get_scan_policies().expect("get_scan_policies");
        assert_eq!(policies.len(), 10, "预置扫描策略应有 10 条");
    }

    #[test]
    fn test_assets_crud_and_fts() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let asset = AssetRecord {
            id: "test-asset-1".to_string(),
            path: "/root/new/shared_proj/sec_fetcher".to_string(),
            asset_type: "script".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("SEC EDGAR financial scraper tool".to_string()),
            desc_kv: serde_json::json!({"purpose": "finance_tool", "domain": "sec"}),
            usage: Some("python main.py".to_string()),
            size_bytes: Some(1024),
            file_count: Some(1),
            tags: vec!["python".to_string(), "sec".to_string()],
            status: "active".to_string(),
        };

        storage.upsert_asset(&asset).expect("upsert_asset");

        let fetched = storage.get_asset_by_path("/root/new/shared_proj/sec_fetcher")
            .expect("get_asset_by_path")
            .expect("should exist");
        assert_eq!(fetched.owner, "user:digoal");
        assert_eq!(fetched.desc_kv["purpose"], "finance_tool");

        let search_res = storage.search_assets_fts("scraper", 10).expect("search_assets_fts");
        assert_eq!(search_res.len(), 1);
        assert_eq!(search_res[0].id, "test-asset-1");
    }

    #[test]
    fn test_active_processes() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let proc = ActiveProcessRecord {
            id: "proc-1".to_string(),
            pid: 14208,
            name: "python3".to_string(),
            command: "python scan_sec.py".to_string(),
            owner: "agent:hermes_cron".to_string(),
            associated_path: Some("/root/new/shared_proj/scan_sec.py".to_string()),
            listening_ports: vec![8080],
            status: "running".to_string(),
            cpu_pct: 95.5,
            mem_bytes: 1024 * 1024 * 50,
            gpu_allocated_mb: 0,
            started_at: Some("2026-11-15T08:30:00Z".to_string()),
        };

        storage.replace_active_processes(&[proc]).expect("replace_active_processes");
        let list = storage.list_active_processes().expect("list_active_processes");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].pid, 14208);
        assert_eq!(list[0].owner, "agent:hermes_cron");
    }

    #[test]
    fn test_scan_jobs_crud() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let job = ScanJobRecord {
            id: "job-test-1".to_string(),
            root_path: "/root/new/work".to_string(),
            status: "pending".to_string(),
            last_path: None,
            scanned_dirs: 0,
            scanned_files: 0,
            total_assets: 0,
            shallow_blocked: 0,
            error_message: None,
            started_at: None,
            paused_at: None,
            completed_at: None,
            created_at: None,
            updated_at: None,
        };

        storage.create_scan_job(&job).expect("create_scan_job");

        let fetched = storage.get_scan_job("job-test-1").expect("get_scan_job").expect("must exist");
        assert_eq!(fetched.status, "pending");
        assert_eq!(fetched.root_path, "/root/new/work");

        let mut updated = fetched;
        updated.status = "running".to_string();
        updated.last_path = Some("/root/new/work/proj1".to_string());
        updated.scanned_dirs = 5;
        updated.scanned_files = 20;
        updated.total_assets = 25;
        storage.update_scan_job(&updated).expect("update_scan_job");

        let active = storage.get_active_scan_job().expect("get_active").expect("should have active");
        assert_eq!(active.id, "job-test-1");
        assert_eq!(active.status, "running");
        assert_eq!(active.scanned_dirs, 5);
        assert_eq!(active.last_path.as_deref(), Some("/root/new/work/proj1"));

        let list = storage.list_scan_jobs(10).expect("list_scan_jobs");
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_property_graph_crud_and_traversal() {
        let storage = Storage::open_in_memory().expect("open_in_memory");

        // 1. 创建节点: 项目 -> 服务 -> 端口
        let p_node = FactNodeRecord {
            id: "proj:agent-steward".to_string(),
            node_type: "project".to_string(),
            name: "agent-steward".to_string(),
            label: "宿主环境大管家工程".to_string(),
            properties: serde_json::json!({"path": "/root/new/work/agent-steward"}),
            updated_at: None,
        };
        let s_node = FactNodeRecord {
            id: "service:agent-steward.service".to_string(),
            node_type: "service".to_string(),
            name: "agent-steward.service".to_string(),
            label: "大管家守护服务".to_string(),
            properties: serde_json::json!({"unit": "/etc/systemd/system/agent-steward.service"}),
            updated_at: None,
        };
        let port_node = FactNodeRecord {
            id: "port:8080".to_string(),
            node_type: "port".to_string(),
            name: "8080".to_string(),
            label: "TCP 8080端口".to_string(),
            properties: serde_json::json!({"proto": "tcp"}),
            updated_at: None,
        };

        storage.upsert_fact_node(&p_node).expect("upsert proj");
        storage.upsert_fact_node(&s_node).expect("upsert service");
        storage.upsert_fact_node(&port_node).expect("upsert port");

        // 2. 创建边
        let edge1 = FactEdgeRecord {
            id: None,
            source_id: "service:agent-steward.service".to_string(),
            target_id: "proj:agent-steward".to_string(),
            relation: "depends_on".to_string(),
            properties: serde_json::json!({}),
            confidence: 1.0,
            updated_at: None,
        };
        let edge2 = FactEdgeRecord {
            id: None,
            source_id: "service:agent-steward.service".to_string(),
            target_id: "port:8080".to_string(),
            relation: "listens_on".to_string(),
            properties: serde_json::json!({}),
            confidence: 1.0,
            updated_at: None,
        };

        storage.upsert_fact_edge(&edge1).expect("edge 1");
        storage.upsert_fact_edge(&edge2).expect("edge 2");

        // 3. 验证从服务节点出发的 1 跳子图
        let sub1 = storage.traverse_fact_graph("service:agent-steward.service", 1).expect("subgraph 1 hop");
        assert_eq!(sub1.nodes.len(), 3, "1跳应该连同项目和端口全部捕获");
        assert_eq!(sub1.edges.len(), 2, "应包含2条边");

        // 4. 验证根据名字模糊查找节点
        let found = storage.find_fact_nodes_by_name("steward").expect("find nodes");
        assert_eq!(found.len(), 2);
    }
}
