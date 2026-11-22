//! 开工乐观直通租约引擎（AOPCC — MVCC Read-Snapshot Lease）
//!
//! 在 `SessionStart (body_brief)` 阶段以 <1ms 开销从持久化事实图谱与本地轻量探针
//! 动态提取高置信度服务/工具/沙盒/CodeGraph 乐观假设租约，使 Agent 对高稳定、可逆操作
//! 免除执行回路内的强制前置查询（消除整轮对话税 `C_turn`）。
//! 当近期 `PostToolUse` 捕获 `optimistic_miss` 碰壁事件时，对应目标的置信度自动降级，
//! 不再签发乐观免查推荐。

use std::collections::HashSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::benchmark::provenance::sha256_hex;
use crate::brief::assembler::estimate_tokens;
use crate::fact::{CapabilityEngine, FactFacade};
use crate::storage::models::{ChangeRecord, FactNodeRecord};
use crate::storage::Storage;

/// 单条乐观直通租约（类比数据库 MVCC Read-Snapshot 短租约）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OptimisticLease {
    pub lease_id: String,
    /// `db_service` | `ai_runtime` | `cli_toolchain` | `temp_sandbox` | `codegraph_index`
    pub category: String,
    pub target: String,
    /// 先验有效概率 P_valid (0.00 ~ 1.00)
    pub confidence: f32,
    /// 基于活跃事实节点与碰壁记录计算的短纪元指纹
    pub epoch_hash: String,
    pub direct_hint: String,
    pub fallback_hint: String,
}

/// 开工乐观租约汇总报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptimisticLeaseReport {
    /// 全量评估后的租约列表（含因 optimistic_miss 被降级的条目）
    pub leases: Vec<OptimisticLease>,
    pub epoch_hash: String,
    pub markdown_section: String,
    pub tokens_estimated: usize,
}

pub struct OptimisticLeaseEngine;

impl OptimisticLeaseEngine {
    /// 签发开工乐观直通租约（严控 Markdown 输出 <= 120 tokens）
    pub fn issue_leases(storage: &Storage, query: &str) -> OptimisticLeaseReport {
        let all_nodes = storage.list_all_fact_nodes(500).unwrap_or_default();
        let changes = storage.list_changes(100).unwrap_or_default();
        let mut downgraded_targets = collect_downgraded_targets(&changes);
        let audit_adjustments =
            crate::audit::evolution::AuditEvolutionAnalyzer::collect_audit_lease_adjustments(
                storage,
            );
        for (target, eval) in &audit_adjustments {
            if eval.should_downgrade {
                downgraded_targets.insert(target.clone());
            } else if eval.rehabilitated {
                downgraded_targets.remove(target);
            }
        }

        let port_nodes: Vec<&FactNodeRecord> = all_nodes
            .iter()
            .filter(|n| n.node_type == "port")
            .collect();
        let tool_nodes: Vec<&FactNodeRecord> = all_nodes
            .iter()
            .filter(|n| n.node_type == "tool")
            .collect();
        let proj_nodes: Vec<&FactNodeRecord> = all_nodes
            .iter()
            .filter(|n| n.node_type == "project")
            .collect();

        // 计算短纪元指纹（8 位十六进制）
        let mut fingerprint_seed = String::new();
        for n in &port_nodes {
            fingerprint_seed.push_str(&n.id);
            fingerprint_seed.push(';');
        }
        for t in &downgraded_targets {
            fingerprint_seed.push_str("miss:");
            fingerprint_seed.push_str(t);
            fingerprint_seed.push(';');
        }
        if fingerprint_seed.is_empty() {
            fingerprint_seed.push_str("host-default");
        }
        let epoch_hash: String = sha256_hex(&fingerprint_seed).chars().take(8).collect();

        let mut candidates: Vec<OptimisticLease> = Vec::new();
        let q_lower = query.trim().to_lowercase();

        // 1. 服务端口类租约 (db_service / ai_runtime)
        if !port_nodes.is_empty() {
            for n in port_nodes {
                let port = n
                    .properties
                    .get("port")
                    .and_then(|v| v.as_u64())
                    .map(|p| p as u16)
                    .or_else(|| n.name.trim_start_matches("port:").parse::<u16>().ok());
                let Some(port) = port else {
                    continue;
                };
                let proc_name = n
                    .properties
                    .get("process_name")
                    .or_else(|| n.properties.get("process"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("service");
                if is_system_noise_port(port, proc_name) {
                    continue;
                }
                candidates.push(build_port_lease(
                    port,
                    proc_name,
                    &epoch_hash,
                    &downgraded_targets,
                ));
            }
        } else if let Ok(live_ports) = FactFacade::query_ports() {
            for p in live_ports {
                let proc_name = p.process_name.as_deref().unwrap_or("service");
                if p.is_system_daemon || is_system_noise_port(p.port, proc_name) {
                    continue;
                }
                candidates.push(build_port_lease(
                    p.port,
                    proc_name,
                    &epoch_hash,
                    &downgraded_targets,
                ));
            }
        }

        // 2. 临时验证沙盒租约 (temp_sandbox)
        let tmp_dir = crate::config::Config::load_effective_or_default().effective_temp_dir();
        let tmp_prefix = format!("{}/", tmp_dir.to_string_lossy().trim_end_matches('/'));
        let sandbox_key = format!("sandbox:{}", tmp_prefix);
        let sandbox_downgraded = downgraded_targets.contains(&sandbox_key)
            || downgraded_targets.contains(&tmp_prefix);
        candidates.push(OptimisticLease {
            lease_id: "lease-tmp-sandbox".to_string(),
            category: "temp_sandbox".to_string(),
            target: tmp_prefix.clone(),
            confidence: if sandbox_downgraded { 0.55 } else { 0.99 },
            epoch_hash: epoch_hash.clone(),
            direct_hint: format!("临时验证直写 {} 免前置放置查询", tmp_prefix),
            fallback_hint: "若写入正式产物，交由 Pre-Hook 静默规约校正直达规范目录".to_string(),
        });

        // 3. 工具链租约 (cli_toolchain)
        let ready_tools = collect_ready_tools(&tool_nodes, &q_lower);
        if !ready_tools.is_empty() {
            let tool_missed = ready_tools.iter().any(|t| {
                downgraded_targets.contains(&format!("tool:{}", t))
                    || downgraded_targets.contains(t)
            });
            let tool_list = ready_tools
                .iter()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .join(",");
            candidates.push(OptimisticLease {
                lease_id: "lease-cli-toolchain".to_string(),
                category: "cli_toolchain".to_string(),
                target: tool_list,
                confidence: if tool_missed { 0.55 } else { 0.96 },
                epoch_hash: epoch_hash.clone(),
                direct_hint: "已就绪 CLI 直接执行，免前置 which 探测".to_string(),
                fallback_hint: "若报 command not found，Post-Hook <2ms 推荐宿主机替代二进制".to_string(),
            });
        }

        // 4. CodeGraph 索引租约 (codegraph_index)
        let cg_repo = find_matching_codegraph_repo(&proj_nodes, &q_lower);
        if let Some(repo_name) = cg_repo {
            let cg_missed = downgraded_targets.contains(&format!("codegraph:{}", repo_name));
            candidates.push(OptimisticLease {
                lease_id: "lease-codegraph".to_string(),
                category: "codegraph_index".to_string(),
                target: format!("{}/.codegraph", repo_name),
                confidence: if cg_missed { 0.55 } else { 0.95 },
                epoch_hash: epoch_hash.clone(),
                direct_hint: "已具 .codegraph，直接 codegraph explore 免 grep".to_string(),
                fallback_hint: "若索引过期由 FreshnessReconciler 提示重建".to_string(),
            });
        }

        // 按优先级与置信度排序（高置信度且带查询亲和力的优先展示）
        candidates.sort_by(|a, b| {
            let a_boost = query_affinity_boost(&a.target, &q_lower);
            let b_boost = query_affinity_boost(&b.target, &q_lower);
            let a_score = a.confidence + a_boost;
            let b_score = b.confidence + b_boost;
            b_score
                .partial_cmp(&a_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // 生成严格控制在 <= 120 tokens 的精简 Markdown 区块（仅含 confidence >= 0.85 的生效租约）
        let markdown_section = format_leases_markdown(&candidates, 120);
        let tokens_estimated = if markdown_section.is_empty() {
            0
        } else {
            estimate_tokens(&markdown_section)
        };

        OptimisticLeaseReport {
            leases: candidates,
            epoch_hash,
            markdown_section,
            tokens_estimated,
        }
    }
}

fn collect_downgraded_targets(changes: &[ChangeRecord]) -> HashSet<String> {
    let cutoff = chrono::Utc::now() - chrono::Duration::hours(24);
    let mut out = HashSet::new();
    for c in changes {
        if c.change_type != "optimistic_miss" {
            continue;
        }
        let recent = match c.created_at.as_deref() {
            None => true,
            Some(ts) => {
                if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts) {
                    dt.with_timezone(&chrono::Utc) >= cutoff
                } else if let Ok(ndt) = chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S") {
                    chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(ndt, chrono::Utc) >= cutoff
                } else {
                    true
                }
            }
        };
        if recent && !c.target.trim().is_empty() {
            out.insert(c.target.trim().to_string());
        }
    }
    out
}

fn is_system_noise_port(port: u16, proc_name: &str) -> bool {
    let p_lower = proc_name.to_lowercase();
    matches!(
        p_lower.as_str(),
        "sshd" | "systemd-resolved" | "systemd-timesyn" | "chronyd" | "cupsd" | "rpcbind"
    ) || port == 22
}

fn build_port_lease(
    port: u16,
    proc_name: &str,
    epoch_hash: &str,
    downgraded_targets: &HashSet<String>,
) -> OptimisticLease {
    let port_key = format!("port:{}", port);
    let proc_lower = proc_name.to_lowercase();
    let (category, label, direct_hint) = if port == 5432 || proc_lower.contains("postgres") {
        (
            "db_service",
            format!("postgresql:{}", port),
            format!("活跃监听，客户端直连(psql -p {})免前置查询", port),
        )
    } else if port == 11434 || proc_lower.contains("ollama") {
        (
            "ai_runtime",
            format!("ollama:{}", port),
            format!("活跃推理服务，直连 localhost:{} 免前置探测", port),
        )
    } else if port == 6379 || proc_lower.contains("redis") {
        (
            "db_service",
            format!("redis:{}", port),
            format!("活跃缓存，直连 redis-cli -p {} 免前置查询", port),
        )
    } else if port == 3306 || proc_lower.contains("mysql") || proc_lower.contains("mariadbd") {
        (
            "db_service",
            format!("mysql:{}", port),
            format!("活跃数据库，直连 mysql -P {} 免前置查询", port),
        )
    } else {
        (
            "ai_runtime",
            format!("{}:{}", proc_name, port),
            format!("本机活跃服务(:{})，客户端直连免前置查询", port),
        )
    };

    let is_downgraded = downgraded_targets.contains(&port_key)
        || downgraded_targets.contains(&label)
        || downgraded_targets.contains(&port.to_string());

    OptimisticLease {
        lease_id: format!("lease-port-{}", port),
        category: category.to_string(),
        target: label,
        confidence: if is_downgraded { 0.55 } else { 0.95 },
        epoch_hash: epoch_hash.to_string(),
        direct_hint,
        fallback_hint: "失败由 Post-Hook <2ms 反查真实端口与启停建议".to_string(),
    }
}

fn collect_ready_tools(tool_nodes: &[&FactNodeRecord], q_lower: &str) -> Vec<String> {
    if !tool_nodes.is_empty() {
        let mut names: Vec<String> = tool_nodes.iter().map(|n| n.name.clone()).collect();
        names.sort_by_key(|name| {
            let nl = name.to_lowercase();
            let hit = !q_lower.is_empty() && (q_lower.contains(&nl) || nl.contains(q_lower));
            (!hit, name.clone())
        });
        return names.into_iter().take(4).collect();
    }

    let mut engine = CapabilityEngine::new();
    engine.scan_host_paths_only();
    let probe_q = if q_lower.is_empty() { "json" } else { q_lower };
    let res = engine.query(probe_q);
    let mut ready: Vec<String> = res
        .all_matching_tools
        .into_iter()
        .filter(|t| t.installed)
        .map(|t| t.tool_name)
        .take(4)
        .collect();
    if ready.is_empty() {
        for bin in ["git", "python3", "cargo", "sqlite3"] {
            if is_binary_in_path(bin) {
                ready.push(bin.to_string());
            }
        }
    }
    ready
}

fn is_binary_in_path(bin: &str) -> bool {
    let path_var = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path_var).any(|dir| dir.join(bin).is_file())
}

fn find_matching_codegraph_repo(proj_nodes: &[&FactNodeRecord], q_lower: &str) -> Option<String> {
    let mut best: Option<(usize, String)> = None;
    for n in proj_nodes {
        let path_str = n
            .properties
            .get("canonical_path")
            .or_else(|| n.properties.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or(&n.name);
        let ready = n
            .properties
            .get("codegraph_ready")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
            || Path::new(path_str).join(".codegraph").exists();
        if !ready {
            continue;
        }
        let nl = n.name.to_lowercase();
        let score = if !q_lower.is_empty() && (q_lower.contains(&nl) || nl.contains(q_lower)) {
            2
        } else {
            1
        };
        match &best {
            Some((best_s, _)) if *best_s >= score => {}
            _ => best = Some((score, n.name.clone())),
        }
    }
    best.map(|(_, name)| name)
}

fn query_affinity_boost(target: &str, q_lower: &str) -> f32 {
    if q_lower.is_empty() {
        return 0.0;
    }
    let tl = target.to_lowercase();
    if tl.contains(q_lower)
        || (q_lower.contains("pg") && tl.contains("postgres"))
        || (q_lower.contains("postgres") && tl.contains("5432"))
        || (q_lower.contains("ollama") && tl.contains("11434"))
    {
        0.10
    } else {
        0.0
    }
}

fn format_leases_markdown(leases: &[OptimisticLease], max_tokens: usize) -> String {
    let active: Vec<&OptimisticLease> = leases.iter().filter(|l| l.confidence >= 0.85).collect();
    if active.is_empty() {
        return String::new();
    }

    let header = "## ⚡ 乐观直通契约 (Optimistic Leases)\n".to_string();
    let mut out = header.clone();
    let mut added = 0usize;

    for lease in active.iter().take(4) {
        let line = format!(
            "- **[{} | p={:.2} | {}]** `{}` → {}（碰壁由 Post-Hook <2ms 自愈）\n",
            lease.lease_id, lease.confidence, lease.epoch_hash, lease.target, lease.direct_hint
        );
        let candidate = format!("{}{}", out, line);
        if estimate_tokens(&candidate) <= max_tokens {
            out = candidate;
            added += 1;
        } else if added == 0 {
            // 极端紧凑回退一行，保证至少输出 1 条租约且严格 <= max_tokens
            let compact = format!(
                "- **[{} p={:.2}]** `{}`: {}\n",
                lease.lease_id,
                lease.confidence,
                lease.target,
                lease.direct_hint.chars().take(24).collect::<String>()
            );
            let compact_candidate = format!("{}{}", out, compact);
            if estimate_tokens(&compact_candidate) <= max_tokens {
                out = compact_candidate;
                added += 1;
            }
            break;
        } else {
            break;
        }
    }

    if added == 0 {
        String::new()
    } else {
        out.push('\n');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{ChangeRecord, FactNodeRecord};

    #[test]
    fn test_issue_leases_and_token_budget() {
        let storage = Storage::open_in_memory().expect("in-memory");
        storage
            .upsert_fact_node(&FactNodeRecord {
                id: "port:5432".to_string(),
                node_type: "port".to_string(),
                name: "port:5432".to_string(),
                label: "postgres".to_string(),
                properties: serde_json::json!({"port": 5432, "process_name": "postgres"}),
                updated_at: None,
            })
            .expect("upsert port");
        storage
            .upsert_fact_node(&FactNodeRecord {
                id: "tool:psql".to_string(),
                node_type: "tool".to_string(),
                name: "psql".to_string(),
                label: "PostgreSQL CLI".to_string(),
                properties: serde_json::json!({}),
                updated_at: None,
            })
            .expect("upsert tool");

        let report = OptimisticLeaseEngine::issue_leases(&storage, "postgres 5432");
        assert!(
            report
                .leases
                .iter()
                .any(|l| l.target == "postgresql:5432" && l.confidence >= 0.90),
            "应签发 postgresql:5432 高置信度乐观租约"
        );
        assert!(
            report
                .leases
                .iter()
                .any(|l| l.category == "temp_sandbox" && l.confidence >= 0.95),
            "应签发 /root/new/tmp/ 沙盒租约"
        );
        assert!(
            report.tokens_estimated <= 120,
            "乐观租约 Markdown 不得超过 120 tokens，实际: {}",
            report.tokens_estimated
        );
        assert!(report.markdown_section.contains("postgresql:5432"));
    }

    #[test]
    fn test_optimistic_miss_downgrades_lease_confidence() {
        let storage = Storage::open_in_memory().expect("in-memory");
        storage
            .upsert_fact_node(&FactNodeRecord {
                id: "port:5432".to_string(),
                node_type: "port".to_string(),
                name: "port:5432".to_string(),
                label: "postgres".to_string(),
                properties: serde_json::json!({"port": 5432, "process_name": "postgres"}),
                updated_at: None,
            })
            .expect("upsert port");

        // 注入一条针对 port:5432 的 optimistic_miss 碰壁降级事件
        storage
            .record_change(&ChangeRecord {
                id: None,
                change_type: "optimistic_miss".to_string(),
                target: "port:5432".to_string(),
                description: Some("psql -p 5432 ECONNREFUSED".to_string()),
                details: serde_json::json!({"failure_kind": "CONNECTION_REFUSED"}),
                source: "hook_post".to_string(),
                created_at: None,
            })
            .expect("record optimistic_miss");

        let report = OptimisticLeaseEngine::issue_leases(&storage, "postgres 5432");
        let pg_lease = report
            .leases
            .iter()
            .find(|l| l.lease_id == "lease-port-5432")
            .expect("应仍包含被降级租约状态");
        assert!(
            pg_lease.confidence < 0.85,
            "存在 optimistic_miss 后置信度必须自动降级 < 0.85，实际: {}",
            pg_lease.confidence
        );
        assert!(
            !report.markdown_section.contains("postgresql:5432"),
            "被降级租约不得继续作为高置信度直通契约输出"
        );
    }

    #[test]
    fn test_optimistic_lease_rehabilitates_after_audit_recovery() {
        let storage = Storage::open_in_memory().expect("in-memory");
        storage
            .upsert_fact_node(&FactNodeRecord {
                id: "port:5432".to_string(),
                node_type: "port".to_string(),
                name: "port:5432".to_string(),
                label: "postgres".to_string(),
                properties: serde_json::json!({"port": 5432, "process_name": "postgres"}),
                updated_at: None,
            })
            .expect("upsert port");

        // 1. 先记录一次审计碰壁
        crate::audit::AuditLogger::record(
            &storage,
            crate::audit::AuditEventInput::in_agent("hook", "hook_post_recovery", "run_command")
                .with_target("tool_target", "port:5432")
                .with_outcome("recovered"),
        )
        .unwrap();
        let rep1 = OptimisticLeaseEngine::issue_leases(&storage, "postgres");
        let l1 = rep1.leases.iter().find(|l| l.lease_id == "lease-port-5432").unwrap();
        assert!(l1.confidence < 0.85, "碰壁后应降级");

        // 2. 连续两次成功直连后，租约自动康复
        for _ in 0..2 {
            crate::audit::AuditLogger::record(
                &storage,
                crate::audit::AuditEventInput::in_agent("hook", "hook_post_exec", "run_command")
                    .with_target("tool_target", "port:5432")
                    .with_outcome("success"),
            )
            .unwrap();
        }
        let rep2 = OptimisticLeaseEngine::issue_leases(&storage, "postgres");
        let l2 = rep2.leases.iter().find(|l| l.lease_id == "lease-port-5432").unwrap();
        assert!(l2.confidence >= 0.85, "连续康复后应回升 >= 0.85，实际 {}", l2.confidence);
    }
}
