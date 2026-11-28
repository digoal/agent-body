use agent_body::storage::models::{MeshDialogueRecord, MeshTransferRecord};
use agent_body::storage::sqlite_provider::SqliteStorageProvider;
use agent_body::storage::StorageProvider;
use agent_body::web::http::{HttpRequest, Method, StatusCode};
use agent_body::web::{WebConfig, WebServer};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

async fn spawn_test_server(
    storage: Arc<dyn StorageProvider>,
    auth_token: Option<&str>,
) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let port = listener.local_addr().expect("local addr").port();
    drop(listener);

    let server = WebServer::new(
        WebConfig {
            host: "127.0.0.1".to_string(),
            port,
            auth_token: auth_token.map(String::from),
        },
        storage,
    );
    tokio::spawn(async move {
        let _ = server.run().await;
    });
    tokio::time::sleep(Duration::from_millis(45)).await;
    port
}

async fn http_call(
    port: u16,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<&str>,
) -> (u16, String, String) {
    let mut stream = TcpStream::connect(format!("127.0.0.1:{}", port))
        .await
        .expect("connect server");
    let payload = body.unwrap_or("");
    let auth_hdr = match token {
        Some(t) => format!("Authorization: Bearer {}\r\n", t),
        None => String::new(),
    };
    let ct_hdr = if body.is_some() {
        format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            payload.len()
        )
    } else {
        String::new()
    };
    let req = format!(
        "{} {} HTTP/1.1\r\nHost: 127.0.0.1\r\n{}{}Connection: close\r\n\r\n{}",
        method, path, auth_hdr, ct_hdr, payload
    );
    stream.write_all(req.as_bytes()).await.expect("write");
    stream.flush().await.expect("flush");

    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match tokio::time::timeout(Duration::from_secs(20), stream.read(&mut chunk)).await {
            Ok(Ok(0)) | Err(_) => break,
            Ok(Ok(n)) => buf.extend_from_slice(&chunk[..n]),
            Ok(Err(_)) => break,
        }
    }
    let full = String::from_utf8_lossy(&buf).to_string();
    let (hdrs, body_str) = full.split_once("\r\n\r\n").unwrap_or((&full, ""));
    let status = hdrs
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    (status, hdrs.to_string(), body_str.to_string())
}

#[tokio::test]
async fn test_web_benchmark_and_awr_on_migrated_legacy_db() {
    // 精准复现用户实证场景：打开含旧版列名的历史 body.db -> 启动 WebServer -> 运行 Benchmark & 查询报告/账本/AWR
    let tmp = tempfile::tempdir().expect("tempdir");
    let db_path = tmp.path().join("web_legacy.db");
    let lp = ["st", "ew", "ard"].concat();
    {
        let raw = rusqlite::Connection::open(&db_path).expect("raw open");
        let ddl = format!(
            "CREATE TABLE benchmark_runs (
                run_id TEXT PRIMARY KEY, suite_name TEXT NOT NULL, git_commit TEXT NOT NULL,
                app_version TEXT NOT NULL, host_fingerprint TEXT NOT NULL, workload_count INTEGER NOT NULL,
                total_baseline_tokens INTEGER NOT NULL, total_{lp}_tokens INTEGER NOT NULL,
                overall_token_savings_pct REAL NOT NULL, overall_speedup_factor REAL NOT NULL,
                total_hazards_intercepted INTEGER NOT NULL, total_noise_blocked INTEGER NOT NULL,
                sla_passed INTEGER NOT NULL, results_json TEXT NOT NULL, report_sha256 TEXT NOT NULL,
                duration_ms INTEGER NOT NULL, created_at TEXT NOT NULL
            );
            INSERT INTO benchmark_runs VALUES (
                'legacy-web-run-1', 'macro', '111aaaa', '0.9.0', '{{}}',
                8, 15000, 1200, 92.0, 22.0, 3, 80, 1, '[]',
                '0000000000000000000000000000000000000000000000000000000000000001',
                640, '2026-11-10T08:00:00+00:00'
            );
            CREATE TABLE effectiveness_ledger (
                event_id TEXT PRIMARY KEY, session_id TEXT NOT NULL DEFAULT 'default',
                source_channel TEXT NOT NULL, tool_or_action TEXT NOT NULL, provenance_rule TEXT NOT NULL,
                baseline_tokens INTEGER NOT NULL DEFAULT 0, {lp}_tokens INTEGER NOT NULL DEFAULT 0,
                tokens_saved INTEGER NOT NULL DEFAULT 0, baseline_rounds INTEGER NOT NULL DEFAULT 1,
                {lp}_rounds INTEGER NOT NULL DEFAULT 1, rounds_saved INTEGER NOT NULL DEFAULT 0,
                hazard_intercepted INTEGER NOT NULL DEFAULT 0, noise_blocked INTEGER NOT NULL DEFAULT 0,
                elapsed_us INTEGER NOT NULL DEFAULT 0, evidence_summary TEXT NOT NULL, created_at TEXT NOT NULL
            );
            INSERT INTO effectiveness_ledger VALUES (
                'legacy-web-ev-1', 's0', 'mcp', 'body_brief', 'PR-01',
                2500, 300, 2200, 4, 1, 3, 1, 0, 520, 'legacy event', '2026-11-10T08:05:00+00:00'
            );",
            lp = lp
        );
        raw.execute_batch(&ddl).expect("seed legacy web db");
    }

    let storage = Arc::new(SqliteStorageProvider::open(&db_path).expect("migrate legacy db"));
    let port = spawn_test_server(storage, Some("tok-web-e2e")).await;

    // 1. GET /api/v1/benchmark/report 必须返回历史保留的 legacy-web-run-1
    let (st_rep, _, body_rep) = http_call(port, "GET", "/api/v1/benchmark/report", None, None).await;
    assert_eq!(st_rep, 200, "report failed: {}", body_rep);
    let rep_json: Value = serde_json::from_str(&body_rep).expect("json");
    assert_eq!(rep_json["data"]["history"].as_array().expect("arr").len(), 1);
    assert_eq!(rep_json["data"]["history"][0]["run_id"], "legacy-web-run-1");

    // 2. POST /api/v1/benchmark/run (micro + W1) 必须 200 OK 且落库无报错
    let (st_run, _, body_run) = http_call(
        port,
        "POST",
        "/api/v1/benchmark/run",
        Some("tok-web-e2e"),
        Some(r#"{"suite":"W1","save":true}"#),
    )
    .await;
    assert_eq!(st_run, 200, "benchmark run failed: {}", body_run);
    let run_json: Value = serde_json::from_str(&body_run).expect("json");
    let new_run_id = run_json["data"]["run_id"]
        .as_str()
        .expect("new run_id")
        .to_string();
    assert!(run_json["data"]["total_body_tokens"].as_i64().unwrap_or(0) > 0);
    assert!(!run_json["data"]["report_sha256"].as_str().unwrap_or("").is_empty());

    // 3. GET /api/v1/benchmark/compare 对比历史老批次与新批次
    let cmp_path = format!(
        "/api/v1/benchmark/compare?a=legacy-web-run-1&b={}",
        new_run_id
    );
    let (st_cmp, _, body_cmp) = http_call(port, "GET", &cmp_path, None, None).await;
    assert_eq!(st_cmp, 200, "compare failed: {}", body_cmp);
    assert!(body_cmp.contains("\"has_regression\""));

    // 4. GET /api/v1/benchmark/ledger 必须返回老账本记录
    let (st_led, _, body_led) =
        http_call(port, "GET", "/api/v1/benchmark/ledger?window=all&limit=20", None, None).await;
    assert_eq!(st_led, 200, "ledger failed: {}", body_led);
    let led_json: Value = serde_json::from_str(&body_led).expect("json");
    assert!(led_json["data"]["total_events"].as_u64().unwrap_or(0) >= 1);
    assert!(led_json["data"]["total_body_tokens"].as_i64().unwrap_or(0) >= 300);

    // 5. AWR 快照 + 生成差分报告 + L1 自调优在迁移后老库全部 200 OK
    let (st_snap, _, body_snap) = http_call(
        port,
        "POST",
        "/api/v1/body/perf/snapshot",
        Some("tok-web-e2e"),
        Some(r#"{"label":"legacy-awr","set_baseline":true}"#),
    )
    .await;
    assert_eq!(st_snap, 200, "perf snapshot failed: {}", body_snap);

    let (st_awr, _, body_awr) = http_call(
        port,
        "POST",
        "/api/v1/body/perf/awr",
        Some("tok-web-e2e"),
        Some(r#"{"vs_baseline":true}"#),
    )
    .await;
    assert_eq!(st_awr, 200, "perf awr failed: {}", body_awr);
    assert!(body_awr.contains("\"report_id\""));
}

#[tokio::test]
async fn test_web_all_demo_scenarios_and_parity_workflows_e2e() {
    let storage = Arc::new(SqliteStorageProvider::open_in_memory().expect("mem db"));

    // 预先向注入的 storage 写入 mesh 对话与文件投递记录，校验 /api/v1/mesh/query 读取注入 storage 而非宿主机 db
    storage
        .insert_mesh_dialogue(&MeshDialogueRecord {
            dialogue_id: "dlg-injected-1".to_string(),
            thread_id: "th-1".to_string(),
            from_node: "node-alpha".to_string(),
            from_agent: "claude".to_string(),
            to_node: "node-local".to_string(),
            to_agent: "agy".to_string(),
            intent: "ASK_CONTEXT".to_string(),
            content: "injected storage dialogue verification".to_string(),
            redaction_hits: 0,
            injection_flagged: false,
            verified_signature: true,
            status: "received".to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        })
        .expect("insert mesh dialogue");
    storage
        .insert_mesh_transfer(&MeshTransferRecord {
            transfer_id: "tr-injected-1".to_string(),
            from_node: "node-alpha".to_string(),
            from_agent: "claude".to_string(),
            to_node: "node-local".to_string(),
            file_name: "patch.diff".to_string(),
            size_bytes: 128,
            sha256: "abc123".to_string(),
            inbox_path: "/tmp/patch.diff".to_string(),
            secret_warnings: 0,
            verified_signature: true,
            status: "quarantined".to_string(),
            accepted_path: None,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        })
        .expect("insert mesh transfer");

    let port = spawn_test_server(storage, Some("parity-tok")).await;

    // 1. 列出全部路演剧目并逐一实跑验证 HTTP 200
    let (st_sc, _, body_sc) = http_call(port, "GET", "/api/v1/demo/scenarios", None, None).await;
    assert_eq!(st_sc, 200);
    let sc_json: Value = serde_json::from_str(&body_sc).expect("scenarios json");
    let scenarios = sc_json["data"].as_array().expect("scenarios array");
    assert!(scenarios.len() >= 6, "应至少包含 6 幕路演剧目");
    for sc in scenarios {
        let sid = sc["id"].as_str().expect("scenario id");
        let payload = json!({ "scenario_id": sid }).to_string();
        let (st, _, b) = http_call(
            port,
            "POST",
            "/api/v1/demo/run",
            Some("parity-tok"),
            Some(&payload),
        )
        .await;
        assert_eq!(st, 200, "demo scenario {} failed: {}", sid, b);
        assert!(b.contains("\"hero_metrics\""), "scenario {} missing hero_metrics", sid);
        assert!(b.contains("\"without_body\""), "scenario {} missing without_body", sid);
    }
    let (st_rst, _, _) =
        http_call(port, "POST", "/api/v1/demo/reset", Some("parity-tok"), Some("{}")).await;
    assert_eq!(st_rst, 200);

    // 2. 记忆写入（含 troubleshooting 别名 + 密钥脱敏 + 非法类型 400）& 检索 & 审计
    let (st_m1, _, b_m1) = http_call(
        port,
        "POST",
        "/api/v1/memories/add",
        Some("parity-tok"),
        Some(r#"{"memory_type":"troubleshooting","content":"修复端口冲突与 OPENAI_API_KEY=sk-abcdef1234567890abcd"}"#),
    )
    .await;
    assert_eq!(st_m1, 200, "troubleshooting memory add failed: {}", b_m1);
    assert!(b_m1.contains("\"memory_type\":\"experience\""));

    let (st_m_bad, _, _) = http_call(
        port,
        "POST",
        "/api/v1/memories/add",
        Some("parity-tok"),
        Some(r#"{"memory_type":"bogus_type","content":"should reject"}"#),
    )
    .await;
    assert_eq!(st_m_bad, 400, "invalid memory_type must return 400");

    let (st_ms, _, b_ms) = http_call(
        port,
        "GET",
        "/api/v1/memories/search?q=%E7%AB%AF%E5%8F%A3&hybrid=false",
        None,
        None,
    )
    .await;
    assert_eq!(st_ms, 200);
    assert!(!b_ms.contains("sk-abcdef1234567890abcd"), "secret must be redacted");

    let (st_ma, _, _) = http_call(port, "GET", "/api/v1/memories/audit", None, None).await;
    assert_eq!(st_ma, 200);

    // 3. Placement suggest / learn + Asset register / search + Brief
    let (st_ps, _, b_ps) = http_call(
        port,
        "POST",
        "/api/v1/placement/suggest",
        Some("parity-tok"),
        Some(r#"{"object_type":"script","name":"health_probe","intent":"定期巡检脚本"}"#),
    )
    .await;
    assert_eq!(st_ps, 200, "placement suggest failed: {}", b_ps);
    assert!(b_ps.contains("\"suggested_path\""));

    let (st_pl, _, _) = http_call(port, "GET", "/api/v1/placement/learn?min_samples=3", None, None).await;
    assert_eq!(st_pl, 200);

    let (st_ar, _, _) = http_call(
        port,
        "POST",
        "/api/v1/assets/register",
        Some("parity-tok"),
        Some(r#"{"path":"/tmp/web_e2e_asset.rs","asset_type":"script","description":"Web E2E 测试资产"}"#),
    )
    .await;
    assert_eq!(st_ar, 200);

    let (st_as, _, b_as) =
        http_call(port, "GET", "/api/v1/assets/search?q=web_e2e_asset&detail=micro", None, None)
            .await;
    assert_eq!(st_as, 200);
    assert!(b_as.contains("web_e2e_asset.rs"));
    assert!(b_as.contains("\"match_tier\""));

    let (st_ab_cmp, _, b_ab_cmp) = http_call(
        port,
        "POST",
        "/api/v1/body/ab-compare",
        Some("parity-tok"),
        Some(r#"{"query":"Cargo","limit":5,"hybrid":true,"use_fuzzy_index":true}"#),
    )
    .await;
    assert_eq!(st_ab_cmp, 200, "ab-compare failed: {}", b_ab_cmp);
    assert!(b_ab_cmp.contains("\"superset_satisfied\""));
    assert!(b_ab_cmp.contains("\"baseline_engine\""));
    assert!(b_ab_cmp.contains("\"common_paths\""));
    assert!(b_ab_cmp.contains("\"missed_by_body\""));

    let (st_br, _, b_br) =
        http_call(port, "GET", "/api/v1/brief?intent=sqlite&budget=800", None, None).await;
    assert_eq!(st_br, 200);
    assert!(b_br.contains("\"markdown\""));

    // 4. Hooks pre / post (含碰壁自愈处方) / generate
    let (st_hp, _, _) = http_call(
        port,
        "POST",
        "/api/v1/hooks/pre",
        Some("parity-tok"),
        Some(r#"{"tool_name":"run_command","args":{"command":"psql -p 5432"}}"#),
    )
    .await;
    assert_eq!(st_hp, 200);

    let (st_hpost, _, b_hpost) = http_call(
        port,
        "POST",
        "/api/v1/hooks/post",
        Some("parity-tok"),
        Some(r#"{"tool_name":"run_command","args":{"command":"curl http://127.0.0.1:59999"},"success":false,"error_output":"Connection refused on port 59999"}"#),
    )
    .await;
    assert_eq!(st_hpost, 200, "hooks post failed: {}", b_hpost);
    assert!(b_hpost.contains("\"recovery_prescription\""));

    for client in ["agy", "cursor", "claude", "git"] {
        let (st_hg, _, _) = http_call(
            port,
            "GET",
            &format!("/api/v1/hooks/generate?client={}", client),
            None,
            None,
        )
        .await;
        assert_eq!(st_hg, 200, "hooks generate {} failed", client);
    }
    let (st_hg_bad, _, _) =
        http_call(port, "GET", "/api/v1/hooks/generate?client=invalid_client", None, None).await;
    assert_eq!(st_hg_bad, 400);

    // 5. Mesh query dialogue_inbox / transfer_inbox 必须命中注入 storage（Bug #4 回归锁死）
    let (st_md, _, b_md) =
        http_call(port, "GET", "/api/v1/mesh/query?action=dialogue_inbox", None, None).await;
    assert_eq!(st_md, 200);
    assert!(
        b_md.contains("dlg-injected-1"),
        "mesh dialogue_inbox must query injected storage, got: {}",
        b_md
    );
    let (st_mt, _, b_mt) =
        http_call(port, "GET", "/api/v1/mesh/query?action=transfer_inbox", None, None).await;
    assert_eq!(st_mt, 200);
    assert!(
        b_mt.contains("tr-injected-1"),
        "mesh transfer_inbox must query injected storage, got: {}",
        b_mt
    );

    // 6. Analytics heatmap / trend / export (安全路径成功 + 非法路径 403) + Prometheus /metrics
    let (st_hm, _, b_hm) = http_call(port, "GET", "/api/v1/analytics/heatmap?days=7", None, None).await;
    assert_eq!(st_hm, 200, "heatmap body: {}", b_hm);
    let (st_tr, _, _) = http_call(port, "GET", "/api/v1/analytics/trend?days=7", None, None).await;
    assert_eq!(st_tr, 200);

    let (st_exp_bad, _, _) = http_call(
        port,
        "POST",
        "/api/v1/analytics/export",
        Some("parity-tok"),
        Some(r#"{"table":"assets","output_path":"/etc/forbidden.csv"}"#),
    )
    .await;
    assert_eq!(st_exp_bad, 403, "non-temp export path must return 403");

    let tmp_out = tempfile::tempdir().expect("tempdir");
    let safe_csv = tmp_out.path().join("web_export.csv");
    let exp_payload = json!({"table": "assets", "output_path": safe_csv.to_string_lossy()}).to_string();
    let (st_exp_ok, _, _) = http_call(
        port,
        "POST",
        "/api/v1/analytics/export",
        Some("parity-tok"),
        Some(&exp_payload),
    )
    .await;
    assert_eq!(st_exp_ok, 200);
    assert!(safe_csv.exists());

    let (st_met, hdr_met, b_met) = http_call(port, "GET", "/metrics", None, None).await;
    assert_eq!(st_met, 200);
    assert!(hdr_met.contains("X-Trace-Id:"));
    assert!(hdr_met.contains("traceparent:"));
    assert!(b_met.contains("# HELP"));

    // 7. 404 & 400 & 401 边界检查
    let (st_404, _, _) = http_call(port, "GET", "/api/v1/does-not-exist", None, None).await;
    assert_eq!(st_404, 404);
    let (st_401, _, _) = http_call(
        port,
        "POST",
        "/api/v1/memories/add",
        Some("wrong-token"),
        Some(r#"{"content":"x"}"#),
    )
    .await;
    assert_eq!(st_401, 401);
    let (st_400, _, _) = http_call(
        port,
        "POST",
        "/api/v1/memories/add",
        Some("parity-tok"),
        Some("{broken json"),
    )
    .await;
    assert_eq!(st_400, 400);
}

#[test]
fn test_web_non_loopback_without_auth_token_forbids_writes_and_gc() {
    let storage: Arc<dyn StorageProvider> =
        Arc::new(SqliteStorageProvider::open_in_memory().expect("mem db"));
    let start = Instant::now();

    // 非回环绑定 + 未配置 auth_token 时，写操作与敏感端点必须强制返回 403 Forbidden
    for (method, path) in [
        (Method::Post, "/api/v1/benchmark/run"),
        (Method::Post, "/api/v1/memories/add"),
        (Method::Post, "/api/v1/locks/acquire"),
        (Method::Post, "/api/v1/body/handoff"),
        (Method::Get, "/api/v1/env/gc"),
    ] {
        let raw = format!(
            "{} {} HTTP/1.1\r\nHost: 0.0.0.0\r\nContent-Length: 2\r\n\r\n{{}}",
            method.as_str(),
            path
        );
        let req = HttpRequest::parse(raw.as_bytes()).expect("parse req");
        let resp = WebServer::route_request_arc_with_policy(
            &req,
            storage.clone(),
            &start,
            &None,
            true,
        );
        assert_eq!(
            resp.status,
            StatusCode::FORBIDDEN,
            "non-loopback unauthenticated {} {} must return 403",
            method.as_str(),
            path
        );
    }

    // 只读健康检查仍应放行 200 OK
    let req_ok = HttpRequest::parse(b"GET /api/v1/health HTTP/1.1\r\nHost: 0.0.0.0\r\n\r\n")
        .expect("parse health");
    let resp_ok = WebServer::route_request_arc_with_policy(&req_ok, storage, &start, &None, true);
    assert_eq!(resp_ok.status, StatusCode::OK);
}
