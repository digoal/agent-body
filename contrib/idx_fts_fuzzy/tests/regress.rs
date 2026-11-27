//! `ext-idx-fts-fuzzy` 集成回归测试：安装迁移、驼峰拆词、精确/模糊检索与 RRF 召回

use std::collections::BTreeMap;

use agent_body_kernel::{KernelRuntime, SearchQuery};
use agent_body_sdk::{CallerIdentity, EvidenceTuple, IndexAmRoutine, OpStrategy, OperatorRoutine};
use ext_idx_fts_fuzzy::{
    scoring::normalize_identifier_phrase, FtsExactOperator, FtsFuzzyOperator, IdxFtsFuzzyExtension,
};

fn make_entity(id: &str, symbol: &str, summary: &str) -> EvidenceTuple {
    EvidenceTuple {
        evidence_id: format!("ev-{id}"),
        host_id: "host-local".to_string(),
        host_labels: BTreeMap::new(),
        entity_type: "entity:symbol".to_string(),
        entity_id: id.to_string(),
        source_ext: "idx_fts_fuzzy".to_string(),
        evidence_tier: "L1_EXPLICIT".to_string(),
        collected_at: "2026-11-27T16:00:00+08:00".to_string(),
        ttl_sec: 60,
        payload: serde_json::json!({
            "symbol": symbol,
            "summary": summary
        }),
        redacted: false,
        evidence_sha256: String::new(),
    }
    .with_computed_sha256()
}

#[test]
fn camel_case_splitting_and_operators_work() {
    assert_eq!(
        normalize_identifier_phrase("ShadowAbEngine"),
        "shadow ab engine"
    );
    assert_eq!(
        normalize_identifier_phrase("shadow_ab_engine"),
        "shadow ab engine"
    );

    let exact = FtsExactOperator;
    assert_eq!(exact.strategy_num(), OpStrategy::Exact.strategy_num());
    assert!(exact
        .evaluate(
            &serde_json::json!("ShadowAbEngine"),
            &serde_json::json!("shadow ab engine")
        )
        .unwrap());
    assert!(!exact
        .evaluate(
            &serde_json::json!("ShadowAbEngine"),
            &serde_json::json!("ShadwAbEngin")
        )
        .unwrap());

    let fuzzy = FtsFuzzyOperator;
    assert_eq!(fuzzy.strategy_num(), OpStrategy::Fuzzy.strategy_num());
    assert!(fuzzy
        .evaluate(
            &serde_json::json!("ShadowAbEngine"),
            &serde_json::json!("ShadwAbEngin")
        )
        .unwrap());
}

#[test]
fn kernel_runtime_installs_extension_and_recalls_exact_camel_and_fuzzy_via_rrf() {
    let mut rt = KernelRuntime::open_sqlite_in_memory(None).unwrap();
    let ext = IdxFtsFuzzyExtension::new();
    let idx_am = ext.index_am();
    let order = rt.install_extensions(&[Box::new(ext)]).unwrap();
    assert_eq!(order, vec!["idx_fts_fuzzy"]);

    let rec = rt
        .get_extension_record("idx_fts_fuzzy")
        .unwrap()
        .expect("extension catalog record must exist");
    assert_eq!(rec.table_prefix, "ext_idx_fts_fuzzy__");

    // 插入含驼峰命名与近似拼写的实体，并验证 ext_idx_fts_fuzzy__terms 表已创建
    let e_camel = make_entity(
        "sym:shadow_ab",
        "ShadowAbEngine",
        "Dual-track shadow execution verification engine",
    );
    let e_typo_target = make_entity(
        "sym:pg_pool",
        "PostgresConnectionPool",
        "Managed PostgreSQL connection pool controller",
    );
    let e_noise = make_entity(
        "sym:metrics",
        "LatencyHistogram",
        "Wait event latency histogram collector",
    );

    for tup in [&e_camel, &e_typo_target, &e_noise] {
        rt.table_am().entity_upsert(tup).unwrap();
        idx_am.aminsert(tup).unwrap();
    }
    assert_eq!(idx_am.cached_doc_count(), 3);

    rt.table_am()
        .exec_scoped_migration(
            "idx_fts_fuzzy",
            "ext_idx_fts_fuzzy__",
            "0.1.1",
            "INSERT INTO ext_idx_fts_fuzzy__terms (entity_type, entity_id, term, term_freq, doc_len, updated_at)
             VALUES ('entity:symbol', 'sym:shadow_ab', 'shadow', 1, 5, '2026-11-27T16:00:00+08:00');",
        )
        .unwrap();

    let caller = CallerIdentity {
        caller_id: "test-runner".to_string(),
        is_loopback: true,
        ..Default::default()
    };

    // 1. 精确检索
    let exact_res = rt
        .search_rrf(
            &SearchQuery {
                query_text: "ShadowAbEngine".to_string(),
                strategy_num: OpStrategy::Exact.strategy_num(),
                limit: 5,
                ..Default::default()
            },
            &caller,
        )
        .unwrap();
    assert!(!exact_res.hits.is_empty());
    assert_eq!(exact_res.hits[0].entity_id, "sym:shadow_ab");

    // 2. 驼峰拆词检索 ("shadow ab engine" -> ShadowAbEngine)
    let split_res = rt
        .search_rrf(
            &SearchQuery {
                query_text: "shadow ab engine".to_string(),
                strategy_num: OpStrategy::Exact.strategy_num(),
                limit: 5,
                ..Default::default()
            },
            &caller,
        )
        .unwrap();
    assert!(!split_res.hits.is_empty());
    assert_eq!(split_res.hits[0].entity_id, "sym:shadow_ab");

    // 3. 模糊错拼检索 ("PostgrsConnectonPool" -> PostgresConnectionPool)
    let fuzzy_res = rt
        .search_rrf(
            &SearchQuery {
                query_text: "PostgrsConnectonPool".to_string(),
                strategy_num: OpStrategy::Fuzzy.strategy_num(),
                limit: 5,
                ..Default::default()
            },
            &caller,
        )
        .unwrap();
    assert!(!fuzzy_res.hits.is_empty());
    assert_eq!(fuzzy_res.hits[0].entity_id, "sym:pg_pool");
}
