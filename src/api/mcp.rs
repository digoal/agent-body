use serde::{Deserialize, Serialize};
use serde_json::Value;
use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use crate::storage::Storage;
use crate::api::mcp_tools::McpToolRegistry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

pub struct McpServer {
    #[allow(dead_code)]
    storage: Storage,
    tools: McpToolRegistry,
}

impl McpServer {
    pub fn new(storage: Storage) -> Self {
        let tools = McpToolRegistry::new(storage.clone());
        Self { storage, tools }
    }

    /// 处理单行 JSON-RPC 消息
    pub fn handle_message(&self, raw: &str) -> Option<String> {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return None;
        }

        let req: JsonRpcRequest = match serde_json::from_str(trimmed) {
            Ok(r) => r,
            Err(e) => {
                let err_resp = JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {}", e),
                        data: None,
                    }),
                };
                return serde_json::to_string(&err_resp).ok();
            }
        };

        // 如果没有 id，则是通知 (Notification)，无需响应
        let id = req.id.clone()?;

        let resp = match req.method.as_str() {
            "initialize" => {
                let result = serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {
                            "listChanged": false
                        }
                    },
                    "serverInfo": {
                        "name": "agent-steward",
                        "version": env!("CARGO_PKG_VERSION")
                    }
                });
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(id),
                    result: Some(result),
                    error: None,
                }
            }

            "notifications/initialized" => {
                return None;
            }

            "ping" => {
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(id),
                    result: Some(serde_json::json!({})),
                    error: None,
                }
            }

            "tools/list" => {
                let list = self.tools.list_tools();
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(id),
                    result: Some(serde_json::json!({ "tools": list })),
                    error: None,
                }
            }

            "tools/call" => {
                let params = req.params.unwrap_or(Value::Null);
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));

                match self.tools.call_tool(tool_name, args) {
                    Ok(text) => {
                        let result = serde_json::json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": text
                                }
                            ],
                            "isError": false
                        });
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: Some(id),
                            result: Some(result),
                            error: None,
                        }
                    }
                    Err(e) => {
                        let result = serde_json::json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": format!("Tool execution error: {}", e)
                                }
                            ],
                            "isError": true
                        });
                        JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: Some(id),
                            result: Some(result),
                            error: None,
                        }
                    }
                }
            }

            _ => {
                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id: Some(id),
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32601,
                        message: format!("Method not found: {}", req.method),
                        data: None,
                    }),
                }
            }
        };

        serde_json::to_string(&resp).ok()
    }

    /// 启动基于 stdio 的 MCP 异步消息循环
    pub async fn run_stdio(&self) -> Result<()> {
        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        while let Some(line) = lines.next_line().await? {
            if let Some(resp_str) = self.handle_message(&line) {
                stdout.write_all(resp_str.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_initialize() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let server = McpServer::new(storage);

        let req = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let resp = server.handle_message(req).expect("handle_message");
        let v: Value = serde_json::from_str(&resp).expect("parse response");

        assert_eq!(v["id"], 1);
        assert_eq!(v["result"]["serverInfo"]["name"], "agent-steward");
    }

    #[test]
    fn test_mcp_tools_list() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let server = McpServer::new(storage);

        let req = r#"{"jsonrpc":"2.0","id":"req-2","method":"tools/list"}"#;
        let resp = server.handle_message(req).expect("handle_message");
        let v: Value = serde_json::from_str(&resp).expect("parse response");

        assert_eq!(v["id"], "req-2");
        let tools = v["result"]["tools"].as_array().expect("tools array");
        assert!(tools.len() >= 8, "应至少注册 8 个核心工具");
        assert!(tools.iter().any(|t| t["name"] == "steward_brief"));
        assert!(tools.iter().any(|t| t["name"] == "steward_placement_suggest"));
        assert!(tools.iter().any(|t| t["name"] == "steward_asset_search"));
    }

    #[test]
    fn test_mcp_tools_call_placement() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let server = McpServer::new(storage);

        let req = r#"{
            "jsonrpc": "2.0",
            "id": 100,
            "method": "tools/call",
            "params": {
                "name": "steward_placement_suggest",
                "arguments": {
                    "object_type": "script",
                    "name": "sec_crawler",
                    "intent": "美股抓取"
                }
            }
        }"#;

        let resp = server.handle_message(req).expect("handle_message");
        let v: Value = serde_json::from_str(&resp).expect("parse response");

        assert_eq!(v["id"], 100);
        assert_eq!(v["result"]["isError"], false);
        let text = v["result"]["content"][0]["text"].as_str().expect("content text");
        assert!(text.contains("推荐目标路径"));
        assert!(text.contains("shared_proj/sec_crawler"));
    }

    #[test]
    fn test_mcp_notification_no_response() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let server = McpServer::new(storage);

        let notification = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let resp = server.handle_message(notification);
        assert!(resp.is_none(), "Notification 无需返回 response");
    }
}
