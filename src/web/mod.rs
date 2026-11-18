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
}
