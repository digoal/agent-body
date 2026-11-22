//! PowerContext 适配器
//!
//! 桥接本地 PowerContext 服务（默认 http://127.0.0.1:8000）的记忆能力，
//! 实现 `MemoryProvider` SPI，支持混合检索（FTS + 向量）与记忆写入。
//!
//! ## 配置示例（steward.toml）
//! ```toml
//! [memory]
//! provider = "powercontext"
//! powercontext_url = "http://127.0.0.1:8000"
//! powercontext_scope_id = "scp_xxx"   # 留空则自动从 /v1/scopes/default 获取
//! ```
//!
//! ## 降级策略
//! 若 PowerContext 服务不可达，自动回退到内置 SQLite 引擎（优雅降级）。

use anyhow::{anyhow, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

use crate::memory::spi::{
    EvictCriteria, MemoryFilter, MemoryItem, MemoryProvider, SessionContext,
};

// ─── PowerContext REST 数据结构 ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct PcSearchRequest<'a> {
    scope_id: &'a str,
    query: &'a str,
    limit: usize,
}

#[derive(Debug, Deserialize)]
struct PcSearchResponse {
    hits: Vec<PcSearchHit>,
}

#[derive(Debug, Deserialize)]
struct PcSearchHit {
    memory: PcMemoryRecord,
    score: f64,
    #[allow(dead_code)]
    matched_by: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct PcMemoryRecord {
    artifact: PcArtifactRef,
    kind: String,
    text: String,
    state: String,
}

#[derive(Debug, Deserialize)]
struct PcArtifactRef {
    artifact_id: String,
}

#[derive(Debug, Serialize)]
struct PcRememberRequest<'a> {
    scope_id: &'a str,
    kind: &'a str,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
}

#[derive(Debug, Deserialize)]
struct PcRememberResponse {
    #[allow(dead_code)]
    changed: bool,
    records: Vec<PcMemoryRecord>,
}

#[derive(Debug, Deserialize)]
struct PcScopeDescriptor {
    scope_id: String,
}

// ─── 适配器主体 ─────────────────────────────────────────────────────────────

/// PowerContext 记忆适配器
#[derive(Debug)]
pub struct PowerContextAdapter {
    client: Client,
    base_url: String,
    /// 解析后的默认 scope_id
    scope_id: String,
}

impl PowerContextAdapter {
    /// 创建适配器并自动解析 default scope
    ///
    /// - `base_url`: 如 "http://127.0.0.1:8000"
    /// - `scope_id`: 预设 scope；传空字符串则自动从 `/v1/scopes/default` 获取
    pub fn new(base_url: &str, scope_id: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;

        let resolved_scope = if scope_id.is_empty() {
            // 自动发现 default scope
            let url = format!("{}/v1/scopes/default", base_url);
            let resp = client
                .get(&url)
                .send()
                .map_err(|e| anyhow!("PowerContext 不可达 ({}): {}", url, e))?;
            if !resp.status().is_success() {
                return Err(anyhow!(
                    "PowerContext /v1/scopes/default 返回 {}",
                    resp.status()
                ));
            }
            let desc: PcScopeDescriptor = resp.json()?;
            desc.scope_id
        } else {
            scope_id.to_string()
        };

        Ok(Self {
            client,
            base_url: base_url.trim_end_matches('/').to_string(),
            scope_id: resolved_scope,
        })
    }

    /// 将 PowerContext 搜索命中结果转换为 SPI 通用 MemoryItem
    fn hit_to_item(hit: PcSearchHit) -> MemoryItem {
        let mut extra = HashMap::new();
        extra.insert("pc_state".to_string(), hit.memory.state.clone());

        MemoryItem {
            id: hit.memory.artifact.artifact_id.clone(),
            memory_type: hit.memory.kind.clone(),
            content: hit.memory.text.clone(),
            summary: None,
            tags: vec![],
            source_agent: Some("powercontext".to_string()),
            source_session: None,
            created_at: String::new(), // PowerContext 返回体无 created_at 字段
            score: Some(hit.score),
            extra,
        }
    }
}

impl MemoryProvider for PowerContextAdapter {
    fn provider_name(&self) -> &'static str {
        "powercontext"
    }

    fn search(
        &self,
        query: &str,
        limit: usize,
        _filter: Option<&MemoryFilter>,
    ) -> Result<Vec<MemoryItem>> {
        let url = format!("{}/v1/memory/search", self.base_url);
        let req_body = PcSearchRequest {
            scope_id: &self.scope_id,
            query,
            limit: limit.min(50), // PowerContext 上限 50
        };

        let resp = self
            .client
            .post(&url)
            .json(&req_body)
            .send()
            .map_err(|e| anyhow!("PowerContext search 请求失败: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(anyhow!("PowerContext search 返回 {}: {}", status, body));
        }

        let pc_resp: PcSearchResponse = resp.json()?;
        let items = pc_resp.hits.into_iter().map(Self::hit_to_item).collect();
        Ok(items)
    }

    fn store(&self, item: &MemoryItem) -> Result<String> {
        let url = format!("{}/v1/memory/remember", self.base_url);
        // description 最多 200 字符，单行（基于字符切片，防止多字节 UTF-8 越界 Panic）
        let desc_raw = item.summary.as_deref().unwrap_or(item.content.as_str());
        let desc_single: String = desc_raw
            .chars()
            .filter(|c| *c != '\n' && *c != '\r')
            .take(197)
            .collect();

        let req_body = PcRememberRequest {
            scope_id: &self.scope_id,
            kind: &item.memory_type,
            text: &item.content,
            description: if desc_single.is_empty() {
                None
            } else {
                Some(&desc_single)
            },
        };

        let resp = self
            .client
            .post(&url)
            .json(&req_body)
            .send()
            .map_err(|e| anyhow!("PowerContext remember 请求失败: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().unwrap_or_default();
            return Err(anyhow!("PowerContext remember 返回 {}: {}", status, body));
        }

        let pc_resp: PcRememberResponse = resp.json()?;
        // 返回第一条记录的 artifact_id（若 changed=false 表示已存在但仍成功）
        if let Some(rec) = pc_resp.records.first() {
            Ok(rec.artifact.artifact_id.clone())
        } else {
            // PowerContext 返回 changed=false 且 records 为空时 —— 幂等成功
            Ok(format!("pc-noop-{}", item.id))
        }
    }

    fn get_context(&self, session_id: &str) -> Result<SessionContext> {
        // PowerContext 没有直接的 session 概念，用 session_id 作为搜索词召回相关记忆
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
        // PowerContext 有自己的记忆生命周期管理（dream/lifecycle）
        // 大管家层面暂不主动干预，直接返回 0（本引擎不管淘汰）
        Ok(0)
    }
}

// ─── 单元测试 ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试 PowerContext 适配器对 scope 解析失败的错误处理
    #[test]
    fn test_powercontext_unreachable_returns_error() {
        // 使用一个必然不可达的端口
        let result = PowerContextAdapter::new("http://127.0.0.1:19999", "");
        assert!(result.is_err(), "不可达的端口应返回 Err");
        let msg = result.err().unwrap().to_string();
        assert!(
            msg.contains("PowerContext 不可达") || msg.contains("Connection refused") || msg.contains("os error"),
            "错误信息应包含不可达提示，实际: {}",
            msg
        );
    }

    /// 测试 hit_to_item 转换正确性
    #[test]
    fn test_hit_to_item_conversion() {
        let hit = PcSearchHit {
            memory: PcMemoryRecord {
                artifact: PcArtifactRef {
                    artifact_id: "art_abc123".to_string(),
                },
                kind: "decision".to_string(),
                text: "选用 Rust 编写核心守护进程".to_string(),
                state: "active".to_string(),
            },
            score: 0.95,
            matched_by: vec!["text".to_string(), "vector".to_string()],
        };

        let item = PowerContextAdapter::hit_to_item(hit);
        assert_eq!(item.id, "art_abc123");
        assert_eq!(item.memory_type, "decision");
        assert_eq!(item.content, "选用 Rust 编写核心守护进程");
        assert_eq!(item.score, Some(0.95));
        assert_eq!(item.source_agent.as_deref(), Some("powercontext"));
        assert_eq!(item.extra.get("pc_state").map(|s| s.as_str()), Some("active"));
    }

    #[test]
    fn test_powercontext_utf8_chinese_truncation_no_panic() {
        // 构造多字节中文超长文本，确保按字符截断不因字节索引落入中文字符内部而 Panic
        let long_cn = "中文记忆内容测试".repeat(40);
        let item = MemoryItem {
            id: "mem-cn-1".to_string(),
            memory_type: "experience".to_string(),
            content: long_cn.clone(),
            summary: None,
            tags: vec![],
            source_agent: None,
            source_session: None,
            created_at: "2026-11-22T00:00:00Z".to_string(),
            score: None,
            extra: Default::default(),
        };
        let desc_raw = item.summary.as_deref().unwrap_or(item.content.as_str());
        let desc_single: String = desc_raw
            .chars()
            .filter(|c| *c != '\n' && *c != '\r')
            .take(197)
            .collect();
        assert_eq!(desc_single.chars().count(), 197);
    }

    /// 集成测试：若宿主机 8000 端口活跃，对真实服务发起一次搜索
    /// 本测试在 CI 环境中被忽略（需要 POWERCONTEXT_INTEGRATION_TEST=1）
    #[test]
    fn test_powercontext_integration_search() {
        if std::env::var("POWERCONTEXT_INTEGRATION_TEST").is_err() {
            return; // 非集成测试环境跳过
        }
        let adapter = PowerContextAdapter::new("http://127.0.0.1:8000", "")
            .expect("PowerContext 应可达");
        assert_eq!(adapter.provider_name(), "powercontext");

        let results = adapter.search("rust agent steward", 5, None)
            .expect("search 应成功");
        // 结果可能为空，但不应报错
        for item in &results {
            assert!(!item.id.is_empty());
            assert!(!item.content.is_empty());
        }
    }
}
