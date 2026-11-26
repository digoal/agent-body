use super::{query_str};
use std::sync::Arc;
use serde::Deserialize;
use crate::config::Config;
use crate::mesh::MeshClient;
use crate::storage::{Storage, StorageProvider};
use crate::web::api::WebResponse;
use crate::web::http::{HttpRequest, HttpResponse, StatusCode};

pub fn handle_feedback_list() -> HttpResponse {
    let fb_path = crate::feedback::FeedbackEngine::default_feedback_path();
    match crate::feedback::FeedbackEngine::list_items(&fb_path) {
        Ok(items) => WebResponse::success(serde_json::json!({
            "path": fb_path.to_string_lossy(),
            "count": items.len(),
            "items": items,
        })),
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct FeedbackActionBody {
    pub action: Option<String>,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub title: Option<String>,
    pub evidence: Option<String>,
    pub target: Option<String>,
    pub auto_heal: Option<bool>,
    pub auto_feedback: Option<bool>,
}

pub fn handle_feedback_post(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let t0 = std::time::Instant::now();
    let body: FeedbackActionBody = match req.json() {
        Ok(b) => b,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };
    let fb_path = crate::feedback::FeedbackEngine::default_feedback_path();
    let action = body.action.as_deref().unwrap_or("sync");
    match action {
        "add" => {
            let cat = body.category.as_deref().unwrap_or("CON");
            let prio = body.priority.as_deref().unwrap_or("P1");
            let title = body.title.as_deref().unwrap_or("");
            let evidence = body.evidence.as_deref().unwrap_or("");
            let target = body.target.as_deref().unwrap_or("");
            match crate::feedback::FeedbackEngine::add_item(&fb_path, cat, prio, title, evidence, target) {
                Ok(item) => {
                    crate::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.as_ref(),
                        "web",
                        "body_feedback",
                        &item.title,
                        t0.elapsed().as_micros() as i64,
                    );
                    WebResponse::success(item)
                }
                Err(e) => WebResponse::error(StatusCode::BAD_REQUEST, &e.to_string()),
            }
        }
        "reconcile" => {
            let st = Storage::new(storage.clone());
            match crate::feedback::probes::FreshnessReconciler::reconcile(&st) {
                Ok(rep) => WebResponse::success(rep),
                Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
            }
        }
        "probes" => {
            let st = Storage::new(storage.clone());
            let auto_heal = body.auto_heal.unwrap_or(true);
            let auto_feedback = body.auto_feedback.unwrap_or(false);
            match crate::feedback::probes::SelfEvolutionProbeEngine::run_probes(
                &st,
                &fb_path,
                auto_heal,
                auto_feedback,
            ) {
                Ok(rep) => WebResponse::success(rep),
                Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
            }
        }
        _ => {
            let st = Storage::new(storage.clone());
            match crate::feedback::FeedbackEngine::sync_verified_to_memories(&fb_path, &st) {
                Ok(added) => {
                    crate::benchmark::ledger::EffectivenessLedger::record_channel_call(
                        storage.as_ref(),
                        "web",
                        "body_feedback",
                        &format!("synced {} items", added),
                        t0.elapsed().as_micros() as i64,
                    );
                    WebResponse::success(serde_json::json!({ "synced_to_memories": added }))
                }
                Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
            }
        }
    }
}

/// 处理 GET /api/v1/audit/events
pub fn handle_audit_query(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let st = Storage::new(storage);
    let filter = crate::storage::models::AuditQueryFilter {
        domain: req.query_params.get("domain").filter(|s| !s.is_empty()).cloned(),
        actor_kind: req.query_params.get("actor_kind").filter(|s| !s.is_empty()).cloned(),
        category: req.query_params.get("category").filter(|s| !s.is_empty()).cloned(),
        action: req.query_params.get("action").filter(|s| !s.is_empty()).cloned(),
        outcome: req.query_params.get("outcome").filter(|s| !s.is_empty()).cloned(),
        trace_id: req.query_params.get("trace_id").filter(|s| !s.is_empty()).cloned(),
        session_id: req.query_params.get("session_id").filter(|s| !s.is_empty()).cloned(),
        target_contains: req.query_params.get("target").filter(|s| !s.is_empty()).cloned(),
        window: Some(
            req.query_params
                .get("window")
                .cloned()
                .unwrap_or_else(|| "7d".to_string()),
        ),
        limit: req
            .query_params
            .get("limit")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(50),
    };
    match st.query_audit_events(&filter) {
        Ok(events) => WebResponse::success(serde_json::json!({
            "count": events.len(),
            "events": events,
        })),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Audit query failed: {}", e),
        ),
    }
}

/// 处理 GET /api/v1/audit/summary
pub fn handle_audit_summary(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    use crate::storage::duckdb_analytics::{AnalyticsProvider, DuckDbAnalyticsEngine};
    let window = req
        .query_params
        .get("window")
        .map(|s| s.as_str())
        .unwrap_or("7d");
    let days = req
        .query_params
        .get("days")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(7);
    let st = Storage::new(storage.clone());
    match st.audit_domain_summary(window) {
        Ok(summary) => {
            let olap = DuckDbAnalyticsEngine::from_synced_audit_events(storage)
                .and_then(|eng| eng.audit_analytics_summary(days))
                .ok();
            WebResponse::success(serde_json::json!({
                "summary": summary,
                "olap": olap,
            }))
        }
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Audit summary failed: {}", e),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct AuditEvolveBody {
    pub auto_feedback: Option<bool>,
    pub min_hits: Option<usize>,
}

/// 处理 POST /api/v1/audit/evolve
pub fn handle_audit_evolve(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let body: AuditEvolveBody = req.json().unwrap_or(AuditEvolveBody {
        auto_feedback: Some(false),
        min_hits: Some(3),
    });
    let st = Storage::new(storage);
    let fb_path = crate::feedback::FeedbackEngine::default_feedback_path();
    match crate::audit::AuditEvolutionAnalyzer::run_evolution_cycle(
        &st,
        Some(&fb_path),
        body.auto_feedback.unwrap_or(false),
        body.min_hits.unwrap_or(3),
    ) {
        Ok(rep) => WebResponse::success(rep),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Audit evolution failed: {}", e),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct AuditExportBody {
    pub output_path: Option<String>,
}

/// 处理 POST /api/v1/audit/export
pub fn handle_audit_export(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    use crate::storage::duckdb_analytics::{AnalyticsProvider, DuckDbAnalyticsEngine};
    let body: AuditExportBody = req.json().unwrap_or(AuditExportBody { output_path: None });
    let default_out = Config::load_effective_or_default()
        .effective_temp_dir()
        .join("audit_events_export.csv")
        .to_string_lossy()
        .to_string();
    let out_path = body
        .output_path
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(default_out);
    match DuckDbAnalyticsEngine::from_synced_audit_events(storage)
        .and_then(|eng| eng.export_to_parquet("audit_events", &out_path))
    {
        Ok(exp) => WebResponse::success(exp),
        Err(e) => WebResponse::error(StatusCode::BAD_REQUEST, &format!("Audit export failed: {}", e)),
    }
}

/* ─── Phase 44: Agent 本体中枢 (BodyManifest / Vitals / Envelope / Continuity / Peers) ─── */

pub fn handle_body_manifest(storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let st = Storage::new(storage);
    WebResponse::success(crate::body::BodyManifestEngine::build(&st))
}

pub fn handle_body_vitals(storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let st = Storage::new(storage);
    WebResponse::success(crate::body::HomeostasisVitalsEngine::assess(&st))
}

pub fn handle_body_envelope(req: &HttpRequest) -> HttpResponse {
    let act = query_str(req, "action", "");
    if !act.trim().is_empty() {
        let target = req.query_params.get("target").map(|s| s.as_str());
        let dry_run = query_str(req, "dry_run", "true") != "false";
        WebResponse::success(crate::body::AutonomyEnvelope::evaluate(&act, target, dry_run))
    } else {
        WebResponse::success(serde_json::json!({
            "matrix": crate::body::AutonomyEnvelope::policy_matrix(),
        }))
    }
}

pub fn handle_body_checkpoints(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let st = Storage::new(storage);
    let status = req
        .query_params
        .get("status")
        .filter(|s| !s.trim().is_empty())
        .map(|s| s.as_str());
    let limit = query_str(req, "limit", "20").parse::<usize>().unwrap_or(20);
    match st.list_body_checkpoints(status, limit) {
        Ok(items) => WebResponse::success(serde_json::json!({
            "count": items.len(),
            "checkpoints": items,
        })),
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub fn handle_body_peers() -> HttpResponse {
    let cfg = Config::load_or_default(None).unwrap_or_default();
    let client = MeshClient::new(&cfg.mesh, cfg.web.auth_token.clone());
    let results = client.query_peer_body_manifests();
    WebResponse::success(serde_json::json!({
        "configured_peers": client.peers(),
        "results": results,
    }))
}

#[derive(Debug, Deserialize)]
pub struct BodyHandoffBody {
    pub from_brain: Option<String>,
    pub session_id: Option<String>,
    pub goal_summary: String,
    pub active_files: Option<Vec<String>>,
    pub verified_facts: Option<Vec<String>>,
    pub open_blockers: Option<Vec<String>>,
    pub next_step_hint: Option<String>,
}

pub fn handle_body_handoff(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let t0 = std::time::Instant::now();
    let body: BodyHandoffBody = match req.json() {
        Ok(b) => b,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };
    if body.goal_summary.trim().is_empty() {
        return WebResponse::error(StatusCode::BAD_REQUEST, "goal_summary cannot be empty");
    }
    let storage_ref = storage.clone();
    let st = Storage::new(storage);
    match crate::body::ContinuityManager::save_capsule(
        &st,
        crate::body::ContinuityInput {
            from_brain: body.from_brain.unwrap_or_else(|| "web-console".to_string()),
            session_id: body.session_id.unwrap_or_else(|| "web-live".to_string()),
            goal_summary: body.goal_summary,
            active_files: body.active_files.unwrap_or_default(),
            active_locks: Vec::new(),
            verified_facts: body.verified_facts.unwrap_or_default(),
            open_blockers: body.open_blockers.unwrap_or_default(),
            next_step_hint: body.next_step_hint.unwrap_or_default(),
            token_budget: Some(300),
        },
    ) {
        Ok(cp) => {
            crate::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage_ref.as_ref(),
                "web",
                "body_state",
                &cp.goal_summary,
                t0.elapsed().as_micros() as i64,
            );
            WebResponse::success(cp)
        }
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Debug, Deserialize)]
pub struct BodyResumeBody {
    pub by_brain: Option<String>,
    pub checkpoint_id: Option<String>,
}

pub fn handle_body_resume(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let t0 = std::time::Instant::now();
    let body: BodyResumeBody = req.json().unwrap_or(BodyResumeBody {
        by_brain: None,
        checkpoint_id: None,
    });
    let storage_ref = storage.clone();
    let st = Storage::new(storage);
    let by = body.by_brain.as_deref().unwrap_or("web-console");
    match crate::body::ContinuityManager::resume_latest(&st, by, body.checkpoint_id.as_deref()) {
        Ok(Some(cp)) => {
            let snippet = crate::body::ContinuityManager::render_brief_snippet(&cp, 300);
            crate::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage_ref.as_ref(),
                "web",
                "body_state",
                &snippet,
                t0.elapsed().as_micros() as i64,
            );
            WebResponse::success(serde_json::json!({
                "resumed": true,
                "checkpoint": cp,
                "brief_snippet": snippet,
            }))
        }
        Ok(None) => WebResponse::success(serde_json::json!({
            "resumed": false,
            "checkpoint": serde_json::Value::Null,
        })),
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

pub fn handle_body_heal(storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let st = Storage::new(storage);
    match crate::body::HomeostasisVitalsEngine::run_l1_autonomic_heal(&st) {
        Ok(rep) => WebResponse::success(rep),
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct BodyPerfSnapshotBody {
    pub label: Option<String>,
    pub set_baseline: Option<bool>,
}

pub fn handle_body_perf_snapshot(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let t0 = std::time::Instant::now();
    let body: BodyPerfSnapshotBody = req.json().unwrap_or_default();
    let storage_ref = storage.clone();
    let st = Storage::new(storage);
    let set_base = body.set_baseline.unwrap_or(false);
    match crate::body::BodyPerfEngine::capture_snapshot(&st, body.label.as_deref(), set_base) {
        Ok(snap) => {
            crate::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage_ref.as_ref(),
                "web",
                "body_state",
                &snap.snapshot_id,
                t0.elapsed().as_micros() as i64,
            );
            WebResponse::success(snap)
        }
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct BodyPerfAwrBody {
    pub snap_a: Option<String>,
    pub snap_b: Option<String>,
    pub vs_baseline: Option<bool>,
}

fn render_body_perf_awr_response(
    storage_ref: Arc<dyn StorageProvider>,
    rep_res: anyhow::Result<crate::body::AwrDiffReport>,
    t0: std::time::Instant,
) -> HttpResponse {
    match rep_res {
        Ok(rep) => {
            let (snapshots_meta, baseline_meta) =
                crate::body::BodyPerfEngine::list_snapshots_meta(storage_ref.as_ref());
            let reports_meta =
                crate::body::BodyPerfEngine::list_awr_reports_meta(storage_ref.as_ref());
            let snapshot_ids: Vec<String> =
                snapshots_meta.iter().map(|m| m.snapshot_id.clone()).collect();
            let baseline_id = baseline_meta.as_ref().map(|m| m.snapshot_id.clone());
            crate::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage_ref.as_ref(),
                "web",
                "body_state",
                &rep.markdown_summary,
                t0.elapsed().as_micros() as i64,
            );
            WebResponse::success(serde_json::json!({
                "report": rep,
                "snapshots": snapshot_ids,
                "snapshots_meta": snapshots_meta,
                "baseline_id": baseline_id,
                "baseline_meta": baseline_meta,
                "reports_meta": reports_meta,
            }))
        }
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/// GET /api/v1/body/perf/awr —— 严格只读（查询已保存历史报告或实时内存预览差分，零写库）
pub fn handle_body_perf_awr(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let t0 = std::time::Instant::now();
    let report_id = req
        .query_params
        .get("report_id")
        .map(|s| s.as_str())
        .filter(|s| !s.trim().is_empty());
    let snap_a = req
        .query_params
        .get("snap_a")
        .map(|s| s.as_str())
        .filter(|s| !s.trim().is_empty());
    let snap_b = req
        .query_params
        .get("snap_b")
        .map(|s| s.as_str())
        .filter(|s| !s.trim().is_empty());
    let vs_baseline = req
        .query_params
        .get("vs_baseline")
        .map(|s| s != "false" && s != "0")
        .unwrap_or(true);
    let view_only = req
        .query_params
        .get("view_only")
        .map(|s| s == "true" || s == "1")
        .unwrap_or(false);

    let storage_ref = storage.clone();
    let st = Storage::new(storage);

    let rep_res = if let Some(rid) = report_id {
        crate::body::BodyPerfEngine::load_awr_report(storage_ref.as_ref(), rid)
            .ok_or_else(|| anyhow::anyhow!("未找到历史 AWR 报告: {}", rid))
    } else if view_only {
        let existing_reports = crate::body::BodyPerfEngine::list_awr_reports_meta(storage_ref.as_ref());
        if let Some(first) = existing_reports.first() {
            crate::body::BodyPerfEngine::load_awr_report(storage_ref.as_ref(), &first.report_id)
                .ok_or_else(|| anyhow::anyhow!("加载最新历史 AWR 报告失败"))
        } else {
            crate::body::BodyPerfEngine::generate_awr_report_with_options(
                &st,
                snap_a,
                snap_b,
                vs_baseline,
                false,
            )
        }
    } else {
        crate::body::BodyPerfEngine::generate_awr_report_with_options(
            &st,
            snap_a,
            snap_b,
            vs_baseline,
            false,
        )
    };

    render_body_perf_awr_response(storage_ref, rep_res, t0)
}

/// POST /api/v1/body/perf/awr —— 生成并持久化保存 AWR 差分报告（受 write auth 保护）
pub fn handle_body_perf_awr_post(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let t0 = std::time::Instant::now();
    let body: BodyPerfAwrBody = req.json().unwrap_or_default();
    let snap_a_q = req
        .query_params
        .get("snap_a")
        .map(|s| s.as_str())
        .filter(|s| !s.trim().is_empty());
    let snap_b_q = req
        .query_params
        .get("snap_b")
        .map(|s| s.as_str())
        .filter(|s| !s.trim().is_empty());
    let snap_a = body
        .snap_a
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(snap_a_q);
    let snap_b = body
        .snap_b
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .or(snap_b_q);
    let vs_baseline = body.vs_baseline.unwrap_or_else(|| {
        req.query_params
            .get("vs_baseline")
            .map(|s| s != "false" && s != "0")
            .unwrap_or(true)
    });

    let storage_ref = storage.clone();
    let st = Storage::new(storage);
    let rep_res = crate::body::BodyPerfEngine::generate_awr_report(&st, snap_a, snap_b, vs_baseline);
    render_body_perf_awr_response(storage_ref, rep_res, t0)
}

pub fn handle_body_perf_tune(storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let t0 = std::time::Instant::now();
    let storage_ref = storage.clone();
    let st = Storage::new(storage);
    match crate::body::BodyPerfEngine::run_l1_auto_tune(&st) {
        Ok(rep) => {
            crate::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage_ref.as_ref(),
                "web",
                "body_state",
                "perf_tune",
                t0.elapsed().as_micros() as i64,
            );
            WebResponse::success(rep)
        }
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct BodyPerfDeleteBody {
    pub kind: Option<String>,
    pub target_kind: Option<String>,
    pub id: Option<String>,
    pub target_id: Option<String>,
    pub report_id: Option<String>,
    pub snapshot_id: Option<String>,
}

/// POST/DELETE /api/v1/body/perf/delete —— 删除指定 AWR 快照或历史诊断报告（受 write auth 保护）
pub fn handle_body_perf_delete(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let t0 = std::time::Instant::now();
    let body: BodyPerfDeleteBody = req.json().unwrap_or_default();
    let kind = body
        .target_kind
        .as_deref()
        .or(body.kind.as_deref())
        .or_else(|| req.query_params.get("kind").map(String::as_str))
        .unwrap_or("auto");
    let target_id = body
        .target_id
        .as_deref()
        .or(body.id.as_deref())
        .or(body.report_id.as_deref())
        .or(body.snapshot_id.as_deref())
        .or_else(|| req.query_params.get("id").map(String::as_str))
        .or_else(|| req.query_params.get("report_id").map(String::as_str))
        .or_else(|| req.query_params.get("snapshot_id").map(String::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let Some(tid) = target_id else {
        return WebResponse::error(
            StatusCode::BAD_REQUEST,
            "缺少待删除目标 id / target_id / report_id / snapshot_id",
        );
    };

    let storage_ref = storage.clone();
    let st = Storage::new(storage);
    match crate::body::BodyPerfEngine::delete_perf_artifact(&st, kind, tid) {
        Ok(res) => {
            let (snapshots_meta, baseline_meta) =
                crate::body::BodyPerfEngine::list_snapshots_meta(storage_ref.as_ref());
            let reports_meta =
                crate::body::BodyPerfEngine::list_awr_reports_meta(storage_ref.as_ref());
            let out_str = serde_json::to_string(&res).unwrap_or_default();
            crate::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage_ref.as_ref(),
                "web",
                "body_state",
                &out_str,
                t0.elapsed().as_micros() as i64,
            );
            WebResponse::success(serde_json::json!({
                "result": res,
                "snapshots_meta": snapshots_meta,
                "baseline_meta": baseline_meta,
                "reports_meta": reports_meta,
            }))
        }
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

/* ─── Phase 58: 三态运行模式与真实 Shadow A/B 对照 API ─────────────────── */

pub fn handle_body_runtime_mode_get() -> HttpResponse {
    let cfg = Config::load_effective_or_default();
    WebResponse::success(crate::body::ShadowAbEngine::build_runtime_status(&cfg))
}

#[derive(Debug, Deserialize, Default)]
pub struct BodyRuntimeModeBody {
    pub mode: Option<String>,
    pub shadow_primary: Option<String>,
    pub mcp_profile: Option<String>,
}

pub fn handle_body_runtime_mode_post(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let t0 = std::time::Instant::now();
    let body: BodyRuntimeModeBody = match req.json() {
        Ok(b) => b,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };
    let cur = Config::load_effective_or_default();
    let next_mode = match body.mode.as_deref() {
        Some(m) => match crate::config::RuntimeMode::parse_str(m) {
            Some(v) => v,
            None => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("未知 mode: {}", m)),
        },
        None => crate::config::RuntimeMode::parse_str(&cur.runtime.mode)
            .unwrap_or(crate::config::RuntimeMode::BodyOnly),
    };
    let next_prof = match body.mcp_profile.as_deref() {
        Some(p) => match crate::config::McpToolProfile::parse_str(p) {
            Some(v) => Some(v),
            None => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("未知 mcp_profile: {}", p)),
        },
        None => None,
    };
    match Config::update_runtime_mode(None, next_mode, body.shadow_primary.as_deref(), next_prof) {
        Ok((cfg_path, updated)) => {
            let mut payload = crate::body::ShadowAbEngine::build_runtime_status(&updated);
            if let Some(obj) = payload.as_object_mut() {
                obj.insert("config_path".to_string(), serde_json::json!(cfg_path.to_string_lossy()));
            }
            crate::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage.as_ref(),
                "web",
                "body_state",
                next_mode.as_str(),
                t0.elapsed().as_micros() as i64,
            );
            WebResponse::success(payload)
        }
        Err(e) => WebResponse::error(StatusCode::BAD_REQUEST, &e.to_string()),
    }
}

pub fn handle_body_ab_report(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let cfg = Config::load_effective_or_default();
    let window = query_str(req, "window", "7d");
    let limit = query_str(req, "limit", "200")
        .parse::<usize>()
        .unwrap_or(200)
        .clamp(20, 2000);
    let st = Storage::new(storage);
    match crate::body::ShadowAbEngine::build_ab_report(&st, &cfg, &window, limit) {
        Ok(rep) => WebResponse::success(rep),
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct BodyAbCompareBody {
    pub query: Option<String>,
    pub limit: Option<usize>,
}

pub fn handle_body_ab_compare(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let t0 = std::time::Instant::now();
    let body: BodyAbCompareBody = req.json().unwrap_or_default();
    let q = body
        .query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| query_str(req, "query", ""));
    if q.trim().is_empty() {
        return WebResponse::error(StatusCode::BAD_REQUEST, "缺少对照检索参数 query");
    }
    let limit = body.limit.unwrap_or(10).clamp(1, 50);
    let cfg = Config::load_effective_or_default();
    let st = Storage::new(storage.clone());
    match crate::body::ShadowAbEngine::compare_asset_search(&st, &cfg, &q, limit, "web-ab") {
        Ok((cmp, _, _)) => {
            crate::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage.as_ref(),
                "web",
                "body_state",
                &cmp.verdict,
                t0.elapsed().as_micros() as i64,
            );
            WebResponse::success(cmp)
        }
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}
