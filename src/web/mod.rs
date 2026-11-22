pub mod http;
pub mod api;
pub mod topology;
pub mod locks_api;
pub mod parity_api;
pub mod assets;
pub mod server;

pub use server::{WebServer, WebConfig};

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use crate::storage::sqlite_provider::SqliteStorageProvider;
    use crate::storage::StorageProvider;
    use crate::storage::CoordinationLock;
    use crate::web::http::{HttpRequest, Method, StatusCode};

    #[test]
    fn test_web_health_and_overview_routing() {
        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite memory init");
        storage.init().expect("sqlite init");
        let start_time = Instant::now();

        // 1. 测试 /api/v1/health
        let req_health = HttpRequest {
            method: Method::Get,
            path: "/api/v1/health".to_string(),
            raw_query: "".to_string(),
            query_params: Default::default(),
            headers: Default::default(),
            body: Vec::new(),
        };

        let resp_health = server::WebServer::route_request(&req_health, &storage, &start_time, &None);
        assert_eq!(resp_health.status, StatusCode::OK);
        let body_str = std::str::from_utf8(&resp_health.body).unwrap();
        assert!(body_str.contains("\"status\":\"ok\""));
        assert!(body_str.contains("\"code\":200"));

        // 2. 测试 /api/v1/overview
        let req_overview = HttpRequest {
            method: Method::Get,
            path: "/api/v1/overview".to_string(),
            raw_query: "".to_string(),
            query_params: Default::default(),
            headers: Default::default(),
            body: Vec::new(),
        };

        let resp_overview = server::WebServer::route_request(&req_overview, &storage, &start_time, &None);
        assert_eq!(resp_overview.status, StatusCode::OK);
        let body_overview = std::str::from_utf8(&resp_overview.body).unwrap();
        assert!(body_overview.contains("hostname"));
        assert!(body_overview.contains("cpu_cores"));

        // 3. 测试 404
        let req_404 = HttpRequest {
            method: Method::Get,
            path: "/api/v1/not_exist".to_string(),
            raw_query: "".to_string(),
            query_params: Default::default(),
            headers: Default::default(),
            body: Vec::new(),
        };
        let resp_404 = server::WebServer::route_request(&req_404, &storage, &start_time, &None);
        assert_eq!(resp_404.status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn test_web_topology_routing() {
        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite memory init");
        storage.init().expect("sqlite init");
        let start_time = Instant::now();

        let req = HttpRequest {
            method: Method::Get,
            path: "/api/v1/topology".to_string(),
            raw_query: "".to_string(),
            query_params: Default::default(),
            headers: Default::default(),
            body: Vec::new(),
        };

        let resp = server::WebServer::route_request(&req, &storage, &start_time, &None);
        assert_eq!(resp.status, StatusCode::OK);
        let body = std::str::from_utf8(&resp.body).unwrap();
        assert!(body.contains("summary"));
        assert!(body.contains("nodes"));
        assert!(body.contains("edges"));
    }

    #[test]
    fn test_web_embedded_assets_serving() {
        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite memory init");
        storage.init().expect("sqlite init");
        let start_time = Instant::now();

        // 首页 HTML
        let req_index = HttpRequest {
            method: Method::Get,
            path: "/".to_string(),
            raw_query: "".to_string(),
            query_params: Default::default(),
            headers: Default::default(),
            body: Vec::new(),
        };
        let resp_index = server::WebServer::route_request(&req_index, &storage, &start_time, &None);
        assert_eq!(resp_index.status, StatusCode::OK);
        assert_eq!(resp_index.headers.get("Content-Type").unwrap(), "text/html; charset=utf-8");

        // CSS
        let req_css = HttpRequest {
            method: Method::Get,
            path: "/app.css".to_string(),
            raw_query: "".to_string(),
            query_params: Default::default(),
            headers: Default::default(),
            body: Vec::new(),
        };
        let resp_css = server::WebServer::route_request(&req_css, &storage, &start_time, &None);
        assert_eq!(resp_css.status, StatusCode::OK);
        assert_eq!(resp_css.headers.get("Content-Type").unwrap(), "text/css; charset=utf-8");

        // JS
        let req_js = HttpRequest {
            method: Method::Get,
            path: "/app.js".to_string(),
            raw_query: "".to_string(),
            query_params: Default::default(),
            headers: Default::default(),
            body: Vec::new(),
        };
        let resp_js = server::WebServer::route_request(&req_js, &storage, &start_time, &None);
        assert_eq!(resp_js.status, StatusCode::OK);
        assert_eq!(resp_js.headers.get("Content-Type").unwrap(), "application/javascript; charset=utf-8");
    }

    #[test]
    fn test_web_locks_routing_and_release() {
        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite memory init");
        storage.init().expect("sqlite init");
        let start_time = Instant::now();

        // 插入测试锁
        let lock = CoordinationLock {
            lock_id: "lock-test-01".to_string(),
            resource_type: "port".to_string(),
            resource_id: "9000".to_string(),
            scope: "exclusive".to_string(),
            holder_agent: "agent_alpha".to_string(),
            holder_pid: Some(1234),
            acquired_at: chrono::Utc::now().to_rfc3339(),
            expires_at: (chrono::Utc::now() + chrono::Duration::seconds(60)).to_rfc3339(),
            metadata: None,
        };
        storage.insert_coordination_lock(&lock).expect("insert lock");

        // 1. 查询 GET /api/v1/locks
        let req_list = HttpRequest {
            method: Method::Get,
            path: "/api/v1/locks".to_string(),
            raw_query: "".to_string(),
            query_params: Default::default(),
            headers: Default::default(),
            body: Vec::new(),
        };
        let resp_list = server::WebServer::route_request(&req_list, &storage, &start_time, &None);
        assert_eq!(resp_list.status, StatusCode::OK);
        let body_list = std::str::from_utf8(&resp_list.body).unwrap();
        assert!(body_list.contains("port"));
        assert!(body_list.contains("9000"));
        assert!(body_list.contains("agent_alpha"));

        // 2. 模拟 POST /api/v1/locks/release
        let release_payload = serde_json::json!({
            "resource_type": "port",
            "resource_id": "9000",
            "agent_id": "agent_alpha"
        });
        let req_release = HttpRequest {
            method: Method::Post,
            path: "/api/v1/locks/release".to_string(),
            raw_query: "".to_string(),
            query_params: Default::default(),
            headers: Default::default(),
            body: serde_json::to_vec(&release_payload).unwrap(),
        };
        let resp_release = server::WebServer::route_request(&req_release, &storage, &start_time, &None);
        assert_eq!(resp_release.status, StatusCode::OK);
        let body_release = std::str::from_utf8(&resp_release.body).unwrap();
        assert!(body_release.contains("\"released\":true"));

        // 3. 再次查询确认已释放
        let resp_after = server::WebServer::route_request(&req_list, &storage, &start_time, &None);
        let body_after = std::str::from_utf8(&resp_after.body).unwrap();
        assert!(body_after.contains("[]"));
    }

    #[test]
    fn test_web_topology_connects_fact_edges_and_process_ports() {
        use crate::storage::models::{ActiveProcessRecord, FactEdgeRecord, FactNodeRecord};

        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite memory init");
        storage.init().expect("sqlite init");

        // 插入真实事实节点与边
        storage.upsert_fact_node(&FactNodeRecord {
            id: "service:agent-steward".to_string(),
            node_type: "service".to_string(),
            name: "agent-steward".to_string(),
            label: "Agent Steward Service".to_string(),
            properties: serde_json::json!({"unit": "agent-steward.service"}),
            updated_at: None,
        }).expect("upsert service");

        storage.upsert_fact_node(&FactNodeRecord {
            id: "port:8756".to_string(),
            node_type: "port".to_string(),
            name: "8756".to_string(),
            label: "TCP 8756".to_string(),
            properties: serde_json::json!({"protocol": "tcp"}),
            updated_at: None,
        }).expect("upsert port");

        storage.upsert_fact_edge(&FactEdgeRecord::new_explicit(
            "service:agent-steward",
            "port:8756",
            "listens_on",
            serde_json::json!({}),
            1.0,
        )).expect("upsert edge");

        // 插入带端口的活跃进程
        storage.replace_active_processes(&[ActiveProcessRecord {
            id: "proc-999".to_string(),
            pid: 999,
            name: "agent-steward".to_string(),
            command: "/usr/local/bin/agent-steward daemon".to_string(),
            owner: "user:digoal".to_string(),
            associated_path: Some("/root/new/work/agent-steward/src/main.rs".to_string()),
            listening_ports: vec![8756],
            status: "running".to_string(),
            cpu_pct: 0.5,
            mem_bytes: 1024,
            gpu_allocated_mb: 0,
            started_at: None,
        }]).expect("replace procs");

        let graph = topology::build_full_topology(&storage);
        assert!(graph.summary.total_edges >= 3, "应包含 fact_edges + proc->port + proc->script 三条边");
        assert!(graph.summary.port_count >= 1);
        assert!(graph.summary.service_count >= 1);
        assert!(graph.summary.process_count >= 1);

        // 验证多跳爆炸半径子图查询
        let impact = topology::build_impact_topology("service:agent-steward", &storage);
        assert!(impact.summary.total_nodes >= 2);
        assert!(impact.summary.total_edges >= 1);
    }

    #[test]
    fn test_web_v2_panorama_endpoints_and_locks_history() {
        use crate::storage::models::{ChangeRecord, ExternalServiceRecord, SkillCatalogRecord};

        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite memory init");
        storage.init().expect("sqlite init");
        let start_time = Instant::now();

        storage.upsert_skill_catalog(&SkillCatalogRecord {
            id: "skill-1".to_string(),
            skill_name: "pgfaq".to_string(),
            root_source: "/root/new/skills".to_string(),
            skill_path: "/root/new/skills/pgfaq/SKILL.md".to_string(),
            description: "PostgreSQL core internals research".to_string(),
            triggers: vec!["postgres".to_string()],
            est_tokens: 450,
            has_scripts: false,
            has_references: false,
            status: "active".to_string(),
            last_scanned_at: None,
        }).expect("upsert skill");

        storage.upsert_external_service(&ExternalServiceRecord {
            id: "svc-1".to_string(),
            service_name: "ollama-local".to_string(),
            category: "embedding".to_string(),
            endpoint: "http://127.0.0.1:11434".to_string(),
            protocol: "http".to_string(),
            capabilities: "local embeddings".to_string(),
            auth_type: "none".to_string(),
            auth_env_var: None,
            health_endpoint: None,
            usage_example: None,
            status: "healthy".to_string(),
            last_checked_at: None,
        }).expect("upsert svc");

        storage.record_change(&ChangeRecord {
            id: None,
            change_type: "lock_acquired".to_string(),
            target: "port:8080".to_string(),
            description: Some("agent_alpha acquired port 8080".to_string()),
            details: serde_json::json!({}),
            source: "coordination".to_string(),
            created_at: None,
        }).expect("record change");

        let make_req = |path: &str| HttpRequest {
            method: Method::Get,
            path: path.to_string(),
            raw_query: "".to_string(),
            query_params: Default::default(),
            headers: Default::default(),
            body: Vec::new(),
        };

        // 1. /api/v1/facts
        let r_facts = server::WebServer::route_request(&make_req("/api/v1/facts"), &storage, &start_time, &None);
        assert_eq!(r_facts.status, StatusCode::OK);
        let b_facts = std::str::from_utf8(&r_facts.body).unwrap();
        assert!(b_facts.contains("listening_ports"));
        assert!(b_facts.contains("capabilities"));

        // 2. /api/v1/ecosystem
        let r_eco = server::WebServer::route_request(&make_req("/api/v1/ecosystem"), &storage, &start_time, &None);
        assert_eq!(r_eco.status, StatusCode::OK);
        let b_eco = std::str::from_utf8(&r_eco.body).unwrap();
        assert!(b_eco.contains("pgfaq"));
        assert!(b_eco.contains("ollama-local"));

        // 3. /api/v1/ecosystem/route
        let mut req_route = make_req("/api/v1/ecosystem/route");
        req_route.query_params.insert("intent".to_string(), "postgres".to_string());
        req_route.query_params.insert("budget".to_string(), "600".to_string());
        let r_route = server::WebServer::route_request(&req_route, &storage, &start_time, &None);
        assert_eq!(r_route.status, StatusCode::OK);
        let b_route = std::str::from_utf8(&r_route.body).unwrap();
        assert!(b_route.contains("pgfaq"));
        assert!(b_route.contains("route_tokens_used"));

        // 4. /api/v1/governance
        let r_gov = server::WebServer::route_request(&make_req("/api/v1/governance"), &storage, &start_time, &None);
        assert_eq!(r_gov.status, StatusCode::OK);
        let b_gov = std::str::from_utf8(&r_gov.body).unwrap();
        assert!(b_gov.contains("placement_rules"));
        assert!(b_gov.contains("preflight"));

        // 5. /api/v1/mesh/status
        let r_mesh = server::WebServer::route_request(&make_req("/api/v1/mesh/status"), &storage, &start_time, &None);
        assert_eq!(r_mesh.status, StatusCode::OK);
        let b_mesh = std::str::from_utf8(&r_mesh.body).unwrap();
        assert!(b_mesh.contains("local_facts"));

        // 6. /api/v1/locks/history
        let r_hist = server::WebServer::route_request(&make_req("/api/v1/locks/history"), &storage, &start_time, &None);
        assert_eq!(r_hist.status, StatusCode::OK);
        let b_hist = std::str::from_utf8(&r_hist.body).unwrap();
        assert!(b_hist.contains("lock_acquired"));
    }

    #[test]
    fn test_web_parity_matrix_100_percent() {
        let storage = std::sync::Arc::new(SqliteStorageProvider::open_in_memory().expect("sqlite memory init"));
        storage.init().expect("sqlite init");
        let start_time = Instant::now();

        let req = HttpRequest {
            method: Method::Get,
            path: "/api/v1/parity/matrix".to_string(),
            raw_query: "".to_string(),
            query_params: Default::default(),
            headers: Default::default(),
            body: Vec::new(),
        };
        let resp = server::WebServer::route_request_arc(&req, storage, &start_time, &None);
        assert_eq!(resp.status, StatusCode::OK);
        let json: serde_json::Value = serde_json::from_slice(&resp.body).unwrap();
        let data = &json["data"];
        assert_eq!(data["total_mcp_tools"].as_u64(), Some(43));
        assert_eq!(data["synced_count"].as_u64(), Some(43));
        assert_eq!(data["missing_count"].as_u64(), Some(0));
        assert_eq!(data["coverage_pct"].as_f64(), Some(100.0));
    }

    #[test]
    fn test_web_parity_endpoints_roundtrip() {
        let storage = std::sync::Arc::new(SqliteStorageProvider::open_in_memory().expect("sqlite memory init"));
        storage.init().expect("sqlite init");
        let start_time = Instant::now();

        let get_req = |path: &str, params: &[(&str, &str)]| -> HttpRequest {
            let mut qp = std::collections::HashMap::new();
            for (k, v) in params {
                qp.insert((*k).to_string(), (*v).to_string());
            }
            HttpRequest {
                method: Method::Get,
                path: path.to_string(),
                raw_query: "".to_string(),
                query_params: qp,
                headers: Default::default(),
                body: Vec::new(),
            }
        };
        let post_req = |path: &str, body: serde_json::Value| -> HttpRequest {
            HttpRequest {
                method: Method::Post,
                path: path.to_string(),
                raw_query: "".to_string(),
                query_params: Default::default(),
                headers: Default::default(),
                body: serde_json::to_vec(&body).unwrap(),
            }
        };

        // 1. Brief & Placement
        let r_brief = server::WebServer::route_request_arc(&get_req("/api/v1/brief", &[("intent", "postgres")]), storage.clone(), &start_time, &None);
        assert_eq!(r_brief.status, StatusCode::OK);
        assert!(std::str::from_utf8(&r_brief.body).unwrap().contains("quality"));

        let r_place = server::WebServer::route_request_arc(
            &post_req("/api/v1/placement/suggest", serde_json::json!({"object_type": "script", "name": "demo_crawler", "intent": "抓取数据"})),
            storage.clone(),
            &start_time,
            &None,
        );
        assert_eq!(r_place.status, StatusCode::OK);
        assert!(std::str::from_utf8(&r_place.body).unwrap().contains("suggested_path"));

        // 2. Assets register & search
        let r_reg = server::WebServer::route_request_arc(
            &post_req("/api/v1/assets/register", serde_json::json!({"path": "/root/new/shared_proj/demo_tool.py", "asset_type": "script", "description": "demo parity tool"})),
            storage.clone(),
            &start_time,
            &None,
        );
        assert_eq!(r_reg.status, StatusCode::OK);

        let r_asearch = server::WebServer::route_request_arc(&get_req("/api/v1/assets/search", &[("q", "demo_tool")]), storage.clone(), &start_time, &None);
        assert_eq!(r_asearch.status, StatusCode::OK);
        assert!(std::str::from_utf8(&r_asearch.body).unwrap().contains("demo_tool.py"));

        // 3. Facts port-check & query
        let r_port = server::WebServer::route_request_arc(&get_req("/api/v1/facts/port-check", &[("port", "59999")]), storage.clone(), &start_time, &None);
        assert_eq!(r_port.status, StatusCode::OK);
        assert!(std::str::from_utf8(&r_port.body).unwrap().contains("recommended_free_ports"));

        let r_fq = server::WebServer::route_request_arc(&get_req("/api/v1/facts/query", &[("q", "pdf"), ("type", "capability")]), storage.clone(), &start_time, &None);
        assert_eq!(r_fq.status, StatusCode::OK);
        assert!(std::str::from_utf8(&r_fq.body).unwrap().contains("answer"));

        // 4. Links audit, repair & mermaid
        let r_laudit = server::WebServer::route_request_arc(&get_req("/api/v1/links/audit", &[]), storage.clone(), &start_time, &None);
        assert_eq!(r_laudit.status, StatusCode::OK);
        let r_lrep = server::WebServer::route_request_arc(&get_req("/api/v1/links/repair", &[]), storage.clone(), &start_time, &None);
        assert_eq!(r_lrep.status, StatusCode::OK);
        let r_lmer = server::WebServer::route_request_arc(&get_req("/api/v1/links/mermaid", &[]), storage.clone(), &start_time, &None);
        assert_eq!(r_lmer.status, StatusCode::OK);
        assert!(std::str::from_utf8(&r_lmer.body).unwrap().contains("graph LR"));

        // 5. Hooks pre, post & generate
        let r_hpre = server::WebServer::route_request_arc(
            &post_req("/api/v1/hooks/pre", serde_json::json!({"tool_name": "write_file", "args": {"file_path": "/root/new/shared_proj/test.py"}})),
            storage.clone(),
            &start_time,
            &None,
        );
        assert_eq!(r_hpre.status, StatusCode::OK);
        let r_hgen = server::WebServer::route_request_arc(&get_req("/api/v1/hooks/generate", &[("client", "agy")]), storage.clone(), &start_time, &None);
        assert_eq!(r_hgen.status, StatusCode::OK);

        // 6. DuckDB Analytics heatmap & trend
        let r_heat = server::WebServer::route_request_arc(&get_req("/api/v1/analytics/heatmap", &[("days", "30")]), storage.clone(), &start_time, &None);
        assert_eq!(r_heat.status, StatusCode::OK);
        let r_trend = server::WebServer::route_request_arc(&get_req("/api/v1/analytics/trend", &[("days", "30")]), storage.clone(), &start_time, &None);
        assert_eq!(r_trend.status, StatusCode::OK);

        // 7. Memories add, search & audit
        let r_madd = server::WebServer::route_request_arc(
            &post_req("/api/v1/memories/add", serde_json::json!({"memory_type": "decision", "content": "parity web memory test"})),
            storage.clone(),
            &start_time,
            &None,
        );
        assert_eq!(r_madd.status, StatusCode::OK);
        let r_ms = server::WebServer::route_request_arc(&get_req("/api/v1/memories/search", &[("q", "parity"), ("hybrid", "false")]), storage.clone(), &start_time, &None);
        assert_eq!(r_ms.status, StatusCode::OK);
        assert!(std::str::from_utf8(&r_ms.body).unwrap().contains("parity web memory test"));
        let r_ma = server::WebServer::route_request_arc(&get_req("/api/v1/memories/audit", &[]), storage.clone(), &start_time, &None);
        assert_eq!(r_ma.status, StatusCode::OK);

        // 8. GC & Session close dry-run
        let r_gc = server::WebServer::route_request_arc(&get_req("/api/v1/env/gc", &[]), storage.clone(), &start_time, &None);
        assert_eq!(r_gc.status, StatusCode::OK);
        let r_sc = server::WebServer::route_request_arc(&post_req("/api/v1/session/close", serde_json::json!({"session_id": "s-test"})), storage.clone(), &start_time, &None);
        assert_eq!(r_sc.status, StatusCode::OK);

        // 9. Locks acquire & wait
        let r_la = server::WebServer::route_request_arc(
            &post_req("/api/v1/locks/acquire", serde_json::json!({"resource_type": "custom", "resource_id": "demo-parity-res", "holder_agent": "agent-1"})),
            storage.clone(),
            &start_time,
            &None,
        );
        assert_eq!(r_la.status, StatusCode::OK);
        assert!(std::str::from_utf8(&r_la.body).unwrap().contains("\"granted\""));
    }

    #[test]
    fn test_web_write_and_gc_auth_enforcement() {
        let storage = std::sync::Arc::new(SqliteStorageProvider::open_in_memory().expect("sqlite memory init"));
        storage.init().expect("sqlite init");
        let start_time = Instant::now();
        let token = Some("top-secret-token".to_string());

        let make_req = |method: Method, path: &str, bearer: Option<&str>, body: Vec<u8>| -> HttpRequest {
            let mut headers = std::collections::HashMap::new();
            if let Some(t) = bearer {
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
        };

        // 1. 配置 auth_token 时，无 Token 访问各写端点与 GC 必须返回 401
        let protected = [
            (Method::Post, "/api/v1/assets/register", b"{}".to_vec()),
            (Method::Post, "/api/v1/memories/add", br#"{"content":"test memory"}"#.to_vec()),
            (Method::Post, "/api/v1/analytics/export", b"{}".to_vec()),
            (Method::Post, "/api/v1/demo/reset", b"{}".to_vec()),
            (Method::Get, "/api/v1/env/gc", vec![]),
        ];
        for (m, p, b) in &protected {
            let r_unauth = server::WebServer::route_request_arc(
                &make_req(m.clone(), p, None, b.clone()),
                storage.clone(),
                &start_time,
                &token,
            );
            assert_eq!(r_unauth.status, StatusCode::UNAUTHORIZED, "端点 {} {} 无 Token 应返回 401", m.as_str(), p);
        }

        // 2. 携带正确 Token 放行
        let r_ok = server::WebServer::route_request_arc(
            &make_req(Method::Get, "/api/v1/env/gc", Some("top-secret-token"), vec![]),
            storage.clone(),
            &start_time,
            &token,
        );
        assert_eq!(r_ok.status, StatusCode::OK);

        // 3. 非回环暴露且未配置 Token 时，写端点与 GC 必须返回 403 Forbidden
        let r_forbidden = server::WebServer::route_request_arc_with_policy(
            &make_req(Method::Post, "/api/v1/memories/add", None, br#"{"content":"remote write"}"#.to_vec()),
            storage.clone(),
            &start_time,
            &None,
            true,
        );
        assert_eq!(r_forbidden.status, StatusCode::FORBIDDEN);

        // 只读健康检查仍可正常访问
        let r_health = server::WebServer::route_request_arc_with_policy(
            &make_req(Method::Get, "/api/v1/health", None, vec![]),
            storage,
            &start_time,
            &None,
            true,
        );
        assert_eq!(r_health.status, StatusCode::OK);
    }
}
