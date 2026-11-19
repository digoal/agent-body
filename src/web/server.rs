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
        println!("│  🌐 Agent Host Steward Web Dashboard                       │");
        println!("│  URL: http://{:<45} │", addr);
        println!("╰────────────────────────────────────────────────────────────╯");

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
                        if let Err(e) = Self::handle_connection(stream, storage, start_time, event_tx, auth_token).await {
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
    ) -> anyhow::Result<()> {
        let mut buffer = [0u8; 4096];
        let bytes_read = stream.read(&mut buffer).await?;
        if bytes_read == 0 {
            return Ok(());
        }
        let mut raw: Vec<u8> = buffer[..bytes_read].to_vec();

        // 若请求体分包到达 (如 reqwest 先发头后发体)，按 Content-Length 补读剩余请求体 (上限 64KB)
        if let Some(header_end) = raw.windows(4).position(|w| w == b"\r\n\r\n") {
            let header_text = String::from_utf8_lossy(&raw[..header_end]).to_lowercase();
            let content_len = header_text
                .lines()
                .find_map(|l| l.strip_prefix("content-length:").and_then(|v| v.trim().parse::<usize>().ok()))
                .unwrap_or(0)
                .min(65536);
            let body_start = header_end + 4;
            while raw.len() < body_start + content_len {
                match tokio::time::timeout(std::time::Duration::from_millis(200), stream.read(&mut buffer)).await {
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

        // 处理 CORS OPTIONS preflight
        if req.method == Method::Options {
            let resp = HttpResponse::new(StatusCode::NO_CONTENT);
            stream.write_all(&resp.to_bytes()).await?;
            stream.flush().await?;
            return Ok(());
        }

        // 处理 SSE 实时流
        if req.method == Method::Get && req.path == "/api/v1/stream" {
            return Self::handle_sse_stream(stream, event_tx).await;
        }

        // Mesh 跨节点协作锁申请需要持有 Arc 存储句柄以构造 CoordinationEngine
        if req.method == Method::Post && req.path == "/api/v1/mesh/locks/acquire" {
            let resp = crate::mesh::handle_mesh_lock_acquire(&req, storage.clone(), &auth_token);
            stream.write_all(&resp.to_bytes()).await?;
            stream.flush().await?;
            return Ok(());
        }

        // 统一路由分发 (在 spawn_blocking 中执行，避免阻塞 Tokio worker 并兼容同步预检/DB调用)
        let is_demo_run = req.method == Method::Post && req.path == "/api/v1/demo/run";
        let demo_path = req.path.clone();
        let resp = tokio::task::spawn_blocking(move || {
            Self::route_request_arc(&req, storage, &start_time, &auth_token)
        })
        .await?;
        if is_demo_run && resp.status == StatusCode::OK {
            let _ = event_tx.send(
                serde_json::json!({
                    "type": "demo_run",
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

    /// 持有 Arc<dyn StorageProvider> 的完整路由分发（覆盖 Phase 21 黑白屏全能力 API）
    pub fn route_request_arc(
        req: &HttpRequest,
        storage: Arc<dyn StorageProvider>,
        start_time: &Instant,
        auth_token: &Option<String>,
    ) -> HttpResponse {
        match (req.method.clone(), req.path.as_str()) {
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
            _ => Self::route_request(req, storage.as_ref(), start_time, auth_token),
        }
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
            (Method::Get, "/api/v1/mesh/locks") => crate::mesh::handle_mesh_locks(req, storage, auth_token),
            _ => HttpResponse::not_found(&format!("Endpoint not found: {} {}", req.method.as_str(), req.path)),
        }
    }

    /// 处理 SSE 长连接推流
    async fn handle_sse_stream(
        mut stream: TcpStream,
        event_tx: broadcast::Sender<String>,
    ) -> anyhow::Result<()> {
        let headers = "HTTP/1.1 200 OK\r\n\
Content-Type: text/event-stream\r\n\
Cache-Control: no-cache\r\n\
Connection: keep-alive\r\n\
Access-Control-Allow-Origin: *\r\n\r\n";

        stream.write_all(headers.as_bytes()).await?;
        stream.flush().await?;

        // 先发送连接成功欢迎消息
        let welcome = format!("data: {}\n\n", serde_json::json!({
            "type": "welcome",
            "message": "Connected to Agent Steward Event Stream",
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
