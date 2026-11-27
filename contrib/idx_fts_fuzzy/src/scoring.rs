//! Pure-Rust 标识符拆词、BM25 打分、字符 Bigram Jaccard 与有界 Levenshtein 编辑距离算法

use std::collections::{BTreeMap, BTreeSet};

/// 拆分驼峰（如 `ShadowAbEngine` -> `shadow ab engine`）、蛇形（`foo_bar`）及 CJK 词项
pub fn split_identifier_terms(text: &str) -> Vec<String> {
    let mut terms = Vec::new();
    let mut ascii_run = String::new();
    let mut cjk_run = String::new();

    for ch in text.chars() {
        if is_cjk_char(ch) {
            flush_ascii_run(&mut ascii_run, &mut terms);
            cjk_run.push(ch);
        } else if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            flush_cjk_run(&mut cjk_run, &mut terms);
            ascii_run.push(ch);
        } else {
            flush_ascii_run(&mut ascii_run, &mut terms);
            flush_cjk_run(&mut cjk_run, &mut terms);
        }
    }
    flush_ascii_run(&mut ascii_run, &mut terms);
    flush_cjk_run(&mut cjk_run, &mut terms);
    terms
}

/// 将任意文本规范化为空格分隔的小写词串（例如 `ShadowAbEngine` -> `"shadow ab engine"`）
pub fn normalize_identifier_phrase(text: &str) -> String {
    split_identifier_terms(text).join(" ")
}

/// 提取候选原始词（用于错别字编辑距离与 Bigram 比较，保留复合标识符与拆词两级粒度）
pub fn extract_candidate_words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut seen = BTreeSet::new();

    for raw in text.split(|c: char| !c.is_alphanumeric() && c != '_') {
        let trimmed = raw.trim_matches('_').to_ascii_lowercase();
        if !trimmed.is_empty() && seen.insert(trimmed.clone()) {
            words.push(trimmed);
        }
    }
    for term in split_identifier_terms(text) {
        if !term.is_empty() && seen.insert(term.clone()) {
            words.push(term);
        }
    }
    words
}

/// 提取字符 Bigram 集合
pub fn char_bigrams(text: &str) -> BTreeSet<(char, char)> {
    let chars: Vec<char> = text.chars().collect();
    let mut set = BTreeSet::new();
    for pair in chars.windows(2) {
        set.insert((pair[0], pair[1]));
    }
    set
}

/// 计算字符 Bigram Jaccard 相似度 (`|A ∩ B| / |A ∪ B|`)
pub fn bigram_jaccard(a: &str, b: &str) -> f64 {
    let set_a = char_bigrams(a);
    let set_b = char_bigrams(b);
    if set_a.is_empty() || set_b.is_empty() {
        return 0.0;
    }
    let inter = set_a.intersection(&set_b).count() as f64;
    let union = set_a.union(&set_b).count() as f64;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

/// 有界 Levenshtein 编辑距离：若超出 `max_dist` 提前剪枝返回 `None`
pub fn bounded_levenshtein(a: &[char], b: &[char], max_dist: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > max_dist {
        return None;
    }
    if a.is_empty() {
        return (b.len() <= max_dist).then_some(b.len());
    }
    if b.is_empty() {
        return (a.len() <= max_dist).then_some(a.len());
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
            let val = del.min(ins).min(sub);
            curr[j + 1] = val;
            if val < row_min {
                row_min = val;
            }
        }
        if row_min > max_dist {
            return None;
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    let final_dist = prev[b.len()];
    (final_dist <= max_dist).then_some(final_dist)
}

/// 词长 `>= 4` 的容差策略（严格限制容差 `<= 2`）
pub fn edit_tolerance_for_len(char_len: usize) -> Option<usize> {
    if char_len < 4 {
        None
    } else if char_len >= 6 {
        Some(2)
    } else {
        Some(1)
    }
}

/// BM25 语料库统计摘要
pub struct Bm25Corpus {
    pub doc_count: usize,
    pub avg_doc_len: f64,
    pub df: BTreeMap<String, usize>,
}

impl Bm25Corpus {
    pub fn build(doc_term_lists: &[Vec<String>]) -> Self {
        let doc_count = doc_term_lists.len();
        let mut total_len = 0usize;
        let mut df: BTreeMap<String, usize> = BTreeMap::new();

        for terms in doc_term_lists {
            total_len += terms.len();
            let unique: BTreeSet<&str> = terms.iter().map(String::as_str).collect();
            for t in unique {
                *df.entry(t.to_string()).or_insert(0) += 1;
            }
        }
        let avg_doc_len = if doc_count == 0 {
            1.0
        } else {
            (total_len as f64 / doc_count as f64).max(1.0)
        };
        Self {
            doc_count,
            avg_doc_len,
            df,
        }
    }

    /// 计算单个文档针对查询词列表的 Okapi BM25 得分 (`k1 = 1.2, b = 0.75`)
    pub fn score(&self, query_terms: &[String], doc_terms: &[String]) -> f64 {
        if query_terms.is_empty() || doc_terms.is_empty() || self.doc_count == 0 {
            return 0.0;
        }
        let k1 = 1.2_f64;
        let b = 0.75_f64;
        let dl = doc_terms.len() as f64;
        let n = self.doc_count as f64;

        let mut tf_map: BTreeMap<&str, usize> = BTreeMap::new();
        for t in doc_terms {
            *tf_map.entry(t.as_str()).or_insert(0) += 1;
        }

        let unique_q: BTreeSet<&str> = query_terms.iter().map(String::as_str).collect();
        let mut total = 0.0_f64;
        for qt in unique_q {
            let tf = tf_map.get(qt).copied().unwrap_or(0) as f64;
            if tf <= 0.0 {
                continue;
            }
            let doc_freq = self.df.get(qt).copied().unwrap_or(0) as f64;
            let idf = (((n - doc_freq + 0.5) / (doc_freq + 0.5)) + 1.0).ln();
            let denom = tf + k1 * (1.0 - b + b * (dl / self.avg_doc_len));
            total += idf * ((tf * (k1 + 1.0)) / denom);
        }
        total
    }
}

/// 综合计算查询与候选文档的多策略得分（返回 `(score, reasoning_path)`）
pub fn score_document(
    query: &str,
    doc_text: &str,
    doc_terms: &[String],
    corpus: &Bm25Corpus,
    strategy_num: u16,
) -> (f64, Option<String>) {
    let q_trimmed = query.trim();
    if q_trimmed.is_empty() || doc_text.trim().is_empty() {
        return (0.0, None);
    }

    let q_lower = q_trimmed.to_ascii_lowercase();
    let doc_lower = doc_text.to_ascii_lowercase();
    let q_terms = split_identifier_terms(q_trimmed);
    let q_phrase = q_terms.join(" ");
    let doc_phrase = doc_terms.join(" ");

    let mut total_score = 0.0_f64;
    let mut reasons = Vec::new();

    if doc_lower.contains(&q_lower) {
        total_score += 2.0;
        reasons.push("exact_substr");
    } else if !q_phrase.is_empty() && doc_phrase.contains(&q_phrase) {
        total_score += 1.5;
        reasons.push("split_phrase");
    }

    let bm25 = corpus.score(&q_terms, doc_terms);
    if bm25 > 0.0 {
        total_score += bm25;
        reasons.push("bm25");
    }

    if strategy_num == 2 {
        let prefix_hits = q_terms
            .iter()
            .filter(|qt| doc_terms.iter().any(|dt| dt.starts_with(qt.as_str())))
            .count();
        if prefix_hits > 0 {
            total_score += 0.8 * (prefix_hits as f64 / q_terms.len().max(1) as f64);
            reasons.push("prefix");
        }
        return (
            total_score,
            (!reasons.is_empty()).then(|| reasons.join("+")),
        );
    }

    if strategy_num == 1 {
        return (
            total_score,
            (!reasons.is_empty()).then(|| reasons.join("+")),
        );
    }

    // Fuzzy 策略：叠加字符 Bigram Jaccard 与词长 >= 4 的有界 Levenshtein 容错
    let doc_words = extract_candidate_words(doc_text);
    let q_words = extract_candidate_words(q_trimmed);

    let mut best_jaccard = bigram_jaccard(&q_lower, &doc_lower);
    if !q_phrase.is_empty() {
        best_jaccard = best_jaccard.max(bigram_jaccard(&q_phrase, &doc_phrase));
    }
    for qw in &q_words {
        for dw in &doc_words {
            best_jaccard = best_jaccard.max(bigram_jaccard(qw, dw));
        }
    }
    if best_jaccard >= 0.30 {
        total_score += 0.9 * best_jaccard;
        reasons.push("bigram_jaccard");
    }

    let mut edit_bonus = 0.0_f64;
    for qw in &q_words {
        let qc: Vec<char> = qw.chars().collect();
        let Some(tol) = edit_tolerance_for_len(qc.len()) else {
            continue;
        };
        let mut word_best = 0.0_f64;
        for dw in &doc_words {
            let dc: Vec<char> = dw.chars().collect();
            if dc.len() < 4 {
                continue;
            }
            if let Some(dist) = bounded_levenshtein(&qc, &dc, tol) {
                let max_len = qc.len().max(dc.len()) as f64;
                let sim = 1.0 - (dist as f64 / max_len);
                word_best = word_best.max(1.1 * sim);
            }
        }
        edit_bonus += word_best;
    }
    if edit_bonus > 0.0 {
        total_score += edit_bonus;
        reasons.push("levenshtein");
    }

    (
        total_score,
        (!reasons.is_empty()).then(|| reasons.join("+")),
    )
}

fn is_cjk_char(ch: char) -> bool {
    matches!(
        ch as u32,
        0x4E00..=0x9FFF | 0x3400..=0x4DBF | 0xF900..=0xFAFF
    )
}

fn flush_cjk_run(buf: &mut String, out: &mut Vec<String>) {
    if buf.is_empty() {
        return;
    }
    let chars: Vec<char> = buf.chars().collect();
    for &c in &chars {
        out.push(c.to_string());
    }
    for pair in chars.windows(2) {
        let mut s = String::with_capacity(8);
        s.push(pair[0]);
        s.push(pair[1]);
        out.push(s);
    }
    buf.clear();
}

fn flush_ascii_run(buf: &mut String, out: &mut Vec<String>) {
    if buf.is_empty() {
        return;
    }
    for segment in buf.split(['_', '-']) {
        if segment.is_empty() {
            continue;
        }
        split_camel_segment(segment, out);
    }
    buf.clear();
}

fn split_camel_segment(seg: &str, out: &mut Vec<String>) {
    let chars: Vec<char> = seg.chars().collect();
    let mut start = 0usize;

    for i in 1..chars.len() {
        let prev = chars[i - 1];
        let curr = chars[i];
        let next_is_lower = chars.get(i + 1).is_some_and(|c| c.is_ascii_lowercase());

        let boundary = (prev.is_ascii_lowercase() && curr.is_ascii_uppercase())
            || (prev.is_ascii_digit() && curr.is_ascii_uppercase())
            || (prev.is_ascii_uppercase() && curr.is_ascii_uppercase() && next_is_lower);

        if boundary {
            let piece: String = chars[start..i].iter().collect();
            push_lower_piece(&piece, out);
            start = i;
        }
    }

    let tail: String = chars[start..].iter().collect();
    push_lower_piece(&tail, out);
}

fn push_lower_piece(piece: &str, out: &mut Vec<String>) {
    let lower = piece.to_ascii_lowercase();
    if !lower.is_empty() {
        out.push(lower);
    }
}
