//! 三态运行模式与真实世界 Shadow A/B 对照及证伪引擎 (Phase 58 · ISSUE #43)
//!
//! 解决的核心问题：
//! 1. `bypass` 模式：零干预直通原生命令与实时文件搜索，供无干扰基准对照；
//! 2. `shadow_ab` 模式：真实运行期同请求双轨对照（原生实时扫描 vs agent-body 索引），
//!    自动统计耗时、Token 净差、召回交集、陈旧假命中与“帮倒忙率”，持久化至 `audit_events`；
//! 3. `body_only` 模式：默认全能力本体运行。

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::audit::{AuditEventInput, AuditLogger};
use crate::benchmark::provenance::estimate_tokens;
use crate::config::{Config, McpToolProfile};
use crate::scanner::importance::path_matches_ignore_pattern;
use crate::scanner::metadata::{format_asset_line, is_sensitive_file, read_head_utf8, DetailLevel, SecretRedactor};
use crate::storage::models::{AssetRecord, AuditQueryFilter};
use crate::storage::Storage;

/// 原生实时工作区搜索单条命中
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveAssetHit {
    pub path: String,
    pub snippet: String,
    pub match_score: usize,
}

/// 单次真实世界 Shadow A/B 对照明细
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowAbComparison {
    pub query: String,
    /// `"asset_search"` | `"brief"` | `"hook_pre"`
    pub kind: String,
    pub mode: String,
    /// 返回给调用方的主响应侧: `"live"` | `"body"`
    pub primary_served: String,
    pub live_elapsed_us: i64,
    pub body_elapsed_us: i64,
    pub live_tokens: i64,
    pub body_tokens: i64,
    pub live_hit_count: usize,
    pub body_hit_count: usize,
    pub overlap_count: usize,
    pub body_missed_count: usize,
    pub body_extra_count: usize,
    pub body_stale_count: usize,
    pub recall_vs_live_pct: f64,
    pub token_savings_pct: f64,
    /// `"body_win"` | `"tie"` | `"body_regression"` | `"body_stale"`
    pub verdict: String,
    pub reason: String,
    pub live_sample_paths: Vec<String>,
    pub body_sample_paths: Vec<String>,
}

/// 按观测类别分组统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbKindStats {
    pub kind: String,
    pub comparisons: usize,
    pub body_wins: usize,
    pub ties: usize,
    pub regressions: usize,
    pub stale_hits: usize,
    pub avg_live_us: f64,
    pub avg_body_us: f64,
    pub avg_token_savings_pct: f64,
    pub avg_recall_vs_live_pct: f64,
}

/// 真实世界 A/B 对照汇总报告（诚实披露正收益与帮倒忙场景）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbAnalysisReport {
    pub window: String,
    pub current_mode: String,
    pub current_mcp_profile: String,
    pub total_observations: usize,
    pub bypass_events: usize,
    pub shadow_comparisons: usize,
    pub body_wins: usize,
    pub ties: usize,
    pub body_regressions: usize,
    pub body_stale_incidents: usize,
    pub win_rate_pct: f64,
    pub regression_rate_pct: f64,
    pub avg_live_us: f64,
    pub avg_body_us: f64,
    pub avg_token_savings_pct: f64,
    pub avg_recall_vs_live_pct: f64,
    /// `"NET_POSITIVE"` | `"MIXED_NEEDS_TUNING"` | `"HURTING_INVESTIGATE"` | `"INSUFFICIENT_DATA"`
    pub verdict: String,
    pub verdict_summary: String,
    pub recommendations: Vec<String>,
    pub by_kind: Vec<AbKindStats>,
    pub worst_regressions: Vec<ShadowAbComparison>,
}

pub struct ShadowAbEngine;

impl ShadowAbEngine {
    /// 构建当前运行模式与 MCP Profile 字节减税状态 JSON（供 CLI / MCP / Web 共用）
    pub fn build_runtime_status(cfg: &Config) -> serde_json::Value {
        let mode = cfg.effective_runtime_mode();
        let profile = cfg.effective_mcp_profile();
        let env_mode = std::env::var("AGENT_BODY_MODE").ok().filter(|s| !s.trim().is_empty());
        let env_profile = std::env::var("AGENT_BODY_MCP_PROFILE").ok().filter(|s| !s.trim().is_empty());

        let full_schemas = crate::api::mcp_schemas::tool_schemas_for_profile(McpToolProfile::Full);
        let active_schemas = crate::api::mcp_schemas::tool_schemas_for_profile(profile);
        let full_bytes = serde_json::to_string(&full_schemas).map(|s| s.len()).unwrap_or(0);
        let active_bytes = serde_json::to_string(&active_schemas).map(|s| s.len()).unwrap_or(0);
        let reduction_pct = if full_bytes > 0 {
            ((full_bytes.saturating_sub(active_bytes)) as f64 / full_bytes as f64) * 100.0
        } else {
            0.0
        };

        serde_json::json!({
            "mode": mode.as_str(),
            "configured_mode": cfg.runtime.mode,
            "env_mode_override": env_mode,
            "mcp_profile": profile.as_str(),
            "configured_mcp_profile": cfg.runtime.mcp_profile,
            "env_profile_override": env_profile,
            "shadow_primary": cfg.runtime.shadow_primary,
            "shadow_max_live_ms": cfg.runtime.shadow_max_live_ms,
            "ab_sample_rate": cfg.runtime.ab_sample_rate,
            "mcp_tools_full_count": full_schemas.len(),
            "mcp_tools_active_count": active_schemas.len(),
            "schema_bytes_full": full_bytes,
            "schema_bytes_active": active_bytes,
            "schema_reduction_pct": (reduction_pct * 10.0).round() / 10.0,
        })
    }

    /// 模拟完全没有 agent-body 时的原生实时工作区搜索（受 `max_ms` 时间预算约束）
    pub fn run_live_workspace_search(
        roots: &[PathBuf],
        ignore_patterns: &[String],
        query: &str,
        limit: usize,
        max_ms: u64,
    ) -> (Vec<LiveAssetHit>, i64, i64) {
        let t0 = Instant::now();
        let budget = Duration::from_millis(max_ms.max(50));
        let terms: Vec<String> = query
            .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '|' | '/'))
            .map(|t| t.trim().to_ascii_lowercase())
            .filter(|t| t.len() >= 2)
            .collect();
        let fallback_term = query.trim().to_ascii_lowercase();
        let search_terms: Vec<String> = if terms.is_empty() && !fallback_term.is_empty() {
            vec![fallback_term]
        } else {
            terms
        };

        if search_terms.is_empty() || limit == 0 {
            let elapsed = t0.elapsed().as_micros() as i64;
            return (Vec::new(), elapsed, 0);
        }

        let mut candidates: Vec<LiveAssetHit> = Vec::new();
        let mut scanned_files = 0usize;
        const MAX_SCAN_FILES: usize = 4000;

        'outer: for root in roots {
            if !root.exists() {
                continue;
            }
            let mut it = WalkDir::new(root).into_iter();
            while let Some(entry_res) = it.next() {
                if t0.elapsed() >= budget || scanned_files >= MAX_SCAN_FILES {
                    break 'outer;
                }
                let entry = match entry_res {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let p = entry.path();
                if entry.file_type().is_dir() {
                    if ignore_patterns
                        .iter()
                        .any(|pat| path_matches_ignore_pattern(p, pat))
                    {
                        it.skip_current_dir();
                    }
                    continue;
                }
                if !entry.file_type().is_file() || is_sensitive_file(p) {
                    continue;
                }
                scanned_files += 1;

                let path_str = p.to_string_lossy().to_string();
                let path_lower = path_str.to_ascii_lowercase();
                let mut score = 0usize;
                for term in &search_terms {
                    if path_lower.contains(term) {
                        score += 2;
                    }
                }

                let mut snippet = String::new();
                if let Some(head) = read_head_utf8(p, 4096) {
                    let head_lower = head.to_ascii_lowercase();
                    let mut matched_line: Option<String> = None;
                    for term in &search_terms {
                        if head_lower.contains(term) {
                            score += 1;
                            if matched_line.is_none() {
                                matched_line = head
                                    .lines()
                                    .find(|l| l.to_ascii_lowercase().contains(term))
                                    .map(|l| SecretRedactor::redact_single_line(l.trim()));
                            }
                        }
                    }
                    if let Some(line) = matched_line {
                        snippet = line.chars().take(120).collect();
                    }
                }

                if score > 0 {
                    candidates.push(LiveAssetHit {
                        path: path_str,
                        snippet,
                        match_score: score,
                    });
                }
            }
        }

        candidates.sort_by(|a, b| {
            b.match_score
                .cmp(&a.match_score)
                .then_with(|| a.path.cmp(&b.path))
        });
        candidates.truncate(limit);

        let rendered = Self::format_live_hits_text(&candidates, query);
        let tokens = estimate_tokens(&rendered);
        let elapsed_us = t0.elapsed().as_micros() as i64;
        (candidates, elapsed_us, tokens)
    }

    pub fn format_live_hits_text(hits: &[LiveAssetHit], query: &str) -> String {
        let mut out = format!(
            "原生实时工作区搜索结果 (query='{}', 共 {} 条):\n",
            query,
            hits.len()
        );
        for h in hits {
            if h.snippet.is_empty() {
                out.push_str(&format!("  - {} (score={})\n", h.path, h.match_score));
            } else {
                out.push_str(&format!(
                    "  - {} (score={}) | {}\n",
                    h.path, h.match_score, h.snippet
                ));
            }
        }
        out
    }

    /// 执行一次完整的实时搜索 vs agent-body 索引对照，并将观察结果写入 `audit_events`
    pub fn compare_asset_search(
        storage: &Storage,
        cfg: &Config,
        query: &str,
        limit: usize,
        session_id: &str,
    ) -> Result<(ShadowAbComparison, Vec<LiveAssetHit>, Vec<AssetRecord>)> {
        let roots = cfg.effective_workspace_roots();
        let (live_hits, live_us, live_tokens) = Self::run_live_workspace_search(
            &roots,
            &cfg.assets.ignore_patterns,
            query,
            limit,
            cfg.runtime.shadow_max_live_ms,
        );

        let t_body = Instant::now();
        let body_hits = storage.search_assets_fts(query, limit)?;
        let body_us = t_body.elapsed().as_micros() as i64;

        let mut body_text = format!("资产搜索匹配结果 (共 {} 条):\n", body_hits.len());
        for a in &body_hits {
            body_text.push_str(&format_asset_line(a, DetailLevel::Standard));
            body_text.push('\n');
        }
        let body_tokens = if body_hits.is_empty() {
            0
        } else {
            estimate_tokens(&body_text)
        };

        let live_set: HashSet<String> = live_hits.iter().map(|h| h.path.clone()).collect();
        let body_set: HashSet<String> = body_hits.iter().map(|a| a.path.clone()).collect();
        let overlap_count = live_set.intersection(&body_set).count();
        let body_missed_count = live_set.difference(&body_set).count();
        let body_extra_count = body_set.difference(&live_set).count();
        let body_stale_count = body_hits
            .iter()
            .filter(|a| !Path::new(&a.path).exists())
            .count();

        let recall_vs_live_pct = if live_set.is_empty() {
            100.0
        } else {
            (overlap_count as f64 / live_set.len() as f64) * 100.0
        };
        let token_savings_pct = if live_tokens > 0 {
            ((live_tokens - body_tokens) as f64 / live_tokens as f64) * 100.0
        } else {
            0.0
        };

        let (verdict, reason, audit_outcome) = if body_stale_count > 0 {
            (
                "body_stale".to_string(),
                format!(
                    "agent-body 返回了 {} 条磁盘已不存在的陈旧路径 (stale hits)",
                    body_stale_count
                ),
                "failed",
            )
        } else if !live_hits.is_empty() && overlap_count == 0 && body_hits.is_empty() {
            (
                "body_regression".to_string(),
                format!(
                    "实时搜索命中 {} 条，但 agent-body FTS 返回 0 条（索引缺失或未同步）",
                    live_hits.len()
                ),
                "warn",
            )
        } else if body_tokens > live_tokens + 60 && overlap_count == 0 && body_extra_count == 0 {
            (
                "body_regression".to_string(),
                "agent-body 输出消耗更高 Token 且未产生有效重叠命中".to_string(),
                "warn",
            )
        } else if !body_hits.is_empty()
            && (body_us <= live_us || token_savings_pct >= 15.0 || body_extra_count > 0)
            && recall_vs_live_pct >= 50.0
        {
            (
                "body_win".to_string(),
                format!(
                    "耗时 {}μs vs 实时 {}μs，相对实时召回 {:.1}%，额外发现 {} 条",
                    body_us, live_us, recall_vs_live_pct, body_extra_count
                ),
                "success",
            )
        } else {
            (
                "tie".to_string(),
                format!(
                    "双侧表现接近 (live={}hits/{}μs, body={}hits/{}μs)",
                    live_hits.len(),
                    live_us,
                    body_hits.len(),
                    body_us
                ),
                "success",
            )
        };

        let cmp = ShadowAbComparison {
            query: SecretRedactor::redact_single_line(query),
            kind: "asset_search".to_string(),
            mode: cfg.effective_runtime_mode().as_str().to_string(),
            primary_served: cfg.runtime.shadow_primary.clone(),
            live_elapsed_us: live_us,
            body_elapsed_us: body_us,
            live_tokens,
            body_tokens,
            live_hit_count: live_hits.len(),
            body_hit_count: body_hits.len(),
            overlap_count,
            body_missed_count,
            body_extra_count,
            body_stale_count,
            recall_vs_live_pct: (recall_vs_live_pct * 10.0).round() / 10.0,
            token_savings_pct: (token_savings_pct * 10.0).round() / 10.0,
            verdict,
            reason,
            live_sample_paths: live_hits.iter().take(5).map(|h| h.path.clone()).collect(),
            body_sample_paths: body_hits.iter().take(5).map(|a| a.path.clone()).collect(),
        };

        let delta_json = serde_json::to_value(&cmp).unwrap_or_else(|_| serde_json::json!({}));
        AuditLogger::record_best_effort(
            storage,
            AuditEventInput::in_agent("ab_shadow", "ab_shadow", "shadow_compare")
                .with_session(session_id)
                .with_actor("shadow_ab_engine")
                .with_target("ab_query", query)
                .with_concurrency_mode("shadow_ab")
                .with_outcome(audit_outcome)
                .with_elapsed_us(body_us)
                .with_delta(delta_json),
        );

        Ok((cmp, live_hits, body_hits))
    }

    /// 记录一次 bypass 旁路直通事件，便于跨会话对照
    pub fn record_bypass_event(
        storage: &Storage,
        channel: &str,
        operation: &str,
        session_id: &str,
        target: &str,
    ) {
        AuditLogger::record_best_effort(
            storage,
            AuditEventInput::in_agent(channel, "ab_shadow", "bypass_passthrough")
                .with_session(session_id)
                .with_actor("runtime_bypass")
                .with_target(operation, target)
                .with_concurrency_mode("bypass")
                .with_outcome("success")
                .with_delta(serde_json::json!({
                    "kind": operation,
                    "mode": "bypass",
                    "channel": channel,
                })),
        );
    }

    /// 记录 `shadow_ab` 下 Pre-Hook 的影子干预判定（用于统计拦截收益与潜在误伤）
    pub fn record_hook_shadow(
        storage: &Storage,
        tool_name: &str,
        target: &str,
        would_block: bool,
        has_warnings: bool,
        elapsed_us: i64,
    ) {
        let verdict = if would_block || has_warnings {
            "body_win"
        } else {
            "tie"
        };
        let reason = if would_block {
            format!("Pre-Hook 捕获高危/阻断级风险 ({})", tool_name)
        } else if has_warnings {
            format!("Pre-Hook 发出前置冲突预警 ({})", tool_name)
        } else {
            format!("Pre-Hook 无冲突放行 ({})", tool_name)
        };
        let cmp = ShadowAbComparison {
            query: SecretRedactor::redact_single_line(target),
            kind: "hook_pre".to_string(),
            mode: "shadow_ab".to_string(),
            primary_served: "live".to_string(),
            live_elapsed_us: 0,
            body_elapsed_us: elapsed_us,
            live_tokens: 0,
            body_tokens: if has_warnings { 40 } else { 0 },
            live_hit_count: 0,
            body_hit_count: if has_warnings { 1 } else { 0 },
            overlap_count: 0,
            body_missed_count: 0,
            body_extra_count: if has_warnings { 1 } else { 0 },
            body_stale_count: 0,
            recall_vs_live_pct: 100.0,
            token_savings_pct: 0.0,
            verdict: verdict.to_string(),
            reason,
            live_sample_paths: vec![],
            body_sample_paths: if target.is_empty() {
                vec![]
            } else {
                vec![SecretRedactor::redact_single_line(target)]
            },
        };
        let delta_json = serde_json::to_value(&cmp).unwrap_or_else(|_| serde_json::json!({}));
        AuditLogger::record_best_effort(
            storage,
            AuditEventInput::in_agent("hook", "ab_shadow", "shadow_compare")
                .with_actor("shadow_ab_hook")
                .with_target("hook_tool", tool_name)
                .with_concurrency_mode("shadow_ab")
                .with_outcome(if would_block { "warn" } else { "success" })
                .with_elapsed_us(elapsed_us)
                .with_delta(delta_json),
        );
    }

    /// 聚合 `audit_events` 中所有 `ab_shadow` 观察记录，输出诚实的收益/回退评估报告
    pub fn build_ab_report(
        storage: &Storage,
        cfg: &Config,
        window: &str,
        limit: usize,
    ) -> Result<AbAnalysisReport> {
        let filter = AuditQueryFilter {
            category: Some("ab_shadow".to_string()),
            window: Some(window.to_string()),
            limit: limit.clamp(20, 2000),
            ..Default::default()
        };
        let events = storage.query_audit_events(&filter)?;

        let mut bypass_events = 0usize;
        let mut comparisons: Vec<ShadowAbComparison> = Vec::new();
        for ev in &events {
            if ev.action == "bypass_passthrough" {
                bypass_events += 1;
                continue;
            }
            if ev.action == "shadow_compare" {
                if let Ok(cmp) = serde_json::from_value::<ShadowAbComparison>(ev.before_after_delta.clone()) {
                    comparisons.push(cmp);
                }
            }
        }

        let shadow_comparisons = comparisons.len();
        let mut body_wins = 0usize;
        let mut ties = 0usize;
        let mut body_regressions = 0usize;
        let mut body_stale_incidents = 0usize;
        let mut sum_live_us = 0.0f64;
        let mut sum_body_us = 0.0f64;
        let mut sum_tok_save = 0.0f64;
        let mut sum_recall = 0.0f64;

        let mut groups: BTreeMap<String, Vec<&ShadowAbComparison>> = BTreeMap::new();
        let mut worst_regressions: Vec<ShadowAbComparison> = Vec::new();

        for cmp in &comparisons {
            match cmp.verdict.as_str() {
                "body_win" => body_wins += 1,
                "body_stale" => {
                    body_stale_incidents += 1;
                    worst_regressions.push(cmp.clone());
                }
                "body_regression" => {
                    body_regressions += 1;
                    worst_regressions.push(cmp.clone());
                }
                _ => ties += 1,
            }
            sum_live_us += cmp.live_elapsed_us as f64;
            sum_body_us += cmp.body_elapsed_us as f64;
            sum_tok_save += cmp.token_savings_pct;
            sum_recall += cmp.recall_vs_live_pct;
            groups.entry(cmp.kind.clone()).or_default().push(cmp);
        }

        let denom = shadow_comparisons.max(1) as f64;
        let win_rate_pct = if shadow_comparisons > 0 {
            (body_wins as f64 / denom) * 100.0
        } else {
            0.0
        };
        let negative_cnt = body_regressions + body_stale_incidents;
        let regression_rate_pct = if shadow_comparisons > 0 {
            (negative_cnt as f64 / denom) * 100.0
        } else {
            0.0
        };

        let mut by_kind = Vec::new();
        for (kind, items) in groups {
            let n = items.len().max(1) as f64;
            let wins = items.iter().filter(|c| c.verdict == "body_win").count();
            let k_ties = items.iter().filter(|c| c.verdict == "tie").count();
            let k_reg = items.iter().filter(|c| c.verdict == "body_regression").count();
            let k_stale = items.iter().filter(|c| c.verdict == "body_stale").count();
            by_kind.push(AbKindStats {
                kind,
                comparisons: items.len(),
                body_wins: wins,
                ties: k_ties,
                regressions: k_reg,
                stale_hits: k_stale,
                avg_live_us: items.iter().map(|c| c.live_elapsed_us as f64).sum::<f64>() / n,
                avg_body_us: items.iter().map(|c| c.body_elapsed_us as f64).sum::<f64>() / n,
                avg_token_savings_pct: items.iter().map(|c| c.token_savings_pct).sum::<f64>() / n,
                avg_recall_vs_live_pct: items.iter().map(|c| c.recall_vs_live_pct).sum::<f64>() / n,
            });
        }

        let (verdict, verdict_summary) = if shadow_comparisons < 3 {
            (
                "INSUFFICIENT_DATA".to_string(),
                format!(
                    "当前仅采集到 {} 次对照样本（至少建议 >= 3 次真实检索/任务后评估）",
                    shadow_comparisons
                ),
            )
        } else if regression_rate_pct >= 25.0 || body_stale_incidents > 0 && regression_rate_pct >= 15.0 {
            (
                "HURTING_INVESTIGATE".to_string(),
                format!(
                    "检测到 {:.1}% 负面回退/陈旧命中率（回退={}, 陈旧={}），建议排查索引新鲜度或暂切 shadow/bypass",
                    regression_rate_pct, body_regressions, body_stale_incidents
                ),
            )
        } else if win_rate_pct >= 50.0 && regression_rate_pct < 15.0 {
            (
                "NET_POSITIVE".to_string(),
                format!(
                    "真实对照净正收益：胜率 {:.1}%，回退+陈旧率 {:.1}%，平均相对实时召回 {:.1}%",
                    win_rate_pct,
                    regression_rate_pct,
                    sum_recall / denom
                ),
            )
        } else {
            (
                "MIXED_NEEDS_TUNING".to_string(),
                format!(
                    "收益与摩擦并存：胜率 {:.1}%，平局 {}，负向 {:.1}%",
                    win_rate_pct, ties, regression_rate_pct
                ),
            )
        };

        let mut recommendations = Vec::new();
        if body_stale_incidents > 0 {
            recommendations.push(
                "存在磁盘已删除但索引仍在召回的陈旧命中：建议立即执行 `agent-body feedback reconcile` 清理并核对 Watcher 监听目录。"
                    .to_string(),
            );
        }
        if body_regressions > 0 {
            recommendations.push(
                "存在实时搜索命中但 FTS 漏召回的场景：建议对工作区执行 `agent-body asset scan <dir>` 或为 `body_asset_search` 补齐向量混合检索。"
                    .to_string(),
            );
        }
        if cfg.effective_mcp_profile() == McpToolProfile::Full {
            recommendations.push(
                "当前 MCP Profile 为 `full`（43 工具全量 Schema）；建议切换 `mcp_profile = \"core\"` 削减 ~78% 工具定义上下文税。"
                    .to_string(),
            );
        }
        if recommendations.is_empty() {
            recommendations.push("当前 A/B 观测健康，可继续保持 `shadow_ab` 或切回 `body_only`。".to_string());
        }

        worst_regressions.truncate(10);

        Ok(AbAnalysisReport {
            window: window.to_string(),
            current_mode: cfg.effective_runtime_mode().as_str().to_string(),
            current_mcp_profile: cfg.effective_mcp_profile().as_str().to_string(),
            total_observations: events.len(),
            bypass_events,
            shadow_comparisons,
            body_wins,
            ties,
            body_regressions,
            body_stale_incidents,
            win_rate_pct: (win_rate_pct * 10.0).round() / 10.0,
            regression_rate_pct: (regression_rate_pct * 10.0).round() / 10.0,
            avg_live_us: if shadow_comparisons > 0 { sum_live_us / denom } else { 0.0 },
            avg_body_us: if shadow_comparisons > 0 { sum_body_us / denom } else { 0.0 },
            avg_token_savings_pct: if shadow_comparisons > 0 { sum_tok_save / denom } else { 0.0 },
            avg_recall_vs_live_pct: if shadow_comparisons > 0 { sum_recall / denom } else { 0.0 },
            verdict,
            verdict_summary,
            recommendations,
            by_kind,
            worst_regressions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config_with_root(root: &Path) -> Config {
        let mut cfg = Config::default();
        cfg.workspace.workspace_roots = vec![root.to_path_buf()];
        cfg.runtime.mode = "shadow_ab".to_string();
        cfg.runtime.shadow_primary = "live".to_string();
        cfg
    }

    #[test]
    fn test_shadow_ab_detects_win_stale_and_regression() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let work_dir = tmp.path().join("work");
        std::fs::create_dir_all(&work_dir).expect("mkdir");

        let real_file = work_dir.join("pg_pool_tuning.py");
        std::fs::write(&real_file, "# PostgreSQL pool connection failover guide\nprint('ok')\n")
            .expect("write real");

        let only_live_file = work_dir.join("duckdb_olap_notes.md");
        std::fs::write(&only_live_file, "# DuckDB OLAP columnar analytics pipeline\n")
            .expect("write live only");

        let storage = Storage::open_in_memory().expect("open_in_memory");
        let cfg = test_config_with_root(&work_dir);

        // 1. 正常命中场景：real_file 已入库，应当判定 body_win 或 tie（无 stale）
        storage
            .upsert_asset(&AssetRecord {
                id: "asset-real-1".to_string(),
                path: real_file.to_string_lossy().to_string(),
                asset_type: "script".to_string(),
                owner: "user:test".to_string(),
                security_level: "internal".to_string(),
                scan_tier: "deep".to_string(),
                description: Some("PostgreSQL pool connection failover guide".to_string()),
                desc_kv: serde_json::json!({"purpose": "PostgreSQL pool failover"}),
                usage: None,
                size_bytes: Some(60),
                file_count: None,
                tags: vec!["pg".to_string()],
                status: "active".to_string(),
            })
            .expect("upsert real");

        let (cmp_ok, live_ok, body_ok) =
            ShadowAbEngine::compare_asset_search(&storage, &cfg, "failover", 5, "sess-ab-1")
                .expect("compare win");
        assert!(!live_ok.is_empty());
        assert!(!body_ok.is_empty());
        assert_eq!(cmp_ok.body_stale_count, 0);
        assert_eq!(cmp_ok.overlap_count, 1);
        assert!(matches!(cmp_ok.verdict.as_str(), "body_win" | "tie"));

        // 2. 漏召回场景：only_live_file 未入库 -> 应诚实标为 body_regression
        let (cmp_reg, _, _) =
            ShadowAbEngine::compare_asset_search(&storage, &cfg, "columnar", 5, "sess-ab-1")
                .expect("compare regression");
        assert_eq!(cmp_reg.verdict, "body_regression");
        assert_eq!(cmp_reg.body_missed_count, 1);

        // 3. 陈旧幽灵命中场景：库中有已删文件路径 -> 应标为 body_stale
        let ghost_path = work_dir.join("deleted_ghost_module.rs");
        storage
            .upsert_asset(&AssetRecord {
                id: "asset-ghost-1".to_string(),
                path: ghost_path.to_string_lossy().to_string(),
                asset_type: "script".to_string(),
                owner: "user:test".to_string(),
                security_level: "internal".to_string(),
                scan_tier: "deep".to_string(),
                description: Some("ghost_quantum_cache deleted".to_string()),
                desc_kv: serde_json::json!({}),
                usage: None,
                size_bytes: Some(10),
                file_count: None,
                tags: vec![],
                status: "active".to_string(),
            })
            .expect("upsert ghost");
        let (cmp_stale, _, _) =
            ShadowAbEngine::compare_asset_search(&storage, &cfg, "ghost_quantum_cache", 5, "sess-ab-1")
                .expect("compare stale");
        assert_eq!(cmp_stale.verdict, "body_stale");
        assert_eq!(cmp_stale.body_stale_count, 1);

        // 4. Bypass 记账 + 汇总报表验证
        ShadowAbEngine::record_bypass_event(&storage, "mcp", "body_brief", "sess-ab-2", "bypass");
        let rep = ShadowAbEngine::build_ab_report(&storage, &cfg, "7d", 100).expect("ab report");
        assert_eq!(rep.bypass_events, 1);
        assert_eq!(rep.shadow_comparisons, 3);
        assert_eq!(rep.body_regressions, 1);
        assert_eq!(rep.body_stale_incidents, 1);
        assert_eq!(rep.verdict, "HURTING_INVESTIGATE");
        assert!(rep.recommendations.iter().any(|r| r.contains("reconcile")));
    }
}
