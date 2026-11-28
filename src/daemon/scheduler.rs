use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::storage::Storage;
use crate::env::EnvCollector;
use crate::fact::property_graph::PropertyGraphOrganizer;
use crate::memory::lifecycle::MemoryLifecycleManager;
use crate::scanner::state_machine::ScanStateMachine;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerCycleReport {
    pub synced_processes: usize,
    pub graph_nodes_updated: usize,
    pub graph_edges_updated: usize,
    pub memory_conflicts_found: usize,
    pub scan_job_resumed: bool,
    pub expired_locks_cleaned: usize,
    pub assets_enriched: usize,
    #[serde(default)]
    pub homeostasis_score: f64,
    #[serde(default)]
    pub homeostasis_healed: bool,
    #[serde(default)]
    pub awr_snapshot_captured: bool,
    #[serde(default)]
    pub sleep_events_compacted: usize,
}

/// 后台巡查与增量调度引擎
pub struct BackgroundScheduler {
    storage: Storage,
}

impl BackgroundScheduler {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// 执行一次Agent 本体深度环境巡检与事实图谱同步循环
    pub fn run_inspection_cycle(&self) -> Result<SchedulerCycleReport> {
        let t0 = std::time::Instant::now();
        // 1. 动态进程与端口采集
        let env_collector = EnvCollector::new(self.storage.clone());
        let synced_processes = env_collector.collect_and_sync_processes()
            .map(|v| v.len())
            .unwrap_or(0);

        // 2. 事实属性图增量构建与刷新
        let organizer = PropertyGraphOrganizer::new(self.storage.clone());
        let (graph_nodes, graph_edges) = organizer.build_host_graph()
            .unwrap_or((0, 0));
        let _fact_embedded = organizer.embed_fact_nodes(40).unwrap_or(0);

        // 3. 记忆生命周期健康度审计 & 向量补全
        let lifecycle = MemoryLifecycleManager::new(&self.storage);
        let memory_conflicts = lifecycle.audit_memories()
            .map(|r| r.conflict_count)
            .unwrap_or(0);
        match crate::memory::MemoryManager::new(self.storage.clone()).backfill_missing_embeddings(20) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("[Scheduler][WARN] 记忆向量补全失败: {}", e);
            }
        }

        // 4. 若有挂起或未完成的断点扫描任务，自动推进恢复执行
        let mut scan_job_resumed = false;
        if let Ok(Some(active_job)) = self.storage.get_active_scan_job() {
            if active_job.status == "paused" || active_job.status == "pending" {
                let sm = ScanStateMachine::new(self.storage.clone())?;
                // 每次后台推进 200 个条目，平滑消耗资源防高负载
                if let Err(e) = sm.run_or_resume(&active_job.id, Some(200)) {
                    eprintln!("[Scheduler][WARN] 恢复扫描任务 {} 失败: {}", active_job.id, e);
                }
                scan_job_resumed = true;
            }
        }

        // 5. 过期协作锁自动回收 (Coordination GC)
        let coord = crate::coordination::CoordinationEngine::new(self.storage.provider().clone());
        let expired_locks_cleaned = coord.gc_expired_locks().unwrap_or(0);

        // 6. 自动刷新宿主机 MCP 与 SKILL 生态目录 (Phase 16)
        let mcp_scanner = crate::ecosystem::McpConfigScanner::new(self.storage.clone());
        if let Err(e) = mcp_scanner.scan_default_clients() {
            eprintln!("[Scheduler][WARN] MCP 生态目录扫描失败: {}", e);
        }
        let skill_scanner = crate::ecosystem::SkillCatalogScanner::new(self.storage.clone());
        if let Err(e) = skill_scanner.scan_default_roots() {
            eprintln!("[Scheduler][WARN] SKILL 生态目录扫描失败: {}", e);
        }

        // 7. 全生命周期信息新鲜度与现实对账 (规约 #21：清理漂移/失踪资产、幽灵端口节点与重复摘要记忆)
        match crate::feedback::probes::FreshnessReconciler::reconcile(&self.storage) {
            Ok(rep) => {
                if rep.missing_assets_marked > 0
                    || rep.drifted_assets_refreshed > 0
                    || rep.ghost_fact_nodes_pruned > 0
                    || rep.duplicate_memories_archived > 0
                {
                    println!(
                        "[Scheduler][Reconcile] 对账完成: checked={}, missing={}, drifted={}, ghost_pruned={}, dup_mem_archived={}, elapsed={}μs",
                        rep.checked_assets,
                        rep.missing_assets_marked,
                        rep.drifted_assets_refreshed,
                        rep.ghost_fact_nodes_pruned,
                        rep.duplicate_memories_archived,
                        rep.elapsed_us
                    );
                }
            }
            Err(e) => {
                eprintln!("[Scheduler][Reconcile][WARN] 新鲜度对账失败: {}", e);
            }
        }

        // 8. 后台异步增量精炼待标注资产元数据 (Phase 23 Ollama 小模型)
        let cfg = crate::config::Config::load_effective_or_default();
        let assets_enriched = if cfg.annotation.enabled && cfg.annotation.auto_enrich_batch_size > 0 {
            let batch_size = cfg.annotation.auto_enrich_batch_size;
            let annotator = crate::scanner::OllamaAnnotator::new(self.storage.clone(), cfg.annotation.clone());
            let enriched = annotator
                .enrich_pending_assets(batch_size, None)
                .map(|r| r.enriched_count)
                .unwrap_or(0);
            if let Err(e) = annotator.embed_pending_assets(batch_size * 2, None) {
                eprintln!("[Scheduler][WARN] 资产向量补全失败: {}", e);
            }
            enriched
        } else {
            0
        };

        // 8.5 L1_AUTONOMIC 空闲窗口期记忆睡眠整理 (Phase 77 M2)
        let sleep_events_compacted =
            crate::memory::SleepCompactionOrchestrator::maybe_run_idle_cycle(&self.storage, &cfg)
                .map(|r| r.events_compacted)
                .unwrap_or(0);

        // 9. 五维本体稳态体检与 L1 自主自愈反射 (Phase 44)
        let mut vitals = crate::body::HomeostasisVitalsEngine::assess(&self.storage);
        let mut homeostasis_healed = false;
        if vitals.overall_score < 70.0 {
            if let Ok(heal) = crate::body::HomeostasisVitalsEngine::run_l1_autonomic_heal(&self.storage) {
                vitals.overall_score = heal.after_score;
                homeostasis_healed = true;
            }
        }

        // 10. 周期性 AWR 性能快照采样 (>=60 分钟或尚无历史快照时自动留档；若尚无基线则自动设为黄金基线)
        let mut awr_snapshot_captured = false;
        let (snap_metas, baseline_meta) =
            crate::body::BodyPerfEngine::list_snapshots_meta(self.storage.provider().as_ref());
        let snapshot_due = if let Some(newest) = snap_metas.first() {
            chrono::DateTime::parse_from_rfc3339(&newest.captured_at)
                .map(|dt| {
                    chrono::Utc::now()
                        .signed_duration_since(dt.with_timezone(&chrono::Utc))
                        .num_minutes()
                        >= 60
                })
                .unwrap_or(true)
        } else {
            true
        };
        if snapshot_due {
            let set_baseline = baseline_meta.is_none();
            if let Err(e) = crate::body::BodyPerfEngine::capture_snapshot(
                &self.storage,
                Some("daemon-auto"),
                set_baseline,
            ) {
                eprintln!("[Scheduler][PerfAWR][WARN] 周期抓取 AWR 快照失败: {}", e);
            } else {
                awr_snapshot_captured = true;
            }
        }

        let report = SchedulerCycleReport {
            synced_processes,
            graph_nodes_updated: graph_nodes,
            graph_edges_updated: graph_edges,
            memory_conflicts_found: memory_conflicts,
            scan_job_resumed,
            expired_locks_cleaned,
            assets_enriched,
            homeostasis_score: vitals.overall_score,
            homeostasis_healed,
            awr_snapshot_captured,
            sleep_events_compacted,
        };

        crate::audit::AuditLogger::record_best_effort(
            &self.storage,
            crate::audit::AuditEventInput::out_of_agent(
                "daemon_scheduler",
                "daemon",
                "daemon_cycle",
                "run_inspection_cycle",
            )
            .with_target("host", "agent-body-daemon")
            .with_outcome("success")
            .with_elapsed_us(t0.elapsed().as_micros() as i64)
            .with_delta(serde_json::to_value(&report).unwrap_or_default()),
        );

        // 巡检结束后主动归还 glibc 空闲堆页，保障常驻守护进程 RSS 稳定 < 90MB（规约 #9）
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            unsafe extern "C" {
                fn malloc_trim(pad: usize) -> i32;
            }
            unsafe {
                let _trimmed = malloc_trim(0);
            }
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_background_scheduler_cycle() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let scheduler = BackgroundScheduler::new(storage);

        let report = scheduler.run_inspection_cycle().expect("run_cycle");
        // 验证巡检报告结构与指标
        assert!(report.graph_nodes_updated > 0 || report.synced_processes > 0);
    }
}
