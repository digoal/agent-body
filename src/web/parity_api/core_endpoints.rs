use super::{query_str};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use serde::Deserialize;
use crate::api::hooks::AgentHookEngine;
use crate::brief::{BriefAssembler, BriefRequest};
use crate::config::Config;
use crate::env::port::PortChecker;
use crate::fact::FactFacade;
use crate::hooks::{ClientHookGenerator, ClientType};
use crate::links::audit::LinkAuditor;
use crate::links::codegraph_bridge::CodeGraphBridge;
use crate::links::repair::LinkRepairer;
use crate::links::LinksEngine;
use crate::placement::learner::PlacementLearner;
use crate::placement::{PlacementEngine, PlacementRequest};
use crate::scanner::Scanner;
use crate::storage::{AssetRecord, Storage, StorageProvider};
use crate::web::api::WebResponse;
use crate::web::http::{HttpRequest, HttpResponse, StatusCode};

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
    let provider = Config::load_effective_or_default().build_memory_provider(st.clone());
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
    let opt_param = |key: &str| -> Option<String> {
        let v = query_str(req, key, "");
        if v.trim().is_empty() { None } else { Some(v) }
    };
    let hybrid_raw = query_str(req, "hybrid", "false");
    let hybrid = matches!(hybrid_raw.trim().to_ascii_lowercase().as_str(), "true" | "1" | "yes");

    let mut opts = crate::scanner::AssetSearchOptions::new(q.trim(), limit);
    opts.modality = opt_param("modality");
    opts.domain = opt_param("domain");
    opts.kv_key = opt_param("kv_key");
    opts.kv_value = opt_param("kv_value");
    opts.hybrid = hybrid;

    match crate::scanner::AssetSearchEngine::search(storage, &opts) {
        Ok(hits) => {
            let projected: Vec<serde_json::Value> = hits
                .iter()
                .map(|h| {
                    let mut view = crate::scanner::project_asset_view(&h.asset, level);
                    if let Some(obj) = view.as_object_mut() {
                        obj.insert("search_score".to_string(), serde_json::json!(h.score));
                        obj.insert(
                            "matched_channels".to_string(),
                            serde_json::json!(h.matched_channels),
                        );
                    }
                    view
                })
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
    let default_owner = Config::load_effective_or_default().effective_default_owner();
    let rec = AssetRecord {
        id: uuid::Uuid::new_v4().to_string(),
        path: body.path.trim().to_string(),
        asset_type: body.asset_type.unwrap_or_else(|| "other".to_string()),
        owner: body.owner.unwrap_or(default_owner),
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

