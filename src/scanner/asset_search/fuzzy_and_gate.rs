use std::collections::{HashMap, HashSet};
use crate::storage::models::AssetRecord;
use crate::fact::knowledge_graph::{tokenize_terms, AliasCanonicalizer};

/// 查询意图分类：严格代码标识符/路径 vs 自然语言语义描述 (ISSUE #56)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryIntent {
    StrictIdentifier,
    NaturalLanguage,
}

/// 识别查询词是否属于严格标识符（驼峰/蛇形/模块路径/带扩展名文件名）
pub fn classify_query_intent(query: &str) -> QueryIntent {
    let q = query.trim();
    if q.is_empty() || q.contains(char::is_whitespace) {
        return QueryIntent::NaturalLanguage;
    }
    let has_cjk = q.chars().any(|c| ('\u{4E00}'..='\u{9FFF}').contains(&c));
    if has_cjk {
        return QueryIntent::NaturalLanguage;
    }
    let has_ident_sep = q.contains('_') || q.contains("::") || q.contains('/') || q.contains('.');
    let has_camel = q.chars().any(|c| c.is_ascii_lowercase())
        && q.chars().skip(1).any(|c| c.is_ascii_uppercase());
    if (has_ident_sep || has_camel) && q.len() >= 4 {
        QueryIntent::StrictIdentifier
    } else {
        QueryIntent::NaturalLanguage
    }
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

pub(super) struct PreparedFuzzyQuery {
    pub(super) q_lower: String,
    pub(super) is_code_identifier: bool,
    pub(super) q_terms: Vec<String>,
    pub(super) q_words: Vec<Vec<char>>,
    pub(super) q_bigrams: HashSet<(char, char)>,
}

impl PreparedFuzzyQuery {
    pub(super) fn new(query: &str) -> Self {
        let raw_trimmed = query.trim();
        let q_lower = raw_trimmed.to_ascii_lowercase();
        let is_code_identifier = classify_query_intent(raw_trimmed) == QueryIntent::StrictIdentifier;
        let aliases = AliasCanonicalizer::default();
        let q_terms = if q_lower.is_empty() {
            Vec::new()
        } else {
            tokenize_terms(&q_lower, &aliases)
        };
        let q_words = q_lower
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|w| w.len() >= 4)
            .map(|w| w.chars().collect())
            .collect();
        let q_bigrams = char_bigrams(&q_lower);
        Self {
            q_lower,
            is_code_identifier,
            q_terms,
            q_words,
            q_bigrams,
        }
    }
}

pub(super) fn fuzzy_text_score_prepared(prep: &PreparedFuzzyQuery, haystack: &str) -> f64 {
    if prep.q_lower.is_empty() || haystack.trim().is_empty() {
        return 0.0;
    }
    let hay_lower = haystack.to_ascii_lowercase();
    let exact_hit = hay_lower.contains(&prep.q_lower);
    let mut score = 0.0f64;

    if exact_hit {
        score += 0.65;
    }

    let mut matched_terms = 0usize;
    if !prep.q_terms.is_empty() {
        for t in &prep.q_terms {
            if hay_lower.contains(t) {
                matched_terms += 1;
            }
        }
        if prep.is_code_identifier && !exact_hit && matched_terms < 2 && prep.q_terms.len() >= 2 {
            score += 0.08 * (matched_terms as f64 / prep.q_terms.len() as f64);
        } else {
            score += 0.35 * (matched_terms as f64 / prep.q_terms.len() as f64);
        }
    }

    let hay_words: Vec<&str> = hay_lower
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.len() >= 4)
        .take(128)
        .collect();
    for qc in &prep.q_words {
        let tol = if qc.len() >= 8 { 2 } else { 1 };
        let mut best_edit_bonus = 0.0f64;
        for hw in &hay_words {
            let hc: Vec<char> = hw.chars().collect();
            if &hc == qc {
                best_edit_bonus = 0.65;
                break;
            }
            if hw.len() >= qc.len() && hc.windows(qc.len()).any(|w| w == qc.as_slice()) {
                best_edit_bonus = best_edit_bonus.max(0.55);
                continue;
            }
            if let Some(d) = bounded_levenshtein(qc, &hc, tol) {
                let ratio = 1.0 - (d as f64 / qc.len().max(hc.len()) as f64);
                let typo_scale = if qc.len() >= 8 { 1.15 } else { 0.72 };
                best_edit_bonus = best_edit_bonus.max(typo_scale * ratio);
            }
        }
        score += best_edit_bonus;
    }

    if !prep.q_bigrams.is_empty() {
        let hb = char_bigrams(&hay_lower.chars().take(800).collect::<String>());
        if !hb.is_empty() {
            let inter = prep.q_bigrams.intersection(&hb).count() as f64;
            let jaccard = inter / prep.q_bigrams.len().max(1) as f64;
            if jaccard >= 0.35 {
                score += 0.30 * jaccard.min(1.0);
            }
        }
    }

    if prep.is_code_identifier && !exact_hit && matched_terms < 2 && prep.q_terms.len() >= 2 {
        score *= 0.35;
    }

    score
}

/// 纯 Rust Fuzzy 模糊匹配打分（子串/前缀 + 字符 bigram Jaccard + 有界 Levenshtein 容错）
pub fn fuzzy_text_score(query: &str, haystack: &str) -> f64 {
    let prep = PreparedFuzzyQuery::new(query);
    fuzzy_text_score_prepared(&prep, haystack)
}

/// 事实标量软调制因子 (重要度 + 30天近因衰减 + 权威清单 + 陈旧降权)
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

/// 精准度与超集无冗余后置门禁 (ISSUE #56):
/// - 保证确定性通道 (`exact_id` / `db_fuzzy_*` / `bm25_fts`) 绝对不漏、始终置顶 (无缺失)；
/// - 阻止弱相似纯向量 (`vec`-only) 或泛词误召回填充结果集 (无多余)；
pub(super) fn apply_precision_gate(
    hits: Vec<super::AssetSearchHit>,
    vec_sims: &HashMap<String, f32>,
    intent: QueryIntent,
    strict_identifier_gate: bool,
    min_vec_sim: f32,
    limit: usize,
) -> Vec<super::AssetSearchHit> {
    let has_deterministic = hits.iter().any(|h| h.match_tier == "deterministic");
    let top_vec_sim = vec_sims.values().copied().fold(0.0f32, f32::max);
    let rel_floor = (top_vec_sim - 0.12).max(min_vec_sim);

    let mut det_hits = Vec::new();
    let mut exp_hits = Vec::new();

    for hit in hits {
        if hit.match_tier == "deterministic" {
            det_hits.push(hit);
            continue;
        }
        let is_pure_vec = hit.matched_channels.len() == 1 && hit.matched_channels[0] == "vec";
        if is_pure_vec {
            if intent == QueryIntent::StrictIdentifier && strict_identifier_gate && has_deterministic {
                continue;
            }
            let sim = vec_sims.get(&hit.asset.path).copied().unwrap_or(0.0);
            if sim < rel_floor {
                continue;
            }
            if has_deterministic && sim < min_vec_sim.max(0.62) {
                continue;
            }
        }
        exp_hits.push(hit);
    }

    det_hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.asset.path.cmp(&b.asset.path))
    });
    exp_hits.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.asset.path.cmp(&b.asset.path))
    });

    if has_deterministic {
        let remaining_slots = limit.saturating_sub(det_hits.len()).min(5);
        exp_hits.truncate(remaining_slots);
    }
    let mut out = det_hits;
    out.extend(exp_hits);
    out.truncate(limit);
    out
}
