use std::time::Instant;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use crate::storage::Storage;

/// 跨存储后端数据迁移报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationReport {
    pub source_provider: String,
    pub target_provider: String,
    pub placement_rules_migrated: usize,
    pub scan_policies_migrated: usize,
    pub assets_migrated: usize,
    #[serde(default)]
    pub asset_embeddings_migrated: usize,
    pub active_processes_migrated: usize,
    pub memories_migrated: usize,
    pub changes_migrated: usize,
    pub links_migrated: usize,
    pub scan_jobs_migrated: usize,
    pub fact_nodes_migrated: usize,
    #[serde(default)]
    pub fact_node_embeddings_migrated: usize,
    pub fact_edges_migrated: usize,
    #[serde(default)]
    pub audit_events_migrated: usize,
    #[serde(default)]
    pub body_checkpoints_migrated: usize,
    #[serde(default)]
    pub coordination_locks_migrated: usize,
    #[serde(default)]
    pub lock_wait_queue_migrated: usize,
    #[serde(default)]
    pub mcp_catalog_migrated: usize,
    #[serde(default)]
    pub skills_catalog_migrated: usize,
    #[serde(default)]
    pub external_services_migrated: usize,
    #[serde(default)]
    pub benchmark_runs_migrated: usize,
    #[serde(default)]
    pub effectiveness_ledger_migrated: usize,
    #[serde(default)]
    pub mesh_dialogues_migrated: usize,
    #[serde(default)]
    pub mesh_transfers_migrated: usize,
    pub duration_ms: u128,
}

/// 跨存储引擎热迁移器 (Storage Migrator)
/// 支撑在 SQLite、PostgreSQL 及 DuckDB 间无损双向迁移全量实体、向量与图谱数据
pub struct StorageMigrator;

impl StorageMigrator {
    /// 执行两个存储提供者之间的全量数据热迁移（幂等可重入）
    pub fn migrate(source: &Storage, target: &Storage) -> Result<MigrationReport> {
        let start = Instant::now();

        // 确保目标库已完成 Schema 初始化
        target.init_schema().context("初始化目标库Schema失败")?;

        // 1. 迁移 Placement Rules
        let rules = source.get_placement_rules()?;
        let mut rules_count = 0;
        for rule in &rules {
            target.insert_placement_rule(rule)?;
            rules_count += 1;
        }

        // 2. 迁移扫描策略 (覆盖含用户自定义的 Scan Policies)
        let policies = source.get_scan_policies()?;
        let mut policies_count = 0;
        for policy in &policies {
            target.upsert_scan_policy(policy)?;
            policies_count += 1;
        }

        // 3. 迁移 Assets (全量资产与元数据 + 非空向量)
        let assets = source.list_assets(1_000_000)?;
        let mut assets_count = 0;
        let mut asset_embeddings_count = 0;
        for asset in &assets {
            target.upsert_asset(asset)?;
            assets_count += 1;
            let emb = source.get_asset_embedding(&asset.path)?;
            if !emb.is_empty() {
                target.update_asset_embedding(&asset.path, &emb)?;
                asset_embeddings_count += 1;
            }
        }

        // 4. 迁移 Active Processes
        let procs = source.list_active_processes()?;
        let procs_count = procs.len();
        if !procs.is_empty() {
            target.replace_active_processes(&procs)?;
        }

        // 5. 迁移 Memories (全状态记忆含 archived/outdated 与向量)
        let memories = source.list_all_memories(1_000_000)?;
        let mut memories_count = 0;
        for mem in &memories {
            target.insert_memory(mem)?;
            memories_count += 1;
        }

        // 6. 迁移 Changes (变更审计轨迹)
        let changes = source.list_changes(1_000_000)?;
        let mut changes_count = 0;
        for change in &changes {
            target.record_change(change)?;
            changes_count += 1;
        }

        // 7. 迁移 Links (关联网络与行号溯源)
        let links = source.list_all_links(1_000_000)?;
        let mut links_count = 0;
        for link in &links {
            target.add_link(link)?;
            links_count += 1;
        }

        // 8. 迁移 Scan Jobs (断点断续作业)
        let jobs = source.list_scan_jobs(1_000_000)?;
        let mut jobs_count = 0;
        for job in &jobs {
            if target.get_scan_job(&job.id)?.is_some() {
                target.update_scan_job(job)?;
            } else {
                target.create_scan_job(job)?;
            }
            jobs_count += 1;
        }

        // 9. 迁移 Fact Graph Nodes & Edges (宿主机属性图 + 非空向量)
        let nodes = source.list_all_fact_nodes(1_000_000)?;
        let mut nodes_count = 0;
        let mut node_embeddings_count = 0;
        for node in &nodes {
            target.upsert_fact_node(node)?;
            nodes_count += 1;
            let emb = source.get_fact_node_embedding(&node.id)?;
            if !emb.is_empty() {
                target.update_fact_node_embedding(&node.id, &emb)?;
                node_embeddings_count += 1;
            }
        }

        let edges = source.list_all_fact_edges(1_000_000)?;
        let mut edges_count = 0;
        for edge in &edges {
            target.upsert_fact_edge(edge)?;
            edges_count += 1;
        }

        // 10. 迁移 Audit Events (双域全链路行为审计事件)
        let audits = source.query_audit_events(&crate::storage::models::AuditQueryFilter {
            limit: 100_000,
            ..Default::default()
        })?;
        let mut audit_count = 0;
        for aud in &audits {
            target.insert_audit_event(aud)?;
            audit_count += 1;
        }

        // 11. 迁移 Body Checkpoints (Agent 本体跨大脑连续性接力胶囊)
        let checkpoints = source.list_body_checkpoints(None, 100_000)?;
        let mut checkpoints_count = 0;
        for cp in &checkpoints {
            target.insert_body_checkpoint(cp)?;
            checkpoints_count += 1;
        }

        // 12. 迁移多 Agent 协作锁与等待队列
        let locks = source.list_coordination_locks(None)?;
        let mut locks_count = 0;
        for lock in &locks {
            if target.insert_coordination_lock(lock)? {
                locks_count += 1;
            }
        }

        let wait_entries = source.list_lock_wait_queue(None, None)?;
        let mut wait_count = 0;
        for entry in &wait_entries {
            target.enqueue_lock_wait(entry)?;
            wait_count += 1;
        }

        // 13. 迁移生态认知目录 (MCP / Skills / External Services)
        let mcps = source.list_mcp_catalog()?;
        let mut mcp_count = 0;
        for rec in &mcps {
            target.upsert_mcp_catalog(rec)?;
            mcp_count += 1;
        }

        let skills = source.list_skills_catalog()?;
        let mut skills_count = 0;
        for rec in &skills {
            target.upsert_skill_catalog(rec)?;
            skills_count += 1;
        }

        let ext_svcs = source.list_external_services(None)?;
        let mut ext_svc_count = 0;
        for rec in &ext_svcs {
            target.upsert_external_service(rec)?;
            ext_svc_count += 1;
        }

        // 14. 迁移 Benchmark Runs (含 AWR snapshots/reports 内部元数据) 与效能存证账本
        let runs = source.list_all_benchmark_runs(100_000)?;
        let mut runs_count = 0;
        for run in &runs {
            target.insert_benchmark_run(run)?;
            runs_count += 1;
        }

        let ledger = source.query_effectiveness_ledger("all", None, 100_000)?;
        let mut ledger_count = 0;
        for ev in &ledger.recent_events {
            target.insert_effectiveness_event(ev)?;
            ledger_count += 1;
        }

        // 15. 迁移联邦网格跨机对话与沙盒文件投递记录
        let dialogues = source.list_mesh_dialogues(100_000)?;
        let mut dialogues_count = 0;
        for dlg in &dialogues {
            target.insert_mesh_dialogue(dlg)?;
            dialogues_count += 1;
        }

        let transfers = source.list_mesh_transfers(100_000)?;
        let mut transfers_count = 0;
        for tr in &transfers {
            target.insert_mesh_transfer(tr)?;
            transfers_count += 1;
        }

        let duration_ms = start.elapsed().as_millis();

        Ok(MigrationReport {
            source_provider: source.provider_name().to_string(),
            target_provider: target.provider_name().to_string(),
            placement_rules_migrated: rules_count,
            scan_policies_migrated: policies_count,
            assets_migrated: assets_count,
            asset_embeddings_migrated: asset_embeddings_count,
            active_processes_migrated: procs_count,
            memories_migrated: memories_count,
            changes_migrated: changes_count,
            links_migrated: links_count,
            scan_jobs_migrated: jobs_count,
            fact_nodes_migrated: nodes_count,
            fact_node_embeddings_migrated: node_embeddings_count,
            fact_edges_migrated: edges_count,
            audit_events_migrated: audit_count,
            body_checkpoints_migrated: checkpoints_count,
            coordination_locks_migrated: locks_count,
            lock_wait_queue_migrated: wait_count,
            mcp_catalog_migrated: mcp_count,
            skills_catalog_migrated: skills_count,
            external_services_migrated: ext_svc_count,
            benchmark_runs_migrated: runs_count,
            effectiveness_ledger_migrated: ledger_count,
            mesh_dialogues_migrated: dialogues_count,
            mesh_transfers_migrated: transfers_count,
            duration_ms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::*;
    use std::sync::Arc;
    use crate::storage::pg_provider::PgStorageProvider;

    #[test]
    fn test_storage_migration_sqlite_to_sqlite() {
        let src = Storage::open_in_memory().expect("src in memory");
        let dst = Storage::open_in_memory().expect("dst in memory");

        // 注入源端数据
        let asset = AssetRecord {
            id: "mig-test-asset".to_string(),
            path: "/root/new/shared_proj/migration_target".to_string(),
            asset_type: "script".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("迁移测试脚本".to_string()),
            desc_kv: serde_json::json!({"engine": "migration"}),
            usage: Some("cargo run".to_string()),
            size_bytes: Some(1024),
            file_count: Some(1),
            tags: vec!["migration".to_string()],
            status: "active".to_string(),
        };
        src.upsert_asset(&asset).expect("upsert asset");

        let node = FactNodeRecord {
            id: "service:mig_daemon".to_string(),
            node_type: "service".to_string(),
            name: "mig_daemon".to_string(),
            label: "迁移守护进程".to_string(),
            properties: serde_json::json!({"env": "test"}),
            updated_at: None,
        };
        src.upsert_fact_node(&node).expect("upsert fact node");

        let report = StorageMigrator::migrate(&src, &dst).expect("migrate");
        assert_eq!(report.source_provider, "sqlite");
        assert_eq!(report.target_provider, "sqlite");
        assert!(report.assets_migrated >= 1);
        assert!(report.fact_nodes_migrated >= 1);

        // 验证目标端
        let dst_asset = dst.get_asset_by_path("/root/new/shared_proj/migration_target")
            .expect("get dst asset")
            .expect("must exist in dst");
        assert_eq!(dst_asset.id, "mig-test-asset");

        let dst_node = dst.get_fact_node("service:mig_daemon")
            .expect("get dst node")
            .expect("must exist in dst");
        assert_eq!(dst_node.name, "mig_daemon");
    }

    #[test]
    fn test_storage_migration_sqlite_to_real_postgres() {
        let pg_url = std::env::var("AGENT_BODY_TEST_PG_URL")
            .unwrap_or_else(|_| "postgresql://postgres@127.0.0.1:5432/agent_body_test".to_string());
        let pg_provider = match PgStorageProvider::connect(&pg_url) {
            Ok(p) => p,
            Err(_) => {
                eprintln!("跳过真实 PG 迁移测试 (本地 PG 暂不可用)");
                return;
            }
        };
        let dst = Storage::new(Arc::new(pg_provider));
        let src = Storage::open_in_memory().expect("src in memory");

        let uid = uuid::Uuid::new_v4();
        let test_path = format!("/root/new/shared_proj/mig_pg_{}", uid);
        let asset = AssetRecord {
            id: format!("asset-{}", uid),
            path: test_path.clone(),
            asset_type: "project".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("跨引擎热迁移测试库".to_string()),
            desc_kv: serde_json::json!({"test_id": uid.to_string()}),
            usage: Some("git status".to_string()),
            size_bytes: Some(2048),
            file_count: Some(5),
            tags: vec!["cross_engine".to_string(), "migration".to_string()],
            status: "active".to_string(),
        };
        src.upsert_asset(&asset).expect("upsert to sqlite src");

        let report = StorageMigrator::migrate(&src, &dst).expect("migrate sqlite -> pg");
        assert_eq!(report.source_provider, "sqlite");
        assert_eq!(report.target_provider, "postgres");
        assert!(report.assets_migrated >= 1);

        // 验证在真实 PostgreSQL 中检索该记录
        let fetched_pg = dst.get_asset_by_path(&test_path)
            .expect("fetch from pg")
            .expect("must exist in pg");
        assert_eq!(fetched_pg.id, format!("asset-{}", uid));
        assert_eq!(fetched_pg.owner, "user:digoal");
    }
}
