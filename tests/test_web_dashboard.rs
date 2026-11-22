use std::sync::Arc;
use std::time::Duration;
use agent_body::storage::sqlite_provider::SqliteStorageProvider;
use agent_body::storage::{StorageProvider, CoordinationLock};
use agent_body::web::{WebServer, WebConfig};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[tokio::test]
async fn test_web_dashboard_e2e_lifecycle() {
    // 1. 初始化测试存储层
    let storage = Arc::new(SqliteStorageProvider::open_in_memory().expect("open memory db"));
    storage.init().expect("init storage");

    // 预先插入一条协作锁
    let test_lock = CoordinationLock {
        lock_id: "lock-web-test-99".to_string(),
        resource_type: "port".to_string(),
        resource_id: "8888".to_string(),
        scope: "exclusive".to_string(),
        holder_agent: "agent_web_tester".to_string(),
        holder_pid: Some(9999),
        acquired_at: chrono::Utc::now().to_rfc3339(),
        expires_at: (chrono::Utc::now() + chrono::Duration::seconds(120)).to_rfc3339(),
        metadata: None,
    };
    storage.insert_coordination_lock(&test_lock).expect("insert coordination lock");

    // 2. 选择一个可用的随机端口启动 Web 服务器
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind free port");
    let bound_addr = listener.local_addr().expect("local addr");
    let bound_port = bound_addr.port();
    drop(listener); // 释放端口供 WebServer 使用

    let config = WebConfig {
        host: "127.0.0.1".to_string(),
        port: bound_port,
        auth_token: Some("secret123".to_string()),
    };

    let server = WebServer::new(config, storage.clone());
    let broadcaster = server.event_broadcaster();

    // 在后台任务中启动 Web 服务器
    tokio::spawn(async move {
        let _ = server.run().await;
    });

    // 稍微等待服务器监听完成
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 辅助函数: 发送 HTTP 请求并完整读取响应体
    async fn send_http_request(host: &str, port: u16, raw_req: &str) -> String {
        let mut stream = TcpStream::connect(format!("{}:{}", host, port)).await.expect("connect web server");
        stream.write_all(raw_req.as_bytes()).await.expect("write request");
        stream.flush().await.expect("flush");

        let mut full = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            match tokio::time::timeout(Duration::from_millis(15000), stream.read(&mut buf)).await {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(n)) => full.extend_from_slice(&buf[..n]),
                Ok(Err(_)) => break,
            }
        }
        String::from_utf8_lossy(&full).to_string()
    }

    // 3. 验证静态资源
    // 3.1 首页 HTML (Web UI 2.0 必须包含五大工作台、双主题切换及路演故事舞台)
    let resp_index = send_http_request("127.0.0.1", bound_port, "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
    assert!(resp_index.contains("HTTP/1.1 200 OK"));
    assert!(resp_index.contains("Content-Type: text/html"));
    assert!(resp_index.contains("Agent Body"));
    assert!(resp_index.contains("view-ecosystem"));
    assert!(resp_index.contains("view-governance"));
    assert!(resp_index.contains("data-theme"));
    assert!(resp_index.contains("btn-theme-toggle"));
    assert!(resp_index.contains("v0.4.0"));
    assert!(resp_index.contains("43/43 对齐"));
    assert!(resp_index.contains("view-audit"));
    assert!(resp_index.contains("view-body"));

    // 3.2 静态 CSS (验证包含白天与夜间双色调语义变量)
    let resp_css = send_http_request("127.0.0.1", bound_port, "GET /app.css HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
    assert!(resp_css.contains("HTTP/1.1 200 OK"));
    assert!(resp_css.contains("Content-Type: text/css"));
    assert!(resp_css.contains("data-theme=\"light\""));
    assert!(resp_css.contains("--canvas-bg"));

    // 3.3 静态 JS (确认已彻底移除 drawMockGraph 且包含双主题与路演组件)
    let resp_js = send_http_request("127.0.0.1", bound_port, "GET /app.js HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
    assert!(resp_js.contains("HTTP/1.1 200 OK"));
    assert!(resp_js.contains("Content-Type: application/javascript"));
    assert!(!resp_js.contains("drawMockGraph"));
    assert!(resp_js.contains("toggleTheme"));
    assert!(resp_js.contains("renderDemoSceneWidget"));

    // 4. 验证 REST API
    // 4.1 GET /api/v1/health
    let resp_health = send_http_request("127.0.0.1", bound_port, "GET /api/v1/health HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
    assert!(resp_health.contains("HTTP/1.1 200 OK"));
    assert!(resp_health.contains("\"status\":\"ok\""));

    // 4.2 GET /api/v1/overview
    let resp_overview = send_http_request("127.0.0.1", bound_port, "GET /api/v1/overview HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
    assert!(resp_overview.contains("HTTP/1.1 200 OK"));
    assert!(resp_overview.contains("hostname"));
    assert!(resp_overview.contains("cpu_cores"));

    // 4.3 GET /api/v1/topology
    let resp_topo = send_http_request("127.0.0.1", bound_port, "GET /api/v1/topology HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
    assert!(resp_topo.contains("HTTP/1.1 200 OK"));
    assert!(resp_topo.contains("nodes"));
    assert!(resp_topo.contains("summary"));

    // 4.4 GET /api/v1/locks (验证查到活跃锁)
    let resp_locks = send_http_request("127.0.0.1", bound_port, "GET /api/v1/locks HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
    assert!(resp_locks.contains("HTTP/1.1 200 OK"));
    assert!(resp_locks.contains("8888"));
    assert!(resp_locks.contains("agent_web_tester"));

    // 4.5 POST /api/v1/locks/release (无 Token 应报 401 Unauthorized)
    let post_unauth = "POST /api/v1/locks/release HTTP/1.1\r\n\
Host: 127.0.0.1\r\n\
Content-Type: application/json\r\n\
Content-Length: 45\r\n\r\n\
{\"resource_type\":\"port\",\"resource_id\":\"8888\"}";
    let resp_unauth = send_http_request("127.0.0.1", bound_port, post_unauth).await;
    assert!(resp_unauth.contains("HTTP/1.1 401 Unauthorized"));

    // 4.6 POST /api/v1/locks/release (带有效 Token 正常释放)
    let post_auth = "POST /api/v1/locks/release HTTP/1.1\r\n\
Host: 127.0.0.1\r\n\
Authorization: Bearer secret123\r\n\
Content-Type: application/json\r\n\
Content-Length: 75\r\n\r\n\
{\"resource_type\":\"port\",\"resource_id\":\"8888\",\"agent_id\":\"agent_web_tester\"}";
    let resp_auth = send_http_request("127.0.0.1", bound_port, post_auth).await;
    assert!(resp_auth.contains("HTTP/1.1 200 OK"));
    assert!(resp_auth.contains("\"released\":true"));

    // 4.7 验证释放后再次查询活跃锁已为空
    let resp_locks_empty = send_http_request("127.0.0.1", bound_port, "GET /api/v1/locks HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
    assert!(resp_locks_empty.contains("[]"));

    // 4.7.1 验证通过 lock_id + force=true 也能释放协作锁
    let test_lock_2 = CoordinationLock {
        lock_id: "lock-web-test-by-id-100".to_string(),
        resource_type: "file".to_string(),
        resource_id: "/tmp/web_release_by_id.txt".to_string(),
        scope: "exclusive".to_string(),
        holder_agent: "other_holder".to_string(),
        holder_pid: None,
        acquired_at: chrono::Utc::now().to_rfc3339(),
        expires_at: (chrono::Utc::now() + chrono::Duration::seconds(120)).to_rfc3339(),
        metadata: None,
    };
    storage.insert_coordination_lock(&test_lock_2).expect("insert lock 2");
    let del_lock_body = "{\"lock_id\":\"lock-web-test-by-id-100\",\"holder_agent\":\"admin\",\"force\":true}";
    let del_lock_req = format!(
        "POST /api/v1/locks/release HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer secret123\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        del_lock_body.len(),
        del_lock_body
    );
    let resp_del_lock = send_http_request("127.0.0.1", bound_port, &del_lock_req).await;
    assert!(resp_del_lock.contains("HTTP/1.1 200 OK"));
    assert!(resp_del_lock.contains("\"released\":true"));

    // 4.8 验证 Web 2.0 新增全景接口 (/facts, /ecosystem, /ecosystem/route, /governance, /mesh/status, /locks/history)
    for ep in [
        "/api/v1/facts",
        "/api/v1/ecosystem",
        "/api/v1/ecosystem/route?intent=postgres&budget=600",
        "/api/v1/governance",
        "/api/v1/mesh/status",
        "/api/v1/locks/history",
        "/api/v1/parity/matrix",
        "/api/v1/demo/scenarios",
        "/api/v1/demo/roi",
        "/api/v1/benchmark/workloads",
        "/api/v1/benchmark/report",
        "/api/v1/benchmark/ledger?window=7d",
        "/api/v1/audit/events?window=7d",
        "/api/v1/audit/summary?window=7d",
        "/api/v1/body/manifest",
        "/api/v1/body/vitals",
        "/api/v1/body/envelope",
        "/api/v1/body/checkpoints",
        "/deck",
        "/presentation",
    ] {
        let req = format!("GET {} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n", ep);
        let resp = send_http_request("127.0.0.1", bound_port, &req).await;
        assert!(resp.contains("HTTP/1.1 200 OK"), "端点 {} 应返回 200 OK", ep);
    }

    // 4.8.1 验证 43/43 黑白屏同步矩阵覆盖率 = 100% 且关键 CLI 映射准确
    let resp_parity = send_http_request("127.0.0.1", bound_port, "GET /api/v1/parity/matrix HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
    assert!(resp_parity.contains("\"total_mcp_tools\":43"));
    assert!(resp_parity.contains("\"synced_count\":43"));
    assert!(resp_parity.contains("\"coverage_pct\":100.0"));
    assert!(resp_parity.contains("agent-body links graph"));
    assert!(resp_parity.contains("agent-body coordination deadlock"));

    // 4.8.1c 验证 Agent 本体中枢跨大脑接力胶囊写端点鉴权与流转
    let handoff_body = "{\"from_brain\":\"antigravity\",\"goal_summary\":\"Verify PG WAL config\"}";
    let handoff_unauth = format!(
        "POST /api/v1/body/handoff HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        handoff_body.len(),
        handoff_body
    );
    let resp_handoff_unauth = send_http_request("127.0.0.1", bound_port, &handoff_unauth).await;
    assert!(resp_handoff_unauth.contains("HTTP/1.1 401 Unauthorized"));

    let handoff_auth = format!(
        "POST /api/v1/body/handoff HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer secret123\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        handoff_body.len(),
        handoff_body
    );
    let resp_handoff_auth = send_http_request("127.0.0.1", bound_port, &handoff_auth).await;
    assert!(resp_handoff_auth.contains("HTTP/1.1 200 OK"));
    assert!(resp_handoff_auth.contains("Verify PG WAL config"));

    let resume_auth = "POST /api/v1/body/resume HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer secret123\r\nContent-Type: application/json\r\nContent-Length: 26\r\n\r\n{\"by_brain\":\"claude-code\"}";
    let resp_resume_auth = send_http_request("127.0.0.1", bound_port, resume_auth).await;
    assert!(resp_resume_auth.contains("HTTP/1.1 200 OK"));
    assert!(resp_resume_auth.contains("\"resumed\":true"));

    // 4.8.1b 验证双域审计自进化写端点受 Bearer 鉴权保护
    let evolve_unauth = "POST /api/v1/audit/evolve HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 2\r\n\r\n{}";
    let resp_evolve_unauth = send_http_request("127.0.0.1", bound_port, evolve_unauth).await;
    assert!(resp_evolve_unauth.contains("HTTP/1.1 401 Unauthorized"));

    let evolve_auth = "POST /api/v1/audit/evolve HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer secret123\r\nContent-Length: 2\r\n\r\n{}";
    let resp_evolve_auth = send_http_request("127.0.0.1", bound_port, evolve_auth).await;
    assert!(resp_evolve_auth.contains("HTTP/1.1 200 OK"));
    assert!(resp_evolve_auth.contains("\"evaluated_events\""));

    // 4.8.2 验证 Benchmark 跑批与重置 API (无 Token 应被 401 拦截，带 Token 放行)
    let bench_body = "{\"suite\":\"macro\",\"save\":true}";
    let bench_unauth_req = format!(
        "POST /api/v1/benchmark/run HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        bench_body.len(),
        bench_body
    );
    let resp_bench_unauth = send_http_request("127.0.0.1", bound_port, &bench_unauth_req).await;
    assert!(resp_bench_unauth.contains("HTTP/1.1 401 Unauthorized"));

    let bench_req = format!(
        "POST /api/v1/benchmark/run HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer secret123\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        bench_body.len(),
        bench_body
    );
    let resp_bench = send_http_request("127.0.0.1", bound_port, &bench_req).await;
    assert!(resp_bench.contains("HTTP/1.1 200 OK"));
    assert!(resp_bench.contains("\"report_sha256\""));

    // 4.9 验证路演剧场执行与沙盒复位 E2E
    let resp_scenarios = send_http_request("127.0.0.1", bound_port, "GET /api/v1/demo/scenarios HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").await;
    assert!(resp_scenarios.contains("\"analogy\""));
    assert!(resp_scenarios.contains("\"intent_summary\""));

    let demo_body = "{\"scenario_id\":\"scene-4\"}";
    let demo_req = format!(
        "POST /api/v1/demo/run HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer secret123\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        demo_body.len(),
        demo_body
    );
    let resp_demo = send_http_request("127.0.0.1", bound_port, &demo_req).await;
    assert!(resp_demo.contains("HTTP/1.1 200 OK"));
    assert!(resp_demo.contains("\"has_deadlock\":true"));
    assert!(resp_demo.contains("\"analogy\""));
    assert!(resp_demo.contains("\"intent_summary\""));
    assert!(resp_demo.contains("\"hero_metrics\""));

    let reset_req = "POST /api/v1/demo/reset HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer secret123\r\nContent-Length: 2\r\n\r\n{}";
    let resp_reset = send_http_request("127.0.0.1", bound_port, reset_req).await;
    assert!(resp_reset.contains("HTTP/1.1 200 OK"));

    // 4.10 验证 Phase 50 内生 AWR 性能快照、差分报告与 L1 自调优 Web API (写接口鉴权 + 读接口响应)
    let snap_unauth = "POST /api/v1/body/perf/snapshot HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
    let resp_snap_unauth = send_http_request("127.0.0.1", bound_port, snap_unauth).await;
    assert!(resp_snap_unauth.contains("HTTP/1.1 401 Unauthorized"));

    let snap_body = "{\"label\":\"e2e-golden\",\"set_baseline\":true}";
    let snap_req = format!(
        "POST /api/v1/body/perf/snapshot HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer secret123\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        snap_body.len(),
        snap_body
    );
    let resp_snap = send_http_request("127.0.0.1", bound_port, &snap_req).await;
    assert!(resp_snap.contains("HTTP/1.1 200 OK"));
    assert!(resp_snap.contains("\"is_baseline\":true"));

    let resp_awr = send_http_request(
        "127.0.0.1",
        bound_port,
        "GET /api/v1/body/perf/awr?vs_baseline=true HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    )
    .await;
    assert!(resp_awr.contains("HTTP/1.1 200 OK"));
    assert!(resp_awr.contains("\"addm_findings\""));
    assert!(resp_awr.contains("\"reports_meta\":[]"), "只读 GET 不应持久化 AWR 报告");

    let awr_post_unauth = "POST /api/v1/body/perf/awr HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}";
    let resp_awr_post_unauth = send_http_request("127.0.0.1", bound_port, awr_post_unauth).await;
    assert!(resp_awr_post_unauth.contains("HTTP/1.1 401 Unauthorized"));

    let awr_post_body = "{\"vs_baseline\":true}";
    let awr_post_auth = format!(
        "POST /api/v1/body/perf/awr HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer secret123\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        awr_post_body.len(),
        awr_post_body
    );
    let resp_awr_post = send_http_request("127.0.0.1", bound_port, &awr_post_auth).await;
    assert!(resp_awr_post.contains("HTTP/1.1 200 OK"));
    assert!(resp_awr_post.contains("\"reports_meta\":[{"));

    // 从响应体中提取 report_id 并验证 /api/v1/body/perf/delete 受鉴权保护且同步清理索引
    let del_unauth = "POST /api/v1/body/perf/delete HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: 31\r\n\r\n{\"kind\":\"report\",\"id\":\"awr-xx\"}";
    let resp_del_unauth = send_http_request("127.0.0.1", bound_port, del_unauth).await;
    assert!(resp_del_unauth.contains("HTTP/1.1 401 Unauthorized"));

    let json_start = resp_awr_post.find("\r\n\r\n").expect("http body") + 4;
    let parsed_awr: serde_json::Value = serde_json::from_str(&resp_awr_post[json_start..]).expect("awr json");
    let created_rid = parsed_awr["data"]["report"]["report_id"].as_str().expect("report_id");
    let del_body = serde_json::json!({ "kind": "report", "id": created_rid }).to_string();
    let del_auth = format!(
        "POST /api/v1/body/perf/delete HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer secret123\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        del_body.len(),
        del_body
    );
    let resp_del_auth = send_http_request("127.0.0.1", bound_port, &del_auth).await;
    assert!(resp_del_auth.contains("HTTP/1.1 200 OK"));
    assert!(resp_del_auth.contains("\"deleted\":true"));
    assert!(resp_del_auth.contains("\"reports_meta\":[]"));

    let mermaid_write_unauth = send_http_request(
        "127.0.0.1",
        bound_port,
        "GET /api/v1/links/mermaid?discover_semantic=true HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
    )
    .await;
    assert!(mermaid_write_unauth.contains("HTTP/1.1 401 Unauthorized"));

    let mermaid_write_auth = send_http_request(
        "127.0.0.1",
        bound_port,
        "GET /api/v1/links/mermaid?discover_semantic=true HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer secret123\r\n\r\n",
    )
    .await;
    assert!(mermaid_write_auth.contains("HTTP/1.1 200 OK"));

    let tune_req = "POST /api/v1/body/perf/tune HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer secret123\r\nContent-Length: 2\r\n\r\n{}";
    let resp_tune = send_http_request("127.0.0.1", bound_port, tune_req).await;
    assert!(resp_tune.contains("HTTP/1.1 200 OK"));
    assert!(resp_tune.contains("\"actions_executed\""));

    // 5. 验证 SSE (Server-Sent Events) 长连接推流
    let mut sse_stream = TcpStream::connect(format!("127.0.0.1:{}", bound_port)).await.expect("connect sse");
    let sse_req = "GET /api/v1/stream HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n";
    sse_stream.write_all(sse_req.as_bytes()).await.expect("write sse req");
    sse_stream.flush().await.expect("flush sse");

    let mut sse_buf = vec![0u8; 1024];
    let n = sse_stream.read(&mut sse_buf).await.expect("read sse welcome");
    let sse_welcome = String::from_utf8_lossy(&sse_buf[..n]);
    assert!(sse_welcome.contains("HTTP/1.1 200 OK"));
    assert!(sse_welcome.contains("Content-Type: text/event-stream"));
    assert!(sse_welcome.contains("Connected to Agent Steward Event Stream"));

    // 通过广播通道触发一次外部事件
    broadcaster.send("{\"event\":\"test_alert\",\"detail\":\"Host port scan complete\"}".to_string()).expect("send broadcast");

    let n2 = sse_stream.read(&mut sse_buf).await.expect("read sse broadcast");
    let sse_event = String::from_utf8_lossy(&sse_buf[..n2]);
    assert!(sse_event.contains("test_alert"));
    assert!(sse_event.contains("Host port scan complete"));
}

#[tokio::test]
async fn test_web_server_fragmented_and_large_headers() {
    let storage = Arc::new(SqliteStorageProvider::open_in_memory().expect("open memory db"));
    storage.init().expect("init storage");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind free port");
    let bound_port = listener.local_addr().expect("local addr").port();
    drop(listener);

    let config = WebConfig {
        host: "127.0.0.1".to_string(),
        port: bound_port,
        auth_token: None,
    };
    let server = WebServer::new(config, storage);
    tokio::spawn(async move {
        let _ = server.run().await;
    });
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 1. 分包发送 HTTP 请求头（两次独立 TCP write）
    let mut s1 = TcpStream::connect(format!("127.0.0.1:{}", bound_port)).await.expect("connect s1");
    s1.write_all(b"GET /api/v1/health HTTP/1.1\r\nHo").await.expect("write part 1");
    s1.flush().await.expect("flush 1");
    tokio::time::sleep(Duration::from_millis(30)).await;
    s1.write_all(b"st: 127.0.0.1\r\n\r\n").await.expect("write part 2");
    s1.flush().await.expect("flush 2");

    let mut resp1 = Vec::new();
    s1.read_to_end(&mut resp1).await.expect("read resp1");
    let text1 = String::from_utf8_lossy(&resp1);
    assert!(text1.contains("HTTP/1.1 200 OK"), "分包头应正常解析并返回 200 OK: {}", text1);

    // 2. 发送 >4KB (5KB) 自定义请求头
    let mut s2 = TcpStream::connect(format!("127.0.0.1:{}", bound_port)).await.expect("connect s2");
    let padding = "x".repeat(5120);
    let large_hdr_req = format!(
        "GET /api/v1/health HTTP/1.1\r\nHost: 127.0.0.1\r\nX-Padding: {}\r\n\r\n",
        padding
    );
    s2.write_all(large_hdr_req.as_bytes()).await.expect("write large header");
    s2.flush().await.expect("flush s2");

    let mut resp2 = Vec::new();
    s2.read_to_end(&mut resp2).await.expect("read resp2");
    let text2 = String::from_utf8_lossy(&resp2);
    assert!(text2.contains("HTTP/1.1 200 OK"), "5KB 大请求头应正常解析: {}", text2);

    // 3. 发送 >32KB 无结束符超限请求头，应返回 400 Bad Request
    let mut s3 = TcpStream::connect(format!("127.0.0.1:{}", bound_port)).await.expect("connect s3");
    let oversized = format!("GET /api/v1/health HTTP/1.1\r\nX-Huge: {}", "a".repeat(34 * 1024));
    let _ = s3.write_all(oversized.as_bytes()).await;
    let _ = s3.flush().await;

    let mut resp3 = Vec::new();
    let _ = s3.read_to_end(&mut resp3).await;
    let text3 = String::from_utf8_lossy(&resp3);
    assert!(text3.contains("400 Bad Request"), "超 32KB 头必须返回 400: {}", text3);
}
