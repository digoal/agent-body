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
    fn delete_auto_links_for_source(&self, source_id: &str) -> Result<usize>;
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
    fn traverse_fact_graph_filtered(
        &self,
        root_id: &str,
        max_hops: usize,
        min_weight: f32,
        tiers: Option<&[&str]>,
    ) -> Result<SubGraph> {
        let mut sub = self.traverse_fact_graph(root_id, max_hops)?;
        sub.edges.retain(|e| {
            e.weight >= min_weight
                && tiers
                    .map(|ts| ts.iter().any(|t| *t == e.evidence_tier))
                    .unwrap_or(true)
        });
        Ok(sub)
    }
    fn list_all_fact_nodes(&self, limit: usize) -> Result<Vec<FactNodeRecord>>;
    fn list_all_fact_edges(&self, limit: usize) -> Result<Vec<FactEdgeRecord>>;
    fn delete_fact_node(&self, _id: &str) -> Result<usize> {
        Ok(0)
    }
    fn delete_fact_edges_by_tier(&self, _tier: &str) -> Result<usize> {
        Ok(0)
    }

    // ─── Embedding Vectors (Phase 29 全链路向量存储) ───────────────────
    fn update_asset_embedding(&self, _path: &str, _embedding: &[f32]) -> Result<()> {
        Ok(())
    }
    fn get_asset_embedding(&self, _path: &str) -> Result<Vec<f32>> {
        Ok(Vec::new())
    }
    fn update_fact_node_embedding(&self, _id: &str, _embedding: &[f32]) -> Result<()> {
        Ok(())
    }
    fn get_fact_node_embedding(&self, _id: &str) -> Result<Vec<f32>> {
        Ok(Vec::new())
    }

    // ─── Multi-Agent Coordination Locks (多 Agent 协作锁) ────────────
    fn insert_coordination_lock(&self, lock: &CoordinationLock) -> Result<bool>;
    fn delete_coordination_lock(&self, lock_id: &str) -> Result<bool>;
    fn get_coordination_lock(&self, resource_type: &str, resource_id: &str) -> Result<Option<CoordinationLock>>;
    fn list_coordination_locks(&self, resource_type: Option<&str>) -> Result<Vec<CoordinationLock>>;
    fn gc_expired_coordination_locks(&self) -> Result<usize>;

    // ─── Ecosystem Catalogs & External Service Contacts (Phase 16) ────
    fn upsert_mcp_catalog(&self, rec: &McpCatalogRecord) -> Result<()>;
    fn list_mcp_catalog(&self) -> Result<Vec<McpCatalogRecord>>;
    fn search_mcp_catalog(&self, query: &str, limit: usize) -> Result<Vec<McpCatalogRecord>>;
    fn upsert_skill_catalog(&self, rec: &SkillCatalogRecord) -> Result<()>;
    fn list_skills_catalog(&self) -> Result<Vec<SkillCatalogRecord>>;
    fn search_skills_catalog(&self, query: &str, limit: usize) -> Result<Vec<SkillCatalogRecord>>;
    fn upsert_external_service(&self, rec: &ExternalServiceRecord) -> Result<()>;
    fn list_external_services(&self, category: Option<&str>) -> Result<Vec<ExternalServiceRecord>>;
    fn search_external_services(&self, query: &str, limit: usize) -> Result<Vec<ExternalServiceRecord>>;

    // ─── Lock Wait Queue (Phase 17 等待队列) ───────────────────────────
    fn enqueue_lock_wait(&self, entry: &LockWaitEntry) -> Result<()>;
    fn list_lock_wait_queue(&self, resource_type: Option<&str>, resource_id: Option<&str>) -> Result<Vec<LockWaitEntry>>;
    fn pop_next_lock_waiter(&self, resource_type: &str, resource_id: &str) -> Result<Option<LockWaitEntry>>;
    fn update_lock_wait_status(&self, wait_id: &str, status: &str) -> Result<bool>;
    fn gc_expired_lock_waits(&self) -> Result<usize>;

    // ─── Empirical Benchmark & Effectiveness Ledger (Phase 27) ─────────
    fn insert_benchmark_run(&self, run: &BenchmarkRunRecord) -> Result<()>;
    fn get_benchmark_run(&self, run_id: &str) -> Result<Option<BenchmarkRunRecord>>;
    fn list_benchmark_runs(&self, limit: usize) -> Result<Vec<BenchmarkRunRecord>>;
    fn insert_effectiveness_event(&self, event: &EffectivenessLedgerRecord) -> Result<()>;
    fn query_effectiveness_ledger(&self, window: &str, tool_filter: Option<&str>, limit: usize) -> Result<EffectivenessSummary>;
    fn delete_bench_sandbox_records(&self) -> Result<usize>;

    // ─── Mesh Protocol v1.0 Dialogues & Sandboxed Transfers (Phase 41) ──
    fn insert_mesh_dialogue(&self, rec: &MeshDialogueRecord) -> Result<()> {
        let _ = rec;
        Ok(())
    }
    fn list_mesh_dialogues(&self, limit: usize) -> Result<Vec<MeshDialogueRecord>> {
        let _ = limit;
        Ok(Vec::new())
    }
    fn update_mesh_dialogue_status(&self, _dialogue_id: &str, _status: &str) -> Result<bool> {
        Ok(false)
    }
    fn insert_mesh_transfer(&self, rec: &MeshTransferRecord) -> Result<()> {
        let _ = rec;
        Ok(())
    }
    fn list_mesh_transfers(&self, limit: usize) -> Result<Vec<MeshTransferRecord>> {
        let _ = limit;
        Ok(Vec::new())
    }
    fn update_mesh_transfer_status(
        &self,
        _transfer_id: &str,
        _status: &str,
        _accepted_path: Option<&str>,
    ) -> Result<bool> {
        Ok(false)
    }

    // ─── Dual-Domain Behavioral Audit Events (Phase 43) ───────────────
    fn insert_audit_event(&self, rec: &AuditEventRecord) -> Result<()> {
        let _ = rec;
        Ok(())
    }
    fn query_audit_events(&self, filter: &AuditQueryFilter) -> Result<Vec<AuditEventRecord>> {
        let _ = filter;
        Ok(Vec::new())
    }
    fn audit_domain_summary(&self, window: &str) -> Result<AuditDomainSummary> {
        let _ = window;
        Ok(AuditDomainSummary::default())
    }
    fn prune_old_audit_events(&self, before_iso: &str) -> Result<usize> {
        let _ = before_iso;
        Ok(0)
    }

    // ─── Agent Embodiment Cross-Brain Continuity Capsules (Phase 44) ─────
    fn insert_body_checkpoint(&self, rec: &BodyCheckpointRecord) -> Result<()> {
        let _ = rec;
        Ok(())
    }
    fn get_latest_active_body_checkpoint(&self, body_id: &str) -> Result<Option<BodyCheckpointRecord>> {
        let _ = body_id;
        Ok(None)
    }
    fn list_body_checkpoints(&self, status_filter: Option<&str>, limit: usize) -> Result<Vec<BodyCheckpointRecord>> {
        let _ = (status_filter, limit);
        Ok(Vec::new())
    }
    fn mark_body_checkpoint_resumed(&self, checkpoint_id: &str, by_brain: &str, resumed_at: &str) -> Result<bool> {
        let _ = (checkpoint_id, by_brain, resumed_at);
        Ok(false)
    }
}
