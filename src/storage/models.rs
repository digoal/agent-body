use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementRule {
    pub id: String,
    pub rule_name: String,
    pub pattern_type: String,
    pub target_base_dir: String,
    pub naming_pattern: Option<String>,
    pub description: Option<String>,
    pub guideline: Option<String>,
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanPolicy {
    pub id: String,
    pub path_pattern: String,
    pub tier: String, // "deep", "shallow", "restricted"
    pub max_depth: i32,
    pub enable_fts: bool,
    pub enable_vec: bool,
    pub enable_graph: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetRecord {
    pub id: String,
    pub path: String,
    pub asset_type: String, // directory, project, script, document, dataset, config, media, other
    pub owner: String,
    pub security_level: String, // public, internal, confidential, secret
    pub scan_tier: String,      // deep, shallow, restricted
    pub description: Option<String>,
    pub desc_kv: serde_json::Value,
    pub usage: Option<String>,
    pub size_bytes: Option<i64>,
    pub file_count: Option<i64>,
    pub tags: Vec<String>,
    pub status: String,
}

/// ISSUE #56: 数据库侧标量下推过滤条件
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssetScalarFilter {
    pub modality: Option<String>,
    pub asset_type: Option<String>,
    pub domain: Option<String>,
    pub scope_path: Option<String>,
    pub kv_key: Option<String>,
    pub kv_value: Option<String>,
    pub kv_filter: Option<String>,
}

/// ISSUE #56: 数据库侧模糊检索命中项 (Trigram 索引加速或纯数据库 LIKE/ILIKE 全文扫描)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbFuzzyAssetHit {
    pub asset: AssetRecord,
    pub match_mode: String, // "db_fuzzy_index" | "db_fuzzy_scan"
    pub rank_score: f64,
    pub matched_line: Option<usize>,
    pub matched_snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveProcessRecord {
    pub id: String,
    pub pid: u32,
    pub name: String,
    pub command: String,
    pub owner: String,
    pub associated_path: Option<String>,
    pub listening_ports: Vec<u16>,
    pub status: String,
    pub cpu_pct: f32,
    pub mem_bytes: u64,
    pub gpu_allocated_mb: u32,
    pub started_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: String,
    pub memory_type: String, // decision, task, experience, preference, fact, note
    pub content: String,
    pub summary: Option<String>,
    pub source_agent: Option<String>,
    pub source_session: Option<String>,
    pub tags: Vec<String>,
    pub weight: f32,
    pub pinned: bool,
    pub status: String,
    #[serde(default)]
    pub embedding: Vec<f32>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeRecord {
    pub id: Option<i64>,
    pub change_type: String, // file_created, file_modified, file_deleted, ...
    pub target: String,
    pub description: Option<String>,
    pub details: serde_json::Value,
    pub source: String,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkRecord {
    pub id: Option<i64>,
    pub source_type: String, // memory, asset, service, change
    pub source_id: String,
    pub target_type: String, // memory, asset, service, change
    pub target_id: String,
    pub relation: String,    // references, depends_on, produced_by, related_to, supersedes, contradicts, confirms
    pub confidence: f32,
    pub auto_discovered: bool,
    pub line_number: Option<i32>,
    pub snippet: Option<String>,
    pub provenance: Option<String>,
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanJobRecord {
    pub id: String,
    pub root_path: String,
    pub status: String, // pending, running, paused, completed, failed
    pub last_path: Option<String>,
    pub scanned_dirs: i64,
    pub scanned_files: i64,
    pub total_assets: i64,
    pub shallow_blocked: i64,
    pub error_message: Option<String>,
    pub started_at: Option<String>,
    pub paused_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactNodeRecord {
    pub id: String,
    pub node_type: String, // project, tool, port, service, cron, file, authoritative_site
    pub name: String,
    pub label: String,
    pub properties: serde_json::Value,
    pub updated_at: Option<String>,
}

fn default_evidence_tier() -> String {
    "L1_EXPLICIT".to_string()
}

fn default_edge_weight() -> f32 {
    1.0
}

fn default_direction_kind() -> String {
    "forward_dependency".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactEdgeRecord {
    pub id: Option<i64>,
    pub source_id: String,
    pub target_id: String,
    pub relation: String, // depends_on, listens_on, scheduled_by, reads, writes, part_of, calls, contains, uses_tool, runs_as, documented_by, semantically_related
    pub properties: serde_json::Value,
    pub confidence: f32,
    #[serde(default = "default_evidence_tier")]
    pub evidence_tier: String, // L1_EXPLICIT | L2_STRUCTURAL | L3_SEMANTIC
    #[serde(default = "default_edge_weight")]
    pub weight: f32,
    #[serde(default = "default_direction_kind")]
    pub direction_kind: String, // forward_dependency | reverse_schedule | derivation_flow | symmetric_affinity
    pub updated_at: Option<String>,
}

impl FactEdgeRecord {
    /// 根据关系名推导默认方向语义
    pub fn infer_direction_kind(relation: &str) -> &'static str {
        match relation {
            "scheduled_by" => "reverse_schedule",
            "documented_by" | "runs_as" | "part_of" | "contains" => "derivation_flow",
            "semantically_related" => "symmetric_affinity",
            _ => "forward_dependency",
        }
    }

    /// 构造标准 L1 物理显式边
    pub fn new_explicit(
        source_id: impl Into<String>,
        target_id: impl Into<String>,
        relation: impl Into<String>,
        properties: serde_json::Value,
        confidence: f32,
    ) -> Self {
        let rel = relation.into();
        let dir = Self::infer_direction_kind(&rel).to_string();
        Self {
            id: None,
            source_id: source_id.into(),
            target_id: target_id.into(),
            relation: rel,
            properties,
            confidence,
            evidence_tier: "L1_EXPLICIT".to_string(),
            weight: confidence.clamp(0.0, 1.0),
            direction_kind: dir,
            updated_at: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SubGraph {
    pub root_node: Option<FactNodeRecord>,
    pub nodes: Vec<FactNodeRecord>,
    pub edges: Vec<FactEdgeRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinationLock {
    pub lock_id: String,
    pub resource_type: String, // "port", "directory", "service", "file", "custom"
    pub resource_id: String,   // "8080", "/etc/nginx/conf.d/", "nginx.service", etc.
    pub scope: String,         // "exclusive", "shared"
    pub holder_agent: String,  // session_id or agent name
    pub holder_pid: Option<u32>,
    pub acquired_at: String,   // ISO8601
    pub expires_at: String,    // ISO8601
    pub metadata: Option<serde_json::Value>,
}

// ─── Phase 16: 生态认知目录与外部服务通讯录 ──────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpCatalogRecord {
    pub id: String,
    pub server_name: String,
    pub client_source: String,
    pub transport: String,
    pub command_or_url: String,
    pub tools_json: serde_json::Value,
    pub tool_count: usize,
    pub est_total_tokens: usize,
    pub status: String,
    pub last_scanned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCatalogRecord {
    pub id: String,
    pub skill_name: String,
    pub root_source: String,
    pub skill_path: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub est_tokens: usize,
    pub has_scripts: bool,
    pub has_references: bool,
    pub status: String,
    pub last_scanned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalServiceRecord {
    pub id: String,
    pub service_name: String,
    pub category: String,
    pub endpoint: String,
    pub protocol: String,
    pub capabilities: String,
    pub auth_type: String,
    pub auth_env_var: Option<String>,
    pub health_endpoint: Option<String>,
    pub usage_example: Option<String>,
    pub status: String,
    pub last_checked_at: Option<String>,
}

// ─── Phase 17: 协作锁等待队列 ──────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockWaitEntry {
    pub wait_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub scope: String,
    pub waiting_agent: String,
    pub priority: i32,
    pub enqueued_at: String,
    pub timeout_at: String,
    pub status: String, // "waiting" | "notified" | "acquired" | "expired" | "cancelled"
}

/// Phase 41: 联邦网格跨机结构化 Agent 对话记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshDialogueRecord {
    pub dialogue_id: String,
    pub thread_id: String,
    pub from_node: String,
    pub from_agent: String,
    pub to_node: String,
    pub to_agent: String,
    pub intent: String, // ASK_CONTEXT | HANDOFF_TASK | SHARE_MEMORY | REPLY
    pub content: String,
    pub redaction_hits: usize,
    pub injection_flagged: bool,
    pub verified_signature: bool,
    pub status: String, // sent | received | acked | replied
    pub created_at: String,
}

/// Phase 41: 联邦网格跨机受控沙盒文件投递记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshTransferRecord {
    pub transfer_id: String,
    pub from_node: String,
    pub from_agent: String,
    pub to_node: String,
    pub file_name: String,
    pub size_bytes: u64,
    pub sha256: String,
    pub inbox_path: String,
    pub secret_warnings: usize,
    pub verified_signature: bool,
    pub status: String, // sent | quarantined | accepted | rejected
    pub accepted_path: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Phase 27: 实验室 A/B 基准评测批次记录 (轨道 A)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkRunRecord {
    pub run_id: String,
    pub suite_name: String,
    pub git_commit: String,
    pub app_version: String,
    pub host_fingerprint: serde_json::Value,
    pub workload_count: usize,
    pub total_baseline_tokens: i64,
    pub total_body_tokens: i64,
    pub overall_token_savings_pct: f64,
    pub overall_speedup_factor: f64,
    pub total_hazards_intercepted: i64,
    pub total_noise_blocked: i64,
    pub sla_passed: bool,
    pub results_json: serde_json::Value,
    pub report_sha256: String,
    pub duration_ms: i64,
    pub created_at: String,
}

/// Phase 27: 生产运行态效能存证账本单条事件 (轨道 B)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectivenessLedgerRecord {
    pub event_id: String,
    pub session_id: String,
    pub source_channel: String,
    pub tool_or_action: String,
    pub provenance_rule: String,
    pub baseline_tokens: i64,
    pub body_tokens: i64,
    pub tokens_saved: i64,
    pub baseline_rounds: i64,
    pub body_rounds: i64,
    pub rounds_saved: i64,
    pub hazard_intercepted: i64,
    pub noise_blocked: i64,
    pub elapsed_us: i64,
    pub evidence_summary: String,
    pub created_at: String,
}

/// 按推导规则聚合统计
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleAggregate {
    pub provenance_rule: String,
    pub event_count: usize,
    pub tokens_saved: i64,
    pub rounds_saved: i64,
    pub hazards_intercepted: i64,
    pub noise_blocked: i64,
}

/// 生产运行态效能存证汇总账单
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EffectivenessSummary {
    pub window: String,
    pub total_events: usize,
    pub total_baseline_tokens: i64,
    pub total_body_tokens: i64,
    pub total_tokens_saved: i64,
    pub token_savings_pct: f64,
    pub total_rounds_saved: i64,
    pub total_hazards_intercepted: i64,
    pub total_noise_blocked: i64,
    pub avg_elapsed_us: f64,
    pub by_rule: Vec<RuleAggregate>,
    pub recent_events: Vec<EffectivenessLedgerRecord>,
}

/// Phase 43: 双域全链路行为审计事件记录 (IN_AGENT / OUT_OF_AGENT)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEventRecord {
    pub event_id: String,
    pub trace_id: String,
    pub session_id: String,
    /// "IN_AGENT" | "OUT_OF_AGENT"
    pub domain: String,
    /// "agent_session" | "mcp_client" | "cli_user" | "web_user" | "mesh_peer" | "daemon_scheduler" | "watcher_inotify" | "governance_engine"
    pub actor_kind: String,
    pub actor_id: String,
    pub source_channel: String,
    pub category: String,
    pub action: String,
    pub target_type: String,
    pub target_id: String,
    pub concurrency_mode: Option<String>,
    /// "success" | "warn" | "blocked" | "failed" | "recovered" | "dry_run"
    pub outcome: String,
    pub before_after_delta: serde_json::Value,
    pub elapsed_us: i64,
    pub evidence_sha256: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditQueryFilter {
    pub domain: Option<String>,
    pub actor_kind: Option<String>,
    pub category: Option<String>,
    pub action: Option<String>,
    pub outcome: Option<String>,
    pub trace_id: Option<String>,
    pub session_id: Option<String>,
    pub target_contains: Option<String>,
    pub window: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditDomainBreakdown {
    pub domain: String,
    pub actor_kind: String,
    pub outcome: String,
    pub event_count: usize,
    pub avg_elapsed_us: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditDomainSummary {
    pub window: String,
    pub total_events: usize,
    pub in_agent_count: usize,
    pub out_of_agent_count: usize,
    pub failed_or_blocked_count: usize,
    pub recovered_count: usize,
    pub breakdown: Vec<AuditDomainBreakdown>,
}

/// Phase 44: Agent 本体跨大脑连续性接力胶囊记录 (`body_checkpoints` — 第 27 张表)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyCheckpointRecord {
    pub checkpoint_id: String,
    pub body_id: String,
    pub from_brain: String,
    pub session_id: String,
    pub goal_summary: String,
    pub active_files: Vec<String>,
    pub active_locks: Vec<String>,
    pub verified_facts: Vec<String>,
    pub open_blockers: Vec<String>,
    pub next_step_hint: String,
    pub token_budget: usize,
    /// "active" | "resumed" | "expired" | "archived"
    pub status: String,
    pub evidence_sha256: String,
    pub created_at: String,
    pub resumed_by_brain: Option<String>,
    pub resumed_at: Option<String>,
    /// Phase 76: 跨大脑接力锚定的事务记忆逻辑纪元 (`tx_epoch`，0 表示未绑定 PG 事务纪元)
    #[serde(default)]
    pub tx_epoch: i64,
    /// Phase 78 (方案 B / M3): 热态快权重跨大脑接力锚定快照 UUID
    #[serde(default)]
    pub fast_weight_snapshot_id: Option<String>,
    /// Phase 78 (方案 B / M3): 该快权重快照绑定的基座模型 ID（异构基座接力时自动回退文本胶囊）
    #[serde(default)]
    pub fast_weight_base_model: Option<String>,
}
