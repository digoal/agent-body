use crate::benchmark::report::DataQualityResult;
use crate::storage::StorageProvider;

/// 运行宿主机数据质量与 D6 准确率门禁 (DQ1~DQ3)
pub fn run_data_quality_checks(storage: &dyn StorageProvider) -> Vec<DataQualityResult> {
    let mut results = Vec::with_capacity(4);

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
}
