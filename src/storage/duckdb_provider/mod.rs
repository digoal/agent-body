use std::path::Path;
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use duckdb::Connection;
use crate::storage::models::*;
use crate::storage::spi::StorageProvider;

pub const DUCKDB_SCHEMA_SQL: &str = include_str!("../duckdb_schema.sql");

#[derive(Clone)]
pub struct DuckDbStorageProvider {
    conn: Arc<Mutex<Connection>>,
}

impl DuckDbStorageProvider {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建 DuckDB 数据库目录失败: {:?}", parent))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("打开 DuckDB 数据库文件失败: {:?}", path))?;
        if let Err(e) = conn.execute_batch("SET threads = 2; SET memory_limit = '256MB';") { eprintln!("[DuckDB][WARN] 设置内存限制失败: {}", e); }
        let provider = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        provider.init()?;
        Ok(provider)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("打开 DuckDB 内存数据库失败")?;
        if let Err(e) = conn.execute_batch("SET threads = 2; SET memory_limit = '256MB';") { eprintln!("[DuckDB][WARN] 设置内存限制失败: {}", e); }
        let provider = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        provider.init()?;
        Ok(provider)
    }

    pub fn conn(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    pub fn begin_transaction(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute_batch("BEGIN TRANSACTION;")
            .context("开启 DuckDB 事务失败")?;
        Ok(())
    }

    pub fn commit_transaction(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute_batch("COMMIT;")
            .context("提交 DuckDB 事务失败")?;
        Ok(())
    }

    pub fn rollback_transaction(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        if let Err(e) = conn.execute_batch("ROLLBACK;") { eprintln!("[DuckDB][WARN] 事务回滚跳过: {}", e); }
        Ok(())
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| anyhow::anyhow!("DuckDB 连接锁异常 (mutex poisoned): {}", e))
    }
}

mod assets;
mod audit;
mod facts;
mod locks;
mod memory;

impl StorageProvider for DuckDbStorageProvider {
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

impl DuckDbStorageProvider {
    pub(super) fn map_body_checkpoint_row(row: &duckdb::Row) -> duckdb::Result<BodyCheckpointRecord> {
        let files_s: String = row.get(5)?;
        let locks_s: String = row.get(6)?;
        let facts_s: String = row.get(7)?;
        let blockers_s: String = row.get(8)?;
        let tb: i32 = row.get(10)?;
        Ok(BodyCheckpointRecord {
            checkpoint_id: row.get(0)?,
            body_id: row.get(1)?,
            from_brain: row.get(2)?,
            session_id: row.get(3)?,
            goal_summary: row.get(4)?,
            active_files: serde_json::from_str(&files_s).unwrap_or_default(),
            active_locks: serde_json::from_str(&locks_s).unwrap_or_default(),
            verified_facts: serde_json::from_str(&facts_s).unwrap_or_default(),
            open_blockers: serde_json::from_str(&blockers_s).unwrap_or_default(),
            next_step_hint: row.get(9)?,
            token_budget: tb.max(0) as usize,
            status: row.get(11)?,
            evidence_sha256: row.get(12)?,
            created_at: row.get(13)?,
            resumed_by_brain: row.get(14)?,
            resumed_at: row.get(15)?,
        })
    }

    pub(super) fn map_benchmark_run_row(row: &duckdb::Row) -> duckdb::Result<BenchmarkRunRecord> {
        let host_str: String = row.get(4)?;
        let res_str: String = row.get(13)?;
        Ok(BenchmarkRunRecord {
            run_id: row.get(0)?,
            suite_name: row.get(1)?,
            git_commit: row.get(2)?,
            app_version: row.get(3)?,
            host_fingerprint: serde_json::from_str(&host_str).unwrap_or_else(|_| serde_json::json!({})),
            workload_count: row.get::<_, i32>(5)? as usize,
            total_baseline_tokens: row.get(6)?,
            total_body_tokens: row.get(7)?,
            overall_token_savings_pct: row.get(8)?,
            overall_speedup_factor: row.get(9)?,
            total_hazards_intercepted: row.get(10)?,
            total_noise_blocked: row.get(11)?,
            sla_passed: row.get(12)?,
            results_json: serde_json::from_str(&res_str).unwrap_or_else(|_| serde_json::json!([])),
            report_sha256: row.get(14)?,
            duration_ms: row.get(15)?,
            created_at: row.get(16)?,
        })
    }
    pub(super) fn map_wait_row(row: &duckdb::Row) -> duckdb::Result<LockWaitEntry> {
        Ok(LockWaitEntry {
            wait_id: row.get(0)?,
            resource_type: row.get(1)?,
            resource_id: row.get(2)?,
            scope: row.get(3)?,
            waiting_agent: row.get(4)?,
            priority: row.get(5)?,
            enqueued_at: row.get(6)?,
            timeout_at: row.get(7)?,
            status: row.get(8)?,
        })
    }
    pub(super) fn map_mcp_row(row: &duckdb::Row) -> duckdb::Result<McpCatalogRecord> {
        let tools_str: String = row.get(5)?;
        let tools_json = serde_json::from_str(&tools_str).unwrap_or_else(|_| serde_json::json!([]));
        Ok(McpCatalogRecord {
            id: row.get(0)?,
            server_name: row.get(1)?,
            client_source: row.get(2)?,
            transport: row.get(3)?,
            command_or_url: row.get(4)?,
            tools_json,
            tool_count: row.get::<_, i32>(6)? as usize,
            est_total_tokens: row.get::<_, i32>(7)? as usize,
            status: row.get(8)?,
            last_scanned_at: row.get(9)?,
        })
    }

    pub(super) fn map_skill_row(row: &duckdb::Row) -> duckdb::Result<SkillCatalogRecord> {
        let triggers_str: String = row.get(5)?;
        let triggers: Vec<String> = serde_json::from_str(&triggers_str).unwrap_or_default();
        Ok(SkillCatalogRecord {
            id: row.get(0)?,
            skill_name: row.get(1)?,
            root_source: row.get(2)?,
            skill_path: row.get(3)?,
            description: row.get(4)?,
            triggers,
            est_tokens: row.get::<_, i32>(6)? as usize,
            has_scripts: row.get(7)?,
            has_references: row.get(8)?,
            status: row.get(9)?,
            last_scanned_at: row.get(10)?,
        })
    }

    pub(super) fn map_ext_svc_row(row: &duckdb::Row) -> duckdb::Result<ExternalServiceRecord> {
        Ok(ExternalServiceRecord {
            id: row.get(0)?,
            service_name: row.get(1)?,
            category: row.get(2)?,
            endpoint: row.get(3)?,
            protocol: row.get(4)?,
            capabilities: row.get(5)?,
            auth_type: row.get(6)?,
            auth_env_var: row.get(7)?,
            health_endpoint: row.get(8)?,
            usage_example: row.get(9)?,
            status: row.get(10)?,
            last_checked_at: row.get(11)?,
        })
    }
}


#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn test_duckdb_provider_init_and_presets() {
        let provider = DuckDbStorageProvider::open_in_memory().expect("open duckdb in memory");
        let rules = provider.get_placement_rules().expect("get rules");
        assert!(rules.len() >= 8, "应至少有 8 条预置规则");

        let policies = provider.get_scan_policies().expect("get policies");
        assert!(policies.len() >= 10, "应至少有 10 条扫描策略");
    }

    #[test]
    fn test_duckdb_provider_assets_crud_and_fts() {
        let provider = DuckDbStorageProvider::open_in_memory().expect("open duckdb in memory");
        let asset = AssetRecord {
            id: "asset-duck-1".to_string(),
            path: "/root/new/work/duckdb-test".to_string(),
            asset_type: "project".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("DuckDB 高性能分析引擎测试工程".to_string()),
            desc_kv: serde_json::json!({"engine": "duckdb"}),
            usage: Some("用于测试 DuckDB Provider SPI 实现".to_string()),
            size_bytes: Some(1024),
            file_count: Some(5),
            tags: vec!["duckdb".to_string(), "olap".to_string()],
            status: "active".to_string(),
        };

        provider.upsert_asset(&asset).expect("upsert asset");
        let fetched = provider.get_asset_by_path(&asset.path).expect("get asset").expect("must exist");
        assert_eq!(fetched.id, asset.id);
        assert_eq!(fetched.tags, vec!["duckdb", "olap"]);

        // 全文/模糊检索
        let search_res = provider.search_assets_fts("性能分析", 10).expect("fts search");
        assert_eq!(search_res.len(), 1);
        assert_eq!(search_res[0].id, asset.id);
    }

    #[test]
    fn test_duckdb_provider_property_graph() {
        let provider = DuckDbStorageProvider::open_in_memory().expect("open duckdb in memory");
        let node1 = FactNodeRecord {
            id: "node:agent-body".to_string(),
            node_type: "project".to_string(),
            name: "agent-body".to_string(),
            label: "Agent 本体".to_string(),
            properties: serde_json::json!({"lang": "rust"}),
            updated_at: Some("2026-11-18T12:00:00Z".to_string()),
        };
        let node2 = FactNodeRecord {
            id: "node:duckdb".to_string(),
            node_type: "tool".to_string(),
            name: "duckdb".to_string(),
            label: "DuckDB CLI".to_string(),
            properties: serde_json::json!({"type": "olap"}),
            updated_at: Some("2026-11-18T12:00:00Z".to_string()),
        };
        let edge = FactEdgeRecord::new_explicit(
            node1.id.clone(),
            node2.id.clone(),
            "depends_on",
            serde_json::json!({"scope": "analytics"}),
            0.95,
        );

        provider.upsert_fact_node(&node1).expect("upsert node1");
        provider.upsert_fact_node(&node2).expect("upsert node2");
        provider.upsert_fact_edge(&edge).expect("upsert edge");

        let graph = provider.traverse_fact_graph(&node1.id, 2).expect("traverse graph");
        assert_eq!(graph.nodes.len(), 2);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].relation, "depends_on");
    }

    #[test]
    fn test_duckdb_audit_events_crud_and_summary() {
        let provider = DuckDbStorageProvider::open_in_memory().expect("open duckdb in memory");
        let rec1 = AuditEventRecord {
            event_id: "aud-duck-1".to_string(),
            trace_id: "tr-duck-1".to_string(),
            session_id: "sess-1".to_string(),
            domain: "IN_AGENT".to_string(),
            actor_kind: "mcp_client".to_string(),
            actor_id: "agent".to_string(),
            source_channel: "mcp".to_string(),
            category: "tool_call".to_string(),
            action: "body_port_check".to_string(),
            target_type: "port".to_string(),
            target_id: "5432".to_string(),
            concurrency_mode: Some("OPTIMISTIC".to_string()),
            outcome: "success".to_string(),
            before_after_delta: serde_json::json!({"port": 5432}),
            elapsed_us: 120,
            evidence_sha256: "a".repeat(64),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let rec2 = AuditEventRecord {
            event_id: "aud-duck-2".to_string(),
            trace_id: "tr-duck-2".to_string(),
            session_id: "system".to_string(),
            domain: "OUT_OF_AGENT".to_string(),
            actor_kind: "governance_engine".to_string(),
            actor_id: "agent-body".to_string(),
            source_channel: "gc".to_string(),
            category: "host_gc".to_string(),
            action: "cleanup_tmp".to_string(),
            target_type: "dir".to_string(),
            target_id: "/root/new/tmp".to_string(),
            concurrency_mode: None,
            outcome: "dry_run".to_string(),
            before_after_delta: serde_json::json!({"candidates": 2}),
            elapsed_us: 400,
            evidence_sha256: "b".repeat(64),
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        provider.insert_audit_event(&rec1).expect("insert 1");
        provider.insert_audit_event(&rec2).expect("insert 2");

        let rows = provider
            .query_audit_events(&AuditQueryFilter {
                domain: Some("OUT_OF_AGENT".to_string()),
                limit: 10,
                ..Default::default()
            })
            .expect("query");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].event_id, "aud-duck-2");

        let sum = provider.audit_domain_summary("7d").expect("summary");
        assert_eq!(sum.total_events, 2);
        assert_eq!(sum.in_agent_count, 1);
        assert_eq!(sum.out_of_agent_count, 1);
    }
}
