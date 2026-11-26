use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

use crate::storage::StorageProvider;
use crate::web::http::{HttpRequest, HttpResponse, Method, StatusCode};
use crate::web::api;
use crate::web::assets;
use crate::web::parity_api;

/// Web 服务器配置
#[derive(Debug, Clone)]
pub struct WebConfig {
    pub host: String,
    pub port: u16,
    pub auth_token: Option<String>,
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 8765,
            auth_token: None,
        }
    }
}

impl WebConfig {
    /// 判断是否绑定在非回环地址（如 0.0.0.0 或局域网 IP）
    pub fn is_non_loopback(&self) -> bool {
        !matches!(self.host.trim(), "127.0.0.1" | "localhost" | "::1")
    }
}

/// 恒定时间字节比较，降低 Bearer Token 时序侧信道风险
fn constant_time_eq(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in ab.iter().zip(bb.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// 统一校验写端点与敏感端点的 Bearer Token 鉴权
fn check_write_or_sensitive_auth(
    req: &HttpRequest,
    auth_token: &Option<String>,
    require_token_if_empty: bool,
) -> Option<HttpResponse> {
    let is_mermaid_write = req.path == "/api/v1/links/mermaid"
        && req
            .query_params
            .get("discover_semantic")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false);
    let is_protected = matches!(req.method, Method::Post | Method::Put | Method::Delete)
        || req.path == "/api/v1/env/gc"
        || is_mermaid_write;
    if !is_protected {
        return None;
    }

    match auth_token.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(expected) => {
            let auth_header = req.headers.get("authorization").cloned().unwrap_or_default();
            let token = auth_header.strip_prefix("Bearer ").unwrap_or(&auth_header);
            if !constant_time_eq(token, expected) {
                return Some(crate::web::api::WebResponse::error(
                    StatusCode::UNAUTHORIZED,
                    "Unauthorized: invalid or missing Bearer auth_token",
                ));
            }
            None
        }
        None if require_token_if_empty => Some(crate::web::api::WebResponse::error(
            StatusCode::FORBIDDEN,
            "Forbidden: write and sensitive endpoints require [web].auth_token when bound to non-loopback address",
        )),
        None => None,
    }
}

/// Web 服务器实例
pub struct WebServer {
    config: WebConfig,
    storage: Arc<dyn StorageProvider>,
    start_time: Instant,
    event_tx: broadcast::Sender<String>,
}

impl WebServer {
    pub fn new(config: WebConfig, storage: Arc<dyn StorageProvider>) -> Self {
        let (event_tx, _) = broadcast::channel(128);
        Self {
            config,
            storage,
            start_time: Instant::now(),
            event_tx,
        }
    }

    /// 获取事件广播发送器
    pub fn event_broadcaster(&self) -> broadcast::Sender<String> {
        self.event_tx.clone()
    }

    /// 广播事件给所有 SSE 客户端
    pub fn broadcast_event(&self, event: &str) {
        let _ = self.event_tx.send(event.to_string());
    }

    /// 启动 HTTP 服务主循环
    pub async fn run(&self) -> anyhow::Result<()> {
        let addr = format!("{}:{}", self.config.host, self.config.port);
        let listener = TcpListener::bind(&addr).await?;

        println!("╭────────────────────────────────────────────────────────────╮");
        println!("│  🌐 Agent Body Web Dashboard                       │");
        println!("│  URL: http://{:<45} │", addr);
        println!("╰────────────────────────────────────────────────────────────╯");

        let non_loopback = self.config.is_non_loopback();
        let has_token = self
            .config
            .auth_token
            .as_deref()
            .map(|s| !s.trim().is_empty())
            .unwrap_or(false);
        if non_loopback && !has_token {
            eprintln!(
                "[Web][SECURITY WARN] 服务绑定在非回环地址 `{}` 且未配置 [web].auth_token，所有写端点与 /api/v1/env/gc 将强制返回 403 Forbidden！",
                self.config.host
            );
        }

        let storage = self.storage.clone();
        let start_time = self.start_time;
        let event_tx = self.event_tx.clone();
        let auth_token = self.config.auth_token.clone();

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let storage = storage.clone();
                    let event_tx = event_tx.clone();
                    let auth_token = auth_token.clone();
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_connection(
                            stream,
                            storage,
                            start_time,
                            event_tx,
                            auth_token,
                            non_loopback,
                        )
                        .await
                        {
                            eprintln!("[Web] Connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    eprintln!("[Web] Accept failed: {}", e);
                }
            }
        }
    }

    /// 处理单次 TCP 连接
    async fn handle_connection(
        mut stream: TcpStream,
        storage: Arc<dyn StorageProvider>,
        start_time: Instant,
        event_tx: broadcast::Sender<String>,
        auth_token: Option<String>,
        enforce_token_for_writes: bool,
    ) -> anyhow::Result<()> {
        const MAX_HEADER_BYTES: usize = 32 * 1024;
        let mut buffer = [0u8; 4096];
        let mut raw: Vec<u8> = Vec::with_capacity(4096);

        // 循环读取直到发现完整 HTTP Header 结束标记 `\r\n\r\n`（兼容分包头与 <= 32KB 大请求头）
        let mut header_end_opt = None;
        loop {
            let timeout_dur = if raw.is_empty() {
                std::time::Duration::from_millis(2000)
            } else {
                std::time::Duration::from_millis(500)
            };
            match tokio::time::timeout(timeout_dur, stream.read(&mut buffer)).await {
                Ok(Ok(0)) => {
                    if raw.is_empty() {
                        return Ok(());
                    }
                    break;
                }
                Ok(Ok(n)) => {
                    raw.extend_from_slice(&buffer[..n]);
                    if let Some(pos) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
                        header_end_opt = Some(pos);
                        break;
                    }
                    if raw.len() > MAX_HEADER_BYTES {
                        let resp = HttpResponse::bad_request("Request headers exceed 32KB limit");
                        stream.write_all(&resp.to_bytes()).await?;
                        stream.flush().await?;
                        return Ok(());
                    }
                }
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => break,
            }
        }

        // 若请求体分包到达 (如 reqwest 先发头后发体)，按 Content-Length 补读剩余请求体
        // 安全边界: 仅 /api/v1/mesh/transfer 放宽至 15MB (承载 10MB + Base64 信封)，其余一律限制 64KB 防 DoS
        if let Some(header_end) = header_end_opt {
            if header_end > MAX_HEADER_BYTES {
                let resp = HttpResponse::bad_request("Request headers exceed 32KB limit");
                stream.write_all(&resp.to_bytes()).await?;
                stream.flush().await?;
                return Ok(());
            }
            let header_text = String::from_utf8_lossy(&raw[..header_end]).to_lowercase();
            let is_mesh_transfer = header_text.starts_with("post /api/v1/mesh/transfer");
            let max_body_limit = if is_mesh_transfer { 15 * 1024 * 1024 } else { 65536 };
            let content_len = header_text
                .lines()
                .find_map(|l| l.strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok()))
                .unwrap_or(0)
                .min(max_body_limit);
            let body_start = header_end + 4;
            while raw.len() < body_start + content_len {
                match tokio::time::timeout(std::time::Duration::from_millis(500), stream.read(&mut buffer)).await {
                    Ok(Ok(0)) | Err(_) => break,
                    Ok(Ok(n)) => raw.extend_from_slice(&buffer[..n]),
                    Ok(Err(e)) => return Err(e.into()),
                }
            }
        }

        let req = match HttpRequest::parse(&raw) {
            Ok(r) => r,
            Err(e) => {
                let resp = HttpResponse::bad_request(&format!("Invalid HTTP request: {}", e));
                stream.write_all(&resp.to_bytes()).await?;
                stream.flush().await?;
                return Ok(());
            }
        };

        let origin_hdr = req.headers.get("origin").cloned();

        // 处理 CORS OPTIONS preflight
        if req.method == Method::Options {
            let mut resp = HttpResponse::new(StatusCode::NO_CONTENT);
            resp.apply_cors_for_origin(origin_hdr.as_deref());
            stream.write_all(&resp.to_bytes()).await?;
            stream.flush().await?;
            return Ok(());
        }

        // 处理 SSE 实时流
        if req.method == Method::Get && req.path == "/api/v1/stream" {
            return Self::handle_sse_stream(stream, event_tx, origin_hdr.as_deref()).await;
        }

        // 统一路由分发 (在 spawn_blocking 中执行，避免阻塞 Tokio worker 并兼容同步预检/DB调用)
        let is_demo_run = req.method == Method::Post && req.path == "/api/v1/demo/run";
        let is_bench_run = req.method == Method::Post && req.path == "/api/v1/benchmark/run";
        let demo_path = req.path.clone();
        let mut resp = tokio::task::spawn_blocking(move || {
            Self::route_request_arc_with_policy(
                &req,
                storage,
                &start_time,
                &auth_token,
                enforce_token_for_writes,
            )
        })
        .await?;
        resp.apply_cors_for_origin(origin_hdr.as_deref());
        if (is_demo_run || is_bench_run) && resp.status == StatusCode::OK {
            let _ = event_tx.send(
                serde_json::json!({
                    "type": if is_bench_run { "benchmark_run" } else { "demo_run" },
                    "path": demo_path,
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                })
                .to_string(),
            );
        }
        stream.write_all(&resp.to_bytes()).await?;
        stream.flush().await?;
        Ok(())
    }

    /// 持有 Arc<dyn StorageProvider> 的完整路由分发（默认本机回环策略）
    pub fn route_request_arc(
        req: &HttpRequest,
        storage: Arc<dyn StorageProvider>,
        start_time: &Instant,
        auth_token: &Option<String>,
    ) -> HttpResponse {
        Self::route_request_arc_with_policy(req, storage, start_time, auth_token, false)
    }

    /// 持有 Arc<dyn StorageProvider> 的完整路由分发（支持非回环强制鉴权策略）
    pub fn route_request_arc_with_policy(
        req: &HttpRequest,
        storage: Arc<dyn StorageProvider>,
        start_time: &Instant,
        auth_token: &Option<String>,
        enforce_token_for_writes: bool,
    ) -> HttpResponse {
        let t0 = Instant::now();
        let should_audit = matches!(req.method, Method::Post | Method::Put | Method::Delete)
            || req.path == "/api/v1/env/gc"
            || req.path == "/api/v1/links/repair"
            || req.path.starts_with("/api/v1/mesh/");
        let is_mesh = req.path.starts_with("/api/v1/mesh/");
        let audit_storage = if should_audit { Some(storage.clone()) } else { None };

        // /api/v1/mesh/dialogue 与 /api/v1/mesh/transfer 内置 MeshEnvelope v1.0 HMAC-SHA256 零信任签名校验
        let uses_envelope_auth = matches!(
            req.path.as_str(),
            "/api/v1/mesh/dialogue" | "/api/v1/mesh/transfer"
        );
        if !uses_envelope_auth {
            if let Some(err_resp) =
                check_write_or_sensitive_auth(req, auth_token, enforce_token_for_writes)
            {
                if should_audit {
                    let st = crate::storage::Storage::new(storage.clone());
                    crate::audit::AuditLogger::record_best_effort(
                        &st,
                        crate::audit::AuditEventInput::in_agent(
                            if is_mesh { "mesh" } else { "web" },
                            if is_mesh { "mesh_collab" } else { "web_api" },
                            format!("{} {}", req.method.as_str(), req.path),
                        )
                        .with_target("endpoint", req.path.clone())
                        .with_outcome("blocked")
                        .with_elapsed_us(t0.elapsed().as_micros() as i64)
                        .with_delta(serde_json::json!({"status": err_resp.status.0})),
                    );
                }
                return err_resp;
            }
        }
        let resp = match (req.method.clone(), req.path.as_str()) {
            (Method::Get, "/api/v1/brief") => parity_api::handle_brief(req, storage),
            (Method::Post, "/api/v1/placement/suggest") => parity_api::handle_placement_suggest(req, storage),
            (Method::Get, "/api/v1/placement/learn") => parity_api::handle_placement_learn(req, storage),
            (Method::Get, "/api/v1/assets/search") => parity_api::handle_assets_search(req, storage.as_ref()),
            (Method::Post, "/api/v1/assets/register") => parity_api::handle_assets_register(req, storage.as_ref()),
            (Method::Post, "/api/v1/assets/scan") => parity_api::handle_assets_scan(req, storage),
            (Method::Get, "/api/v1/facts/port-check") => parity_api::handle_facts_port_check(req),
            (Method::Get, "/api/v1/facts/query") => parity_api::handle_facts_query(req),
            (Method::Get, "/api/v1/links/audit") => parity_api::handle_links_audit(storage),
            (Method::Get, "/api/v1/links/repair") => parity_api::handle_links_repair(storage),
            (Method::Get, "/api/v1/links/mermaid") => parity_api::handle_links_mermaid(req, storage),
            (Method::Get, "/api/v1/links/codegraph") => parity_api::handle_links_codegraph(req),
            (Method::Post, "/api/v1/hooks/pre") => parity_api::handle_hooks_pre(req, storage),
            (Method::Post, "/api/v1/hooks/post") => parity_api::handle_hooks_post(req, storage),
            (Method::Get, "/api/v1/hooks/generate") => parity_api::handle_hooks_generate(req),
            (Method::Get, "/api/v1/analytics/heatmap") => parity_api::handle_analytics_heatmap(req, storage),
            (Method::Get, "/api/v1/analytics/trend") => parity_api::handle_analytics_trend(req, storage),
            (Method::Post, "/api/v1/analytics/export") => parity_api::handle_analytics_export(req, storage),
            (Method::Post, "/api/v1/memories/add") => parity_api::handle_memories_add(req, storage),
            (Method::Get, "/api/v1/memories/search") => parity_api::handle_memories_search(req, storage),
            (Method::Get, "/api/v1/memories/audit") => parity_api::handle_memories_audit(storage),
            (Method::Get, "/api/v1/env/gc") => parity_api::handle_env_gc(req),
            (Method::Post, "/api/v1/session/close") => parity_api::handle_session_close(req, storage),
            (Method::Post, "/api/v1/locks/acquire") => parity_api::handle_locks_acquire(req, storage, auth_token),
            (Method::Post, "/api/v1/locks/wait") => parity_api::handle_locks_wait(req, storage, auth_token),
            (Method::Get, "/api/v1/ecosystem/service-check") => parity_api::handle_service_check(req, storage),
            (Method::Get, "/api/v1/mesh/query") => parity_api::handle_mesh_query(req),
            (Method::Get, "/api/v1/parity/matrix") => parity_api::handle_parity_matrix(storage),
            (Method::Get, "/api/v1/demo/scenarios") => parity_api::handle_demo_scenarios(),
            (Method::Post, "/api/v1/demo/run") => parity_api::handle_demo_run(req, storage),
            (Method::Post, "/api/v1/demo/reset") => parity_api::handle_demo_reset(storage.as_ref()),
            (Method::Get, "/api/v1/demo/roi") => parity_api::handle_demo_roi(storage.as_ref()),
            (Method::Get, "/api/v1/benchmark/workloads") => parity_api::handle_benchmark_workloads(),
            (Method::Post, "/api/v1/benchmark/run") => parity_api::handle_benchmark_run(req, storage),
            (Method::Get, "/api/v1/benchmark/report") => parity_api::handle_benchmark_report(req, storage.as_ref()),
            (Method::Get, "/api/v1/benchmark/ledger") => parity_api::handle_benchmark_ledger(req, storage.as_ref()),
            (Method::Get, "/api/v1/benchmark/compare") => parity_api::handle_benchmark_compare(req, storage.as_ref()),
            (Method::Get, "/api/v1/benchmark/competitors") => parity_api::handle_benchmark_competitors(req, storage),
            (Method::Post, "/api/v1/benchmark/reset") => parity_api::handle_benchmark_reset(storage.as_ref()),
            (Method::Get, "/api/v1/feedback") => parity_api::handle_feedback_list(),
            (Method::Post, "/api/v1/feedback") => parity_api::handle_feedback_post(req, storage.clone()),
            (Method::Get, "/api/v1/audit/events") => parity_api::handle_audit_query(req, storage.clone()),
            (Method::Get, "/api/v1/audit/summary") => parity_api::handle_audit_summary(req, storage.clone()),
            (Method::Post, "/api/v1/audit/evolve") => parity_api::handle_audit_evolve(req, storage.clone()),
            (Method::Post, "/api/v1/audit/export") => parity_api::handle_audit_export(req, storage.clone()),
            (Method::Get, "/api/v1/body/manifest") => parity_api::handle_body_manifest(storage.clone()),
            (Method::Get, "/api/v1/body/vitals") => parity_api::handle_body_vitals(storage.clone()),
            (Method::Get, "/api/v1/body/envelope") => parity_api::handle_body_envelope(req),
            (Method::Get, "/api/v1/body/checkpoints") => parity_api::handle_body_checkpoints(req, storage.clone()),
            (Method::Get, "/api/v1/body/peers") => parity_api::handle_body_peers(),
            (Method::Post, "/api/v1/body/handoff") => parity_api::handle_body_handoff(req, storage.clone()),
            (Method::Post, "/api/v1/body/resume") => parity_api::handle_body_resume(req, storage.clone()),
            (Method::Post, "/api/v1/body/heal") => parity_api::handle_body_heal(storage.clone()),
            (Method::Post, "/api/v1/body/perf/snapshot") => parity_api::handle_body_perf_snapshot(req, storage.clone()),
            (Method::Get, "/api/v1/body/perf/awr") => parity_api::handle_body_perf_awr(req, storage.clone()),
            (Method::Post, "/api/v1/body/perf/awr") => parity_api::handle_body_perf_awr_post(req, storage.clone()),
            (Method::Post, "/api/v1/body/perf/delete") => parity_api::handle_body_perf_delete(req, storage.clone()),
            (Method::Delete, "/api/v1/body/perf/delete") => parity_api::handle_body_perf_delete(req, storage.clone()),
            (Method::Post, "/api/v1/body/perf/tune") => parity_api::handle_body_perf_tune(storage.clone()),
            (Method::Get, "/api/v1/body/runtime-mode") => parity_api::handle_body_runtime_mode_get(),
            (Method::Post, "/api/v1/body/runtime-mode") => parity_api::handle_body_runtime_mode_post(req, storage.clone()),
            (Method::Get, "/api/v1/body/ab-report") => parity_api::handle_body_ab_report(req, storage.clone()),
            (Method::Post, "/api/v1/body/ab-compare") => parity_api::handle_body_ab_compare(req, storage.clone()),
            (Method::Post, "/api/v1/mesh/locks/acquire") => crate::mesh::handle_mesh_lock_acquire(req, storage, auth_token),
            _ => Self::route_request(req, storage.as_ref(), start_time, auth_token),
        };
        if let Some(st_arc) = audit_storage {
            let status_code = resp.status.0;
            let outcome = if status_code == 401 || status_code == 403 {
                "blocked"
            } else if status_code >= 500 {
                "failed"
            } else if status_code >= 400 {
                "warn"
            } else {
                "success"
            };
            let st = crate::storage::Storage::new(st_arc);
            crate::audit::AuditLogger::record_best_effort(
                &st,
                crate::audit::AuditEventInput::in_agent(
                    if is_mesh { "mesh" } else { "web" },
                    if is_mesh { "mesh_collab" } else { "web_api" },
                    format!("{} {}", req.method.as_str(), req.path),
                )
                .with_target("endpoint", req.path.clone())
                .with_outcome(outcome)
                .with_elapsed_us(t0.elapsed().as_micros() as i64)
                .with_delta(serde_json::json!({"status": status_code})),
            );
        }
        resp
    }

    /// 统一路由分发逻辑
    pub fn route_request(
        req: &HttpRequest,
        storage: &dyn StorageProvider,
        start_time: &Instant,
        auth_token: &Option<String>,
    ) -> HttpResponse {
        // 静态资源分发
        if req.method == Method::Get {
            match req.path.as_str() {
                "/" | "/index.html" => return assets::serve_index(),
                "/app.css" => return assets::serve_css(),
                "/app.js" => return assets::serve_js(),
                "/deck" => return assets::serve_deck(),
                "/presentation" => return assets::serve_presentation(),
                _ => {}
            }
        }

        // RESTful API 路由
        match (req.method.clone(), req.path.as_str()) {
            (Method::Get, "/api/v1/health") => api::handle_health(storage, start_time),
            (Method::Get, "/api/v1/overview") => api::handle_overview(storage),
            (Method::Get, "/api/v1/facts") => api::handle_facts(storage),
            (Method::Get, "/api/v1/ecosystem") => api::handle_ecosystem(req, storage),
            (Method::Get, "/api/v1/ecosystem/route") => api::handle_ecosystem_route(req, storage),
            (Method::Get, "/api/v1/governance") => api::handle_governance(req, storage),
            (Method::Get, "/api/v1/mesh/status") => api::handle_mesh_status(storage),
            (Method::Get, "/api/v1/topology") => crate::web::topology::handle_topology(storage),
            (Method::Get, "/api/v1/topology/impact") => crate::web::topology::handle_topology_impact(req, storage),
            (Method::Get, "/api/v1/locks") => crate::web::locks_api::handle_locks_list(storage),
            (Method::Get, "/api/v1/locks/history") => crate::web::locks_api::handle_locks_history(storage),
            (Method::Get, "/api/v1/locks/wait-queue") => crate::web::locks_api::handle_locks_wait_queue(storage),
            (Method::Get, "/api/v1/locks/deadlocks") => crate::web::locks_api::handle_locks_deadlocks(storage),
            (Method::Post, "/api/v1/locks/release") => crate::web::locks_api::handle_lock_release(req, storage, auth_token),
            (Method::Get, "/api/v1/mesh/ping") => crate::mesh::handle_mesh_ping(req, auth_token),
            (Method::Get, "/api/v1/mesh/facts") => crate::mesh::handle_mesh_facts(req, auth_token),
            (Method::Get, "/api/v1/mesh/body_manifest") => crate::mesh::handle_mesh_body_manifest(req, storage, auth_token),
            (Method::Get, "/api/v1/mesh/locks") => crate::mesh::handle_mesh_locks(req, storage, auth_token),
            (Method::Get, "/api/v1/mesh/graph") => crate::mesh::handle_mesh_graph(req, storage, auth_token),
            (Method::Post, "/api/v1/mesh/dialogue") => crate::mesh::handle_mesh_dialogue_http(req, storage, auth_token),
            (Method::Post, "/api/v1/mesh/transfer") => crate::mesh::handle_mesh_transfer_http(req, storage, auth_token),
            _ => HttpResponse::not_found(&format!("Endpoint not found: {} {}", req.method.as_str(), req.path)),
        }
    }

    /// 处理 SSE 长连接推流
    async fn handle_sse_stream(
        mut stream: TcpStream,
        event_tx: broadcast::Sender<String>,
        origin: Option<&str>,
    ) -> anyhow::Result<()> {
        let cors_line = match origin.map(str::trim).filter(|s| crate::web::http::is_loopback_origin(s)) {
            Some(orig) => format!("Access-Control-Allow-Origin: {}\r\nVary: Origin\r\n", orig),
            None => "Vary: Origin\r\n".to_string(),
        };
        let headers = format!(
            "HTTP/1.1 200 OK\r\n\
Content-Type: text/event-stream\r\n\
Cache-Control: no-cache\r\n\
Connection: keep-alive\r\n\
{}\r\n",
            cors_line
        );

        stream.write_all(headers.as_bytes()).await?;
        stream.flush().await?;

        // 先发送连接成功欢迎消息
        let welcome = format!("data: {}\n\n", serde_json::json!({
            "type": "welcome",
            "message": "Connected to Agent Body Event Stream",
            "timestamp": chrono::Utc::now().to_rfc3339()
        }));
        stream.write_all(welcome.as_bytes()).await?;
        stream.flush().await?;

        let mut rx = event_tx.subscribe();
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(15));

        loop {
            tokio::select! {
                // 监听广播事件
                Ok(msg) = rx.recv() => {
                    let sse_line = format!("data: {}\n\n", msg);
                    if let Err(_) = stream.write_all(sse_line.as_bytes()).await {
                        break;
                    }
                    if let Err(_) = stream.flush().await {
                        break;
                    }
                }
                // 定时保活心跳
                _ = interval.tick() => {
                    let ping = ": keepalive-ping\n\n";
                    if let Err(_) = stream.write_all(ping.as_bytes()).await {
                        break;
                    }
                    if let Err(_) = stream.flush().await {
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}
