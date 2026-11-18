//! Mem0 适配器
//!
//! 桥接 Mem0 记忆服务（https://api.mem0.ai 或私有化部署端点），
//! 实现 `MemoryProvider` SPI。
//!
//! ## 配置示例（steward.toml）
//! ```toml
//! [memory]
//! provider = "mem0"
//! mem0_url = "https://api.mem0.ai"       # 或私有化端点
//! mem0_api_key = ""                       # 若私有化部署可留空
//! mem0_user_id = "agent-steward"          # Mem0 用户标识
//! ```
//!
//! ## 参考文档
//! - Mem0 REST API: https://docs.mem0.ai/api-reference/

use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::memory::spi::{
    EvictCriteria, MemoryFilter, MemoryItem, MemoryProvider, SessionContext,
};

// ─── Mem0 REST 数据结构 ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct Mem0SearchRequest<'a> {
    query: &'a str,
    user_id: &'a str,
    limit: usize,
}

#[derive(Debug, Deserialize)]
struct Mem0SearchResponse {
    results: Vec<Mem0Memory>,
}

#[derive(Debug, Deserialize, Clone)]
struct Mem0Memory {
    id: String,
    memory: String,
    #[serde(default)]
    score: Option<f64>,
    #[serde(default)]
    categories: Vec<String>,
    #[serde(default)]
    metadata: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct Mem0AddRequest<'a> {
    messages: Vec<Mem0Message<'a>>,
    user_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
struct Mem0Message<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Debug, Deserialize)]
struct Mem0AddResponse {
    results: Vec<Mem0AddResult>,
}

#[derive(Debug, Deserialize)]
struct Mem0AddResult {
    id: String,
    #[allow(dead_code)]
    event: String,
}

// ─── 适配器主体 ─────────────────────────────────────────────────────────────

/// Mem0 记忆适配器
pub struct Mem0Adapter {
    client: Client,
    base_url: String,
    api_key: String,
    user_id: String,
}

impl Mem0Adapter {
    /// 创建适配器
    ///
    /// - `base_url`: 如 "https://api.mem0.ai" 或私有化端点
    /// - `api_key`: Mem0 API Key（私有化部署可传空字符串）
    /// - `user_id`: 在 Mem0 中标识当前使用者的 ID（如 "agent-steward"）
    pub fn new(base_url: &str, api_key: &str, user_id: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .build()?;

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            user_id: user_id.to_string(),
        })
    }

    fn auth_header(&self) -> String {
        if self.api_key.is_empty() {
            String::new()
        } else {
            format!("Token {}", self.api_key)
        }
    }

    fn mem0_to_item(m: Mem0Memory) -> MemoryItem {
        let mut extra = HashMap::new();
        if let Some(meta) = m.metadata.as_object() {
            for (k, v) in meta {
                if let Some(s) = v.as_str() {
                    extra.insert(k.clone(), s.to_string());
                }
            }
        }

        // 尝试从 categories 推断 memory_type
        let memory_type = m
            .categories
            .first()
            .map(|c| c.to_lowercase())
            .unwrap_or_else(|| "note".to_string());

        MemoryItem {
            id: m.id,
            memory_type,
            content: m.memory,
            summary: None,
            tags: m.categories,
            source_agent: Some("mem0".to_string()),
            source_session: None,
            created_at: String::new(),
            score: m.score,
            extra,
        }
    }

    fn build_request<B: Serialize>(&self, method: reqwest::Method, path: &str, body: &B) -> Result<reqwest::blocking::Response> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.request(method, &url);
        if !self.api_key.is_empty() {
            req = req.header("Authorization", self.auth_header());
        }
        req.json(body)
            .send()
            .map_err(|e| anyhow!("Mem0 请求 {} 失败: {}", url, e))
    }
}

impl MemoryProvider for Mem0Adapter {
    fn provider_name(&self) -> &'static str {
        "mem0"
    }

    fn search(
        &self,
        query: &str,
        limit: usize,
        _filter: Option<&MemoryFilter>,
    ) -> Result<Vec<MemoryItem>> {
        let req_body = Mem0SearchRequest {
            query,
            user_id: &self.user_id,
            limit,
        };

        let resp = self.build_request(reqwest::Method::POST, "/v1/memories/search/", &req_body)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(anyhow!("Mem0 search 返回 {}: {}", status, body));
        }

        let mem0_resp: Mem0SearchResponse = resp
            .json()
            .map_err(|e| anyhow!("Mem0 search 响应解析失败: {}", e))?;

        let items = mem0_resp
            .results
            .into_iter()
            .take(limit)
            .map(Self::mem0_to_item)
            .collect();
        Ok(items)
    }

    fn store(&self, item: &MemoryItem) -> Result<String> {
        // Mem0 以 messages 格式写入，把记忆内容包装为 user 消息
        let mut metadata_map = serde_json::Map::new();
        metadata_map.insert("memory_type".to_string(), serde_json::Value::String(item.memory_type.clone()));
        if let Some(ref agent) = item.source_agent {
            metadata_map.insert("source_agent".to_string(), serde_json::Value::String(agent.clone()));
        }
        for (k, v) in &item.extra {
            metadata_map.insert(k.clone(), serde_json::Value::String(v.clone()));
        }

        let req_body = Mem0AddRequest {
            messages: vec![Mem0Message {
                role: "user",
                content: &item.content,
            }],
            user_id: &self.user_id,
            metadata: Some(serde_json::Value::Object(metadata_map)),
        };

        let resp = self.build_request(reqwest::Method::POST, "/v1/memories/", &req_body)?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(anyhow!("Mem0 store 返回 {}: {}", status, body));
        }

        let add_resp: Mem0AddResponse = resp
            .json()
            .map_err(|e| anyhow!("Mem0 store 响应解析失败: {}", e))?;

        // 返回第一个创建/更新的记忆 ID
        if let Some(r) = add_resp.results.first() {
            Ok(r.id.clone())
        } else {
            Ok(format!("mem0-noop-{}", item.id))
        }
    }

    fn get_context(&self, session_id: &str) -> Result<SessionContext> {
        // 以 session_id 为查询词检索相关记忆
        let items = self.search(session_id, 10, None)?;
        let (decisions, experiences): (Vec<_>, Vec<_>) =
            items.iter().cloned().partition(|m| m.memory_type == "decision");

        Ok(SessionContext {
            session_id: session_id.to_string(),
            key_decisions: decisions,
            active_tasks: vec![],
            relevant_experiences: experiences,
        })
    }

    fn evict(&self, _criteria: &EvictCriteria) -> Result<usize> {
        // Mem0 云服务有自己的记忆管理策略，不主动干预
        Ok(0)
    }
}

// ─── 单元测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mem0_adapter_creation() {
        let adapter = Mem0Adapter::new("https://api.mem0.ai", "test-key", "agent-steward")
            .expect("适配器创建应成功");
        assert_eq!(adapter.provider_name(), "mem0");
        assert_eq!(adapter.user_id, "agent-steward");
    }

    #[test]
    fn test_mem0_to_item_conversion() {
        let m = Mem0Memory {
            id: "mem0-abc".to_string(),
            memory: "选择 Rust 编写高性能守护进程".to_string(),
            score: Some(0.88),
            categories: vec!["decision".to_string()],
            metadata: serde_json::json!({"source": "agent-steward"}),
        };
        let item = Mem0Adapter::mem0_to_item(m);
        assert_eq!(item.id, "mem0-abc");
        assert_eq!(item.memory_type, "decision");
        assert_eq!(item.score, Some(0.88));
        assert_eq!(item.tags, vec!["decision".to_string()]);
        assert_eq!(item.source_agent.as_deref(), Some("mem0"));
    }

    #[test]
    fn test_mem0_auth_header() {
        let adapter_with_key = Mem0Adapter::new("https://api.mem0.ai", "sk-test", "u1").unwrap();
        assert_eq!(adapter_with_key.auth_header(), "Token sk-test");

        let adapter_no_key = Mem0Adapter::new("https://api.mem0.ai", "", "u1").unwrap();
        assert_eq!(adapter_no_key.auth_header(), "");
    }

    /// 集成测试：需设置 MEM0_INTEGRATION_TEST=1 且 MEM0_API_KEY 有效
    #[test]
    fn test_mem0_integration_search() {
        if std::env::var("MEM0_INTEGRATION_TEST").is_err() {
            return;
        }
        let api_key = std::env::var("MEM0_API_KEY").unwrap_or_default();
        let adapter =
            Mem0Adapter::new("https://api.mem0.ai", &api_key, "agent-steward-test").unwrap();
        let results = adapter.search("agent steward rust", 3, None);
        // 结果可能为空列表，但接口调用本身不应报错
        assert!(results.is_ok(), "search 应返回 Ok: {:?}", results.err());
    }
}
