use serde_json::Value;
use anyhow::{bail, Result};


use super::McpToolRegistry;

impl McpToolRegistry {
    pub(super) fn call_body_tool(&self, name: &str, args: &Value) -> Option<Result<String>> {
        if !matches!(name, "body_demo_scenario" | "body_benchmark" | "body_feedback" | "body_audit_query" | "body_state") {
            return None;
        }
        Some(self.call_body_tool_inner(name, args))
    }

    fn call_body_tool_inner(&self, name: &str, args: &Value) -> Result<String> {
        match name {
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
