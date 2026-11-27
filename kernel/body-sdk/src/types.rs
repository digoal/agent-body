//! 微内核通用数据类型、证据元组与二维正交 AOPCC 策略推导

use std::collections::BTreeMap;

use chrono::FixedOffset;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 返回当前北京时区 (+08:00) RFC3339 时间戳
pub fn rfc3339_beijing_now() -> String {
    match FixedOffset::east_opt(8 * 3600) {
        Some(offset) => chrono::Utc::now().with_timezone(&offset).to_rfc3339(),
        None => chrono::Utc::now().to_rfc3339(),
    }
}

/// 统一物理与外部证据元组（所有 FdwRoutine / 检索结果的通用信封）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceTuple {
    pub evidence_id: String,
    pub host_id: String,
    pub host_labels: BTreeMap<String, String>,
    pub entity_type: String,
    pub entity_id: String,
    pub source_ext: String,
    pub evidence_tier: String,
    pub collected_at: String,
    pub ttl_sec: u64,
    pub payload: serde_json::Value,
    pub redacted: bool,
    pub evidence_sha256: String,
}

impl EvidenceTuple {
    /// 基于核心不可变语义字段计算确定性 SHA-256 指纹（排除 `evidence_sha256` 自身）
    pub fn compute_sha256(&self) -> String {
        let mut hasher = Sha256::new();
        let feed_str = |h: &mut Sha256, tag: &str, val: &str| {
            h.update(tag.as_bytes());
            h.update(b":");
            h.update((val.len() as u64).to_le_bytes());
            h.update(val.as_bytes());
            h.update(b"|");
        };
        feed_str(&mut hasher, "evidence_id", &self.evidence_id);
        feed_str(&mut hasher, "host_id", &self.host_id);
        hasher.update(b"host_labels:");
        hasher.update((self.host_labels.len() as u64).to_le_bytes());
        for (k, v) in &self.host_labels {
            feed_str(&mut hasher, k, v);
        }
        feed_str(&mut hasher, "entity_type", &self.entity_type);
        feed_str(&mut hasher, "entity_id", &self.entity_id);
        feed_str(&mut hasher, "source_ext", &self.source_ext);
        feed_str(&mut hasher, "evidence_tier", &self.evidence_tier);
        feed_str(&mut hasher, "collected_at", &self.collected_at);
        hasher.update(b"ttl_sec:");
        hasher.update(self.ttl_sec.to_le_bytes());
        hasher.update(b"|redacted:");
        hasher.update([u8::from(self.redacted)]);
        let payload_canon = canonical_json_string(&self.payload);
        feed_str(&mut hasher, "payload", &payload_canon);

        let digest = hasher.finalize();
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(64);
        for byte in digest {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
        out
    }

    /// 计算并回填 `evidence_sha256`
    pub fn with_computed_sha256(mut self) -> Self {
        self.evidence_sha256 = self.compute_sha256();
        self
    }

    /// 校验当前 `evidence_sha256` 是否与元组内容一致
    pub fn verify_sha256(&self) -> bool {
        !self.evidence_sha256.is_empty() && self.evidence_sha256 == self.compute_sha256()
    }
}

fn canonical_json_string(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Object(map) => {
            let mut sorted: BTreeMap<&str, &serde_json::Value> = BTreeMap::new();
            for (k, v) in map {
                sorted.insert(k.as_str(), v);
            }
            let parts: Vec<String> = sorted
                .into_iter()
                .map(|(k, v)| {
                    let key_json = serde_json::to_string(k).unwrap_or_else(|_| "\"\"".to_string());
                    format!("{key_json}:{}", canonical_json_string(v))
                })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
        serde_json::Value::Array(arr) => {
            let parts: Vec<String> = arr.iter().map(canonical_json_string).collect();
            format!("[{}]", parts.join(","))
        }
        _ => serde_json::to_string(val).unwrap_or_else(|_| "null".to_string()),
    }
}

/// 通用属性图有向加权边
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KernelEdge {
    pub edge_id: String,
    pub src_id: String,
    pub dst_id: String,
    pub edge_type: String,
    pub weight: f32,
    pub evidence_tier: String,
    pub source_ext: String,
    pub collected_at: String,
    pub payload: serde_json::Value,
}

/// 多跳图遍历子图输出
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KernelSubGraph {
    pub root_id: String,
    pub nodes: Vec<EvidenceTuple>,
    pub edges: Vec<KernelEdge>,
    pub truncated: bool,
}

/// Append-Only 通用事件与审计流记录
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KernelEventRecord {
    pub event_id: String,
    pub stream: String,
    pub event_type: String,
    pub source_ext: String,
    pub entity_id: Option<String>,
    pub occurred_at: String,
    pub payload: serde_json::Value,
    pub evidence_sha256: String,
}

/// 事件流时间窗与过滤条件
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventQueryFilter {
    pub stream: Option<String>,
    pub event_type: Option<String>,
    pub source_ext: Option<String>,
    pub since_iso: Option<String>,
    pub until_iso: Option<String>,
    pub limit: usize,
}

/// 通用实体扫描过滤条件
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityScanFilter {
    pub entity_type: Option<String>,
    pub source_ext: Option<String>,
    pub host_id: Option<String>,
    pub label_filters: BTreeMap<String, String>,
    pub limit: usize,
    pub offset: usize,
}

/// 协作租约模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaseMode {
    Shared,
    Exclusive,
}

/// 内核协作租约实体
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KernelLease {
    pub lock_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub holder_id: String,
    pub mode: LeaseMode,
    pub acquired_at: String,
    pub expires_at: String,
    pub fence_token: u64,
}

/// 协作租约 CAS 申请请求
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeaseAcquireReq {
    pub resource_type: String,
    pub resource_id: String,
    pub holder_id: String,
    pub mode: LeaseMode,
    pub ttl_sec: u64,
}

/// 协作租约申请结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LeaseOutcome {
    Acquired(KernelLease),
    Busy {
        holder_id: String,
        expires_at: String,
    },
}

/// 调用方身份凭证上下文
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallerIdentity {
    pub caller_id: String,
    pub tenant_id: Option<String>,
    pub project_id: Option<String>,
    pub env: Option<String>,
    pub roles: Vec<String>,
    pub auth_method: String,
    pub is_loopback: bool,
}

/// 行级安全过滤谓词 (RLS)，用于下推至 TableAM / IndexAM
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowFilterPredicate {
    pub tenant_id: Option<String>,
    pub project_id: Option<String>,
    pub env: Option<String>,
    pub allowed_host_ids: Option<Vec<String>>,
    pub custom_labels: BTreeMap<String, String>,
}

impl RowFilterPredicate {
    /// 内存级二次兜底校验：判断证据元组是否满足行级过滤谓词
    pub fn matches_tuple(&self, tuple: &EvidenceTuple) -> bool {
        if let Some(ref tid) = self.tenant_id {
            if tuple.host_labels.get("tenant_id") != Some(tid) {
                return false;
            }
        }
        if let Some(ref pid) = self.project_id {
            if tuple.host_labels.get("project_id") != Some(pid) {
                return false;
            }
        }
        if let Some(ref env) = self.env {
            if tuple.host_labels.get("env") != Some(env) {
                return false;
            }
        }
        if let Some(ref hosts) = self.allowed_host_ids {
            if !hosts.iter().any(|h| h == &tuple.host_id) {
                return false;
            }
        }
        for (k, v) in &self.custom_labels {
            if tuple.host_labels.get(k) != Some(v) {
                return false;
            }
        }
        true
    }
}

/// 四级自治权限边界
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AutonomyLevel {
    #[serde(rename = "L0_OBSERVE")]
    L0Observe,
    #[serde(rename = "L1_AUTONOMIC")]
    L1Autonomic,
    #[serde(rename = "L2_SUPERVISED")]
    L2Supervised,
    #[serde(rename = "L3_FORBIDDEN")]
    L3Forbidden,
}

impl AutonomyLevel {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::L0Observe => "L0_OBSERVE",
            Self::L1Autonomic => "L1_AUTONOMIC",
            Self::L2Supervised => "L2_SUPERVISED",
            Self::L3Forbidden => "L3_FORBIDDEN",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "L0_OBSERVE" => Some(Self::L0Observe),
            "L1_AUTONOMIC" => Some(Self::L1Autonomic),
            "L2_SUPERVISED" => Some(Self::L2Supervised),
            "L3_FORBIDDEN" => Some(Self::L3Forbidden),
            _ => None,
        }
    }
}

/// 算子波动性分类（对标 PostgreSQL IMMUTABLE / STABLE / VOLATILE）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcVolatility {
    Immutable,
    Stable,
    Volatile,
}

impl ProcVolatility {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Immutable => "immutable",
            Self::Stable => "stable",
            Self::Volatile => "volatile",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "immutable" => Some(Self::Immutable),
            "stable" => Some(Self::Stable),
            "volatile" => Some(Self::Volatile),
            _ => None,
        }
    }
}

/// AOPCC 并发与保护策略模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AopccMode {
    Optimistic,
    CautiousCas,
    PessimisticLock,
    Denied,
}

/// 算子波动性与自治边界的二维正交 AOPCC 策略推导
pub const fn derive_aopcc_mode(
    volatility: ProcVolatility,
    autonomy: AutonomyLevel,
) -> AopccMode {
    match (volatility, autonomy) {
        (_, AutonomyLevel::L3Forbidden) => AopccMode::Denied,
        (ProcVolatility::Immutable, AutonomyLevel::L0Observe)
        | (ProcVolatility::Immutable, AutonomyLevel::L1Autonomic)
        | (ProcVolatility::Stable, AutonomyLevel::L0Observe) => AopccMode::Optimistic,
        (ProcVolatility::Immutable, AutonomyLevel::L2Supervised)
        | (ProcVolatility::Stable, AutonomyLevel::L1Autonomic)
        | (ProcVolatility::Volatile, AutonomyLevel::L0Observe) => AopccMode::CautiousCas,
        (ProcVolatility::Stable, AutonomyLevel::L2Supervised)
        | (ProcVolatility::Volatile, AutonomyLevel::L1Autonomic)
        | (ProcVolatility::Volatile, AutonomyLevel::L2Supervised) => AopccMode::PessimisticLock,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_tuple_sha256_is_deterministic_and_sensitive() {
        let mut labels = BTreeMap::new();
        labels.insert("env".to_string(), "prod".to_string());
        labels.insert("tenant_id".to_string(), "t-001".to_string());

        let tuple = EvidenceTuple {
            evidence_id: "ev-1".to_string(),
            host_id: "host-a".to_string(),
            host_labels: labels,
            entity_type: "entity:process".to_string(),
            entity_id: "pid:100".to_string(),
            source_ext: "probe_core".to_string(),
            evidence_tier: "L1_EXPLICIT".to_string(),
            collected_at: "2026-11-27T16:00:00+08:00".to_string(),
            ttl_sec: 30,
            payload: serde_json::json!({"b": 2, "a": 1}),
            redacted: false,
            evidence_sha256: String::new(),
        }
        .with_computed_sha256();

        assert_eq!(tuple.evidence_sha256.len(), 64);
        assert!(tuple.verify_sha256());

        let mut reordered = tuple.clone();
        reordered.payload = serde_json::json!({"a": 1, "b": 2});
        assert_eq!(reordered.compute_sha256(), tuple.evidence_sha256);

        let mut tampered = tuple.clone();
        tampered.ttl_sec = 31;
        assert!(!tampered.verify_sha256());
    }

    #[test]
    fn derive_aopcc_mode_matches_orthogonal_matrix() {
        use AopccMode::*;
        use AutonomyLevel::*;
        use ProcVolatility::*;

        assert_eq!(derive_aopcc_mode(Immutable, L0Observe), Optimistic);
        assert_eq!(derive_aopcc_mode(Immutable, L1Autonomic), Optimistic);
        assert_eq!(derive_aopcc_mode(Immutable, L2Supervised), CautiousCas);
        assert_eq!(derive_aopcc_mode(Immutable, L3Forbidden), Denied);

        assert_eq!(derive_aopcc_mode(Stable, L0Observe), Optimistic);
        assert_eq!(derive_aopcc_mode(Stable, L1Autonomic), CautiousCas);
        assert_eq!(derive_aopcc_mode(Stable, L2Supervised), PessimisticLock);
        assert_eq!(derive_aopcc_mode(Stable, L3Forbidden), Denied);

        assert_eq!(derive_aopcc_mode(Volatile, L0Observe), CautiousCas);
        assert_eq!(derive_aopcc_mode(Volatile, L1Autonomic), PessimisticLock);
        assert_eq!(derive_aopcc_mode(Volatile, L2Supervised), PessimisticLock);
        assert_eq!(derive_aopcc_mode(Volatile, L3Forbidden), Denied);
    }

    #[test]
    fn row_filter_predicate_enforces_labels_and_hosts() {
        let mut labels = BTreeMap::new();
        labels.insert("tenant_id".to_string(), "t1".to_string());
        labels.insert("env".to_string(), "staging".to_string());

        let tuple = EvidenceTuple {
            evidence_id: "ev-2".to_string(),
            host_id: "node-1".to_string(),
            host_labels: labels,
            entity_type: "entity:socket".to_string(),
            entity_id: "sock:80".to_string(),
            source_ext: "probe_net".to_string(),
            evidence_tier: "L1_EXPLICIT".to_string(),
            collected_at: "2026-11-27T16:00:00+08:00".to_string(),
            ttl_sec: 10,
            payload: serde_json::json!({}),
            redacted: true,
            evidence_sha256: String::new(),
        };

        let matching = RowFilterPredicate {
            tenant_id: Some("t1".to_string()),
            env: Some("staging".to_string()),
            allowed_host_ids: Some(vec!["node-1".to_string()]),
            ..Default::default()
        };
        assert!(matching.matches_tuple(&tuple));

        let wrong_tenant = RowFilterPredicate {
            tenant_id: Some("t2".to_string()),
            ..Default::default()
        };
        assert!(!wrong_tenant.matches_tuple(&tuple));
    }
}
