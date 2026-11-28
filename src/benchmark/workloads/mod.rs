mod advanced;
mod core_workloads;
mod governance_workloads;
mod infra_workloads;
mod knowledge;
use advanced::*;
use core_workloads::*;
use governance_workloads::*;
use infra_workloads::*;
use knowledge::*;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::benchmark::provenance::estimate_tokens;
use crate::fact::process_net::ProcessNetTopology;
use crate::storage::StorageProvider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadSpec {
    pub id: String,
    pub name: String,
    pub title: String,
    pub category: String,
    pub baseline_desc: String,
    pub body_desc: String,
    pub sla_desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadResult {
    pub id: String,
    pub name: String,
    pub title: String,
    pub category: String,
    pub baseline_tokens: i64,
    pub body_tokens: i64,
    pub tokens_saved: i64,
    pub token_savings_pct: f64,
    pub baseline_rounds: i64,
    pub body_rounds: i64,
    pub rounds_saved: i64,
    pub baseline_elapsed_us: u128,
    pub body_elapsed_us: u128,
    pub speedup_factor: f64,
    pub baseline_failures: i64,
    pub hazards_intercepted: i64,
    pub interception_rate_pct: f64,
    pub raw_entries: usize,
    pub kept_entries: usize,
    pub blocked_noise: usize,
    pub noise_reduction_pct: f64,
    pub sla_passed: bool,
    pub baseline_command: String,
    pub body_action: String,
    pub evidence_detail: String,
    #[serde(default)]
    pub quality_score: Option<f64>,
    #[serde(default)]
    pub precision_pct: Option<f64>,
    #[serde(default)]
    pub recall_pct: Option<f64>,
    #[serde(default)]
    pub baseline_quality_score: f64,
    #[serde(default)]
    pub body_quality_score: f64,
    #[serde(default)]
    pub quality_delta: f64,
    #[serde(default)]
    pub baseline_facts_hit: usize,
    #[serde(default)]
    pub body_facts_hit: usize,
    #[serde(default)]
    pub required_facts_total: usize,
    #[serde(default)]
    pub info_density_gain: f64,
}

/// Phase 36 (v3.1): 确定性四维上下文质量评分入参 (R_fact 40% + P_signal 30% + A_action 20% + S_safety 10%)
#[derive(Debug, Clone, Copy)]
pub struct ContextQualityInputs {
    pub required_facts_total: usize,
    pub baseline_facts_hit: usize,
    pub body_facts_hit: usize,
    pub baseline_signal_precision: f64,
    pub body_signal_precision: f64,
    pub baseline_actionability: f64,
    pub body_actionability: f64,
    pub baseline_safety: f64,
    pub body_safety: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct ContextQualityEval {
    pub baseline_quality_score: f64,
    pub body_quality_score: f64,
    pub quality_delta: f64,
    pub body_recall_pct: f64,
    pub body_precision_pct: f64,
    pub info_density_gain: f64,
    pub pareto_passed: bool,
}

/// 确定性四维上下文质量评估器 + 有效信息密度倍数 + 帕累托双门禁判定
pub fn evaluate_context_quality(
    inp: ContextQualityInputs,
    baseline_tokens: i64,
    body_tokens: i64,
    min_quality_sla: f64,
) -> ContextQualityEval {
    let total = inp.required_facts_total.max(1) as f64;
    let r_base = (inp.baseline_facts_hit.min(inp.required_facts_total) as f64) / total;
    let r_body = (inp.body_facts_hit.min(inp.required_facts_total) as f64) / total;
    let p_base = inp.baseline_signal_precision.clamp(0.0, 1.0);
    let p_body = inp.body_signal_precision.clamp(0.0, 1.0);
    let a_base = inp.baseline_actionability.clamp(0.0, 1.0);
    let a_body = inp.body_actionability.clamp(0.0, 1.0);
    let s_base = inp.baseline_safety.clamp(0.0, 1.0);
    let s_body = inp.body_safety.clamp(0.0, 1.0);

    let q_base = ((0.40 * r_base + 0.30 * p_base + 0.20 * a_base + 0.10 * s_base) * 100.0 * 100.0).round() / 100.0;
    let q_body = ((0.40 * r_body + 0.30 * p_body + 0.20 * a_body + 0.10 * s_body) * 100.0 * 100.0).round() / 100.0;
    let delta = ((q_body - q_base) * 100.0).round() / 100.0;

    let rho_base = (inp.baseline_facts_hit as f64 / baseline_tokens.max(1) as f64) * 1000.0;
    let rho_body = (inp.body_facts_hit as f64 / body_tokens.max(1) as f64) * 1000.0;
    let density_gain = ((rho_body / rho_base.max(0.01)) * 100.0).round() / 100.0;

    let pareto_passed = inp.body_facts_hit == inp.required_facts_total
        && q_body >= q_base
        && q_body >= min_quality_sla;

    ContextQualityEval {
        baseline_quality_score: q_base,
        body_quality_score: q_body,
        quality_delta: delta,
        body_recall_pct: (r_body * 100.0 * 100.0).round() / 100.0,
        body_precision_pct: (p_body * 100.0 * 100.0).round() / 100.0,
        info_density_gain: density_gain,
        pareto_passed,
    }
}

pub(super) fn apply_quality_eval(
    res: &mut WorkloadResult,
    inp: ContextQualityInputs,
    min_quality_sla: f64,
) {
    let ev = evaluate_context_quality(inp, res.baseline_tokens, res.body_tokens, min_quality_sla);
    res.baseline_quality_score = ev.baseline_quality_score;
    res.body_quality_score = ev.body_quality_score;
    res.quality_delta = ev.quality_delta;
    res.baseline_facts_hit = inp.baseline_facts_hit;
    res.body_facts_hit = inp.body_facts_hit;
    res.required_facts_total = inp.required_facts_total;
    res.info_density_gain = ev.info_density_gain;
    res.quality_score = Some(ev.body_quality_score);
    res.recall_pct = Some(ev.body_recall_pct);
    res.precision_pct = Some(ev.body_precision_pct);
    res.sla_passed = res.sla_passed && ev.pareto_passed;
}

pub(super) struct BenchTempDir(pub(super) PathBuf);

impl BenchTempDir {
    pub(super) fn create(prefix: &str) -> Result<Self> {
        let dir = std::env::temp_dir().join(format!("{}_{}", prefix, uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir)?;
        Ok(Self(dir))
    }
}

impl Drop for BenchTempDir {
    fn drop(&mut self) {
        if self.0.exists() {
            if let Err(e) = std::fs::remove_dir_all(&self.0) {
                eprintln!("[Bench][WARN] 清理负载临时目录 {:?} 失败: {}", self.0, e);
            }
        }
    }
}

pub fn list_workload_specs() -> Vec<WorkloadSpec> {
    vec![
        WorkloadSpec {
            id: "W1".into(),
            name: "capability_discovery".into(),
            title: "宿主工具能力发现".into(),
            category: "Host Reality Graph".into(),
            baseline_desc: "串行执行 which + --version/--help 外部子进程探查 5 类工具".into(),
            body_desc: "CapabilityEngine 内存能力图谱语义检索".into(),
            sla_desc: "rounds_saved >= 4, body_elapsed_us < 50ms".into(),
        },
        WorkloadSpec {
            id: "W2".into(),
            name: "port_collision_avoidance".into(),
            title: "活跃端口防撞与空闲选址".into(),
            category: "Pre-Hook & PortChecker".into(),
            baseline_desc: "盲目尝试 TcpListener::bind 活跃端口触发 EADDRINUSE 后顺延重试".into(),
            body_desc: "Pre-Hook 1ms 事前拦截 + PortChecker 一次推荐空闲端口".into(),
            sla_desc: "hazards_intercepted >= 1, interception_rate == 100%".into(),
        },
        WorkloadSpec {
            id: "W3".into(),
            name: "granularity_scan_noise_gate".into(),
            title: "分级扫描与黑名单噪音阻断".into(),
            category: "Granularity Tiering".into(),
            baseline_desc: "WalkDir 无过滤递归遍历含 node_modules/.git/__pycache__/.cache 混合树".into(),
            body_desc: "Scanner + PolicyMatcher + NoiseFilter 分级阻断".into(),
            sla_desc: "noise_reduction_pct >= 80%, 黑名单子文件 0 入库".into(),
        },
        WorkloadSpec {
            id: "W4".into(),
            name: "placement_governance".into(),
            title: "8 类资产放置合规门禁".into(),
            category: "Placement Governance".into(),
            baseline_desc: "8 类文件默认裸放根目录或驼峰命名引发规约违规与返工".into(),
            body_desc: "PlacementEngineV2 路径推导 + hook_pre 违规拦截".into(),
            sla_desc: "interception_rate == 100%, body_elapsed_us < 10ms".into(),
        },
        WorkloadSpec {
            id: "W5".into(),
            name: "dependency_impact_and_repair".into(),
            title: "3 跳爆炸半径与断链自愈".into(),
            category: "LinksEngine & Repair".into(),
            baseline_desc: "全目录树逐文件读取文本正则扫描引用链且漏检断链".into(),
            body_desc: "ImpactAnalyzer 3 跳图查询 + LinkAuditor + LinkRepairer".into(),
            sla_desc: "hazards_intercepted >= 2, token_savings_pct >= 50%".into(),
        },
        WorkloadSpec {
            id: "W6".into(),
            name: "multi_agent_deadlock_resolution".into(),
            title: "多 Agent 并发死锁破局".into(),
            category: "Multi-Agent Coordination".into(),
            baseline_desc: "无死锁检测器时双 Agent 交叉等锁陷入超时窗口".into(),
            body_desc: "CoordinationEngine + WFG 环路检测毫秒级裁决".into(),
            sla_desc: "hazards_intercepted >= 1, body_elapsed_us < 10ms".into(),
        },
        WorkloadSpec {
            id: "W7".into(),
            name: "context_budget_routing".into(),
            title: "百级 SKILL 预算路由与渐进披露".into(),
            category: "Context Budget Router".into(),
            baseline_desc: "全量注入宿主机 120+ SKILL 与 MCP 描述进 Prompt".into(),
            body_desc: "ContextBudgetRouter (600 tok 预算) 精准 Top-K 召回".into(),
            sla_desc: "token_savings_pct >= 85%, body_tokens <= 1200".into(),
        },
        WorkloadSpec {
            id: "W8".into(),
            name: "adaptive_brief_assembly".into(),
            title: "冷启动态势简报组装".into(),
            category: "Adaptive Brief".into(),
            baseline_desc: "真机串行执行 6 步冷启动探查命令汇总环境态势".into(),
            body_desc: "BriefAssembler 单次预算受控三维融合简报".into(),
            sla_desc: "rounds_saved == 5, body_elapsed_us < 100ms".into(),
        },
        WorkloadSpec {
            id: "W9".into(),
            name: "hybrid_semantic_recall".into(),
            title: "混合语义检索 Recall@5 准确率".into(),
            category: "D6 Quality & Recall".into(),
            baseline_desc: "纯字面精确匹配在同义词错位场景下漏召回被迫全量扫描".into(),
            body_desc: "属性字典+语义标签混合检索精准召回 Top-K".into(),
            sla_desc: "recall_pct >= 90%, token_savings_pct >= 50%".into(),
        },
        WorkloadSpec {
            id: "W10".into(),
            name: "link_precision_regression".into(),
            title: "复杂模板依赖抽取抗假阳性".into(),
            category: "D6 Quality & Precision".into(),
            baseline_desc: "朴素正则抽取 %s/f-string/占位符/JSON 数字产生大量假断链".into(),
            body_desc: "LinksEngine 模板脱敏抽取实现 >= 95% Precision".into(),
            sla_desc: "precision_pct >= 95% (FPR <= 5%)".into(),
        },
        WorkloadSpec {
            id: "W11".into(),
            name: "security_redaction_and_guardrail".into(),
            title: "全链路脱敏、防命令注入与抗提示词投毒".into(),
            category: "D4/D6 Security Guardrail".into(),
            baseline_desc: "裸采 /proc cmdline、直接 sh -c 拼接与原文摘要注入导致凭据泄露与命令/提示词逃逸".into(),
            body_desc: "SecretRedactor + POSIX 转义 + PromptInjectionScanner 三重微秒级拦截".into(),
            sla_desc: "redaction == 100%, injection_escape == 0, body_elapsed_us < 10_000".into(),
        },
        WorkloadSpec {
            id: "W12".into(),
            name: "freshness_and_staleness_healing".into(),
            title: "资产改删写穿失效与跨表一致性自愈".into(),
            category: "D6 Freshness & Self-Healing".into(),
            baseline_desc: "文件改删后仍沿用旧向量/旧描述/已删FTS/幽灵Fact节点与同路径重复摘要记忆".into(),
            body_desc: "Write-Through 写穿失效 + FreshnessReconciler 跨表一致性自愈闭环".into(),
            sla_desc: "4/4 新鲜度防线命中, token_savings_pct >= 50%, body_elapsed_us < 100_000".into(),
        },
        WorkloadSpec {
            id: "W13".into(),
            name: "semantic_and_weighted_graph_discovery".into(),
            title: "多维证据分层有向加权图谱与隐式语义发现".into(),
            category: "D6 Graph Intelligence".into(),
            baseline_desc: "无结构上卷与向量隐式挖掘时工程节点孤立且物理边/语义边混杂".into(),
            body_desc: "L2 结构上卷 + L3 向量稀疏剪枝 (Top-K=3) + 加权衰减爆炸半径分层过滤".into(),
            sla_desc: "4/4 图谱发现断言命中, token_savings_pct >= 45%, body_elapsed_us < 250_000".into(),
        },
        WorkloadSpec {
            id: "W14".into(),
            name: "mesh_protocol_security_and_collab".into(),
            title: "MeshEnvelope v1.0 零信任签名、防重放与受控沙盒协同".into(),
            category: "D4/D6 Mesh Protocol".into(),
            baseline_desc: "裸 HTTP 明文直写、无防重放与敏感文件直传导致篡改/重放/凭据泄露".into(),
            body_desc: "HMAC-SHA256 签名 + NonceCache 防重放 + 对话脱敏 + mesh_inbox 沙盒隔离".into(),
            sla_desc: "4/4 安全协同防线命中, replay/tamper/sensitive 100% 拦截, body_elapsed_us < 100_000".into(),
        },
        WorkloadSpec {
            id: "W15".into(),
            name: "optimistic_vs_pessimistic_execution_loop".into(),
            title: "AOPCC 乐观直通与悲观门禁自适应及碰壁秒级自愈".into(),
            category: "D2/D6 Concurrency Loop".into(),
            baseline_desc: "全悲观回路每步前置查表付出整轮对话税 (20 rounds) 或裸奔盲猜反复失败".into(),
            body_desc: "Brief MVCC 乐观租约 + 三态分类器 + PostToolUse <2ms 碰壁秒级自愈与自动降级".into(),
            sla_desc: "rounds_saved >= 8, 4/4 AOPCC 断言命中, token_savings_pct >= 50%, body_elapsed_us < 250_000".into(),
        },
        WorkloadSpec {
            id: "W16".into(),
            name: "dual_domain_audit_and_lease_self_evolution".into(),
            title: "双域行为审计脱敏防洪与租约自进化闭环".into(),
            category: "D6/D7 Audit & Self-Evolution".into(),
            baseline_desc: "无统一审计或明文日志泛洪 (10+ 噪声事件)、Daemon 隐身与静态规则漂移 (12 rounds)".into(),
            body_desc: "IN_AGENT/OUT_OF_AGENT 双域审计 + 密钥脱敏 + 60s Watcher 防洪折叠 + AOPCC 滑窗升降级".into(),
            sla_desc: "4/4 双域审计与自进化断言命中, token_savings_pct >= 50%, body_elapsed_us < 1_000_000".into(),
        },
        WorkloadSpec {
            id: "W17".into(),
            name: "cross_brain_continuity_and_body_proprioception".into(),
            title: "跨大脑连续性接力与本体器官自省".into(),
            category: "D6/D7 Embodiment & Continuity".into(),
            baseline_desc: "新大脑切换接盘时盲目重跑 ls/find/ps/git log 重新探索宿主与任务态 (4 rounds)".into(),
            body_desc: "BodyManifest 6 器官自省 + ContinuityCapsule (<=300 tok 脱敏抗投毒) + L0~L3 自治包络拦截".into(),
            sla_desc: "4/4 接力/脱敏/包络断言命中, rounds_saved >= 2, token_savings_pct >= 50%, +ΔQ > 0".into(),
        },
        WorkloadSpec {
            id: "W18".into(),
            name: "perf_awr_and_self_diagnostics".into(),
            title: "内生等待事件与 AWR/ADDM 自诊断闭环".into(),
            category: "D3/D7 Self-Telemetry & AWR".into(),
            baseline_desc: "无内生遥测时外部轮询 ps/lsof/iostat/top 抓性能瓶颈与内存漂移 (5 rounds)".into(),
            body_desc: "18/18 WaitEventRegistry + AWR 快照差分 + ADDM 根因诊断 + 报告删除闭环 (1 round)".into(),
            sla_desc: "4/4 AWR/ADDM/分位单调性断言命中, token_savings_pct >= 50%, body_elapsed_us < 250_000".into(),
        },
        WorkloadSpec {
            id: "W19".into(),
            name: "knowhow_graph_and_omni_rrf_recall".into(),
            title: "弱向量 L1/L2 Know-How 网络与 Omni-RRF 多路融合召回".into(),
            category: "D6 Know-How Graph & Omni-RRF".into(),
            baseline_desc: "弱小模型无图谱时纯字面 FTS 漏召回跨词因果、拼写错误与关联图表 (4 rounds)".into(),
            body_desc: "L1 章节PMI网 + 原文锚定 L2 Know-How + 8维 Omni-RRF + 写穿级联失效 (1 round)".into(),
            sla_desc: "4/4 跨词/容错/防幻觉/写穿断言命中, token_savings_pct >= 50%, body_elapsed_us < 250_000".into(),
        },
        WorkloadSpec {
            id: "W20".into(),
            name: "pg_tx_mvcc_concurrency_and_write_through".into(),
            title: "PostgreSQL 事务记忆 MVCC 写穿、透明降级与防脏读自愈".into(),
            category: "D6 Transactional Memory (Scheme A)".into(),
            baseline_desc: "无双时态/无写穿失效时多 Agent 并发覆写丢更新、读到僵尸记忆且无 MVCC 自愈 (4 rounds)".into(),
            body_desc: "PgTxMemoryAdapter 双时态写穿契约 + SQLite 透明降级 + MVCC_EPOCH_CONFLICT(<2ms) + 胶囊 tx_epoch 锚定 (1 round)".into(),
            sla_desc: "4/4 降级脱敏/OCC自愈/胶囊纪元/墓碑审计断言命中, token_savings_pct >= 50%, body_elapsed_us < 250_000".into(),
        },
        WorkloadSpec {
            id: "W21".into(),
            name: "sleep_time_compaction_and_bypass_pareto".into(),
            title: "L1_AUTONOMIC 空闲睡眠整理、可预测性旁路与 3x~5x 降本".into(),
            category: "D6 Sleep-Time Compaction (M2)".into(),
            baseline_desc: "无睡眠整理时全量注入重复寒暄/auto_sync 噪声与明文秘钥，高成本低信噪比 (5 rounds)".into(),
            body_desc: "SalienceScorer 分级 + SleepTimeCompactor 聚类压缩 + QueryPredictabilityGate 旁路 + OUT_OF_AGENT/PR-15 存证 (1 round)".into(),
            sla_desc: "4/4 显著性/压缩比/旁路召回/审计开关断言命中, Fact Recall=100%, Q_body >= Q_baseline".into(),
        },
        WorkloadSpec {
            id: "W22".into(),
            name: "fast_weight_lineage_gates_and_unlearn_rollback".into(),
            title: "方案 B 旁路快权重血缘、三门禁熔断与热态跨脑接力".into(),
            category: "D7 Fast-Weight Control Plane (M3)".into(),
            baseline_desc: "无旁路快权重控制面与门禁熔断时全量重放冷上下文或直接内联权重导致回归与墓碑泄漏 (4 rounds)".into(),
            body_desc: "FastWeightGatePolicy 三门禁 + URI 禁内联 + ContinuityCapsule v2 同/异构基座校验 + PR-16 存证 (1 round)".into(),
            sla_desc: "4/4 门禁激活/故障熔断/跨脑校验/PR-16 断言命中, norm_ratio<=1.50, glue>=-0.03, unlearn_leak<0.001".into(),
        },
    ]
}

pub fn run_all_workloads(storage: Arc<dyn StorageProvider>) -> Result<Vec<WorkloadResult>> {
    let mut results = Vec::with_capacity(22);
    for spec in list_workload_specs() {
        results.push(run_workload(storage.clone(), &spec.id)?);
    }
    Ok(results)
}

pub fn run_workload(storage: Arc<dyn StorageProvider>, id: &str) -> Result<WorkloadResult> {
    match id.to_uppercase().as_str() {
        "W1" => run_w1_capability_discovery(),
        "W2" => run_w2_port_collision(storage),
        "W3" => run_w3_granularity_scan(),
        "W4" => run_w4_placement_governance(storage),
        "W5" => run_w5_dependency_impact(),
        "W6" => run_w6_deadlock_resolution(),
        "W7" => run_w7_context_routing(storage),
        "W8" => run_w8_adaptive_brief(storage),
        "W9" => run_w9_hybrid_semantic_recall(),
        "W10" => run_w10_link_precision_regression(),
        "W11" => run_w11_security_redaction_and_guardrail(),
        "W12" => run_w12_freshness_and_staleness_healing(storage),
        "W13" => run_w13_semantic_and_weighted_graph_discovery(),
        "W14" => run_w14_mesh_protocol_security_and_collab(),
        "W15" => run_w15_optimistic_vs_pessimistic_execution_loop(),
        "W16" => run_w16_dual_domain_audit_and_lease_self_evolution(),
        "W17" => run_w17_cross_brain_continuity_and_body_proprioception(),
        "W18" => run_w18_perf_awr_and_self_diagnostics(),
        "W19" => run_w19_knowhow_graph_and_omni_rrf(),
        "W20" => run_w20_pg_tx_mvcc_and_write_through(),
        "W21" => run_w21_sleep_time_compaction_pareto(),
        "W22" => run_w22_fast_weight_gates_and_continuity(),
        _ => anyhow::bail!("未知 Benchmark 工作负载 ID: {} (支持 W1~W22)", id),
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn finalize_result(
    id: &str,
    name: &str,
    title: &str,
    category: &str,
    baseline_tokens: i64,
    body_tokens: i64,
    baseline_rounds: i64,
    body_rounds: i64,
    baseline_elapsed_us: u128,
    body_elapsed_us: u128,
    baseline_failures: i64,
    hazards_intercepted: i64,
    raw_entries: usize,
    kept_entries: usize,
    blocked_noise: usize,
    sla_passed: bool,
    baseline_command: String,
    body_action: String,
    evidence_detail: String,
) -> WorkloadResult {
    let tokens_saved = baseline_tokens.saturating_sub(body_tokens);
    let token_savings_pct = if baseline_tokens > 0 {
        ((tokens_saved as f64 / baseline_tokens as f64) * 100.0 * 100.0).round() / 100.0
    } else {
        0.0
    };
    let rounds_saved = baseline_rounds.saturating_sub(body_rounds);
    let speedup_factor = if body_elapsed_us > 0 {
        ((baseline_elapsed_us as f64 / body_elapsed_us as f64) * 100.0).round() / 100.0
    } else {
        1.0
    };
    let interception_rate_pct = if baseline_failures > 0 {
        (((hazards_intercepted.min(baseline_failures) as f64 / baseline_failures as f64) * 100.0) * 100.0).round() / 100.0
    } else {
        100.0
    };
    let noise_reduction_pct = if raw_entries > 0 {
        (((blocked_noise as f64 / raw_entries as f64) * 100.0) * 100.0).round() / 100.0
    } else {
        0.0
    };

    WorkloadResult {
        id: id.into(),
        name: name.into(),
        title: title.into(),
        category: category.into(),
        baseline_tokens,
        body_tokens,
        tokens_saved,
        token_savings_pct,
        baseline_rounds,
        body_rounds,
        rounds_saved,
        baseline_elapsed_us,
        body_elapsed_us,
        speedup_factor,
        baseline_failures,
        hazards_intercepted,
        interception_rate_pct,
        raw_entries,
        kept_entries,
        blocked_noise,
        noise_reduction_pct,
        sla_passed,
        baseline_command,
        body_action,
        evidence_detail,
        quality_score: None,
        precision_pct: None,
        recall_pct: None,
        baseline_quality_score: 0.0,
        body_quality_score: 0.0,
        quality_delta: 0.0,
        baseline_facts_hit: 0,
        body_facts_hit: 0,
        required_facts_total: 0,
        info_density_gain: 1.0,
    }
}


#[cfg(test)]
mod pareto_tests {
    use super::*;

    #[test]
    fn test_pareto_gate_rejects_empty_or_truncated_output() {
        let ev = evaluate_context_quality(
            ContextQualityInputs {
                required_facts_total: 5,
                baseline_facts_hit: 4,
                body_facts_hit: 1,
                baseline_signal_precision: 0.5,
                body_signal_precision: 1.0,
                baseline_actionability: 0.5,
                body_actionability: 0.2,
                baseline_safety: 0.8,
                body_safety: 1.0,
            },
            10_000,
            1,
            85.0,
        );
        assert!(!ev.pareto_passed, "截断丢事实时帕累托门禁必须拒绝通过");
    }
}
