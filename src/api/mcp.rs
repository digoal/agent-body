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
                        "name": "agent-body",
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
        assert_eq!(v["result"]["serverInfo"]["name"], "agent-body");
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
        assert_eq!(tools.len(), 43, "必须注册全部 43 个原生 MCP 工具");
        assert!(tools.iter().any(|t| t["name"] == "body_state"));
        assert!(tools.iter().any(|t| t["name"] == "body_audit_query"));
        assert!(tools.iter().any(|t| t["name"] == "body_feedback"));
        assert!(tools.iter().any(|t| t["name"] == "body_benchmark"));
        assert!(tools.iter().any(|t| t["name"] == "body_demo_scenario"));
        assert!(tools.iter().any(|t| t["name"] == "body_mesh_query"));
        assert!(tools.iter().any(|t| t["name"] == "body_lock_wait"));
        assert!(tools.iter().any(|t| t["name"] == "body_lock_deadlock_check"));
        assert!(tools.iter().any(|t| t["name"] == "body_mcp_catalog"));
        assert!(tools.iter().any(|t| t["name"] == "body_skill_route"));
        assert!(tools.iter().any(|t| t["name"] == "body_service_contact"));
        assert!(tools.iter().any(|t| t["name"] == "body_brief"));
        assert!(tools.iter().any(|t| t["name"] == "body_placement_suggest"));
        assert!(tools.iter().any(|t| t["name"] == "body_asset_search"));
        assert!(tools.iter().any(|t| t["name"] == "body_links_repair"));
        assert!(tools.iter().any(|t| t["name"] == "body_placement_learn"));
        assert!(tools.iter().any(|t| t["name"] == "body_env_gc"));
        assert!(tools.iter().any(|t| t["name"] == "body_memory_audit"));
        assert!(tools.iter().any(|t| t["name"] == "body_fact_query"));
        assert!(tools.iter().any(|t| t["name"] == "body_hook_pre"));
        assert!(tools.iter().any(|t| t["name"] == "body_hook_post"));
        assert!(tools.iter().any(|t| t["name"] == "body_analytics_heatmap"));
        assert!(tools.iter().any(|t| t["name"] == "body_analytics_trend"));
        assert!(tools.iter().any(|t| t["name"] == "body_analytics_export"));
        assert!(tools.iter().any(|t| t["name"] == "body_lock_acquire"));
        assert!(tools.iter().any(|t| t["name"] == "body_lock_release"));
        assert!(tools.iter().any(|t| t["name"] == "body_lock_list"));
        assert!(tools.iter().any(|t| t["name"] == "body_hook_generate"));
    }

    #[test]
    fn test_mcp_tools_call_fact_query() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let server = McpServer::new(storage);

        let req = r#"{
            "jsonrpc": "2.0",
            "id": 105,
            "method": "tools/call",
            "params": {
                "name": "body_fact_query",
                "arguments": {
                    "query": "这台机器上有没有能处理 PDF 的东西？",
                    "fact_type": "capability"
                }
            }
        }"#;
        let resp = server.handle_message(req).expect("handle_message");
        let v: Value = serde_json::from_str(&resp).expect("parse response");
        assert_eq!(v["id"], 105);
        let content_text = v["result"]["content"][0]["text"].as_str().expect("text");
        assert!(content_text.contains("pdftotext"));
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
                "name": "body_placement_suggest",
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
    fn test_mcp_tools_call_links_audit() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let server = McpServer::new(storage);

        let req = r#"{
            "jsonrpc": "2.0",
            "id": 101,
            "method": "tools/call",
            "params": {
                "name": "body_links_audit",
                "arguments": {
                    "fix_dangling": false
                }
            }
        }"#;

        let resp = server.handle_message(req).expect("handle_message");
        let v: Value = serde_json::from_str(&resp).expect("parse response");
        assert_eq!(v["id"], 101);
        assert_eq!(v["result"]["isError"], false);
        let text = v["result"]["content"][0]["text"].as_str().expect("content text");
        assert!(text.contains("health_score"));
    }

    #[test]
    fn test_mcp_tools_call_codegraph_bridge() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let server = McpServer::new(storage);

        let req = r#"{
            "jsonrpc": "2.0",
            "id": 102,
            "method": "tools/call",
            "params": {
                "name": "body_codegraph_bridge",
                "arguments": {
                    "path": "/root/new/src/hk2"
                }
            }
        }"#;

        let resp = server.handle_message(req).expect("handle_message");
        let v: Value = serde_json::from_str(&resp).expect("parse response");
        assert_eq!(v["id"], 102);
        assert_eq!(v["result"]["isError"], false);
        let text = v["result"]["content"][0]["text"].as_str().expect("content text");
        assert!(text.contains("has_codegraph"));
    }

    #[test]
    fn test_mcp_notification_no_response() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let server = McpServer::new(storage);

        let notification = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
        let resp = server.handle_message(notification);
        assert!(resp.is_none(), "Notification 无需返回 response");
    }

    #[test]
    fn test_mcp_tools_call_phase7_autonomy() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let server = McpServer::new(storage);

        // 测试调用 body_memory_audit
        let req = r#"{
            "jsonrpc": "2.0",
            "id": 103,
            "method": "tools/call",
            "params": {
                "name": "body_memory_audit",
                "arguments": {}
            }
        }"#;

        let resp = server.handle_message(req).expect("handle_message");
        let v: Value = serde_json::from_str(&resp).expect("parse response");
        assert_eq!(v["id"], 103);
        assert_eq!(v["result"]["isError"], false);
        let text = v["result"]["content"][0]["text"].as_str().expect("content text");
        assert!(text.contains("conflict_count"));

        // 测试调用 body_links_repair
        let req_repair = r#"{
            "jsonrpc": "2.0",
            "id": 104,
            "method": "tools/call",
            "params": {
                "name": "body_links_repair",
                "arguments": { "apply": false }
            }
        }"#;
        let resp_repair = server.handle_message(req_repair).expect("handle_message");
        let v_rep: Value = serde_json::from_str(&resp_repair).expect("parse response");
        assert_eq!(v_rep["id"], 104);
        assert_eq!(v_rep["result"]["isError"], false);
    }

    #[test]
    fn test_mcp_tools_call_brief_dynamic() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let server = McpServer::new(storage);

        let req = r#"{
            "jsonrpc": "2.0",
            "id": 105,
            "method": "tools/call",
            "params": {
                "name": "body_brief",
                "arguments": {
                    "query": "rust 守护进程",
                    "session_id": "sess-mcp-test",
                    "budget_tokens": 1200
                }
            }
        }"#;

        let resp = server.handle_message(req).expect("handle_message");
        let v: Value = serde_json::from_str(&resp).expect("parse response");
        assert_eq!(v["id"], 105);
        assert_eq!(v["result"]["isError"], false);
        let text = v["result"]["content"][0]["text"].as_str().expect("content text");
        assert!(text.contains("Agent 本体开工简报"), "输出应含简报标题");
        assert!(text.contains("宿主机事实"), "输出应含事实模块");
        assert!(text.contains("工程偏好"), "输出应含偏好模块");
        assert!(text.contains("质量分"), "输出应含质量评分");
    }

    #[test]
    fn test_mcp_tools_call_agent_hooks() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let server = McpServer::new(storage);

        // 1. 调用 body_hook_pre
        let req_pre = r#"{
            "jsonrpc": "2.0",
            "id": 106,
            "method": "tools/call",
            "params": {
                "name": "body_hook_pre",
                "arguments": {
                    "tool_name": "write_file",
                    "args": {
                        "path": "/root/new/work/test_agent.rs"
                    }
                }
            }
        }"#;

        let resp_pre = server.handle_message(req_pre).expect("handle_message");
        let v_pre: Value = serde_json::from_str(&resp_pre).expect("parse response");
        assert_eq!(v_pre["id"], 106);
        assert_eq!(v_pre["result"]["isError"], false);
        let text_pre = v_pre["result"]["content"][0]["text"].as_str().expect("text");
        assert!(text_pre.contains("allowed"));

        // 2. 调用 body_hook_post
        let req_post = r#"{
            "jsonrpc": "2.0",
            "id": 107,
            "method": "tools/call",
            "params": {
                "name": "body_hook_post",
                "arguments": {
                    "tool_name": "write_file",
                    "args": {
                        "path": "/root/new/work/test_agent.rs"
                    },
                    "success": true
                }
            }
        }"#;

        let resp_post = server.handle_message(req_post).expect("handle_message");
        let v_post: Value = serde_json::from_str(&resp_post).expect("parse response");
        assert_eq!(v_post["id"], 107);
        assert_eq!(v_post["result"]["isError"], false);

        // 3. 调用 body_hook_generate
        let req_gen = r#"{
            "jsonrpc": "2.0",
            "id": 108,
            "method": "tools/call",
            "params": {
                "name": "body_hook_generate",
                "arguments": {
                    "client": "agy",
                    "dry_run": true
                }
            }
        }"#;

        let resp_gen = server.handle_message(req_gen).expect("handle_message");
        let v_gen: Value = serde_json::from_str(&resp_gen).expect("parse response");
        assert_eq!(v_gen["id"], 108);
        assert_eq!(v_gen["result"]["isError"], false);
        let text_gen = v_gen["result"]["content"][0]["text"].as_str().expect("text");
        assert!(text_gen.contains(".gemini/config/hooks.json"));
        assert!(text_gen.contains("body_bridge.py"));
    }

    #[test]
    fn test_mcp_tools_call_coordination() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let server = McpServer::new(storage);

        // 1. 获取锁
        let req_acquire = r#"{
            "jsonrpc": "2.0",
            "id": 201,
            "method": "tools/call",
            "params": {
                "name": "body_lock_acquire",
                "arguments": {
                    "resource_type": "port",
                    "resource_id": "8080",
                    "scope": "exclusive",
                    "holder_agent": "test-agent-mcp",
                    "ttl_seconds": 60
                }
            }
        }"#;

        let resp_acquire = server.handle_message(req_acquire).expect("handle_message");
        let v_acq: Value = serde_json::from_str(&resp_acquire).expect("parse response");
        assert_eq!(v_acq["id"], 201);
        let text_content = v_acq["result"]["content"][0]["text"].as_str().expect("text");
        let acq_res: Value = serde_json::from_str(text_content).expect("parse inner");
        assert_eq!(acq_res["status"], "granted");
        let lock_id = acq_res["lock_id"].as_str().expect("lock_id");

        // 2. 列出锁
        let req_list = r#"{
            "jsonrpc": "2.0",
            "id": 202,
            "method": "tools/call",
            "params": {
                "name": "body_lock_list",
                "arguments": {
                    "resource_type": "port"
                }
            }
        }"#;
        let resp_list = server.handle_message(req_list).expect("handle_message");
        let v_list: Value = serde_json::from_str(&resp_list).expect("parse response");
        assert_eq!(v_list["id"], 202);
        let list_text = v_list["result"]["content"][0]["text"].as_str().expect("text");
        let list_arr: Value = serde_json::from_str(list_text).expect("parse inner");
        assert_eq!(list_arr.as_array().unwrap().len(), 1);

        // 3. 释放锁
        let req_release = format!(r#"{{
            "jsonrpc": "2.0",
            "id": 203,
            "method": "tools/call",
            "params": {{
                "name": "body_lock_release",
                "arguments": {{
                    "lock_id": "{}",
                    "holder_agent": "test-agent-mcp"
                }}
            }}
        }}"#, lock_id);
        let resp_release = server.handle_message(&req_release).expect("handle_message");
        let v_rel: Value = serde_json::from_str(&resp_release).expect("parse response");
        assert_eq!(v_rel["id"], 203);
    }
}
