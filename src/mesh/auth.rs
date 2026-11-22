//! Mesh Wire Protocol v1.0 零信任认证内核 (Phase 41.2 / 规约 #24)
//!
//! - 标准 RFC 2104 HMAC-SHA256 规范签名与常量时间比对；
//! - ±replay_window_secs 时间窗校验 + 线程安全 NonceCache 防重放；
//! - 基于 `[mesh].peer_scopes` 的最小权限作用域校验。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use sha2::{Digest, Sha256};

use crate::config::MeshConfig;
use crate::mesh::protocol::{hex_encode, sha256_json_hex, MeshEnvelope, MESH_PROTOCOL_VERSION};
use crate::web::http::StatusCode;

#[derive(Debug, Clone)]
pub struct MeshAuthError {
    pub status: StatusCode,
    pub message: String,
}

impl MeshAuthError {
    fn new(status: StatusCode, msg: impl Into<String>) -> Self {
        Self {
            status,
            message: msg.into(),
        }
    }
}

/// 标准 RFC 2104 HMAC-SHA256 十六进制输出
pub fn hmac_sha256_hex(key: &str, message: &[u8]) -> String {
    const BLOCK_SIZE: usize = 64;
    let key_bytes = key.as_bytes();
    let mut key_block = [0u8; BLOCK_SIZE];
    if key_bytes.len() > BLOCK_SIZE {
        let mut hasher = Sha256::new();
        hasher.update(key_bytes);
        let hashed = hasher.finalize();
        key_block[..hashed.len()].copy_from_slice(&hashed);
    } else {
        key_block[..key_bytes.len()].copy_from_slice(key_bytes);
    }

    let mut o_key_pad = [0x5cu8; BLOCK_SIZE];
    let mut i_key_pad = [0x36u8; BLOCK_SIZE];
    for i in 0..BLOCK_SIZE {
        o_key_pad[i] ^= key_block[i];
        i_key_pad[i] ^= key_block[i];
    }

    let mut inner = Sha256::new();
    inner.update(i_key_pad);
    inner.update(message);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(o_key_pad);
    outer.update(inner_hash);
    hex_encode(&outer.finalize())
}

/// 常量时间字节串比较（防计时侧信道）
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    if ab.len() != bb.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in ab.iter().zip(bb.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// 进程级并发 Nonce 防重放缓存: (from_node, nonce) -> expires_at_ms
pub struct NonceCache {
    seen: Mutex<HashMap<(String, String), i64>>,
}

impl NonceCache {
    pub fn new() -> Self {
        Self {
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// 校验并登记 nonce；若在有效期内重复出现返回 false
    pub fn check_and_record(&self, from_node: &str, nonce: &str, now_ms: i64, window_ms: i64) -> bool {
        let Ok(mut map) = self.seen.lock() else {
            return false;
        };
        // 清理过期条目，防止无界增长
        if map.len() > 2048 {
            map.retain(|_, exp| *exp > now_ms);
        }
        let key = (from_node.to_string(), nonce.to_string());
        if let Some(&exp) = map.get(&key) {
            if exp > now_ms {
                return false;
            }
        }
        map.insert(key, now_ms + window_ms);
        true
    }
}

impl Default for NonceCache {
    fn default() -> Self {
        Self::new()
    }
}

static GLOBAL_NONCE_CACHE: OnceLock<NonceCache> = OnceLock::new();

pub fn global_nonce_cache() -> &'static NonceCache {
    GLOBAL_NONCE_CACHE.get_or_init(NonceCache::new)
}

/// 解析对等节点验证密钥：优先 `[mesh.peer_keys][from_node]` -> `[mesh].shared_secret` -> `[web].auth_token`
pub fn resolve_peer_secret(
    from_node: &str,
    mesh_cfg: &MeshConfig,
    web_auth_token: &Option<String>,
) -> Option<String> {
    if let Some(k) = mesh_cfg.peer_keys.get(from_node) {
        if !k.trim().is_empty() {
            return Some(k.clone());
        }
    }
    if let Some(ref s) = mesh_cfg.shared_secret {
        if !s.trim().is_empty() {
            return Some(s.clone());
        }
    }
    web_auth_token
        .as_ref()
        .filter(|s| !s.trim().is_empty())
        .cloned()
}

/// 校验 MeshEnvelope：协议版本 -> 密钥存在性 -> 时间窗 -> Payload SHA-256 -> HMAC-SHA256 -> Nonce 防重放 -> Scope
pub fn verify_envelope(
    env: &MeshEnvelope,
    required_scope: &str,
    mesh_cfg: &MeshConfig,
    web_auth_token: &Option<String>,
    nonce_cache: &NonceCache,
) -> Result<(), MeshAuthError> {
    if env.protocol != MESH_PROTOCOL_VERSION {
        return Err(MeshAuthError::new(
            StatusCode::BAD_REQUEST,
            format!("Unsupported mesh protocol: {}", env.protocol),
        ));
    }
    if env.from_node.trim().is_empty() || env.nonce.trim().is_empty() {
        return Err(MeshAuthError::new(
            StatusCode::BAD_REQUEST,
            "Envelope missing from_node or nonce",
        ));
    }

    let secret = resolve_peer_secret(&env.from_node, mesh_cfg, web_auth_token).ok_or_else(|| {
        MeshAuthError::new(
            StatusCode::FORBIDDEN,
            "Forbidden: no shared_secret / peer_key / auth_token configured on receiver",
        )
    })?;

    // 1. 校验时间窗
    let now_ms = chrono::Utc::now().timestamp_millis();
    let window_ms = (mesh_cfg.replay_window_secs.max(10) as i64) * 1000;
    if (now_ms - env.timestamp_ms).abs() > window_ms {
        return Err(MeshAuthError::new(
            StatusCode::UNAUTHORIZED,
            "Unauthorized: envelope timestamp outside replay window",
        ));
    }

    // 2. 校验 payload_sha256
    let actual_sha = sha256_json_hex(&env.payload).map_err(|e| {
        MeshAuthError::new(StatusCode::BAD_REQUEST, format!("Payload hash error: {}", e))
    })?;
    if !constant_time_eq(&actual_sha, &env.payload_sha256) {
        return Err(MeshAuthError::new(
            StatusCode::UNAUTHORIZED,
            "Unauthorized: payload_sha256 mismatch",
        ));
    }

    // 3. 校验 HMAC-SHA256 签名
    let expected_sig = hmac_sha256_hex(&secret, env.canonical_signing_string().as_bytes());
    if !constant_time_eq(&expected_sig, &env.signature) {
        return Err(MeshAuthError::new(
            StatusCode::UNAUTHORIZED,
            "Unauthorized: invalid HMAC-SHA256 envelope signature",
        ));
    }

    // 4. 校验 Scope（如果节点配置了 peer_scopes，则必须在白名单内）
    if let Some(allowed) = mesh_cfg.peer_scopes.get(&env.from_node) {
        if !allowed.iter().any(|s| s == required_scope || s == "*") {
            return Err(MeshAuthError::new(
                StatusCode::FORBIDDEN,
                format!("Forbidden: peer `{}` lacks required scope `{}`", env.from_node, required_scope),
            ));
        }
    }
    if !env.scopes.is_empty() && !env.scopes.iter().any(|s| s == required_scope || s == "*") {
        return Err(MeshAuthError::new(
            StatusCode::FORBIDDEN,
            format!("Forbidden: envelope scopes do not include `{}`", required_scope),
        ));
    }

    // 5. 防重放 Nonce 检查（放在签名验真之后，防止未授权噪声污染缓存）
    if !nonce_cache.check_and_record(&env.from_node, &env.nonce, now_ms, window_ms) {
        return Err(MeshAuthError::new(
            StatusCode::CONFLICT,
            "Conflict: duplicate nonce detected (replay attack blocked)",
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_sign_verify_replay_and_scope() {
        let mut cfg = MeshConfig::default();
        cfg.peer_scopes.insert("node-a".into(), vec!["exchange:dialogue".into()]);
        let secret = Some("mesh-test-secret".to_string());
        let cache = NonceCache::new();

        let env = MeshEnvelope::build_signed(
            "dialogue",
            "node-a",
            "agent-1",
            "node-b",
            "agent-2",
            vec!["exchange:dialogue".into()],
            serde_json::json!({"intent": "ASK_CONTEXT", "content": "hello"}),
            "mesh-test-secret",
        )
        .expect("build signed");

        // 1. 合法签名第一次验证应通过
        assert!(verify_envelope(&env, "exchange:dialogue", &cfg, &secret, &cache).is_ok());

        // 2. 重放同一信封 -> 409 Conflict
        let replay = verify_envelope(&env, "exchange:dialogue", &cfg, &secret, &cache).unwrap_err();
        assert_eq!(replay.status, StatusCode::CONFLICT);

        // 3. 篡改载荷 -> 401 Unauthorized
        let mut tampered = MeshEnvelope::build_signed(
            "dialogue",
            "node-a",
            "agent-1",
            "node-b",
            "agent-2",
            vec!["exchange:dialogue".into()],
            serde_json::json!({"intent": "ASK_CONTEXT", "content": "hello"}),
            "mesh-test-secret",
        )
        .expect("build");
        tampered.payload = serde_json::json!({"intent": "ASK_CONTEXT", "content": "hacked"});
        let bad = verify_envelope(&tampered, "exchange:dialogue", &cfg, &secret, &cache).unwrap_err();
        assert_eq!(bad.status, StatusCode::UNAUTHORIZED);

        // 4. 越权 Scope -> 403 Forbidden
        let env2 = MeshEnvelope::build_signed(
            "transfer",
            "node-a",
            "agent-1",
            "node-b",
            "agent-2",
            vec!["transfer:artifact".into()],
            serde_json::json!({"file": "a.txt"}),
            "mesh-test-secret",
        )
        .expect("build");
        let deny_scope = verify_envelope(&env2, "transfer:artifact", &cfg, &secret, &cache).unwrap_err();
        assert_eq!(deny_scope.status, StatusCode::FORBIDDEN);
    }
}
