use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use crate::storage::StorageProvider;
use crate::web::http::{HttpResponse, StatusCode};

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

    // 统计锁数量
    let active_locks_count = storage.list_coordination_locks(None).map(|l| l.len()).unwrap_or(0);

    // 统计资产数量
    let total_assets_count = storage.list_assets(1000).map(|a| a.len()).unwrap_or(0);

    // 统计记忆数量
    let memory_items_count = storage.search_memories_fts("", 1000).map(|m| m.len()).unwrap_or(0);

    // 统计活跃进程数
    let active_processes_count = storage.list_active_processes().map(|p| p.len()).unwrap_or(0);

    // 统计放置规则数
    let placement_rules_count = storage.get_placement_rules().map(|r| r.len()).unwrap_or(0);

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
        server_time: chrono::Utc::now().to_rfc3339(),
    };

    WebResponse::success(overview)
}
