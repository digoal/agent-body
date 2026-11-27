//! `ext-idx-fts-fuzzy`: Pure-Rust FTS + 驼峰/蛇形拆词 + Bigram Jaccard + 有界 Levenshtein 索引扩展

pub mod scoring;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use agent_body_sdk::{
    body_module_magic, BodyExtension, EntityScanFilter, EvidenceTuple, ExtensionKind,
    ExtensionMeta, ExtensionRegistrar, IndexAmRoutine, IndexCostEstimate, IndexHit, IndexScanDesc,
    MigrationScript, OpStrategy, OperatorRoutine, TableAmRoutine, SDK_API_VERSION,
};
use anyhow::bail;

use scoring::{
    bigram_jaccard, bounded_levenshtein, edit_tolerance_for_len, extract_candidate_words,
    normalize_identifier_phrase, score_document, split_identifier_terms, Bm25Corpus,
};

body_module_magic!();

const MIGRATION_0_1_0: &str = include_str!("../sql/idx_fts_fuzzy--0.1.0.sql");

/// 模糊与全文检索索引扩展入口
#[derive(Debug, Default)]
pub struct IdxFtsFuzzyExtension {
    index_am: Arc<FtsFuzzyIndexAm>,
}

impl IdxFtsFuzzyExtension {
    pub fn new() -> Self {
        Self {
            index_am: Arc::new(FtsFuzzyIndexAm::new()),
        }
    }

    pub fn index_am(&self) -> Arc<FtsFuzzyIndexAm> {
        Arc::clone(&self.index_am)
    }
}

impl BodyExtension for IdxFtsFuzzyExtension {
    fn meta(&self) -> ExtensionMeta {
        ExtensionMeta {
            ext_id: "idx_fts_fuzzy".to_string(),
            version: "0.1.0".to_string(),
            kind: ExtensionKind::CompiledCrate,
            min_sdk_version: SDK_API_VERSION.to_string(),
            table_prefix: "ext_idx_fts_fuzzy__".to_string(),
            requires: vec![],
        }
    }

    fn register(&self, reg: &mut ExtensionRegistrar) -> anyhow::Result<()> {
        reg.register_migration(MigrationScript {
            version: "0.1.0".to_string(),
            upgrade_from: None,
            sql: MIGRATION_0_1_0.to_string(),
        });
        reg.register_index_am(Arc::clone(&self.index_am) as Arc<dyn IndexAmRoutine>);
        reg.register_operator(
            "idx:fts_fuzzy",
            "entity:*",
            Arc::new(FtsExactOperator),
        );
        reg.register_operator(
            "idx:fts_fuzzy",
            "entity:*",
            Arc::new(FtsFuzzyOperator),
        );
        Ok(())
    }
}

#[derive(Debug, Default)]
struct CachedDoc {
    source_ext: String,
    entity_id: String,
    terms: Vec<String>,
}

/// Pure-Rust 多策略全文/模糊检索索引访问方法 (`idx:fts_fuzzy`)
#[derive(Debug, Default)]
pub struct FtsFuzzyIndexAm {
    cache: RwLock<BTreeMap<(String, String), CachedDoc>>,
}

impl FtsFuzzyIndexAm {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cached_doc_count(&self) -> usize {
        self.cache.read().map(|g| g.len()).unwrap_or(0)
    }
}

impl IndexAmRoutine for FtsFuzzyIndexAm {
    fn am_id(&self) -> &'static str {
        "idx:fts_fuzzy"
    }

    fn default_weight(&self) -> f64 {
        1.0
    }

    fn amcostestimate(&self, scan: &IndexScanDesc) -> IndexCostEstimate {
        let available = !scan.query_text.trim().is_empty();
        IndexCostEstimate {
            available,
            est_latency_us: 120,
            est_selectivity: if scan.strategy_num == OpStrategy::Exact.strategy_num() {
                0.08
            } else {
                0.25
            },
        }
    }

    fn amgetmulti(
        &self,
        scan: &IndexScanDesc,
        am: &dyn TableAmRoutine,
    ) -> anyhow::Result<Vec<IndexHit>> {
        if scan.query_text.trim().is_empty() {
            return Ok(Vec::new());
        }
        let filter = build_scan_filter(scan);
        let entities = am.entity_scan(&filter, scan.rls_filter.as_ref())?;
        if entities.is_empty() {
            return Ok(Vec::new());
        }

        let docs: Vec<(EvidenceTuple, String, Vec<String>)> = entities
            .into_iter()
            .map(|tuple| {
                let text = flatten_tuple_text(&tuple);
                let terms = split_identifier_terms(&text);
                (tuple, text, terms)
            })
            .collect();

        let term_lists: Vec<Vec<String>> = docs.iter().map(|(_, _, terms)| terms.clone()).collect();
        let corpus = Bm25Corpus::build(&term_lists);

        let mut hits = Vec::new();
        for (tuple, text, terms) in docs {
            let (score, reasoning_path) =
                score_document(&scan.query_text, &text, &terms, &corpus, scan.strategy_num);
            if score > 0.0 {
                hits.push(IndexHit {
                    entity_id: tuple.entity_id.clone(),
                    entity_type: tuple.entity_type.clone(),
                    score,
                    channel: self.am_id().to_string(),
                    reasoning_path,
                    tuple: Some(tuple),
                });
            }
        }

        hits.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.entity_id.cmp(&b.entity_id))
        });
        let limit = if scan.limit == 0 { 20 } else { scan.limit };
        hits.truncate(limit);
        Ok(hits)
    }

    fn aminsert(&self, entity: &EvidenceTuple) -> anyhow::Result<()> {
        let text = flatten_tuple_text(entity);
        let terms = split_identifier_terms(&text);
        let mut guard = self
            .cache
            .write()
            .map_err(|_| anyhow::anyhow!("idx:fts_fuzzy 内存索引锁已中毒"))?;
        guard.insert(
            (entity.entity_type.clone(), entity.entity_id.clone()),
            CachedDoc {
                source_ext: entity.source_ext.clone(),
                entity_id: entity.entity_id.clone(),
                terms,
            },
        );
        Ok(())
    }

    fn amdelete_by_source(&self, source_key: &str) -> anyhow::Result<usize> {
        let mut guard = self
            .cache
            .write()
            .map_err(|_| anyhow::anyhow!("idx:fts_fuzzy 内存索引锁已中毒"))?;
        let before = guard.len();
        guard.retain(|(_, eid), doc| doc.source_ext != source_key && eid != source_key);
        Ok(before.saturating_sub(guard.len()))
    }

    fn amvacuum(&self) -> anyhow::Result<usize> {
        let mut guard = self
            .cache
            .write()
            .map_err(|_| anyhow::anyhow!("idx:fts_fuzzy 内存索引锁已中毒"))?;
        let before = guard.len();
        guard.retain(|_, doc| !doc.terms.is_empty() && !doc.entity_id.is_empty());
        Ok(before.saturating_sub(guard.len()))
    }
}

/// 精确/拆词匹配操作符算子 (`OpStrategy::Exact`)
pub struct FtsExactOperator;

impl OperatorRoutine for FtsExactOperator {
    fn op_id(&self) -> &'static str {
        "op:fts_exact"
    }

    fn strategy_num(&self) -> u16 {
        OpStrategy::Exact.strategy_num()
    }

    fn evaluate(&self, lhs: &serde_json::Value, rhs: &serde_json::Value) -> anyhow::Result<bool> {
        let (Some(hay), Some(needle)) = (lhs.as_str(), rhs.as_str()) else {
            bail!("op:fts_exact 仅支持字符串 JSON 值比对");
        };
        let n_trim = needle.trim();
        if n_trim.is_empty() {
            return Ok(false);
        }
        if hay.to_ascii_lowercase().contains(&n_trim.to_ascii_lowercase()) {
            return Ok(true);
        }
        let hay_phrase = normalize_identifier_phrase(hay);
        let needle_phrase = normalize_identifier_phrase(n_trim);
        if !needle_phrase.is_empty() && hay_phrase.contains(&needle_phrase) {
            return Ok(true);
        }
        let hay_set: BTreeSet<String> = split_identifier_terms(hay).into_iter().collect();
        let needle_terms = split_identifier_terms(n_trim);
        Ok(!needle_terms.is_empty() && needle_terms.iter().all(|t| hay_set.contains(t)))
    }
}

/// 模糊匹配操作符算子 (`OpStrategy::Fuzzy`)：支持 Bigram Jaccard 与有界 Levenshtein
pub struct FtsFuzzyOperator;

impl OperatorRoutine for FtsFuzzyOperator {
    fn op_id(&self) -> &'static str {
        "op:fts_fuzzy"
    }

    fn strategy_num(&self) -> u16 {
        OpStrategy::Fuzzy.strategy_num()
    }

    fn evaluate(&self, lhs: &serde_json::Value, rhs: &serde_json::Value) -> anyhow::Result<bool> {
        if FtsExactOperator.evaluate(lhs, rhs)? {
            return Ok(true);
        }
        let (Some(hay), Some(needle)) = (lhs.as_str(), rhs.as_str()) else {
            bail!("op:fts_fuzzy 仅支持字符串 JSON 值比对");
        };
        let n_trim = needle.trim();
        if n_trim.is_empty() {
            return Ok(false);
        }

        let hay_words = extract_candidate_words(hay);
        let needle_words = extract_candidate_words(n_trim);

        for nw in &needle_words {
            for hw in &hay_words {
                if bigram_jaccard(nw, hw) >= 0.40 {
                    return Ok(true);
                }
                let nc: Vec<char> = nw.chars().collect();
                let hc: Vec<char> = hw.chars().collect();
                if let Some(tol) = edit_tolerance_for_len(nc.len()) {
                    if hc.len() >= 4 && bounded_levenshtein(&nc, &hc, tol).is_some() {
                        return Ok(true);
                    }
                }
            }
        }
        Ok(false)
    }
}

fn build_scan_filter(scan: &IndexScanDesc) -> EntityScanFilter {
    let mut filter = EntityScanFilter {
        entity_type: scan.scalar_filters.get("entity_type").cloned(),
        source_ext: scan.scalar_filters.get("source_ext").cloned(),
        host_id: scan.scalar_filters.get("host_id").cloned(),
        label_filters: BTreeMap::new(),
        limit: 0,
        offset: 0,
    };
    for (k, v) in &scan.scalar_filters {
        if k != "entity_type" && k != "source_ext" && k != "host_id" {
            filter.label_filters.insert(k.clone(), v.clone());
        }
    }
    filter
}

fn flatten_tuple_text(tuple: &EvidenceTuple) -> String {
    let mut parts = vec![
        tuple.entity_type.clone(),
        tuple.entity_id.clone(),
        tuple.host_id.clone(),
        tuple.source_ext.clone(),
    ];
    collect_json_strings(&tuple.payload, &mut parts);
    parts.join(" ")
}

fn collect_json_strings(val: &serde_json::Value, out: &mut Vec<String>) {
    match val {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Number(n) => out.push(n.to_string()),
        serde_json::Value::Array(arr) => {
            for item in arr {
                collect_json_strings(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            for v in map.values() {
                collect_json_strings(v, out);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) => {}
    }
}
