//! 轻量级多机联邦 Peer Mesh (Phase 18.2)
//!
//! 设计取舍（规约 #2 递弱代偿 / #9 轻量化）：
//! - **不造 Raft/Gossip 轮子**：基于已有内嵌 HTTP 服务暴露 `/api/v1/mesh/*`，
//!   对等节点之间以 Bearer Token (复用 `[web].auth_token`) 互相只读联邦查询；
//! - **跨节点协作锁**：采用“资源属主节点即协调者”的 HTTP 租约模型——
//!   向目标节点 `POST /api/v1/mesh/locks/acquire`，由该节点本地 `CoordinationEngine`
//!   裁决（TTL 租约自动过期），无需分布式共识；
//! - **安全**：写操作（锁申请）强制要求服务端已配置 Token 且请求携带正确 Token。

pub mod auth;
pub mod dialogue;
pub mod protocol;
pub mod transfer;

use std::sync::Arc;
use std::time::Instant;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};

use crate::config::MeshConfig;
use crate::coordination::{CoordinationEngine, LockRequest};
use crate::storage::StorageProvider;
use crate::web::api::WebResponse;
use crate::web::http::{HttpRequest, HttpResponse, StatusCode};

// ─────────────────────────── 节点事实模型 ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshPortFact {
    pub port: u16,
    pub protocol: String,
    pub bind_ip: String,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshNodeFacts {
    pub node_id: String,
    pub os: String,
    pub platform_probe: String,
    pub cpu_cores: usize,
    pub total_memory_mb: u64,
    pub listening_ports: Vec<MeshPortFact>,
    pub steward_version: String,
    pub timestamp: String,
}

/// 采集本节点真实物理事实（只读，零捏造）
pub fn build_local_facts() -> MeshNodeFacts {
    let sys = System::new_with_specifics(
        RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything()),
    );
    let probe = crate::platform::current_probe();
    let ports = probe
        .listening_ports()
        .unwrap_or_default()
        .into_iter()
        .filter(|p| !p.is_system_daemon)
        .map(|p| MeshPortFact {
            port: p.port,
            protocol: p.protocol,
            bind_ip: p.bind_ip,
            pid: p.pid,
            process_name: p.process_name,
        })
        .collect();

    MeshNodeFacts {
        node_id: System::host_name().unwrap_or_else(|| "unknown-node".to_string()),
        os: std::env::consts::OS.to_string(),
        platform_probe: probe.name().to_string(),
        cpu_cores: sys.cpus().len(),
        total_memory_mb: sys.total_memory() / (1024 * 1024),
        listening_ports: ports,
        steward_version: crate::VERSION.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    }
}

// ─────────────────────────── 服务端 Handler ───────────────────────────

/// 校验 Bearer Token（常量时间比对）；`require_token=true` 时服务端未配置 Token 也直接拒绝
fn check_mesh_auth(req: &HttpRequest, auth_token: &Option<String>, require_token: bool) -> Option<HttpResponse> {
    match auth_token.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(expected) => {
            let header = req.headers.get("authorization").cloned().unwrap_or_default();
            let token = header.strip_prefix("Bearer ").unwrap_or(&header);
            if !crate::mesh::auth::constant_time_eq(token, expected) {
                return Some(WebResponse::error(StatusCode::UNAUTHORIZED, "Unauthorized: invalid mesh token"));
            }
            None
        }
        None if require_token => Some(WebResponse::error(
            StatusCode::FORBIDDEN,
            "Forbidden: mesh write operations require [web].auth_token to be configured",
        )),
        None => None,
    }
}

/// 校验对等节点只读接口的 Scope 白名单（当在 `[mesh].peer_scopes` 为来源节点配置白名单时生效）
pub fn check_peer_read_scope_with_cfg(
    req: &HttpRequest,
    required_scope: &str,
    mesh_cfg: &MeshConfig,
) -> Option<HttpResponse> {
    let from_node = req
        .headers
        .get("x-body-node")
        .or_else(|| req.headers.get("x-steward-node"))
        .map(String::as_str)
        .or_else(|| req.query_params.get("from_node").map(String::as_str))
        .map(str::trim)
        .filter(|s| !s.is_empty());

    if let Some(peer) = from_node {
        if let Some(allowed) = mesh_cfg.peer_scopes.get(peer) {
            if !allowed.iter().any(|s| s == required_scope || s == "*") {
                return Some(WebResponse::error(
                    StatusCode::FORBIDDEN,
                    &format!("Forbidden: peer `{}` lacks required scope `{}`", peer, required_scope),
                ));
            }
        }
    }
    None
}

fn check_peer_read_scope(req: &HttpRequest, required_scope: &str) -> Option<HttpResponse> {
    if req.headers.contains_key("x-body-node")
        || req.headers.contains_key("x-steward-node")
        || req.query_params.contains_key("from_node")
    {
        let cfg = crate::config::Config::load_or_default(None).unwrap_or_default();
        return check_peer_read_scope_with_cfg(req, required_scope, &cfg.mesh);
    }
    None
}

/// GET /api/v1/mesh/ping
pub fn handle_mesh_ping(req: &HttpRequest, auth_token: &Option<String>) -> HttpResponse {
    if let Some(deny) = check_mesh_auth(req, auth_token, false) {
        return deny;
    }
    WebResponse::success(serde_json::json!({
        "node_id": System::host_name().unwrap_or_else(|| "unknown-node".to_string()),
        "os": std::env::consts::OS,
        "steward_version": crate::VERSION,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

/// GET /api/v1/mesh/facts
pub fn handle_mesh_facts(req: &HttpRequest, auth_token: &Option<String>) -> HttpResponse {
    if let Some(deny) = check_mesh_auth(req, auth_token, false) {
        return deny;
    }
    WebResponse::success(build_local_facts())
}

/// GET /api/v1/mesh/body_manifest —— 多本体只读联邦名片交换 (Phase 44)
pub fn handle_mesh_body_manifest(
    req: &HttpRequest,
    storage: &dyn StorageProvider,
    auth_token: &Option<String>,
) -> HttpResponse {
    if let Some(deny) = check_mesh_auth(req, auth_token, false) {
        return deny;
    }
    if let Some(deny) = check_peer_read_scope(req, "read:facts") {
        return deny;
    }
    let body_id = crate::body::manifest::compute_body_id();
    let hostname = System::host_name().unwrap_or_else(|| "localhost".to_string());
    let fact_nodes = storage.list_all_fact_nodes(2000).unwrap_or_default();
    let assets_cnt = storage.list_assets(20000).map(|v| v.len()).unwrap_or(0);
    let active_cps = storage
        .list_body_checkpoints(Some("active"), 20)
        .map(|v| v.len())
        .unwrap_or(0);
    let locks_cnt = storage.list_coordination_locks(None).map(|v| v.len()).unwrap_or(0);
    WebResponse::success(serde_json::json!({
        "body_id": body_id,
        "hostname": hostname,
        "os": crate::BUILD_OS,
        "arch": crate::BUILD_ARCH,
        "version": crate::VERSION,
        "sensory": {
            "fact_nodes": fact_nodes.len(),
            "assets": assets_cnt,
            "tools": fact_nodes.iter().filter(|n| n.node_type == "tool").count(),
            "ports": fact_nodes.iter().filter(|n| n.node_type == "port").count(),
            "projects": fact_nodes.iter().filter(|n| n.node_type == "project").count(),
        },
        "active_checkpoints": active_cps,
        "active_locks": locks_cnt,
        "mesh_protocol": "MeshEnvelope v1.0",
    }))
}

/// GET /api/v1/mesh/locks
pub fn handle_mesh_locks(req: &HttpRequest, storage: &dyn StorageProvider, auth_token: &Option<String>) -> HttpResponse {
    if let Some(deny) = check_mesh_auth(req, auth_token, false) {
        return deny;
    }
    match storage.list_coordination_locks(None) {
        Ok(locks) => WebResponse::success(locks),
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Failed to list locks: {}", e)),
    }
}

/// POST /api/v1/mesh/locks/acquire —— 本节点作为资源属主协调者裁决锁申请
pub fn handle_mesh_lock_acquire(
    req: &HttpRequest,
    storage: Arc<dyn StorageProvider>,
    auth_token: &Option<String>,
) -> HttpResponse {
    if let Some(deny) = check_mesh_auth(req, auth_token, true) {
        return deny;
    }
    let lock_req: LockRequest = match req.json() {
        Ok(r) => r,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid LockRequest JSON: {}", e)),
    };
    let engine = CoordinationEngine::new(storage);
    match engine.try_acquire(&lock_req) {
        Ok(res) => WebResponse::success(res),
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Acquire failed: {}", e)),
    }
}

/// GET /api/v1/mesh/graph?q=...&hops=2 —— 跨节点只读图谱联邦查询 (Phase 41.3)
pub fn handle_mesh_graph(
    req: &HttpRequest,
    storage: &dyn StorageProvider,
    auth_token: &Option<String>,
) -> HttpResponse {
    if let Some(deny) = check_mesh_auth(req, auth_token, false) {
        return deny;
    }
    if let Some(deny) = check_peer_read_scope(req, "read:facts") {
        return deny;
    }
    let decoded_q = req
        .query_params
        .get("q")
        .map(|raw| {
            urlencoding::decode(raw)
                .map(|d| d.into_owned())
                .unwrap_or_else(|_| raw.clone())
        })
        .unwrap_or_default();
    let q = decoded_q.trim();
    let hops = req
        .query_params
        .get("hops")
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(2)
        .clamp(1, 4);
    let min_weight = req
        .query_params
        .get("min_weight")
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(0.35);

    if q.is_empty() {
        let nodes = storage.list_all_fact_nodes(50).unwrap_or_default();
        let edges = storage.list_all_fact_edges(100).unwrap_or_default();
        return WebResponse::success(serde_json::json!({
            "node_id": System::host_name().unwrap_or_else(|| "unknown-node".to_string()),
            "nodes_count": nodes.len(),
            "edges_count": edges.len(),
            "nodes": nodes,
            "edges": edges,
        }));
    }

    let sub = if storage.get_fact_node(q).ok().flatten().is_some() {
        storage.traverse_fact_graph_filtered(q, hops, min_weight, None)
    } else if let Ok(matches) = storage.find_fact_nodes_by_name(q) {
        if let Some(first) = matches.first() {
            storage.traverse_fact_graph_filtered(&first.id, hops, min_weight, None)
        } else {
            Ok(crate::storage::models::SubGraph::default())
        }
    } else {
        Ok(crate::storage::models::SubGraph::default())
    };

    match sub {
        Ok(g) => WebResponse::success(serde_json::json!({
            "node_id": System::host_name().unwrap_or_else(|| "unknown-node".to_string()),
            "query": q,
            "hops": hops,
            "subgraph": g,
        })),
        Err(e) => WebResponse::error(StatusCode::INTERNAL_SERVER_ERROR, &format!("Graph query failed: {}", e)),
    }
}

/// POST /api/v1/mesh/dialogue —— HTTP 入口（自动加载 Config::mesh）
pub fn handle_mesh_dialogue_http(
    req: &HttpRequest,
    storage: &dyn StorageProvider,
    web_auth_token: &Option<String>,
) -> HttpResponse {
    let cfg = crate::config::Config::load_or_default(None).unwrap_or_default();
    crate::mesh::dialogue::handle_mesh_dialogue(req, storage, &cfg.mesh, web_auth_token)
}

/// POST /api/v1/mesh/transfer —— HTTP 入口（自动加载 Config::mesh）
pub fn handle_mesh_transfer_http(
    req: &HttpRequest,
    storage: &dyn StorageProvider,
    web_auth_token: &Option<String>,
) -> HttpResponse {
    let cfg = crate::config::Config::load_or_default(None).unwrap_or_default();
    crate::mesh::transfer::handle_mesh_transfer(req, storage, &cfg.mesh, web_auth_token)
}

// ─────────────────────────── 客户端 ───────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerResult {
    pub peer: String,
    pub ok: bool,
    pub latency_ms: u64,
    pub data: Option<Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePortStatus {
    pub node_id: String,
    pub peer: String,
    pub occupied: bool,
    pub process_name: Option<String>,
    pub pid: Option<u32>,
}

/// 联邦客户端（阻塞式，内部在独立线程执行 HTTP，避免在 tokio 运行时内误用 blocking 客户端）
pub struct MeshClient {
    peers: Vec<String>,
    node_id: String,
    token: Option<String>,
    timeout_ms: u64,
    cfg: MeshConfig,
}

impl MeshClient {
    pub fn new(cfg: &MeshConfig, token: Option<String>) -> Self {
        let node_id = cfg
            .node_id
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(System::host_name)
            .unwrap_or_else(|| "local-node".to_string());
        Self {
            peers: cfg.peers.iter().map(|p| p.trim_end_matches('/').to_string()).collect(),
            node_id,
            token,
            timeout_ms: cfg.timeout_ms.max(100),
            cfg: cfg.clone(),
        }
    }

    pub fn peers(&self) -> &[String] {
        &self.peers
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    fn resolve_signing_secret(&self) -> Result<String> {
        if let Some(ref s) = self.cfg.shared_secret {
            if !s.trim().is_empty() {
                return Ok(s.clone());
            }
        }
        if let Some(ref t) = self.token {
            if !t.trim().is_empty() {
                return Ok(t.clone());
            }
        }
        Err(anyhow!("未配置 [mesh].shared_secret 或 [web].auth_token，无法对 MeshEnvelope 签名"))
    }

    fn request(&self, peer: &str, method: &str, path: &str, body: Option<Value>) -> PeerResult {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::NetMeshRpc);
        let url = format!("{}{}", peer, path);
        let token = self.token.clone();
        let timeout = std::time::Duration::from_millis(self.timeout_ms);
        let method = method.to_string();
        let peer_owned = peer.to_string();
        let start = Instant::now();

        let outcome: Result<Value> = std::thread::scope(|s| {
            s.spawn(|| -> Result<Value> {
                let client = reqwest::blocking::Client::builder().timeout(timeout).build()?;
                let mut rb = if method == "POST" { client.post(&url) } else { client.get(&url) };
                if let Some(t) = token {
                    rb = rb.bearer_auth(t);
                }
                if let Some(b) = body {
                    rb = rb.json(&b);
                }
                let resp = rb.send()?;
                let status = resp.status();
                let text = resp.text()?;
                let v: Value = serde_json::from_str(&text).map_err(|e| anyhow!("响应非 JSON: {}", e))?;
                if !status.is_success() {
                    let msg = v.get("message").and_then(|m| m.as_str()).unwrap_or("未知错误");
                    return Err(anyhow!("HTTP {}: {}", status.as_u16(), msg));
                }
                Ok(v.get("data").cloned().unwrap_or(Value::Null))
            })
            .join()
            .unwrap_or_else(|_| Err(anyhow!("联邦请求线程异常退出")))
        });

        let latency_ms = start.elapsed().as_millis() as u64;
        match outcome {
            Ok(data) => PeerResult { peer: peer_owned, ok: true, latency_ms, data: Some(data), error: None },
            Err(e) => PeerResult { peer: peer_owned, ok: false, latency_ms, data: None, error: Some(e.to_string()) },
        }
    }

    pub fn ping_all(&self) -> Vec<PeerResult> {
        self.peers.iter().map(|p| self.request(p, "GET", "/api/v1/mesh/ping", None)).collect()
    }

    pub fn facts_all(&self) -> Vec<PeerResult> {
        self.peers.iter().map(|p| self.request(p, "GET", "/api/v1/mesh/facts", None)).collect()
    }

    pub fn locks_all(&self) -> Vec<PeerResult> {
        self.peers.iter().map(|p| self.request(p, "GET", "/api/v1/mesh/locks", None)).collect()
    }

    /// 联邦端口查询：本机 + 全部对等节点，回答“集群哪台机器占用/空闲某端口”
    pub fn federated_port_check(&self, port: u16) -> Vec<NodePortStatus> {
        let mut out = Vec::new();
        let local = build_local_facts();
        out.push(Self::port_status_from_facts(&local, "local", port));

        for r in self.facts_all() {
            if let (true, Some(data)) = (r.ok, r.data) {
                if let Ok(facts) = serde_json::from_value::<MeshNodeFacts>(data) {
                    out.push(Self::port_status_from_facts(&facts, &r.peer, port));
                }
            }
        }
        out
    }

    fn port_status_from_facts(facts: &MeshNodeFacts, peer: &str, port: u16) -> NodePortStatus {
        let hit = facts.listening_ports.iter().find(|p| p.port == port);
        NodePortStatus {
            node_id: facts.node_id.clone(),
            peer: peer.to_string(),
            occupied: hit.is_some(),
            process_name: hit.and_then(|h| h.process_name.clone()),
            pid: hit.and_then(|h| h.pid),
        }
    }

    /// 在指定对等节点（资源属主）上申请协作锁
    pub fn acquire_on_peer(&self, peer: &str, req: &LockRequest) -> Result<PeerResult> {
        let body = serde_json::to_value(req)?;
        Ok(self.request(peer.trim_end_matches('/'), "POST", "/api/v1/mesh/locks/acquire", Some(body)))
    }

    /// 联邦图谱探查：向所有对等节点并发/串行查询指定实体子图
    pub fn federated_graph_query(&self, query: &str, hops: usize) -> Vec<PeerResult> {
        let encoded = urlencoding::encode(query);
        let path = format!("/api/v1/mesh/graph?q={}&hops={}", encoded, hops);
        self.peers
            .iter()
            .map(|p| self.request(p, "GET", &path, None))
            .collect()
    }

    /// 向指定对等节点发送签名结构化 Agent 对话（出站前自动脱敏与抗投毒清洗）
    pub fn send_dialogue(
        &self,
        peer: &str,
        thread_id: &str,
        intent: &str,
        content: &str,
        from_agent: &str,
        to_agent: &str,
    ) -> Result<PeerResult> {
        let secret = self.resolve_signing_secret()?;
        let (cleaned, _, _) = crate::mesh::dialogue::sanitize_dialogue_content(content);
        let payload = serde_json::json!({
            "thread_id": thread_id,
            "intent": intent,
            "content": cleaned,
        });
        let env = crate::mesh::protocol::MeshEnvelope::build_signed(
            "dialogue",
            &self.node_id,
            from_agent,
            peer,
            to_agent,
            vec!["exchange:dialogue".to_string()],
            payload,
            &secret,
        )?;
        let body = serde_json::to_value(&env)?;
        Ok(self.request(peer.trim_end_matches('/'), "POST", "/api/v1/mesh/dialogue", Some(body)))
    }

    /// 向指定对等节点投递受控文件（自动校验体积上限、敏感文件名、SHA-256 并使用 MeshEnvelope 签名）
    pub fn send_transfer(
        &self,
        peer: &str,
        file_path: &std::path::Path,
        from_agent: &str,
    ) -> Result<PeerResult> {
        let secret = self.resolve_signing_secret()?;
        let raw_name = file_path
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| anyhow!("无效文件路径: {}", file_path.display()))?;
        let safe_name = crate::mesh::transfer::validate_safe_file_name(raw_name)
            .map_err(|e| anyhow!(e))?;
        let bytes = std::fs::read(file_path)?;
        if bytes.len() > self.cfg.max_transfer_bytes {
            anyhow::bail!(
                "文件大小 {} 超过上限 {} 字节",
                bytes.len(),
                self.cfg.max_transfer_bytes
            );
        }
        let sha256 = crate::mesh::protocol::sha256_bytes_hex(&bytes);
        let b64 = crate::mesh::transfer::base64_encode(&bytes);
        let payload = serde_json::json!({
            "file_name": safe_name,
            "sha256": sha256,
            "content_base64": b64,
        });
        let env = crate::mesh::protocol::MeshEnvelope::build_signed(
            "transfer",
            &self.node_id,
            from_agent,
            peer,
            "",
            vec!["transfer:artifact".to_string()],
            payload,
            &secret,
        )?;
        let body = serde_json::to_value(&env)?;
        Ok(self.request(peer.trim_end_matches('/'), "POST", "/api/v1/mesh/transfer", Some(body)))
    }

    /// 跨机拉取对等宿主机 Agent 本体名片 (`/api/v1/mesh/body_manifest`)
    pub fn query_peer_body_manifests(&self) -> Vec<PeerResult> {
        self.peers
            .iter()
            .map(|p| self.request(p, "GET", "/api/v1/mesh/body_manifest", None))
            .collect()
    }
}

// ─────────────────────────── 单元测试 ───────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coordination::{LockScope, ResourceType};
    use crate::storage::sqlite_provider::SqliteStorageProvider;
    use crate::web::http::Method;

    fn make_req(method: Method, path: &str, token: Option<&str>, body: Vec<u8>) -> HttpRequest {
        let mut headers = std::collections::HashMap::new();
        if let Some(t) = token {
            headers.insert("authorization".to_string(), format!("Bearer {}", t));
        }
        HttpRequest {
            method,
            path: path.to_string(),
            raw_query: String::new(),
            query_params: Default::default(),
            headers,
            body,
        }
    }

    #[test]
    fn test_local_facts_are_real() {
        let f = build_local_facts();
        assert!(!f.node_id.is_empty());
        assert!(f.cpu_cores > 0);
        assert_eq!(f.os, std::env::consts::OS);
    }

    #[test]
    fn test_mesh_auth_rules() {
        let secret = Some("s3cr3t".to_string());
        // 读接口：Token 错误 -> 401
        let bad = make_req(Method::Get, "/api/v1/mesh/ping", Some("wrong"), vec![]);
        assert_eq!(handle_mesh_ping(&bad, &secret).status, StatusCode::UNAUTHORIZED);
        // 读接口：Token 正确 -> 200
        let good = make_req(Method::Get, "/api/v1/mesh/ping", Some("s3cr3t"), vec![]);
        assert_eq!(handle_mesh_ping(&good, &secret).status, StatusCode::OK);
        // 读接口：服务端未配置 Token -> 放行（与大屏只读一致）
        let anon = make_req(Method::Get, "/api/v1/mesh/ping", None, vec![]);
        assert_eq!(handle_mesh_ping(&anon, &None).status, StatusCode::OK);

        // 写接口：服务端未配置 Token -> 403 拒绝
        let storage: Arc<dyn StorageProvider> = Arc::new(SqliteStorageProvider::open_in_memory().expect("mem"));
        let w = make_req(Method::Post, "/api/v1/mesh/locks/acquire", None, b"{}".to_vec());
        assert_eq!(handle_mesh_lock_acquire(&w, storage, &None).status, StatusCode::FORBIDDEN);
    }

    /// 端到端：真实启动一个 Web 服务（对等节点），用 MeshClient 做 ping / facts / 联邦端口 / 跨节点锁
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_mesh_client_end_to_end_against_real_peer() {
        use crate::web::{WebConfig, WebServer};

        let storage: Arc<dyn StorageProvider> = Arc::new(SqliteStorageProvider::open_in_memory().expect("mem"));
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let port = l.local_addr().expect("addr").port();
        drop(l);

        let server = WebServer::new(
            WebConfig { host: "127.0.0.1".into(), port, auth_token: Some("mesh-secret".into()) },
            storage.clone(),
        );
        tokio::spawn(async move {
            let _ = server.run().await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let peer = format!("http://127.0.0.1:{}", port);
        let cfg = MeshConfig { enabled: true, peers: vec![peer.clone()], timeout_ms: 3000, ..Default::default() };

        let (pings, facts_ports, ok_lock, deny_lock, bad_token) = tokio::task::spawn_blocking(move || {
            let client = MeshClient::new(&cfg, Some("mesh-secret".to_string()));
            let pings = client.ping_all();
            // 对等节点自身 Web 端口一定在监听 -> 联邦查询应在对端命中
            let fed = client.federated_port_check(port);

            let req = |agent: &str| LockRequest {
                resource_type: ResourceType::Port,
                resource_id: "7777".to_string(),
                scope: LockScope::Exclusive,
                holder_agent: agent.to_string(),
                holder_pid: None,
                ttl_seconds: 30,
                metadata: None,
            };
            let ok_lock = client.acquire_on_peer(&peer, &req("agent-x")).expect("acquire x");
            let deny_lock = client.acquire_on_peer(&peer, &req("agent-y")).expect("acquire y");

            let wrong = MeshClient::new(&cfg, Some("wrong".to_string()));
            let bad_token = wrong.ping_all();
            (pings, fed, ok_lock, deny_lock, bad_token)
        })
        .await
        .expect("join");

        assert!(pings[0].ok, "ping 应成功: {:?}", pings[0].error);
        let remote = facts_ports.iter().find(|s| s.peer != "local").expect("remote status");
        assert!(remote.occupied, "对等节点的 Web 端口应被联邦查询识别为占用");

        let ok_json = serde_json::to_string(&ok_lock.data).unwrap_or_default();
        assert!(ok_lock.ok && ok_json.contains("granted"), "首个 Agent 应拿到跨节点锁: {}", ok_json);
        let deny_json = serde_json::to_string(&deny_lock.data).unwrap_or_default();
        assert!(deny_lock.ok && deny_json.contains("denied"), "第二个 Agent 应被属主节点拒绝: {}", deny_json);

        assert!(!bad_token[0].ok, "错误 Token 必须被拒绝");
        assert!(bad_token[0].error.clone().unwrap_or_default().contains("401"));
    }

    #[test]
    fn test_mesh_dialogue_and_transfer_sandbox_lifecycle() {
        use crate::mesh::dialogue::handle_mesh_dialogue;
        use crate::mesh::protocol::MeshEnvelope;
        use crate::mesh::transfer::{accept_transfer_to_dir, base64_encode, handle_mesh_transfer, reject_transfer};
        use crate::storage::sqlite_provider::SqliteStorageProvider;

        let storage = SqliteStorageProvider::open_in_memory().expect("mem");
        let tmp = tempfile::tempdir().expect("tmpdir");
        let mut cfg = MeshConfig::default();
        cfg.node_id = Some("node-b".into());
        cfg.shared_secret = Some("s3cr3t-mesh".into());
        cfg.inbox_dir = Some(tmp.path().join("inbox").to_string_lossy().to_string());

        // 1. 测试签名对话 + 脱敏落库
        let dlg_env = MeshEnvelope::build_signed(
            "dialogue",
            "node-a",
            "agent-a",
            "node-b",
            "agent-b",
            vec!["exchange:dialogue".into()],
            serde_json::json!({
                "thread_id": "thr-100",
                "intent": "ASK_CONTEXT",
                "content": "please check token sk-proj-abcdefghijklmnopqrstuvwxyz123456"
            }),
            "s3cr3t-mesh",
        )
        .expect("dlg env");
        let dlg_req = make_req(
            Method::Post,
            "/api/v1/mesh/dialogue",
            None,
            serde_json::to_vec(&dlg_env).expect("json"),
        );
        let dlg_resp = handle_mesh_dialogue(&dlg_req, &storage, &cfg, &None);
        assert_eq!(dlg_resp.status, StatusCode::OK);
        let dlgs = storage.list_mesh_dialogues(10).expect("list dlgs");
        assert_eq!(dlgs.len(), 1);
        assert_eq!(dlgs[0].redaction_hits, 1);
        assert!(!dlgs[0].content.contains("sk-proj-abcdefghijklmnopqrstuvwxyz"));

        // 2. 测试受控文件投递 -> 隔离沙盒 -> accept 落位
        let content = b"SELECT 1;\n";
        let sha = crate::mesh::protocol::sha256_bytes_hex(content);
        let tx_env = MeshEnvelope::build_signed(
            "transfer",
            "node-a",
            "agent-a",
            "node-b",
            "",
            vec!["transfer:artifact".into()],
            serde_json::json!({
                "file_name": "query.sql",
                "sha256": sha,
                "content_base64": base64_encode(content),
            }),
            "s3cr3t-mesh",
        )
        .expect("tx env");
        let tx_req = make_req(
            Method::Post,
            "/api/v1/mesh/transfer",
            None,
            serde_json::to_vec(&tx_env).expect("json"),
        );
        let tx_resp = handle_mesh_transfer(&tx_req, &storage, &cfg, &None);
        assert_eq!(tx_resp.status, StatusCode::OK);
        let txs = storage.list_mesh_transfers(10).expect("list txs");
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].status, "quarantined");

        let dest = tmp.path().join("accepted");
        let acc = accept_transfer_to_dir(&storage, &txs[0].transfer_id, &dest).expect("accept");
        assert_eq!(acc.status, "accepted");
        assert!(dest.join("query.sql").is_file());
        assert!(!reject_transfer(&storage, "non-existent").expect("reject missing"));
    }

    #[test]
    fn test_mesh_graph_url_decoding_and_read_scope_guard() {
        use crate::storage::models::FactNodeRecord;
        use crate::storage::sqlite_provider::SqliteStorageProvider;

        let storage = SqliteStorageProvider::open_in_memory().expect("mem");
        storage
            .upsert_fact_node(&FactNodeRecord {
                id: "project:/root/new/work/测试项目".to_string(),
                node_type: "project".to_string(),
                name: "/root/new/work/测试项目".to_string(),
                label: "/root/new/work/测试项目".to_string(),
                properties: serde_json::json!({}),
                updated_at: Some("2026-11-22T00:00:00Z".into()),
            })
            .expect("upsert");

        let mut req = make_req(Method::Get, "/api/v1/mesh/graph", None, vec![]);
        req.query_params.insert(
            "q".to_string(),
            urlencoding::encode("project:/root/new/work/测试项目").into_owned(),
        );
        let resp = handle_mesh_graph(&req, &storage, &None);
        assert_eq!(resp.status, StatusCode::OK);
        let body = String::from_utf8_lossy(&resp.body);
        assert!(body.contains("/root/new/work/测试项目"), "URL 解码后的节点必须命中: {}", body);

        let mut mesh_cfg = MeshConfig::default();
        mesh_cfg
            .peer_scopes
            .insert("node-x".to_string(), vec!["exchange:dialogue".to_string()]);
        let mut denied_req = make_req(Method::Get, "/api/v1/mesh/graph", None, vec![]);
        denied_req.headers.insert("x-body-node".to_string(), "node-x".to_string());
        let deny = check_peer_read_scope_with_cfg(&denied_req, "read:facts", &mesh_cfg);
        assert!(deny.is_some());
        assert_eq!(deny.unwrap().status, StatusCode::FORBIDDEN);
    }
}
