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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactEdgeRecord {
    pub id: Option<i64>,
    pub source_id: String,
    pub target_id: String,
    pub relation: String, // depends_on, listens_on, scheduled_by, reads, writes, part_of, calls, contains
    pub properties: serde_json::Value,
    pub confidence: f32,
    pub updated_at: Option<String>,
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
    pub total_steward_tokens: i64,
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
    pub steward_tokens: i64,
    pub tokens_saved: i64,
    pub baseline_rounds: i64,
    pub steward_rounds: i64,
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EffectivenessSummary {
    pub window: String,
    pub total_events: usize,
    pub total_baseline_tokens: i64,
    pub total_steward_tokens: i64,
    pub total_tokens_saved: i64,
    pub token_savings_pct: f64,
    pub total_rounds_saved: i64,
    pub total_hazards_intercepted: i64,
    pub total_noise_blocked: i64,
    pub avg_elapsed_us: f64,
    pub by_rule: Vec<RuleAggregate>,
    pub recent_events: Vec<EffectivenessLedgerRecord>,
}
