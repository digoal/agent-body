use std::path::PathBuf;
use anyhow::Result;
use agent_body::config::Config;
use agent_body::storage::Storage;
use crate::cli_args::*;

pub fn handle_ops(cmd: Commands, cfg: &Config, storage: Storage) -> Result<()> {
    match cmd {
        Commands::Storage { action } => {
            match action {
                StorageCommands::Info => {
                    println!("当前活跃存储提供者: {}", storage.provider_name());
                    println!("配置数据库文件路径: {:?}", cfg.db_path);
                    if let Some(ref pg) = cfg.storage.pg_url {
                        println!("配置 PostgreSQL URL: {}", pg);
                    }
                }
                StorageCommands::Migrate { from, from_url, to, to_url } => {
                    println!("🚀 正在准备执行跨引擎数据迁移: {} -> {} ...", from, to);
                    let src_storage = match from.to_lowercase().as_str() {
                        "postgres" | "pg" => {
                            let url = from_url.unwrap_or_else(|| "postgresql://postgres@127.0.0.1:5432/agent_body".to_string());
                            let p = agent_body::storage::PgStorageProvider::connect(&url)?;
                            Storage::new(std::sync::Arc::new(p))
                        }
                        "duckdb" => {
                            let path = from_url.map(PathBuf::from).unwrap_or_else(|| cfg.data_dir.join("steward.duckdb"));
                            let p = agent_body::storage::DuckDbStorageProvider::open(&path)?;
                            Storage::new(std::sync::Arc::new(p))
                        }
                        _ => {
                            let path = from_url.map(PathBuf::from).unwrap_or_else(|| cfg.db_path.clone());
                            Storage::open(&path)?
                        }
                    };

                    let dst_storage = match to.to_lowercase().as_str() {
                        "postgres" | "pg" => {
                            let url = to_url.unwrap_or_else(|| "postgresql://postgres@127.0.0.1:5432/agent_body".to_string());
                            let p = agent_body::storage::PgStorageProvider::connect(&url)?;
                            Storage::new(std::sync::Arc::new(p))
                        }
                        "duckdb" => {
                            let path = to_url.map(PathBuf::from).unwrap_or_else(|| cfg.data_dir.join("steward.duckdb"));
                            let p = agent_body::storage::DuckDbStorageProvider::open(&path)?;
                            Storage::new(std::sync::Arc::new(p))
                        }
                        _ => {
                            let path = to_url.map(PathBuf::from).unwrap_or_else(|| cfg.db_path.clone());
                            Storage::open(&path)?
                        }
                    };

                    let report = agent_body::storage::StorageMigrator::migrate(&src_storage, &dst_storage)?;
                    println!("✅ 数据热迁移完成！耗时: {} ms", report.duration_ms);
                    println!("  - 放置规则迁移: {} 条", report.placement_rules_migrated);
                    println!("  - 扫描策略迁移: {} 条", report.scan_policies_migrated);
                    println!(
                        "  - 纳管资产迁移: {} 个 (向量 {} 条)",
                        report.assets_migrated, report.asset_embeddings_migrated
                    );
                    println!("  - 活跃进程迁移: {} 个", report.active_processes_migrated);
                    println!("  - 记忆条目迁移: {} 条", report.memories_migrated);
                    println!("  - 变更记录迁移: {} 条", report.changes_migrated);
                    println!("  - 依赖关系迁移: {} 条", report.links_migrated);
                    println!("  - 扫描作业迁移: {} 个", report.scan_jobs_migrated);
                    println!(
                        "  - 属性图迁移:   {} 节点 (向量 {} 条) / {} 边",
                        report.fact_nodes_migrated,
                        report.fact_node_embeddings_migrated,
                        report.fact_edges_migrated
                    );
                    println!(
                        "  - 协作与目录:   锁 {} / 队列 {} / MCP {} / SKILL {} / 外服 {}",
                        report.coordination_locks_migrated,
                        report.lock_wait_queue_migrated,
                        report.mcp_catalog_migrated,
                        report.skills_catalog_migrated,
                        report.external_services_migrated
                    );
                    println!(
                        "  - 存证与审计:   Bench/AWR {} / 账本 {} / 审计 {} / 胶囊 {}",
                        report.benchmark_runs_migrated,
                        report.effectiveness_ledger_migrated,
                        report.audit_events_migrated,
                        report.body_checkpoints_migrated
                    );
                    println!(
                        "  - 网格记录:     对话 {} / 文件投递 {}",
                        report.mesh_dialogues_migrated, report.mesh_transfers_migrated
                    );
                }
                StorageCommands::Sync { to, to_path } => {
                    println!("🔄 正在执行主存储 -> 分析副本 ({}) 同步 ...", to);
                    let target_path = to_path.map(PathBuf::from).unwrap_or_else(|| cfg.data_dir.join("steward.duckdb"));
                    let duckdb_prov = agent_body::storage::DuckDbStorageProvider::open(&target_path)?;
                    let pipeline = agent_body::storage::SyncPipeline::new(storage.provider().clone(), std::sync::Arc::new(duckdb_prov));
                    let rep = pipeline.sync_all()?;
                    println!("✅ 分析副本同步完成！耗时: {} ms", rep.duration_ms);
                    println!("  - 纳管资产同步:   {} 个", rep.assets_synced);
                    println!("  - 变更记录同步:   {} 条", rep.changes_synced);
                    println!("  - 长期记忆同步:   {} 条", rep.memories_synced);
                    println!("  - 属性图同步:     {} 节点 / {} 边", rep.fact_nodes_synced, rep.fact_edges_synced);
                    println!(
                        "  - 拓扑与运行态:   Links {} / 活跃进程 {} / 接力胶囊 {}",
                        rep.links_synced, rep.active_processes_synced, rep.body_checkpoints_synced
                    );
                    println!(
                        "  - 审计与账本:     审计事件 {} / 效能账本 {}",
                        rep.audit_events_synced, rep.effectiveness_ledger_synced
                    );
                }
            }
        }

        Commands::Coordination { action } => {
            let engine = agent_body::coordination::CoordinationEngine::new(storage.provider().clone());
            match action {
                CoordinationCommands::Acquire { resource_type, resource_id, scope, holder_agent, holder_pid, ttl_seconds } => {
                    let req = agent_body::coordination::LockRequest {
                        resource_type: agent_body::coordination::ResourceType::from_str_loose(&resource_type),
                        resource_id,
                        scope: agent_body::coordination::LockScope::from_str_loose(&scope),
                        holder_agent,
                        holder_pid,
                        ttl_seconds,
                        metadata: None,
                    };
                    let res = engine.try_acquire(&req)?;
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
                CoordinationCommands::List { resource_type } => {
                    let rt_opt = resource_type.as_deref().map(agent_body::coordination::ResourceType::from_str_loose);
                    let locks = engine.list_active_locks(rt_opt.as_ref())?;
                    println!("╔══════════════════════════════════════════════════════════════════════════════════════════╗");
                    println!("║                        多 Agent 协作锁全景监控看板                                       ║");
                    println!("╚══════════════════════════════════════════════════════════════════════════════════════════╝");
                    if locks.is_empty() {
                        println!("当前没有任何活跃的协作锁，所有受控资源均处于可用/空闲状态。");
                    } else {
                        println!("{:<36} {:<10} {:<24} {:<12} {:<10} {:<24}", "LOCK_ID", "TYPE", "RESOURCE_ID", "HOLDER", "SCOPE", "EXPIRES_AT");
                        println!("{:-<120}", "");
                        for l in locks {
                            println!("{:<36} {:<10} {:<24} {:<12} {:<10} {:<24}",
                                l.lock_id,
                                l.resource_type,
                                l.resource_id,
                                l.holder_agent,
                                l.scope,
                                l.expires_at,
                            );
                        }
                    }
                }
                CoordinationCommands::Release {
                    lock_id,
                    resource_type,
                    resource_id,
                    holder_agent,
                    force,
                } => {
                    let (resolved_id, ok) = engine.release_flexible(
                        lock_id.as_deref(),
                        resource_type.as_deref(),
                        resource_id.as_deref(),
                        holder_agent.as_deref(),
                        force,
                    )?;
                    let display_id = resolved_id.as_deref().unwrap_or("(未匹配)");
                    if ok {
                        println!("✅ 锁 `{}` 已成功解除！(force={})", display_id, force);
                    } else if resolved_id.is_some() && !force {
                        println!(
                            "⚠️ 锁 `{}` 释放被拒绝：当前非持有者 (请指定正确的 --holder-agent 或使用 --force=true)。",
                            display_id
                        );
                    } else {
                        println!("⚠️ 未找到指定锁（lock_id/资源标识均未命中活跃锁）或该锁已自动过期。");
                    }
                }
                CoordinationCommands::Gc => {
                    let count = engine.gc_expired_locks()?;
                    println!("🧹 协作锁 GC 完成！已清理释放 {} 个过期锁。", count);
                }
                CoordinationCommands::Wait { resource_type, resource_id, scope, holder_agent, ttl_seconds, wait_timeout_ms, priority } => {
                    let req = agent_body::coordination::LockRequest {
                        resource_type: agent_body::coordination::ResourceType::from_str_loose(&resource_type),
                        resource_id,
                        scope: agent_body::coordination::LockScope::from_str_loose(&scope),
                        holder_agent,
                        holder_pid: None,
                        ttl_seconds,
                        metadata: None,
                    };
                    let res = engine.wait_and_acquire(&req, wait_timeout_ms, priority)?;
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
                CoordinationCommands::Deadlock => {
                    let rep = engine.detect_deadlocks()?;
                    let waits = engine.list_wait_queue(None)?;
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                        "deadlock_report": rep,
                        "wait_queue_count": waits.len(),
                        "wait_queue": waits,
                    }))?);
                }
            }
        }

        Commands::Preflight { json } => {
            let report = agent_body::preflight::PreflightChecker::run_all(&cfg.data_dir, Some(&cfg));
            if json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("{}", report.format_table());
            }
            if !report.passed_required {
                std::process::exit(1);
            }
        }

        Commands::Web { host, port, auth_token } => {
            let web_host = host.unwrap_or_else(|| cfg.web.host.clone());
            let web_port = port.unwrap_or(cfg.web.port);
            let token = auth_token.or_else(|| cfg.web.auth_token.clone());

            let web_config = agent_body::web::WebConfig {
                host: web_host,
                port: web_port,
                auth_token: token,
            };

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let web_server = agent_body::web::WebServer::new(web_config, storage.provider().clone());
                web_server.run().await
            })?;
        }

        Commands::Hooks { action } => {
            match action {
                HooksCommands::Generate { client, out_dir, dry_run } => {
                    use agent_body::hooks::{ClientHookGenerator, ClientType};
                    let client_type = client.parse::<ClientType>()?;
                    let res = ClientHookGenerator::generate(client_type, out_dir.as_deref(), dry_run)?;

                    println!("╔══════════════════════════════════════════════════════════════════════════════════════════╗");
                    println!("║                   Agent 客户端 Hook 自动化生成成功                                         ║");
                    println!("╠══════════════════════════════════════════════════════════════════════════════════════════╣");
                    println!("║ 客户端类型:   {:<75} ║", res.client);
                    println!("║ 推荐相对路径: {:<75} ║", res.relative_path);
                    println!("║ 是否可执行:   {:<75} ║", if res.executable { "是 (0o755)" } else { "否 (普通文本规则)" });
                    println!("║ 执行模式:     {:<75} ║", if res.is_dry_run { "Dry-Run 预览 (未写盘)" } else { "写入已生效" });
                    if let Some(ref path) = res.written_path {
                        println!("║ 落地物理路径: {:<75} ║", path);
                    }
                    for comp in &res.companion_files {
                        let comp_display = comp.written_path.as_deref().unwrap_or(&comp.relative_path);
                        println!("║ 配套脚本路径: {:<75} ║", comp_display);
                    }
                    println!("╚══════════════════════════════════════════════════════════════════════════════════════════╝");
                    println!("\n--- 生成内容预览 ---\n{}", res.content);
                }
                HooksCommands::Pre { tool_name, args } => {
                    let parsed_args: serde_json::Value = serde_json::from_str(&args)
                        .unwrap_or_else(|_| serde_json::json!({}));
                    let engine = agent_body::api::hooks::AgentHookEngine::new(storage);
                    let res = engine.pre_tool_call(&tool_name, &parsed_args);
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
                HooksCommands::Post { tool_name, args, success, error_output } => {
                    let parsed_args: serde_json::Value = serde_json::from_str(&args)
                        .unwrap_or_else(|_| serde_json::json!({}));
                    let engine = agent_body::api::hooks::AgentHookEngine::new(storage);
                    let res = engine.post_tool_call_with_error(&tool_name, &parsed_args, success, error_output.as_deref());
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
            }
        }

        Commands::Analytics { action } => {
            use agent_body::storage::{AnalyticsProvider, DuckDbAnalyticsEngine};
            let engine = DuckDbAnalyticsEngine::from_synced_storage(storage.provider().clone())?;
            match action {
                AnalyticsCommands::Heatmap { days } => {
                    let heatmap = engine.changes_heatmap(days)?;
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                        "time_span_days": days,
                        "heatmap": heatmap,
                    }))?);
                }
                AnalyticsCommands::Trend { metric, days } => {
                    let trend = engine.asset_growth_trend(days)?;
                    let decay = engine.memory_decay_curve()?;
                    let stats = engine.fact_graph_stats()?;
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                        "metric": metric,
                        "time_span_days": days,
                        "asset_growth_trend": trend,
                        "memory_decay": decay,
                        "graph_stats": stats,
                    }))?);
                }
                AnalyticsCommands::Export { table, output_path } => {
                    let summary = engine.export_to_parquet(&table, &output_path)?;
                    println!("{}", serde_json::to_string_pretty(&summary)?);
                }
            }
        }

        Commands::McpCatalog { action } => {
            let scanner = agent_body::ecosystem::McpConfigScanner::new(storage.clone());
            match action {
                McpCatalogCommands::Scan => {
                    let (cnt, sanitized) = scanner.scan_default_clients()?;
                    println!("✅ MCP 目录扫描完成：发现 {} 个 Server（已脱敏 {} 个环境变量密钥）", cnt, sanitized);
                }
                McpCatalogCommands::List => {
                    let list = storage.list_mcp_catalog()?;
                    println!("{}", serde_json::to_string_pretty(&list)?);
                }
                McpCatalogCommands::Route { query, limit } => {
                    let list = storage.search_mcp_catalog(&query, limit)?;
                    println!("{}", serde_json::to_string_pretty(&list)?);
                }
            }
        }

        Commands::Skill { action } => {
            match action {
                SkillCommands::Scan => {
                    let scanner = agent_body::ecosystem::SkillCatalogScanner::new(storage.clone());
                    let (cnt, tokens) = scanner.scan_default_roots()?;
                    println!("✅ SKILL 目录扫描完成：已编目 {} 个 SKILL（若全量注入约消耗 {} tokens）", cnt, tokens);
                }
                SkillCommands::List => {
                    let list = storage.list_skills_catalog()?;
                    println!("已编目 SKILL 共 {} 个：", list.len());
                    for s in list {
                        println!("  - {} (~{} tok) -> {}", s.skill_name, s.est_tokens, s.skill_path);
                    }
                }
                SkillCommands::Route { intent, budget_tokens, max_skills } => {
                    let router = agent_body::ecosystem::ContextBudgetRouter::new(storage.clone());
                    let plan = router.route(&agent_body::ecosystem::ContextRouteRequest {
                        intent,
                        budget_tokens,
                        max_skills,
                        max_mcps: 4,
                        max_services: 3,
                    })?;
                    println!("{}", plan.markdown);
                    println!(
                        "📊 Context 防爆统计：全量 {} 个 SKILL (~{} tok) -> 本次按需路由 {} tok（节省 {:.1}%）",
                        plan.total_skills_catalog_count, plan.total_skills_full_tokens, plan.route_tokens_used, plan.saved_pct
                    );
                }
            }
        }

        Commands::Service { action } => {
            let mgr = agent_body::ecosystem::ExternalContactsManager::new(storage.clone());
            match action {
                ServiceCommands::Register {
                    name, category, endpoint, protocol, capabilities,
                    auth_type, auth_env_var, health_endpoint, usage_example
                } => {
                    let rec = agent_body::storage::models::ExternalServiceRecord {
                        id: format!("svc:{}", name),
                        service_name: name.clone(),
                        category,
                        endpoint,
                        protocol,
                        capabilities,
                        auth_type,
                        auth_env_var,
                        health_endpoint,
                        usage_example,
                        status: "unknown".to_string(),
                        last_checked_at: None,
                    };
                    mgr.register_service(&rec)?;
                    println!("✅ 外部服务联系人 '{}' 已安全登记", name);
                }
                ServiceCommands::List { category } => {
                    let list = storage.list_external_services(category.as_deref())?;
                    println!("{}", serde_json::to_string_pretty(&list)?);
                }
                ServiceCommands::Query { query } => {
                    let list = storage.search_external_services(&query, 10)?;
                    println!("{}", serde_json::to_string_pretty(&list)?);
                }
                ServiceCommands::Check { name } => {
                    let rep = mgr.check_health(&name)?;
                    println!("{}", serde_json::to_string_pretty(&rep)?);
                }
                ServiceCommands::Bootstrap => {
                    let cnt = mgr.bootstrap_from_config(&cfg)?;
                    println!("✅ 已从配置自动推导 {} 条外部服务联系人", cnt);
                }
            }
        }

        Commands::Mesh { action } => {
            use agent_body::mesh::{build_local_facts, MeshClient};
            let client = MeshClient::new(&cfg.mesh, cfg.web.auth_token.clone());
            match action {
                MeshCommands::Peers => {
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                        "enabled": cfg.mesh.enabled,
                        "peers": client.peers(),
                        "timeout_ms": cfg.mesh.timeout_ms,
                    }))?);
                }
                MeshCommands::Local => println!("{}", serde_json::to_string_pretty(&build_local_facts())?),
                MeshCommands::Ping => println!("{}", serde_json::to_string_pretty(&client.ping_all())?),
                MeshCommands::Facts => println!("{}", serde_json::to_string_pretty(&client.facts_all())?),
                MeshCommands::Locks => println!("{}", serde_json::to_string_pretty(&client.locks_all())?),
                MeshCommands::Port { port } => {
                    println!("{}", serde_json::to_string_pretty(&client.federated_port_check(port))?);
                }
                MeshCommands::Acquire { peer, resource_type, resource_id, scope, holder_agent, ttl_seconds } => {
                    let req = agent_body::coordination::LockRequest {
                        resource_type: agent_body::coordination::ResourceType::from_str_loose(&resource_type),
                        resource_id,
                        scope: agent_body::coordination::LockScope::from_str_loose(&scope),
                        holder_agent,
                        holder_pid: None,
                        ttl_seconds,
                        metadata: None,
                    };
                    println!("{}", serde_json::to_string_pretty(&client.acquire_on_peer(&peer, &req)?)?);
                }
                MeshCommands::Graph { query, hops } => {
                    println!("{}", serde_json::to_string_pretty(&client.federated_graph_query(&query, hops))?);
                }
                MeshCommands::DialogueSend { peer, thread_id, intent, content, from_agent, to_agent } => {
                    println!("{}", serde_json::to_string_pretty(&client.send_dialogue(&peer, &thread_id, &intent, &content, &from_agent, &to_agent)?)?);
                }
                MeshCommands::DialogueInbox { limit } => {
                    println!("{}", serde_json::to_string_pretty(&storage.list_mesh_dialogues(limit)?)?);
                }
                MeshCommands::DialogueAck { dialogue_id } => {
                    let ok = storage.update_mesh_dialogue_status(&dialogue_id, "acked")?;
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"dialogue_id": dialogue_id, "acked": ok}))?);
                }
                MeshCommands::TransferSend { peer, file, from_agent } => {
                    println!("{}", serde_json::to_string_pretty(&client.send_transfer(&peer, std::path::Path::new(&file), &from_agent)?)?);
                }
                MeshCommands::TransferInbox { limit } => {
                    println!("{}", serde_json::to_string_pretty(&storage.list_mesh_transfers(limit)?)?);
                }
                MeshCommands::TransferAccept { transfer_id, dest_dir } => {
                    let rec = agent_body::mesh::transfer::accept_transfer_to_dir(
                        storage.provider().as_ref(),
                        &transfer_id,
                        std::path::Path::new(&dest_dir),
                    )?;
                    println!("{}", serde_json::to_string_pretty(&rec)?);
                }
                MeshCommands::TransferReject { transfer_id } => {
                    let ok = agent_body::mesh::transfer::reject_transfer(storage.provider().as_ref(), &transfer_id)?;
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"transfer_id": transfer_id, "rejected": ok}))?);
                }
            }
        }

        Commands::Platform { action } => {
            let probe = agent_body::platform::current_probe();
            match action {
                PlatformCommands::Info => {
                    println!("OS: {}  |  平台探针: {}", std::env::consts::OS, probe.name());
                }
                PlatformCommands::Ports => {
                    let ports = probe.listening_ports()?;
                    println!("平台探针 `{}` 识别监听端口 {} 个：", probe.name(), ports.len());
                    for p in ports.iter().filter(|p| !p.is_system_daemon) {
                        println!("  - {}/{} {} pid={:?} {}", p.port, p.protocol, p.bind_ip, p.pid, p.process_name.as_deref().unwrap_or("-"));
                    }
                }
                PlatformCommands::Services => {
                    let svcs = probe.services()?;
                    println!("平台探针 `{}` 识别自动化服务/任务 {} 个：", probe.name(), svcs.len());
                    for s in svcs.iter().take(50) {
                        println!("  - [{}] {} -> {}", s.kind, s.name, s.command);
                    }
                }
            }
        }

        Commands::Demo { action } => {
            use agent_body::api::mcp_tools::McpToolRegistry;
            use agent_body::demo::DemoEngine;
            match action {
                DemoCommands::List => {
                    println!("{}", serde_json::to_string_pretty(&DemoEngine::list_scenarios())?);
                }
                DemoCommands::Run { scenario_id } => {
                    let res = DemoEngine::run_scenario(storage.provider().clone(), &scenario_id)?;
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
                DemoCommands::Roi => {
                    let roi = DemoEngine::compute_roi(storage.provider().as_ref())?;
                    println!("{}", serde_json::to_string_pretty(&roi)?);
                }
                DemoCommands::Parity => {
                    let tools = McpToolRegistry::new(storage.clone()).list_tools();
                    let names: Vec<_> = tools.iter().filter_map(|t| t.get("name").and_then(|v| v.as_str())).collect();
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
                        "total_mcp_tools": names.len(),
                        "tools": names,
                    }))?);
                }
                DemoCommands::Reset => {
                    let cleaned = DemoEngine::reset_sandbox(storage.provider().as_ref())?;
                    println!("✅ 已清理 {} 条 demo:* 沙盒状态", cleaned);
                }
            }
        }

        _ => unreachable!(),
    }
    Ok(())
}
