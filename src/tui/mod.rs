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

        // 1. 采集系统信息
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
            "{}│              🤖 AGENT HOST STEWARD — 宿主环境实时全景看板 (v0.3.0)                    │{}\n",
            c_bold, c_reset
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

        // 图拓扑与健康分
        let score_color = if audit.health_score >= 80 { c_green } else { c_yellow };
        out.push_str(&format!(
            "│ {}依赖拓扑{}: 总边数: {} | 拓扑健康分: {}{}/100{} | 异常断链: {}\n",
            c_magenta, c_reset, audit.total_links, score_color, audit.health_score, c_reset, audit.dangling_count
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
        assert!(frame.contains("AGENT HOST STEWARD"));
        assert!(frame.contains("系统硬件"));
        assert!(frame.contains("内存使用"));
        assert!(frame.contains("依赖拓扑"));
        assert!(frame.contains("活跃 Agent 任务与进程"));

        // 验证 run_loop 在 1 tick 后正常退出
        dashboard.run_loop(1, Some(1)).await?;

        Ok(())
    }
}
