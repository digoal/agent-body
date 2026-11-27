//! 扩展控制清单 (`.control.toml`) 结构化解析、严格校验与轨 B 进程外 JSON-RPC 2.0 线协议

use agent_body_sdk::{AutonomyLevel, ExtensionKind, ExtensionMeta, SDK_API_VERSION};
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

/// JSON-RPC 2.0 协议版本常量
pub const JSONRPC_VERSION: &str = "2.0";

/// 轨 B 进程外扩展标准 JSON-RPC 方法常量
pub const METHOD_EXT_HANDSHAKE: &str = "ext.handshake";
pub const METHOD_FDW_EXPLAIN_SOURCE: &str = "fdw.explain_source";
pub const METHOD_FDW_SCAN: &str = "fdw.scan";
pub const METHOD_FDW_POLL_DRIFT: &str = "fdw.poll_drift";
pub const METHOD_AUDIT_EMIT: &str = "audit.emit";

/// `<ext>.control.toml` 顶层结构
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionControlManifest {
    pub extension: ExtensionSection,
    pub schema: SchemaSection,
    #[serde(default)]
    pub capabilities: CapabilitiesSection,
    pub sandbox: Option<SandboxSection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionSection {
    pub id: String,
    pub version: String,
    pub kind: ExtensionKind,
    #[serde(default = "default_sdk_version")]
    pub min_sdk_version: String,
    #[serde(default)]
    pub requires: Vec<String>,
    #[serde(default = "default_true")]
    pub relocatable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaSection {
    #[serde(default)]
    pub migration_dir: Option<String>,
    pub table_prefix: String,
    #[serde(default = "default_autonomy")]
    pub default_autonomy: AutonomyLevel,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitiesSection {
    #[serde(default)]
    pub provides_types: Vec<String>,
    #[serde(default)]
    pub provides_fdws: Vec<String>,
    #[serde(default)]
    pub provides_workers: Vec<String>,
    #[serde(default)]
    pub provides_hooks: Vec<String>,
    #[serde(default)]
    pub provides_procs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SandboxSection {
    #[serde(default = "default_transport")]
    pub transport: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub timeout_ms: u64,
    #[serde(default = "default_max_output_bytes")]
    pub max_output_bytes: usize,
    #[serde(default)]
    pub env_whitelist: Vec<String>,
}

fn default_sdk_version() -> String {
    SDK_API_VERSION.to_string()
}

fn default_true() -> bool {
    true
}

fn default_autonomy() -> AutonomyLevel {
    AutonomyLevel::L0Observe
}

fn default_transport() -> String {
    "stdio_jsonrpc".to_string()
}

fn default_max_output_bytes() -> usize {
    256 * 1024
}

impl ExtensionControlManifest {
    /// 从 TOML 文本反序列化并执行严格规范校验
    pub fn from_toml_str(raw: &str) -> anyhow::Result<Self> {
        let manifest: Self =
            toml::from_str(raw).context("解析 .control.toml 扩展控制清单失败")?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// 严格校验扩展控制清单的命名空间、依赖前缀与轨 B 沙盒安全边界
    pub fn validate(&self) -> anyhow::Result<()> {
        let id = self.extension.id.trim();
        if id.is_empty() {
            bail!("extension.id 不能为空");
        }
        let mut chars = id.chars();
        let first_ok = chars
            .next()
            .map(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            .unwrap_or(false);
        let rest_ok = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !first_ok || !rest_ok {
            bail!(
                "extension.id '{id}' 非法：仅允许小写字母、数字与下划线，且须以字母或数字开头"
            );
        }

        if self.extension.version.trim().is_empty() {
            bail!("extension.version 不能为空");
        }

        let expected_prefix = format!("ext_{id}__");
        if self.schema.table_prefix != expected_prefix {
            bail!(
                "schema.table_prefix '{}' 必须严格等于 '{expected_prefix}'",
                self.schema.table_prefix
            );
        }

        if self.extension.kind == ExtensionKind::OutOfProcess {
            self.validate_out_of_process()?;
        }

        Ok(())
    }

    fn validate_out_of_process(&self) -> anyhow::Result<()> {
        if self.schema.default_autonomy != AutonomyLevel::L0Observe {
            bail!(
                "轨 B (out_of_process) 扩展 '{}' 的 default_autonomy 仅允许 L0_OBSERVE，当前为 {:?}",
                self.extension.id,
                self.schema.default_autonomy
            );
        }
        if !self.capabilities.provides_procs.is_empty() {
            bail!(
                "轨 B (out_of_process) 扩展 '{}' 不允许挂载同步 ProcRoutine",
                self.extension.id
            );
        }
        if !self.capabilities.provides_workers.is_empty() {
            bail!(
                "轨 B (out_of_process) 扩展 '{}' 不允许挂载内核 BgWorkerRoutine",
                self.extension.id
            );
        }
        for hook in &self.capabilities.provides_hooks {
            if hook != "emit_audit" {
                bail!(
                    "轨 B (out_of_process) 扩展 '{}' 仅允许异步 emit_audit Hook，禁止挂载同步安全 Hook '{hook}'",
                    self.extension.id
                );
            }
        }
        let sb = self.sandbox.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "轨 B (out_of_process) 扩展 '{}' 必须声明 [sandbox] 节",
                self.extension.id
            )
        })?;
        if sb.timeout_ms == 0 {
            bail!(
                "轨 B (out_of_process) 扩展 '{}' 的 sandbox.timeout_ms 必须大于 0",
                self.extension.id
            );
        }
        if sb.max_output_bytes == 0 {
            bail!(
                "轨 B (out_of_process) 扩展 '{}' 的 sandbox.max_output_bytes 必须大于 0",
                self.extension.id
            );
        }
        if sb.command.trim().is_empty() {
            bail!(
                "轨 B (out_of_process) 扩展 '{}' 的 sandbox.command 不能为空",
                self.extension.id
            );
        }
        if sb.transport != "stdio_jsonrpc" && sb.transport != "unix_socket" {
            bail!(
                "轨 B (out_of_process) 扩展 '{}' 的 sandbox.transport '{}' 不受支持",
                self.extension.id,
                sb.transport
            );
        }
        Ok(())
    }

    /// 转换为 SDK `ExtensionMeta`
    pub fn to_extension_meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: self.extension.id.clone(),
            version: self.extension.version.clone(),
            kind: self.extension.kind,
            min_sdk_version: self.extension.min_sdk_version.clone(),
            table_prefix: self.schema.table_prefix.clone(),
            requires: self.extension.requires.clone(),
        }
    }
}

/// JSON-RPC 2.0 请求/响应 ID
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcId {
    Number(i64),
    String(String),
}

/// JSON-RPC 2.0 请求帧
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: RpcId,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

impl RpcRequest {
    pub fn new(id: RpcId, method: impl Into<String>, params: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            method: method.into(),
            params,
        }
    }
}

/// JSON-RPC 2.0 错误对象
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
}

/// JSON-RPC 2.0 响应帧
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: RpcId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl RpcResponse {
    pub fn ok(id: RpcId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: RpcId, error: RpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// `ext.handshake` 请求参数
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeParams {
    pub kernel_sdk_version: String,
    pub expected_ext_id: String,
}

/// `ext.handshake` 响应载荷
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeResult {
    pub ext_id: String,
    pub version: String,
    pub sdk_version: String,
    pub supported_methods: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_compiled_crate_manifest_passes() {
        let toml_str = r#"
[extension]
id = "probe_core"
version = "0.1.0"
kind = "compiled_crate"
min_sdk_version = "2.0"
requires = []
relocatable = true

[schema]
migration_dir = "sql/"
table_prefix = "ext_probe_core__"
default_autonomy = "L0_OBSERVE"

[capabilities]
provides_types = ["entity:host_socket"]
provides_fdws = ["core_probe"]
"#;
        let parsed = ExtensionControlManifest::from_toml_str(toml_str);
        assert!(parsed.is_ok(), "expected valid manifest: {parsed:?}");
        let manifest = parsed.unwrap();
        assert_eq!(manifest.to_extension_meta().table_prefix, "ext_probe_core__");
    }

    #[test]
    fn table_prefix_mismatch_is_rejected() {
        let toml_str = r#"
[extension]
id = "probe_core"
version = "0.1.0"
kind = "compiled_crate"

[schema]
table_prefix = "ext_wrong__"
"#;
        let err = ExtensionControlManifest::from_toml_str(toml_str).unwrap_err();
        assert!(err.to_string().contains("ext_probe_core__"));
    }

    #[test]
    fn out_of_process_rejects_sync_security_hooks_and_zero_timeout() {
        let bad_hook = r#"
[extension]
id = "ext_bridge"
version = "0.1.0"
kind = "out_of_process"

[schema]
table_prefix = "ext_ext_bridge__"
default_autonomy = "L0_OBSERVE"

[capabilities]
provides_hooks = ["client_auth"]

[sandbox]
command = "/usr/local/bin/bridge"
timeout_ms = 500
"#;
        let err = ExtensionControlManifest::from_toml_str(bad_hook).unwrap_err();
        assert!(err.to_string().contains("emit_audit"));

        let bad_autonomy = r#"
[extension]
id = "ext_bridge"
version = "0.1.0"
kind = "out_of_process"

[schema]
table_prefix = "ext_ext_bridge__"
default_autonomy = "L1_AUTONOMIC"

[capabilities]
provides_fdws = ["ticket_fdw"]

[sandbox]
command = "/usr/local/bin/bridge"
timeout_ms = 500
"#;
        let err2 = ExtensionControlManifest::from_toml_str(bad_autonomy).unwrap_err();
        assert!(err2.to_string().contains("L0_OBSERVE"));

        let zero_timeout = r#"
[extension]
id = "ext_bridge"
version = "0.1.0"
kind = "out_of_process"

[schema]
table_prefix = "ext_ext_bridge__"
default_autonomy = "L0_OBSERVE"

[capabilities]
provides_fdws = ["ticket_fdw"]
provides_hooks = ["emit_audit"]

[sandbox]
command = "/usr/local/bin/bridge"
timeout_ms = 0
"#;
        let err3 = ExtensionControlManifest::from_toml_str(zero_timeout).unwrap_err();
        assert!(err3.to_string().contains("timeout_ms"));
    }

    #[test]
    fn out_of_process_allows_l0_fdw_and_async_emit_audit() {
        let valid_oop = r#"
[extension]
id = "ticket_bridge"
version = "0.1.0"
kind = "out_of_process"

[schema]
table_prefix = "ext_ticket_bridge__"
default_autonomy = "L0_OBSERVE"

[capabilities]
provides_fdws = ["ticket_fdw"]
provides_hooks = ["emit_audit"]

[sandbox]
transport = "stdio_jsonrpc"
command = "/opt/ext/ticket-bridge"
timeout_ms = 1200
max_output_bytes = 131072
"#;
        let parsed = ExtensionControlManifest::from_toml_str(valid_oop);
        assert!(parsed.is_ok(), "expected valid oop manifest: {parsed:?}");
    }

    #[test]
    fn jsonrpc_frames_roundtrip() {
        let req = RpcRequest::new(
            RpcId::Number(1),
            METHOD_FDW_SCAN,
            serde_json::json!({"target": "node-1"}),
        );
        let raw = serde_json::to_string(&req).unwrap();
        let decoded: RpcRequest = serde_json::from_str(&raw).unwrap();
        assert_eq!(decoded.jsonrpc, JSONRPC_VERSION);
        assert_eq!(decoded.method, METHOD_FDW_SCAN);

        let resp = RpcResponse::ok(RpcId::String("abc".to_string()), serde_json::json!({"ok": true}));
        let resp_raw = serde_json::to_string(&resp).unwrap();
        let decoded_resp: RpcResponse = serde_json::from_str(&resp_raw).unwrap();
        assert_eq!(decoded_resp, resp);
        assert_eq!(METHOD_EXT_HANDSHAKE, "ext.handshake");
        assert_eq!(METHOD_FDW_EXPLAIN_SOURCE, "fdw.explain_source");
        assert_eq!(METHOD_FDW_POLL_DRIFT, "fdw.poll_drift");
        assert_eq!(METHOD_AUDIT_EMIT, "audit.emit");
    }
}
