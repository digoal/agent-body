use std::collections::{HashMap, HashSet};
use std::path::Path;
use anyhow::Result;
use serde_json::Value;

use crate::body::{WaitEventGuard, WaitEventId};
use crate::config::Config;
use crate::fact::knowledge_graph::{expand_graph_for_query, AliasCanonicalizer};
use crate::memory::embedding::EmbeddingClient;
use crate::scanner::metadata::{format_asset_line, DetailLevel};
use crate::scanner::multimodal::infer_modality;
use crate::storage::models::{AssetRecord, AssetScalarFilter};
use crate::storage::spi::StorageProvider;

mod fuzzy_and_gate;

pub use fuzzy_and_gate::{
    classify_query_intent, compute_scalar_modulation, fuzzy_text_score, QueryIntent,
};
use fuzzy_and_gate::{apply_precision_gate, fuzzy_text_score_prepared, PreparedFuzzyQuery};

/// 统一资产检索选项（支持 DB Fuzzy 索引加速 + BM25 + 标量下推 + 校准向量 + 知识图谱 + 互链 Omni-RRF）
#[derive(Debug, Clone)]
pub struct AssetSearchOptions {
    pub query: String,
    pub limit: usize,
    pub modality: Option<String>,
    pub asset_type: Option<String>,
    pub domain: Option<String>,
    pub kv_key: Option<String>,
    pub kv_value: Option<String>,
    pub kv_filter: Option<String>,
    pub hybrid: bool,
    pub fuzzy: bool,
    pub use_fuzzy_index: bool,
    pub scope_path: Option<String>,
    pub min_vec_similarity: Option<f32>,
    pub graph_hops: usize,
    pub include_links: bool,
    pub include_hierarchy: bool,
    pub context_dir: Option<String>,
}

impl AssetSearchOptions {
    pub fn new(query: impl Into<String>, limit: usize) -> Self {
        let cfg = Config::load_effective_or_default();
        Self {
            query: query.into(),
            limit: limit.clamp(1, 200),
            modality: None,
            asset_type: None,
            domain: None,
            kv_key: None,
            kv_value: None,
            kv_filter: None,
            hybrid: true,
            fuzzy: true,
            use_fuzzy_index: cfg.knowledge_graph.db_fuzzy_index,
            scope_path: None,
            min_vec_similarity: None,
            graph_hops: 1,
            include_links: true,
            include_hierarchy: true,
            context_dir: None,
        }
    }

    pub fn has_filters(&self) -> bool {
        self.modality.as_deref().is_some_and(|s| !s.trim().is_empty())
            || self.asset_type.as_deref().is_some_and(|s| !s.trim().is_empty())
            || self.domain.as_deref().is_some_and(|s| !s.trim().is_empty())
            || self.scope_path.as_deref().is_some_and(|s| !s.trim().is_empty())
            || self.kv_key.as_deref().is_some_and(|s| !s.trim().is_empty())
            || self.kv_value.as_deref().is_some_and(|s| !s.trim().is_empty())
            || self.kv_filter.as_deref().is_some_and(|s| !s.trim().is_empty())
    }

    pub fn to_scalar_filter(&self) -> AssetScalarFilter {
        AssetScalarFilter {
            modality: self.modality.clone(),
            asset_type: self.asset_type.clone(),
            domain: self.domain.clone(),
            scope_path: self.scope_path.clone(),
            kv_key: self.kv_key.clone(),
            kv_value: self.kv_value.clone(),
            kv_filter: self.kv_filter.clone(),
        }
    }
}

/// 资产检索命中项（含多通道归因、匹配行片段、分级分层标识与知识图推理路径）
#[derive(Debug, Clone)]
pub struct AssetSearchHit {
    pub asset: AssetRecord,
    pub score: f64,
    pub matched_channels: Vec<String>,
    pub matched_sections: Vec<String>,
    pub reasoning_paths: Vec<String>,
    pub matched_line: Option<usize>,
    pub matched_snippet: Option<String>,
    pub match_tier: String, // "deterministic" | "expansion"
}

/// 资产 Omni-RRF 混合检索引擎（DB Fuzzy + BM25/FTS5 + Scalar + Calibrated Vec + Graph + Hierarchy + Links）
pub struct AssetSearchEngine;

impl AssetSearchEngine {
    pub fn effective_modality(asset: &AssetRecord) -> String {
        if let Some(m) = asset.desc_kv.get("modality").and_then(|v| v.as_str()) {
            let trimmed = m.trim().to_ascii_lowercase();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
        let ext = Path::new(&asset.path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        infer_modality(&asset.asset_type, ext).to_string()
    }

    fn value_is_non_empty(val: &Value) -> bool {
        match val {
            Value::Null => false,
            Value::String(s) => !s.trim().is_empty(),
            Value::Array(arr) => !arr.is_empty(),
            Value::Object(map) => !map.is_empty(),
            _ => true,
        }
    }

    fn value_contains_ci(val: &Value, needle_lower: &str) -> bool {
        match val {
            Value::Null => false,
            Value::String(s) => s.to_ascii_lowercase().contains(needle_lower),
            Value::Number(n) => n.to_string().to_ascii_lowercase().contains(needle_lower),
            Value::Bool(b) => b.to_string().contains(needle_lower),
            Value::Array(arr) => arr.iter().any(|item| Self::value_contains_ci(item, needle_lower)),
            Value::Object(map) => map.values().any(|v| Self::value_contains_ci(v, needle_lower)),
        }
    }

    /// 结构化事实标量过滤校验（`scope_path` / `modality` / `asset_type` / `domain` / `kv_key` / `kv_value` / `kv_filter`）
    pub fn matches_filters(asset: &AssetRecord, opts: &AssetSearchOptions) -> bool {
        if let Some(ref sc) = opts.scope_path.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            if !asset.path.starts_with(sc) {
                return false;
            }
        }
        if let Some(ref at) = opts.asset_type {
            let want = at.trim();
            if !want.is_empty() && !asset.asset_type.eq_ignore_ascii_case(want) {
                return false;
            }
        }
        if let Some(ref m) = opts.modality {
            let want = m.trim().to_ascii_lowercase();
            if !want.is_empty() && Self::effective_modality(asset) != want {
                return false;
            }
        }
        if let Some(ref d) = opts.domain {
            let want = d.trim().to_ascii_lowercase();
            if !want.is_empty() {
                let got = asset
                    .desc_kv
                    .get("domain")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if got != want && !got.contains(&want) {
                    return false;
                }
            }
        }

        let kv_key_clean = opts.kv_key.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let kv_val_clean = opts.kv_value.as_deref().map(str::trim).filter(|s| !s.is_empty());
        match (kv_key_clean, kv_val_clean) {
            (Some(k), Some(v)) => {
                let needle = v.to_ascii_lowercase();
                let Some(val) = asset.desc_kv.get(k) else {
                    return false;
                };
                if !Self::value_contains_ci(val, &needle) {
                    return false;
                }
            }
            (Some(k), None) => {
                let Some(val) = asset.desc_kv.get(k) else {
                    return false;
                };
                if !Self::value_is_non_empty(val) {
                    return false;
                }
            }
            (None, Some(v)) => {
                let needle = v.to_ascii_lowercase();
                if !Self::value_contains_ci(&asset.desc_kv, &needle) {
                    return false;
                }
            }
            (None, None) => {}
        }

        if let Some(ref expr) = opts.kv_filter {
            let trimmed = expr.trim();
            if !trimmed.is_empty() {
                let preds = crate::scanner::multimodal_ext::parse_kv_filters(trimmed);
                if !crate::scanner::multimodal_ext::matches_kv_filters(asset, &preds) {
                    return false;
                }
            }
        }
        true
    }

    fn build_searchable_haystack(asset: &AssetRecord) -> String {
        let mut parts = vec![asset.path.clone()];
        for arr_key in [
            "symbols",
            "headings",
            "config_sections",
            "svg_text_snippets",
            "ocr_keywords",
            "table_names",
            "columns",
        ] {
            if let Some(arr) = asset.desc_kv.get(arr_key).and_then(|v| v.as_array()) {
                let items: Vec<_> = arr.iter().filter_map(|x| x.as_str()).take(20).collect();
                if !items.is_empty() {
                    parts.push(items.join(" "));
                }
            }
        }
        if let Some(ref desc) = asset.description {
            parts.push(desc.clone());
        }
        if let Some(ref u) = asset.usage {
            parts.push(u.clone());
        }
        if !asset.tags.is_empty() {
            parts.push(asset.tags.join(" "));
        }
        for key in ["purpose", "entry_point", "visual_summary", "pdf_title", "doc_title", "pdf_excerpt"] {
            if let Some(s) = asset.desc_kv.get(key).and_then(|v| v.as_str()) {
                parts.push(s.to_string());
            }
        }
        parts.join(" ")
    }

    /// 执行资产 Omni-RRF 搜索（自动检测 Ollama Embedding 在线态）
    pub fn search(storage: &dyn StorageProvider, opts: &AssetSearchOptions) -> Result<Vec<AssetSearchHit>> {
        let q = opts.query.trim();
        let query_vec = if opts.hybrid && !q.is_empty() {
            let client = EmbeddingClient::default();
            if client.is_available() {
                client.embed_text(q)
            } else {
                None
            }
        } else {
            None
        };
        Self::search_with_query_vec(storage, opts, query_vec.as_deref())
    }

    /// 离线可测核心入口：全量 DB 下推通道 + 精准门禁 Omni-RRF 融合
    pub fn search_with_query_vec(
        storage: &dyn StorageProvider,
        opts: &AssetSearchOptions,
        query_vec: Option<&[f32]>,
    ) -> Result<Vec<AssetSearchHit>> {
        let limit = opts.limit.clamp(1, 200);
        let q = opts.query.trim();
        let cfg = Config::load_effective_or_default();

        // 空查询：全库数据库侧标量过滤与重要度排序（彻底消除 5000 行截断）
        if q.is_empty() {
            let assets = storage.search_assets_scalar_db(&opts.to_scalar_filter(), limit * 3)?;
            let mut hits = Vec::new();
            for a in assets {
                if Self::matches_filters(&a, opts) {
                    let mod_f = compute_scalar_modulation(&a);
                    hits.push(AssetSearchHit {
                        asset: a,
                        score: mod_f,
                        matched_channels: vec!["scalar".to_string()],
                        matched_sections: Vec::new(),
                        reasoning_paths: Vec::new(),
                        matched_line: None,
                        matched_snippet: None,
                        match_tier: "deterministic".to_string(),
                    });
                    if hits.len() >= limit {
                        break;
                    }
                }
            }
            return Ok(hits);
        }

        let candidate_limit = (limit * 8).clamp(80, 600);
        let intent = classify_query_intent(q);
        let prep_q = PreparedFuzzyQuery::new(q);
        let scope_ref = opts.scope_path.as_deref().map(str::trim).filter(|s| !s.is_empty());

        let mut candidate_assets: HashMap<String, AssetRecord> = HashMap::new();
        let mut line_meta: HashMap<String, (Option<usize>, Option<String>)> = HashMap::new();

        // 1. 路 1: 数据库侧 Fuzzy 模糊查询（可选 Trigram 索引加速 / SQL LIKE 兜底，全库无截断）
        let mut fuzzy_db_ranks: HashMap<String, (usize, String)> = HashMap::new();
        let mut exact_ranks: HashMap<String, usize> = HashMap::new();
        let mut fuzzy_ranks: HashMap<String, usize> = HashMap::new();
        let mut fuzzy_strong: HashSet<String> = HashSet::new();
        if opts.fuzzy || intent == QueryIntent::StrictIdentifier {
            let db_fuzzy_hits = storage
                .search_assets_fuzzy_db(q, candidate_limit, opts.use_fuzzy_index, scope_ref)
                .unwrap_or_default();
            for h in db_fuzzy_hits {
                if !Self::matches_filters(&h.asset, opts) {
                    continue;
                }
                let p = h.asset.path.clone();
                if let Some(ref snip) = h.matched_snippet {
                    line_meta.entry(p.clone()).or_insert((h.matched_line, Some(snip.clone())));
                } else if h.matched_line.is_some() {
                    line_meta.entry(p.clone()).or_insert((h.matched_line, None));
                }
                let hay = Self::build_searchable_haystack(&h.asset);
                let hay_lower = hay.to_ascii_lowercase();
                if h.matched_line.is_some()
                    || (!prep_q.q_lower.is_empty() && hay_lower.contains(&prep_q.q_lower))
                {
                    let rank = exact_ranks.len();
                    exact_ranks.entry(p.clone()).or_insert(rank);
                }
                let idx_rank = fuzzy_db_ranks.len();
                fuzzy_db_ranks.entry(p.clone()).or_insert((idx_rank, h.match_mode));
                if opts.fuzzy {
                    let sc = fuzzy_text_score_prepared(&prep_q, &hay).max(h.rank_score);
                    if sc >= 0.70 {
                        fuzzy_strong.insert(p.clone());
                    }
                    let fz_rank = fuzzy_ranks.len();
                    fuzzy_ranks.entry(p.clone()).or_insert(fz_rank);
                }
                candidate_assets.entry(p).or_insert(h.asset);
            }
        }

        // 2. 路 2: BM25 / FTS5 全文检索（含驼峰拆词 + content_text 正文）
        let fts_raw = storage.search_assets_fts(q, candidate_limit).unwrap_or_default();
        let mut bm25_ranks: HashMap<String, usize> = HashMap::new();
        for a in fts_raw {
            if !Self::matches_filters(&a, opts) {
                continue;
            }
            let p = a.path.clone();
            let r = bm25_ranks.len();
            bm25_ranks.entry(p.clone()).or_insert(r);
            let hay_lower = Self::build_searchable_haystack(&a).to_ascii_lowercase();
            if !prep_q.q_lower.is_empty() && hay_lower.contains(&prep_q.q_lower) {
                let er = exact_ranks.len();
                exact_ranks.entry(p.clone()).or_insert(er);
            }
            candidate_assets.entry(p).or_insert(a);
        }

        let has_det_so_far =
            !exact_ranks.is_empty() || !bm25_ranks.is_empty() || !fuzzy_db_ranks.is_empty();

        // 3. 路 3: 校准语义向量通道（全库直扫 active embeddings + 严格相似度与标识符防噪门禁）
        let mut vec_ranks: HashMap<String, usize> = HashMap::new();
        let mut vec_sims: HashMap<String, f32> = HashMap::new();
        let base_min_sim = opts
            .min_vec_similarity
            .unwrap_or(cfg.knowledge_graph.min_vec_similarity)
            .clamp(0.05, 0.99);
        let eff_min_sim = if intent == QueryIntent::StrictIdentifier
            && cfg.knowledge_graph.strict_identifier_vec_gate
        {
            if has_det_so_far {
                base_min_sim.max(0.65)
            } else {
                base_min_sim.max(0.78)
            }
        } else {
            base_min_sim
        };
        if let Some(qvec) = query_vec {
            let _wait = WaitEventGuard::enter(WaitEventId::CpuVectorCosine);
            let vec_hits = storage
                .search_assets_vector_db(qvec, eff_min_sim, candidate_limit, scope_ref)
                .unwrap_or_default();
            for (a, sim) in vec_hits {
                if !Self::matches_filters(&a, opts) {
                    continue;
                }
                let p = a.path.clone();
                let r = vec_ranks.len();
                vec_ranks.entry(p.clone()).or_insert(r);
                vec_sims.insert(p.clone(), sim);
                candidate_assets.entry(p).or_insert(a);
            }
        }

        // 4. 路 4: 属性图 Know-How / 概念 / 章节 1~2 跳扩散通道
        let mut graph_ranks: HashMap<String, usize> = HashMap::new();
        let mut matched_sections_map: HashMap<String, Vec<String>> = HashMap::new();
        let mut reasoning_paths_map: HashMap<String, Vec<String>> = HashMap::new();
        if opts.graph_hops > 0 {
            let aliases = AliasCanonicalizer::default();
            let g_exp = expand_graph_for_query(storage, q, opts.graph_hops.min(2), &aliases);
            matched_sections_map = g_exp.matched_sections;
            reasoning_paths_map = g_exp.reasoning_paths;
            let mut g_scored: Vec<(String, f64)> = g_exp
                .doc_scores
                .into_iter()
                .filter(|(_, sc)| *sc > 0.0)
                .collect();
            g_scored.sort_by(|x, y| y.1.total_cmp(&x.1));
            for (p, _) in g_scored.into_iter().take((limit * 5).max(50)) {
                if resolve_active_asset(storage, &mut candidate_assets, &p, opts).is_some() {
                    let rank = graph_ranks.len();
                    graph_ranks.entry(p).or_insert(rank);
                }
            }
        }

        // 5. 路 5: 层级结构通道（同章节聚合 + 目录局部性）
        let mut hierarchy_ranks: HashMap<String, usize> = HashMap::new();
        if opts.include_hierarchy {
            let mut dir_votes: HashMap<String, f64> = HashMap::new();
            for p in bm25_ranks.keys().chain(fuzzy_db_ranks.keys()).chain(graph_ranks.keys()) {
                if let Some(parent) = Path::new(p).parent() {
                    let dp = parent.to_string_lossy().to_string();
                    *dir_votes.entry(dp).or_insert(0.0) += 1.0;
                }
            }
            if let Some(ref ctx_d) = opts.context_dir {
                if !ctx_d.trim().is_empty() {
                    *dir_votes.entry(ctx_d.trim().to_string()).or_insert(0.0) += 2.5;
                }
            }
            let mut h_scored: Vec<(String, f64)> = Vec::new();
            for p in candidate_assets.keys() {
                let sec_cnt = matched_sections_map.get(p).map(|v| v.len()).unwrap_or(0) as f64;
                let dir_bonus = Path::new(p)
                    .parent()
                    .and_then(|d| dir_votes.get(&d.to_string_lossy().to_string()).copied())
                    .unwrap_or(0.0);
                let has_base_hit = bm25_ranks.contains_key(p)
                    || fuzzy_db_ranks.contains_key(p)
                    || graph_ranks.contains_key(p);
                let total_h = sec_cnt * 0.8 + if has_base_hit { dir_bonus * 0.25 } else { 0.0 };
                if total_h > 0.0 {
                    h_scored.push((p.clone(), total_h));
                }
            }
            h_scored.sort_by(|x, y| y.1.total_cmp(&x.1));
            for (rank, (p, _)) in h_scored.into_iter().take((limit * 4).max(40)).enumerate() {
                hierarchy_ranks.insert(p, rank);
            }
        }

        // 6. 路 6: 文档间显式互链通道 (`query_links_for_node` 全库直查)
        let mut doc_link_ranks: HashMap<String, usize> = HashMap::new();
        if opts.include_links {
            let mut anchor_set: HashSet<String> = HashSet::new();
            for (p, &r) in &bm25_ranks {
                if r < 6 {
                    anchor_set.insert(p.clone());
                }
            }
            for (p, (r, _)) in &fuzzy_db_ranks {
                if *r < 6 {
                    anchor_set.insert(p.clone());
                }
            }
            for (p, &r) in &graph_ranks {
                if r < 4 {
                    anchor_set.insert(p.clone());
                }
            }
            let mut link_scores: HashMap<String, f64> = HashMap::new();
            for anchor in anchor_set.iter().take(10) {
                if let Ok(links) = storage.query_links_for_node("asset", anchor) {
                    for l in links {
                        let (neighbor, weight_mul, arrow) = if l.source_id == *anchor {
                            (&l.target_id, 1.0f64, format!("{} -[{}]-> {}", l.source_id, l.relation, l.target_id))
                        } else {
                            (&l.source_id, 0.85f64, format!("{} <-[{}]- {}", l.source_id, l.relation, l.target_id))
                        };
                        if anchor_set.contains(neighbor) {
                            continue;
                        }
                        if resolve_active_asset(storage, &mut candidate_assets, neighbor, opts).is_some() {
                            *link_scores.entry(neighbor.clone()).or_insert(0.0) += (l.confidence as f64) * weight_mul;
                            let rps = reasoning_paths_map.entry(neighbor.clone()).or_default();
                            if rps.len() < 2 && !rps.contains(&arrow) {
                                rps.push(arrow);
                            }
                        }
                    }
                }
            }
            let mut ls: Vec<(String, f64)> = link_scores.into_iter().collect();
            ls.sort_by(|x, y| y.1.total_cmp(&x.1));
            for (rank, (p, _)) in ls.into_iter().take((limit * 3).max(30)).enumerate() {
                doc_link_ranks.insert(p, rank);
            }
        }

        // 7. 自适应权重重归一化 & Omni-RRF 融合
        let fuzzy_w = if bm25_ranks.is_empty() { 1.40 } else { 0.90 };
        let mut active_w = 0.0f64;
        if !exact_ranks.is_empty() { active_w += 2.20; }
        if !bm25_ranks.is_empty() { active_w += 1.25; }
        if !fuzzy_db_ranks.is_empty() { active_w += fuzzy_w; }
        if !vec_ranks.is_empty() { active_w += 1.00; }
        if !graph_ranks.is_empty() { active_w += 1.10; }
        if !hierarchy_ranks.is_empty() { active_w += 0.60; }
        if !doc_link_ranks.is_empty() { active_w += 0.65; }
        if opts.has_filters() { active_w += 0.35; }
        let norm_div = if active_w > 0.0 { active_w } else { 1.0 };

        let mut all_hit_paths: HashSet<String> = HashSet::new();
        for p in exact_ranks
            .keys()
            .chain(bm25_ranks.keys())
            .chain(fuzzy_db_ranks.keys())
            .chain(vec_ranks.keys())
            .chain(graph_ranks.keys())
            .chain(hierarchy_ranks.keys())
            .chain(doc_link_ranks.keys())
        {
            all_hit_paths.insert(p.clone());
        }

        let k = 60.0f64;
        let mut ranked: Vec<AssetSearchHit> = Vec::with_capacity(all_hit_paths.len());
        for p in all_hit_paths {
            let Some(asset) = candidate_assets.get(&p) else {
                continue;
            };
            let mut rrf_sum = 0.0f64;
            let mut channels = Vec::new();
            let mut is_deterministic = false;

            if let Some(&r) = exact_ranks.get(&p) {
                rrf_sum += (2.20 / norm_div) / (10.0 + r as f64);
                channels.push("exact_id".to_string());
                is_deterministic = true;
            }
            if let Some((r, ref mode)) = fuzzy_db_ranks.get(&p) {
                let eff_w = if fuzzy_strong.contains(&p) { fuzzy_w + 0.55 } else { fuzzy_w };
                rrf_sum += (eff_w / norm_div) / (20.0 + *r as f64);
                channels.push("fuzzy".to_string());
                if mode != "fuzzy" {
                    channels.push(mode.clone());
                }
                is_deterministic = true;
            }
            if let Some(&r) = bm25_ranks.get(&p) {
                rrf_sum += (1.25 / norm_div) / (k + r as f64);
                channels.push("bm25_fts".to_string());
                is_deterministic = true;
            }
            if opts.has_filters() {
                rrf_sum += (0.35 / norm_div) / k;
                channels.push("scalar".to_string());
            }
            if let Some(&r) = vec_ranks.get(&p) {
                rrf_sum += (1.00 / norm_div) / (k + r as f64);
                channels.push("vec".to_string());
            }
            if let Some(&r) = graph_ranks.get(&p) {
                rrf_sum += (1.10 / norm_div) / (k + r as f64);
                channels.push("graph_knowhow".to_string());
            }
            if let Some(&r) = hierarchy_ranks.get(&p) {
                rrf_sum += (0.60 / norm_div) / (k + r as f64);
                channels.push("hierarchy".to_string());
            }
            if let Some(&r) = doc_link_ranks.get(&p) {
                rrf_sum += (0.65 / norm_div) / (k + r as f64);
                channels.push("doc_links".to_string());
            }

            let phi_scalar = compute_scalar_modulation(asset);
            let final_score = rrf_sum * phi_scalar;
            let (matched_line, matched_snippet) = line_meta.get(&p).cloned().unwrap_or((None, None));
            ranked.push(AssetSearchHit {
                asset: asset.clone(),
                score: final_score,
                matched_channels: channels,
                matched_sections: matched_sections_map.get(&p).cloned().unwrap_or_default(),
                reasoning_paths: reasoning_paths_map.get(&p).cloned().unwrap_or_default(),
                matched_line,
                matched_snippet,
                match_tier: if is_deterministic { "deterministic" } else { "expansion" }.to_string(),
            });
        }

        let gated = apply_precision_gate(
            ranked,
            &vec_sims,
            intent,
            cfg.knowledge_graph.strict_identifier_vec_gate,
            eff_min_sim,
            limit,
        );
        Ok(gated)
    }

    /// 统一渲染搜索结果文本（供 CLI / MCP 复用）
    pub fn format_results(hits: &[AssetSearchHit], detail: DetailLevel) -> String {
        let mut out = format!(
            "资产搜索匹配结果 (共 {} 条, detail={:?}):\n",
            hits.len(),
            detail
        );
        for h in hits {
            out.push_str(&format_asset_line(&h.asset, detail));
            if !matches!(detail, DetailLevel::Micro) {
                if !h.matched_channels.is_empty() {
                    out.push_str(&format!("\n    召回通道: [{}]", h.matched_channels.join("+")));
                }
                if let Some(ref snip) = h.matched_snippet {
                    if let Some(ln) = h.matched_line {
                        out.push_str(&format!("\n    命中行(L{}): {}", ln, snip));
                    } else {
                        out.push_str(&format!("\n    命中片段: {}", snip));
                    }
                } else if let Some(sec) = h.matched_sections.first() {
                    out.push_str(&format!("\n    匹配片段: {}", sec));
                }
                if let Some(rp) = h.reasoning_paths.first() {
                    out.push_str(&format!("\n    推理路径: {}", rp));
                }
            }
            out.push('\n');
        }
        out
    }
}

fn resolve_active_asset(
    storage: &dyn StorageProvider,
    cache: &mut HashMap<String, AssetRecord>,
    path: &str,
    opts: &AssetSearchOptions,
) -> Option<AssetRecord> {
    if let Some(a) = cache.get(path) {
        return Some(a.clone());
    }
    if let Ok(Some(a)) = storage.get_asset_by_path(path) {
        if a.status == "active" && AssetSearchEngine::matches_filters(&a, opts) {
            cache.insert(path.to_string(), a.clone());
            return Some(a);
        }
    }
    None
}

/// 顶层便捷格式化函数导出
pub fn format_search_results(hits: &[AssetSearchHit], detail: DetailLevel) -> String {
    AssetSearchEngine::format_results(hits, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KnowledgeGraphConfig;
    use crate::fact::KnowledgeGraphBuilder;
    use crate::storage::models::LinkRecord;
    use crate::storage::Storage;
    use serde_json::json;

    fn sample_asset(path: &str, asset_type: &str, desc: &str, kv: Value) -> AssetRecord {
        AssetRecord {
            id: uuid::Uuid::new_v4().to_string(),
            path: path.to_string(),
            asset_type: asset_type.to_string(),
            owner: "user:tester".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some(desc.to_string()),
            desc_kv: kv,
            usage: None,
            size_bytes: Some(1024),
            file_count: None,
            tags: vec![],
            status: "active".to_string(),
        }
    }

    #[test]
    fn test_structured_filters_and_vector_rrf_recall() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let img = sample_asset(
            "/tmp/test/arch_diagram.png",
            "media",
            "[web_dashboard] PNG 图像 arch_diagram.png 1280x720",
            json!({
                "modality": "image",
                "domain": "web_dashboard",
                "dimensions": "1280x720",
                "visual_summary": "系统三态运行模式架构拓扑图",
                "embedding_status": "ready",
                "importance_score": 0.9
            }),
        );
        let db = sample_asset(
            "/tmp/test/analytics.db",
            "dataset",
            "[storage_engine] DB 数据库，含表 [orders, users]",
            json!({
                "modality": "dataset",
                "domain": "storage_engine",
                "table_names": ["orders", "users"],
                "columns": ["orders.amount", "orders.region"],
                "embedding_status": "pending",
                "importance_score": 0.7
            }),
        );
        let code = sample_asset(
            "/tmp/test/main.rs",
            "script",
            "[asset_scanner] Rust scanner entrypoint",
            json!({
                "modality": "code",
                "domain": "asset_scanner",
                "embedding_status": "ready",
                "importance_score": 0.8
            }),
        );

        storage.upsert_asset(&img).expect("upsert img");
        storage.upsert_asset(&db).expect("upsert db");
        storage.upsert_asset(&code).expect("upsert code");
        storage.update_asset_embedding(&img.path, &[1.0, 0.0, 0.0]).expect("embed img");
        storage.update_asset_embedding(&code.path, &[0.0, 1.0, 0.0]).expect("embed code");

        let opts_vec = AssetSearchOptions::new("system topology diagram", 5);
        let hits = AssetSearchEngine::search_with_query_vec(
            storage.provider().as_ref(),
            &opts_vec,
            Some(&[0.98, 0.02, 0.0]),
        )
        .expect("vec search");
        assert_eq!(hits.len(), 1, "低相似度噪声应被精准门禁剔除，仅返回高相关图片");
        assert_eq!(hits[0].asset.path, "/tmp/test/arch_diagram.png");
        assert!(hits[0].matched_channels.contains(&"vec".to_string()));

        let mut opts_mod = AssetSearchOptions::new("", 10);
        opts_mod.modality = Some("dataset".to_string());
        let db_hits = AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts_mod, None)
            .expect("mod search");
        assert_eq!(db_hits.len(), 1);
        assert_eq!(db_hits[0].asset.path, "/tmp/test/analytics.db");

        let mut opts_kv = AssetSearchOptions::new("analytics", 10);
        opts_kv.kv_key = Some("table_names".to_string());
        opts_kv.kv_value = Some("orders".to_string());
        let kv_hits = AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts_kv, None)
            .expect("kv search");
        assert_eq!(kv_hits.len(), 1);
        assert_eq!(kv_hits[0].asset.path, "/tmp/test/analytics.db");
    }

    #[test]
    fn test_omni_rrf_camel_fuzzy_doc_links_and_graph_recall() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let dir = tempfile::tempdir().expect("tempdir");

        let code = sample_asset(
            "/tmp/omni/runtime_ab.rs",
            "script",
            "[body] 真实运行期对照",
            json!({
                "modality": "code",
                "symbols": ["ShadowAbEngine", "compare_asset_search"],
                "importance_score": 0.9
            }),
        );
        storage.upsert_asset(&code).expect("upsert code");

        let doc1 = dir.path().join("wal_spike_root.md");
        let doc2 = dir.path().join("wal_mitigation.md");
        let linked_spec = dir.path().join("wal_spec_diagram.svg");
        std::fs::write(&doc1, "# WAL 写放大\nfull_page_writes 触发 checkpoint_spike。\n").expect("doc1");
        std::fs::write(&doc2, "# WAL 参数调优\n调大 max_wal_size 缓解 checkpoint_spike 问题。\n").expect("doc2");

        for (p, t, kv) in [
            (doc1.to_string_lossy().to_string(), "document", json!({"modality": "document", "importance_score": 0.8})),
            (doc2.to_string_lossy().to_string(), "document", json!({"modality": "document", "importance_score": 0.8})),
            (linked_spec.to_string_lossy().to_string(), "media", json!({"modality": "image", "importance_score": 0.6})),
        ] {
            storage.upsert_asset(&sample_asset(&p, t, "knowledge doc", kv)).expect("upsert");
        }
        storage
            .add_link(&LinkRecord {
                id: None,
                source_type: "asset".to_string(),
                source_id: doc1.to_string_lossy().to_string(),
                target_type: "asset".to_string(),
                target_id: linked_spec.to_string_lossy().to_string(),
                relation: "references".to_string(),
                confidence: 0.95,
                auto_discovered: true,
                line_number: Some(1),
                snippet: None,
                provenance: Some("md_link".to_string()),
                created_at: None,
            })
            .expect("add link");

        let kg_cfg = KnowledgeGraphConfig {
            pmi_threshold: 0.0,
            ..Default::default()
        };
        KnowledgeGraphBuilder::mine_from_storage(&storage, &kg_cfg).expect("mine kg");

        // A. 驼峰标识符搜索：精确 + 拆词命中
        let mut opts_camel = AssetSearchOptions::new("ShadowAbEngine", 5);
        opts_camel.hybrid = false;
        let camel_hits = AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts_camel, None)
            .expect("camel search");
        assert!(!camel_hits.is_empty());
        assert_eq!(camel_hits[0].asset.path, "/tmp/omni/runtime_ab.rs");
        assert!(camel_hits[0].matched_channels.contains(&"bm25_fts".to_string()));

        // B. 拼写误差模糊检索：ShadwAbEngine -> DB Trigram + Fuzzy 兜底找回 runtime_ab.rs
        let mut opts_typo = AssetSearchOptions::new("ShadwAbEngine", 5);
        opts_typo.hybrid = false;
        let typo_hits = AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts_typo, None)
            .expect("typo search");
        assert!(
            typo_hits.iter().any(|h| h.asset.path == "/tmp/omni/runtime_ab.rs"),
            "DB Trigram + Fuzzy 应容错召回 ShadowAbEngine: {:?}",
            typo_hits.iter().map(|h| &h.asset.path).collect::<Vec<_>>()
        );

        // C. 离线跨词检索 full_page_writes：图谱 2 跳召回 doc2，文档互链召回 linked_spec
        let mut opts_graph = AssetSearchOptions::new("full_page_writes", 10);
        opts_graph.hybrid = false;
        opts_graph.graph_hops = 2;
        let g_hits = AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts_graph, None)
            .expect("omni graph search");
        let doc2_str = doc2.to_string_lossy().to_string();
        let spec_str = linked_spec.to_string_lossy().to_string();
        assert!(
            g_hits.iter().any(|h| h.asset.path == doc2_str),
            "Graph 通道应从 full_page_writes 召回 doc2"
        );
        assert!(
            g_hits.iter().any(|h| h.asset.path == spec_str && h.matched_channels.contains(&"doc_links".to_string())),
            "DocLinks 通道应将 doc1 的引用扩展召回到 linked_spec"
        );

        // D. 泛词噪音 + 低相关向量噪音抑制
        let noise_doc = sample_asset(
            "/tmp/omni/generic_engine_phase13.md",
            "document",
            "Execution engine phase architecture and scheduler engine runtime engine",
            json!({"modality": "document", "importance_score": 0.9, "embedding_status": "ready"}),
        );
        storage.upsert_asset(&noise_doc).expect("upsert noise");
        storage.update_asset_embedding(&noise_doc.path, &[0.35, 0.93, 0.0]).expect("embed noise");
        let id_hits = AssetSearchEngine::search_with_query_vec(
            storage.provider().as_ref(),
            &opts_camel,
            Some(&[1.0, 0.0, 0.0]),
        )
        .expect("id search");
        assert_eq!(id_hits.len(), 1, "精确标识符命中时应阻止低相关向量噪音填充");
        assert_eq!(id_hits[0].asset.path, "/tmp/omni/runtime_ab.rs");
        assert!(id_hits[0].matched_channels.contains(&"exact_id".to_string()));
    }
}
