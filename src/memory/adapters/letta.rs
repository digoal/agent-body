//! Letta (MemGPT) 适配器
//!
//! 桥接 Letta（原 MemGPT）的本地或云端 Agent 记忆服务，
//! 实现 `MemoryProvider` SPI。
//!
//! ## 配置示例（config.toml）
//! ```toml
//! [memory]
//! provider = "letta"
//! letta_url = "http://127.0.0.1:8283"   # 本地 letta server 默认端口
//! letta_agent_id = "agent-body-mem"   # 目标 Agent 的 ID
//! letta_token = ""                        # 若启用认证则填入 Bearer token
//! ```
//!
//! ## 参考文档
//! - Letta API: https://docs.letta.com/api-reference/

use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::memory::spi::{
    EvictCriteria, MemoryFilter, MemoryItem, MemoryProvider, SessionContext,
};

// ─── Letta REST 数据结构 ──────────────────────────────────────────────────────

// LettaMessage / LettaSendRequest 保留以备后续"发送消息到 Agent"扩展
#[allow(dead_code)]
#[derive(Debug, Serialize)]
struct LettaMessage {
    role: String,
    content: String,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
struct LettaSendRequest {
    messages: Vec<LettaMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

/// Letta Agent Memory（核心记忆）响应
#[derive(Debug, Deserialize, Clone)]
struct LettaMemoryBlock {
    label: String,
    value: String,
    #[allow(dead_code)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct LettaMemoryResponse {
    memory: HashMap<String, LettaMemoryBlock>,
}

/// Letta Archival Memory 条目（长期归档记忆）
#[derive(Debug, Deserialize, Clone)]
struct LettaArchivalEntry {
    id: String,
    text: String,
    #[serde(default)]
    timestamp: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LettaArchivalListResponse {
    archival_memory: Vec<LettaArchivalEntry>,
}

#[derive(Debug, Serialize)]
struct LettaArchivalInsertRequest {
    content: String,
}

// ─── 适配器主体 ─────────────────────────────────────────────────────────────

/// Letta/MemGPT 记忆适配器
#[derive(Debug)]
pub struct LettaAdapter {
    client: Client,
    base_url: String,
    agent_id: String,
    token: String,
}

impl LettaAdapter {
    /// 创建适配器
    ///
    /// - `base_url`: Letta server 地址，如 "http://127.0.0.1:8283"
    /// - `agent_id`: 目标 Agent 的 ID
    /// - `token`:    Bearer token（无需认证时传空字符串）
    pub fn new(base_url: &str, agent_id: &str, token: &str) -> Result<Self> {
        if agent_id.is_empty() {
            return Err(anyhow!("LettaAdapter: agent_id 不能为空"));
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(20))
            .build()?;
        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            agent_id: agent_id.to_string(),
            token: token.to_string(),
        })
    }

    fn get<T: for<'de> serde::Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.get(&url);
        if !self.token.is_empty() {
            req = req.bearer_auth(&self.token);
        }
        let resp = req.send().map_err(|e| anyhow!("Letta GET {} 失败: {}", url, e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(anyhow!("Letta GET {} 返回 {}: {}", path, status, body));
        }
        resp.json().map_err(|e| anyhow!("Letta 响应解析失败: {}", e))
    }

    fn post<B: Serialize, T: for<'de> serde::Deserialize<'de>>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<T> {
        let url = format!("{}{}", self.base_url, path);
        let mut req = self.client.post(&url);
        if !self.token.is_empty() {
            req = req.bearer_auth(&self.token);
        }
        let resp = req.json(body).send().map_err(|e| anyhow!("Letta POST {} 失败: {}", url, e))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(anyhow!("Letta POST {} 返回 {}: {}", path, status, body));
        }
        resp.json().map_err(|e| anyhow!("Letta 响应解析失败: {}", e))
    }

    /// 将 Archival Entry 转换为 SPI MemoryItem
    fn archival_to_item(entry: LettaArchivalEntry) -> MemoryItem {
        MemoryItem {
            id: entry.id,
            memory_type: "experience".to_string(),
            content: entry.text,
            summary: None,
            tags: vec![],
            source_agent: Some("letta".to_string()),
            source_session: None,
            created_at: entry.timestamp.unwrap_or_default(),
            score: None,
            extra: HashMap::new(),
        }
    }
}

impl MemoryProvider for LettaAdapter {
    fn provider_name(&self) -> &'static str {
        "letta"
    }

    fn search(
        &self,
        query: &str,
        limit: usize,
        _filter: Option<&MemoryFilter>,
    ) -> Result<Vec<MemoryItem>> {
        // Letta 通过 archival memory 搜索接口检索长期记忆
        // GET /v1/agents/{agent_id}/archival?query=...&limit=...
        let path = format!(
            "/v1/agents/{}/archival?query={}&limit={}",
            self.agent_id,
            urlencoding::encode(query),
            limit
        );

        let resp: LettaArchivalListResponse = self.get(&path)?;
        let items = resp
            .archival_memory
            .into_iter()
            .take(limit)
            .map(Self::archival_to_item)
            .collect();
        Ok(items)
    }

    fn store(&self, item: &MemoryItem) -> Result<String> {
        // 写入 Archival Memory（长期持久化记忆）
        let path = format!("/v1/agents/{}/archival", self.agent_id);
        let body = LettaArchivalInsertRequest {
            content: item.content.clone(),
        };

        // Letta 插入返回新创建的条目列表
        let inserted: Vec<LettaArchivalEntry> = self.post(&path, &body)?;
        if let Some(e) = inserted.first() {
            Ok(e.id.clone())
        } else {
            Ok(format!("letta-inserted-{}", item.id))
        }
    }

    fn get_context(&self, session_id: &str) -> Result<SessionContext> {
        // 1. 获取 Agent 的核心记忆块（persona / human 等）
        let path = format!("/v1/agents/{}/memory", self.agent_id);
        let mem_resp: LettaMemoryResponse = self.get(&path)?;

        let core_items: Vec<MemoryItem> = mem_resp
            .memory
            .values()
            .map(|blk| MemoryItem {
                id: format!("letta-core-{}", blk.label),
                memory_type: "fact".to_string(),
                content: blk.value.clone(),
                summary: Some(format!("Letta 核心记忆块 [{}]", blk.label)),
                tags: vec![blk.label.clone()],
                source_agent: Some("letta".to_string()),
                source_session: Some(session_id.to_string()),
                created_at: String::new(),
                score: Some(1.0),
                extra: HashMap::new(),
            })
            .collect();

        // 2. 以 session_id 为查询词检索归档记忆
        let archival_items = self.search(session_id, 5, None).unwrap_or_default();

        Ok(SessionContext {
            session_id: session_id.to_string(),
            key_decisions: core_items,
            active_tasks: vec![],
            relevant_experiences: archival_items,
        })
    }

    fn evict(&self, _criteria: &EvictCriteria) -> Result<usize> {
        // Letta 有自己的记忆压缩与淘汰机制，不主动干预
        Ok(0)
    }
}

// ─── 单元测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_letta_adapter_creation_ok() {
        let adapter = LettaAdapter::new("http://127.0.0.1:8283", "agent-123", "")
            .expect("有效参数应创建成功");
        assert_eq!(adapter.provider_name(), "letta");
        assert_eq!(adapter.agent_id, "agent-123");
    }

    #[test]
    fn test_letta_adapter_empty_agent_id_fails() {
        let result = LettaAdapter::new("http://127.0.0.1:8283", "", "");
        assert!(result.is_err(), "空 agent_id 应报错");
        assert!(result.unwrap_err().to_string().contains("agent_id 不能为空"));
    }

    #[test]
    fn test_archival_to_item_conversion() {
        let entry = LettaArchivalEntry {
            id: "letta-ent-001".to_string(),
            text: "重要决策：选用 SQLite 作为持久化后端".to_string(),
            timestamp: Some("2026-11-17T10:00:00Z".to_string()),
        };
        let item = LettaAdapter::archival_to_item(entry);
        assert_eq!(item.id, "letta-ent-001");
        assert_eq!(item.memory_type, "experience");
        assert_eq!(item.source_agent.as_deref(), Some("letta"));
        assert_eq!(item.created_at, "2026-11-17T10:00:00Z");
    }

    /// 集成测试：需设置 LETTA_INTEGRATION_TEST=1
    #[test]
    fn test_letta_integration_search() {
        if std::env::var("LETTA_INTEGRATION_TEST").is_err() {
            return;
        }
        let agent_id = std::env::var("LETTA_AGENT_ID").expect("需设置 LETTA_AGENT_ID");
        let token = std::env::var("LETTA_TOKEN").unwrap_or_default();
        let adapter =
            LettaAdapter::new("http://127.0.0.1:8283", &agent_id, &token).unwrap();
        let results = adapter.search("agent-body memory", 5, None);
        assert!(results.is_ok(), "search 应返回 Ok: {:?}", results.err());
    }
}
