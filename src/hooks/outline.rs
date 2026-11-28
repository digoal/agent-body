//! 多语言确定性结构骨架提取器 (`FileOutline`)
//!
//! 零外部 AST/Tree-Sitter 依赖，按行线性扫描源代码与配置/文档的关键符号与精确行号，
//! 在大文件首屏切片或重复读拦截时生成紧凑导航导览，避免 Agent 盲目翻页 `read_file`。

use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Serialize};

const MAX_SCAN_BYTES: usize = 2 * 1024 * 1024;
const MAX_LABEL_CHARS: usize = 88;
const BINARY_SAMPLE_BYTES: usize = 8192;

/// 单个文件骨架符号条目
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutlineItem {
    pub line: usize,
    pub kind: String,
    pub label: String,
}

/// 文件结构骨架摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOutline {
    pub path: String,
    pub language: String,
    pub total_lines: usize,
    pub total_bytes: u64,
    pub items: Vec<OutlineItem>,
    pub truncated_items: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LangKind {
    Rust,
    Python,
    JsTs,
    Go,
    Cpp,
    Sql,
    Markdown,
    Toml,
    Yaml,
    Shell,
    Generic,
}

fn detect_language(path: &Path) -> (LangKind, &'static str) {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "rs" => (LangKind::Rust, "rust"),
        "py" | "pyi" => (LangKind::Python, "python"),
        "ts" | "tsx" | "js" | "jsx" | "mjs" | "cjs" => (LangKind::JsTs, "js/ts"),
        "go" => (LangKind::Go, "go"),
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" => (LangKind::Cpp, "c/c++"),
        "sql" => (LangKind::Sql, "sql"),
        "md" | "mdx" => (LangKind::Markdown, "markdown"),
        "toml" => (LangKind::Toml, "toml"),
        "yaml" | "yml" => (LangKind::Yaml, "yaml"),
        "sh" | "bash" | "zsh" => (LangKind::Shell, "shell"),
        _ => (LangKind::Generic, "text"),
    }
}

/// 从物理文件提取多语言符号骨架（只读扫描前 2MB，遇到 NUL 二进制内容自动跳过）
pub fn extract_file_outline(path: &Path, max_items: usize) -> std::io::Result<FileOutline> {
    let meta = path.metadata()?;
    let total_bytes = meta.len();
    let (lang, lang_name) = detect_language(path);
    let mut file = File::open(path)?;
    let read_cap = (total_bytes as usize).min(MAX_SCAN_BYTES);
    let mut buf = Vec::with_capacity(read_cap.min(256 * 1024));
    (&mut file).take(MAX_SCAN_BYTES as u64).read_to_end(&mut buf)?;

    let sample_len = buf.len().min(BINARY_SAMPLE_BYTES);
    if buf[..sample_len].contains(&0u8) {
        return Ok(FileOutline {
            path: path.to_string_lossy().to_string(),
            language: lang_name.to_string(),
            total_lines: 0,
            total_bytes,
            items: Vec::new(),
            truncated_items: 0,
        });
    }

    let text = String::from_utf8_lossy(&buf);
    let mut lines: Vec<&str> = text.lines().collect();
    if text.ends_with('\n') && lines.is_empty() {
        lines.push("");
    }
    let total_lines = lines.len();
    let mut all_items = scan_lines_for_lang(lang, &lines);

    let limit = max_items.max(1);
    let mut truncated_items = 0usize;
    if all_items.len() > limit {
        truncated_items = all_items.len() - limit;
        all_items.truncate(limit);
    }

    Ok(FileOutline {
        path: path.to_string_lossy().to_string(),
        language: lang_name.to_string(),
        total_lines,
        total_bytes,
        items: all_items,
        truncated_items,
    })
}

/// 将 `FileOutline` 格式化为严格控容的紧凑导航导览文本
pub fn format_outline_compact(
    outline: &FileOutline,
    after_line: Option<usize>,
    max_bytes: usize,
) -> Option<String> {
    if outline.items.is_empty() {
        return None;
    }
    let budget = max_bytes.max(256);
    let base_name = Path::new(&outline.path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&outline.path);

    let (selected, showing_after): (Vec<&OutlineItem>, bool) = match after_line {
        Some(cutoff) => {
            let after: Vec<&OutlineItem> =
                outline.items.iter().filter(|it| it.line > cutoff).collect();
            if after.is_empty() {
                (outline.items.iter().collect(), false)
            } else {
                (after, true)
            }
        }
        None => (outline.items.iter().collect(), false),
    };

    let header = match (after_line, showing_after) {
        (Some(cutoff), true) => format!(
            "[agent-body 文件骨架导览] {} ({}, {}L) | 已读首屏 L1~{}，后续关键符号：",
            base_name, outline.language, outline.total_lines, cutoff
        ),
        _ => format!(
            "[agent-body 文件骨架导览] {} ({}, {}L) | 全文件关键符号：",
            base_name, outline.language, outline.total_lines
        ),
    };
    let footer = "提示：请直接按行号调用 read_file(StartLine, EndLine) 定点读取，避免盲翻页。";
    let reserve = footer.len() + 48;

    let mut out = String::with_capacity(budget.min(4096));
    out.push_str(&header);

    let mut omitted = outline.truncated_items;
    let mut rendered = 0usize;
    for (idx, item) in selected.iter().enumerate() {
        let entry = format!("\n- L{} {}", item.line, item.label);
        if out.len() + entry.len() + reserve > budget && rendered > 0 {
            omitted += selected.len() - idx;
            break;
        }
        out.push_str(&entry);
        rendered += 1;
    }

    if omitted > 0 {
        let more = format!("\n... (+{} more symbols)", omitted);
        if out.len() + more.len() + footer.len() + 2 <= budget {
            out.push_str(&more);
        }
    }
    if out.len() + footer.len() + 1 <= budget {
        out.push('\n');
        out.push_str(footer);
    }

    Some(out)
}

fn clean_label(raw: &str) -> String {
    let trimmed = raw
        .trim()
        .trim_end_matches('{')
        .trim_end_matches(':')
        .trim();
    let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
    let total_chars = collapsed.chars().count();
    if total_chars <= MAX_LABEL_CHARS {
        collapsed
    } else {
        let prefix: String = collapsed.chars().take(MAX_LABEL_CHARS - 3).collect();
        format!("{}...", prefix)
    }
}

fn push_item(items: &mut Vec<OutlineItem>, line: usize, kind: &str, raw_label: &str) {
    let label = clean_label(raw_label);
    if !label.is_empty() {
        items.push(OutlineItem {
            line,
            kind: kind.to_string(),
            label,
        });
    }
}

fn scan_lines_for_lang(lang: LangKind, lines: &[&str]) -> Vec<OutlineItem> {
    match lang {
        LangKind::Rust => scan_rust(lines),
        LangKind::Python => scan_python(lines),
        LangKind::JsTs => scan_jsts(lines),
        LangKind::Go => scan_go(lines),
        LangKind::Cpp => scan_cpp(lines),
        LangKind::Sql => scan_sql(lines),
        LangKind::Markdown => scan_markdown(lines),
        LangKind::Toml => scan_toml(lines),
        LangKind::Yaml => scan_yaml(lines),
        LangKind::Shell => scan_shell(lines),
        LangKind::Generic => Vec::new(),
    }
}

fn strip_block_comment_state(line: &str, in_block: &mut bool) -> String {
    let mut out = String::with_capacity(line.len());
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if *in_block {
            if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                *in_block = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            *in_block = true;
            i += 2;
            continue;
        }
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            break;
        }
        let ch = line[i..].chars().next();
        if let Some(c) = ch {
            out.push(c);
            i += c.len_utf8();
        } else {
            break;
        }
    }
    out
}

fn starts_word(s: &str, word: &str) -> bool {
    if let Some(rest) = s.strip_prefix(word) {
        rest.is_empty() || rest.starts_with(|c: char| c.is_whitespace() || c == '(' || c == '<')
    } else {
        false
    }
}

fn strip_rust_prefixes(mut s: &str) -> &str {
    loop {
        let trimmed = s.trim_start();
        if let Some(rest) = trimmed.strip_prefix("pub(") {
            if let Some(idx) = rest.find(')') {
                s = &rest[idx + 1..];
                continue;
            }
        }
        let mut matched = false;
        for kw in ["pub", "async", "unsafe", "const", "extern"] {
            if starts_word(trimmed, kw) {
                s = trimmed[kw.len()..].trim_start();
                if kw == "extern" && s.starts_with('"') {
                    if let Some(end_q) = s[1..].find('"') {
                        s = s[end_q + 2..].trim_start();
                    }
                }
                matched = true;
                break;
            }
        }
        if !matched {
            return trimmed;
        }
    }
}

fn scan_rust(lines: &[&str]) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    let mut in_block = false;
    for (idx, raw) in lines.iter().enumerate() {
        let clean = strip_block_comment_state(raw, &mut in_block);
        let indent = clean.len() - clean.trim_start().len();
        let trimmed = clean.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed == "#[cfg(test)]" {
            push_item(&mut items, idx + 1, "test_mod", "#[cfg(test)]");
            continue;
        }
        if indent > 4 || trimmed.starts_with('#') {
            continue;
        }
        let core = strip_rust_prefixes(trimmed);
        let kind = if starts_word(core, "fn") {
            Some("fn")
        } else if starts_word(core, "struct") {
            Some("struct")
        } else if starts_word(core, "enum") {
            Some("enum")
        } else if starts_word(core, "trait") {
            Some("trait")
        } else if starts_word(core, "impl") {
            Some("impl")
        } else if starts_word(core, "mod") {
            Some("mod")
        } else if indent == 0 && (starts_word(core, "type") || starts_word(trimmed, "const ") || starts_word(trimmed, "pub const ")) {
            Some("type")
        } else {
            None
        };
        if let Some(k) = kind {
            push_item(&mut items, idx + 1, k, trimmed);
        }
    }
    items
}

fn scan_python(lines: &[&str]) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    let mut triple: Option<&str> = None;
    for (idx, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim();
        if let Some(delim) = triple {
            if trimmed.contains(delim) {
                triple = None;
            }
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        for delim in ["\"\"\"", "'''"] {
            if trimmed.starts_with(delim) {
                let rest = &trimmed[3..];
                if !rest.contains(delim) {
                    triple = Some(delim);
                }
                break;
            }
        }
        if triple.is_some() {
            continue;
        }
        let indent = raw.len() - raw.trim_start().len();
        if indent > 8 {
            continue;
        }
        let kind = if starts_word(trimmed, "class") {
            Some("class")
        } else if starts_word(trimmed, "def") || starts_word(trimmed, "async def") {
            Some("def")
        } else {
            None
        };
        if let Some(k) = kind {
            let prefix = if indent >= 4 { "  " } else { "" };
            push_item(&mut items, idx + 1, k, &format!("{}{}", prefix, trimmed));
        }
    }
    items
}

fn scan_jsts(lines: &[&str]) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    let mut in_block = false;
    for (idx, raw) in lines.iter().enumerate() {
        let clean = strip_block_comment_state(raw, &mut in_block);
        let indent = clean.len() - clean.trim_start().len();
        let trimmed = clean.trim();
        if trimmed.is_empty() || indent > 4 {
            continue;
        }
        let mut core = trimmed;
        for kw in ["export", "default", "declare", "async"] {
            if starts_word(core, kw) {
                core = core[kw.len()..].trim_start();
            }
        }
        let is_export = starts_word(trimmed, "export");
        let kind = if starts_word(core, "function") {
            Some("fn")
        } else if starts_word(core, "class") {
            Some("class")
        } else if starts_word(core, "interface") {
            Some("interface")
        } else if starts_word(core, "type") {
            Some("type")
        } else if starts_word(core, "enum") {
            Some("enum")
        } else if is_export && starts_word(core, "const") && core.contains('=') {
            Some("const")
        } else {
            None
        };
        if let Some(k) = kind {
            push_item(&mut items, idx + 1, k, trimmed);
        }
    }
    items
}

fn scan_go(lines: &[&str]) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    let mut in_block = false;
    for (idx, raw) in lines.iter().enumerate() {
        let clean = strip_block_comment_state(raw, &mut in_block);
        let indent = clean.len() - clean.trim_start().len();
        let trimmed = clean.trim();
        if trimmed.is_empty() || indent > 0 {
            continue;
        }
        if starts_word(trimmed, "func") {
            push_item(&mut items, idx + 1, "func", trimmed);
        } else if starts_word(trimmed, "type") {
            push_item(&mut items, idx + 1, "type", trimmed);
        }
    }
    items
}

fn scan_cpp(lines: &[&str]) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    let mut in_block = false;
    for (idx, raw) in lines.iter().enumerate() {
        let clean = strip_block_comment_state(raw, &mut in_block);
        if clean.is_empty() {
            continue;
        }
        let indent = clean.len() - clean.trim_start().len();
        let trimmed = clean.trim();
        if trimmed.is_empty() || indent > 0 || trimmed.starts_with('#') {
            continue;
        }
        if starts_word(trimmed, "struct")
            || starts_word(trimmed, "class")
            || starts_word(trimmed, "enum")
            || starts_word(trimmed, "namespace")
        {
            if !trimmed.ends_with(';') {
                push_item(&mut items, idx + 1, "type", trimmed);
            }
            continue;
        }
        let blocked_kw = ["if", "for", "while", "switch", "return", "typedef", "using"]
            .iter()
            .any(|kw| starts_word(trimmed, kw));
        if !blocked_kw && trimmed.contains('(') && !trimmed.ends_with(';') {
            push_item(&mut items, idx + 1, "fn", trimmed);
        }
    }
    items
}

fn scan_sql(lines: &[&str]) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    let mut in_block = false;
    for (idx, raw) in lines.iter().enumerate() {
        let pre = raw.split("--").next().unwrap_or("");
        let clean = strip_block_comment_state(pre, &mut in_block);
        let trimmed = clean.trim();
        if trimmed.is_empty() {
            continue;
        }
        let upper = trimmed.to_ascii_uppercase();
        if !upper.starts_with("CREATE ") {
            continue;
        }
        let matched = [
            " TABLE ",
            " VIEW ",
            " MATERIALIZED VIEW ",
            " INDEX ",
            " FUNCTION ",
            " PROCEDURE ",
            " TRIGGER ",
            " TYPE ",
            " EXTENSION ",
        ]
        .iter()
        .any(|kw| upper.contains(kw));
        if matched {
            push_item(&mut items, idx + 1, "ddl", trimmed);
        }
    }
    items
}

fn scan_markdown(lines: &[&str]) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    let mut fence: Option<&str> = None;
    for (idx, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim_start();
        if let Some(f) = fence {
            if trimmed.starts_with(f) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") {
            fence = Some("```");
            continue;
        }
        if trimmed.starts_with("~~~") {
            fence = Some("~~~");
            continue;
        }
        if trimmed.starts_with("# ") || trimmed.starts_with("## ") || trimmed.starts_with("### ") {
            push_item(&mut items, idx + 1, "heading", trimmed);
        }
    }
    items
}

fn scan_toml(lines: &[&str]) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    for (idx, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') && !trimmed.contains('=') {
            push_item(&mut items, idx + 1, "section", trimmed);
        }
    }
    items
}

fn scan_yaml(lines: &[&str]) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    for (idx, raw) in lines.iter().enumerate() {
        if raw.is_empty() || raw.starts_with(' ') || raw.starts_with('\t') {
            continue;
        }
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
            continue;
        }
        if trimmed.contains(':') {
            let key = trimmed.split(':').next().unwrap_or("").trim();
            if !key.is_empty() {
                push_item(&mut items, idx + 1, "key", &format!("{}:", key));
            }
        }
    }
    items
}

fn scan_shell(lines: &[&str]) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    for (idx, raw) in lines.iter().enumerate() {
        let trimmed = raw.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || raw.starts_with(' ') || raw.starts_with('\t') {
            continue;
        }
        if starts_word(trimmed, "function") {
            push_item(&mut items, idx + 1, "fn", trimmed);
            continue;
        }
        if let Some(pos) = trimmed.find("()") {
            let name = trimmed[..pos].trim();
            if !name.is_empty()
                && name
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                push_item(&mut items, idx + 1, "fn", trimmed);
            }
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_temp_with_ext(ext: &str, body: &str) -> tempfile::TempPath {
        let mut f = tempfile::Builder::new()
            .suffix(&format!(".{}", ext))
            .tempfile()
            .expect("tempfile");
        f.write_all(body.as_bytes()).expect("write");
        f.into_temp_path()
    }

    #[test]
    fn test_outline_rust_and_after_line_format() {
        let src = r#"// header
/* block
pub fn ignored_in_comment() {}
*/
pub mod sub;
pub struct ConfigBox {
    x: i32,
}
impl ConfigBox {
    pub fn method_a(&self) {}
}
pub async fn run_server(port: u16) -> Result<(), ()> {
    Ok(())
}
#[cfg(test)]
mod tests {
    #[test]
    fn inner_test() {}
}
"#;
        let path = write_temp_with_ext("rs", src);
        let outline = extract_file_outline(&path, 50).expect("extract rs");
        assert_eq!(outline.language, "rust");
        let labels: Vec<&str> = outline.items.iter().map(|i| i.label.as_str()).collect();
        assert!(labels.iter().all(|l| !l.contains("ignored_in_comment")));
        assert!(labels.iter().any(|l| l.contains("pub struct ConfigBox")));
        assert!(labels.iter().any(|l| l.contains("pub async fn run_server")));
        assert!(labels.iter().any(|l| *l == "#[cfg(test)]"));

        let compact = format_outline_compact(&outline, Some(7), 1024).expect("compact");
        assert!(compact.contains("已读首屏 L1~7"));
        assert!(!compact.contains("L5 pub mod sub"));
        assert!(compact.contains("pub async fn run_server"));
    }

    #[test]
    fn test_outline_python_jsts_go_sql_toml() {
        let py = write_temp_with_ext(
            "py",
            "\"\"\"\ndef fake_doc():\n    pass\n\"\"\"\nclass Engine:\n    def run(self):\n        pass\nasync def serve():\n    pass\n",
        );
        let py_out = extract_file_outline(&py, 20).expect("py");
        assert_eq!(py_out.items.len(), 3);
        assert_eq!(py_out.items[0].line, 5);

        let ts = write_temp_with_ext(
            "ts",
            "export interface UserCfg { id: string }\nexport async function boot(n: number) {\n  return n;\n}\nexport const VERSION = '1.0';\n",
        );
        let ts_out = extract_file_outline(&ts, 20).expect("ts");
        assert_eq!(ts_out.items.len(), 3);

        let go = write_temp_with_ext(
            "go",
            "package main\ntype Server struct {}\nfunc (s *Server) Start() {}\nfunc main() {}\n",
        );
        let go_out = extract_file_outline(&go, 20).expect("go");
        assert_eq!(go_out.items.len(), 3);

        let sql = write_temp_with_ext(
            "sql",
            "-- comment\nCREATE TABLE IF NOT EXISTS users (id int);\nCREATE UNIQUE INDEX idx_u ON users(id);\n",
        );
        let sql_out = extract_file_outline(&sql, 20).expect("sql");
        assert_eq!(sql_out.items.len(), 2);

        let toml = write_temp_with_ext("toml", "[package]\nname = \"x\"\n[dependencies]\n");
        let toml_out = extract_file_outline(&toml, 20).expect("toml");
        assert_eq!(toml_out.items.len(), 2);
    }

    #[test]
    fn test_outline_markdown_skips_fenced_code_and_respects_budget() {
        let md = write_temp_with_ext(
            "md",
            "# Title One\n```bash\n# comment inside code block\n```\n## Section Two\n### Sub Section Three\n",
        );
        let out = extract_file_outline(&md, 50).expect("md");
        assert_eq!(out.items.len(), 3);
        assert!(out.items.iter().all(|i| !i.label.contains("comment inside code block")));

        let compact = format_outline_compact(&out, None, 260).expect("compact");
        assert!(compact.len() <= 260);
    }

    #[test]
    fn test_outline_skips_binary_files() {
        let mut f = NamedTempFile::new().expect("tmp");
        f.write_all(&[0x7f, b'E', b'L', b'F', 0x00, 0x01]).expect("write");
        let out = extract_file_outline(f.path(), 50).expect("binary");
        assert!(out.items.is_empty());
    }
}
