//! Agent 本体身份证与六大器官自省 (`BodyManifest`)

use std::sync::OnceLock;

use chrono::Local;
use serde::{Deserialize, Serialize};
use sysinfo::System;

use crate::benchmark::provenance::sha256_hex;
use crate::storage::Storage;

static CACHED_BODY_ID: OnceLock<String> = OnceLock::new();

/// 确定性生成宿主机本体指纹 (`body-<16hex>`)，常驻内存微秒级命中
pub fn compute_body_id() -> String {
    CACHED_BODY_ID
        .get_or_init(|| {
            let host = System::host_name().unwrap_or_else(|| "default-host".to_string());
            let machine_id = std::fs::read_to_string("/etc/machine-id")
                .or_else(|_| std::fs::read_to_string("/var/lib/dbus/machine-id"))
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| "no-machine-id".to_string());
            let raw = format!("agent-body::{}::{}", host.trim(), machine_id);
            let digest = sha256_hex(&raw);
            let short: String = digest.chars().take(16).collect();
            format!("body-{}", short)
        })
        .clone()
}

/// 生理器官自省快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganStatus {
    pub name: String,
    pub label: String,
    pub status: String,
    pub metrics: serde_json::Value,
}

/// Agent 本体清单（用于新大脑 <10ms 具身着陆与多本体联邦名片交换）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyManifest {
    pub body_id: String,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub version: String,
    pub organs: Vec<OrganStatus>,
    pub active_checkpoints: usize,
    pub non_goals: Vec<String>,
    pub generated_at: String,
}

pub struct BodyManifestEngine;

impl BodyManifestEngine {
    /// 纯索引快速构建本体身份证与六器官自省（禁止派生外部子进程或重扫磁盘）
    pub fn build(storage: &Storage) -> BodyManifest {
        let body_id = compute_body_id();
        let hostname = System::host_name().unwrap_or_else(|| "localhost".to_string());
        let generated_at = Local::now().format("%Y-%m-%dT%H:%M:%S+08:00").to_string();

        let fact_nodes = storage.list_all_fact_nodes(2000).unwrap_or_default();
        let fact_edges = storage.list_all_fact_edges(4000).unwrap_or_default();
        let assets = storage.list_assets(20000).unwrap_or_default();
        let projects_cnt = fact_nodes.iter().filter(|n| n.node_type == "project").count();
        let ports_cnt = fact_nodes.iter().filter(|n| n.node_type == "port").count();
        let tools_cnt = fact_nodes.iter().filter(|n| n.node_type == "tool").count();

        let memories = storage.list_all_active_memories(1000).unwrap_or_default();
        let active_cps = storage.list_body_checkpoints(Some("active"), 50).unwrap_or_default();
        let rules = storage.get_placement_rules().unwrap_or_default();
        let locks = storage.list_coordination_locks(None).unwrap_or_default();
        let audit_summary = storage.audit_domain_summary("24h").unwrap_or_default();
        let lease_evals = crate::audit::AuditEvolutionAnalyzer::collect_audit_lease_adjustments(storage);
        let downgraded_leases = lease_evals.values().filter(|v| v.should_downgrade).count();

        let organs = vec![
            OrganStatus {
                name: "sensory".to_string(),
                label: "感官与本体感觉 (Fact Graph & Assets)".to_string(),
                status: if fact_nodes.is_empty() && assets.is_empty() {
                    "cold".to_string()
                } else {
                    "ready".to_string()
                },
                metrics: serde_json::json!({
                    "fact_nodes": fact_nodes.len(),
                    "fact_edges": fact_edges.len(),
                    "assets": assets.len(),
                    "tools": tools_cnt,
                    "ports": ports_cnt,
                    "projects": projects_cnt,
                }),
            },
            OrganStatus {
                name: "cerebellum".to_string(),
                label: "小脑与条件反射 (AOPCC & Hooks)".to_string(),
                status: if downgraded_leases > 0 {
                    "guarded".to_string()
                } else {
                    "ready".to_string()
                },
                metrics: serde_json::json!({
                    "downgraded_leases_24h": downgraded_leases,
                    "policy_levels": ["L0_OBSERVE", "L1_AUTONOMIC", "L2_SUPERVISED", "L3_FORBIDDEN"],
                }),
            },
            OrganStatus {
                name: "hippocampus".to_string(),
                label: "海马体与连续性 (Memory & ContinuityCapsule)".to_string(),
                status: "ready".to_string(),
                metrics: serde_json::json!({
                    "active_memories": memories.len(),
                    "active_checkpoints": active_cps.len(),
                }),
            },
            OrganStatus {
                name: "spine".to_string(),
                label: "规约脊柱 (Placement & AutonomyEnvelope)".to_string(),
                status: "ready".to_string(),
                metrics: serde_json::json!({
                    "placement_rules": rules.len(),
                    "redaction_enabled": true,
                    "anti_poisoning_enabled": true,
                }),
            },
            OrganStatus {
                name: "metabolism".to_string(),
                label: "代谢免疫与自稳态 (Dual-Domain Audit & Freshness)".to_string(),
                status: "ready".to_string(),
                metrics: serde_json::json!({
                    "audit_events_24h": audit_summary.total_events,
                    "in_agent_24h": audit_summary.in_agent_count,
                    "out_of_agent_24h": audit_summary.out_of_agent_count,
                }),
            },
            OrganStatus {
                name: "social".to_string(),
                label: "社会协同 (Coordination Locks & Mesh)".to_string(),
                status: "ready".to_string(),
                metrics: serde_json::json!({
                    "active_locks": locks.len(),
                    "mesh_protocol": "MeshEnvelope v1.0",
                }),
            },
        ];

        BodyManifest {
            body_id,
            hostname,
            os: crate::BUILD_OS.to_string(),
            arch: crate::BUILD_ARCH.to_string(),
            version: crate::VERSION.to_string(),
            organs,
            active_checkpoints: active_cps.len(),
            non_goals: vec![
                "不内置主战 LLM ReAct 规划循环，决策主权属于接入大脑".to_string(),
                "不替代多 Agent 编排框架 (LangGraph/CrewAI/AutoGen)".to_string(),
                "不替代容器沙箱 (Docker/gVisor) 或密钥保险库 (Vault)".to_string(),
            ],
            generated_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_body_id_deterministic_and_manifest_has_six_organs() {
        let id1 = compute_body_id();
        let id2 = compute_body_id();
        assert_eq!(id1, id2);
        assert!(id1.starts_with("body-"));
        assert_eq!(id1.len(), 21);

        let storage = Storage::open_in_memory().expect("open");
        let manifest = BodyManifestEngine::build(&storage);
        assert_eq!(manifest.body_id, id1);
        assert_eq!(manifest.organs.len(), 6);
        assert_eq!(manifest.non_goals.len(), 3);
    }
}
