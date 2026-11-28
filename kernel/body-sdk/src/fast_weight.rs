//! 方案 B 旁路快权重血缘控制面与神经 Compaction 线协议契约 (`agent-body-sdk`)
//!
//! 严守「脑体分离 × 递弱代偿」架构红线：
//! - 本模块仅定义标准化线协议（JSON-RPC 2.0 请求/响应）、三道硬门禁判定规则与 URI 校验；
//! - 绝不包含任何权重矩阵、神经网络推理/训练循环或平台特定业务词。

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// 旁路模型运行时支持的快权重适配器形态
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AdapterKind {
    #[serde(rename = "inplace_ttt")]
    InplaceTtt,
    #[serde(rename = "atlas_state")]
    AtlasState,
    #[serde(rename = "larimar_mem")]
    LarimarMem,
    #[serde(rename = "scol_lora")]
    ScolLora,
}

impl AdapterKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InplaceTtt => "inplace_ttt",
            Self::AtlasState => "atlas_state",
            Self::LarimarMem => "larimar_mem",
            Self::ScolLora => "scol_lora",
        }
    }

    pub fn parse_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "inplace_ttt" | "ttt" => Ok(Self::InplaceTtt),
            "atlas_state" | "atlas" => Ok(Self::AtlasState),
            "larimar_mem" | "larimar" => Ok(Self::LarimarMem),
            "scol_lora" | "scol" | "lora" => Ok(Self::ScolLora),
            other => anyhow::bail!(
                "未知快权重 adapter_kind: '{}'，允许值: inplace_ttt | atlas_state | larimar_mem | scol_lora",
                other
            ),
        }
    }
}

/// 快权重快照生命周期状态机（`warming -> active -> compacted | rolled_back`）
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum FastWeightSnapshotStatus {
    #[serde(rename = "warming")]
    Warming,
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "compacted")]
    Compacted,
    #[serde(rename = "rolled_back")]
    RolledBack,
}

impl FastWeightSnapshotStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Warming => "warming",
            Self::Active => "active",
            Self::Compacted => "compacted",
            Self::RolledBack => "rolled_back",
        }
    }

    pub fn parse_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "warming" => Ok(Self::Warming),
            "active" => Ok(Self::Active),
            "compacted" => Ok(Self::Compacted),
            "rolled_back" => Ok(Self::RolledBack),
            other => anyhow::bail!(
                "未知快权重快照状态: '{}'，允许值: warming | active | compacted | rolled_back",
                other
            ),
        }
    }
}

/// 方案 B 三道硬门禁安全阈值（范数防雪崩 / 通用能力回归 / 定向遗忘泄漏率）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightGatePolicy {
    /// 权重范数增量比上限（默认 1.50）
    pub max_norm_ratio: f32,
    /// 通用基准准确率变化下限（默认 -0.03，即衰减必须 < 3%）
    pub min_eval_glue_delta: f32,
    /// 定向遗忘后墓碑诱导召回泄漏率上限（默认 0.001，即 < 0.1%）
    pub max_unlearn_leak_rate: f32,
}

impl Default for FastWeightGatePolicy {
    fn default() -> Self {
        Self {
            max_norm_ratio: 1.50,
            min_eval_glue_delta: -0.03,
            max_unlearn_leak_rate: 0.001,
        }
    }
}

/// 旁路运行时返回的候选快照评测指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightGateMetrics {
    pub norm_ratio: f32,
    pub eval_glue_delta: f32,
    pub unlearn_leak_rate: f32,
}

/// 三道硬门禁评估结论
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightGateVerdict {
    pub passed: bool,
    pub norm_ok: bool,
    pub glue_ok: bool,
    pub unlearn_ok: bool,
    pub reasons: Vec<String>,
}

/// 确定性校验候选快权重快照是否全部满足三道硬门禁
pub fn evaluate_fast_weight_gates(
    metrics: &FastWeightGateMetrics,
    policy: &FastWeightGatePolicy,
) -> FastWeightGateVerdict {
    let mut reasons = Vec::new();

    let norm_ok = metrics.norm_ratio.is_finite()
        && metrics.norm_ratio > 0.0
        && metrics.norm_ratio <= policy.max_norm_ratio;
    if !norm_ok {
        reasons.push(format!(
            "norm_ratio={} 超出安全阈值 (0.0, {}]",
            metrics.norm_ratio, policy.max_norm_ratio
        ));
    }

    let glue_ok = metrics.eval_glue_delta.is_finite()
        && metrics.eval_glue_delta >= policy.min_eval_glue_delta;
    if !glue_ok {
        reasons.push(format!(
            "eval_glue_delta={} 低于通用回归红线 >= {}",
            metrics.eval_glue_delta, policy.min_eval_glue_delta
        ));
    }

    let unlearn_ok = metrics.unlearn_leak_rate.is_finite()
        && metrics.unlearn_leak_rate >= 0.0
        && metrics.unlearn_leak_rate < policy.max_unlearn_leak_rate;
    if !unlearn_ok {
        reasons.push(format!(
            "unlearn_leak_rate={} 超出遗忘泄漏上限 < {}",
            metrics.unlearn_leak_rate, policy.max_unlearn_leak_rate
        ));
    }

    FastWeightGateVerdict {
        passed: norm_ok && glue_ok && unlearn_ok,
        norm_ok,
        glue_ok,
        unlearn_ok,
        reasons,
    }
}

/// 校验快权重制品 URI 合法性：仅允许外挂存储引用，严禁内联 `data:` 权重荷载入库
pub fn validate_artifact_uri(uri: &str) -> std::result::Result<(), String> {
    let trimmed = uri.trim();
    if trimmed.is_empty() {
        return Err("artifact_uri 不能为空".to_string());
    }
    if trimmed.len() > 512 {
        return Err(format!("artifact_uri 长度 {} 超过 512 字符上限", trimmed.len()));
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("data:") {
        return Err("禁止通过 data: URI 内联模型权重荷载".to_string());
    }
    let allowed = ["s3://", "file://", "neurstore://", "http://", "https://", "mock://"];
    if !allowed.iter().any(|p| lower.starts_with(p)) {
        return Err(format!(
            "artifact_uri '{}' 必须使用受支持的外部协议前缀 ({})",
            trimmed,
            allowed.join(" / ")
        ));
    }
    if trimmed.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err("artifact_uri 不允许包含控制字符或空白字符".to_string());
    }
    Ok(())
}

/// JSON-RPC 2.0 请求：触发旁路神经 Compaction (`fast_to_warm_lora`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightCompactRequest {
    pub body_id: String,
    pub tenant_scope: String,
    pub base_model_id: String,
    pub adapter_kind: AdapterKind,
    pub parent_snapshot_id: Option<String>,
    pub from_epoch: i64,
    pub to_epoch: i64,
    pub episode_payloads: Vec<String>,
    pub tombstone_ids: Vec<i64>,
    pub audit_trace_id: String,
}

/// JSON-RPC 2.0 响应：旁路神经 Compaction 产物与门禁指标
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightCompactResponse {
    pub snapshot_id: String,
    pub artifact_uri: String,
    pub norm_ratio: f32,
    pub eval_glue_delta: f32,
    pub unlearn_leak_rate: f32,
    pub merged_episode_count: usize,
    pub runtime_ms: u64,
}

/// JSON-RPC 2.0 请求：下发定向遗忘墓碑 (`selective_unlearn`)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightUnlearnRequest {
    pub body_id: String,
    pub tenant_scope: String,
    pub base_model_id: String,
    pub adapter_kind: AdapterKind,
    pub active_snapshot_id: Option<String>,
    pub anchored_epoch: i64,
    pub tombstone_ids: Vec<i64>,
    pub redacted_fact_keys: Vec<String>,
    pub audit_trace_id: String,
}

/// JSON-RPC 2.0 响应：定向遗忘净化后新快照与残留泄漏评测
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightUnlearnResponse {
    pub purged_snapshot_id: String,
    pub artifact_uri: String,
    pub norm_ratio: f32,
    pub eval_glue_delta: f32,
    pub unlearn_leak_rate: f32,
    pub purged_event_count: usize,
    pub runtime_ms: u64,
}

/// JSON-RPC 2.0 请求：通知旁路运行时按 `tx_epoch` 回滚至指定快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightRollbackRequest {
    pub body_id: String,
    pub tenant_scope: String,
    pub target_epoch: i64,
    pub restored_snapshot_id: Option<String>,
    pub rolled_back_snapshot_ids: Vec<String>,
    pub audit_trace_id: String,
}

/// JSON-RPC 2.0 响应：旁路运行时回滚确认
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightRollbackResponse {
    pub acknowledged: bool,
    pub active_snapshot_id: Option<String>,
    pub runtime_ms: u64,
}

/// 旁路模型运行时健康快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FastWeightRuntimeHealth {
    pub runtime_kind: String,
    pub endpoint: String,
    pub reachable: bool,
    pub active_base_models: Vec<String>,
    pub detail: String,
}

/// 旁路 Model Runtime 控制面客户端抽象（支持外部 HTTP JSON-RPC 2.0 或 CI Mock）
pub trait FastWeightRuntimeClient: Send + Sync {
    fn runtime_kind(&self) -> &'static str;

    fn submit_compaction(
        &self,
        req: &FastWeightCompactRequest,
    ) -> Result<FastWeightCompactResponse>;

    fn submit_selective_unlearn(
        &self,
        req: &FastWeightUnlearnRequest,
    ) -> Result<FastWeightUnlearnResponse>;

    fn notify_rollback(
        &self,
        req: &FastWeightRollbackRequest,
    ) -> Result<FastWeightRollbackResponse>;

    fn health_probe(&self) -> Result<FastWeightRuntimeHealth>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_weight_gates_and_uri_validation() {
        let policy = FastWeightGatePolicy::default();

        let ok = evaluate_fast_weight_gates(
            &FastWeightGateMetrics {
                norm_ratio: 1.12,
                eval_glue_delta: -0.015,
                unlearn_leak_rate: 0.0002,
            },
            &policy,
        );
        assert!(ok.passed);
        assert!(ok.reasons.is_empty());

        let bad_norm = evaluate_fast_weight_gates(
            &FastWeightGateMetrics {
                norm_ratio: 2.35,
                eval_glue_delta: 0.0,
                unlearn_leak_rate: 0.0,
            },
            &policy,
        );
        assert!(!bad_norm.passed);
        assert!(!bad_norm.norm_ok);

        let bad_glue = evaluate_fast_weight_gates(
            &FastWeightGateMetrics {
                norm_ratio: 1.10,
                eval_glue_delta: -0.05,
                unlearn_leak_rate: 0.0,
            },
            &policy,
        );
        assert!(!bad_glue.passed);
        assert!(!bad_glue.glue_ok);

        let bad_leak = evaluate_fast_weight_gates(
            &FastWeightGateMetrics {
                norm_ratio: 1.10,
                eval_glue_delta: 0.0,
                unlearn_leak_rate: 0.001,
            },
            &policy,
        );
        assert!(!bad_leak.passed);
        assert!(!bad_leak.unlearn_ok);

        assert!(validate_artifact_uri("neurstore://tenant-a/lora-ep12.safetensors").is_ok());
        assert!(validate_artifact_uri("s3://bucket/snap/fw.bin").is_ok());
        assert!(validate_artifact_uri("data:application/octet-stream;base64,AAA").is_err());
        assert!(validate_artifact_uri("").is_err());
    }
}
