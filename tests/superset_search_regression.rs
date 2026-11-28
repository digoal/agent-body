use std::collections::HashSet;
use std::fs;
use std::path::Path;

use agent_body::body::{AbCompareOptions, ShadowAbEngine};
use agent_body::config::Config;
use agent_body::scanner::{
    backfill_asset_contents, AssetSearchEngine, AssetSearchOptions, Scanner,
};
use agent_body::storage::models::AssetRecord;
use agent_body::storage::Storage;

#[test]
fn test_superset_over_grep_exact_private_fn_cjk_and_zero_noise() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    fs::create_dir_all(root.join("src")).expect("mkdir src");
    fs::create_dir_all(root.join("docs")).expect("mkdir docs");

    let target_rs = root.join("src/engine.rs");
    let mut padding = String::new();
    for i in 0..450 {
        padding.push_str(&format!("// boilerplate filler line {}\n", i));
    }
    fs::write(
        &target_rs,
        format!(
            "{}\nfn bounded_levenshtein_secret_helper() {{}}\npub struct ShadowAbEngine;\n// 避免帮倒忙诊察逻辑\n",
            padding
        ),
    )
    .expect("write target_rs");

    let noise_py = root.join("src/noise_worker.py");
    fs::write(&noise_py, "def unrelated_job():\n    return 42\n").expect("write noise");

    let env_secret = root.join(".env");
    fs::write(&env_secret, "OPENAI_API_KEY=sk-test-super-secret-value-999999\n").expect("write .env");

    let storage = Storage::open_in_memory().expect("open_in_memory");
    let scanner = Scanner::new(storage.clone()).expect("scanner");
    let rep = scanner.scan_path(root).expect("scan workspace");
    assert!(rep.deep_indexed_files >= 2);

    // 写入向量，故意给无关 noise_py 一个中等余弦相似度 (0.40) 以验证防噪门禁不会充当多余结果
    storage
        .update_asset_embedding(noise_py.to_string_lossy().as_ref(), &[0.40, 0.9165, 0.0])
        .expect("embed noise");
    storage
        .update_asset_embedding(target_rs.to_string_lossy().as_ref(), &[0.99, 0.141, 0.0])
        .expect("embed target");

    let query_vec = [1.0f32, 0.0, 0.0];
    for q in [
        "bounded_levenshtein_secret_helper",
        "ShadowAbEngine",
        "帮倒忙",
    ] {
        // 1) Indexed DB Trigram Fuzzy + Omni-RRF
        let mut opts_idx = AssetSearchOptions::new(q, 10);
        opts_idx.use_fuzzy_index = true;
        let hits_idx = AssetSearchEngine::search_with_query_vec(
            storage.provider().as_ref(),
            &opts_idx,
            Some(&query_vec),
        )
        .expect("search indexed");
        assert_eq!(
            hits_idx.len(),
            1,
            "查询 '{}' 应精准命中 target_rs 且 0 无关噪声, got={:?}",
            q,
            hits_idx.iter().map(|h| &h.asset.path).collect::<Vec<_>>()
        );
        assert_eq!(hits_idx[0].asset.path, target_rs.to_string_lossy().as_ref());
        assert_eq!(hits_idx[0].match_tier, "deterministic");
        assert!(hits_idx[0].matched_line.unwrap_or(0) > 400);

        // 2) Pure DB LIKE Scan fallback (use_fuzzy_index = false)
        let mut opts_scan = AssetSearchOptions::new(q, 10);
        opts_scan.use_fuzzy_index = false;
        let hits_scan = AssetSearchEngine::search_with_query_vec(
            storage.provider().as_ref(),
            &opts_scan,
            Some(&query_vec),
        )
        .expect("search db scan");
        assert_eq!(hits_scan.len(), 1);
        assert_eq!(hits_scan[0].asset.path, target_rs.to_string_lossy().as_ref());
    }

    // 3) 不存在的 strict identifier 必须返回 0 条（绝不用弱向量充数）
    // 不相关语义向量（与所有资产向量正交，模拟真实嵌入下的无关查询）
    let unrelated_vec = [0.0f32, 0.0, 1.0];
    let mut opts_none = AssetSearchOptions::new("nonexistent_symbol_xyz_56", 10);
    opts_none.hybrid = true;
    let none_hits = AssetSearchEngine::search_with_query_vec(
        storage.provider().as_ref(),
        &opts_none,
        Some(&unrelated_vec),
    )
    .expect("search nonexistent");
    assert!(
        none_hits.is_empty(),
        "不存在的严格标识符必须返回 0 条而非弱向量垃圾, got={:?}",
        none_hits.iter().map(|h| &h.asset.path).collect::<Vec<_>>()
    );

    // 4) .env 明文秘密绝不允许入库或被检索召回
    let secret_hits = AssetSearchEngine::search_with_query_vec(
        storage.provider().as_ref(),
        &AssetSearchOptions::new("super-secret-value", 10),
        None,
    )
    .expect("secret check");
    assert!(secret_hits.is_empty(), "敏感文件正文严禁被索引或召回");
}

#[test]
fn test_search_breaks_5000_row_truncation_limit_and_excludes_missing() {
    let storage = Storage::open_in_memory().expect("open_in_memory");
    // 快速批量写入 5,300 条资产（超过旧版硬编码 list_assets(5000) 截断线！）
    for i in 0..5300usize {
        let is_target = i == 0;
        let path = format!("/tmp/scale_bench/mod_{:05}.rs", i);
        let rec = AssetRecord {
            id: format!("id-{}", i),
            path: path.clone(),
            asset_type: "script".to_string(),
            owner: "tester".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("generic background worker".to_string()),
            desc_kv: serde_json::json!({}),
            usage: None,
            size_bytes: Some(128),
            file_count: None,
            tags: vec![],
            status: "active".to_string(),
        };
        storage.upsert_asset(&rec).expect("upsert");
        if is_target {
            storage
                .update_asset_content(&path, "pub fn needle_beyond_5000_rows_limit() {}")
                .expect("update needle content");
        }
    }
    // 写入 1 条已删除的 missing 幽灵资产，确保绝不会被搜出
    storage
        .upsert_asset(&AssetRecord {
            id: "id-missing".to_string(),
            path: "/tmp/scale_bench/ghost_missing.rs".to_string(),
            asset_type: "script".to_string(),
            owner: "tester".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("needle_beyond_5000_rows_limit".to_string()),
            desc_kv: serde_json::json!({}),
            usage: None,
            size_bytes: Some(128),
            file_count: None,
            tags: vec![],
            status: "missing".to_string(),
        })
        .expect("upsert missing");

    let opts = AssetSearchOptions::new("needle_beyond_5000_rows_limit", 10);
    let hits = AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts, None)
        .expect("scale search");
    assert_eq!(hits.len(), 1, "必须突破 5000 行截断找到最早的第 0 条并过滤 missing 资产");
    assert_eq!(hits[0].asset.path, "/tmp/scale_bench/mod_00000.rs");
}

#[test]
fn test_shadow_ab_compare_detailed_superset_and_backfill() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    let f1 = root.join("alpha.rs");
    let f2 = root.join("beta.md");
    fs::write(&f1, "pub fn alpha_superset_marker() {}\n").expect("f1");
    fs::write(&f2, "# Related Docs\nMentions alpha_superset_marker concept.\n").expect("f2");

    let storage = Storage::open_in_memory().expect("open_in_memory");
    // 先仅入库元数据（无 content_text），验证 backfill_asset_contents 能全量无损补写正文
    for (p, t) in [(&f1, "script"), (&f2, "document")] {
        storage
            .upsert_asset(&AssetRecord {
                id: uuid::Uuid::new_v4().to_string(),
                path: p.to_string_lossy().to_string(),
                asset_type: t.to_string(),
                owner: "tester".to_string(),
                security_level: "internal".to_string(),
                scan_tier: "deep".to_string(),
                description: Some("test item".to_string()),
                desc_kv: serde_json::json!({}),
                usage: None,
                size_bytes: Some(32),
                file_count: None,
                tags: vec![],
                status: "active".to_string(),
            })
            .expect("upsert");
    }

    let bf = backfill_asset_contents(&storage, 65536, 100, Some(&root.to_string_lossy()), false)
        .expect("backfill contents");
    assert_eq!(bf.updated, 2);

    let mut cfg = Config::default();
    cfg.workspace.workspace_roots = vec![root.to_path_buf()];
    let opts = AbCompareOptions {
        scope_path: Some(root.to_string_lossy().to_string()),
        hybrid: false,
        fuzzy: true,
        use_fuzzy_index: true,
        graph_hops: 1,
        include_links: true,
        ..Default::default()
    };
    let rep = ShadowAbEngine::compare_asset_search_detailed(
        &storage,
        &cfg,
        "alpha_superset_marker",
        10,
        &opts,
        "test-superset",
    )
    .expect("detailed ab compare");

    assert!(rep.superset_satisfied, "missed={:?}", rep.missed_by_body);
    assert!(rep.missed_by_body.is_empty());
    let common_set: HashSet<String> = rep.common_paths.into_iter().collect();
    assert!(common_set.contains(Path::new(&f1).to_string_lossy().as_ref()));
    assert!(common_set.contains(Path::new(&f2).to_string_lossy().as_ref()));
}
