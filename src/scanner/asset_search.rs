use std::collections::{HashMap, HashSet};
use std::path::Path;
use anyhow::Result;
use serde_json::Value;

use crate::body::{WaitEventGuard, WaitEventId};
use crate::fact::knowledge_graph::{
    expand_graph_for_query, tokenize_terms, AliasCanonicalizer,
};
use crate::memory::embedding::EmbeddingClient;
use crate::memory::hybrid::cosine_similarity;
use crate::scanner::metadata::{format_asset_line, DetailLevel};
use crate::scanner::multimodal::infer_modality;
use crate::storage::models::AssetRecord;
use crate::storage::spi::StorageProvider;

/// 统一资产检索选项（支持 Omni-RRF 8 维融合检索 + 多模态/领域/KV 结构化过滤）
#[derive(Debug, Clone)]
pub struct AssetSearchOptions {
    pub query: String,
    pub limit: usize,
    pub modality: Option<String>,
    pub domain: Option<String>,
    pub kv_key: Option<String>,
    pub kv_value: Option<String>,
    pub hybrid: bool,
    pub fuzzy: bool,
    pub graph_hops: usize,
    pub include_links: bool,
    pub include_hierarchy: bool,
    pub context_dir: Option<String>,
}

impl AssetSearchOptions {
    pub fn new(query: impl Into<String>, limit: usize) -> Self {
        Self {
            query: query.into(),
            limit: limit.clamp(1, 200),
            modality: None,
            domain: None,
            kv_key: None,
            kv_value: None,
            hybrid: true,
            fuzzy: true,
            graph_hops: 1,
            include_links: true,
            include_hierarchy: true,
            context_dir: None,
        }
    }

    pub fn has_filters(&self) -> bool {
        self.modality.as_deref().is_some_and(|s| !s.trim().is_empty())
            || self.domain.as_deref().is_some_and(|s| !s.trim().is_empty())
            || self.kv_key.as_deref().is_some_and(|s| !s.trim().is_empty())
            || self.kv_value.as_deref().is_some_and(|s| !s.trim().is_empty())
    }
}

/// 资产检索命中项（含多通道归因、匹配章节片段与知识图推理路径）
#[derive(Debug, Clone)]
pub struct AssetSearchHit {
    pub asset: AssetRecord,
    pub score: f64,
    pub matched_channels: Vec<String>,
    pub matched_sections: Vec<String>,
    pub reasoning_paths: Vec<String>,
}

fn bounded_levenshtein(a: &[char], b: &[char], max_dist: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > max_dist {
        return None;
    }
    if a.is_empty() {
        return Some(b.len());
    }
    if b.is_empty() {
        return Some(a.len());
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        let mut row_min = curr[0];
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            let del = prev[j + 1] + 1;
            let ins = curr[j] + 1;
            let sub = prev[j] + cost;
            let v = del.min(ins).min(sub);
            curr[j + 1] = v;
            if v < row_min {
                row_min = v;
            }
        }
        if row_min > max_dist {
            return None;
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    let final_dist = prev[b.len()];
    if final_dist <= max_dist {
        Some(final_dist)
    } else {
        None
    }
}

fn char_bigrams(s: &str) -> HashSet<(char, char)> {
    let chars: Vec<char> = s.chars().collect();
    let mut set = HashSet::new();
    for w in chars.windows(2) {
        set.insert((w[0], w[1]));
    }
    set
}

/// 纯 Rust Fuzzy 模糊匹配打分（子串/前缀 + 字符 bigram Jaccard + 有界 Levenshtein 容错）
pub fn fuzzy_text_score(query: &str, haystack: &str) -> f64 {
    let q = query.trim().to_ascii_lowercase();
    if q.is_empty() || haystack.trim().is_empty() {
        return 0.0;
    }
    let hay_lower = haystack.to_ascii_lowercase();
    let mut score = 0.0f64;

    if hay_lower.contains(&q) {
        score += 0.65;
    }

    let aliases = AliasCanonicalizer::default();
    let q_terms = tokenize_terms(&q, &aliases);
    if !q_terms.is_empty() {
        let mut matched = 0usize;
        for t in &q_terms {
            if hay_lower.contains(t) {
                matched += 1;
            }
        }
        score += 0.35 * (matched as f64 / q_terms.len() as f64);
    }

    // 针对词长 >= 4 的查询词在候选词表上做 bounded Levenshtein 错别字匹配
    let hay_words: Vec<&str> = hay_lower
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.len() >= 4)
        .take(128)
        .collect();
    for qt in q
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.len() >= 4)
    {
        let qc: Vec<char> = qt.chars().collect();
        let tol = if qc.len() >= 8 { 2 } else { 1 };
        let mut best_edit_bonus = 0.0f64;
        for hw in &hay_words {
            if *hw == qt {
                best_edit_bonus = 0.65;
                break;
            }
            if hw.contains(qt) {
                best_edit_bonus = best_edit_bonus.max(0.55);
                continue;
            }
            let hc: Vec<char> = hw.chars().collect();
            if let Some(d) = bounded_levenshtein(&qc, &hc, tol) {
                let ratio = 1.0 - (d as f64 / qc.len().max(hc.len()) as f64);
                let typo_scale = if qc.len() >= 8 { 1.15 } else { 0.72 };
                best_edit_bonus = best_edit_bonus.max(typo_scale * ratio);
            }
        }
        score += best_edit_bonus;
    }

    // 字符 bigram Jaccard（对中英文连写与碎片均有效）
    let qb = char_bigrams(&q);
    if !qb.is_empty() {
        let hb = char_bigrams(&hay_lower.chars().take(800).collect::<String>());
        if !hb.is_empty() {
            let inter = qb.intersection(&hb).count() as f64;
            let jaccard = inter / qb.len().max(1) as f64;
            if jaccard >= 0.35 {
                score += 0.30 * jaccard.min(1.0);
            }
        }
    }

    score
}

/// 事实标量软调制因子 $\Phi_{\text{scalar}}(d)$（重要度 + 30天近因衰减 + 权威清单 + 陈旧降权）
pub fn compute_scalar_modulation(asset: &AssetRecord) -> f64 {
    let imp = asset
        .desc_kv
        .get("importance_score")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5)
        .clamp(0.0, 1.0);

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let recency_bonus = asset
        .desc_kv
        .get("mtime_secs")
        .and_then(|v| v.as_u64())
        .map(|mt| {
            let age_days = (now_secs.saturating_sub(mt) as f64) / 86400.0;
            0.12 * (-age_days / 30.0).exp()
        })
        .unwrap_or(0.03);

    let auth_bonus = if asset.desc_kv.get("doc_category").and_then(|v| v.as_str()) == Some("authority") {
        0.08
    } else {
        0.0
    };

    let stale_penalty = if asset.desc_kv.get("embedding_status").and_then(|v| v.as_str()) == Some("stale") {
        0.05
    } else {
        0.0
    };

    (0.75 + 0.25 * imp + recency_bonus + auth_bonus - stale_penalty).clamp(0.4, 1.4)
}

/// 资产 Omni-RRF 混合检索引擎（BM25/FTS5 + Fuzzy + Vector + Graph Know-How + Hierarchy + Doc Links + Scalar）
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

    /// 结构化事实标量过滤校验（`modality` / `domain` / `kv_key` / `kv_value`）
    pub fn matches_filters(asset: &AssetRecord, opts: &AssetSearchOptions) -> bool {
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

        true
    }

    fn build_searchable_haystack(asset: &AssetRecord) -> String {
        let mut parts = vec![asset.path.clone()];
        // 将符号、章节与视觉标题前置，确保长描述不会将代码标识符挤出编辑距离窗口
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
        for key in [
            "purpose",
            "entry_point",
            "visual_summary",
            "pdf_title",
            "doc_title",
            "pdf_excerpt",
        ] {
            if let Some(s) = asset.desc_kv.get(key).and_then(|v| v.as_str()) {
                parts.push(s.to_string());
            }
        }
        parts.join(" ")
    }

    /// 执行资产 Omni-RRF 搜索（自动检测 Ollama Embedding 在线态）
    pub fn search(
        storage: &dyn StorageProvider,
        opts: &AssetSearchOptions,
    ) -> Result<Vec<AssetSearchHit>> {
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

    /// 离线可测核心入口：多路通道 RRF 融合 + 自适应权重重归一化 + 标量调制
    pub fn search_with_query_vec(
        storage: &dyn StorageProvider,
        opts: &AssetSearchOptions,
        query_vec: Option<&[f32]>,
    ) -> Result<Vec<AssetSearchHit>> {
        let limit = opts.limit.clamp(1, 200);
        let q = opts.query.trim();

        // 空查询：纯事实标量过滤与重要度排序
        if q.is_empty() {
            let scan_limit = if opts.has_filters() { 5000 } else { limit };
            let assets = storage.list_assets(scan_limit)?;
            let mut hits = Vec::new();
            for a in assets {
                if Self::matches_filters(&a, opts) {
                    let mod_f = compute_scalar_modulation(&a);
                    hits.push(AssetSearchHit {
                        asset: a,
                        score: mod_f,
                        matched_channels: vec!["filter".to_string()],
                        matched_sections: Vec::new(),
                        reasoning_paths: Vec::new(),
                    });
                    if hits.len() >= limit {
                        break;
                    }
                }
            }
            return Ok(hits);
        }

        let all_candidates = storage.list_assets(5000)?;
        let by_path: HashMap<String, AssetRecord> = all_candidates
            .iter()
            .filter(|a| Self::matches_filters(a, opts))
            .map(|a| (a.path.clone(), a.clone()))
            .collect();

        // 1. 路 1: BM25 / FTS5 全文检索（含驼峰拆词）
        let fts_fetch = if opts.has_filters() {
            (limit * 10).clamp(100, 1000)
        } else {
            (limit * 5).clamp(50, 250)
        };
        let fts_raw = storage.search_assets_fts(q, fts_fetch).unwrap_or_default();
        let mut bm25_ranks: HashMap<String, usize> = HashMap::new();
        let mut fallback_records: HashMap<String, AssetRecord> = HashMap::new();
        for a in fts_raw {
            if !Self::matches_filters(&a, opts) {
                continue;
            }
            let r = bm25_ranks.len();
            bm25_ranks.entry(a.path.clone()).or_insert(r);
            fallback_records.entry(a.path.clone()).or_insert(a);
        }

        // 2. 路 2: Pure-Rust Fuzzy 模糊查询通道
        let mut fuzzy_ranks: HashMap<String, usize> = HashMap::new();
        let mut fuzzy_strong: HashSet<String> = HashSet::new();
        if opts.fuzzy {
            let mut fz_scored: Vec<(String, f64)> = Vec::new();
            for (p, a) in &by_path {
                let hay = Self::build_searchable_haystack(a);
                let sc = fuzzy_text_score(q, &hay);
                if sc >= 0.70 {
                    fuzzy_strong.insert(p.clone());
                }
                if sc >= 0.30 {
                    fz_scored.push((p.clone(), sc));
                }
            }
            fz_scored.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
            for (rank, (p, _)) in fz_scored.into_iter().take((limit * 5).max(50)).enumerate() {
                fuzzy_ranks.insert(p, rank);
            }
        }

        // 3. 路 3: 可选 Semantic Vector 通道
        let mut vec_ranks: HashMap<String, usize> = HashMap::new();
        if let Some(qvec) = query_vec {
            let _wait = WaitEventGuard::enter(WaitEventId::CpuVectorCosine);
            let mut vec_scored: Vec<(String, f32)> = Vec::new();
            for (p, a) in &by_path {
                if a.desc_kv.get("embedding_status").and_then(|v| v.as_str()) != Some("ready") {
                    continue;
                }
                if let Ok(vec) = storage.get_asset_embedding(p) {
                    if vec.len() == qvec.len() && !vec.is_empty() {
                        let sim = cosine_similarity(qvec, &vec);
                        if sim > 0.05 {
                            vec_scored.push((p.clone(), sim));
                        }
                    }
                }
            }
            vec_scored.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
            for (rank, (p, _)) in vec_scored.into_iter().take((limit * 4).max(40)).enumerate() {
                vec_ranks.insert(p, rank);
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
                .filter(|(p, _)| by_path.contains_key(p))
                .collect();
            g_scored.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
            for (rank, (p, _)) in g_scored.into_iter().take((limit * 5).max(50)).enumerate() {
                graph_ranks.insert(p, rank);
            }
        }

        // 5. 路 5: 层级结构通道（同章节聚合 + 目录局部性）
        let mut hierarchy_ranks: HashMap<String, usize> = HashMap::new();
        if opts.include_hierarchy {
            let mut dir_votes: HashMap<String, f64> = HashMap::new();
            for p in bm25_ranks.keys().chain(graph_ranks.keys()) {
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
            for p in by_path.keys() {
                let sec_cnt = matched_sections_map.get(p).map(|v| v.len()).unwrap_or(0) as f64;
                let dir_bonus = Path::new(p)
                    .parent()
                    .and_then(|d| dir_votes.get(&d.to_string_lossy().to_string()).copied())
                    .unwrap_or(0.0);
                // 仅有目录亲和且本身已在任意搜索通道出现过、或有章节聚合时计入
                let has_base_hit =
                    bm25_ranks.contains_key(p) || fuzzy_ranks.contains_key(p) || graph_ranks.contains_key(p);
                let total_h = sec_cnt * 0.8 + if has_base_hit { dir_bonus * 0.25 } else { 0.0 };
                if total_h > 0.0 {
                    h_scored.push((p.clone(), total_h));
                }
            }
            h_scored.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
            for (rank, (p, _)) in h_scored.into_iter().take((limit * 4).max(40)).enumerate() {
                hierarchy_ranks.insert(p, rank);
            }
        }

        // 6. 路 6: 文档间显式互链通道 (`links` 表 1-hop)
        let mut doc_link_ranks: HashMap<String, usize> = HashMap::new();
        if opts.include_links {
            let mut anchor_set: HashSet<String> = HashSet::new();
            for (p, &r) in &bm25_ranks {
                if r < 6 {
                    anchor_set.insert(p.clone());
                }
            }
            for (p, &r) in &fuzzy_ranks {
                if r < 4 {
                    anchor_set.insert(p.clone());
                }
            }
            for (p, &r) in &graph_ranks {
                if r < 4 {
                    anchor_set.insert(p.clone());
                }
            }
            if !anchor_set.is_empty() {
                if let Ok(all_links) = storage.list_all_links(3000) {
                    let mut link_scores: HashMap<String, f64> = HashMap::new();
                    for l in all_links {
                        if anchor_set.contains(&l.source_id)
                            && !anchor_set.contains(&l.target_id)
                            && by_path.contains_key(&l.target_id)
                        {
                            *link_scores.entry(l.target_id.clone()).or_insert(0.0) += l.confidence as f64;
                            let rps = reasoning_paths_map.entry(l.target_id.clone()).or_default();
                            if rps.len() < 2 {
                                rps.push(format!("{} -[{}]-> {}", l.source_id, l.relation, l.target_id));
                            }
                        } else if anchor_set.contains(&l.target_id)
                            && !anchor_set.contains(&l.source_id)
                            && by_path.contains_key(&l.source_id)
                        {
                            *link_scores.entry(l.source_id.clone()).or_insert(0.0) +=
                                (l.confidence as f64) * 0.85;
                            let rps = reasoning_paths_map.entry(l.source_id.clone()).or_default();
                            if rps.len() < 2 {
                                rps.push(format!("{} <-[{}]- {}", l.source_id, l.relation, l.target_id));
                            }
                        }
                    }
                    let mut ls: Vec<(String, f64)> = link_scores.into_iter().collect();
                    ls.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
                    for (rank, (p, _)) in ls.into_iter().take((limit * 3).max(30)).enumerate() {
                        doc_link_ranks.insert(p, rank);
                    }
                }
            }
        }

        let fuzzy_w = if bm25_ranks.is_empty() { 1.25 } else { 0.75 };
        // 7. 自适应权重重归一化 & Omni-RRF 融合
        let mut active_w = 0.0f64;
        if !bm25_ranks.is_empty() {
            active_w += 1.20;
        }
        if !fuzzy_ranks.is_empty() {
            active_w += fuzzy_w;
        }
        if !vec_ranks.is_empty() {
            active_w += 1.00;
        }
        if !graph_ranks.is_empty() {
            active_w += 1.10;
        }
        if !hierarchy_ranks.is_empty() {
            active_w += 0.60;
        }
        if !doc_link_ranks.is_empty() {
            active_w += 0.65;
        }
        let norm_div = if active_w > 0.0 { active_w } else { 1.0 };

        let mut all_hit_paths: HashSet<String> = HashSet::new();
        for map in [
            &bm25_ranks,
            &fuzzy_ranks,
            &vec_ranks,
            &graph_ranks,
            &hierarchy_ranks,
            &doc_link_ranks,
        ] {
            for p in map.keys() {
                all_hit_paths.insert(p.clone());
            }
        }

        let k = 60.0f64;
        let mut ranked: Vec<AssetSearchHit> = Vec::with_capacity(all_hit_paths.len());
        for p in all_hit_paths {
            let Some(asset) = by_path.get(&p).or_else(|| fallback_records.get(&p)) else {
                continue;
            };
            let mut rrf_sum = 0.0f64;
            let mut channels = Vec::new();
            if let Some(&r) = bm25_ranks.get(&p) {
                rrf_sum += (1.20 / norm_div) / (k + r as f64);
                channels.push("bm25_fts".to_string());
            }
            if let Some(&r) = fuzzy_ranks.get(&p) {
                let eff_fuzzy_w = if bm25_ranks.is_empty() && fuzzy_strong.contains(&p) {
                    fuzzy_w + 1.15
                } else {
                    fuzzy_w
                };
                rrf_sum += (eff_fuzzy_w / norm_div) / (k + r as f64);
                channels.push("fuzzy".to_string());
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
            ranked.push(AssetSearchHit {
                asset: asset.clone(),
                score: final_score,
                matched_channels: channels,
                matched_sections: matched_sections_map.get(&p).cloned().unwrap_or_default(),
                reasoning_paths: reasoning_paths_map.get(&p).cloned().unwrap_or_default(),
            });
        }

        ranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.asset.path.cmp(&b.asset.path))
        });
        ranked.truncate(limit);
        Ok(ranked)
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
                if let Some(sec) = h.matched_sections.first() {
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
        storage
            .update_asset_embedding(&img.path, &[1.0, 0.0, 0.0])
            .expect("embed img");
        storage
            .update_asset_embedding(&code.path, &[0.0, 1.0, 0.0])
            .expect("embed code");

        let opts_vec = AssetSearchOptions::new("system topology diagram", 5);
        let hits = AssetSearchEngine::search_with_query_vec(
            storage.provider().as_ref(),
            &opts_vec,
            Some(&[0.98, 0.02, 0.0]),
        )
        .expect("vec search");
        assert!(!hits.is_empty());
        assert_eq!(hits[0].asset.path, "/tmp/test/arch_diagram.png");
        assert!(hits[0].matched_channels.contains(&"vec".to_string()));

        let mut opts_mod = AssetSearchOptions::new("", 10);
        opts_mod.modality = Some("dataset".to_string());
        let db_hits =
            AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts_mod, None)
                .expect("mod search");
        assert_eq!(db_hits.len(), 1);
        assert_eq!(db_hits[0].asset.path, "/tmp/test/analytics.db");

        let mut opts_kv = AssetSearchOptions::new("analytics", 10);
        opts_kv.kv_key = Some("table_names".to_string());
        opts_kv.kv_value = Some("orders".to_string());
        let kv_hits =
            AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts_kv, None)
                .expect("kv search");
        assert_eq!(kv_hits.len(), 1);
        assert_eq!(kv_hits[0].asset.path, "/tmp/test/analytics.db");
    }

    #[test]
    fn test_omni_rrf_camel_fuzzy_doc_links_and_graph_recall() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let dir = tempfile::tempdir().expect("tempdir");

        // 1. 含驼峰标识符的代码资产
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

        // 2. 两篇真实 Markdown（通过图谱 2 跳连接）+ 一篇通过 links 表显式指向的架构设计
        let doc1 = dir.path().join("wal_spike_root.md");
        let doc2 = dir.path().join("wal_mitigation.md");
        let linked_spec = dir.path().join("wal_spec_diagram.svg");
        std::fs::write(
            &doc1,
            "# WAL 写放大\nfull_page_writes 触发 checkpoint_spike。\n",
        )
        .expect("doc1");
        std::fs::write(
            &doc2,
            "# WAL 参数调优\n调大 max_wal_size 缓解 checkpoint_spike 问题。\n",
        )
        .expect("doc2");

        for (p, t, kv) in [
            (
                doc1.to_string_lossy().to_string(),
                "document",
                json!({"modality": "document", "importance_score": 0.8}),
            ),
            (
                doc2.to_string_lossy().to_string(),
                "document",
                json!({"modality": "document", "importance_score": 0.8}),
            ),
            (
                linked_spec.to_string_lossy().to_string(),
                "media",
                json!({"modality": "image", "importance_score": 0.6}),
            ),
        ] {
            storage
                .upsert_asset(&sample_asset(&p, t, "knowledge doc", kv))
                .expect("upsert");
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
        let camel_hits =
            AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts_camel, None)
                .expect("camel search");
        assert!(!camel_hits.is_empty());
        assert_eq!(camel_hits[0].asset.path, "/tmp/omni/runtime_ab.rs");
        assert!(camel_hits[0].matched_channels.contains(&"bm25_fts".to_string()));

        // B. 拼写误差模糊检索：ShadwAbEngine -> Fuzzy 兜底找回 runtime_ab.rs
        let mut opts_typo = AssetSearchOptions::new("ShadwAbEngine", 5);
        opts_typo.hybrid = false;
        let typo_hits =
            AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts_typo, None)
                .expect("typo search");
        assert!(
            typo_hits.iter().any(|h| h.asset.path == "/tmp/omni/runtime_ab.rs"),
            "Fuzzy 通道应容错召回 ShadowAbEngine"
        );

        // C. 离线跨词检索 full_page_writes：图谱 2 跳召回 doc2，文档互链召回 linked_spec
        let mut opts_graph = AssetSearchOptions::new("full_page_writes", 10);
        opts_graph.hybrid = false;
        opts_graph.graph_hops = 2;
        let g_hits =
            AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts_graph, None)
                .expect("omni graph search");
        let doc2_str = doc2.to_string_lossy().to_string();
        let spec_str = linked_spec.to_string_lossy().to_string();
        assert!(
            g_hits.iter().any(|h| h.asset.path == doc2_str),
            "Graph 通道应从 full_page_writes 召回 doc2: {:?}",
            g_hits.iter().map(|h| (&h.asset.path, &h.matched_channels)).collect::<Vec<_>>()
        );
        assert!(
            g_hits.iter().any(|h| h.asset.path == spec_str && h.matched_channels.contains(&"doc_links".to_string())),
            "DocLinks 通道应将 doc1 的引用扩展召回到 linked_spec"
        );
    }
}
