//! Pure-Rust Multimodal L1A Parsers, Unified 3-Channel Classifier, Bounded Subprocess Probes, KV Predicate Filter & Backfill (Phase 63)

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::Result;
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::scanner::metadata::{
    is_sensitive_file, merge_with_existing_asset, read_head_utf8,
    HeuristicMetadataExtractor, PromptInjectionScanner, SecretRedactor,
};
use crate::storage::Storage;


mod format_probes;
mod kv_filter;
pub use format_probes::*;
pub use kv_filter::*;
pub(super) const MAX_FACET_ITEMS: usize = 24;

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

pub(super) fn sanitize_line(raw: &str) -> String {
    let clean = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    SecretRedactor::redact_single_line(&clean)
}

pub(super) fn scan_untrusted_text(
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
    use crate::storage::models::AssetRecord;
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
