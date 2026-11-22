//! Mesh 受控沙盒文件投递引擎 (Phase 41.4 / 规约 #2, #19, #24)
//!
//! 安全红线：
//! - 体积上限严格 `<= mesh_cfg.max_transfer_bytes`（默认 10MB），超限返回 `413 Payload Too Large`；
//! - 校验文件名合法性（禁止路径穿越 `..`/`/`/`\`），阻断 `.env*`/`*.pem`/`*.key`/`id_rsa*` 敏感文件 (`403 Forbidden`)；
//! - 校验解码后字节与 `sha256` 摘要一致；
//! - 所有接收文件一律隔离落盘至 `~/.agent-body/mesh_inbox/<from_node>/<transfer_id>_<safe_name>` (`0600`)，绝不跨机直写宿主机路径。

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

use crate::config::MeshConfig;
use crate::mesh::auth::{constant_time_eq, global_nonce_cache, verify_envelope};
use crate::mesh::protocol::{sha256_bytes_hex, MeshAck, MeshEnvelope, MESH_PROTOCOL_VERSION};
use crate::scanner::metadata::SecretRedactor;
use crate::storage::models::MeshTransferRecord;
use crate::storage::StorageProvider;
use crate::web::api::WebResponse;
use crate::web::http::{HttpRequest, HttpResponse, StatusCode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferPayload {
    pub file_name: String,
    pub sha256: String,
    pub content_base64: String,
}

/// 解析合法收件箱根目录（默认 `~/.agent-body/mesh_inbox`）
pub fn resolve_inbox_dir(mesh_cfg: &MeshConfig) -> PathBuf {
    if let Some(ref custom) = mesh_cfg.inbox_dir {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    PathBuf::from(home).join(".agent-body").join("mesh_inbox")
}

/// 校验并净化投递文件名，阻断路径穿越与敏感文件类型
pub fn validate_safe_file_name(raw_name: &str) -> Result<String, &'static str> {
    let trimmed = raw_name.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || trimmed.starts_with('.') && (trimmed == "." || trimmed == "..")
    {
        return Err("Invalid or unsafe file_name (path traversal blocked)");
    }
    let lower = trimmed.to_lowercase();
    if lower.starts_with(".env")
        || lower.ends_with(".pem")
        || lower.ends_with(".key")
        || lower.ends_with(".p12")
        || lower.ends_with(".pfx")
        || lower.contains("id_rsa")
        || lower.contains("id_ed25519")
        || lower == "authorized_keys"
        || lower.contains("credentials")
    {
        return Err("Sensitive credential/key files are forbidden over Mesh transfer");
    }
    let sanitized: String = trimmed
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if sanitized.is_empty() {
        return Err("Empty file_name after sanitization");
    }
    Ok(sanitized)
}

fn sanitize_node_segment(node: &str) -> String {
    let s: String = node
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_') { c } else { '_' })
        .collect();
    if s.is_empty() { "unknown".to_string() } else { s }
}

/// POST /api/v1/mesh/transfer
pub fn handle_mesh_transfer(
    req: &HttpRequest,
    storage: &dyn StorageProvider,
    mesh_cfg: &MeshConfig,
    web_auth_token: &Option<String>,
) -> HttpResponse {
    let env: MeshEnvelope = match req.json() {
        Ok(e) => e,
        Err(e) => {
            return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid MeshEnvelope JSON: {}", e));
        }
    };

    if let Err(auth_err) = verify_envelope(
        &env,
        "transfer:artifact",
        mesh_cfg,
        web_auth_token,
        global_nonce_cache(),
    ) {
        return WebResponse::error(auth_err.status, &auth_err.message);
    }

    let tp: TransferPayload = match serde_json::from_value(env.payload.clone()) {
        Ok(p) => p,
        Err(e) => {
            return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Invalid TransferPayload: {}", e));
        }
    };

    let safe_name = match validate_safe_file_name(&tp.file_name) {
        Ok(n) => n,
        Err(msg) => return WebResponse::error(StatusCode::FORBIDDEN, msg),
    };

    let bytes = match base64_decode(&tp.content_base64) {
        Ok(b) => b,
        Err(e) => return WebResponse::error(StatusCode::BAD_REQUEST, &format!("Base64 decode failed: {}", e)),
    };

    if bytes.len() > mesh_cfg.max_transfer_bytes {
        return WebResponse::error(
            StatusCode(413, "Payload Too Large"),
            &format!(
                "Transfer size {} exceeds max_transfer_bytes {}",
                bytes.len(),
                mesh_cfg.max_transfer_bytes
            ),
        );
    }

    let actual_sha = sha256_bytes_hex(&bytes);
    if !constant_time_eq(&actual_sha, &tp.sha256) {
        return WebResponse::error(
            StatusCode::BAD_REQUEST,
            "SHA-256 mismatch between payload and decoded bytes",
        );
    }

    // 若为 UTF-8 文本，扫描潜在凭据泄露告警数
    let secret_warnings = match std::str::from_utf8(&bytes) {
        Ok(txt) => {
            let redacted = SecretRedactor::redact_preserving_newlines(txt);
            if redacted != txt { 1 } else { 0 }
        }
        Err(_) => 0,
    };

    let transfer_id = format!("tx-{}", uuid::Uuid::new_v4());
    let inbox_root = resolve_inbox_dir(mesh_cfg);
    let node_dir = inbox_root.join(sanitize_node_segment(&env.from_node));
    if let Err(e) = fs::create_dir_all(&node_dir) {
        return WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to create mesh_inbox dir: {}", e),
        );
    }
    crate::config::restrict_private_permissions(&inbox_root, true);
    crate::config::restrict_private_permissions(&node_dir, true);

    let dest_file = node_dir.join(format!("{}_{}", transfer_id, safe_name));
    if let Err(e) = fs::write(&dest_file, &bytes) {
        return WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to write quarantined file: {}", e),
        );
    }
    crate::config::restrict_private_permissions(&dest_file, false);

    let now = chrono::Utc::now().to_rfc3339();
    let rec = MeshTransferRecord {
        transfer_id: transfer_id.clone(),
        from_node: env.from_node.clone(),
        from_agent: env.from_agent.clone(),
        to_node: env.to_node.clone(),
        file_name: safe_name.clone(),
        size_bytes: bytes.len() as u64,
        sha256: actual_sha.clone(),
        inbox_path: dest_file.to_string_lossy().to_string(),
        secret_warnings,
        verified_signature: true,
        status: "quarantined".to_string(),
        accepted_path: None,
        created_at: now.clone(),
        updated_at: now,
    };
    if let Err(e) = storage.insert_mesh_transfer(&rec) {
        return WebResponse::error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("Failed to persist mesh transfer: {}", e),
        );
    }

    let responder_node = mesh_cfg
        .node_id
        .clone()
        .or_else(sysinfo::System::host_name)
        .unwrap_or_else(|| "local-node".to_string());

    WebResponse::success(MeshAck {
        protocol: MESH_PROTOCOL_VERSION.to_string(),
        message_id: env.message_id,
        accepted: true,
        status: "quarantined".to_string(),
        details: serde_json::json!({
            "transfer_id": transfer_id,
            "file_name": safe_name,
            "size_bytes": bytes.len(),
            "sha256": actual_sha,
            "inbox_path": rec.inbox_path,
            "secret_warnings": secret_warnings,
        }),
        responder_node,
        timestamp_ms: chrono::Utc::now().timestamp_millis(),
    })
}

/// 将沙盒中隔离的投递文件显式放行至受控目标目录
pub fn accept_transfer_to_dir(
    storage: &dyn StorageProvider,
    transfer_id: &str,
    dest_dir: &Path,
) -> anyhow::Result<MeshTransferRecord> {
    let transfers = storage.list_mesh_transfers(200)?;
    let rec = transfers
        .into_iter()
        .find(|t| t.transfer_id == transfer_id)
        .ok_or_else(|| anyhow::anyhow!("未找到 transfer_id={}", transfer_id))?;

    if rec.status != "quarantined" {
        anyhow::bail!("传输状态 `{}` 不可再次放行", rec.status);
    }
    let src_path = Path::new(&rec.inbox_path);
    if !src_path.is_file() {
        anyhow::bail!("沙盒文件不存在: {}", rec.inbox_path);
    }
    let safe_name = validate_safe_file_name(&rec.file_name).map_err(|e| anyhow::anyhow!(e))?;
    fs::create_dir_all(dest_dir)?;
    let target = dest_dir.join(&safe_name);
    fs::copy(src_path, &target)?;

    let target_str = target.to_string_lossy().to_string();
    storage.update_mesh_transfer_status(transfer_id, "accepted", Some(&target_str))?;

    let mut updated = rec;
    updated.status = "accepted".to_string();
    updated.accepted_path = Some(target_str);
    Ok(updated)
}

/// 拒绝并物理清除沙盒中的隔离文件
pub fn reject_transfer(storage: &dyn StorageProvider, transfer_id: &str) -> anyhow::Result<bool> {
    let transfers = storage.list_mesh_transfers(200)?;
    if let Some(rec) = transfers.into_iter().find(|t| t.transfer_id == transfer_id) {
        let p = Path::new(&rec.inbox_path);
        if p.is_file() {
            let _ = fs::remove_file(p);
        }
    }
    storage.update_mesh_transfer_status(transfer_id, "rejected", None)
}

// ─────────────────────────── 零依赖 Base64 编解码 ───────────────────────────

const B64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(input: &[u8]) -> String {
    let mut out = String::with_capacity(((input.len() + 2) / 3) * 4);
    for chunk in input.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(B64_TABLE[((triple >> 18) & 0x3F) as usize] as char);
        out.push(B64_TABLE[((triple >> 12) & 0x3F) as usize] as char);
        out.push(if chunk.len() > 1 { B64_TABLE[((triple >> 6) & 0x3F) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64_TABLE[(triple & 0x3F) as usize] as char } else { '=' });
    }
    out
}

pub fn base64_decode(input: &str) -> anyhow::Result<Vec<u8>> {
    let cleaned: Vec<u8> = input
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    if cleaned.len() % 4 != 0 {
        anyhow::bail!("Invalid base64 length");
    }
    let mut out = Vec::with_capacity((cleaned.len() / 4) * 3);
    for chunk in cleaned.chunks(4) {
        let mut vals = [0u32; 4];
        let mut pad = 0usize;
        for (i, &b) in chunk.iter().enumerate() {
            if b == b'=' {
                vals[i] = 0;
                pad += 1;
            } else if let Some(idx) = B64_TABLE.iter().position(|&c| c == b) {
                vals[i] = idx as u32;
            } else {
                anyhow::bail!("Invalid base64 character");
            }
        }
        let triple = (vals[0] << 18) | (vals[1] << 12) | (vals[2] << 6) | vals[3];
        out.push(((triple >> 16) & 0xFF) as u8);
        if pad < 2 {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if pad < 1 {
            out.push((triple & 0xFF) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_and_safe_filename_validation() {
        let raw = b"hello mesh sandbox \x00\xff";
        let enc = base64_encode(raw);
        assert_eq!(base64_decode(&enc).expect("dec"), raw);

        assert!(validate_safe_file_name("report_v1.md").is_ok());
        assert!(validate_safe_file_name("../etc/passwd").is_err());
        assert!(validate_safe_file_name(".env.prod").is_err());
        assert!(validate_safe_file_name("server.key").is_err());
        assert!(validate_safe_file_name("id_rsa").is_err());
    }
}
