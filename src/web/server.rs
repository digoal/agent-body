use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;

use crate::storage::StorageProvider;
use crate::web::http::{HttpRequest, HttpResponse, Method, StatusCode};
use crate::web::api;
use crate::web::assets;

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

        let req = match HttpRequest::parse(&buffer[..bytes_read]) {
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

        // 统一路由分发
        let resp = Self::route_request(&req, storage.as_ref(), &start_time, &auth_token);
        stream.write_all(&resp.to_bytes()).await?;
        stream.flush().await?;
        Ok(())
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
                _ => {}
            }
        }

        // RESTful API 路由
        match (req.method.clone(), req.path.as_str()) {
            (Method::Get, "/api/v1/health") => api::handle_health(storage, start_time),
            (Method::Get, "/api/v1/overview") => api::handle_overview(storage),
            (Method::Get, "/api/v1/topology") => crate::web::topology::handle_topology(storage),
            (Method::Get, "/api/v1/topology/impact") => crate::web::topology::handle_topology_impact(req, storage),
            (Method::Get, "/api/v1/locks") => crate::web::locks_api::handle_locks_list(storage),
            (Method::Get, "/api/v1/locks/history") => crate::web::locks_api::handle_locks_history(storage),
            (Method::Get, "/api/v1/locks/wait-queue") => crate::web::locks_api::handle_locks_wait_queue(storage),
            (Method::Get, "/api/v1/locks/deadlocks") => crate::web::locks_api::handle_locks_deadlocks(storage),
            (Method::Post, "/api/v1/locks/release") => crate::web::locks_api::handle_lock_release(req, storage, auth_token),
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
