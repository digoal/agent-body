pub mod capability;
pub mod process_net;
pub mod automation;
pub mod assets_catalog;
pub mod property_graph;
pub(crate) mod property_graph_helpers;
pub mod semantic_discovery;
pub mod knowledge_graph;
pub mod knowledge_terms;
pub mod knowhow_extractor;

pub use capability::{CapabilityEngine, ToolCapability, CapabilityQueryResult};
pub use process_net::{ProcessNetTopology, ListeningPortFact, PortQueryFactResult};
pub use automation::{AutomationTopology, SystemdServiceFact, CronJobFact, FileImpactReport};
pub use assets_catalog::{AssetsCatalogEngine, RepoFact, DocFact, CatalogOverview};
pub use property_graph::PropertyGraphOrganizer;
pub use semantic_discovery::{GraphRelationshipMiner, MiningReport};
pub use knowledge_graph::{
    expand_graph_for_query, tokenize_terms, AliasCanonicalizer, GraphExpansionResult,
    HeadingChunk, HeadingChunker, KnowledgeGraphBuilder, KnowledgeMiningReport,
};
pub use knowhow_extractor::{
    verify_grounding_quote, KnowHowExtractionReport, KnowHowExtractor, KnowHowTriple,
};

use anyhow::Result;

/// 宿主机客观事实图谱统一查询门面 (Zero-Hallucination Fact Facade)
pub struct FactFacade;

impl FactFacade {
    /// 统一入口：根据类型与自然语言/关键词查询宿主机物理事实
    pub fn query_facts(fact_type: &str, query: &str) -> Result<String> {
        let trimmed_query = query.trim();
        let normalized_type = fact_type.trim().to_lowercase();

        // 智能自动分流 (auto)
        let resolved_type = if normalized_type.is_empty() || normalized_type == "auto" {
            Self::infer_fact_type(trimmed_query)
        } else {
            normalized_type
        };

        match resolved_type.as_str() {
            "capability" | "tool" => {
                let mut engine = CapabilityEngine::new();
                engine.scan_host();
                let res = engine.query(trimmed_query);
                Ok(engine.format_for_agent(&res))
            }
            "port" => {
                // 如果是纯数字，按端口诊断
                if let Ok(port) = trimmed_query.parse::<u16>() {
                    let res = ProcessNetTopology::check_port(port, 3)?;
                    let mut out = format!("🔌 **端口 `{}` 物理占用事实反查**\n\n", port);
                    out.push_str(&format!("- **状态**: `{}`\n", res.status));
                    if let Some(fact) = res.occupied_by {
                        out.push_str(&format!("- **占用进程**: `{}` (PID: {})\n", fact.process_name.as_deref().unwrap_or("未知"), fact.pid.unwrap_or(0)));
                        out.push_str(&format!("- **启动命令**: `{}`\n", fact.command.as_deref().unwrap_or("未知")));
                        out.push_str(&format!("- **工作目录**: `{}`\n", fact.cwd.as_deref().unwrap_or("未知")));
                        out.push_str(&format!("- **所属用户**: `{}`\n", fact.user.as_deref().unwrap_or("未知")));
                    }
                    out.push_str(&format!("- **避坑建议**: {}\n", res.advice));
                    Ok(out)
                } else {
                    // 查询全量监听端口并输出表格
                    let ports = ProcessNetTopology::scan_listening_ports()?;
                    Ok(ProcessNetTopology::format_listening_report(&ports, true))
                }
            }
            "automation" => {
                let services = AutomationTopology::scan_systemd_services()?;
                let cron_jobs = AutomationTopology::scan_cron_jobs()?;
                Ok(AutomationTopology::format_automation_overview(&services, &cron_jobs))
            }
            "impact" => {
                let report = AutomationTopology::find_impact_on_file(trimmed_query)?;
                let mut out = format!("💥 **文件 `{}` 逆向依赖与爆炸半径反查**\n\n", report.target_file);
                out.push_str(&format!("- **结论**: {}\n\n", report.advice));
                if !report.impacted_services.is_empty() {
                    out.push_str("#### 受影响的 Systemd 守护服务:\n");
                    for s in report.impacted_services {
                        out.push_str(&format!("* `{}` (启动命令: `{}`)\n", s.service_name, s.exec_start));
                    }
                    out.push('\n');
                }
                if !report.impacted_cron_jobs.is_empty() {
                    out.push_str("#### 受影响的 Cron 定时调度:\n");
                    for c in report.impacted_cron_jobs {
                        out.push_str(&format!("* [{}] 用户: `{}` 命令: `{}`\n", c.schedule, c.user, c.command));
                    }
                }
                Ok(out)
            }
            "repo" | "code" => {
                let repos = if trimmed_query.is_empty() || trimmed_query == "all" {
                    AssetsCatalogEngine::scan_repos(None)?
                } else {
                    AssetsCatalogEngine::query_repos(trimmed_query, None)?
                };
                let docs = Vec::new();
                Ok(AssetsCatalogEngine::format_catalog_overview(&repos, &docs))
            }
            "doc" | "document" => {
                let docs = if trimmed_query.is_empty() || trimmed_query == "all" {
                    AssetsCatalogEngine::scan_documents(None)?
                } else {
                    AssetsCatalogEngine::query_documents(trimmed_query, None)?
                };
                let repos = Vec::new();
                Ok(AssetsCatalogEngine::format_catalog_overview(&repos, &docs))
            }
            "graph" | "topology" | "property_graph" => {
                let cfg = crate::config::Config::load_or_default(None).unwrap_or_else(|_| crate::config::Config::default());
                let storage = crate::storage::Storage::open(&cfg.db_path)?;
                let organizer = PropertyGraphOrganizer::new(storage.clone());
                
                // 自动增量感知并刷新当前宿主机实体图
                if let Err(e) = organizer.build_host_graph() {
                    eprintln!("[Fact][WARN] 自动增量刷新属性图失败: {}", e);
                }

                let sub = organizer.query_subgraph(trimmed_query, 2)?;
                Ok(organizer.format_graph_report(&sub))
            }
            "knowledge" | "knowhow" | "mine-knowledge" | "extract-knowhow" | "extract_knowhow" => {
                let cfg = crate::config::Config::load_effective_or_default();
                let storage = crate::storage::Storage::open(&cfg.db_path)?;
                if resolved_type == "extract-knowhow" || resolved_type == "extract_knowhow" {
                    let rep = KnowHowExtractor::extract_from_storage(
                        &storage,
                        &cfg.annotation,
                        &cfg.knowledge_graph,
                        10,
                        None,
                    )?;
                    return Ok(format!(
                        "🧠 **L2 Know-How 因果与边界抽取完成** (model={}, ollama_online={})\n- 扫描章节: {} | 抽取三元组: {} | 原文锚定通过: {} | 幻觉拦截: {} | 新增 Know-How 节点: {} | 新增边: {} | 耗时: {}ms\n",
                        rep.model_used,
                        rep.ollama_available,
                        rep.sections_scanned,
                        rep.triples_extracted,
                        rep.triples_grounded,
                        rep.hallucinations_rejected,
                        rep.knowhow_nodes_created,
                        rep.edges_created,
                        rep.elapsed_ms
                    ));
                }
                if resolved_type == "mine-knowledge" || trimmed_query == "mine" {
                    let rep = KnowledgeGraphBuilder::mine_from_storage(&storage, &cfg.knowledge_graph)?;
                    return Ok(format!(
                        "🧠 **行业 Know-How 知识网络构建完成**\n- 文档: {} | 章节切片: {} | 记忆节点: {} | 术语概念: {} | 层级边: {} | 引用边: {} | PMI 共现边: {}\n",
                        rep.docs_indexed,
                        rep.sections_created,
                        rep.memories_indexed,
                        rep.terms_created,
                        rep.hierarchy_edges,
                        rep.mention_edges,
                        rep.pmi_edges
                    ));
                }
                let aliases = AliasCanonicalizer::new(&cfg.knowledge_graph.aliases);
                let exp = expand_graph_for_query(
                    storage.provider().as_ref(),
                    trimmed_query,
                    cfg.knowledge_graph.default_search_hops.max(1),
                    &aliases,
                );
                let mut docs: Vec<(&String, &f64)> = exp.doc_scores.iter().collect();
                docs.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));
                let mut out = format!("🧠 **知识网络扩散召回 (query='{}', 命中 {} 篇)**\n\n", trimmed_query, docs.len());
                for (path, score) in docs.into_iter().take(10) {
                    out.push_str(&format!("- `{}` (graph_score={:.3})\n", path, score));
                    if let Some(secs) = exp.matched_sections.get(path) {
                        for s in secs {
                            out.push_str(&format!("  - 关联片段: {}\n", s));
                        }
                    }
                    if let Some(paths) = exp.reasoning_paths.get(path) {
                        for rp in paths {
                            out.push_str(&format!("  - 推理路径: `{}`\n", rp));
                        }
                    }
                }
                Ok(out)
            }
            "all" | "overview" => {
                let mut out = String::new();
                out.push_str("# 🌐 宿主机客观事实图谱全景认知 (Host Reality Graph)\n\n");
                
                // 1. 端口与进程
                let ports = ProcessNetTopology::scan_listening_ports()?;
                out.push_str(&ProcessNetTopology::format_listening_report(&ports, true));
                out.push_str("\n---\n\n");

                // 2. 自动化拓扑
                let services = AutomationTopology::scan_systemd_services()?;
                let cron_jobs = AutomationTopology::scan_cron_jobs()?;
                out.push_str(&AutomationTopology::format_automation_overview(&services, &cron_jobs));
                out.push_str("\n---\n\n");

                // 3. 源码与资料
                let repos = AssetsCatalogEngine::scan_repos(None)?;
                let docs = AssetsCatalogEngine::scan_documents(None)?;
                out.push_str(&AssetsCatalogEngine::format_catalog_overview(&repos, &docs));

                Ok(out)
            }
            _ => {
                // 默认兜底尝试能力匹配
                let mut engine = CapabilityEngine::new();
                engine.scan_host();
                let res = engine.query(trimmed_query);
                Ok(engine.format_for_agent(&res))
            }
        }
    }

    /// 根据用户输入的内容智能推测事实查询类型
    fn infer_fact_type(query: &str) -> String {
        let q = query.trim();
        if q.contains("依赖图") || q.contains("关系图") || q.contains("拓扑图") || q.contains("架构图") || q.contains("拓扑") || q.contains("图谱") || q.contains("graph") || q.contains("topology") {
            "graph".to_string()
        } else if q.parse::<u16>().is_ok() || q.contains("端口") || q.contains("port") {
            "port".to_string()
        } else if q.contains("systemd") || q.contains("cron") || q.contains("定时任务") || q.contains("服务") {
            "automation".to_string()
        } else if q.starts_with('/') && (q.ends_with(".conf") || q.ends_with(".toml") || q.ends_with(".sh") || q.ends_with(".service")) {
            "impact".to_string()
        } else if q.contains("源码") || q.contains("仓库") || q.contains("git") || q.contains("repo") {
            "repo".to_string()
        } else if q.contains("资料") || q.contains("笔记") || q.contains("权威") || q.contains("文档") {
            "doc".to_string()
        } else if q == "all" || q == "overview" || q == "全景" {
            "all".to_string()
        } else {
            "capability".to_string()
        }
    }

    /// 查询工具能力列表（供情境组装器使用）
    pub fn query_tools(query: &str) -> Result<Vec<ToolCapability>> {
        let mut engine = CapabilityEngine::new();
        engine.scan_host();
        let res = engine.query(query);
        Ok(res.all_matching_tools)
    }

    /// 查询活跃监听端口列表（供情境组装器使用）
    pub fn query_ports() -> Result<Vec<ListeningPortFact>> {
        ProcessNetTopology::scan_listening_ports()
    }

    /// 查询相关仓库列表（供情境组装器使用）
    pub fn query_repos(query: &str) -> Result<Vec<RepoFact>> {
        if query.trim().is_empty() {
            let all_repos = AssetsCatalogEngine::scan_repos(None)?;
            Ok(all_repos.into_iter().take(5).collect())
        } else {
            AssetsCatalogEngine::query_repos(query, None)
        }
    }
}
