use anyhow::Result;
use agent_body::config::Config;
use agent_body::storage::Storage;
use crate::cli_args::*;

pub fn handle_governance(cmd: Commands, cfg: &Config, storage: Storage) -> Result<()> {
    match cmd {
        Commands::Bench { action } => {
            use agent_body::benchmark::ledger::EffectivenessLedger;
            use agent_body::benchmark::provenance::ProvenanceRule;
            use agent_body::benchmark::report::{
                render_csv_report, render_markdown_report, BenchmarkReportPayload,
            };
            use agent_body::benchmark::workloads::list_workload_specs;
            use agent_body::benchmark::BenchmarkEngine;

            match action {
                BenchCommands::Workloads => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "workloads": list_workload_specs(),
                            "provenance_rules": ProvenanceRule::list_all(),
                        }))?
                    );
                }
                BenchCommands::Run { suite, save } => {
                    let report = BenchmarkEngine::run_suite(
                        storage.provider().clone(),
                        &suite,
                        save,
                        None,
                    )?;
                    println!("{}", render_markdown_report(&report)?);
                }
                BenchCommands::Report { run_id, format } => {
                    let rec_opt = if let Some(id) = run_id {
                        storage.get_benchmark_run(&id)?
                    } else {
                        storage.list_benchmark_runs(1)?.into_iter().next()
                    };
                    let Some(rec) = rec_opt else {
                        println!("尚无基准评测记录，请先运行 `agent-body bench run`");
                        return Ok(());
                    };
                    let payload = BenchmarkReportPayload::from_record(&rec);
                    match format.as_str() {
                        "json" => println!("{}", serde_json::to_string_pretty(&payload)?),
                        "csv" => println!("{}", render_csv_report(&payload)?),
                        _ => println!("{}", render_markdown_report(&payload)?),
                    }
                }
                BenchCommands::Ledger { window, tool, limit } => {
                    let summary = storage.query_effectiveness_ledger(&window, tool.as_deref(), limit)?;
                    println!("{}", serde_json::to_string_pretty(&summary)?);
                }
                BenchCommands::Compare { run_a, run_b } => {
                    let a = storage.get_benchmark_run(&run_a)?
                        .ok_or_else(|| anyhow::anyhow!("未找到基准批次: {}", run_a))?;
                    let b = storage.get_benchmark_run(&run_b)?
                        .ok_or_else(|| anyhow::anyhow!("未找到基准批次: {}", run_b))?;
                    let diff = EffectivenessLedger::compare_runs(&a, &b);
                    println!("{}", serde_json::to_string_pretty(&diff)?);
                }
                BenchCommands::Export { format, out } => {
                    let rec = storage.list_benchmark_runs(1)?
                        .into_iter()
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("尚无基准评测记录可导出"))?;
                    let payload = BenchmarkReportPayload::from_record(&rec);
                    let content = match format.as_str() {
                        "json" => serde_json::to_string_pretty(&payload)?,
                        "csv" => render_csv_report(&payload)?,
                        _ => render_markdown_report(&payload)?,
                    };
                    if let Some(path) = out {
                        if let Some(parent) = std::path::Path::new(&path).parent() {
                            if !parent.as_os_str().is_empty() {
                                std::fs::create_dir_all(parent)?;
                            }
                        }
                        std::fs::write(&path, &content)?;
                        println!("✅ 已导出实证基准报告至: {} (SHA-256: {})", path, payload.report_sha256);
                    } else {
                        println!("{}", content);
                    }
                }
                BenchCommands::Reset => {
                    let cleaned = BenchmarkEngine::reset_sandbox(storage.provider().as_ref())?;
                    println!("✅ 已清理 {} 条 bench:* 沙盒状态", cleaned);
                }
                BenchCommands::Competitors { track, format } => {
                    let suite = if track.eq_ignore_ascii_case("all") {
                        "competitors".to_string()
                    } else if track.to_ascii_uppercase().starts_with("COMP-") {
                        track
                    } else {
                        format!("COMP-{}", track.to_ascii_uppercase())
                    };
                    let report = BenchmarkEngine::run_suite(
                        storage.provider().clone(),
                        &suite,
                        false,
                        Some("bench:comp"),
                    )?;
                    match format.as_str() {
                        "json" => println!("{}", serde_json::to_string_pretty(&report)?),
                        "csv" => println!("{}", render_csv_report(&report)?),
                        _ => println!("{}", render_markdown_report(&report)?),
                    }
                }
            }
        }

        Commands::Feedback { action } => {
            let t0 = std::time::Instant::now();
            let fb_path = agent_body::feedback::FeedbackEngine::default_feedback_path();
            match action {
                FeedbackCommands::List => {
                    let items = agent_body::feedback::FeedbackEngine::list_items(&fb_path)?;
                    println!("📋 反馈总账 ({}, 共 {} 条):", fb_path.display(), items.len());
                    for it in &items {
                        println!("  - [{}][{}] {} ({})", it.id, it.priority, it.title, it.status);
                    }
                }
                FeedbackCommands::Add { category, priority, title, evidence, target } => {
                    let item = agent_body::feedback::FeedbackEngine::add_item(
                        &fb_path, &category, &priority, &title, &evidence, &target,
                    )?;
                    println!("✅ 已写入反馈 {}: [{}] {}", item.id, item.priority, item.title);
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_feedback",
                        &item.title,
                        t0.elapsed().as_micros() as i64,
                    );
                }
                FeedbackCommands::Sync => {
                    let provider_ref = storage.provider().clone();
                    let added = agent_body::feedback::FeedbackEngine::sync_verified_to_memories(
                        &fb_path, &storage,
                    )?;
                    println!("✅ 已将 {} 条已验证反馈沉淀至长期记忆库 (memories)", added);
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        provider_ref.as_ref(),
                        "cli",
                        "body_feedback",
                        &format!("synced {} items", added),
                        t0.elapsed().as_micros() as i64,
                    );
                }
                FeedbackCommands::Reconcile { json } => {
                    let rep = agent_body::feedback::probes::FreshnessReconciler::reconcile(&storage)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rep)?);
                    } else {
                        println!(
                            "🧹 全生命周期新鲜度对账完成 ({} μs): 检查资产 {} | 标记失活 {} | 内容漂移失效写穿 {} | 剪枝幽灵节点 {} | 归档重复记忆 {} | 过期 CodeGraph 仓库 {:?}",
                            rep.elapsed_us,
                            rep.checked_assets,
                            rep.missing_assets_marked,
                            rep.drifted_assets_refreshed,
                            rep.ghost_fact_nodes_pruned,
                            rep.duplicate_memories_archived,
                            rep.stale_codegraph_repos
                        );
                    }
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_feedback",
                        &format!(
                            "reconcile missing={} drifted={} ghost={} dup_mem={}",
                            rep.missing_assets_marked,
                            rep.drifted_assets_refreshed,
                            rep.ghost_fact_nodes_pruned,
                            rep.duplicate_memories_archived
                        ),
                        t0.elapsed().as_micros() as i64,
                    );
                }
                FeedbackCommands::Probes { auto_heal, auto_feedback, json } => {
                    let rep = agent_body::feedback::probes::SelfEvolutionProbeEngine::run_probes(
                        &storage,
                        &fb_path,
                        auto_heal,
                        auto_feedback,
                    )?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rep)?);
                    } else {
                        println!("🔬 {}", rep.summary);
                        for p in &rep.probes {
                            println!(
                                "  {} [{}] {}: {} (基线: {}) | Δ: {} | 自愈: {} | {}",
                                if p.healthy { "✅" } else { "⚠️" },
                                p.probe_id,
                                p.hook_stage,
                                p.measured_value,
                                p.comparator_baseline,
                                p.delta_summary,
                                p.healed_action,
                                p.recommendation
                            );
                        }
                        if !rep.feedback_items_created.is_empty() {
                            println!("📝 本次自动去重写入反馈: {:?}", rep.feedback_items_created);
                        }
                    }
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_feedback",
                        &format!("probes healthy={} auto_heal={}", rep.overall_healthy, auto_heal),
                        t0.elapsed().as_micros() as i64,
                    );
                }
            }
        }

        Commands::Audit { action } => {
            use agent_body::audit::AuditEvolutionAnalyzer;
            use agent_body::storage::duckdb_analytics::{AnalyticsProvider, DuckDbAnalyticsEngine};
            use agent_body::storage::models::AuditQueryFilter;

            match action {
                AuditCommands::Query {
                    domain,
                    actor_kind,
                    category,
                    outcome,
                    trace_id,
                    target,
                    window,
                    limit,
                    json,
                } => {
                    let rows = storage.query_audit_events(&AuditQueryFilter {
                        domain,
                        actor_kind,
                        category,
                        action: None,
                        outcome,
                        trace_id,
                        session_id: None,
                        target_contains: target,
                        window: Some(window),
                        limit,
                    })?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rows)?);
                    } else {
                        println!("📜 双域行为审计明细 (共 {} 条):", rows.len());
                        for r in &rows {
                            println!(
                                "  [{}] {} | {} | {}.{} | outcome={} | target={} | trace={}",
                                r.created_at,
                                r.domain,
                                r.actor_kind,
                                r.category,
                                r.action,
                                r.outcome,
                                r.target_id,
                                r.trace_id
                            );
                        }
                    }
                }
                AuditCommands::Summary { window, days, json } => {
                    let summary = storage.audit_domain_summary(&window)?;
                    let olap = DuckDbAnalyticsEngine::from_synced_audit_events(storage.provider().clone())
                        .and_then(|eng| eng.audit_analytics_summary(days))
                        .ok();
                    if json {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "summary": summary,
                                "olap": olap,
                            }))?
                        );
                    } else {
                        println!(
                            "📊 双域行为审计汇总 ({}): 总计 {} 条 | IN_AGENT={} | OUT_OF_AGENT={} | 异常拦截={} | 自愈={}",
                            summary.window,
                            summary.total_events,
                            summary.in_agent_count,
                            summary.out_of_agent_count,
                            summary.failed_or_blocked_count,
                            summary.recovered_count
                        );
                        if let Some(o) = olap {
                            for top in o.top_actions.iter().take(10) {
                                println!(
                                    "  - [{}] {}.{} ({}) -> {} 次 (avg {:.0} μs)",
                                    top.domain, top.category, top.action, top.outcome, top.hit_count, top.avg_elapsed_us
                                );
                            }
                        }
                    }
                }
                AuditCommands::Trace { trace_id, json } => {
                    let rows = storage.query_audit_events(&AuditQueryFilter {
                        trace_id: Some(trace_id.clone()),
                        limit: 100,
                        ..Default::default()
                    })?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rows)?);
                    } else {
                        println!("🔗 Trace `{}` 因果链路 ({} 条):", trace_id, rows.len());
                        for r in &rows {
                            println!(
                                "  [{}] {} | {}.{} | outcome={} | delta={}",
                                r.created_at, r.domain, r.category, r.action, r.outcome, r.before_after_delta
                            );
                        }
                    }
                }
                AuditCommands::Evolve {
                    auto_feedback,
                    min_hits,
                    json,
                } => {
                    let fb_path = agent_body::feedback::FeedbackEngine::default_feedback_path();
                    let rep = AuditEvolutionAnalyzer::run_evolution_cycle(
                        &storage,
                        Some(&fb_path),
                        auto_feedback,
                        min_hits,
                    )?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rep)?);
                    } else {
                        println!(
                            "🧬 审计驱动自进化完成: 评估 {} 条 | 租约降级 {:?} | 租约康复 {:?} | 摩擦热点 {} | 新增反馈 {:?} | 目录规则候选 {}",
                            rep.evaluated_events,
                            rep.downgraded_targets,
                            rep.rehabilitated_targets,
                            rep.friction_hotspots.len(),
                            rep.feedback_items_created,
                            rep.placement_candidates.len()
                        );
                    }
                }
                AuditCommands::Export { output_path, json } => {
                    let resolved_out = output_path.unwrap_or_else(|| {
                        cfg.effective_temp_dir()
                            .join("audit_events_export.csv")
                            .to_string_lossy()
                            .to_string()
                    });
                    let eng = DuckDbAnalyticsEngine::from_synced_audit_events(storage.provider().clone())?;
                    let exp = eng.export_to_parquet("audit_events", &resolved_out)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&exp)?);
                    } else {
                        println!(
                            "📦 已导出 audit_events ({} 行, {} 字节) 至 {}",
                            exp.row_count, exp.file_size_bytes, exp.output_path
                        );
                    }
                }
            }
        }

        Commands::Body { action } => {
            use agent_body::body::{
                AutonomyEnvelope, BodyManifestEngine, ContinuityInput, ContinuityManager,
                HomeostasisVitalsEngine,
            };
            let t0 = std::time::Instant::now();
            match action {
                BodyCommands::Manifest { json } => {
                    let m = BodyManifestEngine::build(&storage);
                    let out = serde_json::to_string_pretty(&m)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!("🧬 Agent 本体身份证: {} ({}, {} / {})", m.body_id, m.hostname, m.os, m.arch);
                        println!("📦 活跃接力胶囊: {} | 六大器官状态:", m.active_checkpoints);
                        for o in &m.organs {
                            println!("  - {:12} [{}] {}", o.name, o.status, o.label);
                        }
                    }
                }
                BodyCommands::Vitals { json } => {
                    let v = HomeostasisVitalsEngine::assess(&storage);
                    let out = serde_json::to_string_pretty(&v)?;
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "🩺 五维本体生命体征 [{}]: 综合稳态分 {:.1}/100 ({})",
                            v.body_id, v.overall_score, v.status
                        );
                        for d in &v.dimensions {
                            println!(
                                "  - {:10} ({:>2}%): {:>5.1} [{}] {}",
                                d.label, d.weight_pct, d.score, d.status, d.detail
                            );
                        }
                        if !v.recommended_l1_actions.is_empty() {
                            println!("💡 建议 L1 自愈动作: {}", v.recommended_l1_actions.join("；"));
                        }
                    }
                }
                BodyCommands::Envelope { action: chk_act, target, dry_run, json } => {
                    if let Some(act) = chk_act {
                        let dec = AutonomyEnvelope::evaluate(&act, target.as_deref(), dry_run);
                        if json {
                            println!("{}", serde_json::to_string_pretty(&dec)?);
                        } else {
                            println!(
                                "🛡️ [{}] allowed={} confirm={} — {}",
                                dec.level_code, dec.allowed, dec.requires_confirmation, dec.reason
                            );
                        }
                    } else {
                        let mat = AutonomyEnvelope::policy_matrix();
                        if json {
                            println!("{}", serde_json::to_string_pretty(&mat)?);
                        } else {
                            for rule in &mat {
                                println!("• {} ({}) — {}", rule.level, rule.label, rule.authorization);
                            }
                        }
                    }
                }
                BodyCommands::Handoff {
                    goal,
                    from_brain,
                    session_id,
                    files,
                    facts,
                    blockers,
                    next_step,
                    json,
                } => {
                    let split_csv = |opt: Option<String>| -> Vec<String> {
                        opt.unwrap_or_default()
                            .split(',')
                            .map(|s| s.trim().to_string())
                            .filter(|s| !s.is_empty())
                            .collect()
                    };
                    let cp = ContinuityManager::save_capsule(
                        &storage,
                        ContinuityInput {
                            from_brain,
                            session_id,
                            goal_summary: goal,
                            active_files: split_csv(files),
                            active_locks: Vec::new(),
                            verified_facts: split_csv(facts),
                            open_blockers: split_csv(blockers),
                            next_step_hint: next_step,
                            token_budget: Some(300),
                        },
                    )?;
                    let out = serde_json::to_string_pretty(&cp)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "🧬 已保存跨大脑接力胶囊 `{}` (来自 `{}` · SHA256={}...)",
                            cp.checkpoint_id,
                            cp.from_brain,
                            cp.evidence_sha256.chars().take(12).collect::<String>()
                        );
                    }
                }
                BodyCommands::Resume { by_brain, checkpoint_id, json } => {
                    let res = ContinuityManager::resume_latest(&storage, &by_brain, checkpoint_id.as_deref())?;
                    let out = serde_json::to_string_pretty(&res)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else if let Some(cp) = res {
                        println!("{}", ContinuityManager::render_brief_snippet(&cp, 300));
                    } else {
                        println!("ℹ️ 当前暂无待接力的活跃跨大脑胶囊。");
                    }
                }
                BodyCommands::Heal { json } => {
                    let rep = HomeostasisVitalsEngine::run_l1_autonomic_heal(&storage)?;
                    if json {
                        println!("{}", serde_json::to_string_pretty(&rep)?);
                    } else {
                        println!(
                            "🩺 L1 稳态反射自愈完成: {:.1} → {:.1} (Δ={:+.2}) | 清理过期锁={} 剪枝幽灵={}",
                            rep.before_score,
                            rep.after_score,
                            rep.delta_score,
                            rep.expired_locks_cleaned,
                            rep.pruned_ghost_nodes
                        );
                    }
                }
                BodyCommands::Peers { json } => {
                    let cfg = Config::load_or_default(None).unwrap_or_default();
                    let client = agent_body::mesh::MeshClient::new(&cfg.mesh, cfg.web.auth_token.clone());
                    let peers = client.query_peer_body_manifests();
                    if json {
                        println!("{}", serde_json::to_string_pretty(&peers)?);
                    } else if peers.is_empty() {
                        println!("ℹ️ 未配置 [mesh].peers 多本体对等节点。");
                    } else {
                        for p in &peers {
                            println!("🌐 {} -> ok={} ({}ms)", p.peer, p.ok, p.latency_ms);
                        }
                    }
                }
                BodyCommands::PerfSnapshot { label, set_baseline, json } => {
                    let snap = agent_body::body::BodyPerfEngine::capture_snapshot(
                        &storage,
                        Some(&label),
                        set_baseline,
                    )?;
                    let out = serde_json::to_string_pretty(&snap)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "📸 已抓取本体 AWR 性能快照 `{}` (label={}, baseline={}) | RSS={} KB | HWM={} KB | FDs={} | 稳态分={:.1}",
                            snap.snapshot_id,
                            snap.label,
                            snap.is_baseline,
                            snap.kernel_telemetry.rss_kb,
                            snap.kernel_telemetry.hwm_kb,
                            snap.kernel_telemetry.open_fds,
                            snap.homeostasis_score
                        );
                    }
                }
                BodyCommands::PerfAwr { snap_a, snap_b, report_id, vs_baseline, json } => {
                    let rep = if let Some(rid) = report_id.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
                        agent_body::body::BodyPerfEngine::load_awr_report(
                            storage.provider().as_ref(),
                            rid,
                        )
                        .ok_or_else(|| anyhow::anyhow!("未找到历史 AWR 报告: {}", rid))?
                    } else {
                        agent_body::body::BodyPerfEngine::generate_awr_report(
                            &storage,
                            snap_a.as_deref(),
                            snap_b.as_deref(),
                            vs_baseline,
                        )?
                    };
                    let out = serde_json::to_string_pretty(&rep)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        print!("{}", rep.markdown_summary);
                    }
                }
                BodyCommands::PerfSnapshots { json } => {
                    let (snapshots, baseline) =
                        agent_body::body::BodyPerfEngine::list_snapshots_meta(storage.provider().as_ref());
                    let payload = serde_json::json!({
                        "baseline": baseline,
                        "snapshots": snapshots,
                        "count": snapshots.len(),
                    });
                    let out = serde_json::to_string_pretty(&payload)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else if snapshots.is_empty() {
                        println!("ℹ️ 快照库暂无已保存的 AWR 性能快照。");
                    } else {
                        println!("📸 已保存 {} 份本体 AWR 性能快照：", snapshots.len());
                        for s in &snapshots {
                            println!(
                                "  - `{}` [{}] {} | RSS={} KB | FDs={} | BodyTime={} μs | 稳态={:.1}",
                                s.snapshot_id,
                                if s.is_baseline { "BASELINE" } else { &s.label },
                                s.captured_at,
                                s.rss_kb,
                                s.open_fds,
                                s.total_body_time_us,
                                s.homeostasis_score
                            );
                        }
                    }
                }
                BodyCommands::PerfReports { json } => {
                    let reports =
                        agent_body::body::BodyPerfEngine::list_awr_reports_meta(storage.provider().as_ref());
                    let payload = serde_json::json!({
                        "reports": reports,
                        "count": reports.len(),
                    });
                    let out = serde_json::to_string_pretty(&payload)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else if reports.is_empty() {
                        println!("ℹ️ 报告库暂无已保存的 AWR 差分诊断报告。");
                    } else {
                        println!("📈 已保存 {} 份 AWR 历史诊断报告：", reports.len());
                        for r in &reports {
                            println!(
                                "  - `{}` [{} → {}] {} | RSS漂移={:+.1}% | 退化={}",
                                r.report_id,
                                r.base_snapshot_id,
                                r.target_snapshot_id,
                                r.generated_at,
                                r.rss_drift_pct,
                                r.regression_detected
                            );
                        }
                    }
                }
                BodyCommands::PerfDelete { kind, id, json } => {
                    let res = agent_body::body::BodyPerfEngine::delete_perf_artifact(
                        &storage,
                        &kind,
                        &id,
                    )?;
                    let out = serde_json::to_string_pretty(&res)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "🗑️ 已处理 AWR 清理请求: kind={}, target_id={}, deleted={}",
                            res.get("kind").and_then(|v| v.as_str()).unwrap_or(&kind),
                            id,
                            res.get("deleted").and_then(|v| v.as_bool()).unwrap_or(false)
                        );
                    }
                }
                BodyCommands::PerfTune { json } => {
                    let rep = agent_body::body::BodyPerfEngine::run_l1_auto_tune(&storage)?;
                    let out = serde_json::to_string_pretty(&rep)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "⚡ L1 内生性能自调优完成 ({} μs): RSS {} KB → {} KB (释放 {:+} KB) | WAL {} KB → {} KB",
                            rep.elapsed_us,
                            rep.before_rss_kb,
                            rep.after_rss_kb,
                            rep.rss_freed_kb,
                            rep.before_wal_bytes / 1024,
                            rep.after_wal_bytes / 1024
                        );
                        for act in &rep.actions_executed {
                            println!("  - {}", act);
                        }
                    }
                }
                BodyCommands::RuntimeMode { set, shadow_primary, mcp_profile, json } => {
                    let payload = if set.is_some() || shadow_primary.is_some() || mcp_profile.is_some() {
                        let next_mode = match set.as_deref() {
                            Some(m) => agent_body::config::RuntimeMode::parse_str(m)
                                .ok_or_else(|| anyhow::anyhow!("未知 mode '{}'，支持: body_only | bypass | shadow_ab", m))?,
                            None => agent_body::config::RuntimeMode::parse_str(&cfg.runtime.mode)
                                .unwrap_or(agent_body::config::RuntimeMode::BodyOnly),
                        };
                        let next_prof = match mcp_profile.as_deref() {
                            Some(p) => Some(
                                agent_body::config::McpToolProfile::parse_str(p)
                                    .ok_or_else(|| anyhow::anyhow!("未知 mcp_profile '{}'，支持: core | dev | ops | full", p))?,
                            ),
                            None => None,
                        };
                        let (cfg_path, updated) = Config::update_runtime_mode(
                            None,
                            next_mode,
                            shadow_primary.as_deref(),
                            next_prof,
                        )?;
                        let mut v = agent_body::body::ShadowAbEngine::build_runtime_status(&updated);
                        if let Some(obj) = v.as_object_mut() {
                            obj.insert("config_path".to_string(), serde_json::json!(cfg_path.to_string_lossy()));
                        }
                        v
                    } else {
                        agent_body::body::ShadowAbEngine::build_runtime_status(cfg)
                    };
                    let out = serde_json::to_string_pretty(&payload)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "🎛️ Agent Body 运行模式: mode={} | shadow_primary={} | mcp_profile={} ({}/{} 工具, Schema {} B, 减税 {:.1}%)",
                            payload["mode"].as_str().unwrap_or("body_only"),
                            payload["shadow_primary"].as_str().unwrap_or("live"),
                            payload["mcp_profile"].as_str().unwrap_or("full"),
                            payload["mcp_tools_active_count"].as_u64().unwrap_or(43),
                            payload["mcp_tools_full_count"].as_u64().unwrap_or(43),
                            payload["schema_bytes_active"].as_u64().unwrap_or(0),
                            payload["schema_reduction_pct"].as_f64().unwrap_or(0.0),
                        );
                    }
                }
                BodyCommands::AbReport { window, limit, json } => {
                    let rep = agent_body::body::ShadowAbEngine::build_ab_report(
                        &storage,
                        cfg,
                        &window,
                        limit,
                    )?;
                    let out = serde_json::to_string_pretty(&rep)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "🧪 真实世界 Shadow A/B 对照报告 [window={}, mode={}]: {}",
                            rep.window, rep.current_mode, rep.verdict
                        );
                        println!("  摘要: {}", rep.verdict_summary);
                        println!(
                            "  样本: 对照={} (胜={}, 平={}, 回退={}, 陈旧={}) | bypass={}",
                            rep.shadow_comparisons,
                            rep.body_wins,
                            rep.ties,
                            rep.body_regressions,
                            rep.body_stale_incidents,
                            rep.bypass_events
                        );
                        for r in &rep.recommendations {
                            println!("  💡 {}", r);
                        }
                    }
                }
                BodyCommands::AbCompare { query, limit, json } => {
                    let (cmp, _, _) = agent_body::body::ShadowAbEngine::compare_asset_search(
                        &storage,
                        cfg,
                        &query,
                        limit,
                        "cli-ab",
                    )?;
                    let out = serde_json::to_string_pretty(&cmp)?;
                    agent_body::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.provider().as_ref(),
                        "cli",
                        "body_state",
                        &out,
                        t0.elapsed().as_micros() as i64,
                    );
                    if json {
                        println!("{}", out);
                    } else {
                        println!(
                            "⚖️ Shadow A/B 单次实测 (`{}`): verdict={} | live={}μs/{}hits/{}tok vs body={}μs/{}hits/{}tok | recall={:.1}% | stale={}",
                            cmp.query,
                            cmp.verdict,
                            cmp.live_elapsed_us,
                            cmp.live_hit_count,
                            cmp.live_tokens,
                            cmp.body_elapsed_us,
                            cmp.body_hit_count,
                            cmp.body_tokens,
                            cmp.recall_vs_live_pct,
                            cmp.body_stale_count
                        );
                        println!("  结论: {}", cmp.reason);
                    }
                }
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}
