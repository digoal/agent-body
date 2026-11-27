use std::collections::HashMap;
use std::path::Path;
use crate::scanner::redactor::{PromptInjectionScanner, SecretRedactor};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Heading-Aware 文档语义章节切片
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadingChunk {
    pub section_id: String,
    pub parent_section_id: Option<String>,
    pub depth: usize,
    pub heading_path: String,
    pub line_range: (usize, usize),
    pub text: String,
    pub preview: String,
}

/// Markdown / 结构化文档章节切分器（避免硬截断造成前提/因果失真）
pub struct HeadingChunker;

impl HeadingChunker {
    pub fn chunk_markdown(source_path: &str, raw_content: &str, max_chars: usize) -> Vec<HeadingChunk> {
        let cap = max_chars.clamp(200, 8000);
        let overlap = 100usize.min(cap / 4);
        let redacted = SecretRedactor::redact(raw_content);
        let sanitized = PromptInjectionScanner::scan_and_sanitize(&redacted).sanitized;

        struct SectionDraft {
            depth: usize,
            heading_path: String,
            parent_idx: Option<usize>,
            start_line: usize,
            end_line: usize,
            lines: Vec<String>,
        }

        let base_name = Path::new(source_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("doc");

        let mut stack: Vec<(usize, String, usize)> = Vec::new();
        let mut drafts: Vec<SectionDraft> = Vec::new();
        let mut current = SectionDraft {
            depth: 0,
            heading_path: base_name.to_string(),
            parent_idx: None,
            start_line: 1,
            end_line: 1,
            lines: Vec::new(),
        };
        let mut in_fence = false;

        for (idx, line) in sanitized.lines().enumerate() {
            let line_no = idx + 1;
            let trimmed = line.trim_start();
            if trimmed.starts_with("```") {
                in_fence = !in_fence;
                current.end_line = line_no;
                current.lines.push(line.to_string());
                continue;
            }
            if !in_fence && trimmed.starts_with('#') {
                let hashes = trimmed.chars().take_while(|&c| c == '#').count();
                let after = &trimmed[hashes..];
                if (1..=3).contains(&hashes) && after.starts_with(' ') {
                    let title = after.trim().to_string();
                    if !title.is_empty() {
                        if !current.lines.is_empty() || current.depth > 0 {
                            drafts.push(current);
                        }
                        while let Some((d, _, _)) = stack.last() {
                            if *d >= hashes {
                                stack.pop();
                            } else {
                                break;
                            }
                        }
                        let parent_idx = stack.last().map(|(_, _, di)| *di);
                        let mut crumbs = vec![base_name.to_string()];
                        for (_, t, _) in &stack {
                            crumbs.push(t.clone());
                        }
                        crumbs.push(title.clone());
                        let next_draft_idx = drafts.len();
                        stack.push((hashes, title, next_draft_idx));
                        current = SectionDraft {
                            depth: hashes,
                            heading_path: crumbs.join(" > "),
                            parent_idx,
                            start_line: line_no,
                            end_line: line_no,
                            lines: Vec::new(),
                        };
                        continue;
                    }
                }
            }
            current.end_line = line_no;
            current.lines.push(line.to_string());
        }
        if !current.lines.is_empty() || drafts.is_empty() {
            drafts.push(current);
        }

        // 先为每篇 draft 分配主 section_id（供子章节建立父子联系），超大章节再拆滑动窗口
        let mut primary_ids: Vec<String> = Vec::with_capacity(drafts.len());
        let mut out: Vec<HeadingChunk> = Vec::new();
        for (idx, d) in drafts.iter().enumerate() {
            let parent_id = d
                .parent_idx
                .and_then(|pi| primary_ids.get(pi).cloned());
            let body_full = d.lines.join("\n").trim().to_string();
            let base_id = make_section_id(source_path, &d.heading_path, idx, 0);
            primary_ids.push(base_id.clone());

            if body_full.is_empty() && d.depth == 0 {
                continue;
            }
            let chars: Vec<char> = body_full.chars().collect();
            if chars.len() <= cap {
                let preview = make_preview(&d.heading_path, &body_full);
                out.push(HeadingChunk {
                    section_id: base_id,
                    parent_section_id: parent_id,
                    depth: d.depth,
                    heading_path: d.heading_path.clone(),
                    line_range: (d.start_line, d.end_line.max(d.start_line)),
                    text: body_full,
                    preview,
                });
            } else {
                let step = cap.saturating_sub(overlap).max(100);
                let mut offset = 0usize;
                let mut part = 0usize;
                while offset < chars.len() {
                    let end = (offset + cap).min(chars.len());
                    let slice_str: String = chars[offset..end].iter().collect();
                    let sid = if part == 0 {
                        base_id.clone()
                    } else {
                        make_section_id(source_path, &d.heading_path, idx, part)
                    };
                    let heading_lbl = if part == 0 {
                        d.heading_path.clone()
                    } else {
                        format!("{} (part {})", d.heading_path, part + 1)
                    };
                    let preview = make_preview(&heading_lbl, &slice_str);
                    out.push(HeadingChunk {
                        section_id: sid,
                        parent_section_id: parent_id.clone(),
                        depth: d.depth,
                        heading_path: heading_lbl,
                        line_range: (d.start_line, d.end_line.max(d.start_line)),
                        text: slice_str,
                        preview,
                    });
                    if end >= chars.len() {
                        break;
                    }
                    offset += step;
                    part += 1;
                }
            }
        }
        out
    }
}

pub(crate) fn sha256_prefix_hex(input: &str, bytes_len: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    digest
        .iter()
        .take(bytes_len)
        .map(|b| format!("{:02x}", b))
        .collect()
}

pub(crate) fn make_section_id(path: &str, heading: &str, draft_idx: usize, part: usize) -> String {
    format!(
        "sec:{}",
        sha256_prefix_hex(&format!("{}|{}|{}|{}", path, heading, draft_idx, part), 8)
    )
}

pub(crate) fn make_preview(heading_path: &str, body: &str) -> String {
    let compact: String = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let snippet: String = compact.chars().take(120).collect();
    if snippet.is_empty() {
        heading_path.to_string()
    } else {
        format!("[{}] {}", heading_path, snippet)
    }
}

/// 行业术语别名归一化器（消除 `PG` / `PostgreSQL` 等多写法造成的图断链）
#[derive(Debug, Clone)]
pub struct AliasCanonicalizer {
    map: HashMap<String, String>,
}

impl Default for AliasCanonicalizer {
    fn default() -> Self {
        Self::new(&HashMap::new())
    }
}

impl AliasCanonicalizer {
    pub fn new(custom: &HashMap<String, String>) -> Self {
        let mut map = HashMap::new();
        let builtins = [
            ("pg", "postgresql"),
            ("postgres", "postgresql"),
            ("postgresql", "postgresql"),
            ("fts", "fts5"),
            ("fts5", "fts5"),
            ("vec", "embedding"),
            ("vector", "embedding"),
            ("vectors", "embedding"),
            ("embedding", "embedding"),
            ("embeddings", "embedding"),
            ("向量", "embedding"),
            ("词向量", "embedding"),
            ("rag", "graphrag"),
            ("graphrag", "graphrag"),
            ("sqlite", "sqlite"),
            ("sqlite3", "sqlite"),
            ("duckdb", "duckdb"),
            ("wal", "wal"),
            ("checkpoint", "checkpoint"),
            ("mvcc", "mvcc"),
            ("rrf", "rrf"),
            ("bm25", "bm25"),
            ("mcp", "mcp"),
            ("olap", "olap"),
            ("oltp", "oltp"),
            ("hbm", "hbm"),
            ("gpu", "gpu"),
            ("svg", "svg"),
            ("agent-body", "agent-body"),
            ("agent_body", "agent-body"),
            ("know-how", "knowhow"),
            ("knowhow", "knowhow"),
            ("aopcc", "aopcc"),
            ("shadow_ab", "shadow_ab"),
            ("shadow-ab", "shadow_ab"),
        ];
        for (k, v) in builtins {
            map.insert(k.to_string(), v.to_string());
        }
        for (k, v) in custom {
            let kk = k.trim().to_ascii_lowercase();
            let vv = v.trim().to_ascii_lowercase();
            if !kk.is_empty() && !vv.is_empty() {
                map.insert(kk, vv);
            }
        }
        Self { map }
    }

    pub fn canonicalize(&self, raw: &str) -> String {
        let lower = raw.trim().to_ascii_lowercase();
        if let Some(mapped) = self.map.get(&lower) {
            return mapped.clone();
        }
        lower
    }
}

fn is_stopword(term: &str) -> bool {
    matches!(
        term,
        "the"
            | "and"
            | "for"
            | "with"
            | "from"
            | "that"
            | "this"
            | "are"
            | "was"
            | "were"
            | "into"
            | "use"
            | "using"
            | "not"
            | "but"
            | "can"
            | "will"
            | "has"
            | "have"
            | "true"
            | "false"
            | "none"
            | "null"
            | "pub"
            | "fn"
            | "let"
            | "mut"
            | "self"
            | "str"
            | "string"
            | "vec"
            | "result"
            | "option"
            | "struct"
            | "impl"
            | "mod"
            | "http"
            | "https"
            | "com"
            | "org"
            | "users"
            | "digoal"
            | "new"
            | "src"
            | "doc"
            | "docs"
            | "markdown"
            | "md"
            | "如果"
            | "但是"
            | "因为"
            | "所以"
            | "可以"
            | "需要"
            | "进行"
            | "实现"
            | "支持"
            | "相关"
            | "使用"
            | "通过"
            | "问题"
            | "方案"
            | "我们"
            | "你们"
            | "他们"
            | "以及"
            | "或者"
            | "一个"
            | "什么"
            | "如何"
            | "怎么"
    )
}

fn is_cjk_char(c: char) -> bool {
    matches!(c as u32, 0x4E00..=0x9FFF | 0x3400..=0x4DBF)
}

fn push_ascii_identifier_tokens(seg: &str, aliases: &AliasCanonicalizer, out: &mut Vec<String>) {
    if seg.len() < 2 {
        return;
    }
    let whole = aliases.canonicalize(seg);
    if whole.len() >= 2 && !is_stopword(&whole) && !out.contains(&whole) {
        out.push(whole);
    }

    // 拆分 camelCase / snake_case / kebab-case
    for sub_chunk in seg.split(['_', '-']) {
        if sub_chunk.is_empty() {
            continue;
        }
        let chars: Vec<char> = sub_chunk.chars().collect();
        let mut cur = String::new();
        for i in 0..chars.len() {
            let ch = chars[i];
            let prev_lower = i > 0 && chars[i - 1].is_ascii_lowercase();
            let next_lower = i + 1 < chars.len() && chars[i + 1].is_ascii_lowercase();
            let is_upper_boundary = ch.is_ascii_uppercase()
                && (prev_lower || (i > 0 && chars[i - 1].is_ascii_uppercase() && next_lower));
            let is_digit_boundary = i > 0 && ch.is_ascii_digit() != chars[i - 1].is_ascii_digit();
            if (is_upper_boundary || is_digit_boundary) && !cur.is_empty() {
                let c_sub = aliases.canonicalize(&cur);
                if c_sub.len() >= 2 && !is_stopword(&c_sub) && !out.contains(&c_sub) {
                    out.push(c_sub);
                }
                cur.clear();
            }
            cur.push(ch);
        }
        if !cur.is_empty() {
            let c_sub = aliases.canonicalize(&cur);
            if c_sub.len() >= 2 && !is_stopword(&c_sub) && !out.contains(&c_sub) {
                out.push(c_sub);
            }
        }
    }
}

fn push_cjk_run_tokens(run: &str, aliases: &AliasCanonicalizer, out: &mut Vec<String>) {
    let chars: Vec<char> = run.chars().collect();
    if chars.len() < 2 {
        return;
    }
    // 对短中文词（2..=6字）保留原词 + 2~3字窗，长句按常见虚词劈分后取 2..=4 字概念片断
    let sub_runs: Vec<String> = run
        .split(['的', '和', '与', '及', '在', '对', '从', '把', '被', '了', '是', '中', '上', '下', '着', '过', '并', '而', '或', '将'])
        .map(str::trim)
        .filter(|s| s.chars().count() >= 2)
        .map(str::to_string)
        .collect();

    for sr in sub_runs {
        let sc: Vec<char> = sr.chars().collect();
        if sc.len() <= 5 {
            let cand = aliases.canonicalize(&sr);
            if !is_stopword(&cand) && !out.contains(&cand) {
                out.push(cand);
            }
        }
        if sc.len() >= 2 {
            for win_size in [2usize, 3] {
                if sc.len() >= win_size {
                    for w in sc.windows(win_size) {
                        let s: String = w.iter().collect();
                        let cand = aliases.canonicalize(&s);
                        if !is_stopword(&cand) && !out.contains(&cand) {
                            out.push(cand);
                            if out.len() >= 24 {
                                return;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// 统一分词器：拆分中英文、驼峰/蛇形符号、技术缩写并执行别名归一
pub fn tokenize_terms(text: &str, aliases: &AliasCanonicalizer) -> Vec<String> {
    let mut out = Vec::new();
    let mut ascii_buf = String::new();
    let mut cjk_buf = String::new();

    for ch in text.chars() {
        if is_cjk_char(ch) {
            if !ascii_buf.is_empty() {
                push_ascii_identifier_tokens(&ascii_buf, aliases, &mut out);
                ascii_buf.clear();
            }
            cjk_buf.push(ch);
        } else if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            if !cjk_buf.is_empty() {
                push_cjk_run_tokens(&cjk_buf, aliases, &mut out);
                cjk_buf.clear();
            }
            ascii_buf.push(ch);
        } else {
            if !ascii_buf.is_empty() {
                push_ascii_identifier_tokens(&ascii_buf, aliases, &mut out);
                ascii_buf.clear();
            }
            if !cjk_buf.is_empty() {
                push_cjk_run_tokens(&cjk_buf, aliases, &mut out);
                cjk_buf.clear();
            }
        }
        if out.len() >= 32 {
            break;
        }
    }
    if !ascii_buf.is_empty() && out.len() < 32 {
        push_ascii_identifier_tokens(&ascii_buf, aliases, &mut out);
    }
    if !cjk_buf.is_empty() && out.len() < 32 {
        push_cjk_run_tokens(&cjk_buf, aliases, &mut out);
    }
    out
}

pub(crate) fn term_node_id(canonical_term: &str) -> String {
    format!("term:{}", sha256_prefix_hex(canonical_term, 6))
}

pub(crate) fn file_anchor_id(path: &str) -> String {
    format!("file:{}", sha256_prefix_hex(path, 8))
}
