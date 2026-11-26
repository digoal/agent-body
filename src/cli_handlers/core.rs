use anyhow::Result;
use agent_body::brief::{BriefAssembler, BriefRequest};
use agent_body::config::Config;
use agent_body::env::EnvCollector;
use agent_body::memory::MemoryManager;
use agent_body::placement::{PlacementEngine, PlacementRequest};
use agent_body::scanner::Scanner;
use agent_body::storage::Storage;
use crate::cli_args::*;

pub fn handle_core(cmd: Commands, cfg: &Config, storage: Storage) -> Result<()> {
    match cmd {
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
            if cfg.effective_runtime_mode() == agent_body::config::RuntimeMode::Bypass {
                agent_body::body::ShadowAbEngine::record_bypass_event(
                    &storage,
                    "cli",
                    "body_brief",
                    &session_id,
                    &query,
                );
                println!("[agent-body runtime.mode=bypass] 已旁路开工上下文注入，请按原生方式直接探索宿主机。");
                return Ok(());
            }
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
                AssetCommands::Scan {
                    path,
                    enrich,
                    enrich_limit,
                    mine_knowledge,
                    extract_knowhow,
                } => {
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
                        let annotator = agent_body::scanner::OllamaAnnotator::with_default_config(storage.clone());
                        let er = annotator.enrich_pending_assets(enrich_limit, Some(&prefix))?;
                        println!("  - 模型: {} | 在线: {} | 候选: {} | 成功: {} | 跳过: {} | 失败: {} | 耗时: {}ms",
                            er.model_used, er.ollama_available, er.total_candidates, er.enriched_count, er.skipped_count, er.failed_count, er.elapsed_ms);
                        for s in er.samples {
                            println!("    ✅ [{}] {} -> {} (入口: {})", s.domain, s.path, s.purpose, s.entry_point);
                        }
                        let emb_cnt = annotator.embed_pending_assets(enrich_limit, Some(&prefix)).unwrap_or(0);
                        println!("  - Ollama Embedding 向量化完成: {} 个核心资产已生成向量", emb_cnt);
                    }
                    if mine_knowledge {
                        let kg_rep = agent_body::fact::KnowledgeGraphBuilder::mine_from_storage(
                            &storage,
                            &cfg.knowledge_graph,
                        )?;
                        println!(
                            "\n🧠 L1 Know-How 知识网络构建完成: 文档={} | 章节={} | 记忆={} | 概念={} | 层级边={} | 引用边={} | PMI边={}",
                            kg_rep.docs_indexed,
                            kg_rep.sections_created,
                            kg_rep.memories_indexed,
                            kg_rep.terms_created,
                            kg_rep.hierarchy_edges,
                            kg_rep.mention_edges,
                            kg_rep.pmi_edges
                        );
                    }
                    if extract_knowhow || (mine_knowledge && cfg.knowledge_graph.l2_extract_enabled) {
                        let prefix = path.to_string_lossy().to_string();
                        let kh_rep = agent_body::fact::KnowHowExtractor::extract_from_storage(
                            &storage,
                            &cfg.annotation,
                            &cfg.knowledge_graph,
                            enrich_limit.clamp(1, 20),
                            Some(&prefix),
                        )?;
                        println!(
                            "🔬 L2 Know-How 小模型抽取完成: model={} | ollama在线={} | 扫描章节={} | 原文锚定通过={}/{} | 幻觉拦截={} | 新增Know-How={} | 新增边={} | 耗时={}ms",
                            kh_rep.model_used,
                            kh_rep.ollama_available,
                            kh_rep.sections_scanned,
                            kh_rep.triples_grounded,
                            kh_rep.triples_extracted,
                            kh_rep.hallucinations_rejected,
                            kh_rep.knowhow_nodes_created,
                            kh_rep.edges_created,
                            kh_rep.elapsed_ms
                        );
                    }
                }
                AssetCommands::Search {
                    query,
                    limit,
                    detail,
                    modality,
                    domain,
                    kv_key,
                    kv_value,
                    no_hybrid,
                    graph_hops,
                    no_fuzzy,
                    no_links,
                } => {
                    let t0 = std::time::Instant::now();
                    let level = agent_body::scanner::DetailLevel::from_str_opt(Some(&detail));
                    let mut opts = agent_body::scanner::AssetSearchOptions::new(&query, limit);
                    opts.modality = modality;
                    opts.domain = domain;
                    opts.kv_key = kv_key;
                    opts.kv_value = kv_value;
                    opts.hybrid = !no_hybrid;
                    opts.graph_hops = graph_hops.min(2);
                    opts.fuzzy = !no_fuzzy;
                    opts.include_links = !no_links;
                    let hits = agent_body::scanner::AssetSearchEngine::search(
                        storage.provider().as_ref(),
                        &opts,
                    )?;
                    let out = agent_body::scanner::format_search_results(&hits, level);
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
                    println!("{:<8} {:<10} {:<6} {:<10} {:<18} COMMAND", "PID", "NAME", "CPU%", "MEM(MB)", "OWNER");
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

        Commands::Mcp { profile } => {
            let resolved_profile = match profile.as_deref() {
                Some(p) => agent_body::config::McpToolProfile::parse_str(p)
                    .ok_or_else(|| anyhow::anyhow!("未知 --profile '{}'，支持: core | dev | ops | full", p))?,
                None => cfg.effective_mcp_profile(),
            };
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let server = agent_body::api::mcp::McpServer::with_profile(storage, resolved_profile);
                server.run_stdio().await
            })?;
        }

        Commands::Daemon => {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                let daemon = agent_body::daemon::StewardDaemon::new(cfg.clone(), storage);
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

        _ => unreachable!(),
    }
    Ok(())
}
