use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use std::time::{Instant, UNIX_EPOCH};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::brief::{BriefAssembler, BriefRequest};
use crate::fact::process_net::ProcessNetTopology;
use crate::feedback::FeedbackEngine;
use crate::links::LinksEngine;
use crate::memory::builtin_provider::BuiltinMemoryProvider;
use crate::scanner::metadata::{merge_with_existing_asset, HeuristicMetadataExtractor};
use crate::storage::models::AssetRecord;
use crate::storage::Storage;

/// 全生命周期信息新鲜度与现实对账报告 (ISSUE #21)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FreshnessReconcileReport {
    pub checked_assets: usize,
    pub missing_assets_marked: usize,
    pub drifted_assets_refreshed: usize,
    pub ghost_fact_nodes_pruned: usize,
    pub duplicate_memories_archived: usize,
    pub stale_codegraph_repos: Vec<String>,
    pub elapsed_us: u128,
}

/// 单个执行回路对比探针读数 (ISSUE #22)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparativeProbeReading {
    pub probe_id: String,
    pub hook_stage: String,
    pub metric_name: String,
    pub comparator_baseline: String,
    pub measured_value: String,
    pub delta_summary: String,
    pub healthy: bool,
    pub healed_action: String,
    pub recommendation: String,
}

/// 五点对比探针自进化总览报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeEvolutionReport {
    pub timestamp: String,
    pub auto_heal_applied: bool,
    pub reconcile_report: Option<FreshnessReconcileReport>,
    pub probes: Vec<ComparativeProbeReading>,
    pub feedback_items_created: Vec<String>,
    pub overall_healthy: bool,
    pub summary: String,
}

pub struct FreshnessReconciler;

impl FreshnessReconciler {
    /// 执行全生命周期信息新鲜度对账（资产改删写穿 + 幽灵节点剪枝 + 同路径记忆去重 + CodeGraph 时效检查）
    pub fn reconcile(storage: &Storage) -> Result<FreshnessReconcileReport> {
        let _wait_guard = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::IoDiskWalk);
        let t0 = Instant::now();
        let mut rep = FreshnessReconcileReport::default();

        // 1. 磁盘物理状态 vs 库内 assets 对账
        let assets = storage.list_assets(25_000)?;
        let links_engine = LinksEngine::new(Arc::new(storage.clone()));
        let active_memories = storage.list_all_active_memories(1000).unwrap_or_default();
        let mut archived_mem_ids: HashSet<String> = HashSet::new();

        for asset in assets {
            if asset.status != "active" {
                continue;
            }
            if asset.path.starts_with("bench:") || asset.path.starts_with("demo:") {
                continue;
            }
            rep.checked_assets += 1;
            let path = Path::new(&asset.path);

            if !path.exists() {
                let mut missing_rec = asset.clone();
                missing_rec.status = "missing".to_string();
                if let Err(e) = storage.upsert_asset(&missing_rec) {
                    eprintln!("[Reconcile][WARN] 标记失活资产失败 ({}): {}", asset.path, e);
                }
                if let Err(e) = storage.delete_auto_links_for_source(&asset.path) {
                    eprintln!("[Reconcile][WARN] 清理失活资产源边失败 ({}): {}", asset.path, e);
                }
                if let Err(e) = storage.delete_links_for_target(&asset.path) {
                    eprintln!("[Reconcile][WARN] 清理失活资产目标边失败 ({}): {}", asset.path, e);
                }
                let pruned = crate::fact::KnowledgeGraphBuilder::invalidate_source_path(
                    storage.provider().as_ref(),
                    &asset.path,
                )
                .unwrap_or(0);
                rep.ghost_fact_nodes_pruned += pruned;

                let marker = format!("产出文件 `{}` 摘要:", asset.path);
                for m in &active_memories {
                    if m.content.contains(&marker) && archived_mem_ids.insert(m.id.clone()) {
                        if let Err(e) = storage.update_memory_status(&m.id, "archived") {
                            eprintln!("[Reconcile][WARN] 归档失活资产记忆失败 ({}): {}", m.id, e);
                        } else {
                            rep.duplicate_memories_archived += 1;
                        }
                    }
                }
                rep.missing_assets_marked += 1;
                continue;
            }

            if path.is_file() && asset.scan_tier == "deep" {
                let Ok(meta) = std::fs::metadata(path) else {
                    continue;
                };
                let disk_size = meta.len() as i64;
                let disk_mtime = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs());
                let db_mtime = asset.desc_kv.get("mtime_secs").and_then(|v| v.as_u64());
                let size_drifted = asset.size_bytes.map(|s| s != disk_size).unwrap_or(false);
                let mtime_drifted = match (db_mtime, disk_mtime) {
                    (Some(old_m), Some(new_m)) => new_m > old_m,
                    _ => false,
                };

                if size_drifted || mtime_drifted {
                    let extracted = HeuristicMetadataExtractor::extract(
                        path,
                        &asset.asset_type,
                        &asset.scan_tier,
                        None,
                        Some(disk_size),
                    );
                    let fresh_rec = AssetRecord {
                        id: asset.id.clone(),
                        path: asset.path.clone(),
                        asset_type: asset.asset_type.clone(),
                        owner: asset.owner.clone(),
                        security_level: asset.security_level.clone(),
                        scan_tier: asset.scan_tier.clone(),
                        description: Some(extracted.description),
                        desc_kv: extracted.desc_kv,
                        usage: extracted.usage,
                        size_bytes: Some(disk_size),
                        file_count: asset.file_count,
                        tags: asset.tags.clone(),
                        status: "active".to_string(),
                    };
                    let merged = merge_with_existing_asset(fresh_rec, Some(&asset));
                    let became_stale = merged
                        .desc_kv
                        .get("embedding_status")
                        .and_then(|v| v.as_str())
                        == Some("stale");
                    if let Err(e) = storage.upsert_asset(&merged) {
                        eprintln!("[Reconcile][WARN] 回写漂移资产失败 ({}): {}", asset.path, e);
                    }
                    if size_drifted || became_stale {
                        if let Err(e) = links_engine.refresh_links_for_asset(path) {
                            eprintln!("[Reconcile][WARN] 刷新漂移资产链接失败 ({}): {}", asset.path, e);
                        }
                        if let Err(e) = crate::fact::KnowledgeGraphBuilder::invalidate_source_path(
                            storage.provider().as_ref(),
                            &asset.path,
                        ) {
                            eprintln!("[Reconcile][WARN] 清理漂移资产衍生知识图节点失败 ({}): {}", asset.path, e);
                        }
                        rep.drifted_assets_refreshed += 1;
                    }
                }
            }
        }

        // 2. 属性图幽灵节点剪枝（已关闭 port / 失踪 file / 失踪 project）
        let live_port_ids: HashSet<String> = ProcessNetTopology::scan_listening_ports()
            .map(|ports| ports.into_iter().map(|p| format!("port:{}", p.port)).collect())
            .unwrap_or_default();

        let fact_nodes = storage.list_all_fact_nodes(2000).unwrap_or_default();
        for node in &fact_nodes {
            if node.id.contains("bench:") || node.id.contains("demo:") {
                continue;
            }
            match node.node_type.as_str() {
                "port" if !live_port_ids.is_empty() && !live_port_ids.contains(&node.id) => {
                    if storage.delete_fact_node(&node.id).unwrap_or(0) > 0 {
                        rep.ghost_fact_nodes_pruned += 1;
                    }
                }
                "file" => {
                    if let Some(p) = node.id.strip_prefix("file:") {
                        if p.starts_with('/') && !Path::new(p).exists()
                            && storage.delete_fact_node(&node.id).unwrap_or(0) > 0 {
                                rep.ghost_fact_nodes_pruned += 1;
                            }
                    }
                }
                "project" => {
                    let canon = node
                        .properties
                        .get("canonical_path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if canon.starts_with('/') && !Path::new(canon).exists()
                        && storage.delete_fact_node(&node.id).unwrap_or(0) > 0 {
                            rep.ghost_fact_nodes_pruned += 1;
                        }
                    // 检查 .codegraph/codegraph.db 是否滞后于源码修改时间
                    if !canon.is_empty() && is_codegraph_stale(Path::new(canon)) {
                        rep.stale_codegraph_repos.push(canon.to_string());
                    }
                }
                _ => {}
            }
        }

        // 3. 长期记忆中同路径 auto_sync 摘要记忆幂等去重归档（active_memories 已按 updated_at DESC 排序，保留首条最新，归档其余旧版）
        let mut seen_auto_paths: HashSet<String> = HashSet::new();
        for m in &active_memories {
            if archived_mem_ids.contains(&m.id) {
                continue;
            }
            if let Some(file_path) = extract_auto_sync_path(&m.content) {
                if (!Path::new(&file_path).exists() || !seen_auto_paths.insert(file_path))
                    && archived_mem_ids.insert(m.id.clone()) {
                        if let Err(e) = storage.update_memory_status(&m.id, "archived") {
                            eprintln!("[Reconcile][WARN] 归档重复记忆失败 ({}): {}", m.id, e);
                        } else {
                            rep.duplicate_memories_archived += 1;
                        }
                    }
            }
        }

        drop(active_memories);
        drop(fact_nodes);
        let _ = storage.provider().get_benchmark_run("awr:pragma_optimize");
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            unsafe extern "C" {
                fn malloc_trim(pad: usize) -> i32;
            }
            unsafe {
                let _ = malloc_trim(0);
            }
        }

        rep.elapsed_us = t0.elapsed().as_micros();
        crate::audit::AuditLogger::record_best_effort(
            storage,
            crate::audit::AuditEventInput::out_of_agent(
                "daemon_scheduler",
                "reconciler",
                "freshness_reconcile",
                "reconcile",
            )
            .with_target("host", "body.db")
            .with_outcome("success")
            .with_elapsed_us(rep.elapsed_us as i64)
            .with_delta(serde_json::json!({
                "checked_assets": rep.checked_assets,
                "missing_assets_marked": rep.missing_assets_marked,
                "drifted_assets_refreshed": rep.drifted_assets_refreshed,
                "ghost_fact_nodes_pruned": rep.ghost_fact_nodes_pruned,
                "duplicate_memories_archived": rep.duplicate_memories_archived,
                "stale_codegraph_repos": rep.stale_codegraph_repos,
            })),
        );
        Ok(rep)
    }
}

pub struct SelfEvolutionProbeEngine;

impl SelfEvolutionProbeEngine {
    /// 在 Agent 执行回路 5 个黄金切面运行确定性对比探针，并按需触发自动自愈与去重反馈沉淀
    pub fn run_probes(
        storage: &Storage,
        fb_path: &Path,
        auto_heal: bool,
        auto_feedback: bool,
    ) -> Result<ProbeEvolutionReport> {
        let mut probes = Vec::with_capacity(5);
        let mut feedback_items_created = Vec::new();

        // 预先采集自愈前后的事实状态，用于对比 Δ
        let pre_stats = collect_staleness_snapshot(storage);
        let reconcile_report = if auto_heal {
            Some(FreshnessReconciler::reconcile(storage)?)
        } else {
            None
        };
        let post_stats = collect_staleness_snapshot(storage);

        // P1: 开工认知探针 (BriefAssembler)
        let provider = Arc::new(BuiltinMemoryProvider::new(storage.clone()));
        let assembler = BriefAssembler::new(storage.clone(), Some(provider));
        let t_brief = Instant::now();
        let brief_out = assembler.assemble(&BriefRequest {
            query: "agent steward".to_string(),
            session_id: String::new(),
            budget_tokens: 1500,
        })?;
        let brief_us = t_brief.elapsed().as_micros();
        let dup_ratio = if post_stats.total_auto_sync_mems == 0 {
            100.0
        } else {
            ((post_stats.unique_auto_sync_paths as f64 / post_stats.total_auto_sync_mems as f64)
                * 100.0
                * 10.0)
                .round()
                / 10.0
        };
        let lease_cnt = brief_out
            .optimistic_leases
            .iter()
            .filter(|l| l.confidence >= 0.85)
            .count();
        let miss_cnt = storage
            .list_changes(100)
            .unwrap_or_default()
            .into_iter()
            .filter(|c| c.change_type == "optimistic_miss")
            .count();

        let p1_healthy = brief_us <= 25_000 && dup_ratio >= 85.0 && !brief_out.markdown.is_empty();
        probes.push(ComparativeProbeReading {
            probe_id: "P1".into(),
            hook_stage: "SessionStart(Brief)".into(),
            metric_name: "简报组装延迟、记忆去重与开工乐观租约签发".into(),
            comparator_baseline: "SLA <= 25,000 μs & 记忆唯一率 >= 85%".into(),
            measured_value: format!("{} μs · 唯一率 {:.1}% · 乐观租约 {} 条", brief_us, dup_ratio, lease_cnt),
            delta_summary: format!(
                "简报体积 {} tok · 重复记忆从 {} -> {} 条 · 有效租约={} (碰壁降级记录={})",
                brief_out.tokens_used,
                pre_stats.duplicate_auto_sync_count,
                post_stats.duplicate_auto_sync_count,
                lease_cnt,
                miss_cnt
            ),
            healthy: p1_healthy,
            healed_action: if auto_heal && pre_stats.duplicate_auto_sync_count > 0 {
                format!("已自动归档 {} 条重复旧摘要记忆", pre_stats.duplicate_auto_sync_count)
            } else {
                "保持 MVCC 乐观直通契约按需签发与自动降级".into()
            },
            recommendation: if p1_healthy {
                "保持 BriefAssembler 索引优先路径与高置信乐观租约签发".into()
            } else {
                "建议排查 Brief 慢路径或运行 `agent-body feedback reconcile`".into()
            },
        });

        // P2: 事前预测探针 (PreToolUse + 效能账本 + 双域审计)
        let ledger = storage
            .query_effectiveness_ledger("7d", None, 50)
            .unwrap_or_default();
        let audit_sum = storage.audit_domain_summary("7d").unwrap_or_default();
        let p2_healthy = true;
        probes.push(ComparativeProbeReading {
            probe_id: "P2".into(),
            hook_stage: "PreToolUse(Guard)".into(),
            metric_name: "7日事前风险拦截、规范路由与双域行为审计".into(),
            comparator_baseline: "零护航基线 (0 次拦截 / 0 Token 节省 / 0 审计)".into(),
            measured_value: format!(
                "{} 次ROI调用 · 拦截 {} 次 · 省 {} tok · 双域审计 {} 条 (IN={} / OUT={})",
                ledger.total_events,
                ledger.total_hazards_intercepted,
                ledger.total_tokens_saved,
                audit_sum.total_events,
                audit_sum.in_agent_count,
                audit_sum.out_of_agent_count
            ),
            delta_summary: format!(
                "+{} 次冲突预警 · 累计省 {} 轮探索 · 自愈 {} 次",
                ledger.total_hazards_intercepted, ledger.total_rounds_saved, audit_sum.recovered_count
            ),
            healthy: p2_healthy,
            healed_action: "持续经由 PR-01~PR-12 双账本带出处留痕与三态 AOPCC 路由".into(),
            recommendation: "若发现高频违规目录可运行 `agent-body audit evolve` 或 `place learn` 自动演化规则".into(),
        });

        // P3: 检索效用探针 (FTS 僵尸泄漏与陈旧向量)
        let fts_hits = storage.search_assets_fts("agent", 30).unwrap_or_default();
        let zombie_leaks = fts_hits.iter().filter(|a| a.status != "active").count();
        let stale_vecs = fts_hits
            .iter()
            .filter(|a| a.desc_kv.get("embedding_status").and_then(|v| v.as_str()) == Some("stale"))
            .count();
        let p3_healthy = zombie_leaks == 0 && stale_vecs == 0;
        probes.push(ComparativeProbeReading {
            probe_id: "P3".into(),
            hook_stage: "InFlight(Search)".into(),
            metric_name: "检索结果僵尸泄漏与陈旧向量检测".into(),
            comparator_baseline: "zombie_leak == 0 & stale_vec == 0".into(),
            measured_value: format!("zombie={}, stale_vec={}", zombie_leaks, stale_vecs),
            delta_summary: if p3_healthy {
                "0 僵尸召回 · 向量无过期".into()
            } else {
                format!("需通过 backfill_embeddings.py 补齐 {} 条 stale 向量", stale_vecs)
            },
            healthy: p3_healthy,
            healed_action: if auto_heal {
                "search_assets_fts 已强制过滤 status='active'".into()
            } else {
                "仅只读探查".into()
            },
            recommendation: if stale_vecs > 0 {
                "运行 `python3 scripts/backfill_embeddings.py` 重算变更文件向量".into()
            } else {
                "检索新鲜度健康".into()
            },
        });

        // P4: 事后漂移探针 (DB Snapshot vs Disk Reality & AOPCC 碰壁自愈)
        let p4_healthy = post_stats.drifted_sample_count == 0 && post_stats.missing_active_sample_count == 0;
        probes.push(ComparativeProbeReading {
            probe_id: "P4".into(),
            hook_stage: "PostToolUse(Drift)".into(),
            metric_name: "库内资产快照漂移率与 AOPCC 碰壁自愈追踪".into(),
            comparator_baseline: "drifted == 0 & missing_active == 0".into(),
            measured_value: format!(
                "抽检 {} 条: drifted={}, missing_active={}, optimistic_miss={}",
                post_stats.sampled_assets, post_stats.drifted_sample_count, post_stats.missing_active_sample_count, miss_cnt
            ),
            delta_summary: format!(
                "对账前 drifted={}, missing={} -> 对账后 drifted={}, missing={} (碰壁降级 {})",
                pre_stats.drifted_sample_count,
                pre_stats.missing_active_sample_count,
                post_stats.drifted_sample_count,
                post_stats.missing_active_sample_count,
                miss_cnt
            ),
            healthy: p4_healthy,
            healed_action: if let Some(ref rr) = reconcile_report {
                format!(
                    "刷新 {} 个漂移资产，标记 {} 个失踪资产（已跟踪 {} 次乐观碰壁降级）",
                    rr.drifted_assets_refreshed, rr.missing_assets_marked, miss_cnt
                )
            } else {
                "仅只读探测".into()
            },
            recommendation: if p4_healthy {
                "库内快照与磁盘物理状态完全一致".into()
            } else {
                "执行 `agent-body feedback reconcile` 修复磁盘漂移".into()
            },
        });

        // P5: 周期图谱与 Benchmark 回归探针
        let stale_cg_count = reconcile_report
            .as_ref()
            .map(|r| r.stale_codegraph_repos.len())
            .unwrap_or(0);
        let bench_delta = compare_latest_two_benchmarks(storage);
        let p5_healthy = post_stats.ghost_port_count == 0 && !bench_delta.regressed;
        probes.push(ComparativeProbeReading {
            probe_id: "P5".into(),
            hook_stage: "Periodic(Regression)".into(),
            metric_name: "幽灵端口剪枝、CodeGraph 新鲜度与 Benchmark 跨批对比".into(),
            comparator_baseline: "ghost_ports == 0 & Benchmark 无退化".into(),
            measured_value: format!(
                "ghost_ports={}, stale_codegraph={}, {}",
                post_stats.ghost_port_count, stale_cg_count, bench_delta.summary
            ),
            delta_summary: format!(
                "幽灵端口 {} -> {} ({})",
                pre_stats.ghost_port_count, post_stats.ghost_port_count, bench_delta.detail
            ),
            healthy: p5_healthy,
            healed_action: if let Some(ref rr) = reconcile_report {
                format!("已剪枝 {} 个幽灵 fact_nodes", rr.ghost_fact_nodes_pruned)
            } else {
                "未启用自动剪枝".into()
            },
            recommendation: if stale_cg_count > 0 {
                format!("存在 {} 个仓 .codegraph 滞后，建议运行 `codegraph sync`", stale_cg_count)
            } else {
                "属性图与性能回归基线稳定".into()
            },
        });

        if auto_feedback {
            for p in &probes {
                if !p.healthy {
                    let title = format!("[Probe:{}] {}异常", p.probe_id, p.metric_name);
                    let evidence = format!(
                        "阶段: {} | 参照: {} | 实测: {} | 差值: {}",
                        p.hook_stage, p.comparator_baseline, p.measured_value, p.delta_summary
                    );
                    if let Ok((item, created)) = FeedbackEngine::add_item_deduped(
                        fb_path,
                        "CON",
                        "P1",
                        &title,
                        &evidence,
                        &p.recommendation,
                    ) {
                        if created {
                            feedback_items_created.push(item.id);
                        }
                    }
                }
            }
        }

        let overall_healthy = probes.iter().all(|p| p.healthy);
        let summary = format!(
            "五点对比探针完成: {}/5 健康{} | 新生成去重反馈 {} 条",
            probes.iter().filter(|p| p.healthy).count(),
            if overall_healthy { " ✅" } else { " ⚠️" },
            feedback_items_created.len()
        );

        Ok(ProbeEvolutionReport {
            timestamp: chrono::Local::now().to_rfc3339(),
            auto_heal_applied: auto_heal,
            reconcile_report,
            probes,
            feedback_items_created,
            overall_healthy,
            summary,
        })
    }
}

#[derive(Default)]
struct StalenessSnapshot {
    sampled_assets: usize,
    drifted_sample_count: usize,
    missing_active_sample_count: usize,
    ghost_port_count: usize,
    total_auto_sync_mems: usize,
    unique_auto_sync_paths: usize,
    duplicate_auto_sync_count: usize,
}

fn collect_staleness_snapshot(storage: &Storage) -> StalenessSnapshot {
    let mut snap = StalenessSnapshot::default();
    let assets = storage.list_assets(2000).unwrap_or_default();
    for a in assets.into_iter().filter(|a| a.status == "active").take(1000) {
        if a.path.starts_with("bench:") || a.path.starts_with("demo:") {
            continue;
        }
        snap.sampled_assets += 1;
        let p = Path::new(&a.path);
        if !p.exists() {
            snap.missing_active_sample_count += 1;
        } else if p.is_file() && a.scan_tier == "deep" {
            if let Ok(meta) = std::fs::metadata(p) {
                if a.size_bytes.map(|s| s != meta.len() as i64).unwrap_or(false) {
                    snap.drifted_sample_count += 1;
                }
            }
        }
    }

    let live_ports: HashSet<String> = ProcessNetTopology::scan_listening_ports()
        .map(|ps| ps.into_iter().map(|p| format!("port:{}", p.port)).collect())
        .unwrap_or_default();
    if !live_ports.is_empty() {
        for n in storage.list_all_fact_nodes(1000).unwrap_or_default() {
            if n.node_type == "port"
                && !n.id.contains("bench:")
                && !n.id.contains("demo:")
                && !live_ports.contains(&n.id)
            {
                snap.ghost_port_count += 1;
            }
        }
    }

    let mut unique_paths = HashSet::new();
    for m in storage.list_all_active_memories(500).unwrap_or_default() {
        if let Some(fp) = extract_auto_sync_path(&m.content) {
            snap.total_auto_sync_mems += 1;
            unique_paths.insert(fp);
        }
    }
    snap.unique_auto_sync_paths = unique_paths.len();
    snap.duplicate_auto_sync_count = snap
        .total_auto_sync_mems
        .saturating_sub(snap.unique_auto_sync_paths);
    snap
}

struct BenchDeltaSummary {
    regressed: bool,
    summary: String,
    detail: String,
}

fn compare_latest_two_benchmarks(storage: &Storage) -> BenchDeltaSummary {
    let runs = storage.list_benchmark_runs(2).unwrap_or_default();
    if runs.len() < 2 {
        return BenchDeltaSummary {
            regressed: false,
            summary: format!("历史 Benchmark 批次数={}", runs.len()),
            detail: "待累积 >=2 次全量基准批次后生成跨版本 Δ".into(),
        };
    }
    let curr = &runs[0];
    let prev = &runs[1];
    let tok_delta = curr.overall_token_savings_pct - prev.overall_token_savings_pct;
    let speed_delta = curr.overall_speedup_factor - prev.overall_speedup_factor;
    let regressed = !curr.sla_passed || tok_delta < -5.0;
    BenchDeltaSummary {
        regressed,
        summary: format!(
            "Token省率 {:.2}% (Δ{:+.2}%), 加速比 {:.2}x (Δ{:+.2}x)",
            curr.overall_token_savings_pct, tok_delta, curr.overall_speedup_factor, speed_delta
        ),
        detail: format!("对比批次 {} vs {}", prev.run_id, curr.run_id),
    }
}

fn extract_auto_sync_path(content: &str) -> Option<String> {
    let prefix = "产出文件 `";
    let start = content.find(prefix)? + prefix.len();
    let end_rel = content[start..].find("` 摘要:")?;
    Some(content[start..start + end_rel].to_string())
}

fn is_codegraph_stale(repo_root: &Path) -> bool {
    let db_path = repo_root.join(".codegraph/codegraph.db");
    let Ok(db_mtime) = std::fs::metadata(&db_path).and_then(|m| m.modified()) else {
        return false;
    };
    let src_dir = repo_root.join("src");
    let scan_root = if src_dir.is_dir() { &src_dir } else { repo_root };
    for entry in WalkDir::new(scan_root)
        .max_depth(4)
        .into_iter()
        .filter_map(|e| e.ok())
        .take(200)
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        if p.to_string_lossy().contains("/target/") || p.to_string_lossy().contains("/.codegraph/") {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            if let Ok(mtime) = meta.modified() {
                if let Ok(diff) = mtime.duration_since(db_mtime) {
                    if diff.as_secs() > 5 {
                        return true;
                    }
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::{FactNodeRecord, MemoryRecord};

    #[test]
    fn test_freshness_reconciler_heals_drifted_missing_ghost_and_duplicate_memories() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let dir = tempfile::tempdir().expect("tempdir");
        let live_file = dir.path().join("live.py");
        std::fs::write(&live_file, "print('v1')\n").unwrap();
        let live_str = live_file.to_string_lossy().to_string();

        // 1. 写入尺寸漂移且带旧向量的 active 资产
        storage
            .upsert_asset(&AssetRecord {
                id: "a-live".into(),
                path: live_str.clone(),
                asset_type: "script".into(),
                owner: "user:digoal".into(),
                security_level: "internal".into(),
                scan_tier: "deep".into(),
                description: Some("old v1".into()),
                desc_kv: serde_json::json!({"content_hash": "oldhash", "annotation_status": "ollama_enriched", "embedding_status": "ready"}),
                usage: None,
                size_bytes: Some(12),
                file_count: None,
                tags: vec![],
                status: "active".into(),
            })
            .unwrap();
        storage.update_asset_embedding(&live_str, &[0.5, 0.5]).unwrap();

        // 2. 写入物理已删但库内仍为 active 的僵尸资产
        let dead_str = dir.path().join("deleted.py").to_string_lossy().to_string();
        storage
            .upsert_asset(&AssetRecord {
                id: "a-dead".into(),
                path: dead_str.clone(),
                asset_type: "script".into(),
                owner: "user:digoal".into(),
                security_level: "internal".into(),
                scan_tier: "deep".into(),
                description: Some("ghost_dead_script".into()),
                desc_kv: serde_json::json!({}),
                usage: None,
                size_bytes: Some(20),
                file_count: None,
                tags: vec![],
                status: "active".into(),
            })
            .unwrap();

        // 3. 写入幽灵文件节点与 2 条同路径重复摘要记忆
        storage
            .upsert_fact_node(&FactNodeRecord {
                id: format!("file:{}", dead_str),
                node_type: "file".into(),
                name: "deleted.py".into(),
                label: "deleted".into(),
                properties: serde_json::json!({}),
                updated_at: None,
            })
            .unwrap();
        for idx in 1..=2 {
            storage
                .insert_memory(&MemoryRecord {
                    id: format!("mem-dup-{}", idx),
                    memory_type: "note".into(),
                    content: format!("产出文件 `{}` 摘要:\nversion {}", live_str, idx),
                    summary: None,
                    source_agent: None,
                    source_session: None,
                    tags: vec![],
                    weight: 0.7,
                    pinned: false,
                    status: "active".into(),
                    embedding: vec![],
                })
                .unwrap();
        }

        // 修改 live_file 内容使尺寸漂移
        std::fs::write(&live_file, "def new_entry_v2():\n    return 'much longer v2 content'\n").unwrap();

        let rep = FreshnessReconciler::reconcile(&storage).expect("reconcile");
        assert_eq!(rep.drifted_assets_refreshed, 1);
        assert_eq!(rep.missing_assets_marked, 1);
        assert!(rep.ghost_fact_nodes_pruned >= 1);
        assert_eq!(rep.duplicate_memories_archived, 1);

        let updated = storage.get_asset_by_path(&live_str).unwrap().unwrap();
        assert_eq!(updated.desc_kv["embedding_status"], "stale");
        assert_eq!(updated.desc_kv["annotation_status"], "needs_re_enrich");
        assert!(storage.get_asset_embedding(&live_str).unwrap().is_empty());
        assert!(storage.search_assets_fts("ghost_dead_script", 10).unwrap().is_empty());
        assert_eq!(storage.list_all_active_memories(10).unwrap().len(), 1);
    }

    #[test]
    fn test_self_evolution_probes_and_deduped_feedback() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let dir = tempfile::tempdir().expect("tempdir");
        let fb = dir.path().join("user_feedback.md");
        std::fs::write(&fb, "# 反馈总账\n\n## 七、第五部分：后续新增反馈标准模板\n").unwrap();

        let (item1, created1) = FeedbackEngine::add_item_deduped(
            &fb,
            "CON",
            "P1",
            "[Probe:P3] 检索结果存在陈旧向量",
            "stale_vec=3",
            "运行 backfill",
        )
        .unwrap();
        assert!(created1);
        let (item2, created2) = FeedbackEngine::add_item_deduped(
            &fb,
            "CON",
            "P1",
            "[Probe:P3] 检索结果存在陈旧向量",
            "stale_vec=5",
            "运行 backfill",
        )
        .unwrap();
        assert!(!created2, "同一未闭环探针问题不应重复刷屏写入");
        assert_eq!(item1.id, item2.id);

        let probe_rep = SelfEvolutionProbeEngine::run_probes(&storage, &fb, true, false).expect("probes");
        assert_eq!(probe_rep.probes.len(), 5);
    }
}
