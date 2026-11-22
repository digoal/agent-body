use std::path::{Path, PathBuf};
use std::sync::Arc;
use serde::Deserialize;

use crate::api::hooks::AgentHookEngine;
use crate::api::mcp_tools::McpToolRegistry;
use crate::brief::{BriefAssembler, BriefRequest};
use crate::config::Config;
use crate::coordination::{CoordinationEngine, LockRequest, LockScope, ResourceType};
use crate::ecosystem::ExternalContactsManager;
use crate::env::gc::HostGarbageCollector;
use crate::env::port::PortChecker;
use crate::fact::FactFacade;
use crate::hooks::{ClientHookGenerator, ClientType};
use crate::links::audit::LinkAuditor;
use crate::links::codegraph_bridge::CodeGraphBridge;
use crate::links::repair::LinkRepairer;
use crate::links::LinksEngine;
use crate::memory::builtin_provider::BuiltinMemoryProvider;
use crate::memory::hybrid::HybridSearchEngine;
use crate::memory::lifecycle::MemoryLifecycleManager;
use crate::memory::MemoryManager;
use crate::mesh::MeshClient;
use crate::placement::learner::PlacementLearner;
use crate::placement::{PlacementEngine, PlacementRequest};
use crate::scanner::Scanner;
use crate::session::SessionManager;
use crate::storage::{AnalyticsProvider, AssetRecord, DuckDbAnalyticsEngine, Storage, StorageProvider};
use crate::web::api::WebResponse;
use crate::web::http::{HttpRequest, HttpResponse, StatusCode};

fn query_str(req: &HttpRequest, key: &str, default_val: &str) -> String {
    req.query_params
        .get(key)
        .map(|s| urlencoding::decode(s).map(|d| d.into_owned()).unwrap_or_else(|_| s.clone()))
        .unwrap_or_else(|| default_val.to_string())
}

fn check_auth(req: &HttpRequest, auth_token: &Option<String>) -> Option<HttpResponse> {
    if let Some(expected_token) = auth_token {
        let auth_header = req.headers.get("authorization").cloned().unwrap_or_default();
        let token = auth_header.strip_prefix("Bearer ").unwrap_or(&auth_header);
        if token != expected_token {
            return Some(WebResponse::error(
                StatusCode::UNAUTHORIZED,
                "Unauthorized: invalid auth token",
            ));
        }
    }
    None
}

/* ─── 21.1.1 领域 A/B/C: Brief / Placement / Assets / Facts / Links ─── */

pub fn handle_brief(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let t0 = std::time::Instant::now();
    let query = query_str(req, "intent", "");
    let session_id = query_str(req, "session_id", "web-console");
    let budget = query_str(req, "budget", "1500")
        .parse::<usize>()
        .unwrap_or(1500)
        .clamp(300, 8000);

    let storage_ref = storage.clone();
    let st = Storage::new(storage);
    let provider = Arc::new(BuiltinMemoryProvider::new(st.clone()));
    let assembler = BriefAssembler::new(st, Some(provider));
    let brief_req = BriefRequest {
        query,
        session_id,
        budget_tokens: budget,
    };
    match assembler.assemble(&brief_req) {
        Ok(output) => {
            crate::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage_ref.as_ref(),
                "web",
                "body_brief",
                &output.markdown,
                t0.elapsed().as_micros() as i64,
            );
            WebResponse::success(serde_json::json!({
                "markdown": output.markdown,
                "budget_tokens": budget,
                "quality": output.quality,
            }))
        }
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to assemble brief: {}", e),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct PlacementSuggestBody {
    pub object_type: Option<String>,
    pub name: String,
    pub intent: Option<String>,
    pub security_level: Option<String>,
}

pub fn handle_placement_suggest(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let body: PlacementSuggestBody = match req.json() {
        Ok(b) => b,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };
    let st = Storage::new(storage);
    let engine = PlacementEngine::new(st);
    let preq = PlacementRequest {
        object_type: body.object_type.unwrap_or_else(|| "script".to_string()),
        name: body.name,
        intent: body.intent.unwrap_or_default(),
        tags: vec![],
        security_level: body.security_level.unwrap_or_else(|| "internal".to_string()),
    };
    match engine.suggest(&preq) {
        Ok(s) => WebResponse::success(s),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Placement suggest failed: {}", e),
        ),
    }
}

pub fn handle_placement_learn(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let min_samples = query_str(req, "min_samples", "3")
        .parse::<usize>()
        .unwrap_or(3)
        .clamp(1, 50);
    let st = Storage::new(storage);
    let learner = PlacementLearner::new(&st);
    match learner.learn_rules(min_samples) {
        Ok(rep) => WebResponse::success(rep),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Placement learn failed: {}", e),
        ),
    }
}

pub fn handle_assets_search(req: &HttpRequest, storage: &dyn StorageProvider) -> HttpResponse {
    let t0 = std::time::Instant::now();
    let q = query_str(req, "q", "");
    let limit = query_str(req, "limit", "40")
        .parse::<usize>()
        .unwrap_or(40)
        .clamp(1, 200);
    let detail_param = query_str(req, "detail", "standard");
    let level = crate::scanner::DetailLevel::from_str_opt(Some(&detail_param));

    let res = if q.trim().is_empty() {
        storage.list_assets(limit)
    } else {
        storage.search_assets_fts(q.trim(), limit)
    };
    match res {
        Ok(list) => {
            let projected: Vec<serde_json::Value> = list
                .iter()
                .map(|a| crate::scanner::project_asset_view(a, level))
                .collect();
            let summary_txt = serde_json::to_string(&projected).unwrap_or_default();
            crate::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage,
                "web",
                "body_asset_search",
                &summary_txt,
                t0.elapsed().as_micros() as i64,
            );
            WebResponse::success(projected)
        }
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Asset search failed: {}", e),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct AssetRegisterBody {
    pub path: String,
    pub asset_type: Option<String>,
    pub owner: Option<String>,
    pub security_level: Option<String>,
    pub description: Option<String>,
}

pub fn handle_assets_register(req: &HttpRequest, storage: &dyn StorageProvider) -> HttpResponse {
    let body: AssetRegisterBody = match req.json() {
        Ok(b) => b,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };
    if body.path.trim().is_empty() {
        return WebResponse::error(StatusCode::BAD_REQUEST, "path must not be empty");
    }
    let rec = AssetRecord {
        id: uuid::Uuid::new_v4().to_string(),
        path: body.path.trim().to_string(),
        asset_type: body.asset_type.unwrap_or_else(|| "other".to_string()),
        owner: body.owner.unwrap_or_else(|| "user:digoal".to_string()),
        security_level: body.security_level.unwrap_or_else(|| "internal".to_string()),
        scan_tier: "deep".to_string(),
        description: body.description,
        desc_kv: serde_json::json!({"source": "web_console"}),
        usage: None,
        size_bytes: None,
        file_count: None,
        tags: vec![],
        status: "active".to_string(),
    };
    match storage.upsert_asset(&rec) {
        Ok(_) => WebResponse::success(rec),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Asset register failed: {}", e),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct AssetScanBody {
    pub path: String,
    #[serde(default)]
    pub enrich_with_ollama: bool,
    #[serde(default = "default_enrich_limit")]
    pub enrich_limit: usize,
}

fn default_enrich_limit() -> usize {
    10
}

pub fn handle_assets_scan(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let body: AssetScanBody = match req.json() {
        Ok(b) => b,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };
    if body.path.trim().is_empty() {
        return WebResponse::error(StatusCode::BAD_REQUEST, "path must not be empty");
    }
    let st = Storage::new(storage);
    let scanner = match Scanner::new(st.clone()) {
        Ok(s) => s,
        Err(e) => {
            return WebResponse::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("Scanner init failed: {}", e),
            )
        }
    };
    match scanner.scan_path(&PathBuf::from(body.path.trim())) {
        Ok(report) => {
            let enrich_report = if body.enrich_with_ollama {
                let annotator = crate::scanner::OllamaAnnotator::with_default_config(st);
                annotator
                    .enrich_pending_assets(body.enrich_limit.clamp(1, 50), Some(body.path.trim()))
                    .ok()
            } else {
                None
            };
            WebResponse::success(serde_json::json!({
                "scan_report": report,
                "enrich_report": enrich_report,
            }))
        }
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Scan failed: {}", e),
        ),
    }
}

pub fn handle_facts_port_check(req: &HttpRequest) -> HttpResponse {
    let port = query_str(req, "port", "8080")
        .parse::<u16>()
        .unwrap_or(8080);
    let suggest_count = query_str(req, "suggest_count", "3")
        .parse::<usize>()
        .unwrap_or(3)
        .clamp(1, 10);
    match PortChecker::check_port(port, suggest_count) {
        Ok(res) => WebResponse::success(res),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Port check failed: {}", e),
        ),
    }
}

pub fn handle_facts_query(req: &HttpRequest) -> HttpResponse {
    let query = query_str(req, "q", "all");
    let fact_type = query_str(req, "type", "auto");
    match FactFacade::query_facts(&fact_type, &query) {
        Ok(answer) => WebResponse::success(serde_json::json!({
            "query": query,
            "fact_type": fact_type,
            "answer": answer,
        })),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Fact query failed: {}", e),
        ),
    }
}

pub fn handle_links_audit(storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let t0 = std::time::Instant::now();
    let storage_ref = storage.clone();
    let auditor = LinkAuditor::new(Arc::new(Storage::new(storage)));
    match auditor.audit_links(false) {
        Ok(rep) => {
            crate::benchmark::ledger::EffectivenessLedger::record_channel_call(
                storage_ref.as_ref(),
                "web",
                "body_links_audit",
                &rep.summary,
                t0.elapsed().as_micros() as i64,
            );
            WebResponse::success(rep)
        }
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Links audit failed: {}", e),
        ),
    }
}

pub fn handle_links_repair(storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let repairer = LinkRepairer::new(Arc::new(Storage::new(storage)));
    match repairer.repair_links(false) {
        Ok(rep) => WebResponse::success(rep),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Links repair preview failed: {}", e),
        ),
    }
}

pub fn handle_links_mermaid(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let limit = query_str(req, "limit", "25")
        .parse::<usize>()
        .unwrap_or(25)
        .clamp(5, 100);
    let discover_semantic = query_str(req, "discover_semantic", "false") == "true";
    let st = Storage::new(storage);
    let mut mining_summary = None;
    if discover_semantic {
        if let Ok(rep) = crate::fact::GraphRelationshipMiner::new(&st).mine_all(true) {
            mining_summary = Some(rep);
        }
    }
    let engine = LinksEngine::new(Arc::new(st));
    match engine.export_mermaid(limit) {
        Ok(mermaid) => WebResponse::success(serde_json::json!({
            "limit": limit,
            "discover_semantic": discover_semantic,
            "mining_report": mining_summary,
            "mermaid": mermaid,
        })),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Mermaid export failed: {}", e),
        ),
    }
}

pub fn handle_links_codegraph(req: &HttpRequest) -> HttpResponse {
    let path_str = query_str(req, "path", "");
    if path_str.trim().is_empty() {
        return WebResponse::error(StatusCode::BAD_REQUEST, "path parameter is required");
    }
    let symbol = query_str(req, "symbol", "");
    let p = Path::new(path_str.trim());
    if !symbol.trim().is_empty() {
        match CodeGraphBridge::query_symbol_callers(p, symbol.trim()) {
            Ok(callers) => WebResponse::success(serde_json::json!({
                "path": path_str,
                "symbol": symbol,
                "callers_count": callers.len(),
                "callers": callers,
            })),
            Err(e) => WebResponse::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("CodeGraph symbol query failed: {}", e),
            ),
        }
    } else {
        match CodeGraphBridge::inspect_project(p) {
            Ok(info) => WebResponse::success(serde_json::json!({
                "path": path_str,
                "has_codegraph": info.is_some(),
                "project_info": info,
            })),
            Err(e) => WebResponse::error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("CodeGraph inspect failed: {}", e),
            ),
        }
    }
}

/* ─── 21.1.2 领域 D/E/F: Hooks / Analytics / Memory / GC / Locks / Mesh / Parity ─── */

#[derive(Debug, Deserialize)]
pub struct HookCallBody {
    pub tool_name: String,
    pub args: Option<serde_json::Value>,
    pub success: Option<bool>,
    pub error_output: Option<String>,
}

pub fn handle_hooks_pre(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let body: HookCallBody = match req.json() {
        Ok(b) => b,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };
    let engine = AgentHookEngine::new(Storage::new(storage));
    let args = body.args.unwrap_or_else(|| serde_json::json!({}));
    let res = engine.pre_tool_call(&body.tool_name, &args);
    WebResponse::success(res)
}

pub fn handle_hooks_post(req: &HttpRequest, storage: Arc<dyn StorageProvider>) -> HttpResponse {
    let body: HookCallBody = match req.json() {
        Ok(b) => b,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid JSON: {}", e)),
    };
    let engine = AgentHookEngine::new(Storage::new(storage));
    let args = body.args.unwrap_or_else(|| serde_json::json!({}));
    let res = engine.post_tool_call_with_error(
        &body.tool_name,
        &args,
        body.success.unwrap_or(true),
        body.error_output.as_deref(),
    );
    WebResponse::success(res)
}

pub fn handle_hooks_generate(req: &HttpRequest) -> HttpResponse {
    let client_str = query_str(req, "client", "agy");
    let client_type = match client_str.parse::<ClientType>() {
        Ok(c) => c,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid client: {}", e)),
    };
    match ClientHookGenerator::generate(client_type, None, true) {
        Ok(res) => WebResponse::success(res),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Hook generate failed: {}", e),
        ),
    }
}

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
    let table = body.table.unwrap_or_else(|| "assets".to_string());
    let default_path = format!("/root/new/tmp/steward_{}.parquet", table);
    let output_path = body.output_path.unwrap_or(default_path);
    if !output_path.starts_with("/root/new/tmp/") || output_path.contains("..") {
        return WebResponse::error(
            StatusCode::FORBIDDEN,
            "Export path must reside inside /root/new/tmp/",
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
    // Web 端默认强制 dry_run=true，绝不静默硬删除
    match HostGarbageCollector::run_gc(Path::new("/root/new/tmp"), max_age_hours, true) {
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
        "body_links_visualize" => ("agent-body links visualize", "GET /api/v1/links/mermaid", "topology", "synced"),
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
        "body_lock_deadlock_check" => ("agent-body coordination deadlock-check", "GET /api/v1/locks/deadlocks", "locks", "synced"),
        "body_mcp_catalog" => ("agent-body mcp-catalog", "GET /api/v1/ecosystem", "ecosystem", "synced"),
        "body_skill_route" => ("agent-body skill route", "GET /api/v1/ecosystem/route", "ecosystem", "synced"),
        "body_service_contact" => ("agent-body service", "GET /api/v1/ecosystem/service-check", "ecosystem", "synced"),
        "body_mesh_query" => ("agent-body mesh", "GET /api/v1/mesh/query", "governance", "synced"),
        "body_demo_scenario" => ("agent-body demo", "POST /api/v1/demo/run", "demo", "synced"),
        "body_benchmark" => ("agent-body bench", "POST /api/v1/benchmark/run", "benchmark", "synced"),
        "body_feedback" => ("agent-body feedback", "GET/POST /api/v1/feedback", "benchmark", "synced"),
        "body_audit_query" => ("agent-body audit", "GET/POST /api/v1/audit/*", "audit", "synced"),
        "body_state" => ("agent-body body", "GET/POST /api/v1/body/*", "body", "synced"),
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
    let out_path = body
        .output_path
        .unwrap_or_else(|| "/root/new/tmp/audit_events_export.csv".to_string());
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

    // 1. 若指定了 report_id，直接加载该历史报告；若 view_only=true 且已有历史报告，默认加载最新一份历史报告而不重复新建
    let rep_res = if let Some(rid) = report_id {
        crate::body::BodyPerfEngine::load_awr_report(storage_ref.as_ref(), rid)
            .ok_or_else(|| anyhow::anyhow!("未找到历史 AWR 报告: {}", rid))
    } else if view_only {
        let existing_reports = crate::body::BodyPerfEngine::list_awr_reports_meta(storage_ref.as_ref());
        if let Some(first) = existing_reports.first() {
            crate::body::BodyPerfEngine::load_awr_report(storage_ref.as_ref(), &first.report_id)
                .ok_or_else(|| anyhow::anyhow!("加载最新历史 AWR 报告失败"))
        } else {
            crate::body::BodyPerfEngine::generate_awr_report(&st, snap_a, snap_b, vs_baseline)
        }
    } else {
        crate::body::BodyPerfEngine::generate_awr_report(&st, snap_a, snap_b, vs_baseline)
    };

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
