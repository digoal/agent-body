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
    event_bus: Option<crate::hooks::EventBusHandle>,
}

impl Storage {
    /// 基于自定义 StorageProvider 构建存储门面
    pub fn new(provider: Arc<dyn StorageProvider>) -> Self {
        Self { provider, event_bus: None }
    }

    /// 关联异步事件总线句柄
    pub fn with_event_bus(provider: Arc<dyn StorageProvider>, event_bus: crate::hooks::EventBusHandle) -> Self {
        Self { provider, event_bus: Some(event_bus) }
    }

    pub fn set_event_bus(&mut self, event_bus: crate::hooks::EventBusHandle) {
        self.event_bus = Some(event_bus);
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
            event_bus: None,
        })
    }

    /// 打开 SQLite 内存数据库（用于单元测试快速隔离）
    pub fn open_in_memory() -> Result<Self> {
        let provider = SqliteStorageProvider::open_in_memory()?;
        Ok(Self {
            provider: Arc::new(provider),
            event_bus: None,
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
                    event_bus: None,
                })
            }
            "duckdb" => {
                let path = config.storage.duckdb_path.clone().unwrap_or_else(|| {
                    config.data_dir.join("steward.duckdb")
                });
                let provider = DuckDbStorageProvider::open(&path)?;
                Ok(Self {
                    provider: Arc::new(provider),
                    event_bus: None,
                })
            }
            _ => {
                let provider = SqliteStorageProvider::open(&config.db_path)?;
                Ok(Self {
                    provider: Arc::new(provider),
                    event_bus: None,
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
        let res = self.provider.upsert_asset(asset)?;
        if let Some(ref bus) = self.event_bus {
            let _ = bus.emit(crate::hooks::StewardEvent::AssetRegistered {
                path: asset.path.clone(),
                object_type: asset.asset_type.clone(),
                owner: asset.owner.clone(),
                security_level: asset.security_level.clone(),
                timestamp: chrono::Local::now().to_rfc3339(),
            });
        }
        Ok(res)
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
        let res = self.provider.insert_memory(mem)?;
        if let Some(ref bus) = self.event_bus {
            let _ = bus.emit(crate::hooks::StewardEvent::MemoryAdded {
                memory_id: mem.id.clone(),
                memory_type: mem.memory_type.clone(),
                summary: mem.summary.clone(),
                tags: mem.tags.clone(),
                timestamp: chrono::Local::now().to_rfc3339(),
            });
        }
        Ok(res)
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
        let mut sanitized = change.clone();
        sanitized.description = change
            .description
            .as_deref()
            .map(crate::scanner::metadata::SecretRedactor::redact_single_line);
        sanitized.details = crate::scanner::metadata::SecretRedactor::redact_json_value(&change.details);
        self.provider.record_change(&sanitized)
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

    pub fn delete_auto_links_for_source(&self, source_id: &str) -> Result<usize> {
        self.provider.delete_auto_links_for_source(source_id)
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

    pub fn traverse_fact_graph_filtered(
        &self,
        root_id: &str,
        max_hops: usize,
        min_weight: f32,
        tiers: Option<&[&str]>,
    ) -> Result<SubGraph> {
        self.provider
            .traverse_fact_graph_filtered(root_id, max_hops, min_weight, tiers)
    }

    pub fn list_all_fact_nodes(&self, limit: usize) -> Result<Vec<FactNodeRecord>> {
        self.provider.list_all_fact_nodes(limit)
    }

    pub fn list_all_fact_edges(&self, limit: usize) -> Result<Vec<FactEdgeRecord>> {
        self.provider.list_all_fact_edges(limit)
    }

    pub fn delete_fact_node(&self, id: &str) -> Result<usize> {
        self.provider.delete_fact_node(id)
    }

    pub fn delete_fact_edges_by_tier(&self, tier: &str) -> Result<usize> {
        self.provider.delete_fact_edges_by_tier(tier)
    }

    // ─── Mesh Dialogues & Transfers (Phase 41) ───────────────────────

    pub fn insert_mesh_dialogue(&self, rec: &MeshDialogueRecord) -> Result<()> {
        self.provider.insert_mesh_dialogue(rec)
    }

    pub fn list_mesh_dialogues(&self, limit: usize) -> Result<Vec<MeshDialogueRecord>> {
        self.provider.list_mesh_dialogues(limit)
    }

    pub fn update_mesh_dialogue_status(&self, dialogue_id: &str, status: &str) -> Result<bool> {
        self.provider.update_mesh_dialogue_status(dialogue_id, status)
    }

    pub fn insert_mesh_transfer(&self, rec: &MeshTransferRecord) -> Result<()> {
        self.provider.insert_mesh_transfer(rec)
    }

    pub fn list_mesh_transfers(&self, limit: usize) -> Result<Vec<MeshTransferRecord>> {
        self.provider.list_mesh_transfers(limit)
    }

    pub fn update_mesh_transfer_status(
        &self,
        transfer_id: &str,
        status: &str,
        accepted_path: Option<&str>,
    ) -> Result<bool> {
        self.provider
            .update_mesh_transfer_status(transfer_id, status, accepted_path)
    }

    // ─── Embedding Vectors (Phase 29) ────────────────────────────────

    pub fn update_asset_embedding(&self, path: &str, embedding: &[f32]) -> Result<()> {
        self.provider.update_asset_embedding(path, embedding)
    }

    pub fn get_asset_embedding(&self, path: &str) -> Result<Vec<f32>> {
        self.provider.get_asset_embedding(path)
    }

    pub fn update_fact_node_embedding(&self, id: &str, embedding: &[f32]) -> Result<()> {
        self.provider.update_fact_node_embedding(id, embedding)
    }

    pub fn get_fact_node_embedding(&self, id: &str) -> Result<Vec<f32>> {
        self.provider.get_fact_node_embedding(id)
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

    // ─── Phase 16: Ecosystem Catalogs & External Service Contacts ────
    pub fn upsert_mcp_catalog(&self, rec: &McpCatalogRecord) -> Result<()> {
        self.provider.upsert_mcp_catalog(rec)
    }

    pub fn list_mcp_catalog(&self) -> Result<Vec<McpCatalogRecord>> {
        self.provider.list_mcp_catalog()
    }

    pub fn search_mcp_catalog(&self, query: &str, limit: usize) -> Result<Vec<McpCatalogRecord>> {
        self.provider.search_mcp_catalog(query, limit)
    }

    pub fn upsert_skill_catalog(&self, rec: &SkillCatalogRecord) -> Result<()> {
        self.provider.upsert_skill_catalog(rec)
    }

    pub fn list_skills_catalog(&self) -> Result<Vec<SkillCatalogRecord>> {
        self.provider.list_skills_catalog()
    }

    pub fn search_skills_catalog(&self, query: &str, limit: usize) -> Result<Vec<SkillCatalogRecord>> {
        self.provider.search_skills_catalog(query, limit)
    }

    pub fn upsert_external_service(&self, rec: &ExternalServiceRecord) -> Result<()> {
        self.provider.upsert_external_service(rec)
    }

    pub fn list_external_services(&self, category: Option<&str>) -> Result<Vec<ExternalServiceRecord>> {
        self.provider.list_external_services(category)
    }

    pub fn search_external_services(&self, query: &str, limit: usize) -> Result<Vec<ExternalServiceRecord>> {
        self.provider.search_external_services(query, limit)
    }

    // ─── Phase 17: Lock Wait Queue ─────────────────────────────────────
    pub fn enqueue_lock_wait(&self, entry: &LockWaitEntry) -> Result<()> {
        self.provider.enqueue_lock_wait(entry)
    }

    pub fn list_lock_wait_queue(&self, resource_type: Option<&str>, resource_id: Option<&str>) -> Result<Vec<LockWaitEntry>> {
        self.provider.list_lock_wait_queue(resource_type, resource_id)
    }

    pub fn pop_next_lock_waiter(&self, resource_type: &str, resource_id: &str) -> Result<Option<LockWaitEntry>> {
        self.provider.pop_next_lock_waiter(resource_type, resource_id)
    }

    pub fn update_lock_wait_status(&self, wait_id: &str, status: &str) -> Result<bool> {
        self.provider.update_lock_wait_status(wait_id, status)
    }

    pub fn gc_expired_lock_waits(&self) -> Result<usize> {
        self.provider.gc_expired_lock_waits()
    }

    // ─── Phase 27: Empirical Benchmark & Effectiveness Ledger ─────────
    pub fn insert_benchmark_run(&self, run: &BenchmarkRunRecord) -> Result<()> {
        self.provider.insert_benchmark_run(run)
    }

    pub fn get_benchmark_run(&self, run_id: &str) -> Result<Option<BenchmarkRunRecord>> {
        self.provider.get_benchmark_run(run_id)
    }

    pub fn list_benchmark_runs(&self, limit: usize) -> Result<Vec<BenchmarkRunRecord>> {
        self.provider.list_benchmark_runs(limit)
    }

    pub fn insert_effectiveness_event(&self, event: &EffectivenessLedgerRecord) -> Result<()> {
        self.provider.insert_effectiveness_event(event)
    }

    pub fn query_effectiveness_ledger(&self, window: &str, tool_filter: Option<&str>, limit: usize) -> Result<EffectivenessSummary> {
        self.provider.query_effectiveness_ledger(window, tool_filter, limit)
    }

    pub fn delete_bench_sandbox_records(&self) -> Result<usize> {
        self.provider.delete_bench_sandbox_records()
    }
}

/// 根据时间窗标识 ("24h" | "7d" | "30d" | "all") 返回统一 RFC3339 截止时间
pub fn window_to_cutoff_iso(window: &str) -> Option<String> {
    let now = chrono::Utc::now();
    match window {
        "24h" | "1d" => Some((now - chrono::Duration::hours(24)).to_rfc3339()),
        "7d" => Some((now - chrono::Duration::days(7)).to_rfc3339()),
        "30d" => Some((now - chrono::Duration::days(30)).to_rfc3339()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ecosystem_catalogs_and_fts() {
        let storage = Storage::open_in_memory().expect("open_in_memory");

        let mcp = McpCatalogRecord {
            id: "mcp-1".to_string(),
            server_name: "agent-steward".to_string(),
            client_source: "hermes".to_string(),
            transport: "stdio".to_string(),
            command_or_url: "/usr/local/bin/agent-steward mcp".to_string(),
            tools_json: serde_json::json!(["steward_brief", "steward_port_check"]),
            tool_count: 2,
            est_total_tokens: 450,
            status: "active".to_string(),
            last_scanned_at: None,
        };
        storage.upsert_mcp_catalog(&mcp).expect("upsert mcp");
        assert_eq!(storage.list_mcp_catalog().expect("list mcp").len(), 1);
        assert_eq!(storage.search_mcp_catalog("port_check", 5).expect("search mcp").len(), 1);

        let skill = SkillCatalogRecord {
            id: "skill-1".to_string(),
            skill_name: "pgfaq".to_string(),
            root_source: "/root/.gemini/antigravity-cli/skills".to_string(),
            skill_path: "/root/.gemini/antigravity-cli/skills/pgfaq/SKILL.md".to_string(),
            description: "Answer PostgreSQL core database internals and SQL questions".to_string(),
            triggers: vec!["postgresql".to_string(), "pg".to_string(), "internals".to_string()],
            est_tokens: 900,
            has_scripts: false,
            has_references: true,
            status: "active".to_string(),
            last_scanned_at: None,
        };
        storage.upsert_skill_catalog(&skill).expect("upsert skill");
        assert_eq!(storage.list_skills_catalog().expect("list skills").len(), 1);
        assert_eq!(storage.search_skills_catalog("PostgreSQL", 5).expect("search skills").len(), 1);

        let svc = ExternalServiceRecord {
            id: "svc-1".to_string(),
            service_name: "local-ollama".to_string(),
            category: "embedding".to_string(),
            endpoint: "http://127.0.0.1:11434".to_string(),
            protocol: "http".to_string(),
            capabilities: "text embedding nomic-embed-text bge-m3".to_string(),
            auth_type: "none".to_string(),
            auth_env_var: None,
            health_endpoint: Some("http://127.0.0.1:11434/api/tags".to_string()),
            usage_example: Some("curl http://127.0.0.1:11434/api/tags".to_string()),
            status: "healthy".to_string(),
            last_checked_at: None,
        };
        storage.upsert_external_service(&svc).expect("upsert svc");
        assert_eq!(storage.list_external_services(Some("embedding")).expect("list svc").len(), 1);
        assert_eq!(storage.search_external_services("bge-m3", 5).expect("search svc").len(), 1);
    }

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

        // 标记 stale 后 upsert 应自动清空 embedding；标记 missing 后 search_assets_fts 应过滤僵尸资产
        storage.update_asset_embedding("/root/new/shared_proj/sec_fetcher", &[0.1, 0.2]).expect("set emb");
        assert_eq!(storage.get_asset_embedding("/root/new/shared_proj/sec_fetcher").unwrap().len(), 2);

        let mut stale_asset = fetched.clone();
        stale_asset.desc_kv = serde_json::json!({"purpose": "finance_tool", "embedding_status": "stale"});
        storage.upsert_asset(&stale_asset).expect("upsert stale");
        assert!(
            storage.get_asset_embedding("/root/new/shared_proj/sec_fetcher").unwrap().is_empty(),
            "embedding_status=stale 必须写穿清空旧向量"
        );

        stale_asset.status = "missing".to_string();
        storage.upsert_asset(&stale_asset).expect("upsert missing");
        assert!(
            storage.search_assets_fts("scraper", 10).unwrap().is_empty(),
            "status=missing 的僵尸资产不应被 FTS 召回"
        );
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
        let edge1 = FactEdgeRecord::new_explicit(
            "service:agent-steward.service",
            "proj:agent-steward",
            "depends_on",
            serde_json::json!({}),
            1.0,
        );
        let edge2 = FactEdgeRecord::new_explicit(
            "service:agent-steward.service",
            "port:8080",
            "listens_on",
            serde_json::json!({}),
            1.0,
        );

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

    #[test]
    fn test_fact_edges_tier_weight_filter_and_legacy_sqlite_migration() {
        let tmp = tempfile::NamedTempFile::new().expect("tmpfile");
        let db_path = tmp.path();

        // 1. 构造 Phase 39 老库（无 evidence_tier 且带有旧窄 CHECK 约束）
        {
            let raw = rusqlite::Connection::open(db_path).expect("raw open");
            raw.execute_batch(
                "CREATE TABLE fact_nodes (
                    id TEXT PRIMARY KEY, node_type TEXT NOT NULL, name TEXT NOT NULL,
                    label TEXT NOT NULL, properties TEXT DEFAULT '{}', embedding TEXT DEFAULT '[]',
                    updated_at TEXT DEFAULT ''
                 );
                 CREATE TABLE fact_edges (
                    id INTEGER PRIMARY KEY AUTOINCREMENT, source_id TEXT NOT NULL, target_id TEXT NOT NULL,
                    relation TEXT NOT NULL CHECK (relation IN ('depends_on','listens_on','scheduled_by','reads','writes','part_of','calls','contains')),
                    properties TEXT DEFAULT '{}', confidence REAL DEFAULT 1.0, updated_at TEXT DEFAULT '',
                    UNIQUE(source_id, target_id, relation)
                 );
                 INSERT INTO fact_nodes (id, node_type, name, label) VALUES
                    ('project:a', 'project', 'a', 'A'),
                    ('project:b', 'project', 'b', 'B'),
                    ('tool:rg', 'tool', 'rg', 'ripgrep');
                 INSERT INTO fact_edges (source_id, target_id, relation, confidence)
                 VALUES ('project:a', 'project:b', 'depends_on', 0.95);",
            ).expect("seed legacy db");
        }

        // 2. 用新 Storage 打开并触发热迁移
        let storage = Storage::open(db_path).expect("open migrated");
        let existing = storage.list_all_fact_edges(50).expect("list existing");
        assert_eq!(existing.len(), 1);
        assert_eq!(existing[0].evidence_tier, "L1_EXPLICIT");
        assert!((existing[0].weight - 0.95).abs() < 1e-5);

        // 3. 插入 L2 与 L3 新关系边（若未迁移 CHECK 约束此处会失败）
        storage.upsert_fact_edge(&FactEdgeRecord {
            id: None,
            source_id: "project:a".to_string(),
            target_id: "tool:rg".to_string(),
            relation: "uses_tool".to_string(),
            properties: serde_json::json!({}),
            confidence: 0.72,
            evidence_tier: "L2_STRUCTURAL".to_string(),
            weight: 0.72,
            direction_kind: "forward_dependency".to_string(),
            updated_at: None,
        }).expect("insert L2 uses_tool");

        storage.upsert_fact_edge(&FactEdgeRecord {
            id: None,
            source_id: "project:b".to_string(),
            target_id: "tool:rg".to_string(),
            relation: "semantically_related".to_string(),
            properties: serde_json::json!({}),
            confidence: 0.45,
            evidence_tier: "L3_SEMANTIC".to_string(),
            weight: 0.45,
            direction_kind: "symmetric_affinity".to_string(),
            updated_at: None,
        }).expect("insert L3 semantically_related");

        // 4. 分层与权重过滤遍历验证
        let sub_l12 = storage
            .traverse_fact_graph_filtered("project:a", 2, 0.50, Some(&["L1_EXPLICIT", "L2_STRUCTURAL"]))
            .expect("filtered traverse");
        assert_eq!(sub_l12.edges.len(), 2, "仅应保留 L1 和 L2 边");
        assert!(sub_l12.edges.iter().all(|e| e.evidence_tier != "L3_SEMANTIC"));

        // 5. 按层清理验证（只清 L3 不伤 L1/L2）
        let deleted = storage.delete_fact_edges_by_tier("L3_SEMANTIC").expect("delete L3");
        assert_eq!(deleted, 1);
        assert_eq!(storage.list_all_fact_edges(50).expect("list after delete").len(), 2);
    }

    #[test]
    fn test_benchmark_runs_and_effectiveness_ledger() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let now = chrono::Utc::now().to_rfc3339();

        let run = BenchmarkRunRecord {
            run_id: "bench:test-run-1".to_string(),
            suite_name: "bench:full".to_string(),
            git_commit: "9ba7bca".to_string(),
            app_version: "0.4.0".to_string(),
            host_fingerprint: serde_json::json!({"os": "linux", "cpus": 8}),
            workload_count: 8,
            total_baseline_tokens: 10000,
            total_steward_tokens: 500,
            overall_token_savings_pct: 95.0,
            overall_speedup_factor: 12.5,
            total_hazards_intercepted: 4,
            total_noise_blocked: 240,
            sla_passed: true,
            results_json: serde_json::json!([{"id": "W1", "passed": true}]),
            report_sha256: "abc123".to_string(),
            duration_ms: 42,
            created_at: now.clone(),
        };
        storage.insert_benchmark_run(&run).expect("insert run");

        let fetched = storage.get_benchmark_run("bench:test-run-1").expect("get run").expect("exists");
        assert_eq!(fetched.workload_count, 8);
        assert!(fetched.sla_passed);
        assert_eq!(storage.list_benchmark_runs(10).expect("list runs").len(), 1);

        let ev1 = EffectivenessLedgerRecord {
            event_id: "ev-1".to_string(),
            session_id: "sess-1".to_string(),
            source_channel: "mcp".to_string(),
            tool_or_action: "steward_skill_route".to_string(),
            provenance_rule: "PR-01".to_string(),
            baseline_tokens: 4000,
            steward_tokens: 300,
            tokens_saved: 3700,
            baseline_rounds: 3,
            steward_rounds: 1,
            rounds_saved: 2,
            hazard_intercepted: 0,
            noise_blocked: 0,
            elapsed_us: 450,
            evidence_summary: "Routed 3/121 skills".to_string(),
            created_at: now.clone(),
        };
        let ev2 = EffectivenessLedgerRecord {
            event_id: "ev-2".to_string(),
            session_id: "bench:sess".to_string(),
            source_channel: "mcp".to_string(),
            tool_or_action: "steward_hook_pre".to_string(),
            provenance_rule: "PR-03".to_string(),
            baseline_tokens: 600,
            steward_tokens: 120,
            tokens_saved: 480,
            baseline_rounds: 3,
            steward_rounds: 1,
            rounds_saved: 2,
            hazard_intercepted: 1,
            noise_blocked: 0,
            elapsed_us: 180,
            evidence_summary: "Blocked port 8756".to_string(),
            created_at: now,
        };
        storage.insert_effectiveness_event(&ev1).expect("insert ev1");
        storage.insert_effectiveness_event(&ev2).expect("insert ev2");

        let summary = storage.query_effectiveness_ledger("7d", None, 10).expect("query ledger");
        assert_eq!(summary.total_events, 2);
        assert_eq!(summary.total_tokens_saved, 4180);
        assert_eq!(summary.total_hazards_intercepted, 1);
        assert_eq!(summary.by_rule.len(), 2);

        let removed = storage.delete_bench_sandbox_records().expect("delete sandbox");
        assert_eq!(removed, 2); // run + ev2
        let after = storage.query_effectiveness_ledger("7d", None, 10).expect("query after");
        assert_eq!(after.total_events, 1);
    }

    #[test]
    fn test_record_change_redacts_secrets() {
        let storage = Storage::open_in_memory().expect("open");
        storage.record_change(&ChangeRecord {
            id: None,
            change_type: "agent_tool:exec".to_string(),
            target: "/root/new/work/app.py".to_string(),
            description: Some("执行命令 --password=ChangeSecret123 连接 postgres://u:DbPass99@127.0.0.1/db".to_string()),
            details: serde_json::json!({
                "api_key": "sk-1234567890abcdef",
                "cmd": "curl -H 'Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.payload.sig' http://localhost"
            }),
            source: "test".to_string(),
            created_at: None,
        }).expect("record_change");

        let changes = storage.list_changes(5).expect("list_changes");
        assert_eq!(changes.len(), 1);
        let desc = changes[0].description.as_deref().unwrap_or("");
        assert!(!desc.contains("ChangeSecret123"));
        assert!(!desc.contains("DbPass99"));
        let details_str = serde_json::to_string(&changes[0].details).unwrap_or_default();
        assert!(!details_str.contains("sk-1234567890abcdef"));
        assert!(!details_str.contains("eyJhbGciOiJIUzI1NiJ9"));
    }
}
