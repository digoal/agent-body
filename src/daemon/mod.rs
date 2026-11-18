use std::path::PathBuf;
use std::time::Duration;
use anyhow::Result;
use tokio::time::interval;
use crate::config::Config;
use crate::storage::Storage;
use crate::env::EnvCollector;
use crate::watcher::FileWatcher;
use crate::watcher::handler::ChangeHandler;

pub mod scheduler;
pub use scheduler::{BackgroundScheduler, SchedulerCycleReport};

pub struct StewardDaemon {
    config: Config,
    storage: Storage,
}

impl StewardDaemon {
    pub fn new(config: Config, storage: Storage) -> Self {
        Self { config, storage }
    }

    /// 启动常驻后台守护进程主循环
    pub async fn run(&self) -> Result<()> {
        println!("╔═══════════════════════════════════════════════════════╗");
        println!("║      Agent Host Steward Daemon 已在后台启动运行       ║");
        println!("╚═══════════════════════════════════════════════════════╝");
        println!("数据目录:       {:?}", self.config.data_dir);
        println!("数据库路径:     {:?}", self.config.db_path);

        // 0. 执行运行时环境依赖预检 (Preflight)
        let preflight = crate::preflight::PreflightChecker::run_all(&self.config.data_dir, Some(&self.config));
        if !preflight.passed_required {
            eprintln!("{}", preflight.format_table());
            anyhow::bail!("Preflight 检查失败：存在 REQUIRED 关键运行依赖未满足，守护进程终止启动。");
        } else {
            for item in &preflight.items {
                if item.status == crate::preflight::CheckStatus::Warn {
                    println!("⚠️ [Preflight 降级提示] {}: {} (建议: {:?})", item.name, item.message, item.remedy.as_deref().unwrap_or("无"));
                }
            }
        }

        // 1. 初始化文件监听目录
        let mut watch_paths = self.config.assets.auto_scan_dirs.clone();
        if watch_paths.is_empty() {
            // 默认监听核心工作目录
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            let default_work = PathBuf::from(&home).join("new").join("work");
            let default_proj = PathBuf::from(&home).join("new").join("shared_proj");
            if default_work.exists() { watch_paths.push(default_work); }
            if default_proj.exists() { watch_paths.push(default_proj); }
        }

        println!("实时监听目录:   {:?}", watch_paths);

        let watcher = match FileWatcher::new(&watch_paths) {
            Ok(w) => {
                println!("✅ inotify 文件系统实时监听就绪 (300ms 防抖)");
                Some(w)
            }
            Err(e) => {
                eprintln!("⚠️ 初始化文件监听失败 (降级为纯定时主动扫描): {}", e);
                None
            }
        };

        let handler = ChangeHandler::new(self.storage.clone())?;
        let env_collector = EnvCollector::new(self.storage.clone());
        let bg_scheduler = BackgroundScheduler::new(self.storage.clone());

        // 2. 定时器
        let mut process_interval = interval(Duration::from_secs(60));
        let mut watch_poll_interval = interval(Duration::from_millis(500));
        let mut inspection_interval = interval(Duration::from_secs(1800)); // 每 30 分钟一次深度全量巡检

        println!("✅ 动态任务进程定时采集就绪 (每 60 秒轮询 /proc)");
        println!("✅ 后台自主深度巡查与属性图增量同步就绪 (每 1800 秒自动巡查)");
        println!("按 Ctrl+C 优雅退出...\n");

        loop {
            tokio::select! {
                // 监听文件系统变更事件
                _ = watch_poll_interval.tick() => {
                    if let Some(ref w) = watcher {
                        let events = w.poll_events();
                        if !events.is_empty() {
                            if let Ok(count) = handler.process_events(&events) {
                                if count > 0 {
                                    println!("[Watcher] 自动感知并增量同步 {} 个文件变更事件", count);
                                }
                            }
                        }
                    }
                }

                // 定时动态进程扫描与同步
                _ = process_interval.tick() => {
                    if let Ok(procs) = env_collector.collect_and_sync_processes() {
                        // 仅静默同步，若有高负载任务或异常可在此告警
                        let _ = procs;
                    }
                }

                // 定时自主深度巡查与属性图同步
                _ = inspection_interval.tick() => {
                    if let Ok(report) = bg_scheduler.run_inspection_cycle() {
                        println!("[Scheduler] 深度巡检完成: 同步进程 {}, 属性图节点更新 {}, 边更新 {}",
                            report.synced_processes, report.graph_nodes_updated, report.graph_edges_updated);
                    }
                }

                // 捕捉退出信号
                _ = tokio::signal::ctrl_c() => {
                    println!("\n[Steward] 收到停机信号，正在保存状态并安全退出...");
                    break;
                }
            }
        }

        Ok(())
    }
}
