use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use anyhow::Result;
use tokio::time::{interval, interval_at, Instant};
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
        println!("║      Agent Body Daemon 已在后台启动运行       ║");
        println!("╚═══════════════════════════════════════════════════════╝");
        println!("数据目录:       {:?}", self.config.data_dir);
        println!("数据库路径:     {:?}", self.config.db_path);

        // 0. 执行运行时环境依赖预检 (Preflight)
        // 在脱离 Tokio 上下文的独立 OS 线程中运行，避免内部 reqwest::blocking 与 Tokio 运行时死锁/忙等
        let preflight_data_dir = self.config.data_dir.clone();
        let preflight_cfg = self.config.clone();
        let (pf_tx, pf_rx) = tokio::sync::oneshot::channel();
        std::thread::spawn(move || {
            let res = crate::preflight::PreflightChecker::run_all(&preflight_data_dir, Some(&preflight_cfg));
            let _ = pf_tx.send(res);
        });
        let preflight = pf_rx
            .await
            .map_err(|_| anyhow::anyhow!("Preflight 检查线程异常中止"))?;
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

        let event_bus = crate::hooks::EventBus::new();
        let event_bus_handle = event_bus.handle();
        let handler = ChangeHandler::with_event_bus(self.storage.clone(), event_bus_handle)?;
        let env_collector = EnvCollector::new(self.storage.clone());
        let bg_scheduler = BackgroundScheduler::new(self.storage.clone());

        // 2. 按需拉起后台 Web 监控大屏，并桥接 EventBus -> SSE 实时事件流
        let sse_broadcaster = if self.config.web.enabled {
            let web_config = crate::web::WebConfig {
                host: self.config.web.host.clone(),
                port: self.config.web.port,
                auth_token: self.config.web.auth_token.clone(),
            };
            let storage_provider = self.storage.provider().clone();
            let server = crate::web::WebServer::new(web_config, storage_provider);
            let broadcaster = server.event_broadcaster();
            crate::hooks::bridge_event_bus_to_sse(event_bus.subscribe(), broadcaster.clone());

            tokio::spawn(async move {
                if let Err(e) = server.run().await {
                    eprintln!("⚠️ [Daemon] Web 监控服务异常退出: {}", e);
                }
            });
            println!("✅ Web 监控大屏服务已拉起 (SSE 事件流已桥接): http://{}:{}", self.config.web.host, self.config.web.port);
            Some(broadcaster)
        } else {
            None
        };

        // 3. 定时器
        let mut process_interval = interval(Duration::from_secs(60));
        let mut watch_poll_interval = interval(Duration::from_millis(500));
        // 首次巡检延迟 10 秒执行，避免与启动阶段抢占资源；之后每 30 分钟一次深度全量巡检
        let mut inspection_interval = interval_at(
            Instant::now() + Duration::from_secs(10),
            Duration::from_secs(1800),
        );
        let inspection_running = Arc::new(AtomicBool::new(false));
        let _ = bg_scheduler; // 保留构造校验，实际每次巡检在独立 OS 线程执行以隔离 reqwest::blocking

        println!("✅ 动态任务进程定时采集就绪 (每 60 秒轮询 /proc)");
        println!("✅ 后台自主深度巡查与属性图增量同步就绪 (每 1800 秒自动巡查)");
        println!("按 Ctrl+C 优雅退出...\n");

        let mut idle_wait_start = std::time::Instant::now();
        loop {
            tokio::select! {
                // 监听文件系统变更事件
                _ = watch_poll_interval.tick() => {
                    crate::body::WaitEventRegistry::record_us(
                        crate::body::WaitEventId::IdleDaemonLoop,
                        idle_wait_start.elapsed().as_micros() as u64,
                    );
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
                    crate::body::WaitEventRegistry::record_us(
                        crate::body::WaitEventId::IdleDaemonLoop,
                        idle_wait_start.elapsed().as_micros() as u64,
                    );
                    if let Ok(procs) = env_collector.collect_and_sync_processes() {
                        if let Some(ref tx) = sse_broadcaster {
                            let msg = serde_json::json!({
                                "type": "ProcessSync",
                                "payload": {
                                    "active_processes": procs.len(),
                                    "timestamp": chrono::Local::now().to_rfc3339(),
                                }
                            });
                            let _ = tx.send(msg.to_string());
                        }
                    }
                }

                // 定时自主深度巡查与属性图同步
                // 必须在脱离 Tokio 运行时的独立 OS 线程中执行，避免内部 reqwest::blocking 导致主线程 100% CPU 忙等死锁
                _ = inspection_interval.tick() => {
                    crate::body::WaitEventRegistry::record_us(
                        crate::body::WaitEventId::IdleDaemonLoop,
                        idle_wait_start.elapsed().as_micros() as u64,
                    );
                    if inspection_running
                        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        let storage_for_cycle = self.storage.clone();
                        let sse_for_cycle = sse_broadcaster.clone();
                        let running_flag = Arc::clone(&inspection_running);
                        std::thread::spawn(move || {
                            let scheduler = BackgroundScheduler::new(storage_for_cycle);
                            if let Ok(report) = scheduler.run_inspection_cycle() {
                                println!(
                                    "[Scheduler] 深度巡检完成: 同步进程 {}, 属性图节点更新 {}, 边更新 {}",
                                    report.synced_processes,
                                    report.graph_nodes_updated,
                                    report.graph_edges_updated
                                );
                                if let Some(ref tx) = sse_for_cycle {
                                    let msg = serde_json::json!({
                                        "type": "SchedulerCycle",
                                        "payload": {
                                            "synced_processes": report.synced_processes,
                                            "graph_nodes": report.graph_nodes_updated,
                                            "graph_edges": report.graph_edges_updated,
                                            "expired_locks_cleaned": report.expired_locks_cleaned,
                                            "awr_snapshot_captured": report.awr_snapshot_captured,
                                            "timestamp": chrono::Local::now().to_rfc3339(),
                                        }
                                    });
                                    let _ = tx.send(msg.to_string());
                                }
                            }
                            running_flag.store(false, Ordering::SeqCst);
                        });
                    }
                }

                // 捕捉退出信号
                _ = tokio::signal::ctrl_c() => {
                    crate::body::WaitEventRegistry::record_us(
                        crate::body::WaitEventId::IdleDaemonLoop,
                        idle_wait_start.elapsed().as_micros() as u64,
                    );
                    println!("\n[Steward] 收到停机信号，正在保存状态并安全退出...");
                    break;
                }
            }
            idle_wait_start = std::time::Instant::now();
        }

        Ok(())
    }
}
