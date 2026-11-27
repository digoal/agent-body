use std::time::Instant;
use serde::{Deserialize, Serialize};

/// 标准化响应溯源元数据
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResponseProvenance {
    /// 数据客观来源 (如 "reality:proc", "reality:systemd", "reality:socket", "storage:sqlite", "storage:postgres")
    pub source: String,
    /// 宿主机标识
    pub host: String,
    /// 响应生成时间戳 (RFC 3339)
    pub timestamp: String,
    /// 端到端计算与检索耗时 (微秒)
    pub duration_us: u64,
}

/// Agent-Native 强类型标准化统一响应封套 (Envelope)
/// 专为 LLM Agent 设计：消除非结构化文本歧义，提供确定性错误码、Token 预估与物理可证元数据
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BodyResponse<T> {
    /// 操作是否成功
    pub success: bool,
    /// 机器可解析的标准状态码 (如 "OK", "PORT_CONFLICT", "NOISE_FILTERED", "RULE_MATCHED", "ASSET_NOT_FOUND")
    pub code: String,
    /// 面向 Agent 友好的自解释概括文本
    pub message: String,
    /// 强类型具体业务负载 (支持 Option 缺失处理)
    pub data: Option<T>,
    /// 估算的 Token 消耗（便于 Agent 规划上下文窗口与预算控制）
    pub token_estimate: usize,
    /// 客观事实溯源标记
    pub provenance: ResponseProvenance,
}

impl<T: Serialize> BodyResponse<T> {
    /// 构造成功的 Agent 统一响应
    pub fn ok(code: impl Into<String>, message: impl Into<String>, data: T, source: &str, start: Instant) -> Self {
        let msg_str = message.into();
        let json_str = serde_json::to_string(&data).unwrap_or_default();
        let token_estimate = (msg_str.len() + json_str.len()) / 3 + 10;
        let duration_us = start.elapsed().as_micros() as u64;

        Self {
            success: true,
            code: code.into(),
            message: msg_str,
            data: Some(data),
            token_estimate,
            provenance: ResponseProvenance {
                source: source.to_string(),
                host: "localhost".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_us,
            },
        }
    }

    /// 构造失败的 Agent 统一响应
    pub fn err(code: impl Into<String>, message: impl Into<String>, source: &str, start: Instant) -> BodyResponse<()> {
        let msg_str = message.into();
        let token_estimate = msg_str.len() / 3 + 10;
        let duration_us = start.elapsed().as_micros() as u64;

        BodyResponse {
            success: false,
            code: code.into(),
            message: msg_str,
            data: None,
            token_estimate,
            provenance: ResponseProvenance {
                source: source.to_string(),
                host: "localhost".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                duration_us,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    struct DemoData {
        port: u16,
        is_free: bool,
    }

    #[test]
    fn test_body_response_ok_serialization() {
        let start = Instant::now();
        let resp = BodyResponse::ok(
            "OK",
            "端口检测成功，端口 9876 可用",
            DemoData { port: 9876, is_free: true },
            "reality:socket",
            start,
        );

        assert!(resp.success);
        assert_eq!(resp.code, "OK");
        assert!(resp.data.is_some());
        assert_eq!(resp.data.as_ref().unwrap().port, 9876);
        assert_eq!(resp.provenance.source, "reality:socket");
        assert!(resp.token_estimate > 0);

        let json = serde_json::to_string_pretty(&resp).expect("serialize");
        assert!(json.contains("\"success\": true"));
        assert!(json.contains("\"code\": \"OK\""));
    }

    #[test]
    fn test_body_response_err_serialization() {
        let start = Instant::now();
        let err_resp = BodyResponse::<()>::err(
            "PORT_CONFLICT",
            "目标端口 5432 已被 postgres (PID 1234) 占用",
            "reality:proc",
            start,
        );

        assert!(!err_resp.success);
        assert_eq!(err_resp.code, "PORT_CONFLICT");
        assert!(err_resp.data.is_none());
    }
}
