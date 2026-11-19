use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use crate::config::Config;
use crate::ecosystem::{ContextBudgetRouter, ContextRouteRequest};
use crate::fact::assets_catalog::AssetsCatalogEngine;
use crate::fact::automation::AutomationTopology;
use crate::fact::capability::CapabilityEngine;
use crate::fact::process_net::ProcessNetTopology;
use crate::mesh::build_local_facts;
use crate::preflight::PreflightChecker;
use crate::storage::StorageProvider;
use crate::web::http::{HttpRequest, HttpResponse, StatusCode};

/// 标准 Web API 统一响应封套
#[derive(Debug, Serialize, Deserialize)]
pub struct WebResponse<T> {
    pub code: u16,
    pub message: String,
    pub data: Option<T>,
    pub timestamp: String,
}

impl<T: Serialize> WebResponse<T> {
    pub fn success(data: T) -> HttpResponse {
        let resp = Self {
            code: 200,
            message: "success".to_string(),
            data: Some(data),
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        HttpResponse::json(StatusCode::OK, &resp)
    }
}

impl WebResponse<()> {
    pub fn error(status: StatusCode, message: &str) -> HttpResponse {
        let resp: WebResponse<()> = WebResponse {
            code: status.0,
            message: message.to_string(),
            data: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };
        HttpResponse::json(status, &resp)
    }
}

/// 健康检查数据
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
    pub uptime_secs: u64,
    pub storage_backend: String,
}

/// 系统全局概览数据
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemOverview {
    pub hostname: String,
    pub os_name: String,
    pub cpu_cores: usize,
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub active_processes_count: usize,
    pub active_locks_count: usize,
    pub total_assets_count: usize,
    pub memory_items_count: usize,
    pub placement_rules_count: usize,
    pub skills_count: usize,
    pub mcp_servers_count: usize,
    pub external_services_count: usize,
    pub fact_nodes_count: usize,
    pub fact_edges_count: usize,
    pub wait_queue_count: usize,
    pub os_platform: String,
    pub server_time: String,
}

/// 处理 GET /api/v1/health
pub fn handle_health(storage: &dyn StorageProvider, start_time: &std::time::Instant) -> HttpResponse {
    let uptime_secs = start_time.elapsed().as_secs();
    let backend_name = storage.provider_name().to_string();

    let health = HealthStatus {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_secs,
        storage_backend: backend_name,
    };

    WebResponse::success(health)
}

/// 处理 GET /api/v1/overview
pub fn handle_overview(storage: &dyn StorageProvider) -> HttpResponse {
    let mut sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );
    sys.refresh_all();

    let hostname = System::host_name().unwrap_or_else(|| "localhost".to_string());
    let os_name = System::long_os_version().unwrap_or_else(|| "Linux".to_string());
    let cpu_cores = sys.cpus().len();
    let total_memory_mb = sys.total_memory() / (1024 * 1024);
    let used_memory_mb = sys.used_memory() / (1024 * 1024);

    let active_locks_count = storage.list_coordination_locks(None).map(|l| l.len()).unwrap_or(0);
    let total_assets_count = storage.list_assets(5000).map(|a| a.len()).unwrap_or(0);
    let memory_items_count = storage.list_all_active_memories(1000).map(|m| m.len()).unwrap_or(0);
    let active_processes_count = storage.list_active_processes().map(|p| p.len()).unwrap_or(0);
    let placement_rules_count = storage.get_placement_rules().map(|r| r.len()).unwrap_or(0);
    let skills_count = storage.list_skills_catalog().map(|s| s.len()).unwrap_or(0);
    let mcp_servers_count = storage.list_mcp_catalog().map(|m| m.len()).unwrap_or(0);
    let external_services_count = storage.list_external_services(None).map(|s| s.len()).unwrap_or(0);
    let fact_nodes_count = storage.list_all_fact_nodes(2000).map(|n| n.len()).unwrap_or(0);
    let fact_edges_count = storage.list_all_fact_edges(2000).map(|e| e.len()).unwrap_or(0);
    let wait_queue_count = storage.list_lock_wait_queue(None, None).map(|w| w.len()).unwrap_or(0);
    let os_platform = std::env::consts::OS.to_string();

    let overview = SystemOverview {
        hostname,
        os_name,
        cpu_cores,
        total_memory_mb,
        used_memory_mb,
        active_processes_count,
        active_locks_count,
        total_assets_count,
        memory_items_count,
        placement_rules_count,
        skills_count,
        mcp_servers_count,
        external_services_count,
        fact_nodes_count,
        fact_edges_count,
        wait_queue_count,
        os_platform,
        server_time: chrono::Utc::now().to_rfc3339(),
    };

    WebResponse::success(overview)
}

/// 处理 GET /api/v1/facts (宿主机运行时物理事实雷达)
pub fn handle_facts(storage: &dyn StorageProvider) -> HttpResponse {
    let listening_ports = ProcessNetTopology::scan_listening_ports().unwrap_or_default();
    let active_processes: Vec<_> = storage
        .list_active_processes()
        .unwrap_or_default()
        .into_iter()
        .take(120)
        .collect();
    let systemd_services: Vec<_> = AutomationTopology::scan_systemd_services()
        .unwrap_or_default()
        .into_iter()
        .take(60)
        .collect();
    let cron_jobs = AutomationTopology::scan_cron_jobs().unwrap_or_default();
    let repos = AssetsCatalogEngine::scan_repos(None).unwrap_or_default();

    let mut cap_engine = CapabilityEngine::new();
    cap_engine.scan_host();
    let capabilities = cap_engine.all_capabilities();

    WebResponse::success(serde_json::json!({
        "listening_ports": listening_ports,
        "active_processes": active_processes,
        "systemd_services": systemd_services,
        "cron_jobs": cron_jobs,
        "repos": repos,
        "capabilities": capabilities,
    }))
}

/// 处理 GET /api/v1/ecosystem (MCP / SKILL / 外部服务通讯录)
pub fn handle_ecosystem(req: &HttpRequest, storage: &dyn StorageProvider) -> HttpResponse {
    let query = req.query_params.get("q").map(|s| s.trim()).unwrap_or("");

    let mcps = storage.list_mcp_catalog().unwrap_or_default();
    let all_skills = storage.list_skills_catalog().unwrap_or_default();
    let total_skills = all_skills.len();
    let total_skills_est_tokens: usize = all_skills.iter().map(|s| s.est_tokens).sum();

    let skills = if query.is_empty() {
        all_skills
    } else {
        let mut matched = storage.search_skills_catalog(query, 100).unwrap_or_default();
        if matched.is_empty() {
            let q_lower = query.to_lowercase();
            matched = all_skills
                .into_iter()
                .filter(|s| {
                    s.skill_name.to_lowercase().contains(&q_lower)
                        || s.description.to_lowercase().contains(&q_lower)
                })
                .collect();
        }
        matched
    };

    let services = storage.list_external_services(None).unwrap_or_default();

    WebResponse::success(serde_json::json!({
        "mcps": mcps,
        "skills": skills,
        "services": services,
        "summary": {
            "total_mcps": mcps.len(),
            "total_skills": total_skills,
            "total_skills_est_tokens": total_skills_est_tokens,
            "total_services": services.len(),
        }
    }))
}

/// 处理 GET /api/v1/ecosystem/route?intent=...&budget=600 (Context 防爆路由器演练)
pub fn handle_ecosystem_route(req: &HttpRequest, storage: &dyn StorageProvider) -> HttpResponse {
    let intent = req
        .query_params
        .get("intent")
        .map(|s| urlencoding::decode(s).map(|d| d.into_owned()).unwrap_or_else(|_| s.clone()))
        .unwrap_or_default();

    let budget = req
        .query_params
        .get("budget")
        .and_then(|b| b.parse::<usize>().ok())
        .unwrap_or(600)
        .clamp(150, 4000);

    let route_req = ContextRouteRequest {
        intent,
        budget_tokens: budget,
        max_skills: 5,
        max_mcps: 5,
        max_services: 4,
    };

    match ContextBudgetRouter::route_with_provider(storage, &route_req) {
        Ok(plan) => WebResponse::success(plan),
        Err(e) => WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to route ecosystem context: {}", e),
        ),
    }
}

/// 处理 GET /api/v1/governance (放置规则、扫描策略、记忆、变更审计与环境预检)
pub fn handle_governance(req: &HttpRequest, storage: &dyn StorageProvider) -> HttpResponse {
    let query = req
        .query_params
        .get("q")
        .map(|s| urlencoding::decode(s).map(|d| d.into_owned()).unwrap_or_else(|_| s.clone()))
        .unwrap_or_default();

    let placement_rules = storage.get_placement_rules().unwrap_or_default();
    let scan_policies = storage.get_scan_policies().unwrap_or_default();
    let memories = if query.trim().is_empty() {
        storage.list_all_active_memories(50).unwrap_or_default()
    } else {
        storage.search_memories_fts(query.trim(), 50).unwrap_or_default()
    };
    let recent_changes = storage.list_changes(80).unwrap_or_default();

    let cfg = Config::load_or_default(None).unwrap_or_default();
    let preflight = PreflightChecker::run_all(&cfg.data_dir, Some(&cfg));

    WebResponse::success(serde_json::json!({
        "placement_rules": placement_rules,
        "scan_policies": scan_policies,
        "memories": memories,
        "recent_changes": recent_changes,
        "preflight": preflight,
    }))
}

/// 处理 GET /api/v1/mesh/status (本机跨平台探针与多机 Peer Mesh 态势)
pub fn handle_mesh_status(storage: &dyn StorageProvider) -> HttpResponse {
    let cfg = Config::load_or_default(None).unwrap_or_default();
    let probe = crate::platform::current_probe();
    let local_facts = build_local_facts();
    let active_locks_count = storage.list_coordination_locks(None).map(|l| l.len()).unwrap_or(0);

    WebResponse::success(serde_json::json!({
        "node_id": local_facts.node_id,
        "mesh_enabled": cfg.mesh.enabled,
        "timeout_ms": cfg.mesh.timeout_ms,
        "peers": cfg.mesh.peers,
        "platform_os": probe.name(),
        "local_facts": local_facts,
        "active_locks_count": active_locks_count,
    }))
}
