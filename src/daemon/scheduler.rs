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
}

/// 后台巡查与增量调度引擎
pub struct BackgroundScheduler {
    storage: Storage,
}

impl BackgroundScheduler {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// 执行一次管家深度环境巡检与事实图谱同步循环
    pub fn run_inspection_cycle(&self) -> Result<SchedulerCycleReport> {
        // 1. 动态进程与端口采集
        let env_collector = EnvCollector::new(self.storage.clone());
        let synced_processes = env_collector.collect_and_sync_processes()
            .map(|v| v.len())
            .unwrap_or(0);

        // 2. 事实属性图增量构建与刷新
        let organizer = PropertyGraphOrganizer::new(self.storage.clone());
        let (graph_nodes, graph_edges) = organizer.build_host_graph()
            .unwrap_or((0, 0));

        // 3. 记忆生命周期健康度审计
        let lifecycle = MemoryLifecycleManager::new(&self.storage);
        let memory_conflicts = lifecycle.audit_memories()
            .map(|r| r.conflict_count)
            .unwrap_or(0);

        // 4. 若有挂起或未完成的断点扫描任务，自动推进恢复执行
        let mut scan_job_resumed = false;
        if let Ok(Some(active_job)) = self.storage.get_active_scan_job() {
            if active_job.status == "paused" || active_job.status == "pending" {
                let sm = ScanStateMachine::new(self.storage.clone())?;
                // 每次后台推进 200 个条目，平滑消耗资源防高负载
                let _ = sm.run_or_resume(&active_job.id, Some(200));
                scan_job_resumed = true;
            }
        }

        // 5. 过期协作锁自动回收 (Coordination GC)
        let coord = crate::coordination::CoordinationEngine::new(self.storage.provider().clone());
        let expired_locks_cleaned = coord.gc_expired_locks().unwrap_or(0);

        // 6. 自动刷新宿主机 MCP 与 SKILL 生态目录 (Phase 16)
        let mcp_scanner = crate::ecosystem::McpConfigScanner::new(self.storage.clone());
        let _ = mcp_scanner.scan_default_clients();
        let skill_scanner = crate::ecosystem::SkillCatalogScanner::new(self.storage.clone());
        let _ = skill_scanner.scan_default_roots();

        Ok(SchedulerCycleReport {
            synced_processes,
            graph_nodes_updated: graph_nodes,
            graph_edges_updated: graph_edges,
            memory_conflicts_found: memory_conflicts,
            scan_job_resumed,
            expired_locks_cleaned,
        })
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
