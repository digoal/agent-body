use super::{check_auth, query_str};
use std::path::{Path};
use std::sync::Arc;
use serde::Deserialize;
use crate::api::mcp_tools::McpToolRegistry;
use crate::config::Config;
use crate::coordination::{CoordinationEngine, LockRequest, LockScope, ResourceType};
use crate::ecosystem::ExternalContactsManager;
use crate::env::gc::HostGarbageCollector;
use crate::memory::hybrid::HybridSearchEngine;
use crate::memory::lifecycle::MemoryLifecycleManager;
use crate::memory::MemoryManager;
use crate::mesh::MeshClient;
use crate::session::SessionManager;
use crate::storage::{AnalyticsProvider, DuckDbAnalyticsEngine, Storage, StorageProvider};
use crate::web::api::WebResponse;
use crate::web::http::{HttpRequest, HttpResponse, StatusCode};

pub fn handle_analytics_heatmap(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let days = query_str(req, "days", "30")
        .parse::<usize>()
        .unwrap_or(30)
        .clamp(1, 365);
    let engine = match DuckDbAnalyticsEngine::from_synced_storage(storage) {
        Ok(e) => e,
        Err(e) => {
            return WebResponse::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("DuckDB sync failed: {}", e),
            )
        }
    };
    match engine.changes_heatmap(days) {
        Ok(heatmap) => WebResponse::success(serde_json::json!({
            "time_span_days": days,
            "heatmap": heatmap,
        })),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Heatmap query failed: {}", e),
        ),
    }
}

pub fn handle_analytics_trend(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let days = query_str(req, "days", "30")
        .parse::<usize>()
        .unwrap_or(30)
        .clamp(1, 365);
    let engine = match DuckDbAnalyticsEngine::from_synced_storage(storage) {
        Ok(e) => e,
        Err(e) => {
            return WebResponse::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("DuckDB sync failed: {}", e),
            )
        }
    };
    let trend = engine.asset_growth_trend(days).unwrap_or_default();
    let decay = engine.memory_decay_curve().unwrap_or_default();
    let stats = match engine.fact_graph_stats() {
        Ok(s) => s,
        Err(e) => {
            return WebResponse::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Graph stats failed: {}", e),
            )
        }
    };
    WebResponse::success(serde_json::json!({
        "time_span_days": days,
        "asset_growth_trend": trend,
        "memory_decay": decay,
        "graph_stats": stats,
    }))
}

#[derive(Debug, Deserialize)]
pub struct AnalyticsExportBody {
    pub table: Option<String>,
    pub output_path: Option<String>,
}

pub fn handle_analytics_export(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let body: AnalyticsExportBody = match req.json() {
        Ok(b) => b,
        Err(_) => AnalyticsExportBody {
            table: None,
            output_path: None,
        },
    };
    let cfg = Config::load_effective_or_default();
    let table = body.table.unwrap_or_else(|| "assets".to_string());
    let default_path = cfg
        .effective_temp_dir()
        .join(format!("agent_body_{}.parquet", table))
        .to_string_lossy()
        .to_string();
    let output_path = body
        .output_path
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(default_path);
    if !cfg.is_allowed_temp_path(Path::new(&output_path)) {
        return WebResponse::error(
            StatusCode::FORBIDDEN,
            "Export path must reside inside configured temporary directory or /tmp",
        );
    }
    let engine = match DuckDbAnalyticsEngine::from_synced_storage(storage) {
        Ok(e) => e,
        Err(e) => {
            return WebResponse::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("DuckDB sync failed: {}", e),
            )
        }
    };
    match engine.export_to_parquet(&table, &output_path) {
        Ok(summary) => WebResponse::success(summary),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Export failed: {}", e),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct MemoryAddBody {
    pub memory_type: Option<String>,
    pub content: String,
    pub summary: Option<String>,
}

pub fn handle_memories_add(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let body: MemoryAddBody = match req.json() {
        Ok(b) => b,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };
    if body.content.trim().is_empty() {
        return WebResponse::error(StatusCode::BAD_REQUEST, "content must not be empty");
    }
    let mm = MemoryManager::new(Storage::new(storage));
    let mem_type = body.memory_type.unwrap_or_else(|| "decision".to_string());
    match mm.add(
        &mem_type,
        body.content.trim(),
        body.summary.as_deref(),
        Some("web_console"),
        vec![],
    ) {
        Ok(id) => WebResponse::success(serde_json::json!({
            "id": id,
            "memory_type": mem_type,
            "saved": true,
        })),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Memory add failed: {}", e),
        ),
    }
}

pub fn handle_memories_search(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let q = query_str(req, "q", "");
    let limit = query_str(req, "limit", "15")
        .parse::<usize>()
        .unwrap_or(15)
        .clamp(1, 100);
    let hybrid = query_str(req, "hybrid", "true") != "false";

    if q.trim().is_empty() {
        let items = storage.list_all_active_memories(limit).unwrap_or_default();
        return WebResponse::success(serde_json::json!({
            "mode": "recent",
            "items": items,
        }));
    }

    let st = Storage::new(storage);
    if hybrid {
        let engine = HybridSearchEngine::new(Arc::new(st));
        let fut = engine.search_hybrid(q.trim(), limit);
        let results = match tokio::runtime::Handle::try_current() {
            Ok(rt) => rt.block_on(fut),
            Err(_) => match tokio::runtime::Runtime::new() {
                Ok(rt) => rt.block_on(fut),
                Err(e) => {
                    return WebResponse::error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        &format!("Runtime init failed: {}", e),
                    )
                }
            },
        };
        match results {
            Ok(list) => WebResponse::success(serde_json::json!({
                "mode": "hybrid_rrf",
                "items": list,
            })),
            Err(e) => WebResponse::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Hybrid memory search failed: {}", e),
            ),
        }
    } else {
        let mm = MemoryManager::new(st);
        match mm.search(q.trim(), limit) {
            Ok(list) => WebResponse::success(serde_json::json!({
                "mode": "fts",
                "items": list,
            })),
            Err(e) => WebResponse::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("FTS memory search failed: {}", e),
            ),
        }
    }
}

pub fn handle_memories_audit(storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let st = Storage::new(storage);
    let mgr = MemoryLifecycleManager::new(&st);
    match mgr.audit_memories() {
        Ok(rep) => WebResponse::success(rep),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Memory audit failed: {}", e),
        ),
    }
}

pub fn handle_env_gc(req: &HttpRequest) -> HttpResponse {
    let max_age_hours = query_str(req, "max_age_hours", "72")
        .parse::<u64>()
        .unwrap_or(72)
        .clamp(1, 8760);
    let tmp_dir = Config::load_effective_or_default().effective_temp_dir();
    // Web 端默认强制 dry_run=true，绝不静默硬删除
    match HostGarbageCollector::run_gc(&tmp_dir, max_age_hours, true) {
        Ok(rep) => WebResponse::success(rep),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("GC dry-run failed: {}", e),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct SessionCloseBody {
    pub session_id: Option<String>,
    pub clean_temp: Option<bool>,
}

pub fn handle_session_close(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let body: SessionCloseBody = match req.json() {
        Ok(b) => b,
        Err(_) => SessionCloseBody {
            session_id: None,
            clean_temp: None,
        },
    };
    let session_id = body
        .session_id
        .unwrap_or_else(|| "web-preview-session".to_string());
    let clean_temp = body.clean_temp.unwrap_or(true);
    let sm = SessionManager::new(Arc::new(Storage::new(storage)));
    // 默认强制 dry_run=true 预览结清清单
    match sm.close_session_with_dry_run(&session_id, clean_temp, true) {
        Ok(rep) => WebResponse::success(rep),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Session close preview failed: {}", e),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct LockAcquireBody {
    pub resource_type: String,
    pub resource_id: String,
    pub scope: Option<String>,
    pub holder_agent: Option<String>,
    pub ttl_seconds: Option<u64>,
    pub wait_timeout_ms: Option<u64>,
    pub priority: Option<i32>,
}

pub fn handle_locks_acquire(
    req: &HttpRequest,
    storage: Arc<dyn StorageProvider>,
    auth_token: &Option<String>,
) -> HttpResponse {
    if let Some(err_resp) = check_auth(req, auth_token) {
        return err_resp;
    }
    let body: LockAcquireBody = match req.json() {
        Ok(b) => b,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };
    let engine = CoordinationEngine::new(storage);
    let lreq = LockRequest {
        resource_type: ResourceType::from_str_loose(&body.resource_type),
        resource_id: body.resource_id,
        scope: LockScope::from_str_loose(body.scope.as_deref().unwrap_or("exclusive")),
        holder_agent: body.holder_agent.unwrap_or_else(|| "web-console".to_string()),
        holder_pid: None,
        ttl_seconds: body.ttl_seconds.unwrap_or(120).clamp(10, 3600),
        metadata: Some(serde_json::json!({"source": "web_console"})),
    };
    match engine.try_acquire(&lreq) {
        Ok(res) => WebResponse::success(res),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Lock acquire failed: {}", e),
        ),
    }
}

pub fn handle_locks_wait(
    req: &HttpRequest,
    storage: Arc<dyn StorageProvider>,
    auth_token: &Option<String>,
) -> HttpResponse {
    if let Some(err_resp) = check_auth(req, auth_token) {
        return err_resp;
    }
    let body: LockAcquireBody = match req.json() {
        Ok(b) => b,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };
    let engine = CoordinationEngine::new(storage);
    let lreq = LockRequest {
        resource_type: ResourceType::from_str_loose(&body.resource_type),
        resource_id: body.resource_id,
        scope: LockScope::from_str_loose(body.scope.as_deref().unwrap_or("exclusive")),
        holder_agent: body.holder_agent.unwrap_or_else(|| "web-waiter".to_string()),
        holder_pid: None,
        ttl_seconds: body.ttl_seconds.unwrap_or(120).clamp(10, 3600),
        metadata: Some(serde_json::json!({"source": "web_console"})),
    };
    let wait_ms = body.wait_timeout_ms.unwrap_or(1200).clamp(100, 5000);
    let priority = body.priority.unwrap_or(100);
    match engine.wait_and_acquire(&lreq, wait_ms, priority) {
        Ok(res) => WebResponse::success(res),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Lock wait failed: {}", e),
        ),
    }
}

pub fn handle_service_check(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let name = query_str(req, "name", "");
    if name.trim().is_empty() {
        return WebResponse::error(StatusCode::BAD_REQUEST, "name parameter is required");
    }
    let mgr = ExternalContactsManager::new(Storage::new(storage));
    match mgr.check_health(name.trim()) {
        Ok(rep) => WebResponse::success(rep),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Service health check failed: {}", e),
        ),
    }
}

pub fn handle_mesh_query(req: &HttpRequest) -> HttpResponse {
    let action = query_str(req, "action", "ping");
    let cfg = Config::load_or_default(None).unwrap_or_default();
    let client = MeshClient::new(&cfg.mesh, cfg.web.auth_token.clone());
    match action.as_str() {
        "facts" => WebResponse::success(client.facts_all()),
        "locks" => WebResponse::success(client.locks_all()),
        "port" => {
            let port = query_str(req, "port", "8080").parse::<u16>().unwrap_or(8080);
            WebResponse::success(client.federated_port_check(port))
        }
        "graph" => {
            let q = query_str(req, "query", "");
            let hops = query_str(req, "hops", "2").parse::<usize>().unwrap_or(2).clamp(1, 4);
            WebResponse::success(client.federated_graph_query(&q, hops))
        }
        "dialogue_inbox" => {
            match Storage::open(&cfg.db_path) {
                Ok(st) => WebResponse::success(st.list_mesh_dialogues(30).unwrap_or_default()),
                Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
            }
        }
        "transfer_inbox" => {
            match Storage::open(&cfg.db_path) {
                Ok(st) => WebResponse::success(st.list_mesh_transfers(30).unwrap_or_default()),
                Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
            }
        }
        _ => WebResponse::success(client.ping_all()),
    }
}

fn map_tool_parity(tool_name: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    match tool_name {
        "body_brief" => ("agent-body brief", "GET /api/v1/brief", "governance", "synced"),
        "body_placement_suggest" => ("agent-body place suggest", "POST /api/v1/placement/suggest", "governance", "synced"),
        "body_placement_rules" => ("agent-body place rules", "GET /api/v1/governance", "governance", "synced"),
        "body_placement_learn" => ("agent-body place learn", "GET /api/v1/placement/learn", "governance", "synced"),
        "body_asset_search" => ("agent-body asset search", "GET /api/v1/assets/search", "facts", "synced"),
        "body_asset_register" => ("agent-body asset register", "POST /api/v1/assets/register", "facts", "synced"),
        "body_asset_scan" => ("agent-body asset scan", "POST /api/v1/assets/scan", "facts", "synced"),
        "body_env_processes" => ("agent-body env processes", "GET /api/v1/facts", "facts", "synced"),
        "body_env_show" => ("agent-body env show", "GET /api/v1/overview", "topbar", "synced"),
        "body_env_gc" => ("agent-body gc", "GET /api/v1/env/gc", "governance", "synced"),
        "body_memory_add" => ("agent-body memory add", "POST /api/v1/memories/add", "governance", "synced"),
        "body_memory_search" => ("agent-body memory search", "GET /api/v1/memories/search", "governance", "synced"),
        "body_memory_audit" => ("agent-body memory audit", "GET /api/v1/memories/audit", "governance", "synced"),
        "body_links_query" => ("agent-body links query", "GET /api/v1/topology", "topology", "synced"),
        "body_links_visualize" => ("agent-body links graph", "GET /api/v1/links/mermaid", "topology", "synced"),
        "body_links_impact" => ("agent-body links impact", "GET /api/v1/topology/impact", "topology", "synced"),
        "body_links_audit" => ("agent-body links audit", "GET /api/v1/links/audit", "topology", "synced"),
        "body_links_repair" => ("agent-body links repair", "GET /api/v1/links/repair", "topology", "synced"),
        "body_codegraph_bridge" => ("agent-body links codegraph", "GET /api/v1/links/codegraph", "topology", "synced"),
        "body_changes_since" => ("agent-body changes", "GET /api/v1/governance", "governance", "synced"),
        "body_port_check" => ("agent-body env port", "GET /api/v1/facts/port-check", "facts", "synced"),
        "body_session_close" => ("agent-body close", "POST /api/v1/session/close", "governance", "synced"),
        "body_fact_query" => ("agent-body fact", "GET /api/v1/facts/query", "facts", "synced"),
        "body_hook_pre" => ("agent-body hooks pre", "POST /api/v1/hooks/pre", "ecosystem", "synced"),
        "body_hook_post" => ("agent-body hooks post", "POST /api/v1/hooks/post", "ecosystem", "synced"),
        "body_hook_generate" => ("agent-body hooks generate", "GET /api/v1/hooks/generate", "ecosystem", "synced"),
        "body_analytics_heatmap" => ("agent-body analytics heatmap", "GET /api/v1/analytics/heatmap", "governance", "synced"),
        "body_analytics_trend" => ("agent-body analytics trend", "GET /api/v1/analytics/trend", "governance", "synced"),
        "body_analytics_export" => ("agent-body analytics export", "POST /api/v1/analytics/export", "governance", "synced"),
        "body_lock_acquire" => ("agent-body coordination acquire", "POST /api/v1/locks/acquire", "locks", "synced"),
        "body_lock_release" => ("agent-body coordination release", "POST /api/v1/locks/release", "locks", "synced"),
        "body_lock_list" => ("agent-body coordination list", "GET /api/v1/locks", "locks", "synced"),
        "body_lock_wait" => ("agent-body coordination wait", "POST /api/v1/locks/wait", "locks", "synced"),
        "body_lock_deadlock_check" => ("agent-body coordination deadlock", "GET /api/v1/locks/deadlocks", "locks", "synced"),
        "body_mcp_catalog" => ("agent-body mcp-catalog", "GET /api/v1/ecosystem", "ecosystem", "synced"),
        "body_skill_route" => ("agent-body skill route", "GET /api/v1/ecosystem/route", "ecosystem", "synced"),
        "body_service_contact" => ("agent-body service", "GET /api/v1/ecosystem/service-check", "ecosystem", "synced"),
        "body_mesh_query" => ("agent-body mesh", "GET /api/v1/mesh/query", "governance", "synced"),
        "body_demo_scenario" => ("agent-body demo", "POST /api/v1/demo/run", "demo", "synced"),
        "body_benchmark" => ("agent-body bench", "POST /api/v1/benchmark/run", "benchmark", "synced"),
        "body_feedback" => ("agent-body feedback", "GET/POST /api/v1/feedback", "benchmark", "synced"),
        "body_audit_query" => ("agent-body audit", "GET /api/v1/audit/events", "audit", "synced"),
        "body_state" => ("agent-body body", "GET /api/v1/body/manifest", "body", "synced"),
        _ => ("", "", "", "missing"),
    }
}

/// 处理 GET /api/v1/parity/matrix (黑白屏全能力对齐自检矩阵)
pub fn handle_parity_matrix(storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let registry = McpToolRegistry::new(Storage::new(storage));
    let tools = registry.list_tools();
    let mut items = Vec::with_capacity(tools.len());
    let mut synced_count = 0usize;

    for t in &tools {
        let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let desc = t.get("description").and_then(|v| v.as_str()).unwrap_or("");
        let (cli_cmd, web_ep, workbench, status) = map_tool_parity(name);
        if status == "synced" {
            synced_count += 1;
        }
        items.push(serde_json::json!({
            "mcp_tool": name,
            "description": desc,
            "cli_command": cli_cmd,
            "web_endpoint": web_ep,
            "web_workbench": workbench,
            "status": status,
        }));
    }

    let total = tools.len();
    let coverage_pct = if total == 0 {
        100.0
    } else {
        (synced_count as f64 / total as f64) * 100.0
    };

    WebResponse::success(serde_json::json!({
        "total_mcp_tools": total,
        "synced_count": synced_count,
        "missing_count": total.saturating_sub(synced_count),
        "coverage_pct": coverage_pct,
        "items": items,
    }))
}

/* ─── Phase 22: 路演演示剧场沙盒 API ─────────────────────────────── */

pub fn handle_demo_scenarios() -> HttpResponse {
    WebResponse::success(crate::demo::DemoEngine::list_scenarios())
}

#[derive(Debug, Deserialize)]
pub struct DemoRunBody {
    pub scenario_id: Option<String>,
}

pub fn handle_demo_run(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let body: DemoRunBody = match req.json() {
        Ok(b) => b,
        Err(_) => DemoRunBody { scenario_id: None },
    };
    let id = body
        .scenario_id
        .or_else(|| req.query_params.get("id").cloned())
        .unwrap_or_else(|| "scene-1".to_string());
    match crate::demo::DemoEngine::run_scenario(storage, &id) {
        Ok(res) => WebResponse::success(res),
        Err(e) => WebResponse::error(
            StatusCode::BAD_REQUEST,
            &format!("Demo run failed: {}", e),
        ),
    }
}

pub fn handle_demo_reset(storage: &dyn StorageProvider) -> HttpResponse {
    match crate::demo::DemoEngine::reset_sandbox(storage) {
        Ok(cleaned) => WebResponse::success(serde_json::json!({ "cleaned": cleaned })),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Demo reset failed: {}", e),
        ),
    }
}

pub fn handle_demo_roi(storage: &dyn StorageProvider) -> HttpResponse {
    match crate::demo::DemoEngine::compute_roi(storage) {
        Ok(roi) => WebResponse::success(roi),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("ROI compute failed: {}", e),
        ),
    }
}

/* ─── Phase 28: 实证 Benchmark 与生产效能存证账本 API ──────────────────── */

pub fn handle_benchmark_workloads() -> HttpResponse {
    WebResponse::success(serde_json::json!({
        "workloads": crate::benchmark::workloads::list_workload_specs(),
        "provenance_rules": crate::benchmark::provenance::ProvenanceRule::list_all(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct BenchmarkRunBody {
    pub suite: Option<String>,
    pub save: Option<bool>,
}

pub fn handle_benchmark_run(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let body: BenchmarkRunBody = match req.json() {
        Ok(b) => b,
        Err(_) => BenchmarkRunBody {
            suite: None,
            save: None,
        },
    };
    let suite = body
        .suite
        .or_else(|| req.query_params.get("suite").cloned())
        .unwrap_or_else(|| "full".to_string());
    let save = body.save.unwrap_or(true);
    match crate::benchmark::BenchmarkEngine::run_suite(storage, &suite, save, None) {
        Ok(report) => WebResponse::success(report),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Benchmark run failed: {}", e),
        ),
    }
}

pub fn handle_benchmark_report(req: &HttpRequest, storage: &dyn StorageProvider) -> HttpResponse {
    let run_id = req.query_params.get("run_id").map(|s| s.as_str());
    let history = storage.list_benchmark_runs(20).unwrap_or_default();
    let rec_opt = if let Some(id) = run_id {
        storage.get_benchmark_run(id).ok().flatten()
    } else {
        history.first().cloned()
    };
    let report = rec_opt.map(|r| crate::benchmark::report::BenchmarkReportPayload::from_record(&r));
    let history_meta: Vec<_> = history
        .iter()
        .map(|r| {
            serde_json::json!({
                "run_id": r.run_id,
                "suite_name": r.suite_name,
                "app_version": r.app_version,
                "git_commit": r.git_commit,
                "overall_token_savings_pct": r.overall_token_savings_pct,
                "overall_speedup_factor": r.overall_speedup_factor,
                "sla_passed": r.sla_passed,
                "report_sha256": r.report_sha256,
                "created_at": r.created_at,
            })
        })
        .collect();
    WebResponse::success(serde_json::json!({
        "latest": report,
        "history": history_meta,
    }))
}

pub fn handle_benchmark_ledger(req: &HttpRequest, storage: &dyn StorageProvider) -> HttpResponse {
    let window = query_str(req, "window", "7d");
    let tool = req.query_params.get("tool").map(|s| s.as_str()).filter(|s| !s.is_empty());
    let limit = query_str(req, "limit", "50")
        .parse::<usize>()
        .unwrap_or(50)
        .clamp(1, 200);
    match storage.query_effectiveness_ledger(&window, tool, limit) {
        Ok(summary) => WebResponse::success(summary),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Ledger query failed: {}", e),
        ),
    }
}

pub fn handle_benchmark_compare(req: &HttpRequest, storage: &dyn StorageProvider) -> HttpResponse {
    let a_id = query_str(req, "a", "");
    let b_id = query_str(req, "b", "");
    if a_id.is_empty() || b_id.is_empty() {
        return WebResponse::error(StatusCode::BAD_REQUEST, "Query parameters 'a' and 'b' are required");
    }
    let a = match storage.get_benchmark_run(&a_id) {
        Ok(Some(r)) => r,
        Ok(None) => return WebResponse::error(StatusCode::NOT_FOUND, &format!("Run not found: {}", a_id)),
        Err(e) => return WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    let b = match storage.get_benchmark_run(&b_id) {
        Ok(Some(r)) => r,
        Ok(None) => return WebResponse::error(StatusCode::NOT_FOUND, &format!("Run not found: {}", b_id)),
        Err(e) => return WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    };
    let diff = crate::benchmark::ledger::EffectivenessLedger::compare_runs(&a, &b);
    WebResponse::success(diff)
}

pub fn handle_benchmark_reset(storage: &dyn StorageProvider) -> HttpResponse {
    match crate::benchmark::BenchmarkEngine::reset_sandbox(storage) {
        Ok(cleaned) => WebResponse::success(serde_json::json!({ "cleaned": cleaned })),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Benchmark sandbox reset failed: {}", e),
        ),
    }
}

pub fn handle_benchmark_competitors(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let track = query_str(req, "track", "all");
    match crate::benchmark::competitors::run_competitor_track(storage, &track) {
        Ok(results) => WebResponse::success(serde_json::json!({
            "spec_version": crate::benchmark::BENCHMARK_SPEC_VERSION,
            "results": results,
            "capability_matrix": crate::benchmark::competitors::build_competitor_capability_matrix(),
        })),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Competitor benchmark failed: {}", e),
        ),
    }
}

