use std::sync::Arc;
use std::time::Duration;
use anyhow::Result;
use crate::storage::Storage;
use crate::env::EnvCollector;
use crate::links::audit::LinkAuditor;

pub struct TuiDashboard {
    storage: Arc<Storage>,
    env: EnvCollector,
    auditor: LinkAuditor,
}

impl TuiDashboard {
    pub fn new(storage: Arc<Storage>) -> Self {
        let env = EnvCollector::new((*storage).clone());
        let auditor = LinkAuditor::new(storage.clone());
        Self { storage, env, auditor }
    }

    /// 生成单帧全景终端看板文本 (支持 ANSI 配色)
    pub fn render_frame(&self) -> String {
        let mut out = String::new();

        // 1. 采集系统信息 + 本体稳态/遥测/审计/接力/锁 (Phase 54)
        let sys = self.env.collect_system_summary();
        let procs = self.storage.list_active_processes().unwrap_or_default();
        let audit = self.auditor.audit_links(false).unwrap_or(crate::links::audit::LinkAuditReport {
            total_links: 0,
            health_score: 100,
            dangling_count: 0,
            fixed_count: 0,
            issues: vec![],
            summary: "正常".to_string(),
        });
        let changes = self.storage.list_changes(5).unwrap_or_default();
        let memories = self.storage.list_all_active_memories(3).unwrap_or_default();
        let vitals = crate::body::HomeostasisVitalsEngine::assess(&self.storage);
        let kernel = crate::body::KernelSelfTelemetry::collect(None);
        let wait_rows = crate::body::WaitEventRegistry::snapshot_all();
        let (snap_metas, baseline_meta) =
            crate::body::BodyPerfEngine::list_snapshots_meta(self.storage.provider().as_ref());
        let report_metas =
            crate::body::BodyPerfEngine::list_awr_reports_meta(self.storage.provider().as_ref());
        let audit_sum = self
            .storage
            .audit_domain_summary("24h")
            .unwrap_or_default();
        let checkpoints = self
            .storage
            .list_body_checkpoints(None, 5)
            .unwrap_or_default();
        let active_locks = self
            .storage
            .list_coordination_locks(None)
            .unwrap_or_default();
        let expired_locks = self
            .storage
            .count_expired_coordination_locks()
            .unwrap_or(0);

        // ANSI 颜色代码
        let c_bold = "\x1b[1m";
        let c_cyan = "\x1b[36m";
        let c_green = "\x1b[32m";
        let c_yellow = "\x1b[33m";
        let c_magenta = "\x1b[35m";
        let c_reset = "\x1b[0m";

        out.push_str(&format!(
            "{}┌────────────────────────────────────────────────────────────────────────────────────────┐{}\n",
            c_cyan, c_reset
        ));
        out.push_str(&format!(
            "{}│{:^88}│{}\n",
            c_bold,
            format!("AGENT BODY — 宿主机具身底座全景看板 (v{})", crate::VERSION),
            c_reset
        ));
        out.push_str(&format!(
            "{}├────────────────────────────────────────────────────────────────────────────────────────┤{}\n",
            c_cyan, c_reset
        ));

        // 硬件指标
        let mem_bar_len = 20;
        let filled_len = ((sys.memory_used_pct / 100.0) * mem_bar_len as f32).round() as usize;
        let mem_bar: String = "█".repeat(filled_len.min(mem_bar_len)) + &"░".repeat(mem_bar_len.saturating_sub(filled_len));

        out.push_str(&format!(
            "│ {}系统硬件{}: {} | 核数: {} ({} 核) | 主机: {}\n",
            c_green, c_reset, sys.os_name, sys.cpu_cores, sys.cpu_brand, sys.host_name
        ));
        out.push_str(&format!(
            "│ {}内存使用{}: [{}{}{}] {:.1}% ({} MB / {} MB)\n",
            c_green, c_reset, c_yellow, mem_bar, c_reset, sys.memory_used_pct, sys.used_memory_mb, sys.total_memory_mb
        ));

        // 本体五维稳态体征
        let vital_color = if vitals.overall_score >= 85.0 { c_green } else { c_yellow };
        let dim_brief: String = vitals
            .dimensions
            .iter()
            .map(|d| format!("{}={:.0}", d.label.chars().take(2).collect::<String>(), d.score))
            .collect::<Vec<_>>()
            .join(" ");
        out.push_str(&format!(
            "│ {}稳态体征{}: {}{:.1}/100 ({}){} | {}\n",
            c_cyan, c_reset, vital_color, vitals.overall_score, vitals.status, c_reset, dim_brief
        ));

        // 内核 RSS / FDs / WaitEvents & AWR 遥测
        let top_waits: String = wait_rows
            .iter()
            .filter(|w| w.calls > 0)
            .take(3)
            .map(|w| format!("{}({}μs)", w.event_name, w.total_us))
            .collect::<Vec<_>>()
            .join(", ");
        let waits_display = if top_waits.is_empty() {
            "0 活跃等待".to_string()
        } else {
            top_waits
        };
        out.push_str(&format!(
            "│ {}AWR遥测{}: RSS={}KB HWM={}KB FDs={} | 快照: {} (基线:{}) 报告: {} | TopWait: {}\n",
            c_cyan,
            c_reset,
            kernel.rss_kb,
            kernel.hwm_kb,
            kernel.open_fds,
            snap_metas.len(),
            if baseline_meta.is_some() { "✓" } else { "-" },
            report_metas.len(),
            waits_display
        ));

        // 双域审计、接力胶囊与协作锁状态
        let cap_hint = checkpoints
            .first()
            .map(|c| {
                let goal: String = c.goal_summary.chars().take(28).collect();
                format!("{}[{}]: {}", c.from_brain, c.status, goal)
            })
            .unwrap_or_else(|| "无接力胶囊".to_string());
        out.push_str(&format!(
            "│ {}双域审计{}: 24h={} (IN:{} OUT:{} 阻/败:{}) | {}接力胶囊{}: {}个 ({})\n",
            c_magenta,
            c_reset,
            audit_sum.total_events,
            audit_sum.in_agent_count,
            audit_sum.out_of_agent_count,
            audit_sum.failed_or_blocked_count,
            c_cyan,
            c_reset,
            checkpoints.len(),
            cap_hint
        ));
        out.push_str(&format!(
            "│ {}协作锁{}: 活跃 {} 把 / 过期 {} 把 | {}依赖拓扑{}: 边={} 健康={}{}/100{} 断链={}\n",
            c_yellow,
            c_reset,
            active_locks.len(),
            expired_locks,
            c_magenta,
            c_reset,
            audit.total_links,
            if audit.health_score >= 80 { c_green } else { c_yellow },
            audit.health_score,
            c_reset,
            audit.dangling_count
        ));

        // 三位一体情境组装中枢状态
        out.push_str(&format!(
            "│ {}情境中枢{}: 事实图谱: {}就绪{} | 偏好规约: {}生效{} | 记忆引擎: {}内置+适配器{} | 简报预算: ≤1500t\n",
            c_cyan, c_reset, c_green, c_reset, c_green, c_reset, c_green, c_reset
        ));

        out.push_str(&format!(
            "{}├────────────────────────────────────────┬───────────────────────────────────────────────┤{}\n",
            c_cyan, c_reset
        ));

        // 左右分栏: 左侧进程/端口，右侧最新变更与记忆
        out.push_str(&format!(
            "│ {}活跃 Agent 任务与进程 (Top 5){}         │ {}近期环境变更历史 (Recent Changes){}            │\n",
            c_bold, c_reset, c_bold, c_reset
        ));

        let max_rows = 5;
        for i in 0..max_rows {
            // 左列
            let left_str = if i < procs.len() {
                let p = &procs[i];
                let cmd_trunc: String = p.command.chars().take(22).collect();
                format!("PID {:<5} {:<10} {:<15}", p.pid, p.name, cmd_trunc)
            } else if i == 0 && procs.is_empty() {
                "无独立运行的 Agent 任务进程".to_string()
            } else {
                "".to_string()
            };

            // 右列
            let right_str = if i < changes.len() {
                let ch = &changes[i];
                let desc_val = ch.description.as_deref().unwrap_or("");
                let desc: String = desc_val.chars().take(35).collect();
                format!("{:<10} {}", ch.change_type, desc)
            } else if i == 0 && changes.is_empty() {
                "暂无近期变更记录".to_string()
            } else {
                "".to_string()
            };

            out.push_str(&format!("│ {:<38} │ {:<45} │\n", left_str, right_str));
        }

        out.push_str(&format!(
            "{}├────────────────────────────────────────────────────────────────────────────────────────┤{}\n",
            c_cyan, c_reset
        ));

        // 底部: 长期认知记忆条目
        out.push_str(&format!("│ {}最新沉淀长期记忆{}: (共活跃 {} 条)\n", c_bold, c_reset, memories.len()));
        if memories.is_empty() {
            out.push_str("│   (暂无沉淀长期记忆)\n");
        } else {
            for m in &memories {
                let text: String = m.content.chars().take(75).collect();
                out.push_str(&format!("│   - [{}] {}\n", m.memory_type, text));
            }
        }

        out.push_str(&format!(
            "{}└────────────────────────────────────────────────────────────────────────────────────────┘{}\n",
            c_cyan, c_reset
        ));

        out
    }

    /// 运行周期刷新循环 (若指定 max_ticks 则在指定轮数后退出)
    pub async fn run_loop(&self, interval_secs: u64, max_ticks: Option<usize>) -> Result<()> {
        let mut ticks = 0;
        loop {
            // 清屏并将光标移至左上角
            print!("\x1b[2J\x1b[1;1H");
            print!("{}", self.render_frame());

            ticks += 1;
            if let Some(max) = max_ticks {
                if ticks >= max {
                    break;
                }
            }

            tokio::time::sleep(Duration::from_secs(interval_secs)).await;
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_tui_render_frame() -> Result<()> {
        let dir = tempdir()?;
        let db_path = dir.path().join("test_tui.db");
        let storage = Arc::new(Storage::open(&db_path)?);

        let dashboard = TuiDashboard::new(storage);
        let frame = dashboard.render_frame();

        // 验证关键 section 存在
        assert!(frame.contains("AGENT BODY"));
        assert!(frame.contains(crate::VERSION));
        assert!(frame.contains("系统硬件"));
        assert!(frame.contains("内存使用"));
        assert!(frame.contains("稳态体征"));
        assert!(frame.contains("AWR遥测"));
        assert!(frame.contains("双域审计"));
        assert!(frame.contains("接力胶囊"));
        assert!(frame.contains("协作锁"));
        assert!(frame.contains("依赖拓扑"));
        assert!(frame.contains("情境中枢"));
        assert!(frame.contains("活跃 Agent 任务与进程"));

        // 验证 run_loop 在 1 tick 后正常退出
        dashboard.run_loop(1, Some(1)).await?;

        Ok(())
    }
}
