use std::path::PathBuf;
use clap::Parser;
use anyhow::Result;
use agent_body::config::Config;
use agent_body::storage::Storage;
use agent_body::placement::{PlacementEngine, PlacementRequest};
use agent_body::scanner::Scanner;
use agent_body::env::EnvCollector;
use agent_body::memory::MemoryManager;
use agent_body::brief::{BriefAssembler, BriefRequest};

mod cli_args;
use cli_args::*;


fn main() -> Result<()> {
    #[cfg(unix)]
    unsafe {
        extern "C" {
            fn signal(sig: i32, handler: usize) -> usize;
        }
        // 恢复 SIGPIPE 默认行为，避免输出被管道下游（如 | head）提前关闭时触发 Broken pipe panic
        signal(13, 0);
    }

    let cli = Cli::parse();
    let cfg = Config::load_or_default(cli.config.as_deref())?;
    let storage = Storage::from_config(&cfg)?;
    let audit_storage = storage.clone();
    let cli_t0 = std::time::Instant::now();
    let cli_cmd_name = cli_command_label(&cli.command);
    let skip_cli_audit = matches!(
        cli.command,
        Commands::Mcp
            | Commands::Daemon
            | Commands::Web { .. }
            | Commands::Tui { .. }
            | Commands::Bench { .. }
            | Commands::Demo { .. }
    );

    let exec_res: Result<()> = (|| -> Result<()> {
        match cli.command {
        Commands::Status => {
            println!("╔═══════════════════════════════════════════════════════╗");
            println!("║      Agent Body (Rust 版) — 系统状态          ║");
            println!("╚═══════════════════════════════════════════════════════╝");
            println!("版本:           {}", agent_body::version_info());
            println!("数据目录:       {:?}", cfg.data_dir);
            println!("数据库文件:     {:?}", cfg.db_path);
            let assets = storage.list_assets(100_000)?;
            let links = storage.list_all_links(100_000)?;
            let rules = storage.get_placement_rules()?;
            let policies = storage.get_scan_policies()?;
            let procs = storage.list_active_processes()?;
            println!("已纳管资产数:   {}", assets.len());
            println!("依赖关系边数:   {} 条", links.len());
            println!("放置规范规则:   {} 条", rules.len());
            println!("分级扫描策略:   {} 条", policies.len());
            println!("当前动态进程:   {} 个", procs.len());
        }

        Commands::Tui { interval } => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let arc_storage = std::sync::Arc::new(storage);
                let dashboard = agent_body::tui::TuiDashboard::new(arc_storage);
                dashboard.run_loop(interval, None).await
            })?;
        }

        Commands::Brief { query, session_id, budget_tokens, legacy } => {
            let t0 = std::time::Instant::now();
            let out = if legacy {
                let env_collector = EnvCollector::new(storage.clone());
                let summary = env_collector.collect_system_summary();
                let procs = env_collector.collect_and_sync_processes()?;
                let assets = storage.list_assets(5)?;

                let mut buf = String::new();
                buf.push_str("=== Steward Brief 上下文简报 ===\n");
                buf.push_str(&format!("系统: {} | 内核: {} | 内存使用率: {:.1}%\n", summary.os_name, summary.kernel_version, summary.memory_used_pct));
                buf.push_str("活跃动态任务进程 (前 5):\n");
                for p in procs.iter().take(5) {
                    let mem_mb = p.mem_bytes as f64 / (1024.0 * 1024.0);
                    buf.push_str(&format!("  - [PID {}] {} (CPU {:.1}%, 内存 {:.1}MB) -> {}\n", p.pid, p.name, p.cpu_pct, mem_mb, p.command));
                }
                buf.push_str("最近活跃资产:\n");
                for a in assets {
                    buf.push_str(&format!("  - [{}] {} ({})\n", a.asset_type, a.path, a.scan_tier));
                }
                buf
            } else {
                let provider = cfg.build_memory_provider(storage.clone());
                let assembler = BriefAssembler::new(storage.clone(), Some(provider));
                let output = assembler.assemble(&BriefRequest {
                    query,
                    session_id,
                    budget_tokens,
                })?;
                output.markdown
            };
            print!("{}", out);
            agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage.provider().as_ref(),
                "cli",
                "body_brief",
                &out,
                t0.elapsed().as_micros() as i64,
            );
        }

        Commands::Place { action } => {
            let engine = PlacementEngine::new(storage.clone());
            match action {
                PlaceCommands::Suggest { r#type, name, intent, security } => {
                    let req = PlacementRequest {
                        object_type: r#type,
                        name,
                        intent,
                        tags: vec![],
                        security_level: security,
                    };
                    let suggestion = engine.suggest(&req)?;
                    println!("╔═══════════════════════════════════════════════════════╗");
                    println!("║             Steward 对象放置与路径建议                ║");
                    println!("╚═══════════════════════════════════════════════════════╝");
                    println!("推荐目标路径:   {:?}", suggestion.suggested_path);
                    println!("命中规范规则:   {}", suggestion.rule_matched);
                    println!("规则理由:       {}", suggestion.reason);
                    println!("命名建议:       {}", suggestion.naming_convention);
                    println!("冲突状态:       {}", if suggestion.conflict_detected { "⚠️ 路径已存在冲突！" } else { "✅ 路径空闲可用" });
                    println!("伴随开发指引:");
                    for g in suggestion.guidelines {
                        println!("  * {}", g);
                    }
                }
                PlaceCommands::Rules => {
                    let rules = storage.get_placement_rules()?;
                    println!("当前生效的宿主机目录放置规范 (共 {} 条):", rules.len());
                    for r in rules {
                        println!("  - [{}] {} -> {} (优先级 {})", r.rule_name, r.pattern_type, r.target_base_dir, r.priority);
                        if let Some(desc) = r.description {
                            println!("    说明: {}", desc);
                        }
                    }
                }
                PlaceCommands::Learn { min_samples, accept } => {
                    let learner = agent_body::placement::learner::PlacementLearner::new(&storage);
                    let rep = learner.learn_rules(min_samples)?;
                    if rep.candidates.is_empty() {
                        println!("未发现新的聚类放置模式（样本数低于 {} 或已被既有规则完全覆盖）。", min_samples);
                    } else {
                        println!("🔍 分析了 {} 个资产，发现 {} 个潜在的放置规则候选:", rep.total_analyzed_assets, rep.candidate_count);
                        for c in &rep.candidates {
                            println!("\n候选规则: {} ({})", c.rule_name, c.candidate_id);
                            println!("建议基准目录: {}", c.target_base_dir);
                            println!("推荐命名模式: {}", c.naming_pattern);
                            println!("样本数: {}, 置信度: {:.2}", c.sample_count, c.confidence);
                            println!("说明: {}", c.description);
                            println!("样本示例:");
                            for s in &c.sample_paths {
                                println!("  - {}", s);
                            }
                            if accept {
                                let rule = learner.apply_candidate(c)?;
                                println!("  ✅ 已自动采纳并持久化新规则: {}", rule.rule_name);
                            }
                        }
                        if !accept {
                            println!("\n💡 提示: 添加 --accept 参数可一键将上述候选规则持久化到 placement_rules 表中。");
                        }
                    }
                }
            }
        }

        Commands::Asset { action } => {
            match action {
                AssetCommands::Scan { path, enrich, enrich_limit } => {
                    println!("正在执行分级颗粒度扫描: {:?}", path);
                    let scanner = Scanner::new(storage.clone())?;
                    let report = scanner.scan_path(&path)?;
                    println!("扫描完成报告:");
                    println!("  - 扫描目录数:       {}", report.scanned_dirs);
                    println!("  - 深层索引文件数:   {} (已完成 L1 确定性元数据提取)", report.deep_indexed_files);
                    println!("  - 黑名单阻断目录数: {}", report.shallow_blocked_dirs.len());
                    for b in report.shallow_blocked_dirs {
                        println!("    ★ [Shallow Tier 阻断] {} ({})", b.path, b.reason);
                    }
                    if enrich {
                        let prefix = path.to_string_lossy().to_string();
                        println!("\n🤖 正在调用本地 Ollama 小模型进行 L2 语义标注精炼 (limit={})...", enrich_limit);
                        let annotator = agent_body::scanner::OllamaAnnotator::with_default_config(storage);
                        let er = annotator.enrich_pending_assets(enrich_limit, Some(&prefix))?;
                        println!("  - 模型: {} | 在线: {} | 候选: {} | 成功: {} | 跳过: {} | 失败: {} | 耗时: {}ms",
                            er.model_used, er.ollama_available, er.total_candidates, er.enriched_count, er.skipped_count, er.failed_count, er.elapsed_ms);
                        for s in er.samples {
                            println!("    ✅ [{}] {} -> {} (入口: {})", s.domain, s.path, s.purpose, s.entry_point);
                        }
                        let emb_cnt = annotator.embed_pending_assets(enrich_limit, Some(&prefix)).unwrap_or(0);
                        println!("  - Ollama Embedding 向量化完成: {} 个核心资产已生成向量", emb_cnt);
                    }
                }
                AssetCommands::Search { query, limit, detail } => {
                    let t0 = std::time::Instant::now();
                    let level = agent_body::scanner::DetailLevel::from_str_opt(Some(&detail));
                    let results = storage.search_assets_fts(&query, limit)?;
                    let mut out = format!("资产搜索匹配结果 (共 {} 条, detail={:?}):\n", results.len(), level);
                    for a in results {
                        out.push_str(&agent_body::scanner::format_asset_line(&a, level));
                        out.push('\n');
                    }
                    print!("{}", out);
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_asset_search",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                }
                AssetCommands::Register { path, r#type, owner, security_level, description, desc_kv } => {
                    let parsed_kv: serde_json::Value = serde_json::from_str(&desc_kv)
                        .unwrap_or_else(|_| serde_json::json!({}));
                    let owner = owner.unwrap_or_else(|| cfg.effective_default_owner());
                    let record = agent_body::storage::models::AssetRecord {
                        id: uuid::Uuid::new_v4().to_string(),
                        path: path.clone(),
                        asset_type: r#type.clone(),
                        owner,
                        security_level,
                        scan_tier: "deep".to_string(),
                        description,
                        desc_kv: parsed_kv,
                        usage: None,
                        size_bytes: None,
                        file_count: None,
                        tags: vec![],
                        status: "active".to_string(),
                    };
                    storage.upsert_asset(&record)?;
                    println!("✅ 资产登记成功: {} [{}]", path, r#type);
                }
                AssetCommands::List { limit } => {
                    let results = storage.list_assets(limit)?;
                    println!("最近资产清单 (前 {} 条):", results.len());
                    for a in results {
                        println!("  - [{:8}] {:6} | {}", a.asset_type, a.scan_tier, a.path);
                    }
                }
                AssetCommands::Job { action, target, limit } => {
                    let sm = agent_body::scanner::ScanStateMachine::new(storage.clone())?;
                    match action.as_str() {
                        "create" => {
                            let p = target.ok_or_else(|| anyhow::anyhow!("缺少目标扫描路径"))?;
                            let job = sm.create_job(std::path::Path::new(&p))?;
                            println!("✅ 成功创建断点续扫任务:");
                            println!("  - Job ID:    {}", job.id);
                            println!("  - 根路径:    {}", job.root_path);
                            println!("  - 当前状态:  {}", job.status);
                        }
                        "run" | "resume" => {
                            let jid = target.ok_or_else(|| anyhow::anyhow!("缺少 Job ID"))?;
                            println!("正在启动或恢复断点续扫任务: {}", jid);
                            let job = sm.run_or_resume(&jid, limit)?;
                            println!("任务执行阶段完成:");
                            println!("  - 状态:       {}", job.status);
                            println!("  - 最新断点:   {:?}", job.last_path);
                            println!("  - 扫描目录数: {}", job.scanned_dirs);
                            println!("  - 扫描文件数: {}", job.scanned_files);
                            println!("  - 录入资产数: {}", job.total_assets);
                        }
                        "pause" => {
                            let jid = target.ok_or_else(|| anyhow::anyhow!("缺少 Job ID"))?;
                            let job = sm.pause_job(&jid)?;
                            println!("任务已暂停: {} (状态: {})", job.id, job.status);
                        }
                        "list" => {
                            let jobs = sm.list_jobs(20)?;
                            println!("最近断点续扫任务记录 (共 {} 条):", jobs.len());
                            for j in jobs {
                                println!("  - [{:9}] {} | 根路径: {} | 资产数: {}", j.status, j.id, j.root_path, j.total_assets);
                            }
                        }
                        "status" => {
                            let jid = target.ok_or_else(|| anyhow::anyhow!("缺少 Job ID"))?;
                            if let Some(j) = sm.get_job(&jid)? {
                                println!("断点任务状态详情:");
                                println!("  - Job ID:      {}", j.id);
                                println!("  - 状态:        {}", j.status);
                                println!("  - 根路径:      {}", j.root_path);
                                println!("  - 最新断点:    {:?}", j.last_path);
                                println!("  - 扫描文件数:  {}", j.scanned_files);
                                println!("  - 总资产数:    {}", j.total_assets);
                                println!("  - 创建时间:    {:?}", j.created_at);
                                println!("  - 完成时间:    {:?}", j.completed_at);
                            } else {
                                println!("未找到 Job ID: {}", jid);
                            }
                        }
                        _ => {
                            println!("未知动作: {} (支持: create, run, pause, list, status)", action);
                        }
                    }
                }
            }
        }

        Commands::Env { action } => {
            let collector = EnvCollector::new(storage);
            match action {
                EnvCommands::Show => {
                    let summary = collector.collect_system_summary();
                    println!("=== 系统与硬件规格 ===");
                    println!("OS:       {}", summary.os_name);
                    println!("Kernel:   {}", summary.kernel_version);
                    println!("Host:     {}", summary.host_name);
                    println!("CPU:      {} 核心 ({})", summary.cpu_cores, summary.cpu_brand);
                    println!("Memory:   {} MB 总量 (已用: {} MB, {:.1}%)", summary.total_memory_mb, summary.used_memory_mb, summary.memory_used_pct);
                    println!("Disks:");
                    for d in summary.disks {
                        println!("  - {:<10} 总计 {:.1} GB / 可用 {:.1} GB", d.mount_point, d.total_space_gb, d.available_space_gb);
                    }
                }
                EnvCommands::Processes => {
                    let procs = collector.collect_and_sync_processes()?;
                    println!("活跃动态任务进程 (共 {} 个):", procs.len());
                    println!("{:<8} {:<10} {:<6} {:<10} {:<18} {}", "PID", "NAME", "CPU%", "MEM(MB)", "OWNER", "COMMAND");
                    for p in procs {
                        let mem_mb = p.mem_bytes as f64 / (1024.0 * 1024.0);
                        let cmd_trunc = if p.command.chars().count() > 60 {
                    let t: String = p.command.chars().take(57).collect();
                    format!("{}...", t)
                } else {
                    p.command
                };
                        println!("{:<8} {:<10} {:<6.1} {:<10.1} {:<18} {}", p.pid, p.name, p.cpu_pct, mem_mb, p.owner, cmd_trunc);
                    }
                }
                EnvCommands::Port { port, suggest_count } => {
                    let res = agent_body::env::port::PortChecker::check_port(port, suggest_count)?;
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
            }
        }

        Commands::Memory { action } => {
            match action {
                MemoryCommands::Add { r#type, content, summary } => {
                    let mm = MemoryManager::new(storage);
                    let id = mm.add(&r#type, &content, summary.as_deref(), Some("cli"), vec![])?;
                    println!("✅ 记忆已保存, ID: {}", id);
                }
                MemoryCommands::Search { query, limit, hybrid } => {
                    let t0 = std::time::Instant::now();
                    let provider_ref = storage.provider().clone();
                    let mut out = String::new();
                    if hybrid {
                        let arc_storage = std::sync::Arc::new(storage);
                        let results = tokio::runtime::Runtime::new()?.block_on(async {
                            let engine = agent_body::memory::hybrid::HybridSearchEngine::new(arc_storage);
                            engine.search_hybrid(&query, limit).await
                        })?;
                        out.push_str(&format!("记忆搜索结果 (混合模式 RRF, 共 {} 条):\n", results.len()));
                        for r in results {
                            let sim_str = r.cosine_similarity.map(|s| format!(" (语义相似度: {:.2})", s)).unwrap_or_default();
                            out.push_str(&format!("  - [{}] {} [RRF: {:.4}]{} (ID: {})\n",
                                r.memory.memory_type, r.memory.content, r.rrf_score, sim_str, r.memory.id));
                        }
                    } else {
                        let mm = MemoryManager::new(storage);
                        let results = mm.search(&query, limit)?;
                        out.push_str(&format!("记忆搜索结果 (FTS 纯文本, 共 {} 条):\n", results.len()));
                        for m in results {
                            out.push_str(&format!("  - [{}] {} (ID: {})\n", m.memory_type, m.content, m.id));
                        }
                    }
                    print!("{}", out);
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        provider_ref.as_ref(),
                        "cli",
                        "body_memory_search",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                }
                MemoryCommands::Audit { archive_id } => {
                    let mgr = agent_body::memory::lifecycle::MemoryLifecycleManager::new(&storage);
                    if let Some(aid) = archive_id {
                        mgr.archive_memory(&aid)?;
                        println!("✅ 已将记忆条目 '{}' 状态归档为 'outdated'。", aid);
                    }
                    let rep = mgr.audit_memories()?;
                    println!("长期记忆体检报告 (总记录: {}, 活跃: {}):", rep.total_memories, rep.active_memories);
                    println!("  - 发现对立认知矛盾: {} 处", rep.conflict_count);
                    println!("  - 发现低热度衰减候选: {} 处", rep.decayed_count);
                    if !rep.conflicts.is_empty() {
                        println!("\n⚠️ 冲突矛盾对详情:");
                        for c in &rep.conflicts {
                            println!("\n  冲突原因: {}", c.conflict_reason);
                            println!("  [条目 A ({})]: {}", c.memory_id_a, c.content_a);
                            println!("  [条目 B ({})]: {}", c.memory_id_b, c.content_b);
                            println!("  仲裁建议: {}", c.suggested_resolution);
                        }
                        println!("\n💡 提示: 可通过 'steward memory audit --archive-id <ID>' 归档旧记录。");
                    }
                }
            }
        }

        Commands::Mcp => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let server = agent_body::api::mcp::McpServer::new(storage);
                server.run_stdio().await
            })?;
        }

        Commands::Daemon => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let daemon = agent_body::daemon::StewardDaemon::new(cfg, storage);
                daemon.run().await
            })?;
        }

        Commands::Links { action } => {
            let arc_storage = std::sync::Arc::new(storage);
            let engine = agent_body::links::LinksEngine::new(arc_storage.clone());
            match action {
                LinksCommands::Query { r#type, id, max_hops, min_weight, include_semantic } => {
                    let links = engine.query_links(&r#type, &id)?;
                    println!("文件级关联拓扑 (共 {} 条):", links.len());
                    for l in &links {
                        println!("  - [{}] {} --({})--> [{}] {} (置信度: {:.2})",
                            l.source_type, l.source_id, l.relation, l.target_type, l.target_id, l.confidence);
                    }
                    let tiers: Vec<&str> = if include_semantic {
                        vec!["L1_EXPLICIT", "L2_STRUCTURAL", "L3_SEMANTIC"]
                    } else {
                        vec!["L1_EXPLICIT", "L2_STRUCTURAL"]
                    };
                    let root_cand = if id.contains(':') { id.clone() } else { format!("{}:{}", r#type, id) };
                    if let Ok(sub) = arc_storage.traverse_fact_graph_filtered(&root_cand, max_hops, min_weight, Some(&tiers)) {
                        if !sub.edges.is_empty() {
                            println!("\n属性图谱分层关系 ({} 跳, 共 {} 条边):", max_hops, sub.edges.len());
                            for e in &sub.edges {
                                println!("  - [{} w={:.2} {}] {} --({})--> {}",
                                    e.evidence_tier, e.weight, e.direction_kind, e.source_id, e.relation, e.target_id);
                            }
                        }
                    }
                }
                LinksCommands::Graph { limit, discover_semantic } => {
                    if discover_semantic {
                        let miner = agent_body::fact::GraphRelationshipMiner::new(arc_storage.as_ref());
                        let rep = miner.mine_all(true)?;
                        println!("# 关系挖掘完成: L2上卷={}, 服务桥接={}, 工具={}, 衍生={}, L3语义={}",
                            rep.l2_rollup_edges, rep.l2_service_bridge_edges, rep.l2_tool_usage_edges, rep.l2_derivation_edges, rep.l3_semantic_edges);
                    }
                    let mermaid = engine.export_mermaid(limit)?;
                    println!("{}", mermaid);
                }
                LinksCommands::Impact { target, max_depth, min_weight, include_semantic } => {
                    let analyzer = agent_body::links::impact::ImpactAnalyzer::new(arc_storage);
                    let res = analyzer.analyze_impact_weighted(&target, max_depth, min_weight, include_semantic)?;
                    println!("{}", serde_json::to_string_pretty(&res)?);
                }
                LinksCommands::Audit { fix } => {
                    let t0 = std::time::Instant::now();
                    let auditor = agent_body::links::audit::LinkAuditor::new(arc_storage.clone());
                    let rep = auditor.audit_links(fix)?;
                    println!("{}", rep.summary);
                    if !rep.issues.is_empty() {
                        println!("\n发现以下异常断链 (共 {} 处):", rep.issues.len());
                        for iss in &rep.issues {
                            println!("  - [{}] {} -> {} (行号: {:?})", iss.severity, iss.source, iss.target, iss.line_number);
                            println!("    详情: {}", iss.description);
                            if let Some(snip) = &iss.snippet {
                                println!("    代码: {}", snip);
                            }
                            println!("    建议: {}", iss.suggested_fix);
                        }
                    }
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        arc_storage.provider().as_ref(),
                        "cli",
                        "body_links_audit",
                        &rep.summary,
                        t0.elapsed().as_micros() as i64,
                    );
                }
                LinksCommands::Repair { apply } => {
                    let repairer = agent_body::links::repair::LinkRepairer::new(arc_storage);
                    let rep = repairer.repair_links(apply)?;
                    if rep.suggestions.is_empty() {
                        println!("✅ 未发现可修复的断链或拓扑完全健康。");
                    } else {
                        println!("🔍 发现 {} 处缺失目标，找到 {} 处可自动修复候选 (已应用: {}):",
                            rep.total_missing, rep.reparable_count, rep.applied_count);
                        for sug in &rep.suggestions {
                            println!("\n源文件: {} (行号: {:?})", sug.source_file, sug.line_number);
                            println!("失效目标: {}", sug.missing_target);
                            println!("推荐重定向: {} (相似度: {:.2}, 置信度: {})", sug.candidate_target, sug.similarity, sug.confidence);
                            println!("补丁预览:\n{}", sug.patch_diff);
                        }
                        if !apply && rep.reparable_count > 0 {
                            println!("\n💡 提示: 添加 --apply 参数可自动应用补丁并更新关联表。");
                        }
                    }
                }
                LinksCommands::Codegraph { path, symbol } => {
                    let p = std::path::Path::new(&path);
                    if let Some(sym) = symbol {
                        let callers = agent_body::links::codegraph_bridge::CodeGraphBridge::query_symbol_callers(p, &sym)?;
                        println!("CodeGraph 符号调用方分析: '{}' (共 {} 条):", sym, callers.len());
                        for c in callers {
                            println!("  - [{}] {} ({}:{:?})", c.kind, c.qualified_name, c.file_path, c.line);
                        }
                    } else {
                        let info = agent_body::links::codegraph_bridge::CodeGraphBridge::inspect_project(p)?;
                        match info {
                            Some(stats) => {
                                println!("CodeGraph 工程概览:");
                                println!("  根目录: {}", stats.root_dir);
                                println!("  数据库: {}", stats.db_path);
                                println!("  节点数: {} (Nodes)", stats.node_count);
                                println!("  调用边: {} (Edges)", stats.edge_count);
                                println!("  文件数: {} (Files)", stats.file_count);
                            }
                            None => println!("未在 '{}' 及其父目录中检测到 .codegraph/ 索引。", path),
                        }
                    }
                }
            }
        }

        Commands::Changes { since, limit } => {
            let arc_storage = std::sync::Arc::new(storage);
            let engine = agent_body::changes::ChangesEngine::new(arc_storage);
            let summary = engine.generate_summary_markdown(since.as_deref(), limit)?;
            println!("{}", summary);
        }

        Commands::Close { session_id, clean_temp, dry_run } => {
            let arc_storage = std::sync::Arc::new(storage);
            let sm = agent_body::session::SessionManager::new(arc_storage);
            let res = sm.close_session_with_dry_run(&session_id, clean_temp, dry_run)?;
            println!("{}", serde_json::to_string_pretty(&res)?);
        }

        Commands::Gc { tmp_dir, max_age_hours, apply } => {
            let dry_run = !apply;
            let resolved_tmp_dir = tmp_dir.unwrap_or_else(|| cfg.effective_temp_dir());
            println!("正在扫描宿主垃圾资源 (目录: {:?}, 超期阈值: {} 小时, 模式: {})...",
                resolved_tmp_dir, max_age_hours, if dry_run { "预览模式 (dry-run)" } else { "执行清理 (apply)" });
            let rep = agent_body::env::gc::HostGarbageCollector::run_gc_audited(&storage, &resolved_tmp_dir, max_age_hours, dry_run)?;
            println!("\n🧹 临时文件清理结果:");
            println!("  - 扫描临时目录:   {}", rep.tmp_dir_scanned);
            println!("  - 发现超期候选:   {} 个文件 ({:.2} MB)", rep.candidate_files_count, rep.candidate_bytes_reclaimed as f64 / (1024.0 * 1024.0));
            println!("  - 实际删除释放:   {} 个文件 ({:.2} MB)", rep.deleted_files_count, rep.deleted_bytes_reclaimed as f64 / (1024.0 * 1024.0));
            if dry_run && rep.candidate_files_count > 0 {
                println!("  💡 提示: 添加 --apply 参数可真正物理删除上述超期文件。");
            }

            println!("\n👻 孤儿/失控 Agent Worker 进程诊断 (共 {} 个):", rep.orphan_processes.len());
            if rep.orphan_processes.is_empty() {
                println!("  ✅ 未发现孤儿僵尸进程，系统进程状态健康。");
            } else {
                for proc_ in &rep.orphan_processes {
                    println!("  - [PID {}] {} (内存: {} MB, 运行: {} 秒)", proc_.pid, proc_.name, proc_.memory_mb, proc_.running_secs);
                    println!("    命令: {}", proc_.command);
                    println!("    诊断: {}", proc_.reason);
                }
            }
        }

        Commands::Fact { query, fact_type } => {
            let t0 = std::time::Instant::now();
            let q = query.unwrap_or_else(|| "all".to_string());
            let report = agent_body::fact::FactFacade::query_facts(&fact_type, &q)?;
            println!("{}", report);
            agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage.provider().as_ref(),
                "cli",
                "body_fact_query",
                &report,
                t0.elapsed().as_micros() as i64,
            );
        }

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

        Commands::Bench { action } => {
            use agent_body::benchmark::ledger::EffectivenessLedger;
            use agent_body::benchmark::provenance::ProvenanceRule;
            use agent_body::benchmark::report::{
                render_csv_report, render_markdown_report, BenchmarkReportPayload,
            };
            use agent_body::benchmark::workloads::list_workload_specs;
            use agent_body::benchmark::BenchmarkEngine;

            match action {
                BenchCommands::Workloads => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "workloads": list_workload_specs(),
                            "provenance_rules": ProvenanceRule::list_all(),
                        }))?
                    );
                }
                BenchCommands::Run { suite, save } => {
                    let report = BenchmarkEngine::run_suite(
                        storage.provider().clone(),
                        &suite,
                        save,
                        None,
                    )?;
                    println!("{}", render_markdown_report(&report)?);
                }
                BenchCommands::Report { run_id, format } => {
                    let rec_opt = if let Some(id) = run_id {
                        storage.get_benchmark_run(&id)?
                    } else {
                        storage.list_benchmark_runs(1)?.into_iter().next()
                    };
                    let Some(rec) = rec_opt else {
                        println!("尚无基准评测记录，请先运行 `agent-body bench run`");
                        return Ok(());
                    };
                    let payload = BenchmarkReportPayload::from_record(&rec);
                    match format.as_str() {
                        "json" => println!("{}", serde_json::to_string_pretty(&payload)?),
                        "csv" => println!("{}", render_csv_report(&payload)?),
                        _ => println!("{}", render_markdown_report(&payload)?),
                    }
                }
                BenchCommands::Ledger { window, tool, limit } => {
                    let summary = storage.query_effectiveness_ledger(&window, tool.as_deref(), limit)?;
                    println!("{}", serde_json::to_string_pretty(&summary)?);
                }
                BenchCommands::Compare { run_a, run_b } => {
                    let a = storage.get_benchmark_run(&run_a)?
                        .ok_or_else(|| anyhow::anyhow!("未找到基准批次: {}", run_a))?;
                    let b = storage.get_benchmark_run(&run_b)?
                        .ok_or_else(|| anyhow::anyhow!("未找到基准批次: {}", run_b))?;
                    let diff = EffectivenessLedger::compare_runs(&a, &b);
                    println!("{}", serde_json::to_string_pretty(&diff)?);
                }
                BenchCommands::Export { format, out } => {
                    let rec = storage.list_benchmark_runs(1)?
                        .into_iter()
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("尚无基准评测记录可导出"))?;
                    let payload = BenchmarkReportPayload::from_record(&rec);
                    let content = match format.as_str() {
                        "json" => serde_json::to_string_pretty(&payload)?,
                        "csv" => render_csv_report(&payload)?,
                        _ => render_markdown_report(&payload)?,
                    };
                    if let Some(path) = out {
                        if let Some(parent) = std::path::Path::new(&path).parent() {
                            if !parent.as_os_str().is_empty() {
                                std::fs::create_dir_all(parent)?;
                            }
                        }
                        std::fs::write(&path, &content)?;
                        println!("✅ 已导出实证基准报告至: {} (SHA-256: {})", path, payload.report_sha256);
                    } else {
                        println!("{}", content);
                    }
                }
                BenchCommands::Reset => {
                    let cleaned = BenchmarkEngine::reset_sandbox(storage.provider().as_ref())?;
                    println!("✅ 已清理 {} 条 bench:* 沙盒状态", cleaned);
                }
                BenchCommands::Competitors { track, format } => {
                    let suite = if track.eq_ignore_ascii_case("all") {
                        "competitors".to_string()
                    } else if track.to_ascii_uppercase().starts_with("COMP-") {
                        track
                    } else {
                        format!("COMP-{}", track.to_ascii_uppercase())
                    };
                    let report = BenchmarkEngine::run_suite(
                        storage.provider().clone(),
                        &suite,
                        false,
                        Some("bench:comp"),
                    )?;
                    match format.as_str() {
                        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
                        "csv" => println!("{}", render_csv_report(&report)?),
                        _ => println!("{}", render_markdown_report(&report)?),
                    }
                }
            }
        }

        Commands::Feedback { action } => {
            let t0 = std::time::Instant::now();
            let fb_path = agent_body::feedback::FeedbackEngine::default_feedback_path();
            match action {
                FeedbackCommands::List => {
                    let items = agent_body::feedback::FeedbackEngine::list_items(&fb_path)?;
                    println!("📋 反馈总账 ({}, 共 {} 条):", fb_path.display(), items.len());
                    for it in &items {
                        println!("  - [{}][{}] {} ({})", it.id, it.priority, it.title, it.status);
                    }
                }
                FeedbackCommands::Add { category, priority, title, evidence, target } => {
                    let item = agent_body::feedback::FeedbackEngine::add_item(
                        &fb_path, &category, &priority, &title, &evidence, &target,
                    )?;
                    println!("✅ 已写入反馈 {}: [{}] {}", item.id, item.priority, item.title);
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_feedback",
                        &item.title,
                        t0.elapsed().as_micros() as i64,
                    );
                }
                FeedbackCommands::Sync => {
                    let provider_ref = storage.provider().clone();
                    let added = agent_body::feedback::FeedbackEngine::sync_verified_to_memories(
                        &fb_path, &storage,
                    )?;
                    println!("✅ 已将 {} 条已验证反馈沉淀至长期记忆库 (memories)", added);
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        provider_ref.as_ref(),
                        "cli",
                        "body_feedback",
                        &format!("synced {} items", added),
                        t0.elapsed().as_micros() as i64,
                    );
                }
                FeedbackCommands::Reconcile { json } => {
                    let rep = agent_body::feedback::probes::FreshnessReconciler::reconcile(&storage)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rep)?);
                    } else {
                        println!(
                            "🧹 全生命周期新鲜度对账完成 ({} μs): 检查资产 {} | 标记失活 {} | 内容漂移失效写穿 {} | 剪枝幽灵节点 {} | 归档重复记忆 {} | 过期 CodeGraph 仓库 {:?}",
                            rep.elapsed_us,
                            rep.checked_assets,
                            rep.missing_assets_marked,
                            rep.drifted_assets_refreshed,
                            rep.ghost_fact_nodes_pruned,
                            rep.duplicate_memories_archived,
                            rep.stale_codegraph_repos
                        );
                    }
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_feedback",
                        &format!(
                            "reconcile missing={} drifted={} ghost={} dup_mem={}",
                            rep.missing_assets_marked,
                            rep.drifted_assets_refreshed,
                            rep.ghost_fact_nodes_pruned,
                            rep.duplicate_memories_archived
                        ),
                        t0.elapsed().as_micros() as i64,
                    );
                }
                FeedbackCommands::Probes { auto_heal, auto_feedback, json } => {
                    let rep = agent_body::feedback::probes::SelfEvolutionProbeEngine::run_probes(
                        &storage,
                        &fb_path,
                        auto_heal,
                        auto_feedback,
                    )?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rep)?);
                    } else {
                        println!("🔬 {}", rep.summary);
                        for p in &rep.probes {
                            println!(
                                "  {} [{}] {}: {} (基线: {}) | Δ: {} | 自愈: {} | {}",
                                if p.healthy { "✅" } else { "⚠️" },
                                p.probe_id,
                                p.hook_stage,
                                p.measured_value,
                                p.comparator_baseline,
                                p.delta_summary,
                                p.healed_action,
                                p.recommendation
                            );
                        }
                        if !rep.feedback_items_created.is_empty() {
                            println!("📝 本次自动去重写入反馈: {:?}", rep.feedback_items_created);
                        }
                    }
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_feedback",
                        &format!("probes healthy={} auto_heal={}", rep.overall_healthy, auto_heal),
                        t0.elapsed().as_micros() as i64,
                    );
                }
            }
        }

        Commands::Audit { action } => {
            use agent_body::audit::AuditEvolutionAnalyzer;
            use agent_body::storage::duckdb_analytics::{AnalyticsProvider, DuckDbAnalyticsEngine};
            use agent_body::storage::models::AuditQueryFilter;

            match action {
                AuditCommands::Query {
                    domain,
                    actor_kind,
                    category,
                    outcome,
                    trace_id,
                    target,
                    window,
                    limit,
                    json,
                } => {
                    let rows = storage.query_audit_events(&AuditQueryFilter {
                        domain,
                        actor_kind,
                        category,
                        action: None,
                        outcome,
                        trace_id,
                        session_id: None,
                        target_contains: target,
                        window: Some(window),
                        limit,
                    })?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rows)?);
                    } else {
                        println!("📜 双域行为审计明细 (共 {} 条):", rows.len());
                        for r in &rows {
                            println!(
                                "  [{}] {} | {} | {}.{} | outcome={} | target={} | trace={}",
                                r.created_at,
                                r.domain,
                                r.actor_kind,
                                r.category,
                                r.action,
                                r.outcome,
                                r.target_id,
                                r.trace_id
                            );
                        }
                    }
                }
                AuditCommands::Summary { window, days, json } => {
                    let summary = storage.audit_domain_summary(&window)?;
                    let olap = DuckDbAnalyticsEngine::from_synced_audit_events(storage.provider().clone())
                        .and_then(|eng| eng.audit_analytics_summary(days))
                        .ok();
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "summary": summary,
                                "olap": olap,
                            }))?
                        );
                    } else {
                        println!(
                            "📊 双域行为审计汇总 ({}): 总计 {} 条 | IN_AGENT={} | OUT_OF_AGENT={} | 异常拦截={} | 自愈={}",
                            summary.window,
                            summary.total_events,
                            summary.in_agent_count,
                            summary.out_of_agent_count,
                            summary.failed_or_blocked_count,
                            summary.recovered_count
                        );
                        if let Some(o) = olap {
                            for top in o.top_actions.iter().take(10) {
                                println!(
                                    "  - [{}] {}.{} ({}) -> {} 次 (avg {:.0} μs)",
                                    top.domain, top.category, top.action, top.outcome, top.hit_count, top.avg_elapsed_us
                                );
                            }
                        }
                    }
                }
                AuditCommands::Trace { trace_id, json } => {
                    let rows = storage.query_audit_events(&AuditQueryFilter {
                        trace_id: Some(trace_id.clone()),
                        limit: 100,
                        ..Default::default()
                    })?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rows)?);
                    } else {
                        println!("🔗 Trace `{}` 因果链路 ({} 条):", trace_id, rows.len());
                        for r in &rows {
                            println!(
                                "  [{}] {} | {}.{} | outcome={} | delta={}",
                                r.created_at, r.domain, r.category, r.action, r.outcome, r.before_after_delta
                            );
                        }
                    }
                }
                AuditCommands::Evolve {
                    auto_feedback,
                    min_hits,
                    json,
                } => {
                    let fb_path = agent_body::feedback::FeedbackEngine::default_feedback_path();
                    let rep = AuditEvolutionAnalyzer::run_evolution_cycle(
                        &storage,
                        Some(&fb_path),
                        auto_feedback,
                        min_hits,
                    )?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rep)?);
                    } else {
                        println!(
                            "🧬 审计驱动自进化完成: 评估 {} 条 | 租约降级 {:?} | 租约康复 {:?} | 摩擦热点 {} | 新增反馈 {:?} | 目录规则候选 {}",
                            rep.evaluated_events,
                            rep.downgraded_targets,
                            rep.rehabilitated_targets,
                            rep.friction_hotspots.len(),
                            rep.feedback_items_created,
                            rep.placement_candidates.len()
                        );
                    }
                }
                AuditCommands::Export { output_path, json } => {
                    let resolved_out = output_path.unwrap_or_else(|| {
                        cfg.effective_temp_dir()
                            .join("audit_events_export.csv")
                            .to_string_lossy()
                            .to_string()
                    });
                    let eng = DuckDbAnalyticsEngine::from_synced_audit_events(storage.provider().clone())?;
                    let exp = eng.export_to_parquet("audit_events", &resolved_out)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&exp)?);
                    } else {
                        println!(
                            "📦 已导出 audit_events ({} 行, {} 字节) 至 {}",
                            exp.row_count, exp.file_size_bytes, exp.output_path
                        );
                    }
                }
            }
        }

        Commands::Body { action } => {
            use agent_body::body::{
                AutonomyEnvelope, BodyManifestEngine, ContinuityInput, ContinuityManager,
                HomeostasisVitalsEngine,
            };
            let t0 = std::time::Instant::now();
            match action {
                BodyCommands::Manifest { json } => {
                    let m = BodyManifestEngine::build(&storage);
                    let out = serde_json::to_string_pretty(&m)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!("🧬 Agent 本体身份证: {} ({}, {} / {})", m.body_id, m.hostname, m.os, m.arch);
                        println!("📦 活跃接力胶囊: {} | 六大器官状态:", m.active_checkpoints);
                        for o in &m.organs {
                            println!("  - {:12} [{}] {}", o.name, o.status, o.label);
                        }
                    }
                }
                BodyCommands::Vitals { json } => {
                    let v = HomeostasisVitalsEngine::assess(&storage);
                    let out = serde_json::to_string_pretty(&v)?;
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "🩺 五维本体生命体征 [{}]: 综合稳态分 {:.1}/100 ({})",
                            v.body_id, v.overall_score, v.status
                        );
                        for d in &v.dimensions {
                            println!(
                                "  - {:10} ({:>2}%): {:>5.1} [{}] {}",
                                d.label, d.weight_pct, d.score, d.status, d.detail
                            );
                        }
                        if !v.recommended_l1_actions.is_empty() {
                            println!("💡 建议 L1 自愈动作: {}", v.recommended_l1_actions.join("；"));
                        }
                    }
                }
                BodyCommands::Envelope { action: chk_act, target, dry_run, json } => {
                    if let Some(act) = chk_act {
                        let dec = AutonomyEnvelope::evaluate(&act, target.as_deref(), dry_run);
                        if json {
                            println!("{}", serde_json::to_string_pretty(&dec)?);
                        } else {
                            println!(
                                "🛡️ [{}] allowed={} confirm={} — {}",
                                dec.level_code, dec.allowed, dec.requires_confirmation, dec.reason
                            );
                        }
                    } else {
                        let mat = AutonomyEnvelope::policy_matrix();
                        if json {
                            println!("{}", serde_json::to_string_pretty(&mat)?);
                        } else {
                            for rule in &mat {
                                println!("• {} ({}) — {}", rule.level, rule.label, rule.authorization);
                            }
                        }
                    }
                }
                BodyCommands::Handoff {
                    goal,
                    from_brain,
                    session_id,
                    files,
                    facts,
                    blockers,
                    next_step,
                    json,
                } => {
                    let split_csv = |opt: Option<String>| -> Vec<String> {
                        opt.unwrap_or_default()
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    };
                    let cp = ContinuityManager::save_capsule(
                        &storage,
                        ContinuityInput {
                            from_brain,
                            session_id,
                            goal_summary: goal,
                            active_files: split_csv(files),
                            active_locks: Vec::new(),
                            verified_facts: split_csv(facts),
                            open_blockers: split_csv(blockers),
                            next_step_hint: next_step,
                            token_budget: Some(300),
                        },
                    )?;
                    let out = serde_json::to_string_pretty(&cp)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "🧬 已保存跨大脑接力胶囊 `{}` (来自 `{}` · SHA256={}...)",
                            cp.checkpoint_id,
                            cp.from_brain,
                            cp.evidence_sha256.chars().take(12).collect::<String>()
                        );
                    }
                }
                BodyCommands::Resume { by_brain, checkpoint_id, json } => {
                    let res = ContinuityManager::resume_latest(&storage, &by_brain, checkpoint_id.as_deref())?;
                    let out = serde_json::to_string_pretty(&res)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else if let Some(cp) = res {
                        println!("{}", ContinuityManager::render_brief_snippet(&cp, 300));
                    } else {
                        println!("ℹ️ 当前暂无待接力的活跃跨大脑胶囊。");
                    }
                }
                BodyCommands::Heal { json } => {
                    let rep = HomeostasisVitalsEngine::run_l1_autonomic_heal(&storage)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rep)?);
                    } else {
                        println!(
                            "🩺 L1 稳态反射自愈完成: {:.1} → {:.1} (Δ={:+.2}) | 清理过期锁={} 剪枝幽灵={}",
                            rep.before_score,
                            rep.after_score,
                            rep.delta_score,
                            rep.expired_locks_cleaned,
                            rep.pruned_ghost_nodes
                        );
                    }
                }
                BodyCommands::Peers { json } => {
                    let cfg = Config::load_or_default(None).unwrap_or_default();
                    let client = agent_body::mesh::MeshClient::new(&cfg.mesh, cfg.web.auth_token.clone());
                    let peers = client.query_peer_body_manifests();
                    if json {
                        println!("{}", serde_json::to_string_pretty(&peers)?);
                    } else if peers.is_empty() {
                        println!("ℹ️ 未配置 [mesh].peers 多本体对等节点。");
                    } else {
                        for p in &peers {
                            println!("🌐 {} -> ok={} ({}ms)", p.peer, p.ok, p.latency_ms);
                        }
                    }
                }
                BodyCommands::PerfSnapshot { label, set_baseline, json } => {
                    let snap = agent_body::body::BodyPerfEngine::capture_snapshot(
                        &storage,
                        Some(&label),
                        set_baseline,
                    )?;
                    let out = serde_json::to_string_pretty(&snap)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "📸 已抓取本体 AWR 性能快照 `{}` (label={}, baseline={}) | RSS={} KB | HWM={} KB | FDs={} | 稳态分={:.1}",
                            snap.snapshot_id,
                            snap.label,
                            snap.is_baseline,
                            snap.kernel_telemetry.rss_kb,
                            snap.kernel_telemetry.hwm_kb,
                            snap.kernel_telemetry.open_fds,
                            snap.homeostasis_score
                        );
                    }
                }
                BodyCommands::PerfAwr { snap_a, snap_b, report_id, vs_baseline, json } => {
                    let rep = if let Some(rid) = report_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                        agent_body::body::BodyPerfEngine::load_awr_report(
                            storage.provider().as_ref(),
                            rid,
                        )
                        .ok_or_else(|| anyhow::anyhow!("未找到历史 AWR 报告: {}", rid))?
                    } else {
                        agent_body::body::BodyPerfEngine::generate_awr_report(
                            &storage,
                            snap_a.as_deref(),
                            snap_b.as_deref(),
                            vs_baseline,
                        )?
                    };
                    let out = serde_json::to_string_pretty(&rep)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        print!("{}", rep.markdown_summary);
                    }
                }
                BodyCommands::PerfSnapshots { json } => {
                    let (snapshots, baseline) =
                        agent_body::body::BodyPerfEngine::list_snapshots_meta(storage.provider().as_ref());
                    let payload = serde_json::json!({
                        "baseline": baseline,
                        "snapshots": snapshots,
                        "count": snapshots.len(),
                    });
                    let out = serde_json::to_string_pretty(&payload)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else if snapshots.is_empty() {
                        println!("ℹ️ 快照库暂无已保存的 AWR 性能快照。");
                    } else {
                        println!("📸 已保存 {} 份本体 AWR 性能快照：", snapshots.len());
                        for s in &snapshots {
                            println!(
                                "  - `{}` [{}] {} | RSS={} KB | FDs={} | BodyTime={} μs | 稳态={:.1}",
                                s.snapshot_id,
                                if s.is_baseline { "BASELINE" } else { &s.label },
                                s.captured_at,
                                s.rss_kb,
                                s.open_fds,
                                s.total_body_time_us,
                                s.homeostasis_score
                            );
                        }
                    }
                }
                BodyCommands::PerfReports { json } => {
                    let reports =
                        agent_body::body::BodyPerfEngine::list_awr_reports_meta(storage.provider().as_ref());
                    let payload = serde_json::json!({
                        "reports": reports,
                        "count": reports.len(),
                    });
                    let out = serde_json::to_string_pretty(&payload)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else if reports.is_empty() {
                        println!("ℹ️ 报告库暂无已保存的 AWR 差分诊断报告。");
                    } else {
                        println!("📈 已保存 {} 份 AWR 历史诊断报告：", reports.len());
                        for r in &reports {
                            println!(
                                "  - `{}` [{} → {}] {} | RSS漂移={:+.1}% | 退化={}",
                                r.report_id,
                                r.base_snapshot_id,
                                r.target_snapshot_id,
                                r.generated_at,
                                r.rss_drift_pct,
                                r.regression_detected
                            );
                        }
                    }
                }
                BodyCommands::PerfDelete { kind, id, json } => {
                    let res = agent_body::body::BodyPerfEngine::delete_perf_artifact(
                        &storage,
                        &kind,
                        &id,
                    )?;
                    let out = serde_json::to_string_pretty(&res)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "🗑️ 已处理 AWR 清理请求: kind={}, target_id={}, deleted={}",
                            res.get("kind").and_then(|v| v.as_str()).unwrap_or(&kind),
                            id,
                            res.get("deleted").and_then(|v| v.as_bool()).unwrap_or(false)
                        );
                    }
                }
                BodyCommands::PerfTune { json } => {
                    let rep = agent_body::body::BodyPerfEngine::run_l1_auto_tune(&storage)?;
                    let out = serde_json::to_string_pretty(&rep)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "⚡ L1 内生性能自调优完成 ({} μs): RSS {} KB → {} KB (释放 {:+} KB) | WAL {} KB → {} KB",
                            rep.elapsed_us,
                            rep.before_rss_kb,
                            rep.after_rss_kb,
                            rep.rss_freed_kb,
                            rep.before_wal_bytes / 1024,
                            rep.after_wal_bytes / 1024
                        );
                        for act in &rep.actions_executed {
                            println!("  - {}", act);
                        }
                    }
                }
            }
        }
        }
        Ok(())
    })();

    if !skip_cli_audit {
        let outcome = if exec_res.is_ok() { "success" } else { "failed" };
        agent_body::audit::AuditLogger::record_best_effort(
            &audit_storage,
            agent_body::audit::AuditEventInput::in_agent("cli", "cli_command", cli_cmd_name)
                .with_target("command", cli_cmd_name)
                .with_outcome(outcome)
                .with_elapsed_us(cli_t0.elapsed().as_micros() as i64)
                .with_delta(serde_json::json!({
                    "ok": exec_res.is_ok(),
                    "error": exec_res.as_ref().err().map(|e| e.to_string()),
                })),
        );
    }

    exec_res
}

fn cli_command_label(cmd: &Commands) -> &'static str {
    match cmd {
        Commands::Status => "status",
        Commands::Tui { .. } => "tui",
        Commands::Brief { .. } => "brief",
        Commands::Place { .. } => "place",
        Commands::Asset { .. } => "asset",
        Commands::Env { .. } => "env",
        Commands::Fact { .. } => "fact",
        Commands::Memory { .. } => "memory",
        Commands::Links { .. } => "links",
        Commands::Changes { .. } => "changes",
        Commands::Close { .. } => "close",
        Commands::Gc { .. } => "gc",
        Commands::Mcp => "mcp",
        Commands::Daemon => "daemon",
        Commands::Preflight { .. } => "preflight",
        Commands::Web { .. } => "web",
        Commands::Hooks { .. } => "hooks",
        Commands::Storage { .. } => "storage",
        Commands::Coordination { .. } => "coordination",
        Commands::Analytics { .. } => "analytics",
        Commands::McpCatalog { .. } => "mcp-catalog",
        Commands::Skill { .. } => "skill",
        Commands::Service { .. } => "service",
        Commands::Mesh { .. } => "mesh",
        Commands::Platform { .. } => "platform",
        Commands::Demo { .. } => "demo",
        Commands::Bench { .. } => "bench",
        Commands::Feedback { .. } => "feedback",
        Commands::Audit { .. } => "audit",
        Commands::Body { .. } => "body",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_1to1_mcp_subcommands_parse() {
        assert!(Cli::try_parse_from(["steward", "asset", "register", "--path", "/root/new/tmp/a.py", "--type", "script"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "hooks", "pre", "--tool-name", "bind_port", "--args", "{\"port\":8080}"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "hooks", "post", "--tool-name", "write_file", "--args", "{\"path\":\"/tmp/a\"}"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "coordination", "acquire", "-t", "port", "-i", "9090"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "analytics", "heatmap", "--days", "7"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "analytics", "trend", "--metric", "assets", "--days", "14"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "analytics", "export", "--table", "assets", "--output-path", "/root/new/tmp/a.parquet"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "close", "sess-1", "--dry-run"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "mcp-catalog", "scan"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "skill", "route", "PostgreSQL调优", "--budget-tokens", "500"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "service", "bootstrap"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "coordination", "wait", "-t", "port", "-i", "8080"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "coordination", "deadlock"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "mesh", "port", "8080"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "mesh", "acquire", "--peer", "http://10.0.0.2:8756", "-t", "port", "-i", "9000"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "platform", "ports"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "demo", "run", "scene-1"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "feedback", "list"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "feedback", "sync"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "feedback", "reconcile"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "feedback", "probes", "--auto-heal"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "brief"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "brief", "--query", "postgres", "--budget-tokens", "1200"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "brief", "--legacy"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "audit", "query", "--domain", "OUT_OF_AGENT"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "audit", "summary", "--window", "7d"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "audit", "evolve", "--auto-feedback"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "body", "perf-snapshot", "--label", "gold", "--set-baseline"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "body", "perf-awr", "--vs-baseline", "true"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "body", "perf-tune"]).is_ok());

        // 验证 Clap 4 布尔标志支持显式 false 与隐式 true 写法
        let c1 = Cli::try_parse_from(["agent-body", "memory", "search", "pg", "--hybrid=false"]).expect("hybrid=false");
        assert!(matches!(c1.command, Commands::Memory { action: MemoryCommands::Search { hybrid: false, .. } }));
        let c2 = Cli::try_parse_from(["agent-body", "memory", "search", "pg", "--hybrid", "false"]).expect("hybrid false");
        assert!(matches!(c2.command, Commands::Memory { action: MemoryCommands::Search { hybrid: false, .. } }));
        let c3 = Cli::try_parse_from(["agent-body", "memory", "search", "pg", "--hybrid"]).expect("hybrid implicit true");
        assert!(matches!(c3.command, Commands::Memory { action: MemoryCommands::Search { hybrid: true, .. } }));
        let c4 = Cli::try_parse_from(["agent-body", "close", "s1", "--clean-temp=false"]).expect("clean-temp=false");
        assert!(matches!(c4.command, Commands::Close { clean_temp: false, .. }));
    }
}
