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

    /// 批量读取指定实体的 `id -> created_at` 映射（用于全量同步保留真实时间）
    pub fn list_created_at_map(&self, table: &str) -> Result<std::collections::HashMap<String, String>> {
        self.provider.list_created_at_map(table)
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
                    "postgresql://postgres@127.0.0.1:5432/agent_body"
                );
                let provider = PgStorageProvider::connect(url)?;
                Ok(Self {
                    provider: Arc::new(provider),
                    event_bus: None,
                })
            }
            "duckdb" => {
                let path = config.storage.duckdb_path.clone().unwrap_or_else(|| {
                    config.data_dir.join("body.duckdb")
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
}

/// 生成宿主机可移植默认放置规则种子（基于 workspace_base 自动适配本机 `~/new` 或便携目录）
pub(crate) fn default_seed_placement_rules(base_dir: &Path) -> Vec<PlacementRule> {
    let b = base_dir.to_string_lossy().trim_end_matches('/').to_string();
    vec![
        PlacementRule {
            id: "rule-1".into(),
            rule_name: "shared_tools".into(),
            pattern_type: "script".into(),
            target_base_dir: format!("{}/shared_proj/", b),
            naming_pattern: Some("{name}/".into()),
            description: Some("长期沉淀的可复用脚本与程序库".into()),
            guideline: Some("建立独立子目录，配备 README.md 与入口脚本，严禁硬编码敏感信息".into()),
            priority: 10,
        },
        PlacementRule {
            id: "rule-2".into(),
            rule_name: "work_projects".into(),
            pattern_type: "project".into(),
            target_base_dir: format!("{}/work/", b),
            naming_pattern: Some("{name}/".into()),
            description: Some("常规立项的工作工程与开发目录".into()),
            guideline: Some("每个工程独立版本控制，优先采用设计文档闭环验证".into()),
            priority: 20,
        },
        PlacementRule {
            id: "rule-3".into(),
            rule_name: "ai_skills".into(),
            pattern_type: "skill".into(),
            target_base_dir: format!("{}/skills/", b),
            naming_pattern: Some("{name}/".into()),
            description: Some("沉淀的通用高频高质量 AI 技能".into()),
            guideline: Some("必须包含标准 SKILL.md，保障被高频调用的稳定性".into()),
            priority: 30,
        },
        PlacementRule {
            id: "rule-4".into(),
            rule_name: "open_source_src".into(),
            pattern_type: "src".into(),
            target_base_dir: format!("{}/src/", b),
            naming_pattern: Some("{name}/".into()),
            description: Some("浅克隆的开源项目源码库".into()),
            guideline: Some("克隆后进入根目录优先使用 codegraph init 生成代码地图".into()),
            priority: 40,
        },
        PlacementRule {
            id: "rule-5".into(),
            rule_name: "notes_and_lists".into(),
            pattern_type: "note".into(),
            target_base_dir: format!("{}/note/", b),
            naming_pattern: Some("{name}.md".into()),
            description: Some("重要笔记、知识点与权威网站清单".into()),
            guideline: Some("好记性不如烂笔头，用于跨周期翻阅沉淀".into()),
            priority: 50,
        },
        PlacementRule {
            id: "rule-6".into(),
            rule_name: "cron_jobs".into(),
            pattern_type: "other".into(),
            target_base_dir: format!("{}/cron/", b),
            naming_pattern: Some("{name}/".into()),
            description: Some("定时任务的 prompt 与输出产物".into()),
            guideline: Some("多 cron 必须先查撞点表错峰，粒度至少 30 分钟".into()),
            priority: 60,
        },
        PlacementRule {
            id: "rule-7".into(),
            rule_name: "temp_files".into(),
            pattern_type: "tmp".into(),
            target_base_dir: format!("{}/tmp/", b),
            naming_pattern: Some("{name}".into()),
            description: Some("可随时删除的临时文件与调试日志".into()),
            guideline: Some("明确生命周期，定期巡检清理".into()),
            priority: 70,
        },
        PlacementRule {
            id: "rule-8".into(),
            rule_name: "open_source_shared".into(),
            pattern_type: "other".into(),
            target_base_dir: format!("{}/shared_git/", b),
            naming_pattern: Some("{name}/".into()),
            description: Some("沉淀的可对外开源精品项目".into()),
            guideline: Some("高质量开源工程，结构规整文档齐全".into()),
            priority: 80,
        },
    ]
}

/// 生成宿主机可移植默认扫描颗粒度策略种子
pub(crate) fn default_seed_scan_policies(base_dir: &Path, home_dir: &Path) -> Vec<ScanPolicy> {
    let b = base_dir.to_string_lossy().trim_end_matches('/').to_string();
    let h = home_dir.to_string_lossy().trim_end_matches('/').to_string();
    vec![
        ScanPolicy {
            id: "policy-1".into(),
            path_pattern: format!("{}/shared_proj/**", b),
            tier: "deep".into(),
            max_depth: -1,
            enable_fts: true,
            enable_vec: true,
            enable_graph: true,
            notes: Some("白名单: 核心工具库，深度递归到底".into()),
        },
        ScanPolicy {
            id: "policy-2".into(),
            path_pattern: format!("{}/work/**", b),
            tier: "deep".into(),
            max_depth: -1,
            enable_fts: true,
            enable_vec: true,
            enable_graph: true,
            notes: Some("白名单: 核心工作区，深度递归到底".into()),
        },
        ScanPolicy {
            id: "policy-3".into(),
            path_pattern: format!("{}/skills/**", b),
            tier: "deep".into(),
            max_depth: -1,
            enable_fts: true,
            enable_vec: true,
            enable_graph: true,
            notes: Some("白名单: 技能库，全量递归".into()),
        },
        ScanPolicy {
            id: "policy-4".into(),
            path_pattern: "**/node_modules/**".into(),
            tier: "shallow".into(),
            max_depth: 0,
            enable_fts: false,
            enable_vec: false,
            enable_graph: false,
            notes: Some("黑名单: 仅记录当前目录实体本身标量，严禁递归下钻".into()),
        },
        ScanPolicy {
            id: "policy-5".into(),
            path_pattern: "**/.cache/**".into(),
            tier: "shallow".into(),
            max_depth: 0,
            enable_fts: false,
            enable_vec: false,
            enable_graph: false,
            notes: Some("黑名单: 缓存目录，仅记录当前目录标量，不下钻".into()),
        },
        ScanPolicy {
            id: "policy-6".into(),
            path_pattern: "**/.git/**".into(),
            tier: "shallow".into(),
            max_depth: 0,
            enable_fts: false,
            enable_vec: false,
            enable_graph: false,
            notes: Some("黑名单: Git 内部目录，仅记录当前目录标量，不下钻".into()),
        },
        ScanPolicy {
            id: "policy-7".into(),
            path_pattern: format!("{}/.ssh/**", h),
            tier: "shallow".into(),
            max_depth: 0,
            enable_fts: false,
            enable_vec: false,
            enable_graph: false,
            notes: Some("黑名单: 私密目录，仅记录目录存在，严禁提取内容".into()),
        },
        ScanPolicy {
            id: "policy-8".into(),
            path_pattern: "/proc/**".into(),
            tier: "shallow".into(),
            max_depth: 0,
            enable_fts: false,
            enable_vec: false,
            enable_graph: false,
            notes: Some("黑名单: 系统虚拟文件系统，仅记录根目录，严禁递归".into()),
        },
        ScanPolicy {
            id: "policy-9".into(),
            path_pattern: format!("{}/src/**", b),
            tier: "restricted".into(),
            max_depth: 2,
            enable_fts: true,
            enable_vec: false,
            enable_graph: false,
            notes: Some("受限区: 开源项目库，仅索引项目入口与 README".into()),
        },
        ScanPolicy {
            id: "policy-10".into(),
            path_pattern: format!("{}/tmp/**", b),
            tier: "restricted".into(),
            max_depth: 1,
            enable_fts: false,
            enable_vec: false,
            enable_graph: false,
            notes: Some("受限区: 临时目录，仅记录浅层文件".into()),
        },
    ]
}

impl Storage {

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

    pub fn upsert_scan_policy(&self, policy: &ScanPolicy) -> Result<()> {
        self.provider.upsert_scan_policy(policy)
    }

    // ─── Assets ──────────────────────────────────────────────────────

    pub fn upsert_asset(&self, asset: &AssetRecord) -> Result<()> {
        self.provider.upsert_asset(asset)?;
        if let Some(ref bus) = self.event_bus {
            let _delivered = bus.emit(crate::hooks::BodyEvent::AssetRegistered {
                path: asset.path.clone(),
                object_type: asset.asset_type.clone(),
                owner: asset.owner.clone(),
                security_level: asset.security_level.clone(),
                timestamp: chrono::Local::now().to_rfc3339(),
            });
        }
        Ok(())
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

    pub fn update_asset_content(&self, path: &str, content_text: &str) -> Result<()> {
        self.provider.update_asset_content(path, content_text)
    }

    pub fn get_asset_content(&self, path: &str) -> Result<Option<String>> {
        self.provider.get_asset_content(path)
    }

    pub fn search_assets_fuzzy_db(
        &self,
        query: &str,
        limit: usize,
        use_index: bool,
        scope_path: Option<&str>,
    ) -> Result<Vec<DbFuzzyAssetHit>> {
        self.provider.search_assets_fuzzy_db(query, limit, use_index, scope_path)
    }

    pub fn search_assets_scalar_db(
        &self,
        filter: &AssetScalarFilter,
        limit: usize,
    ) -> Result<Vec<AssetRecord>> {
        self.provider.search_assets_scalar_db(filter, limit)
    }

    pub fn search_assets_vector_db(
        &self,
        query_vec: &[f32],
        min_sim: f32,
        limit: usize,
        scope_path: Option<&str>,
    ) -> Result<Vec<(AssetRecord, f32)>> {
        self.provider.search_assets_vector_db(query_vec, min_sim, limit, scope_path)
    }

    pub fn set_fuzzy_index_enabled(&self, enabled: bool) -> Result<bool> {
        self.provider.set_fuzzy_index_enabled(enabled)
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
        self.provider.insert_memory(mem)?;
        if let Some(ref bus) = self.event_bus {
            let _delivered = bus.emit(crate::hooks::BodyEvent::MemoryAdded {
                memory_id: mem.id.clone(),
                memory_type: mem.memory_type.clone(),
                summary: mem.summary.clone(),
                tags: mem.tags.clone(),
                timestamp: chrono::Local::now().to_rfc3339(),
            });
        }
        Ok(())
    }

    pub fn search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        self.provider.search_memories_fts(query, limit)
    }

    pub fn search_memories_fuzzy_db(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        self.provider.search_memories_fuzzy_db(query, limit)
    }

    pub fn list_all_active_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        self.provider.list_all_active_memories(limit)
    }

    pub fn list_all_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        self.provider.list_all_memories(limit)
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

    pub fn count_expired_coordination_locks(&self) -> Result<usize> {
        self.provider.count_expired_coordination_locks()
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

    pub fn list_all_benchmark_runs(&self, limit: usize) -> Result<Vec<BenchmarkRunRecord>> {
        self.provider.list_all_benchmark_runs(limit)
    }

    pub fn delete_benchmark_run(&self, run_id: &str) -> Result<bool> {
        self.provider.delete_benchmark_run(run_id)
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

    // ─── Phase 43: Dual-Domain Behavioral Audit Events ──────────────
    pub fn insert_audit_event(&self, rec: &AuditEventRecord) -> Result<()> {
        self.provider.insert_audit_event(rec)
    }

    pub fn query_audit_events(&self, filter: &AuditQueryFilter) -> Result<Vec<AuditEventRecord>> {
        self.provider.query_audit_events(filter)
    }

    pub fn audit_domain_summary(&self, window: &str) -> Result<AuditDomainSummary> {
        self.provider.audit_domain_summary(window)
    }

    pub fn prune_old_audit_events(&self, before_iso: &str) -> Result<usize> {
        self.provider.prune_old_audit_events(before_iso)
    }

    pub fn insert_body_checkpoint(&self, rec: &BodyCheckpointRecord) -> Result<()> {
        self.provider.insert_body_checkpoint(rec)
    }

    pub fn get_latest_active_body_checkpoint(&self, body_id: &str) -> Result<Option<BodyCheckpointRecord>> {
        self.provider.get_latest_active_body_checkpoint(body_id)
    }

    pub fn list_body_checkpoints(&self, status_filter: Option<&str>, limit: usize) -> Result<Vec<BodyCheckpointRecord>> {
        self.provider.list_body_checkpoints(status_filter, limit)
    }

    pub fn mark_body_checkpoint_resumed(&self, checkpoint_id: &str, by_brain: &str, resumed_at: &str) -> Result<bool> {
        self.provider.mark_body_checkpoint_resumed(checkpoint_id, by_brain, resumed_at)
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
            server_name: "agent-body".to_string(),
            client_source: "hermes".to_string(),
            transport: "stdio".to_string(),
            command_or_url: "/usr/local/bin/agent-body mcp".to_string(),
            tools_json: serde_json::json!(["body_brief", "body_port_check"]),
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
            id: "proj:agent-body".to_string(),
            node_type: "project".to_string(),
            name: "agent-body".to_string(),
            label: "Agent 本体工程".to_string(),
            properties: serde_json::json!({"path": "/root/new/work/agent-body"}),
            updated_at: None,
        };
        let s_node = FactNodeRecord {
            id: "service:agent-body.service".to_string(),
            node_type: "service".to_string(),
            name: "agent-body.service".to_string(),
            label: "Agent 本体守护服务".to_string(),
            properties: serde_json::json!({"unit": "/etc/systemd/system/agent-body.service"}),
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
            "service:agent-body.service",
            "proj:agent-body",
            "depends_on",
            serde_json::json!({}),
            1.0,
        );
        let edge2 = FactEdgeRecord::new_explicit(
            "service:agent-body.service",
            "port:8080",
            "listens_on",
            serde_json::json!({}),
            1.0,
        );

        storage.upsert_fact_edge(&edge1).expect("edge 1");
        storage.upsert_fact_edge(&edge2).expect("edge 2");

        // 3. 验证从服务节点出发的 1 跳子图
        let sub1 = storage.traverse_fact_graph("service:agent-body.service", 1).expect("subgraph 1 hop");
        assert_eq!(sub1.nodes.len(), 3, "1跳应该连同项目和端口全部捕获");
        assert_eq!(sub1.edges.len(), 2, "应包含2条边");

        // 4. 验证根据名字模糊查找节点
        let found = storage.find_fact_nodes_by_name("agent-body").expect("find nodes");
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
            total_body_tokens: 500,
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
            tool_or_action: "body_skill_route".to_string(),
            provenance_rule: "PR-01".to_string(),
            baseline_tokens: 4000,
            body_tokens: 300,
            tokens_saved: 3700,
            baseline_rounds: 3,
            body_rounds: 1,
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
            tool_or_action: "body_hook_pre".to_string(),
            provenance_rule: "PR-03".to_string(),
            baseline_tokens: 600,
            body_tokens: 120,
            tokens_saved: 480,
            baseline_rounds: 3,
            body_rounds: 1,
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

    #[test]
    fn test_sqlite_memory_upsert_and_fts_active_filter() {
        let storage = Storage::open_in_memory().expect("open");
        let mem1 = MemoryRecord {
            id: "mem-upsert-1".to_string(),
            memory_type: "experience".to_string(),
            content: "Phase53 sqlite upsert alpha".to_string(),
            summary: Some("alpha summary".to_string()),
            source_agent: Some("agent-a".to_string()),
            source_session: Some("sess-a".to_string()),
            tags: vec!["tag1".to_string()],
            weight: 0.7,
            pinned: false,
            status: "active".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
        };
        storage.insert_memory(&mem1).expect("insert mem1");

        // 同一 ID 二次写入（空向量），应保留原向量并更新标签/权重/置顶
        let mem1_upd = MemoryRecord {
            id: "mem-upsert-1".to_string(),
            memory_type: "decision".to_string(),
            content: "Phase53 sqlite upsert uniquetok".to_string(),
            summary: Some("updated summary".to_string()),
            source_agent: None,
            source_session: None,
            tags: vec!["tag1".to_string(), "tag2".to_string()],
            weight: 0.95,
            pinned: true,
            status: "active".to_string(),
            embedding: Vec::new(),
        };
        storage.insert_memory(&mem1_upd).expect("upsert empty emb");

        let hits = storage.search_memories_fts("uniquetok", 10).expect("fts active");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(hits[0].tags, vec!["tag1".to_string(), "tag2".to_string()]);
        assert!(hits[0].pinned);
        assert_eq!(hits[0].source_agent.as_deref(), Some("agent-a"));

        // 非空新向量应覆盖旧向量
        let mut mem1_new_emb = mem1_upd.clone();
        mem1_new_emb.embedding = vec![0.9, 0.8];
        storage.insert_memory(&mem1_new_emb).expect("upsert new emb");
        let hits2 = storage.search_memories_fts("uniquetok", 10).expect("fts new emb");
        assert_eq!(hits2[0].embedding, vec![0.9, 0.8]);

        // 归档后 FTS 不再返回，但 list_all_memories 仍可全量读出用于迁移
        storage.update_memory_status("mem-upsert-1", "archived").expect("archive");
        let hits_archived = storage.search_memories_fts("uniquetok", 10).expect("fts after archive");
        assert!(hits_archived.is_empty(), "archived 记忆不应出现在 active FTS 召回中");
        let all = storage.list_all_memories(10).expect("list all memories");
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].status, "archived");
    }

    #[test]
    fn test_sqlite_benchmark_awr_isolation_and_delete() {
        let storage = Storage::open_in_memory().expect("open");
        let mk = |run_id: &str, suite: &str| BenchmarkRunRecord {
            run_id: run_id.to_string(),
            suite_name: suite.to_string(),
            git_commit: "test".to_string(),
            app_version: crate::VERSION.to_string(),
            host_fingerprint: serde_json::json!({}),
            workload_count: 1,
            total_baseline_tokens: 100,
            total_body_tokens: 10,
            overall_token_savings_pct: 90.0,
            overall_speedup_factor: 2.0,
            total_hazards_intercepted: 0,
            total_noise_blocked: 0,
            sla_passed: true,
            results_json: serde_json::json!([]),
            report_sha256: "sha".to_string(),
            duration_ms: 5,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        storage.insert_benchmark_run(&mk("bench:normal-1", "full")).expect("insert normal");
        storage.insert_benchmark_run(&mk("awr:snap:snap-1", "awr_internal")).expect("insert awr");

        assert_eq!(storage.list_benchmark_runs(10).expect("filtered").len(), 1);
        assert_eq!(storage.list_all_benchmark_runs(10).expect("all").len(), 2);
        assert!(storage.delete_benchmark_run("awr:snap:snap-1").expect("delete awr"));
        assert!(storage.get_benchmark_run("awr:snap:snap-1").expect("get deleted").is_none());
    }
}
