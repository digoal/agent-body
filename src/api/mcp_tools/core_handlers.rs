use std::path::PathBuf;
use serde_json::Value;
use anyhow::{bail, Result};
use crate::storage::models::AssetRecord;
use crate::placement::{PlacementEngine, PlacementRequest};
use crate::scanner::Scanner;
use crate::env::EnvCollector;
use crate::memory::MemoryManager;
use crate::links::LinksEngine;
use crate::changes::ChangesEngine;
use crate::brief::{BriefAssembler, BriefRequest};
use std::sync::Arc;


use super::McpToolRegistry;

impl McpToolRegistry {
    pub(super) fn call_core_tool(&self, name: &str, args: &Value) -> Option<Result<String>> {
        if !matches!(name, "body_brief" | "body_placement_suggest" | "body_placement_rules" | "body_asset_search" | "body_asset_register" | "body_asset_scan" | "body_env_processes" | "body_env_show" | "body_memory_add" | "body_memory_search" | "body_links_query" | "body_links_visualize" | "body_changes_since" | "body_port_check" | "body_links_impact" | "body_links_audit" | "body_codegraph_bridge") {
            return None;
        }
        Some(self.call_core_tool_inner(name, args))
    }

    fn call_core_tool_inner(&self, name: &str, args: &Value) -> Result<String> {
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
                search_opts.asset_type = args.get("asset_type").and_then(|v| v.as_str()).map(str::to_string);
                search_opts.domain = args.get("domain").and_then(|v| v.as_str()).map(str::to_string);
                search_opts.kv_key = args.get("kv_key").and_then(|v| v.as_str()).map(str::to_string);
                search_opts.kv_value = args.get("kv_value").and_then(|v| v.as_str()).map(str::to_string);
                search_opts.kv_filter = args.get("kv_filter").and_then(|v| v.as_str()).map(str::to_string);
                if let Some(hops) = args.get("graph_hops").and_then(|v| v.as_u64()) {
                    search_opts.graph_hops = (hops as usize).min(2);
                }
                if let Some(fz) = args.get("fuzzy").and_then(|v| v.as_bool()) {
                    search_opts.fuzzy = fz;
                }
                if let Some(lk) = args.get("include_links").and_then(|v| v.as_bool()) {
                    search_opts.include_links = lk;
                }
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
                let mine_kg = args.get("mine_knowledge").and_then(|v| v.as_bool()).unwrap_or(false);
                let extract_kh = args.get("extract_knowhow").and_then(|v| v.as_bool()).unwrap_or(false);
                let backfill_mm = args.get("backfill_multimodal").and_then(|v| v.as_bool()).unwrap_or(false);
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
                if mine_kg {
                    let cfg = crate::config::Config::load_effective_or_default();
                    let kg = crate::fact::KnowledgeGraphBuilder::mine_from_storage(&self.storage, &cfg.knowledge_graph)?;
                    out.push_str(&format!("  - L1 Know-How 建网: 文档={} 章节={} 概念={} PMI边={}\n", kg.docs_indexed, kg.sections_created, kg.terms_created, kg.pmi_edges));
                }
                if extract_kh {
                    let cfg = crate::config::Config::load_effective_or_default();
                    let kh = crate::fact::KnowHowExtractor::extract_from_storage(&self.storage, &cfg.annotation, &cfg.knowledge_graph, enrich_limit.clamp(1, 20), Some(path_str))?;
                    out.push_str(&format!("  - L2 Know-How 抽取: model={} 章节={} 原文锚定={}/{} 幻觉拦截={} 新增边={}\n", kh.model_used, kh.sections_scanned, kh.triples_grounded, kh.triples_extracted, kh.hallucinations_rejected, kh.edges_created));
                }
                if backfill_mm {
                    let bf = crate::scanner::backfill_multimodal_assets(&self.storage, 200, Some(path_str))?;
                    out.push_str(&format!("  - 多模态增量回填: scanned={} upgraded={} skipped={}\n", bf.scanned, bf.upgraded, bf.skipped_existing));
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

            _ => bail!("Unknown tool: {}", name),
        }
    }
}
