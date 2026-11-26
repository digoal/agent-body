use std::collections::{HashMap, HashSet};
use std::path::Path;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::config::KnowledgeGraphConfig;
use crate::scanner::metadata::{PromptInjectionScanner, SecretRedactor};
use crate::storage::models::{FactEdgeRecord, FactNodeRecord};
use crate::storage::spi::StorageProvider;
use crate::storage::Storage;

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

fn make_section_id(path: &str, heading: &str, draft_idx: usize, part: usize) -> String {
    format!(
        "sec:{}",
        sha256_prefix_hex(&format!("{}|{}|{}|{}", path, heading, draft_idx, part), 8)
    )
}

fn make_preview(heading_path: &str, body: &str) -> String {
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

/// L1 知识网络构建报告
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KnowledgeMiningReport {
    pub docs_indexed: usize,
    pub memories_indexed: usize,
    pub sections_created: usize,
    pub terms_created: usize,
    pub hierarchy_edges: usize,
    pub mention_edges: usize,
    pub pmi_edges: usize,
}

/// 行业 Know-How L1 确定性知识网络构造器
pub struct KnowledgeGraphBuilder;

impl KnowledgeGraphBuilder {
    pub fn mine_from_storage(
        storage: &Storage,
        cfg: &KnowledgeGraphConfig,
    ) -> Result<KnowledgeMiningReport> {
        let mut report = KnowledgeMiningReport::default();
        if !cfg.enabled {
            return Ok(report);
        }

        let aliases = AliasCanonicalizer::new(&cfg.aliases);
        drop(storage.delete_fact_edges_by_tier("L1_KNOWLEDGE"));
        drop(storage.delete_fact_edges_by_tier("L2_KNOWLEDGE_PMI"));

        let assets = storage.list_assets(5000)?;
        let memories = storage.list_all_active_memories(1000)?;

        struct UnitOccurrence {
            unit_node_id: String,
            terms: Vec<String>,
        }

        let mut units: Vec<UnitOccurrence> = Vec::new();
        let mut term_unit_freq: HashMap<String, usize> = HashMap::new();
        let mut term_sample_paths: HashMap<String, Vec<String>> = HashMap::new();
        let mut term_domain: HashMap<String, String> = HashMap::new();

        for asset in &assets {
            if asset.status != "active" {
                continue;
            }
            let modality = asset
                .desc_kv
                .get("modality")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let is_knowledge_doc = asset.asset_type == "document"
                || (modality == "image"
                    && (asset.desc_kv.get("svg_text_snippets").is_some()
                        || asset.desc_kv.get("ocr_keywords").is_some()))
                || (asset.asset_type == "dataset" && asset.desc_kv.get("table_names").is_some());
            if !is_knowledge_doc {
                continue;
            }

            let file_id = file_anchor_id(&asset.path);
            let file_label = Path::new(&asset.path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(&asset.path)
                .to_string();
            let domain = asset
                .desc_kv
                .get("domain")
                .and_then(|v| v.as_str())
                .unwrap_or("workspace_general")
                .to_string();
            let file_node = FactNodeRecord {
                id: file_id.clone(),
                node_type: "file".to_string(),
                name: asset.path.clone(),
                label: file_label.clone(),
                properties: json!({
                    "source_path": asset.path,
                    "domain": domain,
                    "modality": modality,
                    "content_hash": asset.desc_kv.get("content_hash").cloned().unwrap_or(Value::Null),
                }),
                updated_at: None,
            };
            storage.upsert_fact_node(&file_node)?;
            report.docs_indexed += 1;

            let p = Path::new(&asset.path);
            let ext = p
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let is_plain_text = matches!(ext.as_str(), "md" | "markdown" | "txt" | "rst" | "adoc" | "org");
            let chunks = if is_plain_text && p.is_file() {
                if let Ok(meta) = std::fs::metadata(p) {
                    if meta.len() <= 512 * 1024 {
                        if let Ok(raw) = std::fs::read_to_string(p) {
                            HeadingChunker::chunk_markdown(&asset.path, &raw, cfg.max_section_chars)
                        } else {
                            Vec::new()
                        }
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            let effective_chunks = if chunks.is_empty() {
                let mut synth_parts = Vec::new();
                if let Some(desc) = &asset.description {
                    synth_parts.push(desc.clone());
                }
                for key in ["headings", "svg_text_snippets", "ocr_keywords", "table_names", "columns"] {
                    if let Some(arr) = asset.desc_kv.get(key).and_then(|v| v.as_array()) {
                        let vals: Vec<_> = arr.iter().filter_map(|x| x.as_str()).take(10).collect();
                        if !vals.is_empty() {
                            synth_parts.push(vals.join(" "));
                        }
                    }
                }
                if let Some(excerpt) = asset.desc_kv.get("pdf_excerpt").and_then(|v| v.as_str()) {
                    synth_parts.push(excerpt.to_string());
                }
                if let Some(vis) = asset.desc_kv.get("visual_summary").and_then(|v| v.as_str()) {
                    synth_parts.push(vis.to_string());
                }
                let synth_text = synth_parts.join(" | ");
                vec![HeadingChunk {
                    section_id: make_section_id(&asset.path, &file_label, 0, 0),
                    parent_section_id: None,
                    depth: 1,
                    heading_path: file_label.clone(),
                    line_range: (1, 1),
                    preview: make_preview(&file_label, &synth_text),
                    text: synth_text,
                }]
            } else {
                chunks
            };

            for chunk in &effective_chunks {
                let sec_node = FactNodeRecord {
                    id: chunk.section_id.clone(),
                    node_type: "section".to_string(),
                    name: chunk.heading_path.clone(),
                    label: chunk.heading_path.clone(),
                    properties: json!({
                        "source_path": asset.path,
                        "parent_section_id": chunk.parent_section_id,
                        "depth": chunk.depth,
                        "heading_path": chunk.heading_path,
                        "line_range": [chunk.line_range.0, chunk.line_range.1],
                        "snippet_preview": chunk.preview,
                        "content_hash": asset.desc_kv.get("content_hash").cloned().unwrap_or(Value::Null),
                    }),
                    updated_at: None,
                };
                storage.upsert_fact_node(&sec_node)?;
                report.sections_created += 1;

                let parent_target = chunk
                    .parent_section_id
                    .clone()
                    .unwrap_or_else(|| file_id.clone());
                storage.upsert_fact_edge(&FactEdgeRecord {
                    id: None,
                    source_id: parent_target,
                    target_id: chunk.section_id.clone(),
                    relation: "contains".to_string(),
                    properties: json!({"source_path": asset.path}),
                    confidence: 1.0,
                    evidence_tier: "L1_KNOWLEDGE".to_string(),
                    weight: 0.90,
                    direction_kind: "structural_containment".to_string(),
                    updated_at: None,
                })?;
                report.hierarchy_edges += 1;

                let combined = format!("{} {}", chunk.text, chunk.heading_path);
                let mut terms = tokenize_terms(&combined, &aliases);
                terms.truncate(24);
                for t in &terms {
                    *term_unit_freq.entry(t.clone()).or_insert(0) += 1;
                    term_domain.entry(t.clone()).or_insert_with(|| domain.clone());
                    let paths = term_sample_paths.entry(t.clone()).or_default();
                    if paths.len() < 4 && !paths.contains(&asset.path) {
                        paths.push(asset.path.clone());
                    }
                }
                if !terms.is_empty() {
                    units.push(UnitOccurrence {
                        unit_node_id: chunk.section_id.clone(),
                        terms,
                    });
                }
            }
        }

        // 纳管活跃记忆节点用于跨记忆+文档概念贯通
        for mem in &memories {
            let mem_node_id = format!("mem:{}", mem.id);
            let summary = mem
                .summary
                .clone()
                .unwrap_or_else(|| mem.content.chars().take(120).collect());
            storage.upsert_fact_node(&FactNodeRecord {
                id: mem_node_id.clone(),
                node_type: "knowhow".to_string(),
                name: format!("memory:{}", mem.id),
                label: summary.clone(),
                properties: json!({
                    "memory_id": mem.id,
                    "memory_type": mem.memory_type,
                    "claim": summary,
                    "source_path": format!("memory://{}", mem.id),
                }),
                updated_at: None,
            })?;
            report.memories_indexed += 1;

            let combined = format!("{} {} {}", summary, mem.content, mem.tags.join(" "));
            let mut terms = tokenize_terms(&combined, &aliases);
            terms.truncate(10);
            for t in &terms {
                *term_unit_freq.entry(t.clone()).or_insert(0) += 1;
                term_domain
                    .entry(t.clone())
                    .or_insert_with(|| "memory_context".to_string());
            }
            if !terms.is_empty() {
                units.push(UnitOccurrence {
                    unit_node_id: mem_node_id,
                    terms,
                });
            }
        }

        // 过滤高频噪音术语，仅保留在 >= 1 个单元出现（若总单元 >= 4 则忽略覆盖 >70% 单元的泛词）的概念词
        let total_units = units.len().max(1);
        let max_df = if total_units >= 5 {
            ((total_units as f64) * 0.70).ceil() as usize
        } else {
            total_units + 1
        };
        let min_df = 1usize;

        let valid_terms: HashSet<String> = term_unit_freq
            .iter()
            .filter(|(_, &df)| df >= min_df && df <= max_df)
            .map(|(t, _)| t.clone())
            .collect();

        for term in &valid_terms {
            let tid = term_node_id(term);
            let df = term_unit_freq.get(term).copied().unwrap_or(1);
            let samples = term_sample_paths.get(term).cloned().unwrap_or_default();
            let dom = term_domain
                .get(term)
                .cloned()
                .unwrap_or_else(|| "workspace_general".to_string());
            storage.upsert_fact_node(&FactNodeRecord {
                id: tid,
                node_type: "domain_term".to_string(),
                name: term.clone(),
                label: term.clone(),
                properties: json!({
                    "canonical_name": term,
                    "domain": dom,
                    "doc_freq": df,
                    "source_sample_paths": samples,
                }),
                updated_at: None,
            })?;
            report.terms_created += 1;
        }

        // 建立 unit -> domain_term mentions 边与共现计数
        let mut co_counts: HashMap<(String, String), usize> = HashMap::new();
        for unit in &units {
            let filtered: Vec<String> = unit
                .terms
                .iter()
                .filter(|t| valid_terms.contains(*t))
                .cloned()
                .collect();
            for t in &filtered {
                let tid = term_node_id(t);
                storage.upsert_fact_edge(&FactEdgeRecord {
                    id: None,
                    source_id: unit.unit_node_id.clone(),
                    target_id: tid,
                    relation: "mentions".to_string(),
                    properties: json!({}),
                    confidence: 0.9,
                    evidence_tier: "L1_KNOWLEDGE".to_string(),
                    weight: 0.75,
                    direction_kind: "concept_reference".to_string(),
                    updated_at: None,
                })?;
                report.mention_edges += 1;
            }
            for i in 0..filtered.len() {
                for j in (i + 1)..filtered.len() {
                    let (a, b) = if filtered[i] <= filtered[j] {
                        (filtered[i].clone(), filtered[j].clone())
                    } else {
                        (filtered[j].clone(), filtered[i].clone())
                    };
                    if a != b && !a.contains(&b) && !b.contains(&a) {
                        *co_counts.entry((a, b)).or_insert(0) += 1;
                    }
                }
            }
        }

        // 计算 PMI 点互信息并按单节点 Top-5 剪枝写入 co_occurs_with（兼顾跨文档桥接词，抑制罕见词偏差）
        let min_co = if total_units >= 12 { 2usize } else { 1usize };
        let mut per_term_candidates: HashMap<String, Vec<(String, f32, usize, f32)>> = HashMap::new();
        let n_f = total_units as f64;
        for ((a, b), co) in co_counts {
            if co < min_co {
                continue;
            }
            let ca_u = term_unit_freq.get(&a).copied().unwrap_or(1);
            let cb_u = term_unit_freq.get(&b).copied().unwrap_or(1);
            let ca = ca_u as f64;
            let cb = cb_u as f64;
            let pmi = ((co as f64 * n_f) / (ca * cb)).log2();
            if pmi >= cfg.pmi_threshold {
                let weight = (0.55 + (pmi / 5.0).clamp(0.0, 0.35)) as f32;
                let co_bonus = if co >= 2 { 0.25 } else { 0.0 };
                let score_to_b = weight + co_bonus + if cb_u >= 2 { 0.30 } else { 0.0 };
                let score_to_a = weight + co_bonus + if ca_u >= 2 { 0.30 } else { 0.0 };
                per_term_candidates
                    .entry(a.clone())
                    .or_default()
                    .push((b.clone(), weight, co, score_to_b));
                per_term_candidates
                    .entry(b)
                    .or_default()
                    .push((a, weight, co, score_to_a));
            }
        }

        let mut inserted_pairs: HashSet<(String, String)> = HashSet::new();
        let mut co_degree: HashMap<String, usize> = HashMap::new();
        let mut sorted_src_terms: Vec<_> = per_term_candidates.into_iter().collect();
        sorted_src_terms.sort_by(|a, b| a.0.cmp(&b.0));
        for (src_term, mut nbrs) in sorted_src_terms {
            nbrs.sort_by(|x, y| {
                y.3.partial_cmp(&x.3)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| x.0.cmp(&y.0))
            });
            for (dst_term, w, co, _) in nbrs.into_iter().take(5) {
                let (u, v) = if src_term <= dst_term {
                    (src_term.clone(), dst_term.clone())
                } else {
                    (dst_term.clone(), src_term.clone())
                };
                if co_degree.get(&u).copied().unwrap_or(0) >= 32
                    || co_degree.get(&v).copied().unwrap_or(0) >= 32
                {
                    continue;
                }
                if !inserted_pairs.insert((u.clone(), v.clone())) {
                    continue;
                }
                *co_degree.entry(u.clone()).or_insert(0) += 1;
                *co_degree.entry(v.clone()).or_insert(0) += 1;
                storage.upsert_fact_edge(&FactEdgeRecord {
                    id: None,
                    source_id: term_node_id(&u),
                    target_id: term_node_id(&v),
                    relation: "co_occurs_with".to_string(),
                    properties: json!({"co_count": co}),
                    confidence: (w + 0.15).min(0.95),
                    evidence_tier: "L2_KNOWLEDGE_PMI".to_string(),
                    weight: w,
                    direction_kind: "statistical_association".to_string(),
                    updated_at: None,
                })?;
                report.pmi_edges += 1;
            }
        }

        Ok(report)
    }

    /// 写穿级联清理指定源文件派生的全部 `section` / `knowhow` 节点、文件锚点及其关联边
    pub fn invalidate_source_path(
        storage: &dyn StorageProvider,
        source_path: &str,
    ) -> Result<usize> {
        let mut candidate_anchors = Vec::new();
        let fid = file_anchor_id(source_path);
        candidate_anchors.push(fid);
        let legacy_fid = format!("file:{}", source_path);
        if !candidate_anchors.contains(&legacy_fid) {
            candidate_anchors.push(legacy_fid);
        }

        let mut deleted = 0usize;
        let mut nodes_to_delete: HashSet<String> = HashSet::new();

        for anchor in &candidate_anchors {
            if let Ok(sub) = storage.traverse_fact_graph(anchor, 4) {
                for n in sub.nodes {
                    let same_source = n
                        .properties
                        .get("source_path")
                        .and_then(|v| v.as_str())
                        == Some(source_path);
                    if n.id == *anchor
                        || (matches!(n.node_type.as_str(), "section" | "knowhow") && same_source)
                    {
                        nodes_to_delete.insert(n.id);
                    }
                }
            } else if storage.get_fact_node(anchor)?.is_some() {
                nodes_to_delete.insert(anchor.clone());
            }
        }

        if let Some(fname) = Path::new(source_path).file_name().and_then(|s| s.to_str()) {
            if let Ok(by_name) = storage.find_fact_nodes_by_name(fname) {
                for n in by_name {
                    if matches!(n.node_type.as_str(), "section" | "knowhow")
                        && n.properties
                            .get("source_path")
                            .and_then(|v| v.as_str())
                            == Some(source_path)
                    {
                        nodes_to_delete.insert(n.id);
                    }
                }
            }
        }

        for nid in nodes_to_delete {
            deleted += storage.delete_fact_node(&nid).unwrap_or(0);
        }
        Ok(deleted)
    }
}

/// 知识图扩散召回结果
#[derive(Debug, Clone, Default)]
pub struct GraphExpansionResult {
    pub doc_scores: HashMap<String, f64>,
    pub memory_scores: HashMap<String, f64>,
    pub matched_sections: HashMap<String, Vec<String>>,
    pub reasoning_paths: HashMap<String, Vec<String>>,
}

/// 基于 `fact_nodes`/`fact_edges` 执行索引下沉定向子图 1~2 跳加权扩散，补偿弱向量语义与跨词因果断层
pub fn expand_graph_for_query(
    storage: &dyn StorageProvider,
    query: &str,
    max_hops: usize,
    aliases: &AliasCanonicalizer,
) -> GraphExpansionResult {
    let mut res = GraphExpansionResult::default();
    let trimmed = query.trim();
    if trimmed.is_empty() || max_hops == 0 {
        return res;
    }

    let q_terms = tokenize_terms(trimmed, aliases);
    if q_terms.is_empty() {
        return res;
    }

    // 1. 索引下沉定向种子定位（O(log N) PK + 名称索引查找，避免全表扫 4w+ 节点导致 300ms 延迟与 4000 截断）
    let mut node_map: HashMap<String, FactNodeRecord> = HashMap::new();
    let mut all_edges_map: HashMap<(String, String, String), FactEdgeRecord> = HashMap::new();
    let mut seed_scores: HashMap<String, f64> = HashMap::new();

    for qt in q_terms.iter().take(8) {
        let tid = term_node_id(qt);
        if let Ok(Some(exact_node)) = storage.get_fact_node(&tid) {
            seed_scores
                .entry(exact_node.id.clone())
                .and_modify(|s| *s = s.max(1.0))
                .or_insert(1.0);
            node_map.insert(exact_node.id.clone(), exact_node);
        }
        if let Ok(matched) = storage.find_fact_nodes_by_name(qt) {
            for n in matched
                .into_iter()
                .filter(|n| matches!(n.node_type.as_str(), "domain_term" | "knowhow" | "section"))
                .take(8)
            {
                let name_lower = n.name.to_ascii_lowercase();
                let sc = if name_lower == *qt { 1.0 } else { 0.65 };
                seed_scores
                    .entry(n.id.clone())
                    .and_modify(|s| *s = s.max(sc))
                    .or_insert(sc);
                node_map.entry(n.id.clone()).or_insert(n);
            }
        }
    }

    if seed_scores.is_empty() {
        return res;
    }

    let mut ranked_seeds: Vec<(String, f64)> = seed_scores.into_iter().collect();
    ranked_seeds.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked_seeds.truncate(12);

    let hops = max_hops.min(2);
    for (seed_id, _) in &ranked_seeds {
        if let Ok(sub) = storage.traverse_fact_graph(seed_id, hops) {
            for n in sub.nodes {
                node_map.entry(n.id.clone()).or_insert(n);
            }
            for e in sub.edges {
                all_edges_map
                    .entry((e.source_id.clone(), e.target_id.clone(), e.relation.clone()))
                    .or_insert(e);
            }
        }
    }

    let mut adj: HashMap<String, Vec<(String, String, f64)>> = HashMap::new();
    let mut suppressed_targets: HashSet<String> = HashSet::new();

    for e in all_edges_map.values() {
        if e.relation == "supersedes" {
            suppressed_targets.insert(e.target_id.clone());
            continue;
        }
        let eff_w = (e.weight as f64).clamp(0.2, 1.0) * (0.5 + 0.5 * (e.confidence as f64).clamp(0.1, 1.0));
        adj.entry(e.source_id.clone()).or_default().push((
            e.target_id.clone(),
            e.relation.clone(),
            eff_w,
        ));
        let rev_factor = match e.relation.as_str() {
            "co_occurs_with" | "semantically_related" | "contrasts_with" => 1.0,
            "mentions" | "contains" => 0.90,
            _ => 0.80,
        };
        adj.entry(e.target_id.clone()).or_default().push((
            e.source_id.clone(),
            e.relation.clone(),
            eff_w * rev_factor,
        ));
    }

    let mut frontier: Vec<(String, f64, String)> = Vec::new();
    let mut best_node_score: HashMap<String, f64> = HashMap::new();
    for (sid, mut seed_score) in ranked_seeds {
        if suppressed_targets.contains(&sid) {
            seed_score *= 0.15;
        }
        let lbl = node_map
            .get(&sid)
            .map(|n| n.label.clone())
            .unwrap_or_else(|| sid.clone());
        best_node_score.insert(sid.clone(), seed_score);
        frontier.push((sid, seed_score, lbl));
    }

    // 记录命中源文档 / 记忆的工具闭包
    let mut record_hit = |nid: &str, score: f64, path_str: &str| {
        let Some(node) = node_map.get(nid) else {
            return;
        };
        if let Some(mem_id) = node.properties.get("memory_id").and_then(|v| v.as_str()) {
            let e = res.memory_scores.entry(mem_id.to_string()).or_insert(0.0);
            if score > *e {
                *e = score;
            }
            return;
        }
        if let Some(src_path) = node.properties.get("source_path").and_then(|v| v.as_str()) {
            if src_path.starts_with("memory://") {
                return;
            }
            let e = res.doc_scores.entry(src_path.to_string()).or_insert(0.0);
            *e += score;
            if node.node_type == "section" {
                if let Some(preview) = node.properties.get("snippet_preview").and_then(|v| v.as_str()) {
                    let secs = res.matched_sections.entry(src_path.to_string()).or_default();
                    let short: String = preview.chars().take(110).collect();
                    if secs.len() < 3 && !secs.contains(&short) {
                        secs.push(short);
                    }
                }
            } else if node.node_type == "knowhow" {
                if let Some(claim) = node.properties.get("claim").and_then(|v| v.as_str()) {
                    let secs = res.matched_sections.entry(src_path.to_string()).or_default();
                    let bcond = node
                        .properties
                        .get("boundary_condition")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    let formatted = if bcond.trim().is_empty() {
                        format!("[Know-How] {}", claim)
                    } else {
                        format!("[Know-How] {} (边界: {})", claim, bcond)
                    };
                    let short: String = formatted.chars().take(120).collect();
                    if secs.len() < 3 && !secs.contains(&short) {
                        secs.push(short);
                    }
                }
            }
            let rpaths = res.reasoning_paths.entry(src_path.to_string()).or_default();
            if rpaths.len() < 2 && !path_str.is_empty() && !rpaths.contains(&path_str.to_string()) {
                rpaths.push(path_str.to_string());
            }
        }
    };

    for (nid, sc, lbl) in &frontier {
        record_hit(nid, *sc, lbl);
    }

    // 2. 最多 2 跳扩散 (decay = 0.65，带枢纽度数适度阻尼)
    for _hop in 0..hops {
        let mut next_frontier: Vec<(String, f64, String)> = Vec::new();
        for (uid, u_score, u_trace) in &frontier {
            let Some(nbrs) = adj.get(uid) else {
                continue;
            };
            let hub_damp = 1.0 / (1.0 + ((nbrs.len().saturating_sub(6)) as f64).ln_1p() * 0.25);
            for (vid, rel, edge_w) in nbrs {
                let mut prop_score = u_score * 0.65 * edge_w * hub_damp;
                if suppressed_targets.contains(vid) {
                    prop_score *= 0.15;
                }
                if prop_score < 0.04 {
                    continue;
                }
                let prev_best = best_node_score.get(vid).copied().unwrap_or(0.0);
                if prop_score > prev_best {
                    best_node_score.insert(vid.clone(), prop_score);
                    let v_lbl = node_map
                        .get(vid)
                        .map(|n| {
                            let s: String = n.label.chars().take(28).collect();
                            s
                        })
                        .unwrap_or_else(|| vid.clone());
                    let trace = format!("{} -[{}]-> {}", u_trace, rel, v_lbl);
                    record_hit(vid, prop_score, &trace);
                    next_frontier.push((vid.clone(), prop_score, trace));
                }
            }
        }
        if next_frontier.is_empty() {
            break;
        }
        next_frontier.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        next_frontier.truncate(48);
        frontier = next_frontier;
    }

    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::AssetRecord;

    #[test]
    fn test_heading_chunker_breadcrumbs_and_hierarchy() {
        let md = "# PostgreSQL 内核指南\n简介内容\n\n## WAL 写入机制\ncheckpoint_timeout 过短会导致频繁 full_page_writes。\n\n### 优化参数调优\n建议调大 max_wal_size 以缓解 Checkpoint 尖刺。\n";
        let chunks = HeadingChunker::chunk_markdown("/tmp/doc/pg-wal.md", md, 900);
        assert!(chunks.len() >= 3);
        let opt = chunks
            .iter()
            .find(|c| c.heading_path.contains("优化参数调优"))
            .expect("should find level 3 section");
        assert_eq!(opt.depth, 3);
        assert!(opt.heading_path.contains("pg-wal.md > PostgreSQL 内核指南 > WAL 写入机制 > 优化参数调优"));
        assert!(opt.parent_section_id.is_some());
    }

    #[test]
    fn test_alias_pmi_graph_and_two_hop_cross_term_recall() {
        let storage = Storage::open_in_memory().expect("in_memory");
        let dir = tempfile::tempdir().expect("tempdir");

        // 文档 A：连接 PG WAL 与 checkpoint（即使只提了简称 PG）
        let doc_a = dir.path().join("wal_diag.md");
        std::fs::write(
            &doc_a,
            "# PG WAL 诊断\n## Checkpoint 触发\n高频 checkpoint 与 full_page_writes 会引起 wal_spike 写放大。\n",
        )
        .expect("write doc_a");

        // 文档 B：只讲 checkpoint 参数优化缓解方案，不出现 full_page_writes，但通过 PMI/图谱二跳相连
        let doc_b = dir.path().join("wal_tuning.md");
        std::fs::write(
            &doc_b,
            "# Checkpoint 优化手册\n## 参数调优\n调大 max_wal_size 可有效缓解 checkpoint_timeout 造成的 wal_spike 抖动。\n",
        )
        .expect("write doc_b");

        for p in [&doc_a, &doc_b] {
            storage
                .upsert_asset(&AssetRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    path: p.to_string_lossy().to_string(),
                    asset_type: "document".to_string(),
                    owner: "user:tester".to_string(),
                    security_level: "internal".to_string(),
                    scan_tier: "deep".to_string(),
                    description: Some("Markdown doc".to_string()),
                    desc_kv: json!({"domain": "storage_engine"}),
                    usage: None,
                    size_bytes: Some(128),
                    file_count: None,
                    tags: vec!["document".to_string()],
                    status: "active".to_string(),
                })
                .expect("upsert");
        }

        let cfg = KnowledgeGraphConfig {
            pmi_threshold: 0.0,
            ..Default::default()
        };
        let report = KnowledgeGraphBuilder::mine_from_storage(&storage, &cfg).expect("mine kg");
        assert_eq!(report.docs_indexed, 2);
        assert!(report.terms_created >= 3);
        assert!(report.mention_edges >= 4);

        // 查询 doc_a 独有词 full_page_writes，经 wal_spike/checkpoint 共现图谱 2 跳召回 doc_b（完全无向量模型）
        let aliases = AliasCanonicalizer::default();
        let exp = expand_graph_for_query(
            storage.provider().as_ref(),
            "full_page_writes",
            2,
            &aliases,
        );
        let b_str = doc_b.to_string_lossy().to_string();
        assert!(
            exp.doc_scores.contains_key(&b_str),
            "2跳知识图扩散应从 full_page_writes 召回 doc_b，实际 doc_scores={:?}",
            exp.doc_scores
        );
        assert!(!exp.reasoning_paths.get(&b_str).cloned().unwrap_or_default().is_empty());
    }
}
