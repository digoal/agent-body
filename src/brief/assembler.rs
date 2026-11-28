//! 自适应情境简报组装器（BriefAssembler）
//!
//! 在 `≤ budget_tokens` 的 token 预算内，从三大模块按优先级矩阵采样，
//! 组装为 Agent 可直接消费的高信噪比 Markdown 情境简报。
//!
//! ## 三模块采样优先级矩阵
//!
//! | 模块 | 预算占比 | 采样内容 |
//! |---|:---:|---|
//! | 事实图谱 (Fact) | 30% | 工具能力 / 活跃端口 / 相关仓库 |
//! | 偏好规约 (Pref) | 30% | 放置规则 / 工程纪律摘要 |
//! | 记忆经验 (Memory) | 40% | 历史决策 / 语义搜索命中经验 |
//!
//! ## Token 预算估算
//!
//! 使用 `chars / 4` 快速近似（1 token ≈ 4 chars ASCII；中文字符按 1.5x 计）。

use std::collections::HashSet;

use anyhow::Result;
use chrono::Local;

use crate::brief::format;
use crate::brief::optimistic::{OptimisticLease, OptimisticLeaseEngine};
use crate::fact::FactFacade;
use crate::memory::{MemoryFilter, MemoryProvider};
use crate::placement::conventions::ConventionsEngine;
use crate::placement::PlacementEngine;
use crate::storage::Storage;

// ─── 质量分结构 ──────────────────────────────────────────────────────────────

/// 情境简报质量评分（0-100）
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct BriefQuality {
    /// 事实覆盖率分（0-100）：工具/端口/仓库命中与查询词的匹配程度
    pub fact_coverage: u8,
    /// 记忆命中率分（0-100）：返回条目相关度均值
    pub memory_relevance: u8,
    /// 偏好合规率分（0-100）：放置规则与工程纪律是否被注入
    pub preference_compliance: u8,
    /// 综合质量分（加权平均）
    pub overall: u8,
}

impl BriefQuality {
    /// 计算综合质量分：Fact 30% / Memory 40% / Pref 30%
    pub fn compute(fact: u8, memory: u8, pref: u8) -> Self {
        let overall = (fact as u32 * 30 + memory as u32 * 40 + pref as u32 * 30) / 100;
        Self {
            fact_coverage: fact,
            memory_relevance: memory,
            preference_compliance: pref,
            overall: overall as u8,
        }
    }
}

// ─── 简报请求与响应 ──────────────────────────────────────────────────────────

/// BriefAssembler 请求参数
#[derive(Debug, Clone)]
pub struct BriefRequest {
    /// 本次任务意图/关键词（用于语义检索事实与记忆）
    pub query: String,
    /// 当前会话 ID（用于拉取历史决策记录）
    pub session_id: String,
    /// Token 预算上限（默认 1500）
    pub budget_tokens: usize,
}

impl Default for BriefRequest {
    fn default() -> Self {
        Self {
            query: String::new(),
            session_id: String::new(),
            budget_tokens: 1500,
        }
    }
}

/// BriefAssembler 响应
#[derive(Debug, Clone)]
pub struct BriefOutput {
    /// Markdown 格式的情境简报正文
    pub markdown: String,
    /// 实际估算消耗 tokens
    pub tokens_used: usize,
    /// 质量评分
    pub quality: BriefQuality,
    /// 开工签发的 MVCC 乐观直通租约列表 (Phase 42 AOPCC)
    pub optimistic_leases: Vec<OptimisticLease>,
    /// 本次注入的跨大脑连续性接力胶囊 ID (Phase 44 Agent 本体)
    pub active_capsule_id: Option<String>,
}

// ─── 组装器主体 ─────────────────────────────────────────────────────────────

/// 自适应情境简报组装器
pub struct BriefAssembler {
    storage: Storage,
    memory_provider: Option<std::sync::Arc<dyn MemoryProvider>>,
}

impl BriefAssembler {
    /// 创建组装器（memory_provider 可选，为 None 时跳过记忆模块）
    pub fn new(
        storage: Storage,
        memory_provider: Option<std::sync::Arc<dyn MemoryProvider>>,
    ) -> Self {
        Self {
            storage,
            memory_provider,
        }
    }

    /// 组装情境简报
    pub fn assemble(&self, req: &BriefRequest) -> Result<BriefOutput> {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::CpuBriefAssemble);
        let now = Local::now().format("%Y-%m-%dT%H:%M CST").to_string();
        let budget = req.budget_tokens.max(300); // 至少 300 tokens

        // 三模块预算分配
        let fact_budget = budget * 30 / 100;
        let pref_budget = budget * 30 / 100;
        let mem_budget = budget * 40 / 100;

        // ── 1. 事实模块采样 ───────────────────────────────────────────────
        let (fact_section, fact_score) = self.build_fact_section(&req.query, fact_budget);

        // ── 1.5 开工乐观直通租约签发 (Phase 42 AOPCC) ─────────────────────
        let lease_report = OptimisticLeaseEngine::issue_leases(&self.storage, &req.query);
        let lease_section = if budget <= 350 {
            String::new()
        } else {
            lease_report.markdown_section.clone()
        };

        // ── 2. 偏好规约采样 ───────────────────────────────────────────────
        let (pref_section, pref_score) = self.build_pref_section(&req.query, pref_budget);

        // ── 3. 记忆经验采样 ───────────────────────────────────────────────
        let (mem_section, mem_score) = self.build_memory_section(
            &req.query,
            &req.session_id,
            mem_budget,
        );

        // ── 4. 按需生态路由 (Phase 16 Context 防爆按需披露) ─────────────
        let eco_section = self.build_ecosystem_section(&req.query, (budget * 20 / 100).min(350));

        // ── 4.5 跨大脑连续性接力胶囊 (Phase 44 Agent 本体，仅在非沙盒且预算充足时注入) ──
        let body_id = crate::body::manifest::compute_body_id();
        let (capsule_section, active_capsule_id) = if budget >= 800
            && !req.session_id.starts_with("bench:")
            && !req.session_id.starts_with("demo:")
        {
            match self.storage.get_latest_active_body_checkpoint(&body_id) {
                Ok(Some(cp)) => (
                    format!("{}\n", crate::body::ContinuityManager::render_brief_snippet(&cp, 150)),
                    Some(cp.checkpoint_id),
                ),
                _ => (String::new(), None),
            }
        } else {
            (String::new(), None)
        };

        // ── 5. 拼装最终 Markdown ─────────────────────────────────────────
        let quality = BriefQuality::compute(fact_score, mem_score, pref_score);
        let session_disp = if req.session_id.is_empty() {
            "未指定".to_string()
        } else {
            req.session_id.chars().take(16).collect::<String>()
        };

        let markdown = format!(
            "# 🧬 Agent 本体开工简报 (body_brief)\n\n\
             **时间**: {now} | **本体**: {body_id} | **会话**: {session_disp} | **质量分**: {overall}/100\n\n\
             {capsule_section}\
             {fact_section}\n\
             {lease_section}\
             {pref_section}\n\
             {mem_section}\
             {eco_section}",
            now = now,
            body_id = body_id,
            session_disp = session_disp,
            overall = quality.overall,
            capsule_section = capsule_section,
            fact_section = fact_section,
            lease_section = lease_section,
            pref_section = pref_section,
            mem_section = mem_section,
            eco_section = eco_section,
        );

        let tokens_used = estimate_tokens(&markdown);
        let leases_count = lease_report.leases.len();

        if !req.session_id.starts_with("bench:") && !req.session_id.starts_with("demo:") {
            let lease_ids: Vec<&str> = lease_report
                .leases
                .iter()
                .map(|l| l.lease_id.as_str())
                .collect();
            crate::audit::AuditLogger::record_best_effort(
                &self.storage,
                crate::audit::AuditEventInput::in_agent("brief", "brief_lease", "assemble_brief")
                    .with_session(if req.session_id.is_empty() {
                        "default".to_string()
                    } else {
                        req.session_id.clone()
                    })
                    .with_target("query", req.query.chars().take(120).collect::<String>())
                    .with_concurrency_mode("OPTIMISTIC")
                    .with_outcome("success")
                    .with_delta(serde_json::json!({
                        "tokens_used": tokens_used,
                        "quality_overall": quality.overall,
                        "leases_count": leases_count,
                        "lease_ids": lease_ids,
                        "active_capsule_id": active_capsule_id,
                    })),
            );
        }

        Ok(BriefOutput {
            markdown,
            tokens_used,
            quality,
            optimistic_leases: lease_report.leases,
            active_capsule_id,
        })
    }

    // ── 事实模块（Index-First：优先读取 fact_nodes / assets 持久化索引） ────────

    fn build_fact_section(&self, query: &str, budget_tokens: usize) -> (String, u8) {
        let mut lines: Vec<String> = vec!["## 🔧 宿主机事实\n".to_string()];
        let mut score: u8 = 60; // 基础分
        let q_lower = query.trim().to_lowercase();
        let q_terms: Vec<&str> = q_lower
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .filter(|s| s.len() >= 2)
            .collect();

        // 一次批量拉取持久化事实图谱节点（SQLite < 500 μs，避免扫盘与派生子进程）
        let all_nodes = self.storage.list_all_fact_nodes(500).unwrap_or_default();
        let tool_nodes: Vec<_> = all_nodes.iter().filter(|n| n.node_type == "tool").collect();
        let port_nodes: Vec<_> = all_nodes.iter().filter(|n| n.node_type == "port").collect();
        let proj_nodes: Vec<_> = all_nodes.iter().filter(|n| n.node_type == "project").collect();

        // 1. 工具能力：优先消费 fact_nodes(tool)，空库时走免子进程路径探测
        if !tool_nodes.is_empty() {
            let mut scored_tools: Vec<(usize, &crate::storage::models::FactNodeRecord)> = tool_nodes
                .iter()
                .map(|n| {
                    let hay = format!(
                        "{} {} {}",
                        n.name.to_lowercase(),
                        n.label.to_lowercase(),
                        n.properties
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_lowercase()
                    );
                    let hits = q_terms.iter().filter(|t| hay.contains(*t)).count();
                    (hits, *n)
                })
                .collect();
            scored_tools.sort_by_key(|a| std::cmp::Reverse(a.0));
            let tool_line = scored_tools
                .iter()
                .take(5)
                .map(|(_, n)| {
                    let desc = n
                        .properties
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or(&n.label);
                    format!("{} ({})", n.name, format::truncate_clean(desc, 36))
                })
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("- **可用工具**: {}\n", tool_line));
            score = score.saturating_add(15);
        } else {
            let tool_query = if query.is_empty() { "文件处理" } else { query };
            let mut cap_engine = crate::fact::CapabilityEngine::new();
            cap_engine.scan_host_paths_only();
            let res = cap_engine.query(tool_query);
            if !res.all_matching_tools.is_empty() {
                let tool_line = res
                    .all_matching_tools
                    .iter()
                    .take(5)
                    .map(|t| format!("{} ({})", t.tool_name, format::truncate_clean(&t.description, 36)))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!("- **可用工具**: {}\n", tool_line));
                score = score.saturating_add(15);
            } else {
                lines.push("- **可用工具**: 无法获取\n".to_string());
            }
        }

        // 2. 活跃监听端口：优先消费 fact_nodes(port)，空库时回退物理探针
        if !port_nodes.is_empty() {
            let port_line = port_nodes
                .iter()
                .take(5)
                .map(|n| {
                    let port_str = n
                        .properties
                        .get("port")
                        .and_then(|v| v.as_u64())
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| n.name.trim_start_matches("port:").to_string());
                    let proc_name = n
                        .properties
                        .get("process_name")
                        .or_else(|| n.properties.get("process"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("未知");
                    format!(":{} ({})", port_str, proc_name.chars().take(20).collect::<String>())
                })
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("- **活跃端口**: {}\n", port_line));
            score = score.saturating_add(10);
        } else {
            match FactFacade::query_ports() {
                Ok(ports) if !ports.is_empty() => {
                    let port_line = ports
                        .iter()
                        .take(5)
                        .map(|p| format!(":{} ({})", p.port, p.process_name.as_deref().unwrap_or("未知").chars().take(20).collect::<String>()))
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(format!("- **活跃端口**: {}\n", port_line));
                    score = score.saturating_add(10);
                }
                _ => {
                    lines.push("- **活跃端口**: 无监听信息\n".to_string());
                }
            }
        }

        // 3. 相关 Git 仓库/工程：优先消费 fact_nodes(project)，其次 assets FTS，避免派生 git 子进程扫盘
        if !proj_nodes.is_empty() {
            let mut scored_proj: Vec<(usize, &crate::storage::models::FactNodeRecord)> = proj_nodes
                .iter()
                .map(|n| {
                    let path_str = n
                        .properties
                        .get("canonical_path")
                        .or_else(|| n.properties.get("path"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(&n.name);
                    let hay = format!(
                        "{} {} {}",
                        n.name.to_lowercase(),
                        path_str.to_lowercase(),
                        n.properties
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_lowercase()
                    );
                    let hits = q_terms.iter().filter(|t| hay.contains(*t)).count();
                    (hits, *n)
                })
                .collect();
            scored_proj.sort_by_key(|a| std::cmp::Reverse(a.0));
            let repo_line = scored_proj
                .iter()
                .take(3)
                .map(|(_, n)| {
                    let path_str = n
                        .properties
                        .get("canonical_path")
                        .or_else(|| n.properties.get("path"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(&n.name);
                    let cg = if n
                        .properties
                        .get("codegraph_ready")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                        || std::path::Path::new(path_str).join(".codegraph").exists()
                    {
                        " [codegraph✓]"
                    } else {
                        ""
                    };
                    format!("{}{}", format::truncate_clean(path_str, 64), cg)
                })
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("- **相关仓库**: {}\n", repo_line));
            score = score.saturating_add(15);
        } else if !q_lower.is_empty() {
            let mut opts = crate::scanner::AssetSearchOptions::new(query, 5);
            opts.hybrid = false;
            opts.include_links = false;
            if let Ok(hits) =
                crate::scanner::AssetSearchEngine::search(self.storage.provider().as_ref(), &opts)
            {
                let repos: Vec<_> = hits
                    .into_iter()
                    .map(|h| h.asset)
                    .filter(|a| a.asset_type == "project" || a.asset_type == "repo")
                    .collect();
                if !repos.is_empty() {
                    let repo_line = repos
                        .iter()
                        .take(3)
                        .map(|a| format::truncate_clean(&a.path, 64))
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(format!("- **相关仓库**: {}\n", repo_line));
                    score = score.saturating_add(15);
                }
            }
        }

        // 4. 活跃动态任务（索引优先读取 active_processes 表，避免阻塞扫 /proc）
        if let Ok(procs) = self.storage.list_active_processes() {
            if !procs.is_empty() {
                let proc_line = procs
                    .iter()
                    .take(3)
                    .map(|p| format!("{} (PID {})", p.name, p.pid))
                    .collect::<Vec<_>>()
                    .join(", ");
                lines.push(format!("- **活跃任务**: {}\n", proc_line));
            }
        }

        let section = format::truncate_to_budget_clean(lines.join(""), budget_tokens);
        (section, score.min(100))
    }

    // ── 偏好规约模块 ─────────────────────────────────────────────────────

    fn build_pref_section(&self, query: &str, budget_tokens: usize) -> (String, u8) {
        let conventions = ConventionsEngine::get_filtered_conventions(query);
        let placement_engine = PlacementEngine::new(self.storage.clone());
        let mut lines: Vec<String> = vec!["## 📐 工程偏好\n".to_string()];
        let mut score: u8 = 50;

        // 核心工程纪律（取前 3 条最相关）
        for conv in conventions.iter().take(3) {
            let desc = format!("{}: {}", conv.rule_title, conv.requirement);
            let short = format::truncate_clean(&desc, 96);
            lines.push(format!("- {}\n", short));
            score = score.saturating_add(10);
        }

        // 放置规则摘要（从引擎获取核心规则提示）
        let rules_summary = placement_engine.get_rules_summary();
        if !rules_summary.is_empty() {
            lines.push(format!(
                "- **放置规则摘要**: {}\n",
                format::truncate_clean(&rules_summary, 140)
            ));
            score = score.saturating_add(20);
        }

        let section = format::truncate_to_budget_clean(lines.join(""), budget_tokens);
        (section, score.min(100))
    }

    // ── 记忆经验模块 ─────────────────────────────────────────────────────

    fn build_memory_section(
        &self,
        query: &str,
        session_id: &str,
        budget_tokens: usize,
    ) -> (String, u8) {
        let mut lines: Vec<String> = vec!["## 🧠 相关历史经验\n".to_string()];
        let mut score: u8 = 40;

        let Some(ref provider) = self.memory_provider else {
            lines.push("- 记忆模块未启用\n".to_string());
            return (lines.join(""), score);
        };

        let mut seen_ids: HashSet<String> = HashSet::new();
        let mut seen_norm_keys: HashSet<String> = HashSet::new();
        let mut seen_targets: HashSet<String> = HashSet::new();
        let mut shown_count: usize = 0;

        let mut accept_memory = |id: &str, clean_text: &str| -> bool {
            if !id.is_empty() && seen_ids.contains(id) {
                return false;
            }
            let (norm_key, target_key) = format::memory_dedup_keys(clean_text);
            if !norm_key.is_empty() && seen_norm_keys.contains(&norm_key) {
                return false;
            }
            if let Some(ref tk) = target_key {
                if seen_targets.contains(tk) {
                    return false;
                }
            }
            if !id.is_empty() {
                seen_ids.insert(id.to_string());
            }
            if !norm_key.is_empty() {
                seen_norm_keys.insert(norm_key);
            }
            if let Some(tk) = target_key {
                seen_targets.insert(tk);
            }
            true
        };

        // 1. 历史决策（按 session_id 过滤）
        if !session_id.is_empty() {
            if let Ok(ctx) = provider.get_context(session_id) {
                let mut dec_shown = 0usize;
                for item in ctx.key_decisions.iter() {
                    if dec_shown >= 3 {
                        break;
                    }
                    let clean = crate::scanner::metadata::PromptInjectionScanner::scan_and_sanitize(&item.content).sanitized;
                    if !accept_memory(&item.id, &clean) {
                        continue;
                    }
                    shown_count += 1;
                    dec_shown += 1;
                    let preview = format::truncate_clean(&clean, 120);
                    lines.push(format!("{}. [决策] {}\n", shown_count, preview));
                    score = score.saturating_add(10);
                }
            }
        }

        // 2. 语义搜索命中经验（结合 QueryPredictabilityGate 在常规 queries 优先利用睡眠压缩摘要）
        let search_query = if query.is_empty() { "agent body" } else { query };
        let filter = MemoryFilter {
            memory_type: None,
            tags: vec![],
            source_session: None,
            min_score: Some(0.1),
        };
        if let Ok(mut items) = provider.search(search_query, 20, Some(&filter)) {
            let summaries: Vec<String> = items
                .iter()
                .filter(|it| {
                    it.tags
                        .iter()
                        .any(|t| t == "sleep_compacted" || t == "sleep_summary")
                })
                .map(|it| it.content.clone())
                .collect();
            if !summaries.is_empty() {
                let s_refs: Vec<&str> = summaries.iter().map(|s| s.as_str()).collect();
                let dec = ext_mem_pg_tx::QueryPredictabilityGate::evaluate(
                    search_query,
                    &s_refs,
                    None,
                    false,
                );
                if !dec.bypass_compacted {
                    items.sort_by_key(|it| {
                        if it.tags.iter().any(|t| t == "sleep_compacted" || t == "sleep_summary") {
                            0u8
                        } else {
                            1u8
                        }
                    });
                }
            }
            let mut search_shown = 0usize;
            for item in items.iter() {
                if search_shown >= 5 {
                    break;
                }
                let clean = crate::scanner::metadata::PromptInjectionScanner::scan_and_sanitize(&item.content).sanitized;
                if !accept_memory(&item.id, &clean) {
                    continue;
                }
                shown_count += 1;
                search_shown += 1;
                let preview = format::truncate_clean(&clean, 120);
                let score_str = item.score.map(|s| format!(" ({:.2})", s)).unwrap_or_default();
                lines.push(format!(
                    "{}. [{}]{} {}\n",
                    shown_count,
                    item.memory_type,
                    score_str,
                    preview
                ));
                score = score.saturating_add(8);
            }
        }

        if lines.len() == 1 {
            lines.push("- 暂无相关历史经验\n".to_string());
        }

        let section = format::truncate_to_budget_clean(lines.join(""), budget_tokens);
        (section, score.min(100))
    }

    // ── 生态按需路由模块 (Phase 16) ─────────────────────────────────────────

    fn build_ecosystem_section(&self, query: &str, budget_tokens: usize) -> String {
        if query.trim().is_empty() || budget_tokens < 100 {
            return String::new();
        }
        let router = crate::ecosystem::ContextBudgetRouter::new(self.storage.clone());
        let req = crate::ecosystem::ContextRouteRequest {
            intent: query.to_string(),
            budget_tokens,
            max_skills: 2,
            max_mcps: 2,
            max_services: 2,
        };
        match router.route(&req) {
            Ok(plan) if !plan.skills.is_empty() || !plan.mcps.is_empty() || !plan.services.is_empty() => {
                format!("\n{}", plan.markdown)
            }
            _ => String::new(),
        }
    }
}

// ─── Token 估算工具 ──────────────────────────────────────────────────────────

/// 快速 Token 估算：中文字符 ×1.5 / 其他字符 ÷4
pub fn estimate_tokens(text: &str) -> usize {
    let mut tokens = 0usize;
    for ch in text.chars() {
        if ch as u32 > 0x4E00 {
            // 中日韩字符
            tokens += 2; // 约 1.5 tokens，乘以 2 再除以 3（整数近似）
        } else {
            tokens += 1;
        }
    }
    tokens / 4 + 1
}

// ─── 单元测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;

    fn make_assembler() -> BriefAssembler {
        let storage = Storage::open_in_memory().expect("in-memory storage");
        BriefAssembler::new(storage, None)
    }

    #[test]
    fn test_estimate_tokens_ascii() {
        let text = "hello world this is a test of token estimation for ascii";
        let t = estimate_tokens(text);
        // 57 chars / 4 ≈ 14~15 tokens
        assert!(t > 10 && t < 25, "ASCII token 估算应合理，实际: {}", t);
    }

    #[test]
    fn test_estimate_tokens_chinese() {
        let text = "选用 Rust 编写高性能Agent 本体守护进程";
        let t = estimate_tokens(text);
        assert!(t > 5, "中文 token 估算应大于 5，实际: {}", t);
    }

    #[test]
    fn test_brief_assembly_no_memory_provider() {
        let assembler = make_assembler();
        let req = BriefRequest {
            query: "rust agent".to_string(),
            session_id: "sess-test-001".to_string(),
            budget_tokens: 1500,
        };
        let output = assembler.assemble(&req).expect("assemble 应成功");
        assert!(!output.markdown.is_empty(), "简报不应为空");
        assert!(output.markdown.contains("Agent 本体开工简报"), "简报应包含标题");
        assert!(output.markdown.contains("🔧 宿主机事实"), "应包含事实模块");
        assert!(output.markdown.contains("📐 工程偏好"), "应包含偏好模块");
        assert!(output.markdown.contains("🧠 相关历史经验"), "应包含记忆模块");
        assert!(output.tokens_used > 0, "token 消耗应大于 0");
    }

    #[test]
    fn test_brief_quality_compute() {
        let q = BriefQuality::compute(80, 70, 90);
        // 80*30 + 70*40 + 90*30 = 2400 + 2800 + 2700 = 7900 / 100 = 79
        assert_eq!(q.overall, 79);
        assert_eq!(q.fact_coverage, 80);
        assert_eq!(q.memory_relevance, 70);
        assert_eq!(q.preference_compliance, 90);
    }

    #[test]
    fn test_brief_respects_token_budget() {
        let assembler = make_assembler();
        let req = BriefRequest {
            query: String::new(),
            session_id: String::new(),
            budget_tokens: 300, // 极小预算，测试截断
        };
        let output = assembler.assemble(&req).expect("assemble 应成功");
        // 极小预算下也应生成合理简报
        assert!(!output.markdown.is_empty());
        // token 消耗不应超出预算太多（允许合理边界）
        assert!(
            output.tokens_used < 600,
            "token 消耗 {} 超出 300 预算过多",
            output.tokens_used
        );
    }

    #[test]
    fn test_brief_with_memory_provider() {
        use crate::memory::builtin_provider::BuiltinMemoryProvider;
        use crate::memory::spi::MemoryItem;
        use std::sync::Arc;
        use std::collections::HashMap;

        let storage = Storage::open_in_memory().expect("in-memory");
        let provider = Arc::new(BuiltinMemoryProvider::new(storage.clone()));

        // 预植入一条记忆
        let item = MemoryItem {
            id: "mem-p4-001".to_string(),
            memory_type: "decision".to_string(),
            content: "Phase 4 决策：BriefAssembler 三模块优先级矩阵".to_string(),
            summary: Some("BriefAssembler 设计".to_string()),
            tags: vec!["brief".to_string(), "phase4".to_string()],
            source_agent: Some("test".to_string()),
            source_session: Some("sess-p4".to_string()),
            created_at: "2026-11-17 19:00:00".to_string(),
            score: Some(1.0),
            extra: HashMap::new(),
        };
        provider.store(&item).expect("store 应成功");

        let assembler = BriefAssembler::new(storage, Some(provider));
        let req = BriefRequest {
            query: "BriefAssembler".to_string(),
            session_id: "sess-p4".to_string(),
            budget_tokens: 1500,
        };
        let output = assembler.assemble(&req).expect("assemble 应成功");
        assert!(output.markdown.contains("🧠 相关历史经验"), "应包含记忆模块");
        // 预植入的决策应出现在简报中
        assert!(
            output.markdown.contains("BriefAssembler") || output.markdown.contains("Phase 4"),
            "简报应包含预植入记忆内容"
        );
    }

    #[test]
    fn test_brief_index_first_uses_fact_nodes() {
        use crate::storage::models::FactNodeRecord;

        let storage = Storage::open_in_memory().expect("in-memory");
        storage
            .upsert_fact_node(&FactNodeRecord {
                id: "tool:rg".to_string(),
                node_type: "tool".to_string(),
                name: "rg".to_string(),
                label: "ripgrep".to_string(),
                properties: serde_json::json!({"description": "极速代码检索工具"}),
                updated_at: None,
            })
            .expect("upsert tool");
        storage
            .upsert_fact_node(&FactNodeRecord {
                id: "port:8765".to_string(),
                node_type: "port".to_string(),
                name: "port:8765".to_string(),
                label: "8765".to_string(),
                properties: serde_json::json!({"port": 8765, "process_name": "agent-body"}),
                updated_at: None,
            })
            .expect("upsert port");
        storage
            .upsert_fact_node(&FactNodeRecord {
                id: "project:agent-body".to_string(),
                node_type: "project".to_string(),
                name: "agent-body".to_string(),
                label: "agent-body".to_string(),
                properties: serde_json::json!({
                    "canonical_path": "/root/new/work/agent-body",
                    "description": "Agent 本体"
                }),
                updated_at: None,
            })
            .expect("upsert project");

        let assembler = BriefAssembler::new(storage, None);
        let output = assembler
            .assemble(&BriefRequest {
                query: "agent-body".to_string(),
                session_id: "sess-idx".to_string(),
                budget_tokens: 1500,
            })
            .expect("assemble");

        assert!(output.markdown.contains("rg (极速代码检索工具)"));
        assert!(output.markdown.contains(":8765 (agent-body)"));
        assert!(output.markdown.contains("/root/new/work/agent-body"));
    }

    #[test]
    fn test_brief_includes_active_process_index() {
        use crate::storage::models::ActiveProcessRecord;

        let storage = Storage::open_in_memory().expect("in-memory");
        storage
            .replace_active_processes(&[ActiveProcessRecord {
                id: "proc-1234".to_string(),
                pid: 1234,
                name: "uvicorn".to_string(),
                command: "uvicorn app:main --port 8000".to_string(),
                owner: "user:digoal".to_string(),
                associated_path: None,
                listening_ports: vec![8000],
                status: "running".to_string(),
                cpu_pct: 1.0,
                mem_bytes: 1024 * 1024,
                gpu_allocated_mb: 0,
                started_at: None,
            }])
            .expect("replace active procs");

        let assembler = BriefAssembler::new(storage, None);
        let output = assembler
            .assemble(&BriefRequest {
                query: "uvicorn".to_string(),
                session_id: String::new(),
                budget_tokens: 1500,
            })
            .expect("assemble");

        assert!(output.markdown.contains("活跃任务"));
        assert!(output.markdown.contains("uvicorn (PID 1234)"));
    }

    #[test]
    fn test_brief_memory_dedup_and_clean_truncation() {
        use crate::memory::builtin_provider::BuiltinMemoryProvider;
        use crate::memory::spi::MemoryItem;
        use std::collections::HashMap;
        use std::sync::Arc;

        let storage = Storage::open_in_memory().expect("in-memory");
        let provider = Arc::new(BuiltinMemoryProvider::new(storage.clone()));

        let samples = vec![
            (
                "mem-dup-1",
                "task",
                "会话 [7e171592-8786-4328-8bf1-8e5f593a260a] 完工环境总结 (dry_run: false):\n- 清理/待清理临时文件数: 0\n- 环境变更: agent-body",
                vec!["session".to_string(), "close".to_string()],
            ),
            (
                "mem-dup-2",
                "task",
                "会话 [7e171592-8786-4328-8bf1-8e5f593a260a] 完工环境总结 (dry_run: false):\n- 清理/待清理临时文件数: 0\n- 环境变更: agent-body",
                vec!["session".to_string(), "close".to_string()],
            ),
            (
                "mem-dup-sleep",
                "experience",
                "[sleep_summary:session] 会话 [7e171592-8786-4328-8bf1-8e5f593a260a] 完工环境总结 (dry_run: false) agent-body",
                vec!["sleep_summary".to_string()],
            ),
            (
                "mem-note-toml",
                "note",
                "产出文件 `/root/new/src/agent-body/config.example.toml` 摘要:\n# ======================\n# agent-body 配置示例说明",
                vec!["auto_sync".to_string()],
            ),
            (
                "mem-md-sleep",
                "experience",
                "[sleep_summary:md] 产出文件 `/root/new/src/agent-body/docs/src_doc/roadmap-phase80.md` 完成阶段80开发、测试与一等公民同步",
                vec!["sleep_summary".to_string()],
            ),
        ];

        for (id, mtype, content, tags) in samples {
            provider
                .store(&MemoryItem {
                    id: id.to_string(),
                    memory_type: mtype.to_string(),
                    content: content.to_string(),
                    summary: None,
                    tags,
                    source_agent: Some("test".to_string()),
                    source_session: None,
                    created_at: "2026-11-28 22:00:00".to_string(),
                    score: Some(1.0),
                    extra: HashMap::new(),
                })
                .expect("store sample");
        }

        let assembler = BriefAssembler::new(storage, Some(provider));
        let output = assembler
            .assemble(&BriefRequest {
                query: "agent body".to_string(),
                session_id: String::new(),
                budget_tokens: 1500,
            })
            .expect("assemble");

        let mem_block = output
            .markdown
            .split("## 🧠 相关历史经验")
            .nth(1)
            .expect("memory section");
        let occ = mem_block
            .matches("7e171592-8786-4328-8bf1-8e5f593a260a")
            .count();
        assert_eq!(occ, 1, "同一会话总结与睡眠摘要应只出现 1 次:\n{}", mem_block);
        assert!(!mem_block.contains("==="), "不应包含装饰等号:\n{}", mem_block);
        assert!(
            mem_block.contains("roadmap-phase80.md`"),
            "反引号路径应完整闭合:\n{}",
            mem_block
        );
        assert!(mem_block.contains("1. ") && mem_block.contains("2. ") && mem_block.contains("3. "));
    }
}
