use std::path::PathBuf;
use serde_json::Value;
use anyhow::{bail, Result};
use crate::storage::Storage;
use crate::storage::models::AssetRecord;
use crate::placement::{PlacementEngine, PlacementRequest};
use crate::scanner::Scanner;
use crate::env::EnvCollector;
use crate::memory::MemoryManager;
use crate::links::LinksEngine;
use crate::changes::ChangesEngine;
use crate::brief::{BriefAssembler, BriefRequest};
use std::sync::Arc;

#[derive(Clone)]
pub struct McpToolRegistry {
    storage: Storage,
    profile: crate::config::McpToolProfile,
}

impl McpToolRegistry {
    pub fn new(storage: Storage) -> Self {
        Self::with_profile(storage, crate::config::McpToolProfile::Full)
    }

    pub fn with_profile(storage: Storage, profile: crate::config::McpToolProfile) -> Self {
        Self { storage, profile }
    }

    /// 列出所有按当前 Profile 暴露的 MCP 工具及其标准 JSON Schema
    pub fn list_tools(&self) -> Vec<Value> {
        crate::api::mcp_schemas::tool_schemas_for_profile(self.profile)
    }


    /// 执行具体工具调用（带生产效能存证与双域审计自动记账）
    pub fn call_tool(&self, name: &str, args: Value) -> Result<String> {
        let t0 = std::time::Instant::now();
        let args_preview = args.clone();
        let res = self.call_tool_inner(name, args);
        let elapsed_us = t0.elapsed().as_micros() as i64;
        if let Ok(ref out) = res {
            crate::benchmark::ledger::EffectivenessLedger::record_mcp_call(
                self.storage.provider().as_ref(),
                name,
                out,
                elapsed_us,
            );
        }
        if name != "body_benchmark" && name != "body_demo_scenario" {
            let outcome = if res.is_ok() { "success" } else { "failed" };
            let target_id = args_preview
                .get("path")
                .or_else(|| args_preview.get("id"))
                .or_else(|| args_preview.get("query"))
                .or_else(|| args_preview.get("resource_id"))
                .or_else(|| args_preview.get("port"))
                .map(|v| v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string()))
                .unwrap_or_default();
            crate::audit::AuditLogger::record_best_effort(
                &self.storage,
                crate::audit::AuditEventInput::in_agent("mcp", "tool_call", name)
                    .with_target("mcp_arg", target_id)
                    .with_outcome(outcome)
                    .with_elapsed_us(elapsed_us)
                    .with_delta(serde_json::json!({
                        "args": args_preview,
                        "ok": res.is_ok(),
                        "error": res.as_ref().err().map(|e| e.to_string()),
                    })),
            );
        }
        res
    }

    fn call_tool_inner(&self, name: &str, args: Value) -> Result<String> {
        match name {
            "body_brief" => {
                let cfg = crate::config::Config::load_effective_or_default();
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let session_id = args.get("session_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let budget_tokens = args.get("budget_tokens").and_then(|v| v.as_u64()).unwrap_or(1500) as usize;
                let mode = cfg.effective_runtime_mode();
                if mode == crate::config::RuntimeMode::Bypass {
                    crate::body::ShadowAbEngine::record_bypass_event(
                        &self.storage,
                        "mcp",
                        "body_brief",
                        &session_id,
                        &query,
                    );
                    return Ok("[agent-body runtime.mode=bypass] 已旁路开工上下文注入，请按原生方式直接探索宿主机。".to_string());
                }

                let provider = cfg.build_memory_provider(self.storage.clone());
                let assembler = BriefAssembler::new(self.storage.clone(), Some(provider));
                let req = BriefRequest {
                    query,
                    session_id,
                    budget_tokens,
                };
                let output = assembler.assemble(&req)?;
                if mode == crate::config::RuntimeMode::ShadowAb && cfg.runtime.shadow_primary == "live" {
                    return Ok(format!(
                        "[agent-body runtime.mode=shadow_ab | shadow_primary=live] 本次简报已旁路注入（后台已采集 {} tokens 对比快照）。",
                        output.tokens_used
                    ));
                }
                Ok(output.markdown)
            }

            "body_placement_suggest" => {
                let engine = PlacementEngine::new(self.storage.clone());
                let object_type = args.get("object_type").and_then(|v| v.as_str()).unwrap_or("project").to_string();
                let name_arg = args.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed").to_string();
                let intent = args.get("intent").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let security_level = args.get("security_level").and_then(|v| v.as_str()).unwrap_or("internal").to_string();

                let req = PlacementRequest {
                    object_type,
                    name: name_arg,
                    intent,
                    tags: vec![],
                    security_level,
                };
                let suggestion = engine.suggest(&req)?;

                let mut out = String::new();
                out.push_str("=== Steward 对象放置建议 ===\n");
                out.push_str(&format!("推荐目标路径: {:?}\n", suggestion.suggested_path));
                out.push_str(&format!("命中规范规则: {}\n", suggestion.rule_matched));
                out.push_str(&format!("规则理由:     {}\n", suggestion.reason));
                out.push_str(&format!("命名建议:     {}\n", suggestion.naming_convention));
                out.push_str(&format!("冲突状态:     {}\n", if suggestion.conflict_detected { "⚠️ 路径已存在冲突！" } else { "✅ 路径空闲可用" }));
                out.push_str("伴随开发与安全指引:\n");
                for g in suggestion.guidelines {
                    out.push_str(&format!("  * {}\n", g));
                }
                Ok(out)
            }

            "body_placement_rules" => {
                let rules = self.storage.get_placement_rules()?;
                let mut out = format!("当前生效的宿主机目录放置规范 (共 {} 条):\n", rules.len());
                for r in rules {
                    out.push_str(&format!("  - [{}] {} -> {} (优先级 {})\n", r.rule_name, r.pattern_type, r.target_base_dir, r.priority));
                    if let Some(desc) = r.description {
                        out.push_str(&format!("    说明: {}\n", desc));
                    }
                }
                Ok(out)
            }

            "body_asset_search" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                let detail = crate::scanner::DetailLevel::from_str_opt(
                    args.get("detail_level").and_then(|v| v.as_str()),
                );
                let mut search_opts = crate::scanner::AssetSearchOptions::new(query, limit);
                search_opts.modality = args.get("modality").and_then(|v| v.as_str()).map(str::to_string);
                search_opts.domain = args.get("domain").and_then(|v| v.as_str()).map(str::to_string);
                search_opts.kv_key = args.get("kv_key").and_then(|v| v.as_str()).map(str::to_string);
                search_opts.kv_value = args.get("kv_value").and_then(|v| v.as_str()).map(str::to_string);
                let cfg = crate::config::Config::load_effective_or_default();
                match cfg.effective_runtime_mode() {
                    crate::config::RuntimeMode::Bypass => {
                        let roots = cfg.effective_workspace_roots();
                        let (hits, _, _) = crate::body::ShadowAbEngine::run_live_workspace_search(
                            &roots,
                            &cfg.assets.ignore_patterns,
                            query,
                            limit,
                            cfg.runtime.shadow_max_live_ms,
                        );
                        crate::body::ShadowAbEngine::record_bypass_event(
                            &self.storage,
                            "mcp",
                            "body_asset_search",
                            "mcp-live",
                            query,
                        );
                        Ok(crate::body::ShadowAbEngine::format_live_hits_text(&hits, query))
                    }
                    crate::config::RuntimeMode::ShadowAb => {
                        let (cmp, live_hits, body_hits) =
                            crate::body::ShadowAbEngine::compare_asset_search(
                                &self.storage,
                                &cfg,
                                query,
                                limit,
                                "mcp-live",
                            )?;
                        let mut out = if cfg.runtime.shadow_primary == "live" {
                            crate::body::ShadowAbEngine::format_live_hits_text(&live_hits, query)
                        } else {
                            let mut buf = format!("资产搜索匹配结果 (共 {} 条, detail={:?}):\n", body_hits.len(), detail);
                            for a in body_hits {
                                buf.push_str(&crate::scanner::format_asset_line(&a, detail));
                                buf.push('\n');
                            }
                            buf
                        };
                        out.push_str(&format!(
                            "[shadow_ab: verdict={}, live={}μs/{}hits, body={}μs/{}hits, recall={:.1}%]\n",
                            cmp.verdict, cmp.live_elapsed_us, cmp.live_hit_count, cmp.body_elapsed_us, cmp.body_hit_count, cmp.recall_vs_live_pct
                        ));
                        Ok(out)
                    }
                    crate::config::RuntimeMode::BodyOnly => {
                        let hits = crate::scanner::AssetSearchEngine::search(
                            self.storage.provider().as_ref(),
                            &search_opts,
                        )?;
                        Ok(crate::scanner::format_search_results(&hits, detail))
                    }
                }
            }

            "body_asset_register" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let asset_type = args.get("type").and_then(|v| v.as_str()).unwrap_or("other");
                let default_owner = crate::config::Config::load_effective_or_default().effective_default_owner();
                let owner = args.get("owner").and_then(|v| v.as_str()).unwrap_or(&default_owner);
                let security_level = args.get("security_level").and_then(|v| v.as_str()).unwrap_or("internal");
                let description = args.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
                let desc_kv = args.get("desc_kv").cloned().unwrap_or(serde_json::json!({}));

                let record = AssetRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    path: path.to_string(),
                    asset_type: asset_type.to_string(),
                    owner: owner.to_string(),
                    security_level: security_level.to_string(),
                    scan_tier: "deep".to_string(),
                    description,
                    desc_kv,
                    usage: None,
                    size_bytes: None,
                    file_count: None,
                    tags: vec![],
                    status: "active".to_string(),
                };
                self.storage.upsert_asset(&record)?;
                Ok(format!("✅ 资产登记成功: {} [{}]", path, asset_type))
            }

            "body_asset_scan" => {
                let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let enrich = args.get("enrich_with_ollama").and_then(|v| v.as_bool()).unwrap_or(false);
                let enrich_limit = args.get("enrich_limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

                let scanner = Scanner::new(self.storage.clone())?;
                let report = scanner.scan_path(&PathBuf::from(path_str))?;

                let mut out = format!("分级颗粒度扫描报告 ({:?}):\n", path_str);
                out.push_str(&format!("  - 扫描目录数:       {}\n", report.scanned_dirs));
                out.push_str(&format!("  - 深层索引文件数:   {} (已完成 L1 确定性元数据提取)\n", report.deep_indexed_files));
                out.push_str(&format!("  - 黑名单阻断目录数: {}\n", report.shallow_blocked_dirs.len()));
                for b in report.shallow_blocked_dirs {
                    out.push_str(&format!("    ★ [Shallow 阻断] {} ({})\n", b.path, b.reason));
                }
                if enrich {
                    let annotator = crate::scanner::OllamaAnnotator::with_default_config(self.storage.clone());
                    let er = annotator.enrich_pending_assets(enrich_limit, Some(path_str))?;
                    out.push_str(&format!(
                        "  - Ollama L2 精炼:   模型={} | 在线={} | 成功={}/{} (耗时 {}ms)\n",
                        er.model_used, er.ollama_available, er.enriched_count, er.total_candidates, er.elapsed_ms
                    ));
                    for s in er.samples {
                        out.push_str(&format!("    🤖 [{}] {} -> {} (入口: {})\n", s.domain, s.path, s.purpose, s.entry_point));
                    }
                }
                Ok(out)
            }

            "body_env_processes" => {
                let collector = EnvCollector::new(self.storage.clone());
                let procs = collector.collect_and_sync_processes()?;
                let mut out = format!("当前活跃动态任务进程 (共 {} 个):\n", procs.len());
                for p in procs {
                    let mem_mb = p.mem_bytes as f64 / (1024.0 * 1024.0);
                    out.push_str(&format!("  - [PID {}] {} (CPU {:.1}%, 内存 {:.1}MB) [{}] -> {}\n", p.pid, p.name, p.cpu_pct, mem_mb, p.owner, p.command));
                }
                Ok(out)
            }

            "body_env_show" => {
                let collector = EnvCollector::new(self.storage.clone());
                let s = collector.collect_system_summary();
                let mut out = String::new();
                out.push_str("=== 系统与硬件规格 ===\n");
                out.push_str(&format!("OS:     {}\n", s.os_name));
                out.push_str(&format!("Kernel: {}\n", s.kernel_version));
                out.push_str(&format!("CPU:    {} 核心 ({})\n", s.cpu_cores, s.cpu_brand));
                out.push_str(&format!("Memory: {} MB 总量 (已用: {} MB, {:.1}%)\n", s.total_memory_mb, s.used_memory_mb, s.memory_used_pct));
                for d in s.disks {
                    out.push_str(&format!("Disk:   {:<10} 总计 {:.1} GB / 可用 {:.1} GB\n", d.mount_point, d.total_space_gb, d.available_space_gb));
                }
                Ok(out)
            }

            "body_memory_add" => {
                let mm = MemoryManager::new(self.storage.clone());
                let mem_type = args.get("type").and_then(|v| v.as_str()).unwrap_or("decision");
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let summary = args.get("summary").and_then(|v| v.as_str());

                let id = mm.add(mem_type, content, summary, Some("mcp"), vec![])?;
                Ok(format!("✅ 长期记忆已录入, ID: {}", id))
            }

            "body_memory_search" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                let use_hybrid = args.get("hybrid").and_then(|v| v.as_bool()).unwrap_or(true);

                if use_hybrid {
                    let arc_storage = Arc::new(self.storage.clone());
                    let engine = crate::memory::hybrid::HybridSearchEngine::new(arc_storage);
                    let rt = tokio::runtime::Handle::current();
                    let results = tokio::task::block_in_place(|| {
                        rt.block_on(engine.search_hybrid(query, limit))
                    })?;

                    let mut out = format!("记忆检索结果 (混合模式 RRF, 共 {} 条):\n", results.len());
                    for r in results {
                        let sim_str = r.cosine_similarity.map(|s| format!(" (语义相似度: {:.2})", s)).unwrap_or_default();
                        out.push_str(&format!("  - [{}] {} [RRF: {:.4}]{} (ID: {})\n",
                            r.memory.memory_type, r.memory.content, r.rrf_score, sim_str, r.memory.id));
                    }
                    Ok(out)
                } else {
                    let mm = MemoryManager::new(self.storage.clone());
                    let results = mm.search(query, limit)?;
                    let mut out = format!("记忆检索结果 (FTS 纯文本, 共 {} 条):\n", results.len());
                    for m in results {
                        out.push_str(&format!("  - [{}] {} (ID: {})\n", m.memory_type, m.content, m.id));
                    }
                    Ok(out)
                }
            }

            "body_links_query" => {
                let arc_storage = Arc::new(self.storage.clone());
                let engine = LinksEngine::new(arc_storage.clone());
                let node_type = args.get("type").and_then(|v| v.as_str()).unwrap_or("asset");
                let node_id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let max_hops = args.get("max_hops").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
                let min_weight = args.get("min_weight").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let include_semantic = args.get("include_semantic").and_then(|v| v.as_bool()).unwrap_or(true);

                let links = engine.query_links(node_type, node_id)?;
                let mut out = format!("文件级实体关联拓扑 (共 {} 条):\n", links.len());
                for l in links {
                    out.push_str(&format!("  - [{}] {} --({})--> [{}] {} (置信度: {:.2})\n",
                        l.source_type, l.source_id, l.relation, l.target_type, l.target_id, l.confidence));
                }

                let tiers: Vec<&str> = if include_semantic {
                    vec!["L1_EXPLICIT", "L2_STRUCTURAL", "L3_SEMANTIC"]
                } else {
                    vec!["L1_EXPLICIT", "L2_STRUCTURAL"]
                };
                let root_cand = if node_id.contains(':') {
                    node_id.to_string()
                } else {
                    format!("{}:{}", node_type, node_id)
                };
                if let Ok(sub) = arc_storage.traverse_fact_graph_filtered(&root_cand, max_hops, min_weight, Some(&tiers)) {
                    if !sub.edges.is_empty() {
                        out.push_str(&format!("\n属性图谱分层关系 ({} 跳, 共 {} 条边):\n", max_hops, sub.edges.len()));
                        for e in &sub.edges {
                            out.push_str(&format!(
                                "  - [{} w={:.2} {}] {} --({})--> {}\n",
                                e.evidence_tier, e.weight, e.direction_kind, e.source_id, e.relation, e.target_id
                            ));
                        }
                    }
                }
                Ok(out)
            }

            "body_links_visualize" => {
                let arc_storage = Arc::new(self.storage.clone());
                let discover = args.get("discover_semantic").and_then(|v| v.as_bool()).unwrap_or(false);
                let mut header = String::new();
                if discover {
                    let miner = crate::fact::GraphRelationshipMiner::new(arc_storage.as_ref());
                    let rep = miner.mine_all(true)?;
                    header = format!(
                        "%% 关系挖掘完成: L2上卷={}, 服务桥接={}, 工具={}, 衍生={}, L3语义={}\n",
                        rep.l2_rollup_edges, rep.l2_service_bridge_edges, rep.l2_tool_usage_edges, rep.l2_derivation_edges, rep.l3_semantic_edges
                    );
                }
                let engine = LinksEngine::new(arc_storage);
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                let mermaid = engine.export_mermaid(limit)?;
                Ok(format!("{}{}", header, mermaid))
            }

            "body_changes_since" => {
                let arc_storage = Arc::new(self.storage.clone());
                let engine = ChangesEngine::new(arc_storage);
                let since = args.get("since").and_then(|v| v.as_str());
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                engine.generate_summary_markdown(since, limit)
            }

            "body_port_check" => {
                let port = args.get("port").and_then(|v| v.as_u64()).unwrap_or(8080) as u16;
                let suggest_count = args.get("suggest_count").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
                let res = crate::env::port::PortChecker::check_port(port, suggest_count)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_links_impact" => {
                let arc_storage = Arc::new(self.storage.clone());
                let analyzer = crate::links::impact::ImpactAnalyzer::new(arc_storage);
                let target = args.get("target").and_then(|v| v.as_str()).unwrap_or("");
                let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
                let min_weight = args.get("min_weight").and_then(|v| v.as_f64()).unwrap_or(0.50) as f32;
                let include_semantic = args.get("include_semantic").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = analyzer.analyze_impact_weighted(target, max_depth, min_weight, include_semantic)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_links_audit" => {
                let arc_storage = Arc::new(self.storage.clone());
                let auditor = crate::links::audit::LinkAuditor::new(arc_storage);
                let fix_dangling = args.get("fix_dangling").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = auditor.audit_links(fix_dangling)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_codegraph_bridge" => {
                let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let symbol_opt = args.get("symbol").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty());
                let p = std::path::Path::new(path_str);

                if let Some(symbol) = symbol_opt {
                    let callers = crate::links::codegraph_bridge::CodeGraphBridge::query_symbol_callers(p, symbol)?;
                    Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "path": path_str,
                        "symbol": symbol,
                        "callers_count": callers.len(),
                        "callers": callers,
                    }))?)
                } else {
                    let info = crate::links::codegraph_bridge::CodeGraphBridge::inspect_project(p)?;
                    Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "path": path_str,
                        "has_codegraph": info.is_some(),
                        "project_info": info,
                    }))?)
                }
            }

            "body_session_close" => {
                let arc_storage = Arc::new(self.storage.clone());
                let sm = crate::session::SessionManager::new(arc_storage);
                let session_id = args.get("session_id").and_then(|v| v.as_str()).unwrap_or("default");
                let clean_temp = args.get("clean_temp").and_then(|v| v.as_bool()).unwrap_or(true);
                let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = sm.close_session_with_dry_run(session_id, clean_temp, dry_run)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_links_repair" => {
                let arc_storage = Arc::new(self.storage.clone());
                let repairer = crate::links::repair::LinkRepairer::new(arc_storage);
                let apply = args.get("apply").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = repairer.repair_links(apply)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_placement_learn" => {
                let learner = crate::placement::learner::PlacementLearner::new(&self.storage);
                let min_samples = args.get("min_samples").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
                let accept = args.get("accept").and_then(|v| v.as_bool()).unwrap_or(false);
                let rep = learner.learn_rules(min_samples)?;
                let mut accepted_rules = Vec::new();
                if accept {
                    for c in &rep.candidates {
                        let rule = learner.apply_candidate(c)?;
                        accepted_rules.push(rule);
                    }
                }
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "learn_report": rep,
                    "accepted_count": accepted_rules.len(),
                    "accepted_rules": accepted_rules,
                }))?)
            }

            "body_env_gc" => {
                let default_tmp = crate::config::Config::load_effective_or_default().effective_temp_dir();
                let tmp_dir_buf = args
                    .get("tmp_dir")
                    .and_then(|v| v.as_str())
                    .map(PathBuf::from)
                    .unwrap_or(default_tmp);
                let max_age_hours = args.get("max_age_hours").and_then(|v| v.as_u64()).unwrap_or(72);
                let apply = args.get("apply").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = crate::env::gc::HostGarbageCollector::run_gc(&tmp_dir_buf, max_age_hours, !apply)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_memory_audit" => {
                let mgr = crate::memory::lifecycle::MemoryLifecycleManager::new(&self.storage);
                if let Some(aid) = args.get("archive_id").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()) {
                    mgr.archive_memory(aid)?;
                }
                let rep = mgr.audit_memories()?;
                Ok(serde_json::to_string_pretty(&rep)?)
            }

            "body_fact_query" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("all");
                let fact_type = args.get("fact_type").and_then(|v| v.as_str()).unwrap_or("auto");
                crate::fact::FactFacade::query_facts(fact_type, query)
            }

            "body_hook_pre" => {
                let tool_name = args.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                let tool_args = args.get("args").cloned().unwrap_or(serde_json::json!({}));
                let engine = crate::api::hooks::AgentHookEngine::new(self.storage.clone());
                let res = engine.pre_tool_call(tool_name, &tool_args);
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_hook_post" => {
                let tool_name = args.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                let tool_args = args.get("args").cloned().unwrap_or(serde_json::json!({}));
                let success = args.get("success").and_then(|v| v.as_bool()).unwrap_or(true);
                let error_output = args.get("error_output").and_then(|v| v.as_str());
                let engine = crate::api::hooks::AgentHookEngine::new(self.storage.clone());
                let res = engine.post_tool_call_with_error(tool_name, &tool_args, success, error_output);
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_hook_generate" => {
                let client_str = args.get("client").and_then(|v| v.as_str()).unwrap_or("agy");
                let client_type = client_str.parse::<crate::hooks::ClientType>()?;
                let out_dir = args.get("out_dir").and_then(|v| v.as_str()).map(std::path::Path::new);
                let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

                let res = crate::hooks::ClientHookGenerator::generate(client_type, out_dir, dry_run)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_analytics_heatmap" => {
                use crate::storage::AnalyticsProvider;
                let days = args.get("days").and_then(|v| v.as_u64()).unwrap_or(30) as usize;
                let engine = crate::storage::DuckDbAnalyticsEngine::from_synced_storage(self.storage.provider().clone())?;
                let heatmap = engine.changes_heatmap(days)?;
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "time_span_days": days,
                    "heatmap": heatmap,
                }))?)
            }

            "body_analytics_trend" => {
                use crate::storage::AnalyticsProvider;
                let days = args.get("days").and_then(|v| v.as_u64()).unwrap_or(30) as usize;
                let engine = crate::storage::DuckDbAnalyticsEngine::from_synced_storage(self.storage.provider().clone())?;
                let trend = engine.asset_growth_trend(days)?;
                let decay = engine.memory_decay_curve()?;
                let stats = engine.fact_graph_stats()?;
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "time_span_days": days,
                    "asset_growth_trend": trend,
                    "memory_decay": decay,
                    "graph_stats": stats,
                }))?)
            }

            "body_analytics_export" => {
                use crate::storage::AnalyticsProvider;
                let table = args.get("table").and_then(|v| v.as_str()).unwrap_or("assets");
                let default_out = crate::config::Config::load_effective_or_default()
                    .effective_temp_dir()
                    .join("export.parquet");
                let default_out_str = default_out.to_string_lossy().to_string();
                let output_path = args.get("output_path").and_then(|v| v.as_str()).unwrap_or(&default_out_str);
                let engine = crate::storage::DuckDbAnalyticsEngine::from_synced_storage(self.storage.provider().clone())?;
                let summary = engine.export_to_parquet(table, output_path)?;
                Ok(serde_json::to_string_pretty(&summary)?)
            }

            "body_lock_acquire" => {
                use crate::coordination::{CoordinationEngine, LockRequest, ResourceType, LockScope};
                let rt_str = args.get("resource_type").and_then(|v| v.as_str()).unwrap_or("custom");
                let res_id = args.get("resource_id").and_then(|v| v.as_str()).unwrap_or("");
                let scope_str = args.get("scope").and_then(|v| v.as_str()).unwrap_or("exclusive");
                let holder = args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("current-agent");
                let pid = args.get("holder_pid").and_then(|v| v.as_u64()).map(|p| p as u32);
                let ttl = args.get("ttl_seconds").and_then(|v| v.as_u64()).unwrap_or(300);
                let meta = args.get("metadata").cloned();

                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let req = LockRequest {
                    resource_type: ResourceType::from_str_loose(rt_str),
                    resource_id: res_id.to_string(),
                    scope: LockScope::from_str_loose(scope_str),
                    holder_agent: holder.to_string(),
                    holder_pid: pid,
                    ttl_seconds: ttl,
                    metadata: meta,
                };
                let res = engine.try_acquire(&req)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_lock_release" => {
                use crate::coordination::CoordinationEngine;
                let lock_id = args.get("lock_id").and_then(|v| v.as_str());
                let resource_type = args.get("resource_type").and_then(|v| v.as_str());
                let resource_id = args.get("resource_id").and_then(|v| v.as_str());
                let holder = args
                    .get("holder_agent")
                    .or_else(|| args.get("agent_id"))
                    .and_then(|v| v.as_str())
                    .or(Some("current-agent"));
                let force = args.get("force").and_then(|v| v.as_bool()).unwrap_or(false);
                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let (resolved_id, released) = engine.release_flexible(
                    lock_id,
                    resource_type,
                    resource_id,
                    holder,
                    force,
                )?;
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "lock_id": resolved_id.unwrap_or_default(),
                    "resource_type": resource_type,
                    "resource_id": resource_id,
                    "released": released,
                    "force": force,
                }))?)
            }

            "body_lock_list" => {
                use crate::coordination::{CoordinationEngine, ResourceType};
                let rt_opt = args.get("resource_type").and_then(|v| v.as_str()).map(ResourceType::from_str_loose);
                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let list = engine.list_active_locks(rt_opt.as_ref())?;
                Ok(serde_json::to_string_pretty(&list)?)
            }

            "body_mcp_catalog" => {
                use crate::ecosystem::McpConfigScanner;
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("route");
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
                let scanner = McpConfigScanner::new(self.storage.clone());
                match action {
                    "scan" => {
                        let (cnt, sanitized) = scanner.scan_default_clients()?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "mcp_servers_discovered": cnt,
                            "sanitized_env_keys_count": sanitized
                        }))?)
                    }
                    "list" => {
                        let list = self.storage.list_mcp_catalog()?;
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                    _ => {
                        let list = if query.trim().is_empty() {
                            self.storage.list_mcp_catalog()?.into_iter().take(limit).collect()
                        } else {
                            self.storage.search_mcp_catalog(query, limit)?
                        };
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                }
            }

            "body_skill_route" => {
                use crate::ecosystem::{ContextBudgetRouter, ContextRouteRequest, SkillCatalogScanner};
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("route");
                let intent = args.get("intent").and_then(|v| v.as_str()).unwrap_or("");
                let budget_tokens = args.get("budget_tokens").and_then(|v| v.as_u64()).unwrap_or(600) as usize;
                let max_skills = args.get("max_skills").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
                match action {
                    "scan" => {
                        let scanner = SkillCatalogScanner::new(self.storage.clone());
                        let (cnt, tokens) = scanner.scan_default_roots()?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "skills_discovered": cnt,
                            "total_full_tokens_estimated": tokens
                        }))?)
                    }
                    "list" => {
                        let list = self.storage.list_skills_catalog()?;
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                    _ => {
                        let router = ContextBudgetRouter::new(self.storage.clone());
                        let plan = router.route(&ContextRouteRequest {
                            intent: intent.to_string(),
                            budget_tokens,
                            max_skills,
                            max_mcps: 4,
                            max_services: 3,
                        })?;
                        Ok(serde_json::to_string_pretty(&plan)?)
                    }
                }
            }

            "body_service_contact" => {
                use crate::ecosystem::ExternalContactsManager;
                use crate::storage::models::ExternalServiceRecord;
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
                let mgr = ExternalContactsManager::new(self.storage.clone());
                match action {
                    "register" => {
                        let service_name = args.get("service_name").and_then(|v| v.as_str()).unwrap_or("unnamed-service");
                        let rec = ExternalServiceRecord {
                            id: format!("svc:{}", service_name),
                            service_name: service_name.to_string(),
                            category: args.get("category").and_then(|v| v.as_str()).unwrap_or("api").to_string(),
                            endpoint: args.get("endpoint").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            protocol: args.get("protocol").and_then(|v| v.as_str()).unwrap_or("https").to_string(),
                            capabilities: args.get("capabilities").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            auth_type: args.get("auth_type").and_then(|v| v.as_str()).unwrap_or("none").to_string(),
                            auth_env_var: args.get("auth_env_var").and_then(|v| v.as_str()).map(String::from),
                            health_endpoint: args.get("health_endpoint").and_then(|v| v.as_str()).map(String::from),
                            usage_example: args.get("usage_example").and_then(|v| v.as_str()).map(String::from),
                            status: "unknown".to_string(),
                            last_checked_at: None,
                        };
                        mgr.register_service(&rec)?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "registered": true,
                            "service_name": rec.service_name,
                            "auth_env_ready": ExternalContactsManager::is_auth_env_ready(rec.auth_env_var.as_deref())
                        }))?)
                    }
                    "check" => {
                        let name_arg = args.get("service_name").and_then(|v| v.as_str()).unwrap_or("");
                        let rep = mgr.check_health(name_arg)?;
                        Ok(serde_json::to_string_pretty(&rep)?)
                    }
                    "query" => {
                        let q = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                        let list = self.storage.search_external_services(q, 10)?;
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                    "bootstrap" => {
                        let cfg = crate::config::Config::load_or_default(None).unwrap_or_default();
                        let count = mgr.bootstrap_from_config(&cfg)?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({ "bootstrapped": count }))?)
                    }
                    _ => {
                        let cat = args.get("category").and_then(|v| v.as_str());
                        let list = self.storage.list_external_services(cat)?;
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                }
            }

            "body_lock_wait" => {
                use crate::coordination::{CoordinationEngine, LockRequest, LockScope, ResourceType};
                let rt_str = args.get("resource_type").and_then(|v| v.as_str()).unwrap_or("custom");
                let res_id = args.get("resource_id").and_then(|v| v.as_str()).unwrap_or("");
                let scope_str = args.get("scope").and_then(|v| v.as_str()).unwrap_or("exclusive");
                let holder = args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("current-agent");
                let ttl = args.get("ttl_seconds").and_then(|v| v.as_u64()).unwrap_or(300);
                let wait_ms = args.get("wait_timeout_ms").and_then(|v| v.as_u64()).unwrap_or(5000);
                let priority = args.get("priority").and_then(|v| v.as_i64()).unwrap_or(100) as i32;

                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let req = LockRequest {
                    resource_type: ResourceType::from_str_loose(rt_str),
                    resource_id: res_id.to_string(),
                    scope: LockScope::from_str_loose(scope_str),
                    holder_agent: holder.to_string(),
                    holder_pid: None,
                    ttl_seconds: ttl,
                    metadata: None,
                };
                let res = engine.wait_and_acquire(&req, wait_ms, priority)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "body_lock_deadlock_check" => {
                use crate::coordination::CoordinationEngine;
                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let rep = engine.detect_deadlocks()?;
                let waits = engine.list_wait_queue(None)?;
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "deadlock_report": rep,
                    "wait_queue_count": waits.len(),
                    "wait_queue": waits,
                }))?)
            }

            "body_mesh_query" => {
                use crate::coordination::{LockRequest, LockScope, ResourceType};
                use crate::mesh::{build_local_facts, MeshClient};
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("ping");
                let cfg = crate::config::Config::load_or_default(None).unwrap_or_default();
                let client = MeshClient::new(&cfg.mesh, cfg.web.auth_token.clone());
                match action {
                    "local" => Ok(serde_json::to_string_pretty(&build_local_facts())?),
                    "peers" => Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "enabled": cfg.mesh.enabled,
                        "peers": client.peers(),
                        "timeout_ms": cfg.mesh.timeout_ms,
                    }))?),
                    "facts" => Ok(serde_json::to_string_pretty(&client.facts_all())?),
                    "locks" => Ok(serde_json::to_string_pretty(&client.locks_all())?),
                    "port" => {
                        let port = args.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
                        Ok(serde_json::to_string_pretty(&client.federated_port_check(port))?)
                    }
                    "acquire" => {
                        let peer = args.get("peer").and_then(|v| v.as_str()).unwrap_or("");
                        if peer.is_empty() {
                            bail!("action=acquire 必须提供 peer 参数");
                        }
                        let req = LockRequest {
                            resource_type: ResourceType::from_str_loose(args.get("resource_type").and_then(|v| v.as_str()).unwrap_or("custom")),
                            resource_id: args.get("resource_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            scope: LockScope::from_str_loose(args.get("scope").and_then(|v| v.as_str()).unwrap_or("exclusive")),
                            holder_agent: args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("current-agent").to_string(),
                            holder_pid: None,
                            ttl_seconds: args.get("ttl_seconds").and_then(|v| v.as_u64()).unwrap_or(300),
                            metadata: None,
                        };
                        Ok(serde_json::to_string_pretty(&client.acquire_on_peer(peer, &req)?)?)
                    }
                    "graph" => {
                        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                        let hops = args.get("hops").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
                        Ok(serde_json::to_string_pretty(&client.federated_graph_query(query, hops))?)
                    }
                    "dialogue_send" => {
                        let peer = args.get("peer").and_then(|v| v.as_str()).unwrap_or("");
                        if peer.is_empty() {
                            bail!("action=dialogue_send 必须提供 peer 参数");
                        }
                        let thread_id = args.get("thread_id").and_then(|v| v.as_str()).unwrap_or("");
                        let intent = args.get("intent").and_then(|v| v.as_str()).unwrap_or("ASK_CONTEXT");
                        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        let to_agent = args.get("to_agent").and_then(|v| v.as_str()).unwrap_or("");
                        let from_agent = args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("mcp-agent");
                        Ok(serde_json::to_string_pretty(&client.send_dialogue(
                            peer, thread_id, intent, content, from_agent, to_agent,
                        )?)?)
                    }
                    "dialogue_inbox" => {
                        let rows = self.storage.list_mesh_dialogues(30)?;
                        Ok(serde_json::to_string_pretty(&rows)?)
                    }
                    "dialogue_ack" => {
                        let did = args.get("dialogue_id").and_then(|v| v.as_str()).unwrap_or("");
                        let updated = self.storage.update_mesh_dialogue_status(did, "acked")?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({ "dialogue_id": did, "acked": updated }))?)
                    }
                    "transfer_send" => {
                        let peer = args.get("peer").and_then(|v| v.as_str()).unwrap_or("");
                        let file_path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
                        if peer.is_empty() || file_path.is_empty() {
                            bail!("action=transfer_send 必须提供 peer 与 file_path 参数");
                        }
                        let from_agent = args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("mcp-agent");
                        Ok(serde_json::to_string_pretty(&client.send_transfer(
                            peer, std::path::Path::new(file_path), from_agent,
                        )?)?)
                    }
                    "transfer_inbox" => {
                        let rows = self.storage.list_mesh_transfers(30)?;
                        Ok(serde_json::to_string_pretty(&rows)?)
                    }
                    "transfer_accept" => {
                        let tid = args.get("transfer_id").and_then(|v| v.as_str()).unwrap_or("");
                        let default_dest = crate::config::Config::load_effective_or_default()
                            .effective_temp_dir()
                            .join("mesh_accepted");
                        let dest_buf = args
                            .get("dest_dir")
                            .and_then(|v| v.as_str())
                            .map(PathBuf::from)
                            .unwrap_or(default_dest);
                        let rec = crate::mesh::transfer::accept_transfer_to_dir(
                            self.storage.provider().as_ref(),
                            tid,
                            &dest_buf,
                        )?;
                        Ok(serde_json::to_string_pretty(&rec)?)
                    }
                    "transfer_reject" => {
                        let tid = args.get("transfer_id").and_then(|v| v.as_str()).unwrap_or("");
                        let ok = crate::mesh::transfer::reject_transfer(self.storage.provider().as_ref(), tid)?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({ "transfer_id": tid, "rejected": ok }))?)
                    }
                    _ => Ok(serde_json::to_string_pretty(&client.ping_all())?),
                }
            }

            "body_demo_scenario" => {
                use crate::demo::DemoEngine;
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
                match action {
                    "run" => {
                        let id = args.get("scenario_id").and_then(|v| v.as_str()).unwrap_or("scene-1");
                        let res = DemoEngine::run_scenario(self.storage.provider().clone(), id)?;
                        Ok(serde_json::to_string_pretty(&res)?)
                    }
                    "roi" => {
                        let roi = DemoEngine::compute_roi(self.storage.provider().as_ref())?;
                        Ok(serde_json::to_string_pretty(&roi)?)
                    }
                    "reset" => {
                        let cleaned = DemoEngine::reset_sandbox(self.storage.provider().as_ref())?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({ "cleaned": cleaned }))?)
                    }
                    "parity" => {
                        let tools = self.list_tools();
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "total_mcp_tools": tools.len(),
                            "tools": tools.iter().filter_map(|t| t.get("name").and_then(|v| v.as_str())).collect::<Vec<_>>(),
                        }))?)
                    }
                    _ => Ok(serde_json::to_string_pretty(&DemoEngine::list_scenarios())?),
                }
            }

            "body_benchmark" => {
                use crate::benchmark::ledger::EffectivenessLedger;
                use crate::benchmark::provenance::ProvenanceRule;
                use crate::benchmark::report::{
                    render_csv_report, render_markdown_report, BenchmarkReportPayload,
                };
                use crate::benchmark::workloads::list_workload_specs;
                use crate::benchmark::BenchmarkEngine;

                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("report");
                match action {
                    "workloads" => Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "workloads": list_workload_specs(),
                        "provenance_rules": ProvenanceRule::list_all(),
                    }))?),
                    "run" => {
                        let suite = args.get("suite").and_then(|v| v.as_str()).unwrap_or("full");
                        let save = args.get("save").and_then(|v| v.as_bool()).unwrap_or(true);
                        let report = BenchmarkEngine::run_suite(
                            self.storage.provider().clone(),
                            suite,
                            save,
                            None,
                        )?;
                        Ok(serde_json::to_string_pretty(&report)?)
                    }
                    "report" => {
                        let run_id = args.get("run_id").and_then(|v| v.as_str());
                        let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("json");
                        let rec_opt = if let Some(id) = run_id {
                            self.storage.get_benchmark_run(id)?
                        } else {
                            self.storage.list_benchmark_runs(1)?.into_iter().next()
                        };
                        let Some(rec) = rec_opt else {
                            return Ok(serde_json::to_string_pretty(&serde_json::json!({
                                "status": "empty",
                                "message": "尚无基准评测记录，请先执行 body_benchmark(action='run')"
                            }))?);
                        };
                        let payload = BenchmarkReportPayload::from_record(&rec);
                        match format {
                            "markdown" => render_markdown_report(&payload),
                            "csv" => render_csv_report(&payload),
                            _ => Ok(serde_json::to_string_pretty(&payload)?),
                        }
                    }
                    "ledger" => {
                        let window = args.get("window").and_then(|v| v.as_str()).unwrap_or("7d");
                        let tool_filter = args.get("tool_filter").and_then(|v| v.as_str());
                        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                        let summary = self.storage.query_effectiveness_ledger(window, tool_filter, limit)?;
                        Ok(serde_json::to_string_pretty(&summary)?)
                    }
                    "compare" => {
                        let a_id = args.get("run_a").and_then(|v| v.as_str()).unwrap_or("");
                        let b_id = args.get("run_b").and_then(|v| v.as_str()).unwrap_or("");
                        let Some(a) = self.storage.get_benchmark_run(a_id)? else {
                            bail!("未找到基准批次 run_a: {}", a_id);
                        };
                        let Some(b) = self.storage.get_benchmark_run(b_id)? else {
                            bail!("未找到基准批次 run_b: {}", b_id);
                        };
                        let diff = EffectivenessLedger::compare_runs(&a, &b);
                        Ok(serde_json::to_string_pretty(&diff)?)
                    }
                    "export" => {
                        let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("markdown");
                        let rec_opt = self.storage.list_benchmark_runs(1)?.into_iter().next();
                        let Some(rec) = rec_opt else {
                            bail!("尚无基准评测记录可导出");
                        };
                        let payload = BenchmarkReportPayload::from_record(&rec);
                        match format {
                            "csv" => render_csv_report(&payload),
                            "json" => Ok(serde_json::to_string_pretty(&payload)?),
                            _ => render_markdown_report(&payload),
                        }
                    }
                    "reset" => {
                        let cleaned = BenchmarkEngine::reset_sandbox(self.storage.provider().as_ref())?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({ "cleaned": cleaned }))?)
                    }
                    "competitors" => {
                        let track = args.get("track").and_then(|v| v.as_str()).unwrap_or("all");
                        let results = crate::benchmark::competitors::run_competitor_track(
                            self.storage.provider().clone(),
                            track,
                        )?;
                        let matrix = crate::benchmark::competitors::build_competitor_capability_matrix();
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "spec_version": crate::benchmark::BENCHMARK_SPEC_VERSION,
                            "results": results,
                            "capability_matrix": matrix,
                        }))?)
                    }
                    other => bail!("未知 benchmark action: {}", other),
                }
            }

            "body_feedback" => {
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
                let fb_path = crate::feedback::FeedbackEngine::default_feedback_path();
                match action {
                    "list" => {
                        let items = crate::feedback::FeedbackEngine::list_items(&fb_path)?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "path": fb_path.to_string_lossy(),
                            "count": items.len(),
                            "items": items
                        }))?)
                    }
                    "add" => {
                        let category = args.get("category").and_then(|v| v.as_str()).unwrap_or("CON");
                        let priority = args.get("priority").and_then(|v| v.as_str()).unwrap_or("P1");
                        let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let evidence = args.get("evidence").and_then(|v| v.as_str()).unwrap_or("");
                        let target = args.get("target").and_then(|v| v.as_str()).unwrap_or("");
                        let item = crate::feedback::FeedbackEngine::add_item(
                            &fb_path, category, priority, title, evidence, target,
                        )?;
                        Ok(serde_json::to_string_pretty(&item)?)
                    }
                    "sync" => {
                        let added = crate::feedback::FeedbackEngine::sync_verified_to_memories(
                            &fb_path,
                            &self.storage,
                        )?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "synced_to_memories": added,
                            "path": fb_path.to_string_lossy()
                        }))?)
                    }
                    "reconcile" => {
                        let rep = crate::feedback::probes::FreshnessReconciler::reconcile(&self.storage)?;
                        Ok(serde_json::to_string_pretty(&rep)?)
                    }
                    "probes" => {
                        let auto_heal = args.get("auto_heal").and_then(|v| v.as_bool()).unwrap_or(true);
                        let auto_feedback = args.get("auto_feedback").and_then(|v| v.as_bool()).unwrap_or(false);
                        let rep = crate::feedback::probes::SelfEvolutionProbeEngine::run_probes(
                            &self.storage,
                            &fb_path,
                            auto_heal,
                            auto_feedback,
                        )?;
                        Ok(serde_json::to_string_pretty(&rep)?)
                    }
                    other => bail!("未知 feedback action: {}", other),
                }
            }

            "body_audit_query" => {
                use crate::storage::duckdb_analytics::{AnalyticsProvider, DuckDbAnalyticsEngine};
                use crate::storage::models::AuditQueryFilter;

                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("query");
                match action {
                    "summary" => {
                        let window = args.get("window").and_then(|v| v.as_str()).unwrap_or("7d");
                        let days = args.get("days").and_then(|v| v.as_u64()).unwrap_or(7) as usize;
                        let summary = self.storage.audit_domain_summary(window)?;
                        let olap = DuckDbAnalyticsEngine::from_synced_audit_events(self.storage.provider().clone())
                            .and_then(|eng| eng.audit_analytics_summary(days))
                            .ok();
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "summary": summary,
                            "olap": olap,
                        }))?)
                    }
                    "trace" => {
                        let trace_id = args.get("trace_id").and_then(|v| v.as_str()).unwrap_or("");
                        if trace_id.trim().is_empty() {
                            bail!("action=trace 必须提供 trace_id");
                        }
                        let rows = self.storage.query_audit_events(&AuditQueryFilter {
                            trace_id: Some(trace_id.to_string()),
                            limit: args.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize,
                            ..Default::default()
                        })?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "trace_id": trace_id,
                            "count": rows.len(),
                            "events": rows,
                        }))?)
                    }
                    "evolve" => {
                        let auto_fb = args.get("auto_feedback").and_then(|v| v.as_bool()).unwrap_or(false);
                        let fb_path = crate::feedback::FeedbackEngine::default_feedback_path();
                        let rep = crate::audit::AuditEvolutionAnalyzer::run_evolution_cycle(
                            &self.storage,
                            Some(&fb_path),
                            auto_fb,
                            3,
                        )?;
                        Ok(serde_json::to_string_pretty(&rep)?)
                    }
                    "export" => {
                        let default_out = crate::config::Config::load_effective_or_default()
                            .effective_temp_dir()
                            .join("audit_events_export.csv");
                        let default_out_str = default_out.to_string_lossy().to_string();
                        let out_path = args
                            .get("output_path")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&default_out_str);
                        let engine = DuckDbAnalyticsEngine::from_synced_audit_events(self.storage.provider().clone())?;
                        let exp = engine.export_to_parquet("audit_events", out_path)?;
                        Ok(serde_json::to_string_pretty(&exp)?)
                    }
                    _ => {
                        let filter = AuditQueryFilter {
                            domain: args.get("domain").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            actor_kind: args.get("actor_kind").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            category: args.get("category").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            action: args.get("filter_action").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            outcome: args.get("outcome").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            trace_id: args.get("trace_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            session_id: args.get("session_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            target_contains: args.get("target_contains").and_then(|v| v.as_str()).map(|s| s.to_string()),
                            window: Some(args.get("window").and_then(|v| v.as_str()).unwrap_or("7d").to_string()),
                            limit: args.get("limit").and_then(|v| v.as_u64()).unwrap_or(30) as usize,
                        };
                        let rows = self.storage.query_audit_events(&filter)?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "count": rows.len(),
                            "events": rows,
                        }))?)
                    }
                }
            }

            "body_state" => {
                use crate::body::{
                    AutonomyEnvelope, BodyManifestEngine, ContinuityInput, ContinuityManager,
                    HomeostasisVitalsEngine,
                };
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("manifest");
                match action {
                    "manifest" => {
                        let m = BodyManifestEngine::build(&self.storage);
                        Ok(serde_json::to_string_pretty(&m)?)
                    }
                    "vitals" => {
                        let v = HomeostasisVitalsEngine::assess(&self.storage);
                        Ok(serde_json::to_string_pretty(&v)?)
                    }
                    "envelope" => {
                        if let Some(chk_act) = args.get("check_action").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()) {
                            let target = args.get("target").and_then(|v| v.as_str());
                            let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(true);
                            let dec = AutonomyEnvelope::evaluate(chk_act, target, dry_run);
                            Ok(serde_json::to_string_pretty(&dec)?)
                        } else {
                            Ok(serde_json::to_string_pretty(&serde_json::json!({
                                "matrix": AutonomyEnvelope::policy_matrix(),
                            }))?)
                        }
                    }
                    "handoff_save" => {
                        let goal = args
                            .get("goal_summary")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .trim();
                        if goal.is_empty() {
                            bail!("action=handoff_save 必须提供 goal_summary");
                        }
                        let parse_vec = |k: &str| -> Vec<String> {
                            args.get(k)
                                .and_then(|v| v.as_array())
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                                        .collect()
                                })
                                .unwrap_or_default()
                        };
                        let cp = ContinuityManager::save_capsule(
                            &self.storage,
                            ContinuityInput {
                                from_brain: args
                                    .get("from_brain")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("mcp-brain")
                                    .to_string(),
                                session_id: args
                                    .get("session_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("mcp-live")
                                    .to_string(),
                                goal_summary: goal.to_string(),
                                active_files: parse_vec("active_files"),
                                active_locks: Vec::new(),
                                verified_facts: parse_vec("verified_facts"),
                                open_blockers: parse_vec("open_blockers"),
                                next_step_hint: args
                                    .get("next_step_hint")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string(),
                                token_budget: Some(300),
                            },
                        )?;
                        Ok(serde_json::to_string_pretty(&cp)?)
                    }
                    "handoff_resume" => {
                        let by_brain = args
                            .get("from_brain")
                            .and_then(|v| v.as_str())
                            .unwrap_or("mcp-brain");
                        let cid = args.get("checkpoint_id").and_then(|v| v.as_str());
                        let resumed = ContinuityManager::resume_latest(&self.storage, by_brain, cid)?;
                        let snippet = resumed
                            .as_ref()
                            .map(|c| ContinuityManager::render_brief_snippet(c, 300));
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "resumed": resumed.is_some(),
                            "checkpoint": resumed,
                            "brief_snippet": snippet,
                        }))?)
                    }
                    "heal" => {
                        let rep = HomeostasisVitalsEngine::run_l1_autonomic_heal(&self.storage)?;
                        Ok(serde_json::to_string_pretty(&rep)?)
                    }
                    "peers" => {
                        let cfg = crate::config::Config::load_or_default(None).unwrap_or_default();
                        let client = crate::mesh::MeshClient::new(&cfg.mesh, cfg.web.auth_token.clone());
                        let peers = client.query_peer_body_manifests();
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "configured_peers": client.peers(),
                            "results": peers,
                        }))?)
                    }
                    "runtime_mode" => {
                        let mode_arg = args.get("mode").and_then(|v| v.as_str());
                        let primary_arg = args.get("shadow_primary").and_then(|v| v.as_str());
                        let profile_arg = args.get("mcp_profile").and_then(|v| v.as_str());
                        if mode_arg.is_some() || primary_arg.is_some() || profile_arg.is_some() {
                            let cur = crate::config::Config::load_effective_or_default();
                            let next_mode = match mode_arg {
                                Some(m) => crate::config::RuntimeMode::parse_str(m)
                                    .ok_or_else(|| anyhow::anyhow!("未知 mode: {}", m))?,
                                None => crate::config::RuntimeMode::parse_str(&cur.runtime.mode)
                                    .unwrap_or(crate::config::RuntimeMode::BodyOnly),
                            };
                            let next_profile = match profile_arg {
                                Some(p) => Some(
                                    crate::config::McpToolProfile::parse_str(p)
                                        .ok_or_else(|| anyhow::anyhow!("未知 mcp_profile: {}", p))?,
                                ),
                                None => None,
                            };
                            let (cfg_path, updated) = crate::config::Config::update_runtime_mode(
                                None,
                                next_mode,
                                primary_arg,
                                next_profile,
                            )?;
                            let mut payload = crate::body::ShadowAbEngine::build_runtime_status(&updated);
                            if let Some(obj) = payload.as_object_mut() {
                                obj.insert("config_path".to_string(), serde_json::json!(cfg_path.to_string_lossy()));
                            }
                            Ok(serde_json::to_string_pretty(&payload)?)
                        } else {
                            let cur = crate::config::Config::load_effective_or_default();
                            Ok(serde_json::to_string_pretty(
                                &crate::body::ShadowAbEngine::build_runtime_status(&cur),
                            )?)
                        }
                    }
                    "ab_report" => {
                        let cur = crate::config::Config::load_effective_or_default();
                        let window = args.get("window").and_then(|v| v.as_str()).unwrap_or("7d");
                        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(200) as usize;
                        let rep = crate::body::ShadowAbEngine::build_ab_report(
                            &self.storage,
                            &cur,
                            window,
                            limit,
                        )?;
                        Ok(serde_json::to_string_pretty(&rep)?)
                    }
                    "ab_compare" => {
                        let query = args
                            .get("query")
                            .and_then(|v| v.as_str())
                            .map(str::trim)
                            .filter(|s| !s.is_empty())
                            .ok_or_else(|| anyhow::anyhow!("action=ab_compare 必须提供 query"))?;
                        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                        let cur = crate::config::Config::load_effective_or_default();
                        let (cmp, _, _) = crate::body::ShadowAbEngine::compare_asset_search(
                            &self.storage,
                            &cur,
                            query,
                            limit,
                            "mcp-ab",
                        )?;
                        Ok(serde_json::to_string_pretty(&cmp)?)
                    }
                    "perf_snapshot" => {
                        let label = args.get("label").and_then(|v| v.as_str());
                        let set_baseline = args
                            .get("set_baseline")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        let snap = crate::body::BodyPerfEngine::capture_snapshot(
                            &self.storage,
                            label,
                            set_baseline,
                        )?;
                        Ok(serde_json::to_string_pretty(&snap)?)
                    }
                    "perf_awr" => {
                        if let Some(rid) = args
                            .get("report_id")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.trim().is_empty())
                        {
                            let rep = crate::body::BodyPerfEngine::load_awr_report(
                                self.storage.provider().as_ref(),
                                rid,
                            )
                            .ok_or_else(|| anyhow::anyhow!("未找到历史 AWR 报告: {}", rid))?;
                            return Ok(serde_json::to_string_pretty(&rep)?);
                        }
                        let snap_a = args.get("snap_a").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty());
                        let snap_b = args.get("snap_b").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty());
                        let vs_baseline = args
                            .get("vs_baseline")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(true);
                        let rep = crate::body::BodyPerfEngine::generate_awr_report(
                            &self.storage,
                            snap_a,
                            snap_b,
                            vs_baseline,
                        )?;
                        Ok(serde_json::to_string_pretty(&rep)?)
                    }
                    "perf_snapshots" => {
                        let (snapshots, baseline) =
                            crate::body::BodyPerfEngine::list_snapshots_meta(self.storage.provider().as_ref());
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "baseline": baseline,
                            "snapshots": snapshots,
                            "count": snapshots.len(),
                        }))?)
                    }
                    "perf_reports" => {
                        let reports =
                            crate::body::BodyPerfEngine::list_awr_reports_meta(self.storage.provider().as_ref());
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "reports": reports,
                            "count": reports.len(),
                        }))?)
                    }
                    "perf_delete" => {
                        let kind = args
                            .get("target_kind")
                            .or_else(|| args.get("kind"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("auto");
                        let target_id = args
                            .get("target_id")
                            .or_else(|| args.get("id"))
                            .or_else(|| args.get("report_id"))
                            .or_else(|| args.get("snapshot_id"))
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| anyhow::anyhow!("perf_delete 需要提供 target_id (或 report_id / snapshot_id)"))?;
                        let res = crate::body::BodyPerfEngine::delete_perf_artifact(
                            &self.storage,
                            kind,
                            target_id,
                        )?;
                        Ok(serde_json::to_string_pretty(&res)?)
                    }
                    "perf_tune" => {
                        let rep = crate::body::BodyPerfEngine::run_l1_auto_tune(&self.storage)?;
                        Ok(serde_json::to_string_pretty(&rep)?)
                    }
                    _ => bail!("未知 body_state action: {}", action),
                }
            }

            _ => bail!("Unknown tool: {}", name),
        }
    }
}
