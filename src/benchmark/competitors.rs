use std::sync::Arc;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::storage::StorageProvider;
use super::competitor_tracks::{
    run_comp_c1_memory, run_comp_c2_context_db, run_comp_c3_agentfs, run_comp_c4_host_cli,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitorComparisonResult {
    pub id: String,
    pub track: String,
    pub title: String,
    pub competitor_name: String,
    pub evidence_tier: String,
    pub competitor_elapsed_us: u128,
    pub body_elapsed_us: u128,
    pub speedup_factor: f64,
    pub competitor_tokens: i64,
    pub body_tokens: i64,
    pub token_savings_pct: f64,
    pub competitor_metric_label: String,
    pub competitor_metric_value: String,
    pub body_metric_label: String,
    pub body_metric_value: String,
    pub body_advantage: String,
    pub complement_note: String,
    pub passed: bool,
    #[serde(default)]
    pub competitor_quality_score: f64,
    #[serde(default)]
    pub body_quality_score: f64,
    #[serde(default)]
    pub quality_delta: f64,
    #[serde(default)]
    pub competitor_facts_hit: usize,
    #[serde(default)]
    pub body_facts_hit: usize,
    #[serde(default)]
    pub required_facts_total: usize,
    #[serde(default)]
    pub info_density_gain: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetitorCapabilityRow {
    pub product: String,
    pub track: String,
    pub evidence_tier: String,
    pub source_ref: String,
    pub host_reality_probe: bool,
    pub in_place_zero_copy: bool,
    pub noise_gate_tiering: bool,
    pub token_budget_routing: bool,
    pub persistent_memory: bool,
    pub secret_redaction: bool,
    pub anti_poisoning_guard: bool,
    pub multi_agent_wfg_lock: bool,
    pub macro_3hop_graph: bool,
    pub micro_ast_bridge: bool,
    pub single_binary_offline: bool,
    pub relationship: String,
}

/// 基于本机 /root/new/src/ 真实克隆仓库核验的 12 维竞品能力对标矩阵 (ARCH_VERIFIED)
pub fn build_competitor_capability_matrix() -> Vec<CompetitorCapabilityRow> {
    vec![
        CompetitorCapabilityRow {
            product: "Agent Body (本系统)".into(),
            track: "All-in-One 宿主认知底座".into(),
            evidence_tier: "LIVE_BENCHMARKED".into(),
            source_ref: "src/lib.rs (25 modules, 41 MCP tools)".into(),
            host_reality_probe: true,
            in_place_zero_copy: true,
            noise_gate_tiering: true,
            token_budget_routing: true,
            persistent_memory: true,
            secret_redaction: true,
            anti_poisoning_guard: true,
            multi_agent_wfg_lock: true,
            macro_3hop_graph: true,
            micro_ast_bridge: true,
            single_binary_offline: true,
            relationship: "核心宿主认知底座 (单二进制零云端Token)".into(),
        },
        CompetitorCapabilityRow {
            product: "Mem0 (mem0ai/mem0)".into(),
            track: "C1 · Agent 长期记忆".into(),
            evidence_tier: "ARCH_VERIFIED".into(),
            source_ref: "/root/new/src/mem0/README.md".into(),
            host_reality_probe: false,
            in_place_zero_copy: false,
            noise_gate_tiering: false,
            token_budget_routing: false,
            persistent_memory: true,
            secret_redaction: false,
            anti_poisoning_guard: false,
            multi_agent_wfg_lock: false,
            macro_3hop_graph: false,
            micro_ast_bridge: false,
            single_binary_offline: false,
            relationship: "互补生态 (MemoryProvider SPI 可一行外挂 Mem0Adapter)".into(),
        },
        CompetitorCapabilityRow {
            product: "Letta / MemGPT".into(),
            track: "C1 · Agent 长期记忆".into(),
            evidence_tier: "ARCH_VERIFIED".into(),
            source_ref: "/root/new/src/letta/README.md".into(),
            host_reality_probe: false,
            in_place_zero_copy: false,
            noise_gate_tiering: false,
            token_budget_routing: false,
            persistent_memory: true,
            secret_redaction: false,
            anti_poisoning_guard: false,
            multi_agent_wfg_lock: false,
            macro_3hop_graph: false,
            micro_ast_bridge: false,
            single_binary_offline: false,
            relationship: "互补生态 (MemoryProvider SPI 可一行外挂 LettaAdapter)".into(),
        },
        CompetitorCapabilityRow {
            product: "PowerContext (OceanBase)".into(),
            track: "C1 · 会话交接与记忆".into(),
            evidence_tier: "ARCH_VERIFIED".into(),
            source_ref: "/root/new/src/powercontext/README_CN.md".into(),
            host_reality_probe: false,
            in_place_zero_copy: false,
            noise_gate_tiering: false,
            token_budget_routing: false,
            persistent_memory: true,
            secret_redaction: false,
            anti_poisoning_guard: false,
            multi_agent_wfg_lock: false,
            macro_3hop_graph: false,
            micro_ast_bridge: false,
            single_binary_offline: false,
            relationship: "互补生态 (MemoryProvider SPI 可外挂 PowerContextAdapter)".into(),
        },
        CompetitorCapabilityRow {
            product: "Shodh-Memory".into(),
            track: "C1 · Rust 嵌入式记忆".into(),
            evidence_tier: "ARCH_VERIFIED".into(),
            source_ref: "/root/new/src/shodh-memory/Cargo.toml".into(),
            host_reality_probe: false,
            in_place_zero_copy: false,
            noise_gate_tiering: false,
            token_budget_routing: false,
            persistent_memory: true,
            secret_redaction: false,
            anti_poisoning_guard: false,
            multi_agent_wfg_lock: false,
            macro_3hop_graph: false,
            micro_ast_bridge: false,
            single_binary_offline: true,
            relationship: "同语言局部竞品 (聚焦认知记忆，无宿主机环境/资产/锁感知)".into(),
        },
        CompetitorCapabilityRow {
            product: "OpenViking (ByteDance)".into(),
            track: "C2 · 上下文数据库".into(),
            evidence_tier: "LIVE_BENCHMARKED".into(),
            source_ref: "/root/new/src/OpenViking/README.md (viking:// L0/L1/L2)".into(),
            host_reality_probe: false,
            in_place_zero_copy: false,
            noise_gate_tiering: false,
            token_budget_routing: true,
            persistent_memory: true,
            secret_redaction: false,
            anti_poisoning_guard: false,
            multi_agent_wfg_lock: false,
            macro_3hop_graph: false,
            micro_ast_bridge: false,
            single_binary_offline: false,
            relationship: "C2 对标 (OpenViking 需导入 viking:// 树；Agent Body 原位零搬迁)".into(),
        },
        CompetitorCapabilityRow {
            product: "AgentFS (Turso)".into(),
            track: "C3 · Agent 文件系统".into(),
            evidence_tier: "LIVE_BENCHMARKED".into(),
            source_ref: "/root/new/src/agentfs/SPEC.md (fs_inode/fs_dentry/fs_data/tool_calls)".into(),
            host_reality_probe: false,
            in_place_zero_copy: false,
            noise_gate_tiering: false,
            token_budget_routing: false,
            persistent_memory: false,
            secret_redaction: false,
            anti_poisoning_guard: false,
            multi_agent_wfg_lock: false,
            macro_3hop_graph: false,
            micro_ast_bridge: false,
            single_binary_offline: true,
            relationship: "C3 对标 (AgentFS 管虚拟块审计；Agent Body 管原生 FS 阻断与协作锁)".into(),
        },
        CompetitorCapabilityRow {
            product: "rg + psutil + ss + git".into(),
            track: "C4 · 宿主 CLI 探针组合".into(),
            evidence_tier: "LIVE_BENCHMARKED".into(),
            source_ref: "/usr/bin/rg, python3 psutil, /usr/bin/ss".into(),
            host_reality_probe: true,
            in_place_zero_copy: true,
            noise_gate_tiering: false,
            token_budget_routing: false,
            persistent_memory: false,
            secret_redaction: false,
            anti_poisoning_guard: false,
            multi_agent_wfg_lock: false,
            macro_3hop_graph: false,
            micro_ast_bridge: false,
            single_binary_offline: true,
            relationship: "C4 基线对照 (Agent Body 将 6 步探针融合为 <10ms 索引简报)".into(),
        },
        CompetitorCapabilityRow {
            product: "CodeGraph".into(),
            track: "C4 · 微观 AST 符号图".into(),
            evidence_tier: "ARCH_VERIFIED".into(),
            source_ref: "src/links/codegraph_bridge.rs".into(),
            host_reality_probe: false,
            in_place_zero_copy: true,
            noise_gate_tiering: false,
            token_budget_routing: false,
            persistent_memory: false,
            secret_redaction: false,
            anti_poisoning_guard: false,
            multi_agent_wfg_lock: false,
            macro_3hop_graph: false,
            micro_ast_bridge: true,
            single_binary_offline: true,
            relationship: "战略联邦伙伴 (Agent Body 管宏观拓扑，CodeGraph 管微观符号)".into(),
        },
    ]
}

pub fn run_all_competitor_benchmarks(
    storage: Arc<dyn StorageProvider>,
) -> Result<Vec<CompetitorComparisonResult>> {
    Ok(vec![
        run_comp_c1_memory()?,
        run_comp_c2_context_db(storage.clone())?,
        run_comp_c3_agentfs()?,
        run_comp_c4_host_cli(storage)?,
    ])
}

pub fn run_competitor_track(
    storage: Arc<dyn StorageProvider>,
    track: &str,
) -> Result<Vec<CompetitorComparisonResult>> {
    let norm = track.trim().to_ascii_uppercase();
    match norm.as_str() {
        "" | "ALL" | "COMPETITORS" => run_all_competitor_benchmarks(storage),
        "C1" | "COMP-C1" => Ok(vec![run_comp_c1_memory()?]),
        "C2" | "COMP-C2" => Ok(vec![run_comp_c2_context_db(storage)?]),
        "C3" | "COMP-C3" => Ok(vec![run_comp_c3_agentfs()?]),
        "C4" | "COMP-C4" => Ok(vec![run_comp_c4_host_cli(storage)?]),
        other => anyhow::bail!("未知的竞品对标赛道: {} (支持 all | C1 | C2 | C3 | C4)", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SqliteStorageProvider;

    #[test]
    fn test_competitor_benchmarks_all_tracks() {
        let storage = Arc::new(SqliteStorageProvider::open_in_memory().expect("sqlite"));
        let results = run_all_competitor_benchmarks(storage).expect("run competitors");
        assert_eq!(results.len(), 4);
        for r in &results {
            assert_eq!(r.evidence_tier, "LIVE_BENCHMARKED");
            assert!(r.passed, "competitor track {} should pass: {:?}", r.id, r);
        }
        let matrix = build_competitor_capability_matrix();
        assert!(matrix.len() >= 9);
    }
}
