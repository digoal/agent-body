use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use agent_body::config::MeshConfig;
use agent_body::mesh::dialogue::{handle_mesh_dialogue, DialoguePayload};
use agent_body::mesh::protocol::{sha256_bytes_hex, MeshEnvelope};
use agent_body::mesh::transfer::{base64_encode, handle_mesh_transfer, TransferPayload};
use agent_body::storage::Storage;
use agent_body::web::http::{HttpRequest, Method, StatusCode};
use agent_body::web::server::WebServer;

fn make_post_request(path: &str, body_bytes: Vec<u8>) -> HttpRequest {
    HttpRequest {
        method: Method::Post,
        path: path.to_string(),
        raw_query: String::new(),
        query_params: HashMap::new(),
        headers: HashMap::new(),
        body: body_bytes,
    }
}

#[test]
fn test_mesh_non_loopback_write_requires_auth_returns_403() {
    let storage = Storage::open_in_memory().expect("open storage");
    let req = make_post_request(
        "/api/v1/assets/register",
        br#"{"path":"/tmp/x.py","asset_type":"script"}"#.to_vec(),
    );

    // enforce_token_for_writes = true and auth_token = None => 403 Forbidden
    let resp = WebServer::route_request_arc_with_policy(
        &req,
        storage.provider().clone(),
        &Instant::now(),
        &None,
        true,
    );
    assert_eq!(resp.status.0, StatusCode::FORBIDDEN.0);
    let body_str = String::from_utf8_lossy(&resp.body);
    assert!(body_str.contains("non-loopback") || body_str.contains("Forbidden"));

    // With configured auth_token and missing Bearer => 401 Unauthorized
    let token = Some("super-secret-token".to_string());
    let resp_unauth = WebServer::route_request_arc_with_policy(
        &req,
        storage.provider().clone(),
        &Instant::now(),
        &token,
        true,
    );
    assert_eq!(resp_unauth.status.0, StatusCode::UNAUTHORIZED.0);
}

#[test]
fn test_mesh_signed_dialogue_sanitizes_and_persists() {
    let storage = Storage::open_in_memory().expect("open storage");
    let secret = "mesh-shared-secret-integration";
    let mesh_cfg = MeshConfig {
        enabled: true,
        node_id: Some("node-local".to_string()),
        shared_secret: Some(secret.to_string()),
        ..Default::default()
    };

    let payload = serde_json::to_value(DialoguePayload {
        thread_id: "thr-int-01".to_string(),
        intent: "ASK_CONTEXT".to_string(),
        content: "请核对 PG 配置，密钥 sk-abc1234567890def1234567890 不应明文落盘".to_string(),
    })
    .expect("serialize dialogue payload");

    let env = MeshEnvelope::build_signed(
        "dialogue",
        "node-remote",
        "planner",
        "node-local",
        "executor",
        vec!["exchange:dialogue".to_string()],
        payload,
        secret,
    )
    .expect("build signed envelope");

    let req = make_post_request("/api/v1/mesh/dialogue", serde_json::to_vec(&env).expect("to vec"));
    let resp = handle_mesh_dialogue(&req, storage.provider().as_ref(), &mesh_cfg, &None);
    assert_eq!(resp.status.0, StatusCode::OK.0);

    let list = storage.provider().list_mesh_dialogues(10).expect("list dialogues");
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].thread_id, "thr-int-01");
    assert!(list[0].verified_signature);
    assert!(list[0].redaction_hits >= 1);
    assert!(!list[0].content.contains("sk-abc1234567890def"));
}

#[test]
fn test_mesh_sandbox_transfer_writes_isolated_and_blocks_traversal_and_env() {
    let storage = Storage::open_in_memory().expect("open storage");
    let tmp = tempfile::tempdir().expect("tempdir");
    let secret = "mesh-transfer-secret-integration";
    let mesh_cfg = MeshConfig {
        enabled: true,
        node_id: Some("node-local".to_string()),
        shared_secret: Some(secret.to_string()),
        inbox_dir: Some(tmp.path().to_string_lossy().to_string()),
        ..Default::default()
    };

    let content = b"SELECT 1 AS healthy;\n";
    let payload = serde_json::to_value(TransferPayload {
        file_name: "health_check.sql".to_string(),
        sha256: sha256_bytes_hex(content),
        content_base64: base64_encode(content),
    })
    .expect("serialize transfer payload");

    let env = MeshEnvelope::build_signed(
        "transfer",
        "node-peer-a",
        "dba-agent",
        "node-local",
        "receiver",
        vec!["transfer:artifact".to_string()],
        payload,
        secret,
    )
    .expect("build signed transfer");

    let raw_env = serde_json::to_vec(&env).expect("serialize env");
    let req = make_post_request("/api/v1/mesh/transfer", raw_env.clone());
    let resp = handle_mesh_transfer(&req, storage.provider().as_ref(), &mesh_cfg, &None);
    assert_eq!(resp.status.0, StatusCode::OK.0);

    let transfers = storage.provider().list_mesh_transfers(10).expect("list transfers");
    assert_eq!(transfers.len(), 1);
    assert!(transfers[0].inbox_path.starts_with(tmp.path().to_string_lossy().as_ref()));
    assert!(std::path::Path::new(&transfers[0].inbox_path).exists());

    // Replay same nonce must be rejected
    let replay_resp = handle_mesh_transfer(
        &make_post_request("/api/v1/mesh/transfer", raw_env),
        storage.provider().as_ref(),
        &mesh_cfg,
        &None,
    );
    assert_ne!(replay_resp.status.0, StatusCode::OK.0);

    // Path traversal blocked with 403
    let bad_payload = serde_json::to_value(TransferPayload {
        file_name: "../../etc/passwd".to_string(),
        sha256: sha256_bytes_hex(content),
        content_base64: base64_encode(content),
    })
    .expect("bad payload");
    let bad_env = MeshEnvelope::build_signed(
        "transfer",
        "node-peer-a",
        "dba-agent",
        "node-local",
        "receiver",
        vec!["transfer:artifact".to_string()],
        bad_payload,
        secret,
    )
    .expect("bad signed");
    let bad_resp = handle_mesh_transfer(
        &make_post_request("/api/v1/mesh/transfer", serde_json::to_vec(&bad_env).expect("vec")),
        storage.provider().as_ref(),
        &mesh_cfg,
        &None,
    );
    assert_eq!(bad_resp.status.0, StatusCode::FORBIDDEN.0);

    // Sensitive .env blocked with 403
    let env_payload = serde_json::to_value(TransferPayload {
        file_name: ".env.production".to_string(),
        sha256: sha256_bytes_hex(content),
        content_base64: base64_encode(content),
    })
    .expect("env payload");
    let sec_env = MeshEnvelope::build_signed(
        "transfer",
        "node-peer-a",
        "dba-agent",
        "node-local",
        "receiver",
        vec!["transfer:artifact".to_string()],
        env_payload,
        secret,
    )
    .expect("sec signed");
    let sec_resp = handle_mesh_transfer(
        &make_post_request("/api/v1/mesh/transfer", serde_json::to_vec(&sec_env).expect("vec")),
        storage.provider().as_ref(),
        &mesh_cfg,
        &None,
    );
    assert_eq!(sec_resp.status.0, StatusCode::FORBIDDEN.0);
    let _arc_guard: Arc<Storage> = Arc::new(storage);
}

#[test]
fn test_mesh_invalid_signature_returns_401_and_peer_scope_enforced() {
    use agent_body::mesh::check_peer_read_scope_with_cfg;

    let storage = Storage::open_in_memory().expect("open storage");
    let mesh_cfg = MeshConfig {
        enabled: true,
        node_id: Some("node-local".to_string()),
        shared_secret: Some("real-secret".to_string()),
        peer_scopes: HashMap::from([(
            "node-limited".to_string(),
            vec!["read:locks".to_string()],
        )]),
        ..Default::default()
    };

    // Wrong signature must be rejected with 401
    let bad_sig_env = MeshEnvelope::build_signed(
        "dialogue",
        "node-remote",
        "planner",
        "node-local",
        "executor",
        vec!["exchange:dialogue".to_string()],
        serde_json::json!({
            "thread_id": "t-bad-sig",
            "intent": "ASK_CONTEXT",
            "content": "hello"
        }),
        "wrong-secret",
    )
    .expect("build bad sig");
    let resp = handle_mesh_dialogue(
        &make_post_request("/api/v1/mesh/dialogue", serde_json::to_vec(&bad_sig_env).expect("vec")),
        storage.provider().as_ref(),
        &mesh_cfg,
        &None,
    );
    assert_eq!(resp.status.0, StatusCode::UNAUTHORIZED.0);

    // Peer without read:facts scope must receive 403
    let mut headers = HashMap::new();
    headers.insert("x-body-node".to_string(), "node-limited".to_string());
    let scope_req = HttpRequest {
        method: Method::Get,
        path: "/api/v1/mesh/graph".to_string(),
        raw_query: String::new(),
        query_params: HashMap::new(),
        headers,
        body: Vec::new(),
    };
    let denied = check_peer_read_scope_with_cfg(&scope_req, "read:facts", &mesh_cfg);
    assert!(denied.is_some());
    assert_eq!(denied.expect("403").status.0, StatusCode::FORBIDDEN.0);
}

