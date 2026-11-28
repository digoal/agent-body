use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use postgres::{Client, NoTls};
use crate::storage::models::*;
use crate::storage::spi::StorageProvider;

pub const PG_SCHEMA_SQL: &str = include_str!("../pg_schema.sql");

#[derive(Clone)]
pub struct PgStorageProvider {
    client: Arc<Mutex<Client>>,
}

impl PgStorageProvider {
    /// 连接到 PostgreSQL 数据库并自动执行 Schema 初始化
    pub fn connect(url: &str) -> Result<Self> {
        let url_owned = url.to_string();
        let client = std::thread::spawn(move || {
            Client::connect(&url_owned, NoTls)
        })
        .join()
        .map_err(|_| anyhow::anyhow!("连接 PostgreSQL 线程异常退出"))?
        .with_context(|| format!("连接 PostgreSQL 数据库失败: {}", url))?;

        let provider = Self {
            client: Arc::new(Mutex::new(client)),
        };
        provider.init()?;
        Ok(provider)
    }

    /// 基于已建立的 Client 构造
    pub fn from_client(client: Client) -> Result<Self> {
        let provider = Self {
            client: Arc::new(Mutex::new(client)),
        };
        provider.init()?;
        Ok(provider)
    }

    fn lock_client(&self) -> Result<std::sync::MutexGuard<'_, Client>> {
        self.client
            .lock()
            .map_err(|e| anyhow::anyhow!("PostgreSQL 连接锁异常 (mutex poisoned): {}", e))
    }
}

mod assets;
mod audit;
mod facts;
mod locks;
mod memory;

impl StorageProvider for PgStorageProvider {
    fn init(&self) -> Result<()> {
        self.impl_init()
    }
    fn provider_name(&self) -> &'static str {
        self.impl_provider_name()
    }
    fn list_created_at_map(&self, table: &str) -> Result<std::collections::HashMap<String, String>> {
        self.impl_list_created_at_map(table)
    }
    fn get_placement_rules(&self) -> Result<Vec<PlacementRule>> {
        self.impl_get_placement_rules()
    }
    fn insert_placement_rule(&self, rule: &PlacementRule) -> Result<()> {
        self.impl_insert_placement_rule(rule)
    }
    fn get_scan_policies(&self) -> Result<Vec<ScanPolicy>> {
        self.impl_get_scan_policies()
    }
    fn upsert_scan_policy(&self, policy: &ScanPolicy) -> Result<()> {
        self.impl_upsert_scan_policy(policy)
    }
    fn upsert_asset(&self, asset: &AssetRecord) -> Result<()> {
        self.impl_upsert_asset(asset)
    }
    fn get_asset_by_path(&self, path: &str) -> Result<Option<AssetRecord>> {
        self.impl_get_asset_by_path(path)
    }
    fn search_assets_fts(&self, query: &str, limit: usize) -> Result<Vec<AssetRecord>> {
        self.impl_search_assets_fts(query, limit)
    }
    fn list_assets(&self, limit: usize) -> Result<Vec<AssetRecord>> {
        self.impl_list_assets(limit)
    }
    fn replace_active_processes(&self, procs: &[ActiveProcessRecord]) -> Result<()> {
        self.impl_replace_active_processes(procs)
    }
    fn list_active_processes(&self) -> Result<Vec<ActiveProcessRecord>> {
        self.impl_list_active_processes()
    }
    fn insert_memory(&self, mem: &MemoryRecord) -> Result<()> {
        self.impl_insert_memory(mem)
    }
    fn search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        self.impl_search_memories_fts(query, limit)
    }
    fn list_all_active_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        self.impl_list_all_active_memories(limit)
    }
    fn list_all_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        self.impl_list_all_memories(limit)
    }
    fn update_memory_status(&self, memory_id: &str, status: &str) -> Result<usize> {
        self.impl_update_memory_status(memory_id, status)
    }
    fn record_change(&self, change: &ChangeRecord) -> Result<()> {
        self.impl_record_change(change)
    }
    fn list_changes(&self, limit: usize) -> Result<Vec<ChangeRecord>> {
        self.impl_list_changes(limit)
    }
    fn add_link(&self, link: &LinkRecord) -> Result<()> {
        self.impl_add_link(link)
    }
    fn delete_link(&self, id: i64) -> Result<()> {
        self.impl_delete_link(id)
    }
    fn delete_links_for_target(&self, target_id: &str) -> Result<usize> {
        self.impl_delete_links_for_target(target_id)
    }
    fn delete_auto_links_for_source(&self, source_id: &str) -> Result<usize> {
        self.impl_delete_auto_links_for_source(source_id)
    }
    fn update_link_target(&self, source_id: &str, old_target: &str, new_target: &str) -> Result<usize> {
        self.impl_update_link_target(source_id, old_target, new_target)
    }
    fn query_links_for_node(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        self.impl_query_links_for_node(node_type, node_id)
    }
    fn list_all_links(&self, limit: usize) -> Result<Vec<LinkRecord>> {
        self.impl_list_all_links(limit)
    }
    fn create_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        self.impl_create_scan_job(job)
    }
    fn update_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        self.impl_update_scan_job(job)
    }
    fn get_scan_job(&self, id: &str) -> Result<Option<ScanJobRecord>> {
        self.impl_get_scan_job(id)
    }
    fn list_scan_jobs(&self, limit: usize) -> Result<Vec<ScanJobRecord>> {
        self.impl_list_scan_jobs(limit)
    }
    fn get_active_scan_job(&self) -> Result<Option<ScanJobRecord>> {
        self.impl_get_active_scan_job()
    }
    fn upsert_fact_node(&self, node: &FactNodeRecord) -> Result<()> {
        self.impl_upsert_fact_node(node)
    }
    fn upsert_fact_edge(&self, edge: &FactEdgeRecord) -> Result<()> {
        self.impl_upsert_fact_edge(edge)
    }
    fn get_fact_node(&self, id: &str) -> Result<Option<FactNodeRecord>> {
        self.impl_get_fact_node(id)
    }
    fn find_fact_nodes_by_name(&self, name: &str) -> Result<Vec<FactNodeRecord>> {
        self.impl_find_fact_nodes_by_name(name)
    }
    fn traverse_fact_graph(&self, root_id: &str, max_hops: usize) -> Result<SubGraph> {
        self.impl_traverse_fact_graph(root_id, max_hops)
    }
    fn list_all_fact_nodes(&self, limit: usize) -> Result<Vec<FactNodeRecord>> {
        self.impl_list_all_fact_nodes(limit)
    }
    fn list_all_fact_edges(&self, limit: usize) -> Result<Vec<FactEdgeRecord>> {
        self.impl_list_all_fact_edges(limit)
    }
    fn delete_fact_node(&self, id: &str) -> Result<usize> {
        self.impl_delete_fact_node(id)
    }
    fn delete_fact_edges_by_tier(&self, tier: &str) -> Result<usize> {
        self.impl_delete_fact_edges_by_tier(tier)
    }
    fn update_asset_embedding(&self, path: &str, embedding: &[f32]) -> Result<()> {
        self.impl_update_asset_embedding(path, embedding)
    }
    fn get_asset_embedding(&self, path: &str) -> Result<Vec<f32>> {
        self.impl_get_asset_embedding(path)
    }
    fn update_fact_node_embedding(&self, id: &str, embedding: &[f32]) -> Result<()> {
        self.impl_update_fact_node_embedding(id, embedding)
    }
    fn get_fact_node_embedding(&self, id: &str) -> Result<Vec<f32>> {
        self.impl_get_fact_node_embedding(id)
    }
    fn insert_coordination_lock(&self, lock: &CoordinationLock) -> Result<bool> {
        self.impl_insert_coordination_lock(lock)
    }
    fn delete_coordination_lock(&self, lock_id: &str) -> Result<bool> {
        self.impl_delete_coordination_lock(lock_id)
    }
    fn get_coordination_lock(&self, resource_type: &str, resource_id: &str) -> Result<Option<CoordinationLock>> {
        self.impl_get_coordination_lock(resource_type, resource_id)
    }
    fn list_coordination_locks(&self, resource_type: Option<&str>) -> Result<Vec<CoordinationLock>> {
        self.impl_list_coordination_locks(resource_type)
    }
    fn gc_expired_coordination_locks(&self) -> Result<usize> {
        self.impl_gc_expired_coordination_locks()
    }
    fn count_expired_coordination_locks(&self) -> Result<usize> {
        self.impl_count_expired_coordination_locks()
    }
    fn upsert_mcp_catalog(&self, rec: &McpCatalogRecord) -> Result<()> {
        self.impl_upsert_mcp_catalog(rec)
    }
    fn list_mcp_catalog(&self) -> Result<Vec<McpCatalogRecord>> {
        self.impl_list_mcp_catalog()
    }
    fn search_mcp_catalog(&self, query: &str, limit: usize) -> Result<Vec<McpCatalogRecord>> {
        self.impl_search_mcp_catalog(query, limit)
    }
    fn upsert_skill_catalog(&self, rec: &SkillCatalogRecord) -> Result<()> {
        self.impl_upsert_skill_catalog(rec)
    }
    fn list_skills_catalog(&self) -> Result<Vec<SkillCatalogRecord>> {
        self.impl_list_skills_catalog()
    }
    fn search_skills_catalog(&self, query: &str, limit: usize) -> Result<Vec<SkillCatalogRecord>> {
        self.impl_search_skills_catalog(query, limit)
    }
    fn upsert_external_service(&self, rec: &ExternalServiceRecord) -> Result<()> {
        self.impl_upsert_external_service(rec)
    }
    fn list_external_services(&self, category: Option<&str>) -> Result<Vec<ExternalServiceRecord>> {
        self.impl_list_external_services(category)
    }
    fn search_external_services(&self, query: &str, limit: usize) -> Result<Vec<ExternalServiceRecord>> {
        self.impl_search_external_services(query, limit)
    }
    fn enqueue_lock_wait(&self, entry: &LockWaitEntry) -> Result<()> {
        self.impl_enqueue_lock_wait(entry)
    }
    fn list_lock_wait_queue(&self, resource_type: Option<&str>, resource_id: Option<&str>) -> Result<Vec<LockWaitEntry>> {
        self.impl_list_lock_wait_queue(resource_type, resource_id)
    }
    fn pop_next_lock_waiter(&self, resource_type: &str, resource_id: &str) -> Result<Option<LockWaitEntry>> {
        self.impl_pop_next_lock_waiter(resource_type, resource_id)
    }
    fn update_lock_wait_status(&self, wait_id: &str, status: &str) -> Result<bool> {
        self.impl_update_lock_wait_status(wait_id, status)
    }
    fn gc_expired_lock_waits(&self) -> Result<usize> {
        self.impl_gc_expired_lock_waits()
    }
    fn insert_benchmark_run(&self, run: &BenchmarkRunRecord) -> Result<()> {
        self.impl_insert_benchmark_run(run)
    }
    fn get_benchmark_run(&self, run_id: &str) -> Result<Option<BenchmarkRunRecord>> {
        self.impl_get_benchmark_run(run_id)
    }
    fn list_benchmark_runs(&self, limit: usize) -> Result<Vec<BenchmarkRunRecord>> {
        self.impl_list_benchmark_runs(limit)
    }
    fn list_all_benchmark_runs(&self, limit: usize) -> Result<Vec<BenchmarkRunRecord>> {
        self.impl_list_all_benchmark_runs(limit)
    }
    fn delete_benchmark_run(&self, run_id: &str) -> Result<bool> {
        self.impl_delete_benchmark_run(run_id)
    }
    fn insert_effectiveness_event(&self, event: &EffectivenessLedgerRecord) -> Result<()> {
        self.impl_insert_effectiveness_event(event)
    }
    fn query_effectiveness_ledger( &self, window: &str, tool_filter: Option<&str>, limit: usize, ) -> Result<EffectivenessSummary> {
        self.impl_query_effectiveness_ledger(window, tool_filter, limit)
    }
    fn delete_bench_sandbox_records(&self) -> Result<usize> {
        self.impl_delete_bench_sandbox_records()
    }
    fn insert_mesh_dialogue(&self, rec: &MeshDialogueRecord) -> Result<()> {
        self.impl_insert_mesh_dialogue(rec)
    }
    fn list_mesh_dialogues(&self, limit: usize) -> Result<Vec<MeshDialogueRecord>> {
        self.impl_list_mesh_dialogues(limit)
    }
    fn update_mesh_dialogue_status(&self, dialogue_id: &str, status: &str) -> Result<bool> {
        self.impl_update_mesh_dialogue_status(dialogue_id, status)
    }
    fn insert_mesh_transfer(&self, rec: &MeshTransferRecord) -> Result<()> {
        self.impl_insert_mesh_transfer(rec)
    }
    fn list_mesh_transfers(&self, limit: usize) -> Result<Vec<MeshTransferRecord>> {
        self.impl_list_mesh_transfers(limit)
    }
    fn update_mesh_transfer_status( &self, transfer_id: &str, status: &str, accepted_path: Option<&str>, ) -> Result<bool> {
        self.impl_update_mesh_transfer_status(transfer_id, status, accepted_path)
    }
    fn insert_audit_event(&self, rec: &AuditEventRecord) -> Result<()> {
        self.impl_insert_audit_event(rec)
    }
    fn query_audit_events(&self, filter: &AuditQueryFilter) -> Result<Vec<AuditEventRecord>> {
        self.impl_query_audit_events(filter)
    }
    fn audit_domain_summary(&self, window: &str) -> Result<AuditDomainSummary> {
        self.impl_audit_domain_summary(window)
    }
    fn prune_old_audit_events(&self, before_iso: &str) -> Result<usize> {
        self.impl_prune_old_audit_events(before_iso)
    }
    fn insert_body_checkpoint(&self, rec: &BodyCheckpointRecord) -> Result<()> {
        self.impl_insert_body_checkpoint(rec)
    }
    fn get_latest_active_body_checkpoint(&self, body_id: &str) -> Result<Option<BodyCheckpointRecord>> {
        self.impl_get_latest_active_body_checkpoint(body_id)
    }
    fn list_body_checkpoints(&self, status_filter: Option<&str>, limit: usize) -> Result<Vec<BodyCheckpointRecord>> {
        self.impl_list_body_checkpoints(status_filter, limit)
    }
    fn mark_body_checkpoint_resumed(&self, checkpoint_id: &str, by_brain: &str, resumed_at: &str) -> Result<bool> {
        self.impl_mark_body_checkpoint_resumed(checkpoint_id, by_brain, resumed_at)
    }
}

impl PgStorageProvider {
    pub(super) fn row_to_body_checkpoint(r: &postgres::Row) -> BodyCheckpointRecord {
        let files_s: String = r.get(5);
        let locks_s: String = r.get(6);
        let facts_s: String = r.get(7);
        let blockers_s: String = r.get(8);
        let tb: i32 = r.get(10);
        BodyCheckpointRecord {
            checkpoint_id: r.get(0),
            body_id: r.get(1),
            from_brain: r.get(2),
            session_id: r.get(3),
            goal_summary: r.get(4),
            active_files: serde_json::from_str(&files_s).unwrap_or_default(),
            active_locks: serde_json::from_str(&locks_s).unwrap_or_default(),
            verified_facts: serde_json::from_str(&facts_s).unwrap_or_default(),
            open_blockers: serde_json::from_str(&blockers_s).unwrap_or_default(),
            next_step_hint: r.get(9),
            token_budget: tb.max(0) as usize,
            status: r.get(11),
            evidence_sha256: r.get(12),
            created_at: r.get(13),
            resumed_by_brain: r.get(14),
            resumed_at: r.get(15),
            tx_epoch: r.try_get::<_, i64>(16).unwrap_or(0),
        }
    }

    pub(super) fn row_to_benchmark_run(r: &postgres::Row) -> BenchmarkRunRecord {
        let host_str: String = r.get(4);
        let res_str: String = r.get(13);
        BenchmarkRunRecord {
            run_id: r.get(0),
            suite_name: r.get(1),
            git_commit: r.get(2),
            app_version: r.get(3),
            host_fingerprint: serde_json::from_str(&host_str).unwrap_or_else(|_| serde_json::json!({})),
            workload_count: r.get::<_, i32>(5) as usize,
            total_baseline_tokens: r.get(6),
            total_body_tokens: r.get(7),
            overall_token_savings_pct: r.get(8),
            overall_speedup_factor: r.get(9),
            total_hazards_intercepted: r.get(10),
            total_noise_blocked: r.get(11),
            sla_passed: r.get(12),
            results_json: serde_json::from_str(&res_str).unwrap_or_else(|_| serde_json::json!([])),
            report_sha256: r.get(14),
            duration_ms: r.get(15),
            created_at: r.get(16),
        }
    }
    pub(super) fn row_to_wait(r: &postgres::Row) -> LockWaitEntry {
        LockWaitEntry {
            wait_id: r.get(0),
            resource_type: r.get(1),
            resource_id: r.get(2),
            scope: r.get(3),
            waiting_agent: r.get(4),
            priority: r.get(5),
            enqueued_at: r.get(6),
            timeout_at: r.get(7),
            status: r.get(8),
        }
    }
    pub(super) fn row_to_mcp(r: &postgres::Row) -> McpCatalogRecord {
        McpCatalogRecord {
            id: r.get(0),
            server_name: r.get(1),
            client_source: r.get(2),
            transport: r.get(3),
            command_or_url: r.get(4),
            tools_json: r.get(5),
            tool_count: r.get::<_, i32>(6) as usize,
            est_total_tokens: r.get::<_, i32>(7) as usize,
            status: r.get(8),
            last_scanned_at: r.get(9),
        }
    }

    pub(super) fn row_to_skill(r: &postgres::Row) -> SkillCatalogRecord {
        let triggers_val: serde_json::Value = r.get(5);
        let triggers: Vec<String> = serde_json::from_value(triggers_val).unwrap_or_default();
        SkillCatalogRecord {
            id: r.get(0),
            skill_name: r.get(1),
            root_source: r.get(2),
            skill_path: r.get(3),
            description: r.get(4),
            triggers,
            est_tokens: r.get::<_, i32>(6) as usize,
            has_scripts: r.get(7),
            has_references: r.get(8),
            status: r.get(9),
            last_scanned_at: r.get(10),
        }
    }

    pub(super) fn row_to_ext_svc(r: &postgres::Row) -> ExternalServiceRecord {
        ExternalServiceRecord {
            id: r.get(0),
            service_name: r.get(1),
            category: r.get(2),
            endpoint: r.get(3),
            protocol: r.get(4),
            capabilities: r.get(5),
            auth_type: r.get(6),
            auth_env_var: r.get(7),
            health_endpoint: r.get(8),
            usage_example: r.get(9),
            status: r.get(10),
            last_checked_at: r.get(11),
        }
    }
    /// 原生 PostgreSQL Advisory Lock 尝试加锁（会话级锁，基于 64 位整数哈希）
    pub fn try_advisory_lock(&self, key: i64) -> Result<bool> {
        let mut client = self.lock_client()?;
        let row = client.query_one("SELECT pg_try_advisory_lock($1)", &[&key])?;
        let granted: bool = row.get(0);
        Ok(granted)
    }

    /// 原生 PostgreSQL Advisory Lock 解锁
    pub fn advisory_unlock(&self, key: i64) -> Result<bool> {
        let mut client = self.lock_client()?;
        let row = client.query_one("SELECT pg_advisory_unlock($1)", &[&key])?;
        let unlocked: bool = row.get(0);
        Ok(unlocked)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    const PG_TEST_URL: &str = "postgresql://postgres@127.0.0.1:5432/agent_body_test";

    fn get_test_provider() -> Option<PgStorageProvider> {
        let url = std::env::var("AGENT_BODY_TEST_PG_URL").unwrap_or_else(|_| PG_TEST_URL.to_string());
        PgStorageProvider::connect(&url).ok()
    }

    #[test]
    fn test_pg_provider_schema_init_and_presets() {
        let provider = match get_test_provider() {
            Some(p) => p,
            None => {
                eprintln!("跳过本地 PG 测试 (本地 PG 未就绪)");
                return;
            }
        };

        assert_eq!(provider.provider_name(), "postgres");

        let rules = provider.get_placement_rules().expect("get placement rules");
        assert_eq!(rules.len(), 8, "PG 应加载 8 条默认放置规则");

        let policies = provider.get_scan_policies().expect("get scan policies");
        assert_eq!(policies.len(), 10, "PG 应加载 10 条默认扫描策略");
    }

    #[test]
    fn test_pg_provider_assets_crud_and_tsvector() {
        let provider = match get_test_provider() {
            Some(p) => p,
            None => return,
        };

        let uid = uuid::Uuid::new_v4();
        let test_path = format!("/root/new/shared_proj/pg_analyzer_{}", uid);
        let asset = AssetRecord {
            id: format!("pg-asset-{}", uid),
            path: test_path.clone(),
            asset_type: "script".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("PostgreSQL 性能诊断与内核分析脚本".to_string()),
            desc_kv: serde_json::json!({"engine": "postgres", "author": "digoal"}),
            usage: Some("python run_pg.py".to_string()),
            size_bytes: Some(4096),
            file_count: Some(2),
            tags: vec!["postgres".to_string(), "analyzer".to_string()],
            status: "active".to_string(),
        };

        provider.upsert_asset(&asset).expect("upsert asset");

        let fetched = provider.get_asset_by_path(&test_path)
            .expect("get asset")
            .expect("must exist");
        assert_eq!(fetched.owner, "user:digoal");

        // 测试 tsvector 全文检索
        let search_res = provider.search_assets_fts("内核分析", 10).expect("fts search");
        assert!(!search_res.is_empty(), "PG tsvector 检索应该成功命中目标资产");
        assert!(search_res.iter().any(|r| r.id == asset.id));
    }

    #[test]
    fn test_pg_provider_property_graph() {
        let provider = match get_test_provider() {
            Some(p) => p,
            None => return,
        };

        let node = FactNodeRecord {
            id: "service:pg_test_daemon".to_string(),
            node_type: "service".to_string(),
            name: "pg_test_daemon".to_string(),
            label: "PG测试守护进程".to_string(),
            properties: serde_json::json!({"port": 5432}),
            updated_at: None,
        };
        provider.upsert_fact_node(&node).expect("upsert fact node");

        let fetched_node = provider.get_fact_node("service:pg_test_daemon")
            .expect("get node")
            .expect("must exist");
        assert_eq!(fetched_node.name, "pg_test_daemon");

        let port_node = FactNodeRecord {
            id: "port:5432".to_string(),
            node_type: "port".to_string(),
            name: "5432".to_string(),
            label: "PostgreSQL端口".to_string(),
            properties: serde_json::json!({}),
            updated_at: None,
        };
        provider.upsert_fact_node(&port_node).expect("upsert port node");

        let edge = FactEdgeRecord::new_explicit(
            "service:pg_test_daemon",
            "port:5432",
            "listens_on",
            serde_json::json!({}),
            1.0,
        );
        provider.upsert_fact_edge(&edge).expect("upsert edge");

        let sub = provider.traverse_fact_graph("service:pg_test_daemon", 1).expect("traverse graph");
        assert_eq!(sub.nodes.len(), 2);
        assert_eq!(sub.edges.len(), 1);
    }

    #[test]
    fn test_pg_provider_phase53_parity() {
        let pg_url = std::env::var("AGENT_BODY_TEST_PG_URL").unwrap_or_else(|_| PG_TEST_URL.to_string());
        let provider = match PgStorageProvider::connect(&pg_url) {
            Ok(p) => p,
            Err(_) => {
                eprintln!("跳过 Phase 53 PG 一致性测试 (PG 暂不可用)");
                return;
            }
        };

        // 1. 验证无需 vector 扩展也可建库，且 memories.sensitive 列存在
        {
            let mut client = provider.lock_client().expect("lock client");
            let row = client
                .query_one(
                    "SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'memories' AND column_name = 'sensitive'",
                    &[],
                )
                .expect("check sensitive column");
            assert_eq!(row.get::<_, i64>(0), 1);
        }

        let uid = uuid::Uuid::new_v4();

        // 2. upsert_asset 标记 stale 清空向量
        let asset_path = format!("/root/new/work/pg_stale_{}", uid);
        let asset = AssetRecord {
            id: format!("pg-stale-{}", uid),
            path: asset_path.clone(),
            asset_type: "project".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("PG stale embedding test".to_string()),
            desc_kv: serde_json::json!({}),
            usage: None,
            size_bytes: Some(128),
            file_count: Some(1),
            tags: vec!["pg".to_string()],
            status: "active".to_string(),
        };
        provider.upsert_asset(&asset).expect("upsert asset");
        provider
            .update_asset_embedding(&asset_path, &[0.5, 0.5])
            .expect("set emb");
        assert_eq!(
            provider.get_asset_embedding(&asset_path).expect("get emb"),
            vec![0.5, 0.5]
        );
        let mut stale_asset = asset.clone();
        stale_asset.desc_kv = serde_json::json!({"embedding_status": "stale"});
        provider.upsert_asset(&stale_asset).expect("upsert stale");
        assert!(
            provider
                .get_asset_embedding(&asset_path)
                .expect("cleared emb")
                .is_empty()
        );

        // 3. insert_memory ON CONFLICT 全字段更新与空向量保留
        let mem_id = format!("pg-mem-{}", uid);
        let mem = MemoryRecord {
            id: mem_id.clone(),
            memory_type: "experience".to_string(),
            content: format!("PG upsert memory token {}", uid),
            summary: Some("pg s1".to_string()),
            source_agent: Some("agent-pg".to_string()),
            source_session: Some("sess-pg".to_string()),
            tags: vec!["v1".to_string()],
            weight: 0.6,
            pinned: false,
            status: "active".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
        };
        provider.insert_memory(&mem).expect("insert mem");
        let mut mem2 = mem.clone();
        mem2.tags = vec!["v1".to_string(), "v2".to_string()];
        mem2.pinned = true;
        mem2.embedding = Vec::new();
        provider.insert_memory(&mem2).expect("upsert mem empty emb");
        let found = provider
            .search_memories_fts(&uid.to_string(), 5)
            .expect("fts mem");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(found[0].tags, vec!["v1".to_string(), "v2".to_string()]);
        assert!(found[0].pinned);

        // 4. 同日仅过期 5 秒的协作锁必须立即失效并被 GC 清理（回归 NOW()::text ' ' vs RFC3339 'T' Bug）
        let expired_at = (chrono::Utc::now() - chrono::Duration::seconds(5)).to_rfc3339();
        let acquired_at = (chrono::Utc::now() - chrono::Duration::seconds(65)).to_rfc3339();
        let res_id = format!("/root/new/tmp/pg_lock_{}", uid);
        let exp_lock = CoordinationLock {
            lock_id: format!("lock-exp-{}", uid),
            resource_type: "file".to_string(),
            resource_id: res_id.clone(),
            scope: "exclusive".to_string(),
            holder_agent: "agent-old".to_string(),
            holder_pid: Some(1234),
            acquired_at,
            expires_at: expired_at,
            metadata: None,
        };
        assert!(provider.insert_coordination_lock(&exp_lock).expect("insert exp lock"));
        assert!(
            provider
                .get_coordination_lock("file", &res_id)
                .expect("get exp lock")
                .is_none(),
            "同日已过期 5 秒的锁必须不可见"
        );
        assert!(
            provider
                .gc_expired_coordination_locks()
                .expect("gc locks")
                >= 1
        );

        // 5. benchmark_runs awr:% 过滤与删除
        let awr_id = format!("awr:snap:pg-{}", uid);
        let awr_run = BenchmarkRunRecord {
            run_id: awr_id.clone(),
            suite_name: "awr_internal".to_string(),
            git_commit: "pg".to_string(),
            app_version: crate::VERSION.to_string(),
            host_fingerprint: serde_json::json!({}),
            workload_count: 0,
            total_baseline_tokens: 0,
            total_body_tokens: 0,
            overall_token_savings_pct: 0.0,
            overall_speedup_factor: 1.0,
            total_hazards_intercepted: 0,
            total_noise_blocked: 0,
            sla_passed: true,
            results_json: serde_json::json!({}),
            report_sha256: "awr".to_string(),
            duration_ms: 0,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        provider.insert_benchmark_run(&awr_run).expect("insert awr");
        assert!(
            provider
                .list_benchmark_runs(100)
                .expect("filtered")
                .iter()
                .all(|r| !r.run_id.starts_with("awr:"))
        );
        assert!(
            provider
                .list_all_benchmark_runs(100)
                .expect("all")
                .iter()
                .any(|r| r.run_id == awr_id)
        );
        assert!(provider.delete_benchmark_run(&awr_id).expect("del awr"));

        // 6. Preflight PostgreSQL 连通性探针通过
        let mut cfg = crate::config::Config::default();
        cfg.storage.provider = "postgres".to_string();
        cfg.storage.pg_url = Some(pg_url);
        let mut rep = crate::preflight::PreflightReport::new();
        crate::preflight::PreflightChecker::check_storage_backend(&mut rep, &cfg);
        assert!(rep.passed_required);
    }
}
