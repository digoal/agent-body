//! 轻量级多机联邦 Peer Mesh (Phase 18.2)
//!
//! 设计取舍（规约 #2 递弱代偿 / #9 轻量化）：
//! - **不造 Raft/Gossip 轮子**：基于已有内嵌 HTTP 服务暴露 `/api/v1/mesh/*`，
//!   对等节点之间以 Bearer Token (复用 `[web].auth_token`) 互相只读联邦查询；
//! - **跨节点协作锁**：采用“资源属主节点即协调者”的 HTTP 租约模型——
//!   向目标节点 `POST /api/v1/mesh/locks/acquire`，由该节点本地 `CoordinationEngine`
//!   裁决（TTL 租约自动过期），无需分布式共识；
//! - **安全**：写操作（锁申请）强制要求服务端已配置 Token 且请求携带正确 Token。

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

/// 校验 Bearer Token；`require_token=true` 时服务端未配置 Token 也直接拒绝
fn check_mesh_auth(req: &HttpRequest, auth_token: &Option<String>, require_token: bool) -> Option<HttpResponse> {
    match auth_token {
        Some(expected) => {
            let header = req.headers.get("authorization").cloned().unwrap_or_default();
            let token = header.strip_prefix("Bearer ").unwrap_or(&header);
            if token != expected {
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
    token: Option<String>,
    timeout_ms: u64,
}

impl MeshClient {
    pub fn new(cfg: &MeshConfig, token: Option<String>) -> Self {
        Self {
            peers: cfg.peers.iter().map(|p| p.trim_end_matches('/').to_string()).collect(),
            token,
            timeout_ms: cfg.timeout_ms.max(100),
        }
    }

    pub fn peers(&self) -> &[String] {
        &self.peers
    }

    fn request(&self, peer: &str, method: &str, path: &str, body: Option<Value>) -> PeerResult {
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
        let cfg = MeshConfig { enabled: true, peers: vec![peer.clone()], timeout_ms: 3000 };

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
}
