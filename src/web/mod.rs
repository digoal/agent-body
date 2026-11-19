pub mod http;
pub mod api;
pub mod topology;
pub mod locks_api;
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

        storage.upsert_fact_edge(&FactEdgeRecord {
            id: None,
            source_id: "service:agent-steward".to_string(),
            target_id: "port:8756".to_string(),
            relation: "listens_on".to_string(),
            properties: serde_json::json!({}),
            confidence: 1.0,
            updated_at: None,
        }).expect("upsert edge");

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
}
