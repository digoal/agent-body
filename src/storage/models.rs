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
    pub created_at: Option<String>,
}
