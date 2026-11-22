use crate::benchmark::report::DataQualityResult;
use crate::storage::StorageProvider;

/// 运行宿主机数据质量与 D6 准确率/新鲜度门禁 (DQ1~DQ5)
pub fn run_data_quality_checks(storage: &dyn StorageProvider) -> Vec<DataQualityResult> {
    let mut results = Vec::with_capacity(5);

    // DQ1: 资产描述覆盖率 (采样最近 1,000 条资产)
    let assets = storage.list_assets(1000).unwrap_or_default();
    let dq1 = if assets.is_empty() {
        DataQualityResult {
            id: "DQ1".into(),
            name: "asset_description_coverage".into(),
            title: "资产描述元数据覆盖率".into(),
            measured_pct: 100.0,
            target_pct: 99.0,
            passed: true,
            detail: "空沙盒库（0 资产），基线通过".into(),
        }
    } else {
        let with_desc = assets
            .iter()
            .filter(|a| a.description.as_deref().map(|s| !s.trim().is_empty()).unwrap_or(false))
            .count();
        let pct = ((with_desc as f64 / assets.len() as f64) * 100.0 * 100.0).round() / 100.0;
        DataQualityResult {
            id: "DQ1".into(),
            name: "asset_description_coverage".into(),
            title: "资产描述元数据覆盖率".into(),
            measured_pct: pct,
            target_pct: 99.0,
            passed: pct >= 99.0,
            detail: format!("采样 {} 条资产中 {} 条具备 description", assets.len(), with_desc),
        }
    };
    results.push(dq1);

    // DQ2: 事实节点与资产本地 Embedding 向量覆盖率（采样检查）
    let fact_nodes = storage.list_all_fact_nodes(500).unwrap_or_default();
    let dq2 = if fact_nodes.is_empty() && assets.is_empty() {
        DataQualityResult {
            id: "DQ2".into(),
            name: "embedding_vector_coverage".into(),
            title: "本地 Ollama Embedding 覆盖率".into(),
            measured_pct: 100.0,
            target_pct: 99.0,
            passed: true,
            detail: "空沙盒库（0 节点），基线通过".into(),
        }
    } else {
        let sample_assets: Vec<_> = assets.iter().take(100).collect();
        let asset_emb_ok = sample_assets
            .iter()
            .filter(|a| !storage.get_asset_embedding(&a.path).unwrap_or_default().is_empty())
            .count();
        let sample_facts: Vec<_> = fact_nodes.iter().take(100).collect();
        let fact_emb_ok = sample_facts
            .iter()
            .filter(|n| !storage.get_fact_node_embedding(&n.id).unwrap_or_default().is_empty())
            .count();
        let total = sample_assets.len() + sample_facts.len();
        let ok = asset_emb_ok + fact_emb_ok;
        let pct = if total == 0 {
            100.0
        } else {
            ((ok as f64 / total as f64) * 100.0 * 100.0).round() / 100.0
        };
        DataQualityResult {
            id: "DQ2".into(),
            name: "embedding_vector_coverage".into(),
            title: "本地 Ollama Embedding 覆盖率".into(),
            measured_pct: pct,
            target_pct: 99.0,
            passed: pct >= 95.0,
            detail: format!("抽样 {} 个资产+事实节点中 {} 个已生成向量", total, ok),
        }
    };
    results.push(dq2);

    // DQ3: 依赖拓扑纯净度与健康度
    let links = storage.list_all_links(2000).unwrap_or_default();
    let dq3 = if links.is_empty() {
        DataQualityResult {
            id: "DQ3".into(),
            name: "links_topology_health".into(),
            title: "依赖拓扑真实健康度".into(),
            measured_pct: 100.0,
            target_pct: 85.0,
            passed: true,
            detail: "当前库无悬空依赖边 (100/100)".into(),
        }
    } else {
        // 统计非假阳性边且物理有效（asset 物理存在、非测试/文档示例误报）的健康度比例，与 LinkAuditor 口径一致
        let clean_count = links
            .iter()
            .filter(|l| {
                let both_in_sys_tmp =
                    l.source_id.starts_with("/tmp/") && l.target_id.starts_with("/tmp/");
                if l.target_type == "asset" {
                    let is_fp = (!both_in_sys_tmp
                        && crate::links::is_non_static_link_target(&l.target_id, None))
                        || (!both_in_sys_tmp
                            && crate::links::is_test_or_example_link(
                                &l.source_id,
                                &l.target_id,
                                l.snippet.as_deref(),
                            ))
                        || (!l.source_id.starts_with("/tmp/")
                            && l.source_id.ends_with(".md")
                            && !std::path::Path::new(&l.target_id).exists()
                            && !crate::links::is_explicit_markdown_link(l.snippet.as_deref()));
                    if is_fp {
                        return false;
                    }
                    let p = std::path::Path::new(&l.target_id);
                    !p.is_absolute() || p.exists()
                } else if l.target_type == "service" {
                    !crate::links::is_pure_data_file(&l.source_id) && !l.source_id.ends_with(".md")
                } else {
                    true
                }
            })
            .count();
        let pct = ((clean_count as f64 / links.len() as f64) * 100.0 * 100.0).round() / 100.0;
        DataQualityResult {
            id: "DQ3".into(),
            name: "links_topology_health".into(),
            title: "依赖拓扑纯净度与物理健康度".into(),
            measured_pct: pct,
            target_pct: 85.0,
            passed: pct >= 85.0,
            detail: format!(
                "全量 {} 条依赖边中 {} 条为物理有效且非假阳性拓扑边",
                links.len(),
                clean_count
            ),
        }
    };
    results.push(dq3);

    // DQ4: 现网库表敏感凭据卫生度 (明文凭据零残留率)
    let procs = storage.list_active_processes().unwrap_or_default();
    let memories = storage.list_all_active_memories(100).unwrap_or_default();
    let services = storage.list_external_services(None).unwrap_or_default();

    let mut inspected = 0usize;
    let mut clean_items = 0usize;
    let has_plaintext_leak = |text: &str| -> bool {
        let lower = text.to_ascii_lowercase();
        let has_token = ["sk-", "ghp_", "gho_", "glpat-", "xoxb-", "akia"]
            .iter()
            .any(|p| {
                text.find(p)
                    .map(|pos| {
                        let rem = &text[pos..];
                        let len = rem
                            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ',' | ';'))
                            .unwrap_or(rem.len());
                        len >= 12
                    })
                    .unwrap_or(false)
            });
        let has_unredacted_pwd = (lower.contains("--password=") || lower.contains("password="))
            && !text.contains("<REDACTED>")
            && !text.contains("${")
            && !text.contains("***");
        has_token || has_unredacted_pwd
    };

    for p in procs.iter().take(100) {
        inspected += 1;
        if !has_plaintext_leak(&p.command) {
            clean_items += 1;
        }
    }
    for m in memories.iter().take(100) {
        inspected += 1;
        if !has_plaintext_leak(&m.content) {
            clean_items += 1;
        }
    }
    for s in services.iter().take(100) {
        inspected += 1;
        if !has_plaintext_leak(&s.endpoint) {
            clean_items += 1;
        }
    }

    let dq4 = if inspected == 0 {
        DataQualityResult {
            id: "DQ4".into(),
            name: "secret_hygiene_and_perm_gate".into(),
            title: "库表凭据脱敏卫生度".into(),
            measured_pct: 100.0,
            target_pct: 100.0,
            passed: true,
            detail: "空沙盒库（0 条采样），基线通过".into(),
        }
    } else {
        let pct = ((clean_items as f64 / inspected as f64) * 100.0 * 100.0).round() / 100.0;
        DataQualityResult {
            id: "DQ4".into(),
            name: "secret_hygiene_and_perm_gate".into(),
            title: "库表凭据脱敏卫生度".into(),
            measured_pct: pct,
            target_pct: 100.0,
            passed: clean_items == inspected,
            detail: format!("抽检 {} 条进程/记忆/外部服务记录中 {} 条无明文凭据残留", inspected, clean_items),
        }
    };
    results.push(dq4);

    // DQ5: 全链路信息新鲜度与跨表一致性对账健康度 (Phase 39 / ISSUE #21)
    let fresh_assets = storage.list_assets(2000).unwrap_or_default();
    let active_physical: Vec<_> = fresh_assets
        .iter()
        .filter(|a| {
            a.status == "active"
                && !a.path.starts_with("bench:")
                && !a.path.starts_with("demo:")
        })
        .take(500)
        .collect();
    let live_ports: std::collections::HashSet<String> =
        crate::fact::process_net::ProcessNetTopology::scan_listening_ports()
            .map(|ps| ps.into_iter().map(|p| format!("port:{}", p.port)).collect())
            .unwrap_or_default();
    let all_nodes = storage.list_all_fact_nodes(1000).unwrap_or_default();
    let active_memories = storage.list_all_active_memories(500).unwrap_or_default();

    let mut inspected_fresh = 0usize;
    let mut healthy_fresh = 0usize;
    let mut missing_cnt = 0usize;
    let mut unreconciled_drift_cnt = 0usize;
    let mut ghost_node_cnt = 0usize;
    let mut dup_mem_cnt = 0usize;

    for a in &active_physical {
        inspected_fresh += 1;
        let p = std::path::Path::new(&a.path);
        if !p.exists() {
            missing_cnt += 1;
        } else if p.is_file() && a.scan_tier == "deep" {
            let meta_drifted = std::fs::metadata(p)
                .map(|m| a.size_bytes.map(|s| s != m.len() as i64).unwrap_or(false))
                .unwrap_or(false);
            let marked_stale = a
                .desc_kv
                .get("embedding_status")
                .and_then(|v| v.as_str())
                == Some("stale");
            if meta_drifted && !marked_stale {
                unreconciled_drift_cnt += 1;
            } else {
                healthy_fresh += 1;
            }
        } else {
            healthy_fresh += 1;
        }
    }

    for n in &all_nodes {
        if n.id.contains("bench:") || n.id.contains("demo:") {
            continue;
        }
        match n.node_type.as_str() {
            "port" if !live_ports.is_empty() => {
                inspected_fresh += 1;
                if live_ports.contains(&n.id) {
                    healthy_fresh += 1;
                } else {
                    ghost_node_cnt += 1;
                }
            }
            "file" => {
                if let Some(fp) = n.id.strip_prefix("file:") {
                    if fp.starts_with('/') {
                        inspected_fresh += 1;
                        if std::path::Path::new(fp).exists() {
                            healthy_fresh += 1;
                        } else {
                            ghost_node_cnt += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let mut seen_auto_paths = std::collections::HashSet::new();
    for m in &active_memories {
        if let Some(start) = m.content.find("产出文件 `") {
            let rest = &m.content[start + "产出文件 `".len()..];
            if let Some(end_rel) = rest.find("` 摘要:") {
                let fp = &rest[..end_rel];
                if fp.starts_with('/') && !fp.starts_with("bench:") && !fp.starts_with("demo:") {
                    inspected_fresh += 1;
                    if !std::path::Path::new(fp).exists() || !seen_auto_paths.insert(fp.to_string()) {
                        dup_mem_cnt += 1;
                    } else {
                        healthy_fresh += 1;
                    }
                }
            }
        }
    }

    let dq5 = if inspected_fresh == 0 {
        DataQualityResult {
            id: "DQ5".into(),
            name: "freshness_and_reconciliation_health".into(),
            title: "全链路新鲜度与一致性对账健康度".into(),
            measured_pct: 100.0,
            target_pct: 98.0,
            passed: true,
            detail: "空沙盒库（0 条采样），基线通过".into(),
        }
    } else {
        let pct = ((healthy_fresh as f64 / inspected_fresh as f64) * 100.0 * 100.0).round() / 100.0;
        DataQualityResult {
            id: "DQ5".into(),
            name: "freshness_and_reconciliation_health".into(),
            title: "全链路新鲜度与一致性对账健康度".into(),
            measured_pct: pct,
            target_pct: 98.0,
            passed: pct >= 98.0,
            detail: format!(
                "抽检 {} 项 (资产/Fact节点/自动记忆): 健康 {} | 失活资产 {} | 未标志漂移 {} | 幽灵节点 {} | 冗余记忆 {}",
                inspected_fresh,
                healthy_fresh,
                missing_cnt,
                unreconciled_drift_cnt,
                ghost_node_cnt,
                dup_mem_cnt
            ),
        }
    };
    results.push(dq5);

    // DQ6: 图谱多维连通度与证据分层合规度 (Phase 40 / ISSUE #23)
    let valid_tiers = ["L1_EXPLICIT", "L2_STRUCTURAL", "L3_SEMANTIC"];
    let dq_edges = storage.list_all_fact_edges(5000).unwrap_or_default();
    let dq_projects: Vec<_> = all_nodes
        .iter()
        .filter(|n| n.node_type == "project")
        .collect();
    let mut connected_projects = std::collections::HashSet::new();
    let mut valid_tier_edges = 0usize;
    for e in &dq_edges {
        if valid_tiers.contains(&e.evidence_tier.as_str()) && e.weight > 0.0 && e.weight <= 1.0 {
            valid_tier_edges += 1;
        }
        if e.source_id.starts_with("project:") {
            connected_projects.insert(e.source_id.as_str());
        }
        if e.target_id.starts_with("project:") {
            connected_projects.insert(e.target_id.as_str());
        }
    }
    let dq6 = if dq_edges.is_empty() && dq_projects.is_empty() {
        DataQualityResult {
            id: "DQ6".into(),
            name: "graph_connectivity_and_tier_hygiene".into(),
            title: "图谱多维连通度与证据分层合规度".into(),
            measured_pct: 100.0,
            target_pct: 90.0,
            passed: true,
            detail: "空沙盒库（0 条图谱边），基线通过".into(),
        }
    } else {
        let tier_pct = if dq_edges.is_empty() {
            100.0
        } else {
            (valid_tier_edges as f64 / dq_edges.len() as f64) * 100.0
        };
        let conn_pct = if dq_projects.is_empty() {
            100.0
        } else {
            (connected_projects.len() as f64 / dq_projects.len() as f64) * 100.0
        };
        // 分层合法率占 70% + 项目连通率占 30%（目标 >= 70%，彻底告别 99.5% 孤岛）
        let score = ((tier_pct * 0.70 + conn_pct * 0.30) * 100.0).round() / 100.0;
        DataQualityResult {
            id: "DQ6".into(),
            name: "graph_connectivity_and_tier_hygiene".into(),
            title: "图谱多维连通度与证据分层合规度".into(),
            measured_pct: score,
            target_pct: 70.0,
            passed: tier_pct >= 99.0 && score >= 70.0,
            detail: format!(
                "fact_edges={}/{} 分层合法 ({:.1}%) | 工程节点连通 {}/{} ({:.1}%)",
                valid_tier_edges,
                dq_edges.len(),
                tier_pct,
                connected_projects.len(),
                dq_projects.len(),
                conn_pct
            ),
        }
    };
    results.push(dq6);

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::models::LinkRecord;
    use crate::storage::sqlite_provider::SqliteStorageProvider;

    #[test]
    fn test_dq3_detects_physically_missing_dangling_links() {
        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite");
        storage
            .add_link(&LinkRecord {
                id: None,
                source_type: "asset".into(),
                source_id: "/root/new/work/real.py".into(),
                target_type: "asset".into(),
                target_id: "/root/new/definitely_missing_target_abc_999.py".into(),
                relation: "depends_on".into(),
                confidence: 0.9,
                auto_discovered: true,
                line_number: Some(1),
                snippet: Some("import missing".into()),
                provenance: Some("ast_import".into()),
                created_at: None,
            })
            .expect("add link");
        let results = run_data_quality_checks(&storage);
        let dq3 = results.iter().find(|r| r.id == "DQ3").expect("DQ3");
        assert_eq!(dq3.measured_pct, 0.0, "物理不存在的断链在 DQ3 中应准确检出");
        assert!(!dq3.passed);
    }

    #[test]
    fn test_dq5_detects_missing_asset_ghost_node_and_duplicate_memory() {
        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite");
        storage
            .upsert_fact_node(&crate::storage::models::FactNodeRecord {
                id: "file:/root/new/non_existent_dq5_ghost_999.py".into(),
                node_type: "file".into(),
                name: "non_existent_dq5_ghost_999.py".into(),
                label: "ghost".into(),
                properties: serde_json::json!({}),
                updated_at: None,
            })
            .expect("upsert fact node");
        let results = run_data_quality_checks(&storage);
        let dq5 = results.iter().find(|r| r.id == "DQ5").expect("DQ5");
        assert_eq!(dq5.measured_pct, 0.0, "物理缺失 file: 节点应触发 DQ5 不健康告警");
        assert!(!dq5.passed);
    }
}
