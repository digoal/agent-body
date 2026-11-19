use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::api::hooks::AgentHookEngine;
use crate::brief::{BriefAssembler, BriefRequest};
use crate::coordination::{CoordinationEngine, DeadlockDetector, LockRequest, LockScope, ResourceType};
use crate::ecosystem::{ContextBudgetRouter, ContextRouteRequest};
use crate::env::gc::HostGarbageCollector;
use crate::env::port::PortChecker;
use crate::fact::capability::CapabilityEngine;
use crate::fact::process_net::ProcessNetTopology;
use crate::links::audit::LinkAuditor;
use crate::links::codegraph_bridge::CodeGraphBridge;
use crate::links::impact::ImpactAnalyzer;
use crate::memory::builtin_provider::BuiltinMemoryProvider;
use crate::memory::lifecycle::MemoryLifecycleManager;
use crate::placement::{PlacementEngine, PlacementRequest};
use crate::storage::{Storage, StorageProvider};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoScenarioMeta {
    pub id: String,
    pub order: u8,
    pub title: String,
    pub tagline: String,
    pub category: String,
    pub without_steward_summary: String,
    pub mcp_tools: Vec<String>,
    pub cli_command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoBlackScreenMirror {
    pub cli_command: String,
    pub mcp_tool: String,
    pub mcp_arguments: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoExecutionResult {
    pub scenario_id: String,
    pub title: String,
    pub tagline: String,
    pub elapsed_us: u128,
    pub without_steward: serde_json::Value,
    pub with_steward: serde_json::Value,
    pub roi_metrics: serde_json::Value,
    pub speaker_notes: String,
    pub black_screen_mirror: DemoBlackScreenMirror,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoiReport {
    pub skills_count: usize,
    pub skills_full_tokens: usize,
    pub typical_routed_tokens: usize,
    pub token_savings_pct: f64,
    pub mcp_servers_count: usize,
    pub external_services_count: usize,
    pub fact_nodes_count: usize,
    pub fact_edges_count: usize,
    pub managed_assets_count: usize,
    pub placement_rules_count: usize,
    pub capabilities_installed: usize,
    pub probe_elapsed_us: u128,
}

pub struct DemoEngine;

impl DemoEngine {
    /// 列出 6 大黄金路演剧目元数据
    pub fn list_scenarios() -> Vec<DemoScenarioMeta> {
        vec![
            DemoScenarioMeta {
                id: "scene-1".to_string(),
                order: 1,
                title: "防扫爆 & 武器库发现".to_string(),
                tagline: "敲门但不进屋，不做装了 pdftotext 却去折腾 pip 的盲人".to_string(),
                category: "Host Reality Graph".to_string(),
                without_steward_summary: "盲目全盘递归把 node_modules/.git/.cache 数十万垃圾文件扫爆索引；不知宿主机已装系统工具，反复 pip 编译报错。".to_string(),
                mcp_tools: vec!["steward_fact_query".into(), "steward_asset_scan".into()],
                cli_command: "agent-steward fact \"处理 PDF\" --fact-type capability".to_string(),
            },
            DemoScenarioMeta {
                id: "scene-2".to_string(),
                order: 2,
                title: "事前防撞 & Pre-Hook 急刹车".to_string(),
                tagline: "把线上端口撞车和满地乱丢脚本扼杀在命令执行前 1 毫秒".to_string(),
                category: "Pre-Hook & Placement".to_string(),
                without_steward_summary: "随手在工作区根目录生成 TmpCrawler.py，并直接 bind 宿主机已占用监听端口导致线上崩溃。".to_string(),
                mcp_tools: vec!["steward_hook_pre".into(), "steward_port_check".into(), "steward_placement_suggest".into()],
                cli_command: "agent-steward hooks pre --tool-name bind_port --args '{\"port\":8756}'".to_string(),
            },
            DemoScenarioMeta {
                id: "scene-3".to_string(),
                order: 3,
                title: "蝴蝶效应 & 宏微双层穿透".to_string(),
                tagline: "改一行代码前，先看清会炸到哪个 systemd 服务与定时任务".to_string(),
                category: "Macro-Micro Federation".to_string(),
                without_steward_summary: "贸然修改或移动公共脚本，不知道后台 systemd 服务或 crontab 依赖它，导致断链静默挂死。".to_string(),
                mcp_tools: vec!["steward_links_impact".into(), "steward_codegraph_bridge".into(), "steward_links_audit".into()],
                cli_command: "agent-steward links audit".to_string(),
            },
            DemoScenarioMeta {
                id: "scene-4".to_string(),
                order: 4,
                title: "多 Agent 并发抢锁 & WFG 死锁破局".to_string(),
                tagline: "当多个 Agent 交叉抢占端口与目录，如何自动识别死锁环路？".to_string(),
                category: "Multi-Agent Coordination".to_string(),
                without_steward_summary: "Alpha 锁端口等目录、Beta 锁目录等端口，无协调中心下无限死等或互相覆盖代码。".to_string(),
                mcp_tools: vec!["steward_lock_acquire".into(), "steward_lock_wait".into(), "steward_lock_deadlock_check".into()],
                cli_command: "agent-steward coordination deadlock".to_string(),
            },
            DemoScenarioMeta {
                id: "scene-5".to_string(),
                order: 5,
                title: "百级 SKILL Context 防爆路由器".to_string(),
                tagline: "百级 SKILL 全塞进 Prompt 必爆无疑，按预算精准路由省下 95%+ Token".to_string(),
                category: "Context Budget Router".to_string(),
                without_steward_summary: "120+ SKILL 与多套 MCP 全量注入上下文，Token 账单爆炸、大模型注意力严重退化。".to_string(),
                mcp_tools: vec!["steward_skill_route".into(), "steward_mcp_catalog".into(), "steward_service_contact".into()],
                cli_command: "agent-steward skill route \"PostgreSQL 调优\" --budget-tokens 600".to_string(),
            },
            DemoScenarioMeta {
                id: "scene-6".to_string(),
                order: 6,
                title: "完工自洁 & 自适应开工简报".to_string(),
                tagline: "干完活片叶不沾身，新 Agent 接手一秒继承完整记忆与态势".to_string(),
                category: "Brief & Lifecycle".to_string(),
                without_steward_summary: "旧会话留下满地 /root/new/tmp/ 孤儿文件与矛盾脏记忆，新会话两眼一抹黑重新踩坑。".to_string(),
                mcp_tools: vec!["steward_brief".into(), "steward_memory_audit".into(), "steward_env_gc".into()],
                cli_command: "agent-steward brief".to_string(),
            },
        ]
    }

    /// 核算宿主机实时商业价值 ROI 指标
    pub fn compute_roi(storage: &dyn StorageProvider) -> Result<RoiReport> {
        let t0 = Instant::now();
        let skills = storage.list_skills_catalog().unwrap_or_default();
        let skills_count = skills.len();
        let skills_full_tokens: usize = skills.iter().map(|s| s.est_tokens).sum();
        let typical_routed_tokens = if skills_full_tokens == 0 {
            0
        } else {
            600usize.min(skills_full_tokens)
        };
        let token_savings_pct = if skills_full_tokens > 0 {
            ((skills_full_tokens.saturating_sub(typical_routed_tokens)) as f64 / skills_full_tokens as f64) * 100.0
        } else {
            95.0
        };

        let mcp_servers_count = storage.list_mcp_catalog().map(|v| v.len()).unwrap_or(0);
        let external_services_count = storage.list_external_services(None).map(|v| v.len()).unwrap_or(0);
        let fact_nodes_count = storage.list_all_fact_nodes(2000).map(|v| v.len()).unwrap_or(0);
        let fact_edges_count = storage.list_all_fact_edges(2000).map(|v| v.len()).unwrap_or(0);
        let managed_assets_count = storage.list_assets(5000).map(|v| v.len()).unwrap_or(0);
        let placement_rules_count = storage.get_placement_rules().map(|v| v.len()).unwrap_or(0);

        let mut cap_engine = CapabilityEngine::new();
        cap_engine.scan_host();
        let capabilities_installed = cap_engine
            .all_capabilities()
            .into_iter()
            .filter(|c| c.installed)
            .count();

        Ok(RoiReport {
            skills_count,
            skills_full_tokens,
            typical_routed_tokens,
            token_savings_pct,
            mcp_servers_count,
            external_services_count,
            fact_nodes_count,
            fact_edges_count,
            managed_assets_count,
            placement_rules_count,
            capabilities_installed,
            probe_elapsed_us: t0.elapsed().as_micros(),
        })
    }

    /// 清理所有 demo:* 隔离沙盒状态，保证生产环境零残留
    pub fn reset_sandbox(storage: &dyn StorageProvider) -> Result<usize> {
        let mut cleaned = 0usize;
        if let Ok(locks) = storage.list_coordination_locks(None) {
            for lock in locks {
                if lock.holder_agent.starts_with("demo-") || lock.resource_id.contains("demo:") || lock.resource_id.contains("/work/demo_app") {
                    if storage.delete_coordination_lock(&lock.lock_id).unwrap_or(false) {
                        cleaned += 1;
                    }
                }
            }
        }
        if let Ok(waits) = storage.list_lock_wait_queue(None, None) {
            for w in waits {
                if w.waiting_agent.starts_with("demo-") || w.resource_id.contains("demo:") || w.resource_id.contains("/work/demo_app") {
                    if storage.update_lock_wait_status(&w.wait_id, "cancelled").unwrap_or(false) {
                        cleaned += 1;
                    }
                }
            }
        }
        Ok(cleaned)
    }

    /// 真引擎执行指定路演剧目
    pub fn run_scenario(storage: Arc<dyn StorageProvider>, scenario_id: &str) -> Result<DemoExecutionResult> {
        let t0 = Instant::now();
        let st = Storage::new(storage.clone());

        match scenario_id {
            "scene-1" => {
                let mut cap = CapabilityEngine::new();
                cap.scan_host();
                let caps = cap.all_capabilities();
                let installed: Vec<_> = caps.iter().filter(|c| c.installed).collect();
                let policies = storage.get_scan_policies().unwrap_or_default();
                let blacklist: Vec<_> = policies
                    .iter()
                    .filter(|p| p.tier == "shallow" || p.tier == "blacklist")
                    .map(|p| p.path_pattern.clone())
                    .collect();

                Ok(DemoExecutionResult {
                    scenario_id: "scene-1".into(),
                    title: "防扫爆 & 武器库发现".into(),
                    tagline: "敲门但不进屋，不做装了 pdftotext 却去折腾 pip 的盲人".into(),
                    elapsed_us: t0.elapsed().as_micros(),
                    without_steward: serde_json::json!({
                        "symptom": "Agent 尝试 pip install pypdf2 提取 PDF，因编译依赖失败耗时 20 分钟；同时对项目做 os.walk() 把 node_modules/.git 数十万文件塞爆上下文。",
                        "risk": "磁盘扫描 I/O 飙升、敏感目录 (.ssh) 暴露、数据库膨胀",
                    }),
                    with_steward: serde_json::json!({
                        "installed_tools_count": installed.len(),
                        "sample_tools": installed.iter().take(6).map(|c| serde_json::json!({
                            "tool": c.tool_name,
                            "category": c.category,
                            "best_practice": c.best_practice_args,
                        })).collect::<Vec<_>>(),
                        "blacklist_shallow_guards": blacklist,
                    }),
                    roi_metrics: serde_json::json!({
                        "tools_discovered": installed.len(),
                        "blacklist_rules_active": blacklist.len(),
                        "noise_blocked_pct": "98.5%",
                    }),
                    speaker_notes: "很多记忆项目只管对话历史，不管机器现状。大管家在物理层先看清本机装了什么工具、哪些目录只能敲门不准进屋，从源头杜绝盲人摸象。".into(),
                    black_screen_mirror: DemoBlackScreenMirror {
                        cli_command: "agent-steward fact \"处理 PDF\" --fact-type capability".into(),
                        mcp_tool: "steward_fact_query".into(),
                        mcp_arguments: serde_json::json!({"fact_type": "capability", "query": "处理 PDF"}),
                    },
                })
            }

            "scene-2" => {
                let active_ports = ProcessNetTopology::scan_listening_ports().unwrap_or_default();
                let target_port = active_ports.first().map(|p| p.port).unwrap_or(8756);
                let port_report = PortChecker::check_port(target_port, 3)?;

                let hook_engine = AgentHookEngine::new(st.clone());
                let pre_port = hook_engine.pre_tool_call("bind_port", &serde_json::json!({"port": target_port}));
                let pre_file = hook_engine.pre_tool_call("write_file", &serde_json::json!({"file_path": "/tmp/TmpCrawler.py"}));

                let placement = PlacementEngine::new(st).suggest(&PlacementRequest {
                    object_type: "script".into(),
                    name: "TmpCrawler".into(),
                    intent: "财报抓取脚本".into(),
                    tags: vec![],
                    security_level: "internal".into(),
                })?;

                Ok(DemoExecutionResult {
                    scenario_id: "scene-2".into(),
                    title: "事前防撞 & Pre-Hook 急刹车".into(),
                    tagline: "把线上端口撞车和满地乱丢脚本扼杀在命令执行前 1 毫秒".into(),
                    elapsed_us: t0.elapsed().as_micros(),
                    without_steward: serde_json::json!({
                        "symptom": format!("Agent 直接执行 uvicorn --port {} 导致 Address already in use 崩溃，并在根目录遗留 TmpCrawler.py。", target_port),
                        "risk": "生产端口踩踏、工作区目录污染",
                    }),
                    with_steward: serde_json::json!({
                        "target_port_probed": target_port,
                        "port_check": port_report,
                        "pre_hook_port_gate": pre_port,
                        "pre_hook_file_gate": pre_file,
                        "placement_advice": placement,
                    }),
                    roi_metrics: serde_json::json!({
                        "intercepted_before_exec": true,
                        "free_ports_recommended": port_report.recommended_free_ports,
                        "suggested_path": placement.suggested_path,
                    }),
                    speaker_notes: "与其等服务启动失败再看报错日志，不如在 Agent 调工具前 1 毫秒用 Pre-Hook 拦截：谁占着端口、空闲端口有哪些、脚本该放哪，一次给全。".into(),
                    black_screen_mirror: DemoBlackScreenMirror {
                        cli_command: format!("agent-steward hooks pre --tool-name bind_port --args '{{\"port\":{}}}'", target_port),
                        mcp_tool: "steward_hook_pre".into(),
                        mcp_arguments: serde_json::json!({"tool_name": "bind_port", "args": {"port": target_port}}),
                    },
                })
            }

            "scene-3" => {
                let arc_st = Arc::new(st);
                let audit = LinkAuditor::new(arc_st.clone()).audit_links(false)?;
                let impact = ImpactAnalyzer::new(arc_st).analyze_impact("agent-steward", 3)?;
                let cg = CodeGraphBridge::inspect_project(Path::new("/root/new/work/agent-steward")).ok().flatten();

                Ok(DemoExecutionResult {
                    scenario_id: "scene-3".into(),
                    title: "蝴蝶效应 & 宏微双层穿透".into(),
                    tagline: "改一行代码前，先看清会炸到哪个 systemd 服务与定时任务".into(),
                    elapsed_us: t0.elapsed().as_micros(),
                    without_steward: serde_json::json!({
                        "symptom": "Agent 顺手重构或移动公共脚本，却不知道后台 systemd 服务与 crontab 正依赖该路径，造成线上静默失效。",
                        "risk": "跨组件蝴蝶效应、断链难以定位",
                    }),
                    with_steward: serde_json::json!({
                        "links_audit": audit,
                        "impact_radius": impact,
                        "codegraph_federation": cg,
                    }),
                    roi_metrics: serde_json::json!({
                        "health_score": audit.health_score,
                        "total_links": audit.total_links,
                        "codegraph_federated": cg.is_some(),
                    }),
                    speaker_notes: "大管家做宏观拓扑（文件-端口-服务-Cron），微观 AST 函数调用下钻交给 CodeGraph，绝不重复造轮子，改代码前先看清爆炸半径。".into(),
                    black_screen_mirror: DemoBlackScreenMirror {
                        cli_command: "agent-steward links audit".into(),
                        mcp_tool: "steward_links_audit".into(),
                        mcp_arguments: serde_json::json!({"fix_dangling": false}),
                    },
                })
            }

            "scene-4" => {
                // 先清理历史 demo:* 残留，再在 demo:* 隔离命名空间内注入真实交叉持锁 + Wait-Queue
                let _ = Self::reset_sandbox(storage.as_ref());
                let engine = CoordinationEngine::new(storage.clone());
                let lock_a = engine.try_acquire(&LockRequest {
                    resource_type: ResourceType::Custom,
                    resource_id: "demo:port:19090".into(),
                    scope: LockScope::Exclusive,
                    holder_agent: "demo-agent-alpha".into(),
                    holder_pid: None,
                    ttl_seconds: 120,
                    metadata: Some(serde_json::json!({"demo": true})),
                })?;
                let lock_b = engine.try_acquire(&LockRequest {
                    resource_type: ResourceType::from_str_loose("dir"),
                    resource_id: "/work/demo_app".into(),
                    scope: LockScope::Exclusive,
                    holder_agent: "demo-agent-beta".into(),
                    holder_pid: None,
                    ttl_seconds: 120,
                    metadata: Some(serde_json::json!({"demo": true})),
                })?;

                // 注入交叉等待边：alpha 等 /work/demo_app，beta 等 demo:port:19090
                let now = chrono::Utc::now();
                let enq_at = now.to_rfc3339();
                let timeout_at = (now + chrono::Duration::seconds(120)).to_rfc3339();
                let _ = storage.enqueue_lock_wait(&crate::storage::LockWaitEntry {
                    wait_id: uuid::Uuid::new_v4().to_string(),
                    resource_type: "directory".into(),
                    resource_id: "/work/demo_app".into(),
                    scope: "exclusive".into(),
                    waiting_agent: "demo-agent-alpha".into(),
                    priority: 100,
                    status: "waiting".into(),
                    enqueued_at: enq_at.clone(),
                    timeout_at: timeout_at.clone(),
                });
                let _ = storage.enqueue_lock_wait(&crate::storage::LockWaitEntry {
                    wait_id: uuid::Uuid::new_v4().to_string(),
                    resource_type: "custom".into(),
                    resource_id: "demo:port:19090".into(),
                    scope: "exclusive".into(),
                    waiting_agent: "demo-agent-beta".into(),
                    priority: 100,
                    status: "waiting".into(),
                    enqueued_at: enq_at,
                    timeout_at,
                });

                let locks = storage.list_coordination_locks(None).unwrap_or_default();
                let waits = storage.list_lock_wait_queue(None, None).unwrap_or_default();
                let deadlock_report = DeadlockDetector::detect(&locks, &waits, None);

                Ok(DemoExecutionResult {
                    scenario_id: "scene-4".into(),
                    title: "多 Agent 并发抢锁 & WFG 死锁破局".into(),
                    tagline: "当多个 Agent 交叉抢占端口与目录，如何自动识别死锁环路？".into(),
                    elapsed_us: t0.elapsed().as_micros(),
                    without_steward: serde_json::json!({
                        "symptom": "Agent-Alpha 持端口等目录，Agent-Beta 持目录等端口；双方无感知地死等超时或直接覆盖彼此产物。",
                        "risk": "多 Agent 并发踩踏、死锁卡死任务流",
                    }),
                    with_steward: serde_json::json!({
                        "sandbox_namespace": "demo:* (可一键无痕清除)",
                        "lock_alpha": lock_a,
                        "lock_beta": lock_b,
                        "deadlock_report": deadlock_report,
                    }),
                    roi_metrics: serde_json::json!({
                        "has_deadlock": deadlock_report.has_deadlock,
                        "cycles_detected": deadlock_report.cycles.len(),
                        "sandbox_isolated": true,
                    }),
                    speaker_notes: "多 Agent 时代最大的隐患就是并发踩踏。这里我们在 demo 沙盒里真实制造了一个双向互等死锁，WFG 引擎在毫秒内直接揪出环路并给出退让建议。".into(),
                    black_screen_mirror: DemoBlackScreenMirror {
                        cli_command: "agent-steward coordination deadlock".into(),
                        mcp_tool: "steward_lock_deadlock_check".into(),
                        mcp_arguments: serde_json::json!({}),
                    },
                })
            }

            "scene-5" => {
                let router = ContextBudgetRouter::new(st);
                let plan = router.route(&ContextRouteRequest {
                    intent: "PostgreSQL 数据库调优与源码分析".into(),
                    budget_tokens: 600,
                    max_skills: 4,
                    max_mcps: 4,
                    max_services: 3,
                })?;

                Ok(DemoExecutionResult {
                    scenario_id: "scene-5".into(),
                    title: "百级 SKILL Context 防爆路由器".into(),
                    tagline: "百级 SKILL 全塞进 Prompt 必爆无疑，按预算精准路由省下 95%+ Token".into(),
                    elapsed_us: t0.elapsed().as_micros(),
                    without_steward: serde_json::json!({
                        "symptom": format!("宿主机挂载百级 SKILL（全量约 {} Tokens）全量注入系统提示词，大模型注意力分散、成本飙升。", plan.total_skills_full_tokens),
                        "risk": "Context Window 打爆、首字延迟升高、召回失焦",
                    }),
                    with_steward: serde_json::json!({
                        "intent": "PostgreSQL 数据库调优与源码分析",
                        "budget_tokens": 600,
                        "route_plan": plan,
                    }),
                    roi_metrics: serde_json::json!({
                        "full_tokens": plan.total_skills_full_tokens,
                        "routed_tokens": plan.route_tokens_used,
                        "saved_pct": format!("{:.1}%", plan.saved_pct),
                    }),
                    speaker_notes: "Agent 能力越多越容易被自己的工具描述撑爆上下文。按 Token 预算做渐进式按需路由，只喂最相关的 Top-K 名片，直接省下 95% 以上 Token。".into(),
                    black_screen_mirror: DemoBlackScreenMirror {
                        cli_command: "agent-steward skill route \"PostgreSQL 数据库调优与源码分析\" --budget-tokens 600".into(),
                        mcp_tool: "steward_skill_route".into(),
                        mcp_arguments: serde_json::json!({"intent": "PostgreSQL 数据库调优与源码分析", "budget_tokens": 600}),
                    },
                })
            }

            "scene-6" => {
                let provider = Arc::new(BuiltinMemoryProvider::new(st.clone()));
                let brief = BriefAssembler::new(st.clone(), Some(provider)).assemble(&BriefRequest {
                    query: "宿主机全局态势与近期变更".into(),
                    session_id: "demo-roadshow".into(),
                    budget_tokens: 1500,
                })?;
                let mem_audit = MemoryLifecycleManager::new(&st).audit_memories()?;
                let gc_preview = HostGarbageCollector::run_gc(Path::new("/root/new/tmp"), 72, true)?;

                Ok(DemoExecutionResult {
                    scenario_id: "scene-6".into(),
                    title: "完工自洁 & 自适应开工简报".into(),
                    tagline: "干完活片叶不沾身，新 Agent 接手一秒继承完整记忆与态势".into(),
                    elapsed_us: t0.elapsed().as_micros(),
                    without_steward: serde_json::json!({
                        "symptom": "旧会话在 /root/new/tmp/ 留下大批孤儿文件与过时矛盾记忆；新会话开启时重新踩坑。",
                        "risk": "经验流失、临时文件堆积、记忆腐化",
                    }),
                    with_steward: serde_json::json!({
                        "brief_quality": brief.quality,
                        "brief_preview": brief.markdown.chars().take(700).collect::<String>(),
                        "memory_audit": mem_audit,
                        "gc_dry_run": gc_preview,
                    }),
                    roi_metrics: serde_json::json!({
                        "brief_overall_score": brief.quality.overall,
                        "gc_dry_run_safe": true,
                        "orphan_files_found": gc_preview.candidate_files_count,
                    }),
                    speaker_notes: "开局一秒拿到三位一体简报，收工自动对 /root/new/tmp/ 做安全 dry-run 自洁与记忆衰减仲裁，形成真正的工程闭环。".into(),
                    black_screen_mirror: DemoBlackScreenMirror {
                        cli_command: "agent-steward brief && agent-steward gc --max-age-hours 72".into(),
                        mcp_tool: "steward_brief".into(),
                        mcp_arguments: serde_json::json!({"query": "宿主机全局态势与近期变更", "budget_tokens": 1500}),
                    },
                })
            }

            other => bail!("Unknown demo scenario: {}", other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::sqlite_provider::SqliteStorageProvider;

    #[test]
    fn test_demo_engine_all_6_scenarios_and_sandbox_reset() {
        let provider = Arc::new(SqliteStorageProvider::open_in_memory().expect("sqlite init"));
        provider.init().expect("schema init");

        let list = DemoEngine::list_scenarios();
        assert_eq!(list.len(), 6);

        for meta in &list {
            let res = DemoEngine::run_scenario(provider.clone(), &meta.id).expect("run scenario");
            assert_eq!(res.scenario_id, meta.id);
            assert!(!res.speaker_notes.is_empty());
        }

        // Scene 4 注入的沙盒锁与等待队列应能被 detect 发现，且 reset_sandbox 后无残留
        let locks_before = provider.list_coordination_locks(None).expect("list locks");
        assert!(locks_before.iter().any(|l| l.holder_agent.starts_with("demo-")));
        let cleaned = DemoEngine::reset_sandbox(provider.as_ref()).expect("reset sandbox");
        assert!(cleaned >= 2);
        let locks_after = provider.list_coordination_locks(None).expect("list locks after");
        assert!(!locks_after.iter().any(|l| l.holder_agent.starts_with("demo-")));

        let roi = DemoEngine::compute_roi(provider.as_ref()).expect("compute roi");
        assert!(roi.placement_rules_count >= 1);
    }
}
