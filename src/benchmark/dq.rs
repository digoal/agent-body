use crate::benchmark::report::DataQualityResult;
use crate::storage::StorageProvider;

/// 运行宿主机数据质量与 D6/D7 准确率/新鲜度/图谱/遥测/睡眠整理/快权重健康门禁 (DQ1~DQ12)
pub fn run_data_quality_checks(storage: &dyn StorageProvider) -> Vec<DataQualityResult> {
    let mut results = Vec::with_capacity(11);

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
    const DQ2_TARGET_PCT: f64 = 95.0;
    let dq2 = if fact_nodes.is_empty() && assets.is_empty() {
        DataQualityResult {
            id: "DQ2".into(),
            name: "embedding_vector_coverage".into(),
            title: "本地 Ollama Embedding 覆盖率".into(),
            measured_pct: 100.0,
            target_pct: DQ2_TARGET_PCT,
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
            target_pct: DQ2_TARGET_PCT,
            passed: pct >= DQ2_TARGET_PCT,
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

    // DQ6: 图谱多维连通度与证据分层合规度 (Phase 40 / ISSUE #23 + Phase 61~62 Know-How 扩展)
    let valid_tiers = [
        "L1_EXPLICIT",
        "L2_STRUCTURAL",
        "L3_SEMANTIC",
        "L1_KNOWLEDGE",
        "L2_KNOWLEDGE_PMI",
        "L2_KNOWHOW",
    ];
    let dq_edges = storage.list_all_fact_edges(5000).unwrap_or_default();
    let dq_projects: Vec<_> = all_nodes
        .iter()
        .filter(|n| n.node_type == "project")
        .collect();
    let mut connected_projects: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut valid_tier_edges = 0usize;
    for e in &dq_edges {
        if valid_tiers.contains(&e.evidence_tier.as_str()) && e.weight > 0.0 && e.weight <= 1.0 {
            valid_tier_edges += 1;
        }
        if e.source_id.starts_with("project:") {
            connected_projects.insert(e.source_id.clone());
        }
        if e.target_id.starts_with("project:") {
            connected_projects.insert(e.target_id.clone());
        }
    }
    // 知识网络扩边后 fact_edges 可达十万级，list_all_fact_edges(5000) 会截断漏掉工程边；
    // 对尚未出现在采样边中的 project 节点走索引 1-hop 定向核查，消除假孤岛。
    for p in &dq_projects {
        if !connected_projects.contains(&p.id) {
            if let Ok(sg) = storage.traverse_fact_graph(&p.id, 1) {
                if !sg.edges.is_empty() {
                    connected_projects.insert(p.id.clone());
                }
            }
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

    // DQ7: 双域行为审计完整性、脱敏合规与 SHA-256 存证门禁 (Phase 43 / ISSUE #26)
    let dq_audits = storage
        .query_audit_events(&crate::storage::models::AuditQueryFilter {
            limit: 500,
            ..Default::default()
        })
        .unwrap_or_default();
    let dq7 = if dq_audits.is_empty() {
        DataQualityResult {
            id: "DQ7".into(),
            name: "audit_completeness_and_redaction_hygiene".into(),
            title: "双域行为审计完整性与脱敏卫生门禁".into(),
            measured_pct: 100.0,
            target_pct: 99.0,
            passed: true,
            detail: "空沙盒库（0 条审计流水），基线通过".into(),
        }
    } else {
        let mut clean_cnt = 0usize;
        for ev in &dq_audits {
            let valid_domain = ev.domain == "IN_AGENT" || ev.domain == "OUT_OF_AGENT";
            let valid_sha = ev.evidence_sha256.len() == 64;
            let delta_str = ev.before_after_delta.to_string();
            let has_leak = delta_str.contains("sk-")
                || delta_str.contains("ghp_")
                || delta_str.contains("Bearer eyJ");
            if valid_domain && valid_sha && !has_leak {
                clean_cnt += 1;
            }
        }
        let pct = ((clean_cnt as f64 / dq_audits.len() as f64) * 10000.0).round() / 100.0;
        DataQualityResult {
            id: "DQ7".into(),
            name: "audit_completeness_and_redaction_hygiene".into(),
            title: "双域行为审计完整性与脱敏卫生门禁".into(),
            measured_pct: pct,
            target_pct: 99.0,
            passed: pct >= 99.0,
            detail: format!(
                "抽检 {} 条 audit_events：{}/{} SHA-256 完整且零明文凭据泄露 ({:.1}%)",
                dq_audits.len(),
                clean_cnt,
                dq_audits.len(),
                pct
            ),
        }
    };
    results.push(dq7);

    // DQ8: 本体五维稳态健康度与自治包络合规门禁 (Phase 44 / ISSUE #27)
    let vitals = crate::body::HomeostasisVitalsEngine::assess_provider(storage);
    let l3_chk = crate::body::AutonomyEnvelope::evaluate("write_file", Some("/etc/shadow"), false);
    let cps = storage.list_body_checkpoints(None, 200).unwrap_or_default();
    let dims_valid = vitals.dimensions.len() == 5
        && vitals
            .dimensions
            .iter()
            .all(|d| (0.0..=100.0).contains(&d.score));
    let cps_clean = cps.iter().all(|c| {
        let txt = format!("{} {}", c.goal_summary, c.next_step_hint);
        let no_leak = !txt.contains("sk-") && !txt.contains("ghp_") && !txt.contains("Bearer eyJ");
        no_leak && c.token_budget <= 300
    });
    let envelope_ok = l3_chk.level == crate::body::AutonomyLevel::L3Forbidden && !l3_chk.allowed;
    let dq8_pct = if dims_valid && cps_clean && envelope_ok {
        100.0
    } else if envelope_ok && dims_valid {
        70.0
    } else {
        0.0
    };
    results.push(DataQualityResult {
        id: "DQ8".into(),
        name: "body_homeostasis_and_envelope_hygiene".into(),
        title: "本体五维稳态健康度与自治包络合规门禁".into(),
        measured_pct: dq8_pct,
        target_pct: 99.0,
        passed: dq8_pct >= 99.0,
        detail: format!(
            "稳态分={:.1}/100 ({}) | 5维合法={} | 抽检 {} 个胶囊脱敏预算合规={} | L3拦截={}",
            vitals.overall_score,
            vitals.status,
            dims_valid,
            cps.len(),
            cps_clean,
            envelope_ok
        ),
    });

    // DQ9: 内生 WaitEvent 遥测、分位单调性、过期锁 SPI 与 AWR 仓库索引一致性门禁 (Phase 54)
    let wait_rows = crate::body::WaitEventRegistry::snapshot_all();
    let all_events_present = wait_rows.len() == crate::body::perf_awr::WAIT_EVENT_COUNT;
    let p95_monotonic = wait_rows
        .iter()
        .all(|r| r.calls == 0 || r.est_p95_us <= r.max_us);
    let expired_spi_ok = storage.count_expired_coordination_locks().is_ok();
    let (snap_ids, _) = crate::body::BodyPerfEngine::list_snapshot_ids(storage);
    let report_metas = crate::body::BodyPerfEngine::list_awr_reports_meta(storage);
    let snaps_loadable = snap_ids
        .iter()
        .all(|id| crate::body::BodyPerfEngine::load_snapshot(storage, id).is_some());
    let reports_loadable = report_metas
        .iter()
        .all(|r| crate::body::BodyPerfEngine::load_awr_report(storage, &r.report_id).is_some());
    let awr_repo_consistent = snaps_loadable && reports_loadable;

    let checks_ok = [
        all_events_present,
        p95_monotonic,
        expired_spi_ok,
        awr_repo_consistent,
    ]
    .into_iter()
    .filter(|ok| *ok)
    .count();
    let dq9_pct = ((checks_ok as f64 / 4.0) * 100.0 * 100.0).round() / 100.0;
    results.push(DataQualityResult {
        id: "DQ9".into(),
        name: "telemetry_awr_and_storage_integrity".into(),
        title: "内生遥测、AWR 报告库与锁状态一致性门禁".into(),
        measured_pct: dq9_pct,
        target_pct: 100.0,
        passed: checks_ok == 4,
        detail: format!(
            "WaitEvents={}/{} | P95<=Max={} | ExpiredLockSPI={} | AWR仓库索引一致={} (snaps={}, reports={})",
            wait_rows.len(),
            crate::body::perf_awr::WAIT_EVENT_COUNT,
            p95_monotonic,
            expired_spi_ok,
            awr_repo_consistent,
            snap_ids.len(),
            report_metas.len()
        ),
    });

    // DQ10: 知识网络防毛线球度数上限 (co_occurs_with <= 32)、L2 Know-How 原文锚定与零缺失资产幽灵节点 (Phase 62)
    let all_kg_nodes = storage.list_all_fact_nodes(5000).unwrap_or_default();
    let all_kg_edges = storage.list_all_fact_edges(8000).unwrap_or_default();
    let mut co_degree_map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for e in &all_kg_edges {
        if e.relation == "co_occurs_with" {
            *co_degree_map.entry(e.source_id.clone()).or_insert(0) += 1;
            *co_degree_map.entry(e.target_id.clone()).or_insert(0) += 1;
        }
    }
    let max_co_deg = co_degree_map.values().copied().max().unwrap_or(0);
    let hairball_violations = co_degree_map.values().filter(|&&d| d > 32).count();

    let all_assets_status = storage.list_assets(5000).unwrap_or_default();
    let missing_paths: std::collections::HashSet<&str> = all_assets_status
        .iter()
        .filter(|a| a.status == "missing")
        .map(|a| a.path.as_str())
        .collect();

    let mut knowhow_extracted_total = 0usize;
    let mut ungrounded_knowhow = 0usize;
    let mut ghost_derived_nodes = 0usize;
    for n in &all_kg_nodes {
        let src = n
            .properties
            .get("source_path")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if matches!(n.node_type.as_str(), "section" | "knowhow") && missing_paths.contains(src) {
            ghost_derived_nodes += 1;
        }
        if n.node_type == "knowhow"
            && n.properties
                .get("extractor")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.trim().is_empty())
        {
            knowhow_extracted_total += 1;
            let quote_ok = n
                .properties
                .get("evidence_quote")
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.trim().is_empty());
            if !quote_ok || src.trim().is_empty() {
                ungrounded_knowhow += 1;
            }
        }
    }

    let dq10_ok = [
        hairball_violations == 0,
        ungrounded_knowhow == 0,
        ghost_derived_nodes == 0,
    ]
    .into_iter()
    .filter(|ok| *ok)
    .count();
    let dq10_pct = ((dq10_ok as f64 / 3.0) * 100.0 * 100.0).round() / 100.0;
    results.push(DataQualityResult {
        id: "DQ10".into(),
        name: "knowledge_graph_anti_hairball_and_grounding".into(),
        title: "知识网络防毛线球度数、原文锚定与零幽灵章节门禁".into(),
        measured_pct: dq10_pct,
        target_pct: 100.0,
        passed: dq10_ok == 3,
        detail: format!(
            "max_co_deg={}/32 (violations={}) | knowhow_extracted={} (ungrounded={}) | ghost_derived={}",
            max_co_deg, hairball_violations, knowhow_extracted_total, ungrounded_knowhow, ghost_derived_nodes
        ),
    });

    // DQ11: 双时态睡眠整理摘要纯净度、旁路不变量与显著性分级门禁 (Phase 77 · M2)
    let all_mem_dq11 = storage.list_all_memories(1000).unwrap_or_default();
    let sleep_summaries: Vec<_> = all_mem_dq11
        .iter()
        .filter(|m| m.tags.iter().any(|t| t == "sleep_compacted" || t == "sleep_summary"))
        .collect();
    let sleep_clean_ok = sleep_summaries
        .iter()
        .all(|m| !m.content.trim().is_empty() && !has_plaintext_leak(&m.content));

    let sleep_audits = storage
        .query_audit_events(&crate::storage::models::AuditQueryFilter {
            domain: Some("OUT_OF_AGENT".to_string()),
            action: Some("sleep_compaction".to_string()),
            limit: 10,
            ..Default::default()
        })
        .unwrap_or_default();
    let audit_traceable_ok = sleep_summaries.is_empty()
        || sleep_audits
            .iter()
            .all(|a| !a.evidence_sha256.trim().is_empty())
        || sleep_summaries.iter().all(|m| m.source_session.is_some());

    let sal_h = ext_mem_pg_tx::SalienceScorer::score(&ext_mem_pg_tx::SalienceInput {
        content: "decision lock".to_string(),
        memory_type: "decision".to_string(),
        evidence_tier: "L1_EXPLICIT".to_string(),
        cross_session_reuses: 2,
        conflict_corrections: 1,
        novelty_ratio: 0.9,
        age_hours: 1.0,
        is_auto_sync: false,
        pinned: true,
    });
    let sal_l = ext_mem_pg_tx::SalienceScorer::score(&ext_mem_pg_tx::SalienceInput {
        content: "auto_sync noise".to_string(),
        memory_type: "note".to_string(),
        evidence_tier: "L3_SEMANTIC".to_string(),
        cross_session_reuses: 0,
        conflict_corrections: 0,
        novelty_ratio: 0.1,
        age_hours: 96.0,
        is_auto_sync: true,
        pinned: false,
    });
    let bypass_dec = ext_mem_pg_tx::QueryPredictabilityGate::evaluate(
        "PgTxEngine::search_raw_bypass SQLSTATE 40001",
        &["[sleep_summary:pg] primary port=5432"],
        None,
        false,
    );
    let invariants_ok = sal_h.score > sal_l.score && bypass_dec.bypass_compacted;

    let dq11_checks_ok = [sleep_clean_ok, audit_traceable_ok, invariants_ok]
        .into_iter()
        .filter(|ok| *ok)
        .count();
    let dq11_pct = ((dq11_checks_ok as f64 / 3.0) * 100.0 * 100.0).round() / 100.0;
    results.push(DataQualityResult {
        id: "DQ11".into(),
        name: "tx_bitemporal_and_sleep_purity".into(),
        title: "双时态睡眠整理纯净度、显著性分级与旁路不变量门禁".into(),
        measured_pct: dq11_pct,
        target_pct: 100.0,
        passed: dq11_checks_ok == 3,
        detail: format!(
            "sleep_summaries={} | clean={} | traceable={} | invariants_ok={}",
            sleep_summaries.len(),
            sleep_clean_ok,
            audit_traceable_ok,
            invariants_ok
        ),
    });

    // DQ12: 方案 B 快权重快照锚定、三门禁不变量与 URI 禁内联门禁 (Phase 78 · M3)
    let cps_dq12 = storage.list_body_checkpoints(None, 200).unwrap_or_default();
    let anchored_cps_ok = cps_dq12.iter().all(|cp| {
        if let Some(ref sid) = cp.fast_weight_snapshot_id {
            !sid.trim().is_empty()
                && cp
                    .fast_weight_base_model
                    .as_deref()
                    .map(str::trim)
                    .is_some_and(|m| !m.is_empty())
                && !cp.evidence_sha256.trim().is_empty()
        } else {
            true
        }
    });
    let fw_policy = ext_mem_pg_tx::FastWeightGatePolicy::default();
    let gate_invariants_ok = !ext_mem_pg_tx::evaluate_fast_weight_gates(
        &ext_mem_pg_tx::FastWeightGateMetrics {
            norm_ratio: 2.1,
            eval_glue_delta: 0.0,
            unlearn_leak_rate: 0.0,
        },
        &fw_policy,
    )
    .passed
        && !ext_mem_pg_tx::evaluate_fast_weight_gates(
            &ext_mem_pg_tx::FastWeightGateMetrics {
                norm_ratio: 1.05,
                eval_glue_delta: -0.06,
                unlearn_leak_rate: 0.0,
            },
            &fw_policy,
        )
        .passed
        && !ext_mem_pg_tx::evaluate_fast_weight_gates(
            &ext_mem_pg_tx::FastWeightGateMetrics {
                norm_ratio: 1.05,
                eval_glue_delta: 0.0,
                unlearn_leak_rate: 0.002,
            },
            &fw_policy,
        )
        .passed
        && ext_mem_pg_tx::validate_artifact_uri("data:application/octet-stream;base64,AAA").is_err();
    let fw_audits = storage
        .query_audit_events(&crate::storage::models::AuditQueryFilter {
            domain: Some("OUT_OF_AGENT".to_string()),
            actor_kind: Some("fast_weight".to_string()),
            limit: 10,
            ..Default::default()
        })
        .unwrap_or_default();
    let fw_audit_ok = fw_audits.iter().all(|a| !a.evidence_sha256.trim().is_empty());

    let dq12_checks_ok = [anchored_cps_ok, gate_invariants_ok, fw_audit_ok]
        .into_iter()
        .filter(|ok| *ok)
        .count();
    let dq12_pct = ((dq12_checks_ok as f64 / 3.0) * 100.0 * 100.0).round() / 100.0;
    results.push(DataQualityResult {
        id: "DQ12".into(),
        name: "fast_weight_lineage_and_unlearn_integrity".into(),
        title: "快权重血缘锚定、三道硬门禁与定向遗忘审计不变量门禁".into(),
        measured_pct: dq12_pct,
        target_pct: 100.0,
        passed: dq12_checks_ok == 3,
        detail: format!(
            "fw_checkpoints_ok={} | gate_invariants_ok={} | fw_audit_ok={} (audits={})",
            anchored_cps_ok,
            gate_invariants_ok,
            fw_audit_ok,
            fw_audits.len()
        ),
    });

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

    #[test]
    fn test_dq9_telemetry_and_awr_integrity_check() {
        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite");
        let results = run_data_quality_checks(&storage);
        assert_eq!(results.len(), 12);
        let dq9 = results.iter().find(|r| r.id == "DQ9").expect("DQ9");
        assert!(dq9.passed, "空库下 DQ9 应全通过: {}", dq9.detail);
        assert_eq!(dq9.measured_pct, 100.0);
        let dq10 = results.iter().find(|r| r.id == "DQ10").expect("DQ10");
        assert!(dq10.passed, "空库下 DQ10 应全通过: {}", dq10.detail);
        let dq11 = results.iter().find(|r| r.id == "DQ11").expect("DQ11");
        assert!(dq11.passed, "空库下 DQ11 应全通过: {}", dq11.detail);
        let dq12 = results.iter().find(|r| r.id == "DQ12").expect("DQ12");
        assert!(dq12.passed, "空库下 DQ12 应全通过: {}", dq12.detail);
    }

    #[test]
    fn test_dq12_detects_unanchored_fast_weight_checkpoint() {
        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite");
        storage
            .insert_body_checkpoint(&crate::storage::models::BodyCheckpointRecord {
                checkpoint_id: "cp-bad-fw".to_string(),
                body_id: "b1".to_string(),
                from_brain: "agy".to_string(),
                session_id: "s1".to_string(),
                goal_summary: "missing base model".to_string(),
                active_files: vec![],
                active_locks: vec![],
                verified_facts: vec![],
                open_blockers: vec![],
                next_step_hint: "".to_string(),
                token_budget: 200,
                status: "active".to_string(),
                evidence_sha256: "a".repeat(64),
                created_at: "2026-11-28T00:00:00Z".to_string(),
                resumed_by_brain: None,
                resumed_at: None,
                tx_epoch: 1,
                fast_weight_snapshot_id: Some("11111111-1111-4111-8111-111111111111".to_string()),
                fast_weight_base_model: None,
            })
            .expect("insert bad fw checkpoint");
        let results = run_data_quality_checks(&storage);
        let dq12 = results.iter().find(|r| r.id == "DQ12").expect("DQ12");
        assert!(!dq12.passed, "绑定 snapshot_id 但缺失 base_model 的胶囊应被 DQ12 拦截");
    }

    #[test]
    fn test_dq11_detects_corrupt_sleep_summary() {
        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite");
        storage
            .insert_memory(&crate::storage::models::MemoryRecord {
                id: "sleep-sum-bad".to_string(),
                memory_type: "experience".to_string(),
                content: "leaked api_key=sk-proj-BadLeakSleepSummary12345678".to_string(),
                summary: None,
                source_agent: Some("sleep_time_compactor".to_string()),
                source_session: Some("sleep-1".to_string()),
                tags: vec!["sleep_compacted".to_string()],
                weight: 0.8,
                pinned: false,
                status: "active".to_string(),
                embedding: vec![],
            })
            .expect("insert bad sleep summary");
        let results = run_data_quality_checks(&storage);
        let dq11 = results.iter().find(|r| r.id == "DQ11").expect("DQ11");
        assert!(!dq11.passed, "含明文密钥的睡眠摘要应被 DQ11 拦截");
    }

    #[test]
    fn test_dq10_detects_ungrounded_knowhow_and_hairball() {
        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite");
        storage
            .upsert_fact_node(&crate::storage::models::FactNodeRecord {
                id: "kh:bad-ungrounded".into(),
                node_type: "knowhow".into(),
                name: "bad_claim".into(),
                label: "bad_claim".into(),
                properties: serde_json::json!({
                    "claim": "bad_claim",
                    "extractor": "qwen3.5:0.8b",
                    "evidence_quote": "",
                    "source_path": "/tmp/a.md"
                }),
                updated_at: None,
            })
            .expect("upsert");
        let results = run_data_quality_checks(&storage);
        let dq10 = results.iter().find(|r| r.id == "DQ10").expect("DQ10");
        assert!(!dq10.passed, "无 evidence_quote 的 Know-How 节点应被 DQ10 拦下");
    }

    #[test]
    fn test_dq2_consistency_and_dq6_project_connectivity_beyond_edge_limit() {
        let storage = SqliteStorageProvider::open_in_memory().expect("sqlite");
        // 1. 空库检查：所有 passed 项必须满足 measured_pct >= target_pct
        for r in run_data_quality_checks(&storage) {
            if r.passed {
                assert!(
                    r.measured_pct >= r.target_pct,
                    "{} 判定与展示 SLA 口径矛盾: measured={} target={}",
                    r.id,
                    r.measured_pct,
                    r.target_pct
                );
            }
        }

        // 2. 构造 1 个 project 节点 + 1 条连接边 + 验证 DQ2 与 DQ6 连通检查
        storage
            .upsert_fact_node(&crate::storage::models::FactNodeRecord {
                id: "project:demo".into(),
                node_type: "project".into(),
                name: "demo".into(),
                label: "demo".into(),
                properties: serde_json::json!({}),
                updated_at: None,
            })
            .expect("proj");
        storage
            .upsert_fact_node(&crate::storage::models::FactNodeRecord {
                id: "term:rust".into(),
                node_type: "domain_term".into(),
                name: "rust".into(),
                label: "rust".into(),
                properties: serde_json::json!({}),
                updated_at: None,
            })
            .expect("term");
        storage
            .upsert_fact_edge(&crate::storage::models::FactEdgeRecord::new_explicit(
                "project:demo",
                "term:rust",
                "depends_on",
                serde_json::json!({}),
                0.9,
            ))
            .expect("edge");
        let results = run_data_quality_checks(&storage);
        let dq2 = results.iter().find(|r| r.id == "DQ2").expect("DQ2");
        assert_eq!(dq2.target_pct, 95.0);
        let dq6 = results.iter().find(|r| r.id == "DQ6").expect("DQ6");
        assert!(dq6.passed, "DQ6 应识别 project:demo 连通: {}", dq6.detail);
        assert!(dq6.detail.contains("1/1 (100.0%)"), "实测: {}", dq6.detail);
    }
}
