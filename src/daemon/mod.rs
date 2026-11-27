use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use anyhow::Result;
use tokio::time::{interval, interval_at, Instant};
use crate::body::obs_log::emit_obs_log;
use crate::config::Config;
use crate::storage::Storage;
use crate::env::EnvCollector;
use crate::watcher::FileWatcher;
use crate::watcher::handler::ChangeHandler;

pub mod scheduler;
pub use scheduler::{BackgroundScheduler, SchedulerCycleReport};

struct RunningFlagGuard(Arc<AtomicBool>);

impl Drop for RunningFlagGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

pub struct BodyDaemon {
    config: Config,
    storage: Storage,
}

impl BodyDaemon {
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
            if pf_tx.send(res).is_err() {
                eprintln!("[Daemon][WARN] Preflight 结果发送失败：接收端已关闭");
            }
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

        // 1. 初始化文件监听目录（默认优先监听核心工作区 work 与 shared_proj，避免误监听庞大第三方 src/ 与镜像目录）
        let mut watch_paths = self.config.assets.auto_scan_dirs.clone();
        if watch_paths.is_empty() {
            let base = self.config.effective_workspace_base();
            for sub in ["work", "shared_proj"] {
                let p = base.join(sub);
                if p.exists() && !watch_paths.contains(&p) {
                    watch_paths.push(p);
                }
            }
            if watch_paths.is_empty() {
                for p in self.config.effective_workspace_roots() {
                    if p.exists() && !watch_paths.contains(&p) {
                        watch_paths.push(p);
                    }
                }
            }
        }

        println!("实时监听目录:   {:?}", watch_paths);

        let watcher = match FileWatcher::with_ignore_patterns(&watch_paths, &self.config.assets.ignore_patterns) {
            Ok(w) => {
                let watch_kind = match std::env::consts::OS {
                    "macos" => "FSEvents",
                    "windows" => "ReadDirectoryChangesW",
                    _ => "inotify",
                };
                println!("✅ {} 文件系统实时监听就绪 (300ms 防抖)", watch_kind);
                Some(w)
            }
            Err(e) => {
                eprintln!("⚠️ 初始化文件监听失败 (降级为纯定时主动扫描): {}", e);
                None
            }
        };

        let event_bus = crate::hooks::EventBus::new();
        let event_bus_handle = event_bus.handle();
        let handler = ChangeHandler::with_options(
            self.storage.clone(),
            Some(event_bus_handle),
            self.config.assets.ignore_patterns.clone(),
            self.config.effective_default_owner(),
        )?;
        let env_collector = EnvCollector::new(self.storage.clone());

        // 1b. 挂载配置驱动的 [[hooks]] 分发器与 [[pipelines]] 编排引擎至 EventBus
        {
            let hook_dispatcher = crate::hooks::HookDispatcher::new(
                self.config.hooks.clone(),
                self.config.assets.ignore_patterns.clone(),
            );
            let pipeline_engine = crate::hooks::HookPipelineEngine::new(
                self.config.pipelines.clone(),
                Some(self.storage.clone()),
            );
            let has_hooks = !self.config.hooks.is_empty();
            let has_pipelines = !self.config.pipelines.is_empty();
            println!(
                "✅ 事件驱动 Hook 分发器与编排流水线就绪 (hooks: {}, pipelines: {})",
                self.config.hooks.len(),
                self.config.pipelines.len()
            );
            let mut hook_rx = event_bus.subscribe();
            tokio::spawn(async move {
                loop {
                    match hook_rx.recv().await {
                        Ok(ev) => {
                            if has_hooks {
                                let _hook_actions = hook_dispatcher.dispatch(Arc::clone(&ev)).await;
                            }
                            if has_pipelines {
                                let _pipe_audits = pipeline_engine.execute(ev).await;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            eprintln!("[Hooks][WARN] EventBus 消费滞后，跳过 {} 条历史事件", skipped);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
        }

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
        let _scheduler_ready = std::any::type_name::<BackgroundScheduler>(); // 每次巡检在独立 OS 线程构造调度器以隔离 reqwest::blocking

        let proc_probe_kind = match std::env::consts::OS {
            "macos" => "macOS libproc/lsof",
            "windows" => "Windows 进程探针",
            _ => "/proc",
        };
        println!("✅ 动态任务进程定时采集就绪 (每 60 秒轮询 {})", proc_probe_kind);
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
                                    emit_obs_log(
                                        "INFO",
                                        "watcher",
                                        None,
                                        &format!("自动感知并增量同步 {count} 个文件变更事件"),
                                        None,
                                        None,
                                    );
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
                            if tx.receiver_count() > 0 && tx.send(msg.to_string()).is_err() {
                                eprintln!("[Daemon][WARN] SSE ProcessSync 广播失败");
                            }
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
                            let _running_guard = RunningFlagGuard(running_flag);
                            let scheduler = BackgroundScheduler::new(storage_for_cycle);
                            match scheduler.run_inspection_cycle() {
                                Ok(report) => {
                                    emit_obs_log(
                                        "INFO",
                                        "scheduler",
                                        None,
                                        &format!(
                                            "深度巡检完成: 同步进程 {}, 属性图节点更新 {}, 边更新 {}",
                                            report.synced_processes,
                                            report.graph_nodes_updated,
                                            report.graph_edges_updated
                                        ),
                                        None,
                                        None,
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
                                        if tx.receiver_count() > 0 && tx.send(msg.to_string()).is_err() {
                                            emit_obs_log("WARN", "daemon", None, "SSE SchedulerCycle 广播失败", None, None);
                                        }
                                    }
                                }
                                Err(e) => {
                                    emit_obs_log("WARN", "daemon", None, &format!("后台深度巡检失败: {e}"), None, None);
                                }
                            }
                        });
                    }
                }

                // 捕捉退出信号
                _ = tokio::signal::ctrl_c() => {
                    crate::body::WaitEventRegistry::record_us(
                        crate::body::WaitEventId::IdleDaemonLoop,
                        idle_wait_start.elapsed().as_micros() as u64,
                    );
                    println!("\n[Agent Body] 收到停机信号，正在保存状态并安全退出...");
                    break;
                }
            }
            idle_wait_start = std::time::Instant::now();
        }

        Ok(())
    }
}
