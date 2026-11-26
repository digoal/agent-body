use std::fs::{self, File};
use std::io::Write;
use agent_body::body::ShadowAbEngine;
use agent_body::config::{Config, RuntimeMode};
use agent_body::feedback::probes::FreshnessReconciler;
use agent_body::hooks::AutoSyncEngine;
use agent_body::scanner::{
    backfill_multimodal_assets, classify_asset_path, AssetSearchEngine, AssetSearchOptions, Scanner,
};
use agent_body::storage::models::AssetRecord;
use agent_body::storage::Storage;
use serde_json::json;
use tempfile::tempdir;

#[test]
fn test_multimodal_3_channel_parity_kv_filter_and_backfill() {
    let dir = tempdir().expect("tempdir");
    let work = dir.path().join("work");
    fs::create_dir_all(&work).expect("mkdir");

    let html_p = work.join("deck.html");
    fs::write(
        &html_p,
        "<!DOCTYPE html><html><head><title>Omni RRF Deck</title></head>\
         <body><h1>架构总览</h1><section>1</section><section>2</section><svg></svg></body></html>",
    )
    .expect("write html");

    let csv_p = work.join("metrics.csv");
    fs::write(&csv_p, "id,latency_ms,ok\n1,12.5,true\n2,8.4,false\n").expect("write csv");

    let fake_png = work.join("diagram.dat");
    fs::write(&fake_png, b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x10\x00\x00\x00\x10\x08\x06\x00\x00\x00").expect("write png");

    // 1. 统一三通道分类验证 + Scanner 全量入库
    assert_eq!(classify_asset_path(&html_p).asset_type, "document");
    assert_eq!(classify_asset_path(&csv_p).asset_type, "dataset");
    assert_eq!(classify_asset_path(&fake_png).asset_type, "media");
    assert_eq!(classify_asset_path(&fake_png).mime_type, "image/png");

    let storage = Storage::open_in_memory().expect("mem db");
    let scanner = Scanner::new(storage.clone()).expect("scanner");
    scanner.scan_path(&work).expect("scan_path");

    // AutoSync 通道一致性验证
    let auto_html = work.join("notes.html");
    fs::write(&auto_html, "<html><title>AutoSync Page</title><body><h1>Hello</h1></body></html>")
        .expect("write auto html");
    let engine = AutoSyncEngine::new(storage.clone());
    engine
        .sync_file_output(&auto_html.to_string_lossy(), "write_file")
        .expect("auto_sync");
    let auto_rec = storage
        .get_asset_by_path(&auto_html.to_string_lossy())
        .expect("get auto")
        .expect("auto stored");
    assert_eq!(auto_rec.asset_type, "document");
    assert_eq!(
        auto_rec.desc_kv.get("schema_version").and_then(|v| v.as_str()),
        Some("2.0")
    );

    // 2. 多谓词 KV 过滤 + asset_type 联合检索
    let mut opts = AssetSearchOptions::new("", 10);
    opts.asset_type = Some("document".to_string());
    opts.kv_filter = Some("schema_version=2.0,has:doc_title,section_count>=2".to_string());
    opts.hybrid = false;
    let hits = AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts, None)
        .expect("kv search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].asset.path, html_p.to_string_lossy().as_ref());

    // 3. 存量 legacy `other` 记录经 Reconcile / Backfill 无损升级
    let legacy_csv = work.join("legacy_events.jsonl");
    {
        let mut f = File::create(&legacy_csv).expect("create jsonl");
        writeln!(f, "{{\"ev\":\"boot\",\"ok\":true}}").expect("write jsonl");
    }
    storage
        .upsert_asset(&AssetRecord {
            id: "leg-jsonl".to_string(),
            path: legacy_csv.to_string_lossy().to_string(),
            asset_type: "other".to_string(),
            owner: "user:test".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("已有 L2 描述".to_string()),
            desc_kv: json!({"annotation_status": "ollama_enriched", "content_hash": "old"}),
            usage: None,
            size_bytes: Some(10),
            file_count: None,
            tags: vec![],
            status: "active".to_string(),
        })
        .expect("seed legacy");
    let bf = backfill_multimodal_assets(&storage, 10, None).expect("backfill");
    assert_eq!(bf.upgraded, 1);
    let rec_rep = FreshnessReconciler::reconcile(&storage).expect("reconcile");
    assert_eq!(rec_rep.multimodal_assets_backfilled, 0);

    // 4. 运行模式 override 解析 + Shadow A/B H1 自动自愈集成验证
    assert_eq!(
        Config::read_mode_override_in(dir.path()),
        None
    );
    fs::write(dir.path().join("runtime_mode.override"), "bypass\n").expect("write override");
    assert_eq!(
        Config::read_mode_override_in(dir.path()),
        Some(RuntimeMode::Bypass)
    );

    let unindexed = work.join("unindexed_live_note.md");
    fs::write(&unindexed, "# LivePhase63HealMarker\nContent\n").expect("write unindexed");
    let mut cfg = Config::default();
    cfg.workspace.workspace_roots = vec![work.clone()];
    cfg.runtime.mode = "shadow_ab".to_string();
    let (cmp, _, merged) =
        ShadowAbEngine::compare_asset_search(&storage, &cfg, "LivePhase63HealMarker", 5, "s1")
            .expect("ab compare");
    assert!(cmp.auto_healed_count >= 1);
    assert!(merged
        .iter()
        .any(|a| a.path == unindexed.to_string_lossy().as_ref()));
}
