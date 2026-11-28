//! 旁路快权重 Model Runtime 客户端实现（HTTP JSON-RPC 2.0 + 确定性 Mock Runtime）
//!
//! 严守零推理/训练依赖进入 Rust 本体原则：仅通过 JSON-RPC 2.0 协议与外部计算服务交互。

use std::collections::HashSet;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use agent_body_sdk::{
    FastWeightCompactRequest, FastWeightCompactResponse, FastWeightRollbackRequest,
    FastWeightRollbackResponse, FastWeightRuntimeClient, FastWeightRuntimeHealth,
    FastWeightUnlearnRequest, FastWeightUnlearnResponse,
};
use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use uuid::Uuid;

/// CI 与沙盒故障注入器：用于精确复现范数雪崩、通用能力衰减或遗忘泄漏门禁拦截
#[derive(Debug, Clone, Default)]
pub struct MockRuntimeFault {
    pub norm_ratio: Option<f32>,
    pub eval_glue_delta: Option<f32>,
    pub unlearn_leak_rate: Option<f32>,
    pub rpc_error: Option<String>,
}

#[derive(Debug, Default)]
struct MockRuntimeState {
    active_snapshot_id: Option<String>,
    purged_event_ids: HashSet<i64>,
    compact_calls: u64,
    unlearn_calls: u64,
    rollback_calls: u64,
    fault: MockRuntimeFault,
}

/// 内存态旁路快权重模拟器（支持正常神经 Compaction、定向遗忘与故障注入）
pub struct MockFastWeightRuntime {
    endpoint: String,
    state: Mutex<MockRuntimeState>,
}

impl Default for MockFastWeightRuntime {
    fn default() -> Self {
        Self::new("mock://fast-weight-runtime/v1")
    }
}

impl MockFastWeightRuntime {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            state: Mutex::new(MockRuntimeState::default()),
        }
    }

    pub fn set_fault(&self, fault: MockRuntimeFault) -> Result<()> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("MockFastWeightRuntime 状态锁已中毒"))?;
        guard.fault = fault;
        Ok(())
    }

    pub fn clear_fault(&self) -> Result<()> {
        self.set_fault(MockRuntimeFault::default())
    }

    pub fn purged_event_ids(&self) -> Result<Vec<i64>> {
        let guard = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("MockFastWeightRuntime 状态锁已中毒"))?;
        let mut ids: Vec<i64> = guard.purged_event_ids.iter().copied().collect();
        ids.sort_unstable();
        Ok(ids)
    }

    pub fn call_counts(&self) -> Result<(u64, u64, u64)> {
        let guard = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("MockFastWeightRuntime 状态锁已中毒"))?;
        Ok((guard.compact_calls, guard.unlearn_calls, guard.rollback_calls))
    }
}

impl FastWeightRuntimeClient for MockFastWeightRuntime {
    fn runtime_kind(&self) -> &'static str {
        "mock"
    }

    fn submit_compaction(
        &self,
        req: &FastWeightCompactRequest,
    ) -> Result<FastWeightCompactResponse> {
        let t0 = Instant::now();
        let mut guard = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("MockFastWeightRuntime 状态锁已中毒"))?;
        guard.compact_calls += 1;
        if let Some(ref err_msg) = guard.fault.rpc_error {
            anyhow::bail!("旁路 Runtime JSON-RPC 错误 (compact): {}", err_msg);
        }

        for id in &req.tombstone_ids {
            guard.purged_event_ids.insert(*id);
        }
        let snapshot_id = Uuid::new_v4().to_string();
        let norm_ratio = guard.fault.norm_ratio.unwrap_or(1.12);
        let eval_glue_delta = guard.fault.eval_glue_delta.unwrap_or(-0.01);
        let unlearn_leak_rate = guard.fault.unlearn_leak_rate.unwrap_or(0.0002);

        if norm_ratio <= 1.50 && eval_glue_delta >= -0.03 && unlearn_leak_rate < 0.001 {
            guard.active_snapshot_id = Some(snapshot_id.clone());
        }

        let clean_model = req
            .base_model_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>();
        let artifact_uri = format!(
            "mock://fast-weight/{}/{}/ep_{}.safetensors",
            clean_model,
            req.adapter_kind.as_str(),
            req.to_epoch
        );

        Ok(FastWeightCompactResponse {
            snapshot_id,
            artifact_uri,
            norm_ratio,
            eval_glue_delta,
            unlearn_leak_rate,
            merged_episode_count: req.episode_payloads.len(),
            runtime_ms: t0.elapsed().as_millis().max(1) as u64,
        })
    }

    fn submit_selective_unlearn(
        &self,
        req: &FastWeightUnlearnRequest,
    ) -> Result<FastWeightUnlearnResponse> {
        let t0 = Instant::now();
        let mut guard = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("MockFastWeightRuntime 状态锁已中毒"))?;
        guard.unlearn_calls += 1;
        if let Some(ref err_msg) = guard.fault.rpc_error {
            anyhow::bail!("旁路 Runtime JSON-RPC 错误 (unlearn): {}", err_msg);
        }

        for id in &req.tombstone_ids {
            guard.purged_event_ids.insert(*id);
        }
        let purged_snapshot_id = Uuid::new_v4().to_string();
        let norm_ratio = guard.fault.norm_ratio.unwrap_or(1.08);
        let eval_glue_delta = guard.fault.eval_glue_delta.unwrap_or(-0.005);
        let unlearn_leak_rate = guard.fault.unlearn_leak_rate.unwrap_or(0.0001);

        if norm_ratio <= 1.50 && eval_glue_delta >= -0.03 && unlearn_leak_rate < 0.001 {
            guard.active_snapshot_id = Some(purged_snapshot_id.clone());
        }

        let clean_model = req
            .base_model_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect::<String>();
        let artifact_uri = format!(
            "mock://fast-weight/{}/{}/unlearned_ep_{}.safetensors",
            clean_model,
            req.adapter_kind.as_str(),
            req.anchored_epoch
        );

        Ok(FastWeightUnlearnResponse {
            purged_snapshot_id,
            artifact_uri,
            norm_ratio,
            eval_glue_delta,
            unlearn_leak_rate,
            purged_event_count: req.tombstone_ids.len(),
            runtime_ms: t0.elapsed().as_millis().max(1) as u64,
        })
    }

    fn notify_rollback(
        &self,
        req: &FastWeightRollbackRequest,
    ) -> Result<FastWeightRollbackResponse> {
        let t0 = Instant::now();
        let mut guard = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("MockFastWeightRuntime 状态锁已中毒"))?;
        guard.rollback_calls += 1;
        if let Some(ref err_msg) = guard.fault.rpc_error {
            anyhow::bail!("旁路 Runtime JSON-RPC 错误 (rollback): {}", err_msg);
        }
        guard.active_snapshot_id = req.restored_snapshot_id.clone();
        Ok(FastWeightRollbackResponse {
            acknowledged: true,
            active_snapshot_id: guard.active_snapshot_id.clone(),
            runtime_ms: t0.elapsed().as_millis().max(1) as u64,
        })
    }

    fn health_probe(&self) -> Result<FastWeightRuntimeHealth> {
        let guard = self
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("MockFastWeightRuntime 状态锁已中毒"))?;
        Ok(FastWeightRuntimeHealth {
            runtime_kind: "mock".to_string(),
            endpoint: self.endpoint.clone(),
            reachable: guard.fault.rpc_error.is_none(),
            active_base_models: vec!["qwen3-coder-next".to_string()],
            detail: format!(
                "compact={}, unlearn={}, rollback={}, purged_ids={}",
                guard.compact_calls,
                guard.unlearn_calls,
                guard.rollback_calls,
                guard.purged_event_ids.len()
            ),
        })
    }
}

/// 生产旁路 HTTP JSON-RPC 2.0 客户端（对接外部 In-Place TTT / Atlas / Larimar / SCoL 服务）
pub struct HttpFastWeightRuntime {
    endpoint: String,
    bearer_token: Option<String>,
    client: reqwest::blocking::Client,
}

impl HttpFastWeightRuntime {
    pub fn new(
        endpoint: impl Into<String>,
        bearer_token: Option<String>,
        timeout_ms: u64,
    ) -> Result<Self> {
        let ep = endpoint.into().trim().trim_end_matches('/').to_string();
        if !(ep.starts_with("http://") || ep.starts_with("https://")) {
            anyhow::bail!(
                "fast_weight_runtime_url '{}' 必须以 http:// 或 https:// 开头",
                ep
            );
        }
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(timeout_ms.clamp(500, 120_000)))
            .build()
            .context("构建 HttpFastWeightRuntime 客户端失败")?;
        Ok(Self {
            endpoint: ep,
            bearer_token: bearer_token.filter(|s| !s.trim().is_empty()),
            client,
        })
    }

    fn call_rpc<Req: Serialize, Resp: DeserializeOwned>(
        &self,
        method: &str,
        params: &Req,
    ) -> Result<Resp> {
        let rpc_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": Uuid::new_v4().to_string(),
            "method": method,
            "params": params,
        });
        let mut req = self.client.post(&self.endpoint).json(&rpc_body);
        if let Some(ref tok) = self.bearer_token {
            req = req.bearer_auth(tok);
        }
        let resp = req
            .send()
            .with_context(|| format!("调用旁路快权重 JSON-RPC '{}' 失败 ({})", method, self.endpoint))?;
        let status = resp.status();
        let payload: serde_json::Value = resp
            .json()
            .with_context(|| format!("解析旁路 JSON-RPC '{}' 响应失败 (HTTP {})", method, status))?;
        if !status.is_success() {
            anyhow::bail!("旁路 JSON-RPC '{}' 返回 HTTP {}: {}", method, status, payload);
        }
        if let Some(err) = payload.get("error").filter(|v| !v.is_null()) {
            anyhow::bail!("旁路 JSON-RPC '{}' 返回错误: {}", method, err);
        }
        let result_val = payload
            .get("result")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("旁路 JSON-RPC '{}' 响应缺少 result 字段", method))?;
        serde_json::from_value(result_val)
            .with_context(|| format!("反序列化旁路 JSON-RPC '{}' result 失败", method))
    }
}

impl FastWeightRuntimeClient for HttpFastWeightRuntime {
    fn runtime_kind(&self) -> &'static str {
        "http_jsonrpc"
    }

    fn submit_compaction(
        &self,
        req: &FastWeightCompactRequest,
    ) -> Result<FastWeightCompactResponse> {
        self.call_rpc("fast_weight.compact", req)
    }

    fn submit_selective_unlearn(
        &self,
        req: &FastWeightUnlearnRequest,
    ) -> Result<FastWeightUnlearnResponse> {
        self.call_rpc("fast_weight.selective_unlearn", req)
    }

    fn notify_rollback(
        &self,
        req: &FastWeightRollbackRequest,
    ) -> Result<FastWeightRollbackResponse> {
        self.call_rpc("fast_weight.rollback", req)
    }

    fn health_probe(&self) -> Result<FastWeightRuntimeHealth> {
        self.call_rpc("fast_weight.health", &serde_json::json!({}))
    }
}
