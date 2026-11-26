//! Pure-Rust Multimodal L1A Parsers, Unified 3-Channel Classifier, Bounded Subprocess Probes, KV Predicate Filter & Backfill (Phase 63)

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::scanner::metadata::{
    is_sensitive_file, merge_with_existing_asset, read_head_utf8, truncate_chars,
    HeuristicMetadataExtractor, PromptInjectionScanner, SecretRedactor,
};
use crate::storage::models::AssetRecord;
use crate::storage::Storage;

const MAX_FACET_ITEMS: usize = 24;

/// 统一多模态资产分类结果（CLI Scanner / Watcher / AutoSync 三通道共用）
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetClassification {
    pub asset_type: &'static str,
    pub modality: &'static str,
    pub mime_type: &'static str,
    pub tags: Vec<String>,
}

fn read_prefix_bytes(path: &Path, cap: usize) -> Vec<u8> {
    let Ok(f) = File::open(path) else {
        return Vec::new();
    };
    let mut buf = Vec::with_capacity(cap.min(4096));
    let mut limited = f.take(cap as u64);
    if limited.read_to_end(&mut buf).is_err() {
        return Vec::new();
    }
    buf
}

fn sanitize_line(raw: &str) -> String {
    let clean = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    SecretRedactor::redact_single_line(&clean)
}

fn scan_untrusted_text(
    raw: &str,
    kv: &mut Map<String, Value>,
    extra_tags: &mut Vec<String>,
) -> String {
    let scan = PromptInjectionScanner::scan_and_sanitize(raw);
    if scan.suspected {
        kv.insert("security_flags".to_string(), json!(["prompt_injection_suspected"]));
        kv.insert("injection_patterns".to_string(), json!(scan.matched_patterns));
        if !extra_tags.iter().any(|t| t == "untrusted_prompt_flagged") {
            extra_tags.push("untrusted_prompt_flagged".to_string());
        }
    }
    scan.sanitized
}

fn classify_by_magic(h: &[u8], ext: &str) -> Option<(&'static str, &'static str, &'static str)> {
    if h.len() >= 8 && &h[0..8] == b"\x89PNG\r\n\x1a\n" {
        return Some(("media", "image", "image/png"));
    }
    if h.len() >= 3 && h[0..3] == [0xFF, 0xD8, 0xFF] {
        return Some(("media", "image", "image/jpeg"));
    }
    if h.len() >= 6 && (&h[0..6] == b"GIF87a" || &h[0..6] == b"GIF89a") {
        return Some(("media", "image", "image/gif"));
    }
    if h.len() >= 12 && &h[0..4] == b"RIFF" {
        if &h[8..12] == b"WEBP" {
            return Some(("media", "image", "image/webp"));
        }
        if &h[8..12] == b"WAVE" {
            return Some(("media", "audio", "audio/wav"));
        }
        if &h[8..12] == b"AVI " {
            return Some(("media", "video", "video/x-msvideo"));
        }
    }
    if h.len() >= 5 && &h[0..5] == b"%PDF-" {
        return Some(("document", "pdf", "application/pdf"));
    }
    if h.len() >= 16 && &h[0..16] == b"SQLite format 3\0" {
        return Some(("dataset", "dataset", "application/vnd.sqlite3"));
    }
    if h.len() >= 4 && &h[0..4] == b"PAR1" {
        return Some(("dataset", "dataset", "application/vnd.apache.parquet"));
    }
    if h.len() >= 12 && &h[8..12] == b"DUCK" {
        return Some(("dataset", "dataset", "application/x-duckdb"));
    }
    if h.len() >= 4 && &h[0..4] == b"fLaC" {
        return Some(("media", "audio", "audio/flac"));
    }
    if h.len() >= 4 && &h[0..4] == b"OggS" {
        return Some(("media", "audio", "audio/ogg"));
    }
    if h.len() >= 3 && &h[0..3] == b"ID3" {
        return Some(("media", "audio", "audio/mpeg"));
    }
    if h.len() >= 8 && &h[4..8] == b"ftyp" {
        return Some(match ext {
            "m4a" => ("media", "audio", "audio/mp4"),
            "mov" => ("media", "video", "video/quicktime"),
            _ => ("media", "video", "video/mp4"),
        });
    }
    if h.len() >= 4 && h[0..4] == [0x1A, 0x45, 0xDF, 0xA3] {
        return Some(if ext == "webm" {
            ("media", "video", "video/webm")
        } else {
            ("media", "video", "video/x-matroska")
        });
    }
    if h.len() >= 4 && &h[0..4] == b"PK\x03\x04" {
        return Some(match ext {
            "docx" => (
                "document",
                "office",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ),
            "xlsx" => (
                "document",
                "office",
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            ),
            "pptx" => (
                "document",
                "office",
                "application/vnd.openxmlformats-officedocument.presentationml.presentation",
            ),
            "odt" | "ods" | "odp" | "epub" => ("document", "office", "application/octet-stream"),
            _ => ("dataset", "dataset", "application/zip"),
        });
    }
    if h.len() >= 2 && h[0] == b'B' && h[1] == b'M' && (ext == "bmp" || ext == "dib") {
        return Some(("media", "image", "image/bmp"));
    }
    if h.len() >= 2 && h[0] == 0x1F && h[1] == 0x8B {
        return Some(("dataset", "dataset", "application/gzip"));
    }
    if h.len() >= 4 && h[0..4] == [0x28, 0xB5, 0x2F, 0xFD] {
        return Some(("dataset", "dataset", "application/zstd"));
    }
    if !matches!(
        ext,
        "rs" | "py" | "sh" | "js" | "ts" | "tsx" | "jsx" | "go" | "md" | "txt" | "toml" | "json"
    ) {
        let lower = String::from_utf8_lossy(h).to_ascii_lowercase();
        let trimmed = lower.trim_start();
        if trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && lower.contains("<svg")) {
            return Some(("media", "image", "image/svg+xml"));
        }
        if trimmed.starts_with("<!doctype html") || trimmed.starts_with("<html") {
            return Some(("document", "document", "text/html"));
        }
    }
    if ext == "ts" && h.len() >= 377 && h[0] == 0x47 && h[188] == 0x47 && h[376] == 0x47 {
        return Some(("media", "video", "video/mp2t"));
    }
    None
}

fn finish_classification(
    cls: (&'static str, &'static str, &'static str),
    ext: &str,
    file_name_lower: &str,
) -> AssetClassification {
    let (asset_type, modality, mime_type) = cls;
    let mut tags = Vec::new();
    match asset_type {
        "script" => match ext {
            "rs" => tags.push("rust".to_string()),
            "py" | "pyi" => tags.push("python".to_string()),
            "sh" | "bash" | "zsh" | "fish" => tags.push("shell".to_string()),
            "js" | "mjs" | "cjs" | "jsx" => tags.push("javascript".to_string()),
            "ts" | "tsx" => tags.push("typescript".to_string()),
            "go" => tags.push("go".to_string()),
            "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "hh" => tags.push("c_cpp".to_string()),
            _ if !ext.is_empty() => tags.push(ext.to_string()),
            _ => tags.push("script".to_string()),
        },
        "document" => {
            tags.push("document".to_string());
            if modality == "office" {
                tags.push("office".to_string());
            }
            if !ext.is_empty() && ext != "md" && ext != "txt" {
                tags.push(ext.to_string());
            }
        }
        "config" => {
            tags.push("config".to_string());
            if !ext.is_empty() {
                tags.push(ext.to_string());
            }
        }
        "dataset" => {
            tags.push("dataset".to_string());
            if !ext.is_empty() {
                tags.push(ext.to_string());
            }
        }
        "media" => {
            tags.push("media".to_string());
            if modality != "media" {
                tags.push(modality.to_string());
            }
            if !ext.is_empty() {
                tags.push(ext.to_string());
            }
        }
        _ => {
            if !ext.is_empty() {
                tags.push(ext.to_string());
            }
        }
    }
    if matches!(
        file_name_lower,
        "makefile" | "dockerfile" | "cargo.lock" | "cmakelists.txt" | ".gitignore"
    ) && !tags.iter().any(|t| t == "config")
    {
        tags.push("config".to_string());
    }
    AssetClassification {
        asset_type,
        modality,
        mime_type,
        tags,
    }
}

/// 统一三通道（Scanner / Watcher / AutoSync）魔数 + 扩展名联合分类器
pub fn classify_asset_path(path: &Path) -> AssetClassification {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    let file_name_lower = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    let header = if !is_sensitive_file(path) && path.is_file() {
        read_prefix_bytes(path, 512)
    } else {
        Vec::new()
    };

    if let Some(cls) = classify_by_magic(&header, &ext) {
        return finish_classification(cls, &ext, &file_name_lower);
    }

    let cls = match ext.as_str() {
        "rs" => ("script", "code", "text/x-rust"),
        "py" | "pyi" => ("script", "code", "text/x-python"),
        "sh" | "bash" | "zsh" | "fish" => ("script", "code", "text/x-shellscript"),
        "js" | "jsx" | "mjs" | "cjs" => ("script", "code", "application/javascript"),
        "ts" | "tsx" => ("script", "code", "application/typescript"),
        "go" => ("script", "code", "text/x-go"),
        "c" | "cpp" | "cc" | "cxx" | "h" | "hpp" | "hh" => ("script", "code", "text/x-c"),
        "lua" | "rb" | "pl" | "pm" | "java" | "kt" | "kts" | "php" | "r" | "scala" | "swift"
        | "zig" | "hs" | "ex" | "exs" | "erl" | "clj" | "dart" | "vue" | "svelte" | "css"
        | "scss" | "sass" | "less" | "proto" | "graphql" | "gql" => ("script", "code", "text/plain"),

        "toml" => ("config", "config", "application/toml"),
        "yaml" | "yml" => ("config", "config", "application/yaml"),
        "json" | "jsonc" | "json5" => ("config", "config", "application/json"),
        "sql" => ("config", "config", "application/sql"),
        "xml" | "xsd" | "xsl" | "plist" => ("config", "config", "application/xml"),
        "ini" | "conf" | "cfg" | "service" | "timer" | "socket" | "target" | "env"
        | "properties" | "editorconfig" | "cmake" | "mk" | "nix" | "tf" | "hcl" => {
            ("config", "config", "text/plain")
        }

        "md" | "markdown" | "mdx" => ("document", "document", "text/markdown"),
        "txt" | "rst" | "adoc" | "asciidoc" | "org" | "tex" | "bib" | "man" => {
            ("document", "document", "text/plain")
        }
        "html" | "htm" | "xhtml" => ("document", "document", "text/html"),
        "pdf" => ("document", "pdf", "application/pdf"),
        "docx" => (
            "document",
            "office",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ),
        "xlsx" => (
            "document",
            "office",
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        ),
        "pptx" => (
            "document",
            "office",
            "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        ),
        "odt" | "ods" | "odp" | "rtf" | "epub" | "doc" | "xls" | "ppt" => {
            ("document", "office", "application/octet-stream")
        }

        "svg" | "svgz" => ("media", "image", "image/svg+xml"),
        "png" => ("media", "image", "image/png"),
        "jpg" | "jpeg" => ("media", "image", "image/jpeg"),
        "gif" => ("media", "image", "image/gif"),
        "webp" => ("media", "image", "image/webp"),
        "bmp" | "dib" => ("media", "image", "image/bmp"),
        "ico" | "cur" => ("media", "image", "image/x-icon"),
        "tif" | "tiff" => ("media", "image", "image/tiff"),
        "avif" => ("media", "image", "image/avif"),
        "heic" | "heif" => ("media", "image", "image/heif"),

        "mp3" => ("media", "audio", "audio/mpeg"),
        "wav" => ("media", "audio", "audio/wav"),
        "ogg" | "oga" | "opus" => ("media", "audio", "audio/ogg"),
        "flac" => ("media", "audio", "audio/flac"),
        "m4a" | "aac" => ("media", "audio", "audio/mp4"),
        "wma" | "aiff" | "aif" => ("media", "audio", "audio/x-aiff"),

        "mp4" | "m4v" => ("media", "video", "video/mp4"),
        "webm" => ("media", "video", "video/webm"),
        "mov" => ("media", "video", "video/quicktime"),
        "mkv" => ("media", "video", "video/x-matroska"),
        "avi" => ("media", "video", "video/x-msvideo"),

        "csv" => ("dataset", "dataset", "text/csv"),
        "tsv" => ("dataset", "dataset", "text/tab-separated-values"),
        "jsonl" | "ndjson" => ("dataset", "dataset", "application/x-ndjson"),
        "parquet" => ("dataset", "dataset", "application/vnd.apache.parquet"),
        "sqlite" | "sqlite3" | "db" => ("dataset", "dataset", "application/vnd.sqlite3"),
        "duckdb" => ("dataset", "dataset", "application/x-duckdb"),

        _ => {
            if matches!(
                file_name_lower.as_str(),
                "makefile" | "dockerfile" | "cargo.lock" | "cmakelists.txt" | ".gitignore"
            ) {
                ("config", "config", "text/plain")
            } else {
                ("other", "other", "application/octet-stream")
            }
        }
    };

    finish_classification(cls, &ext, &file_name_lower)
}

/// 读取宿主机多模态外部探针策略 (`enabled`, `timeout_ms`)
pub fn effective_multimodal_tool_policy() -> (bool, u64) {
    if let Ok(v) = std::env::var("AGENT_BODY_MULTIMODAL_TOOLS") {
        let low = v.trim().to_ascii_lowercase();
        if matches!(low.as_str(), "0" | "false" | "off" | "disabled") {
            return (false, 500);
        }
    }
    let cfg = crate::config::Config::load_effective_or_default();
    let timeout = cfg.annotation.multimodal_tool_timeout_ms.clamp(50, 5000);
    (cfg.annotation.multimodal_tools_enabled, timeout)
}

/// 安全有界运行只读探针子进程（超时立即 kill + wait 防止僵尸进程）
pub fn run_command_bounded(program: &str, args: &[&str], timeout_ms: u64) -> Option<String> {
    let bin = crate::preflight::PreflightChecker::resolve_binary_path(program)?;
    let out_path = std::env::temp_dir().join(format!(
        "ab_probe_{}.out",
        uuid::Uuid::new_v4().simple()
    ));
    let out_file = File::create(&out_path).ok()?;
    let mut child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::from(out_file))
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + Duration::from_millis(timeout_ms.max(50));
    let mut exited_ok = false;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                exited_ok = status.success();
                break;
            }
            Ok(None) if Instant::now() >= deadline => {
                if let Err(e) = child.kill() {
                    eprintln!("[Multimodal][WARN] 终止超时探针 ({}): {}", program, e);
                }
                if let Err(e) = child.wait() {
                    eprintln!("[Multimodal][WARN] 回收超时探针 ({}): {}", program, e);
                }
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(_) => {
                if let Err(e) = child.kill() {
                    eprintln!("[Multimodal][WARN] 异常终止探针 ({}): {}", program, e);
                }
                if let Err(e) = child.wait() {
                    eprintln!("[Multimodal][WARN] 异常回收探针 ({}): {}", program, e);
                }
                break;
            }
        }
    }

    let content = if exited_ok {
        read_head_utf8(&out_path, 32 * 1024)
    } else {
        None
    };
    if let Err(e) = std::fs::remove_file(&out_path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            eprintln!("[Multimodal][WARN] 清理探针临时输出失败: {}", e);
        }
    }
    content
}

fn strip_inline_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => {
                in_tag = false;
                out.push(' ');
            }
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn extract_all_html_tag_texts(html: &str, tag: &str, max_items: usize) -> Vec<String> {
    let lower = html.to_ascii_lowercase();
    let (open_pat, close_pat) = (format!("<{}", tag), format!("</{}>", tag));
    let (mut out, mut pos) = (Vec::new(), 0usize);
    while pos < lower.len() && out.len() < max_items {
        let Some(rel_start) = lower[pos..].find(&open_pat) else {
            break;
        };
        let abs_start = pos + rel_start;
        let next_ch = lower[abs_start + open_pat.len()..].chars().next();
        if !matches!(next_ch, Some('>') | Some(' ') | Some('\n') | Some('\t') | Some('/')) {
            pos = abs_start + open_pat.len();
            continue;
        }
        let Some(gt_rel) = lower[abs_start..].find('>') else {
            break;
        };
        let body_start = abs_start + gt_rel + 1;
        let Some(end_rel) = lower[body_start..].find(&close_pat) else {
            pos = body_start;
            continue;
        };
        let body_end = body_start + end_rel;
        if let (Some(slice), true) = (html.get(body_start..body_end), true) {
            let text = sanitize_line(&strip_inline_tags(slice));
            if !text.is_empty() && !out.contains(&text) {
                out.push(truncate_chars(&text, 80));
            }
        }
        pos = body_end + close_pat.len();
    }
    out
}

fn strip_html_for_preview(html: &str, max_chars: usize) -> String {
    let lower = html.to_ascii_lowercase();
    let (mut cleaned, mut idx) = (String::with_capacity(html.len().min(32768)), 0usize);
    while idx < html.len() {
        let skip = match (lower[idx..].find("<script"), lower[idx..].find("<style")) {
            (Some(a), Some(b)) => Some((idx + a.min(b), a < b)),
            (Some(a), None) => Some((idx + a, true)),
            (None, Some(b)) => Some((idx + b, false)),
            (None, None) => None,
        };
        match skip {
            Some((tag_start, is_script)) => {
                if let Some(chunk) = html.get(idx..tag_start) {
                    cleaned.push_str(chunk);
                }
                let close = if is_script { "</script>" } else { "</style>" };
                if let Some(end_rel) = lower[tag_start..].find(close) {
                    idx = tag_start + end_rel + close.len();
                } else {
                    break;
                }
            }
            None => {
                if let Some(rest) = html.get(idx..) {
                    cleaned.push_str(rest);
                }
                break;
            }
        }
    }
    let stripped = sanitize_line(&strip_inline_tags(&cleaned));
    truncate_chars(&stripped, max_chars)
}

/// 纯 Rust HTML L1A 深度特征提取（标题、h1~h3 纲要、section/幻灯片数、内嵌 SVG、正文摘录与防注入脱敏）
pub fn extract_html_facets(
    path: &Path,
    snippet: &str,
    file_name: &str,
    actual_size: Option<i64>,
    kv: &mut Map<String, Value>,
    extra_tags: &mut Vec<String>,
) -> (String, String, Option<String>) {
    let raw = read_head_utf8(path, 256 * 1024).unwrap_or_else(|| snippet.to_string());
    kv.insert("doc_category".into(), json!("html"));
    kv.insert("modality".into(), json!("document"));

    if let Some(title_raw) = extract_all_html_tag_texts(&raw, "title", 1).into_iter().next() {
        let clean = scan_untrusted_text(&title_raw, kv, extra_tags);
        if !clean.is_empty() {
            kv.insert("doc_title".into(), json!(clean));
        }
    }

    let mut headings = Vec::new();
    for tag in ["h1", "h2", "h3"] {
        for h in extract_all_html_tag_texts(&raw, tag, 6) {
            let clean = scan_untrusted_text(&h, kv, extra_tags);
            if !clean.is_empty() && !headings.contains(&clean) && headings.len() < 8 {
                headings.push(clean);
            }
        }
    }
    if !headings.is_empty() {
        kv.insert("headings".into(), json!(&headings));
    }

    let lower = raw.to_ascii_lowercase();
    let section_count = lower.matches("<section").count();
    kv.insert("section_count".into(), json!(section_count));
    if section_count > 1 || (section_count > 0 && (lower.contains("reveal") || lower.contains("slide"))) {
        kv.insert("slide_count".into(), json!(section_count));
    }
    let inline_svg_count = lower.matches("<svg").count();
    if inline_svg_count > 0 {
        kv.insert("inline_svg_count".into(), json!(inline_svg_count));
    }

    let preview_raw = strip_html_for_preview(&raw, 420);
    if !preview_raw.is_empty() {
        let clean_preview = scan_untrusted_text(&preview_raw, kv, extra_tags);
        kv.insert("html_excerpt".into(), json!(clean_preview));
    }

    let title_str = kv
        .get("doc_title")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("HTML 文档 ({}, {} KB)", file_name, (actual_size.unwrap_or(0) / 1024).max(1)));
    let slide_note = if section_count > 1 {
        format!(" · {} sections", section_count)
    } else {
        String::new()
    };
    let purpose = format!("HTML 富文档: {}{}", title_str, slide_note);
    let entry_point = headings.first().cloned().unwrap_or_else(|| title_str.clone());
    kv.insert("capabilities".into(), json!(vec!["HTML 标题/章节/SVG/正文语义检索".to_string()]));
    extra_tags.push("html".to_string());
    (purpose, entry_point, Some(format!("检索 HTML 文档纲要或浏览器查看: {}", file_name)))
}

struct ThriftCur<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> ThriftCur<'a> {
    fn read_u8(&mut self) -> Option<u8> {
        let b = *self.buf.get(self.pos)?;
        self.pos += 1;
        Some(b)
    }
    fn read_uvarint(&mut self) -> Option<u64> {
        let mut res = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.read_u8()?;
            res |= ((b & 0x7F) as u64) << shift;
            if (b & 0x80) == 0 {
                return Some(res);
            }
        }
        None
    }
    fn read_zigzag_i64(&mut self) -> Option<i64> {
        let n = self.read_uvarint()?;
        Some(((n >> 1) as i64) ^ (-((n & 1) as i64)))
    }
    fn read_field_hdr(&mut self, last_fid: &mut i16) -> Option<(i16, u8)> {
        let b = self.read_u8()?;
        if b == 0 {
            return Some((0, 0));
        }
        let wtype = b & 0x0F;
        let delta = ((b >> 4) & 0x0F) as i16;
        let fid = if delta == 0 {
            self.read_zigzag_i64()? as i16
        } else {
            *last_fid + delta
        };
        *last_fid = fid;
        Some((fid, wtype))
    }
    fn read_list_hdr(&mut self) -> Option<(u8, usize)> {
        let b = self.read_u8()?;
        let elem_t = b & 0x0F;
        let hi = (b >> 4) & 0x0F;
        let count = if hi == 15 {
            self.read_uvarint()? as usize
        } else {
            hi as usize
        };
        Some((elem_t, count))
    }
    fn read_string(&mut self) -> Option<String> {
        let len = self.read_uvarint()? as usize;
        if self.pos.checked_add(len)? > self.buf.len() {
            return None;
        }
        let s = String::from_utf8_lossy(&self.buf[self.pos..self.pos + len]).to_string();
        self.pos += len;
        Some(s)
    }
    fn skip_value(&mut self, wtype: u8) -> bool {
        match wtype {
            1 | 2 => true,
            3 => self.read_u8().is_some(),
            4..=6 => self.read_uvarint().is_some(),
            7 => {
                if self.pos.checked_add(8).is_some_and(|e| e <= self.buf.len()) {
                    self.pos += 8;
                    true
                } else {
                    false
                }
            }
            8 => match self.read_uvarint() {
                Some(len) => match self.pos.checked_add(len as usize) {
                    Some(end) if end <= self.buf.len() => {
                        self.pos = end;
                        true
                    }
                    _ => false,
                },
                None => false,
            },
            9 | 10 => {
                let Some((et, cnt)) = self.read_list_hdr() else {
                    return false;
                };
                (0..cnt.min(4096)).all(|_| self.skip_value(if et == 2 { 1 } else { et }))
            }
            11 => {
                let Some(cnt) = self.read_uvarint() else {
                    return false;
                };
                if cnt == 0 {
                    return true;
                }
                let Some(kv_t) = self.read_u8() else {
                    return false;
                };
                let (kt, vt) = ((kv_t >> 4) & 0x0F, kv_t & 0x0F);
                (0..(cnt as usize).min(2048)).all(|_| {
                    self.skip_value(if kt == 2 { 1 } else { kt })
                        && self.skip_value(if vt == 2 { 1 } else { vt })
                })
            }
            12 => {
                let mut lf = 0i16;
                for _ in 0..128 {
                    let Some((fid, wt)) = self.read_field_hdr(&mut lf) else {
                        return false;
                    };
                    if fid == 0 {
                        return true;
                    }
                    if !self.skip_value(wt) {
                        return false;
                    }
                }
                false
            }
            _ => false,
        }
    }
}

fn read_parquet_schema_elem(cur: &mut ThriftCur<'_>) -> Option<(String, Option<String>, Option<i32>)> {
    let (mut name, mut phys_t, mut num_children, mut last_fid) = (String::new(), None, None, 0i16);
    for _ in 0..24 {
        let (fid, wtype) = cur.read_field_hdr(&mut last_fid)?;
        if fid == 0 {
            return Some((name, phys_t, num_children));
        }
        match (fid, wtype) {
            (1, 5) => {
                let code = cur.read_zigzag_i64()? as i32;
                let tname = match code {
                    0 => "bool",
                    1 => "int32",
                    2 => "int64",
                    3 => "int96",
                    4 => "float",
                    5 => "double",
                    6 => "bytes",
                    7 => "fixed_bytes",
                    _ => "unknown",
                };
                phys_t = Some(tname.to_string());
            }
            (4, 8) => name = cur.read_string()?,
            (5, 5) => num_children = Some(cur.read_zigzag_i64()? as i32),
            _ => {
                if !cur.skip_value(wtype) {
                    return None;
                }
            }
        }
    }
    None
}

/// 纯 Rust 解析 Parquet Thrift Compact Footer（不依赖 DuckDB，零 panic 处理截断/损坏文件）
pub fn parse_parquet_footer_l1a(
    path: &Path,
    actual_size: u64,
) -> Option<(Option<i64>, Vec<String>, Vec<String>)> {
    if actual_size < 12 {
        return None;
    }
    let mut f = File::open(path).ok()?;
    let mut tail = [0u8; 8];
    f.seek(SeekFrom::End(-8)).ok()?;
    f.read_exact(&mut tail).ok()?;
    if &tail[4..8] != b"PAR1" {
        return None;
    }
    let footer_len = u32::from_le_bytes([tail[0], tail[1], tail[2], tail[3]]) as u64;
    if footer_len == 0 || footer_len > 2 * 1024 * 1024 || footer_len + 12 > actual_size {
        return None;
    }
    let mut buf = vec![0u8; footer_len as usize];
    f.seek(SeekFrom::End(-8 - (footer_len as i64))).ok()?;
    f.read_exact(&mut buf).ok()?;

    let mut cur = ThriftCur { buf: &buf, pos: 0 };
    let mut num_rows: Option<i64> = None;
    let mut columns = Vec::new();
    let mut col_types = Vec::new();
    let mut last_fid = 0i16;

    for _ in 0..32 {
        let (fid, wtype) = cur.read_field_hdr(&mut last_fid)?;
        if fid == 0 {
            break;
        }
        match (fid, wtype) {
            (2, 9) => {
                let (_et, count) = cur.read_list_hdr()?;
                for _ in 0..count.min(256) {
                    if let Some((name, phys_t, num_children)) = read_parquet_schema_elem(&mut cur) {
                        if num_children.unwrap_or(0) == 0
                            && name != "schema"
                            && columns.len() < MAX_FACET_ITEMS
                        {
                            columns.push(sanitize_line(&name));
                            col_types.push(phys_t.unwrap_or_else(|| "any".to_string()));
                        }
                    } else {
                        break;
                    }
                }
            }
            (3, 6) => num_rows = cur.read_zigzag_i64(),
            _ => {
                if !cur.skip_value(wtype) {
                    break;
                }
            }
        }
    }
    Some((num_rows, columns, col_types))
}

pub fn infer_column_type(samples: &[&str]) -> &'static str {
    let (mut saw, mut i_ok, mut f_ok, mut b_ok) = (false, true, true, true);
    for s in samples {
        let t = s.trim().trim_matches('"');
        if t.is_empty() {
            continue;
        }
        saw = true;
        i_ok &= t.parse::<i64>().is_ok();
        f_ok &= t.parse::<f64>().is_ok();
        b_ok &= matches!(
            t.to_ascii_lowercase().as_str(),
            "true" | "false" | "0" | "1" | "yes" | "no"
        );
    }
    if !saw {
        "string"
    } else if i_ok {
        "int"
    } else if f_ok {
        "float"
    } else if b_ok {
        "bool"
    } else {
        "string"
    }
}

/// 纯 Rust 解析 CSV / TSV 表头、推断列类型并估算总行数
pub fn extract_delimited_facets(
    path: &Path,
    snippet: &str,
    ext: &str,
    file_name: &str,
    actual_size: Option<i64>,
    kv: &mut Map<String, Value>,
    extra_tags: &mut Vec<String>,
) -> (String, String, Option<String>) {
    let fmt = if ext.eq_ignore_ascii_case("tsv") { "tsv" } else { "csv" };
    let delim = if fmt == "tsv" { '\t' } else { ',' };
    kv.insert("dataset_format".into(), json!(fmt));
    let sample = if !snippet.is_empty() {
        snippet.to_string()
    } else {
        read_head_utf8(path, 16384).unwrap_or_default()
    };
    let mut lines = sample.lines().filter(|l| !l.trim().is_empty());
    let mut cols: Vec<String> = Vec::new();
    let mut sample_rows_in_head = 0usize;
    if let Some(hdr) = lines.next() {
        cols = hdr
            .split(delim)
            .map(|c| scan_untrusted_text(&sanitize_line(c.trim().trim_matches('"')), kv, extra_tags))
            .filter(|c| !c.is_empty())
            .take(MAX_FACET_ITEMS)
            .collect();
        let rows: Vec<&str> = lines.collect();
        sample_rows_in_head = rows.len();
        let sample_rows: Vec<&str> = rows.into_iter().take(5).collect();
        if !cols.is_empty() {
            kv.insert("columns".into(), json!(&cols));
        }
        if !cols.is_empty() && !sample_rows.is_empty() {
            let mut types = Vec::with_capacity(cols.len());
            for idx in 0..cols.len() {
                let vals: Vec<&str> = sample_rows
                    .iter()
                    .filter_map(|r| r.split(delim).nth(idx))
                    .collect();
                types.push(infer_column_type(&vals).to_string());
            }
            kv.insert("column_types".into(), json!(types));
        }
        let sample_cnt = 1 + sample_rows.len();
        let sample_bytes = hdr.len() + 1 + sample_rows.iter().map(|r| r.len() + 1).sum::<usize>();
        let sz = actual_size.unwrap_or(0).max(0) as u64;
        if sample_bytes > 0 && sz > 0 {
            let est = ((sz as f64) * (sample_cnt as f64) / (sample_bytes as f64))
                .round()
                .max(1.0) as u64;
            kv.insert("row_estimate".into(), json!(est.saturating_sub(1)));
        }
    }
    kv.insert("sample_rows_in_head".into(), json!(sample_rows_in_head));
    kv.insert("capabilities".into(), json!(vec![format!("{} 表格结构检索与 DuckDB 直连", fmt)]));
    extra_tags.push(fmt.to_string());
    let purpose = if cols.is_empty() {
        format!("{} 数据集 ({}, {} bytes)", fmt, file_name, actual_size.unwrap_or(0))
    } else {
        format!(
            "{} 表格数据集 ({} 列: {})",
            fmt.to_ascii_uppercase(),
            cols.len(),
            cols.iter().take(4).cloned().collect::<Vec<_>>().join(", ")
        )
    };
    (purpose, file_name.to_string(), Some(format!("数据检索/DuckDB分析: {}", file_name)))
}

/// 纯 Rust 解析 JSONL / NDJSON 联合键集合并估算行数
pub fn extract_jsonl_facets(
    path: &Path,
    snippet: &str,
    ext: &str,
    file_name: &str,
    actual_size: Option<i64>,
    kv: &mut Map<String, Value>,
    extra_tags: &mut Vec<String>,
) -> (String, String, Option<String>) {
    let fmt = if ext.eq_ignore_ascii_case("ndjson") { "ndjson" } else { "jsonl" };
    kv.insert("dataset_format".into(), json!(fmt));
    let sample = if !snippet.is_empty() {
        snippet.to_string()
    } else {
        read_head_utf8(path, 16384).unwrap_or_default()
    };
    let mut keys = BTreeSet::new();
    let (mut valid_rows, mut bytes_seen) = (0usize, 0usize);
    for line in sample.lines().filter(|l| !l.trim().is_empty()).take(12) {
        bytes_seen += line.len() + 1;
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(line) {
            valid_rows += 1;
            for k in map.keys() {
                let clean = scan_untrusted_text(&sanitize_line(k), kv, extra_tags);
                if !clean.is_empty() {
                    keys.insert(clean);
                }
            }
        }
    }
    let cols: Vec<String> = keys.into_iter().take(MAX_FACET_ITEMS).collect();
    if !cols.is_empty() {
        kv.insert("columns".into(), json!(&cols));
    }
    kv.insert("sample_rows_in_head".into(), json!(valid_rows));
    let sz = actual_size.unwrap_or(0).max(0) as u64;
    if valid_rows > 0 && bytes_seen > 0 && sz > 0 {
        let est = ((sz as f64) * (valid_rows as f64) / (bytes_seen as f64))
            .round()
            .max(1.0) as u64;
        kv.insert("row_estimate".into(), json!(est));
    }
    kv.insert("capabilities".into(), json!(vec![format!("{} 流式结构化数据存取", fmt)]));
    extra_tags.push(fmt.to_string());
    let purpose = if cols.is_empty() {
        format!("{} 数据集 ({}, {} bytes)", fmt, file_name, actual_size.unwrap_or(0))
    } else {
        format!(
            "{} 流式数据集 (~{} 行, {} 字段: {})",
            fmt.to_ascii_uppercase(),
            kv.get("row_estimate").and_then(|v| v.as_u64()).unwrap_or(valid_rows as u64),
            cols.len(),
            cols.iter().take(4).cloned().collect::<Vec<_>>().join(", ")
        )
    };
    (purpose, file_name.to_string(), Some(format!("数据检索/DuckDB分析: {}", file_name)))
}

/// 纯 Rust 音频 L1A 容器头解析（MP3 ID3v2+MPEG 帧 / FLAC STREAMINFO / OGG Vorbis & Opus / WAV）
pub fn parse_audio_l1a(data: &[u8], ext: &str, actual_size: u64, kv: &mut Map<String, Value>) {
    if data.len() >= 42 && &data[0..4] == b"fLaC" && (data[4] & 0x7F) == 0 {
        kv.insert("codec".into(), json!("flac"));
        let sr = ((data[18] as u64) << 12) | ((data[19] as u64) << 4) | ((data[20] as u64) >> 4);
        let ch = (((data[20] >> 1) & 0x07) + 1) as u64;
        let samples = (((data[21] & 0x0F) as u64) << 32)
            | ((data[22] as u64) << 24)
            | ((data[23] as u64) << 16)
            | ((data[24] as u64) << 8)
            | (data[25] as u64);
        if sr > 0 {
            kv.insert("sample_rate".into(), json!(sr));
            kv.insert("audio_channels".into(), json!(ch));
            if samples > 0 {
                let dur = (samples as f64) / (sr as f64);
                kv.insert("duration_secs".into(), json!((dur * 100.0).round() / 100.0));
            }
        }
        return;
    }

    if data.len() >= 36 && &data[0..4] == b"OggS" {
        if let Some(pos) = data.windows(7).position(|w| w == b"\x01vorbis") {
            kv.insert("codec".into(), json!("vorbis"));
            if pos + 16 <= data.len() {
                let ch = data[pos + 11] as u64;
                let sr = u32::from_le_bytes([data[pos + 12], data[pos + 13], data[pos + 14], data[pos + 15]]) as u64;
                if ch > 0 {
                    kv.insert("audio_channels".into(), json!(ch));
                }
                if sr > 0 {
                    kv.insert("sample_rate".into(), json!(sr));
                }
            }
            return;
        }
        if let Some(pos) = data.windows(8).position(|w| w == b"OpusHead") {
            kv.insert("codec".into(), json!("opus"));
            if pos + 16 <= data.len() {
                kv.insert("audio_channels".into(), json!(data[pos + 9] as u64));
                let sr = u32::from_le_bytes([data[pos + 12], data[pos + 13], data[pos + 14], data[pos + 15]]) as u64;
                kv.insert("sample_rate".into(), json!(if sr > 0 { sr } else { 48000 }));
            }
            return;
        }
    }

    if ext == "mp3" || (data.len() >= 3 && &data[0..3] == b"ID3") {
        kv.insert("codec".into(), json!("mp3"));
        let mut offset = 0usize;
        if data.len() >= 10 && &data[0..3] == b"ID3" {
            offset = 10
                + ((((data[6] & 0x7F) as usize) << 21)
                    | (((data[7] & 0x7F) as usize) << 14)
                    | (((data[8] & 0x7F) as usize) << 7)
                    | ((data[9] & 0x7F) as usize));
        }
        if offset + 4 <= data.len() {
            for pos in offset..(data.len() - 4) {
                if data[pos] == 0xFF && (data[pos + 1] & 0xE0) == 0xE0 {
                    let (b1, b2) = (data[pos + 1], data[pos + 2]);
                    let (br_idx, sr_idx) = (((b2 >> 4) & 0x0F) as usize, ((b2 >> 2) & 0x03) as usize);
                    if ((b1 >> 3) & 0x03) == 3
                        && ((b1 >> 1) & 0x03) == 1
                        && br_idx > 0
                        && br_idx < 15
                        && sr_idx < 3
                    {
                        const BR: [u64; 15] =
                            [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320];
                        const SR: [u64; 3] = [44100, 48000, 32000];
                        let kbps = BR[br_idx];
                        kv.insert("bitrate_kbps".into(), json!(kbps));
                        kv.insert("sample_rate".into(), json!(SR[sr_idx]));
                        kv.insert(
                            "audio_channels".into(),
                            json!(if ((data[pos + 3] >> 6) & 0x03) == 3 { 1 } else { 2 }),
                        );
                        let audio_bytes = actual_size.saturating_sub(offset as u64);
                        if kbps > 0 {
                            let dur = ((audio_bytes * 8) as f64) / ((kbps * 1000) as f64);
                            kv.insert("duration_secs".into(), json!((dur * 100.0).round() / 100.0));
                        }
                    }
                    break;
                }
            }
        }
    }
}

/// 标量多谓词比较算子
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KvOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    Contains,
    Exists,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KvPredicate {
    pub key: String,
    pub op: KvOp,
    pub value: String,
}

/// 解析多谓词标量过滤表达式（逗号或分号分隔：`key=v`, `!=`, `>=`, `>`, `<=`, `<`, `~`, `has:key` 或裸键 `key`）
pub fn parse_kv_filters(expr: &str) -> Vec<KvPredicate> {
    let mut out = Vec::new();
    for raw_clause in expr.split([',', ';']) {
        let clause = raw_clause.trim();
        if clause.is_empty() {
            continue;
        }
        if let Some(k) = clause.strip_prefix("has:") {
            let key = k.trim();
            if !key.is_empty() {
                out.push(KvPredicate {
                    key: key.to_string(),
                    op: KvOp::Exists,
                    value: String::new(),
                });
            }
            continue;
        }
        let mut matched_op = false;
        for (sep, op) in [
            (">=", KvOp::Ge),
            ("<=", KvOp::Le),
            ("!=", KvOp::Ne),
            ("~", KvOp::Contains),
            (">", KvOp::Gt),
            ("<", KvOp::Lt),
            ("=", KvOp::Eq),
        ] {
            if let Some((k, v)) = clause.split_once(sep) {
                let key = k.trim();
                let val = v.trim().trim_matches('"').trim_matches('\'');
                if !key.is_empty() {
                    out.push(KvPredicate {
                        key: key.to_string(),
                        op,
                        value: val.to_string(),
                    });
                }
                matched_op = true;
                break;
            }
        }
        if !matched_op {
            out.push(KvPredicate {
                key: clause.to_string(),
                op: KvOp::Exists,
                value: String::new(),
            });
        }
    }
    out
}

fn eval_single_predicate(asset: &AssetRecord, pred: &KvPredicate) -> bool {
    let val_opt: Option<Value> = match pred.key.as_str() {
        "asset_type" | "type" => Some(json!(asset.asset_type)),
        "path" => Some(json!(asset.path)),
        "size_bytes" => asset
            .size_bytes
            .map(|s| json!(s))
            .or_else(|| asset.desc_kv.get("size_bytes").cloned()),
        _ => asset.desc_kv.get(&pred.key).cloned(),
    };
    let Some(val) = val_opt else {
        return matches!(pred.op, KvOp::Ne);
    };
    match pred.op {
        KvOp::Exists => match &val {
            Value::Null => false,
            Value::String(s) => !s.trim().is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(m) => !m.is_empty(),
            _ => true,
        },
        KvOp::Gt | KvOp::Ge | KvOp::Lt | KvOp::Le => {
            let (Some(actual), Ok(target)) = (
                val.as_f64()
                    .or_else(|| val.as_str().and_then(|s| s.trim().parse::<f64>().ok())),
                pred.value.parse::<f64>(),
            ) else {
                return false;
            };
            match pred.op {
                KvOp::Gt => actual > target,
                KvOp::Ge => actual >= target,
                KvOp::Lt => actual < target,
                KvOp::Le => actual <= target,
                _ => false,
            }
        }
        KvOp::Eq | KvOp::Ne => {
            let eq = match &val {
                Value::String(s) => s.eq_ignore_ascii_case(&pred.value),
                Value::Number(n) => n
                    .as_f64()
                    .zip(pred.value.parse::<f64>().ok())
                    .map(|(a, b)| (a - b).abs() < 1e-9)
                    .unwrap_or_else(|| n.to_string() == pred.value),
                Value::Bool(b) => pred.value.parse::<bool>().map(|t| *b == t).unwrap_or(false),
                Value::Array(arr) => arr.iter().any(|x| {
                    x.as_str()
                        .map(|s| s.eq_ignore_ascii_case(&pred.value))
                        .unwrap_or(false)
                }),
                _ => val.to_string().eq_ignore_ascii_case(&pred.value),
            };
            if matches!(pred.op, KvOp::Eq) { eq } else { !eq }
        }
        KvOp::Contains => {
            let needle = pred.value.to_ascii_lowercase();
            match &val {
                Value::String(s) => s.to_ascii_lowercase().contains(&needle),
                Value::Array(arr) => arr
                    .iter()
                    .any(|x| x.to_string().to_ascii_lowercase().contains(&needle)),
                _ => val.to_string().to_ascii_lowercase().contains(&needle),
            }
        }
    }
}

/// 对单条资产执行多谓词 AND 求值
pub fn matches_kv_filters(asset: &AssetRecord, predicates: &[KvPredicate]) -> bool {
    predicates.iter().all(|p| eval_single_predicate(asset, p))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MultimodalBackfillReport {
    pub scanned: usize,
    pub upgraded: usize,
    pub skipped_existing: usize,
    pub skipped_missing_on_disk: usize,
    pub errors: usize,
    pub by_asset_type: BTreeMap<String, usize>,
    pub by_modality: BTreeMap<String, usize>,
    pub duration_ms: u64,
    pub updated_at: String,
}

/// 幂等升级存量 `other` 误判或旧版 `schema_version` 资产（无损保留已有 L2 Ollama 精炼语义与向量就绪态）
pub fn backfill_multimodal_assets(
    storage: &Storage,
    limit: usize,
    path_prefix: Option<&str>,
) -> Result<MultimodalBackfillReport> {
    let started = Instant::now();
    let mut report = MultimodalBackfillReport {
        updated_at: Local::now().to_rfc3339(),
        ..Default::default()
    };
    let fetch_cap = if limit == 0 { 100_000 } else { limit.max(50_000) };

    for existing in storage.list_assets(fetch_cap)? {
        if existing.status != "active"
            || existing.asset_type == "directory"
            || existing.asset_type == "project"
        {
            continue;
        }
        if let Some(prefix) = path_prefix.map(str::trim).filter(|s| !s.is_empty()) {
            if !existing.path.starts_with(prefix) {
                continue;
            }
        }
        if limit > 0 && report.upgraded >= limit {
            break;
        }

        report.scanned += 1;
        let p = Path::new(&existing.path);
        if !p.exists() || !p.is_file() {
            report.skipped_missing_on_disk += 1;
            continue;
        }

        let cls = classify_asset_path(p);
        let version_ok = existing.desc_kv.get("schema_version").and_then(|v| v.as_str())
            == Some("2.0")
            && existing.desc_kv.get("mime_type").is_some()
            && existing.desc_kv.get("modality").is_some();
        if version_ok && existing.asset_type == cls.asset_type {
            report.skipped_existing += 1;
            continue;
        }

        let size_opt = p.metadata().ok().map(|m| m.len() as i64).or(existing.size_bytes);
        let extracted = HeuristicMetadataExtractor::extract(
            p,
            cls.asset_type,
            &existing.scan_tier,
            None,
            size_opt,
        );

        let mut candidate = existing.clone();
        candidate.asset_type = cls.asset_type.to_string();
        candidate.description = Some(extracted.description.clone());
        candidate.usage = extracted.usage.clone().or_else(|| existing.usage.clone());
        candidate.desc_kv = extracted.desc_kv.clone();
        candidate.size_bytes = size_opt;
        let mut merged_tags = existing.tags.clone();
        for t in cls.tags.iter().cloned().chain(extracted.extra_tags) {
            if !merged_tags.contains(&t) {
                merged_tags.push(t);
            }
        }
        candidate.tags = merged_tags;

        let mut merged = merge_with_existing_asset(candidate, Some(&existing));
        merged.asset_type = cls.asset_type.to_string();
        if existing.asset_type != cls.asset_type {
            if let Some(obj) = merged.desc_kv.as_object_mut() {
                obj.insert("schema_version".into(), json!("2.0"));
                obj.insert("mime_type".into(), json!(cls.mime_type));
                obj.insert("modality".into(), json!(cls.modality));
                if existing.asset_type == "other" {
                    obj.insert("embedding_status".into(), json!("stale"));
                }
            }
        }

        if let Err(e) = storage.upsert_asset(&merged) {
            eprintln!("[Multimodal][WARN] 回填更新失败 ({}): {}", merged.path, e);
            report.errors += 1;
            continue;
        }
        report.upgraded += 1;
        *report.by_asset_type.entry(cls.asset_type.to_string()).or_insert(0) += 1;
        *report.by_modality.entry(cls.modality.to_string()).or_insert(0) += 1;
    }

    report.duration_ms = started.elapsed().as_millis() as u64;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_classify_magic_and_extensions() {
        let dir = tempdir().expect("tempdir");
        let fake_bin = dir.path().join("photo.bin");
        std::fs::write(&fake_bin, b"\x89PNG\r\n\x1a\nfake").expect("write png magic");
        let c1 = classify_asset_path(&fake_bin);
        assert_eq!(c1.asset_type, "media");
        assert_eq!(c1.modality, "image");
        assert_eq!(c1.mime_type, "image/png");

        let deck = dir.path().join("slide.html");
        std::fs::write(&deck, "<!DOCTYPE html><html><title>Deck</title></html>").expect("html");
        let c2 = classify_asset_path(&deck);
        assert_eq!(c2.asset_type, "document");
        assert_eq!(c2.mime_type, "text/html");

        let parq = dir.path().join("metrics.parquet");
        std::fs::write(&parq, b"PAR1broken").expect("parquet");
        let c3 = classify_asset_path(&parq);
        assert_eq!(c3.asset_type, "dataset");
        assert_eq!(c3.mime_type, "application/vnd.apache.parquet");
    }

    #[test]
    fn test_html_csv_jsonl_parquet_and_audio_l1a() {
        let dir = tempdir().expect("tempdir");

        // 1. HTML + inline SVG + script 剥离
        let html_p = dir.path().join("report.html");
        std::fs::write(
            &html_p,
            "<html><head><title>架构演进</title><script>var secret='ignore';</script></head>\
             <body><h1>第一部分</h1><h2>查询优化</h2><section>A</section><section>B</section>\
             <svg></svg><p>Pure Rust parser works</p></body></html>",
        )
        .expect("write html");
        let mut h_kv = Map::new();
        let mut h_tags = Vec::new();
        let (hp, _, _) = extract_html_facets(&html_p, "", "report.html", Some(200), &mut h_kv, &mut h_tags);
        assert!(hp.contains("架构演进"));
        assert_eq!(h_kv.get("section_count").and_then(|v| v.as_u64()), Some(2));
        assert_eq!(h_kv.get("inline_svg_count").and_then(|v| v.as_u64()), Some(1));
        assert!(h_kv
            .get("html_excerpt")
            .and_then(|v| v.as_str())
            .is_some_and(|s| s.contains("Pure Rust parser") && !s.contains("secret")));

        // 2. CSV 推断列类型与行数估计
        let csv_p = dir.path().join("sales.csv");
        let csv_body = "id,price,active,region\n1,12.5,true,shanghai\n2,8.0,false,beijing\n";
        std::fs::write(&csv_p, csv_body).expect("write csv");
        let mut c_kv = Map::new();
        let mut c_tags = Vec::new();
        extract_delimited_facets(
            &csv_p,
            csv_body,
            "csv",
            "sales.csv",
            Some(csv_body.len() as i64),
            &mut c_kv,
            &mut c_tags,
        );
        assert_eq!(c_kv.get("columns"), Some(&json!(["id", "price", "active", "region"])));
        assert_eq!(
            c_kv.get("column_types"),
            Some(&json!(["int", "float", "bool", "string"]))
        );

        // 3. JSONL 联合字段解析
        let jsonl_p = dir.path().join("events.jsonl");
        let j_body = "{\"id\":1,\"kind\":\"click\"}\n{\"id\":2,\"latency_ms\":15}\n";
        std::fs::write(&jsonl_p, j_body).expect("write jsonl");
        let mut j_kv = Map::new();
        let mut j_tags = Vec::new();
        extract_jsonl_facets(
            &jsonl_p,
            j_body,
            "jsonl",
            "events.jsonl",
            Some(j_body.len() as i64),
            &mut j_kv,
            &mut j_tags,
        );
        assert_eq!(j_kv.get("columns"), Some(&json!(["id", "kind", "latency_ms"])));

        // 4. 损坏的 Parquet 文件零 panic
        let bad_pq = dir.path().join("corrupt.parquet");
        std::fs::write(&bad_pq, b"PAR1truncated_footer_garbage_PAR1").expect("bad pq");
        assert!(parse_parquet_footer_l1a(&bad_pq, 33).is_none());

        // 5. FLAC header
        let mut flac = vec![0u8; 42];
        flac[0..4].copy_from_slice(b"fLaC");
        flac[4] = 0x00;
        // sr = 44100 (0x0AC44), ch = 2 (ch-1 = 1), samples = 44100
        flac[18] = 0x0A;
        flac[19] = 0xC4;
        flac[20] = 0x42; // low nibble 4, channels=2 -> (1<<1)=2
        flac[24] = 0xAC;
        flac[25] = 0x44;
        let mut a_kv = Map::new();
        parse_audio_l1a(&flac, "flac", 1024, &mut a_kv);
        assert_eq!(a_kv.get("codec"), Some(&json!("flac")));
        assert_eq!(a_kv.get("sample_rate"), Some(&json!(44100)));
        assert_eq!(a_kv.get("audio_channels"), Some(&json!(2)));
    }

    #[test]
    fn test_parquet_footer_and_kv_filter_and_backfill() {
        let dir = tempdir().expect("tempdir");

        // 构造真实 Parquet Thrift Compact Footer（id: INT64, name: BYTE_ARRAY, num_rows = 42）
        let pq_path = dir.path().join("orders.parquet");
        let mut pq = Vec::new();
        pq.extend_from_slice(b"PAR1");
        let mut footer = Vec::new();
        // field 1: version = 1 (zigzag(1)=2)
        footer.extend_from_slice(&[0x15, 0x02]);
        // field 2: schema list<struct> len 3: delta=1 -> 0x19, count=3, et=12 -> 0x3C
        footer.extend_from_slice(&[0x19, 0x3C]);
        // elem 0: root schema (field 4 name="schema", field 5 num_children=2)
        footer.push(0x48);
        footer.push(6);
        footer.extend_from_slice(b"schema");
        footer.extend_from_slice(&[0x15, 0x04, 0x00]); // fid 4->5 delta 1, zigzag(2)=4, stop
        // elem 1: id (fid1 type=INT64(2->4), fid4 name="id")
        footer.extend_from_slice(&[0x15, 0x04, 0x38, 2]);
        footer.extend_from_slice(b"id");
        footer.push(0x00);
        // elem 2: name (fid1 type=BYTE_ARRAY(6->12), fid4 name="name")
        footer.extend_from_slice(&[0x15, 0x0C, 0x38, 4]);
        footer.extend_from_slice(b"name");
        footer.push(0x00);
        // field 3: num_rows = 42 (fid 2->3 delta 1, i64 wtype 6 -> 0x16, zigzag(42)=84)
        footer.extend_from_slice(&[0x16, 84, 0x00]);
        let flen = footer.len() as u32;
        pq.extend_from_slice(&footer);
        pq.extend_from_slice(&flen.to_le_bytes());
        pq.extend_from_slice(b"PAR1");
        let mut pf = File::create(&pq_path).expect("create pq");
        pf.write_all(&pq).expect("write pq");
        let parsed = parse_parquet_footer_l1a(&pq_path, pq.len() as u64).expect("parquet parse");
        assert_eq!(parsed.0, Some(42));
        assert_eq!(parsed.1, vec!["id".to_string(), "name".to_string()]);
        assert_eq!(parsed.2, vec!["int64".to_string(), "bytes".to_string()]);

        // KV predicate filter 测试
        let preds = parse_kv_filters("importance_score>=0.7, modality=document; has:doc_title; custom_note~rust; extra_flag");
        assert_eq!(preds.len(), 5);
        let rec = AssetRecord {
            id: "a1".into(),
            path: "/tmp/a.html".into(),
            asset_type: "document".into(),
            owner: "u".into(),
            security_level: "internal".into(),
            scan_tier: "deep".into(),
            description: Some("L2 custom".into()),
            desc_kv: json!({
                "importance_score": 0.85,
                "modality": "document",
                "doc_title": "Design",
                "custom_note": "Rust Pure Parser",
                "extra_flag": true,
            }),
            usage: None,
            size_bytes: Some(128),
            file_count: None,
            tags: vec![],
            status: "active".into(),
        };
        assert!(matches_kv_filters(&rec, &preds));

        // 幂等无损 Backfill 测试：旧 `other` 记录应被升级且保留 `ollama_enriched` 描述
        let st = Storage::open_in_memory().expect("mem db");
        let old_html = dir.path().join("legacy.html");
        std::fs::write(&old_html, "<html><title>Legacy Rich</title><body><p>Body</p></body></html>")
            .expect("legacy html");
        st.upsert_asset(&AssetRecord {
            id: "leg-1".into(),
            path: old_html.to_string_lossy().to_string(),
            asset_type: "other".into(),
            owner: "user:test".into(),
            security_level: "internal".into(),
            scan_tier: "deep".into(),
            description: Some("手工与Ollama精炼描述".into()),
            desc_kv: json!({
                "annotation_status": "ollama_enriched",
                "content_hash": "old_mismatch",
            }),
            usage: Some("preserve usage".into()),
            size_bytes: Some(10),
            file_count: None,
            tags: vec!["legacy".into()],
            status: "active".into(),
        })
        .expect("seed old");
        let rep = backfill_multimodal_assets(&st, 10, None).expect("backfill");
        assert_eq!(rep.upgraded, 1);
        let after = st
            .get_asset_by_path(old_html.to_string_lossy().as_ref())
            .expect("get")
            .expect("exists");
        assert_eq!(after.asset_type, "document");
        assert_eq!(
            after.desc_kv.get("schema_version").and_then(|v| v.as_str()),
            Some("2.0")
        );
        assert_eq!(
            after.desc_kv.get("mime_type").and_then(|v| v.as_str()),
            Some("text/html")
        );
    }

    #[test]
    fn test_run_command_bounded_timeout_kills_sleep() {
        let started = Instant::now();
        let out = run_command_bounded("sh", &["-c", "sleep 2; echo done"], 80);
        assert!(out.is_none());
        assert!(started.elapsed().as_millis() < 1200);
    }
}
