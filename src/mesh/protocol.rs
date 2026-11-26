//! Mesh Wire Protocol v1.0 线协议模型 (Phase 41.2 / 规约 #24)

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub const MESH_PROTOCOL_VERSION: &str = "steward.mesh/1.0";

/// 跨主机 Mesh 协同标准信封
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshEnvelope {
    pub protocol: String,
    pub message_id: String,
    pub msg_type: String, // dialogue | transfer | lock_acquire | graph_query
    pub from_node: String,
    pub from_agent: String,
    pub to_node: String,
    #[serde(default)]
    pub to_agent: String,
    pub timestamp_ms: i64,
    pub nonce: String,
    #[serde(default = "default_ttl_secs")]
    pub ttl_secs: u64,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub payload_sha256: String,
    pub payload: Value,
    pub signature: String,
}

fn default_ttl_secs() -> u64 {
    300
}

/// 跨主机 Mesh 协同标准回执
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshAck {
    pub protocol: String,
    pub message_id: String,
    pub accepted: bool,
    pub status: String,
    pub details: Value,
    pub responder_node: String,
    pub timestamp_ms: i64,
}

impl MeshEnvelope {
    /// 构造并签名一个新的 MeshEnvelope
    #[allow(clippy::too_many_arguments)]
    pub fn build_signed(
        msg_type: &str,
        from_node: &str,
        from_agent: &str,
        to_node: &str,
        to_agent: &str,
        scopes: Vec<String>,
        payload: Value,
        secret: &str,
    ) -> anyhow::Result<Self> {
        let message_id = format!("msg-{}", uuid::Uuid::new_v4());
        let timestamp_ms = chrono::Utc::now().timestamp_millis();
        let nonce = uuid::Uuid::new_v4().to_string();
        let payload_sha256 = sha256_json_hex(&payload)?;

        let mut env = Self {
            protocol: MESH_PROTOCOL_VERSION.to_string(),
            message_id,
            msg_type: msg_type.to_string(),
            from_node: from_node.to_string(),
            from_agent: from_agent.to_string(),
            to_node: to_node.to_string(),
            to_agent: to_agent.to_string(),
            timestamp_ms,
            nonce,
            ttl_secs: default_ttl_secs(),
            scopes,
            payload_sha256,
            payload,
            signature: String::new(),
        };
        let canonical = env.canonical_signing_string();
        env.signature = crate::mesh::auth::hmac_sha256_hex(secret, canonical.as_bytes());
        Ok(env)
    }

    /// 规范签名原文（固定字段顺序 `\n` 拼接）
    pub fn canonical_signing_string(&self) -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
            self.protocol,
            self.message_id,
            self.msg_type,
            self.from_node,
            self.to_node,
            self.timestamp_ms,
            self.nonce,
            self.payload_sha256
        )
    }
}

/// 计算 JSON 载荷的规范 SHA-256 十六进制摘要
pub fn sha256_json_hex(v: &Value) -> anyhow::Result<String> {
    let bytes = serde_json::to_vec(v)?;
    Ok(sha256_bytes_hex(&bytes))
}

/// 计算任意字节切片的 SHA-256 十六进制摘要
pub fn sha256_bytes_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    hex_encode(&digest)
}

pub fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write;
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}
