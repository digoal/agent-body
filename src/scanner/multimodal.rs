use std::fs::File;
use std::io::Read;
use std::path::Path;
use serde_json::{json, Map, Value};
use crate::scanner::metadata::{truncate_chars, PromptInjectionScanner, SecretRedactor};
use crate::scanner::multimodal_ext::{
    effective_multimodal_tool_policy, parse_audio_l1a, parse_parquet_footer_l1a, run_command_bounded,
};

/// 根据资产类别与扩展名推导统一多模态类型 (`modality`)
pub fn infer_modality(asset_type: &str, ext: &str) -> &'static str {
    let e = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    match e.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "ico" | "svg" => "image",
        "mp3" | "wav" | "ogg" | "flac" | "m4a" | "aac" => "audio",
        "mp4" | "webm" | "mov" | "mkv" | "avi" => "video",
        "pdf" => "pdf",
        "docx" | "pptx" | "xlsx" => "office",
        "csv" | "tsv" | "jsonl" | "ndjson" | "parquet" | "db" | "sqlite" | "sqlite3" | "duckdb" => {
            "dataset"
        }
        _ => match asset_type {
            "media" => "media",
            "dataset" => "dataset",
            "document" => "document",
            "script" | "project" => "code",
            "config" => "config",
            _ => "other",
        },
    }
}

/// RFC 4648 Base64 编码器（纯 Rust 实现，零额外 crate 依赖）
pub fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(triple >> 18) & 0x3F] as char);
        out.push(TABLE[(triple >> 12) & 0x3F] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(triple >> 6) & 0x3F] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[triple & 0x3F] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn read_head_bytes(path: &Path, max_bytes: usize) -> Option<Vec<u8>> {
    let f = File::open(path).ok()?;
    let mut buf = Vec::with_capacity(max_bytes.min(64 * 1024));
    let mut limited = f.take(max_bytes as u64);
    limited.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn record_injection_if_any(
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

/// 纯 Rust 解析常见图片魔数与尺寸头（PNG / JPEG / GIF / WebP / BMP）
fn parse_raster_image_header(bytes: &[u8], fmt: &str) -> (Option<u32>, Option<u32>, Option<&'static str>) {
    // PNG: 8-byte signature + IHDR length(4) + "IHDR"(4) + width(4 BE) + height(4 BE) + bit_depth + color_type
    if bytes.len() >= 26 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n" && &bytes[12..16] == b"IHDR" {
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        let color_type = match bytes[25] {
            0 => Some("grayscale"),
            2 => Some("rgb"),
            3 => Some("indexed"),
            4 => Some("grayscale_alpha"),
            6 => Some("rgba"),
            _ => None,
        };
        return (Some(w), Some(h), color_type);
    }

    // GIF87a / GIF89a
    if bytes.len() >= 10 && (&bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a") {
        let w = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
        let h = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
        return (Some(w), Some(h), Some("indexed"));
    }

    // BMP
    if bytes.len() >= 26 && &bytes[0..2] == b"BM" {
        let w = u32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]);
        let h = i32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]).unsigned_abs();
        return (Some(w), Some(h), Some("rgb"));
    }

    // WebP (RIFF....WEBP)
    if bytes.len() >= 30 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        if &bytes[12..16] == b"VP8X" && bytes.len() >= 30 {
            let w = 1 + (bytes[24] as u32 | ((bytes[25] as u32) << 8) | ((bytes[26] as u32) << 16));
            let h = 1 + (bytes[27] as u32 | ((bytes[28] as u32) << 8) | ((bytes[29] as u32) << 16));
            return (Some(w), Some(h), Some("rgba"));
        }
        if &bytes[12..16] == b"VP8L" && bytes.len() >= 25 {
            let b0 = bytes[21] as u32;
            let b1 = bytes[22] as u32;
            let b2 = bytes[23] as u32;
            let b3 = bytes[24] as u32;
            let bits = b0 | (b1 << 8) | (b2 << 16) | (b3 << 24);
            let w = (bits & 0x3FFF) + 1;
            let h = ((bits >> 14) & 0x3FFF) + 1;
            return (Some(w), Some(h), Some("rgba"));
        }
        if &bytes[12..16] == b"VP8 " && bytes.len() >= 30 {
            let w = (u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3FFF) as u32;
            let h = (u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3FFF) as u32;
            return (Some(w), Some(h), Some("ycbcr"));
        }
    }

    // JPEG SOF scanner
    if (fmt == "jpg" || fmt == "jpeg" || (bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xD8))
        && bytes.len() >= 4
        && bytes[0] == 0xFF
        && bytes[1] == 0xD8
    {
        let mut idx = 2usize;
        while idx + 4 <= bytes.len() {
            if bytes[idx] != 0xFF {
                idx += 1;
                continue;
            }
            while idx < bytes.len() && bytes[idx] == 0xFF {
                idx += 1;
            }
            if idx >= bytes.len() {
                break;
            }
            let marker = bytes[idx];
            idx += 1;
            // Standalone markers without length
            if marker == 0x00 || marker == 0x01 || (0xD0..=0xD9).contains(&marker) {
                if marker == 0xD9 || marker == 0xDA {
                    break;
                }
                continue;
            }
            if idx + 2 > bytes.len() {
                break;
            }
            let seg_len = u16::from_be_bytes([bytes[idx], bytes[idx + 1]]) as usize;
            if seg_len < 2 || idx + seg_len > bytes.len() {
                break;
            }
            let is_sof = matches!(
                marker,
                0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF
            );
            if is_sof && seg_len >= 7 {
                let h = u16::from_be_bytes([bytes[idx + 3], bytes[idx + 4]]) as u32;
                let w = u16::from_be_bytes([bytes[idx + 5], bytes[idx + 6]]) as u32;
                let comp = bytes[idx + 7];
                let color = match comp {
                    1 => Some("grayscale"),
                    3 => Some("ycbcr"),
                    4 => Some("cmyk"),
                    _ => None,
                };
                return (Some(w), Some(h), color);
            }
            idx += seg_len;
        }
    }

    (None, None, None)
}

fn extract_svg_attr(snippet: &str, attr: &str) -> Option<String> {
    let double_pat = format!("{}=\"", attr);
    if let Some(i) = snippet.find(&double_pat) {
        let rest = &snippet[i + double_pat.len()..];
        if let Some(end) = rest.find('"') {
            return Some(rest[..end].trim().to_string());
        }
    }
    let single_pat = format!("{}='", attr);
    if let Some(i) = snippet.find(&single_pat) {
        let rest = &snippet[i + single_pat.len()..];
        if let Some(end) = rest.find('\'') {
            return Some(rest[..end].trim().to_string());
        }
    }
    None
}

fn extract_svg_text_nodes(snippet: &str) -> Vec<String> {
    let mut results = Vec::new();
    for tag in ["<title", "<desc", "<text", "<tspan"] {
        let mut cursor = snippet;
        while let Some(start) = cursor.find(tag) {
            let after_tag = &cursor[start + tag.len()..];
            let Some(gt) = after_tag.find('>') else {
                break;
            };
            let body = &after_tag[gt + 1..];
            let Some(lt) = body.find('<') else {
                break;
            };
            let raw_text = body[..lt].split_whitespace().collect::<Vec<_>>().join(" ");
            if !raw_text.is_empty() && raw_text.chars().count() >= 2 {
                let cleaned = truncate_chars(&raw_text, 48);
                if !results.contains(&cleaned) {
                    results.push(cleaned);
                }
                if results.len() >= 12 {
                    return results;
                }
            }
            cursor = &body[lt..];
        }
    }
    results
}

/// 图片类多模态 L1 元数据提取（SVG 文本/尺寸 + 光栅头分辨率与色彩模式）
pub fn extract_image_metadata(
    path: &Path,
    snippet: &str,
    ext: &str,
    file_name: &str,
    size_bytes: Option<i64>,
    kv: &mut Map<String, Value>,
    extra_tags: &mut Vec<String>,
) -> (String, String, Option<String>) {
    let fmt = if ext.is_empty() { "image" } else { ext };
    kv.insert("modality".to_string(), json!("image"));
    kv.insert("media_format".to_string(), json!(fmt));
    extra_tags.push("image".to_string());
    if fmt != "image" {
        extra_tags.push(fmt.to_string());
    }

    if fmt == "svg" {
        let svg_text = if snippet.is_empty() {
            read_head_bytes(path, 32 * 1024)
                .map(|b| String::from_utf8_lossy(&b).to_string())
                .unwrap_or_default()
        } else {
            snippet.to_string()
        };
        let redacted_svg = SecretRedactor::redact(&svg_text);
        let viewbox = extract_svg_attr(&redacted_svg, "viewBox");
        let w_attr = extract_svg_attr(&redacted_svg, "width");
        let h_attr = extract_svg_attr(&redacted_svg, "height");
        let text_nodes = extract_svg_text_nodes(&redacted_svg)
            .into_iter()
            .map(|t| record_injection_if_any(&t, kv, extra_tags))
            .collect::<Vec<_>>();

        if let Some(ref vb) = viewbox {
            kv.insert("viewbox".to_string(), json!(vb));
        }
        if let Some(ref w) = w_attr {
            kv.insert("svg_width".to_string(), json!(w));
        }
        if let Some(ref h) = h_attr {
            kv.insert("svg_height".to_string(), json!(h));
        }
        if !text_nodes.is_empty() {
            kv.insert("svg_text_snippets".to_string(), json!(text_nodes));
            kv.insert("ocr_keywords".to_string(), json!(text_nodes));
        }

        let summary_labels = kv
            .get("svg_text_snippets")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str())
                    .take(3)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        let purpose = if !summary_labels.is_empty() {
            format!("SVG 矢量图 ({}): [{}]", file_name, truncate_chars(&summary_labels, 60))
        } else if let Some(ref vb) = viewbox {
            format!("SVG 矢量图形 ({}, viewBox: {})", file_name, vb)
        } else {
            format!("SVG 矢量图形 ({}, {} bytes)", file_name, size_bytes.unwrap_or(0))
        };
        kv.insert("capabilities".to_string(), json!(vec!["SVG 矢量图表/架构图理解".to_string()]));
        return (purpose, file_name.to_string(), None);
    }

    let head = read_head_bytes(path, 65_536).unwrap_or_default();
    let (width, height, color_type) = parse_raster_image_header(&head, fmt);
    if let Some(w) = width {
        kv.insert("width".to_string(), json!(w));
    }
    if let Some(h) = height {
        kv.insert("height".to_string(), json!(h));
    }
    if let (Some(w), Some(h)) = (width, height) {
        kv.insert("dimensions".to_string(), json!(format!("{}x{}", w, h)));
    }
    if let Some(ct) = color_type {
        kv.insert("color_type".to_string(), json!(ct));
    }

    let purpose = match (width, height) {
        (Some(w), Some(h)) => format!(
            "{} 图像 ({}, {}x{}, {} bytes)",
            fmt.to_uppercase(),
            file_name,
            w,
            h,
            size_bytes.unwrap_or(0)
        ),
        _ => format!(
            "{} 图像资源 ({}, {} bytes)",
            fmt.to_uppercase(),
            file_name,
            size_bytes.unwrap_or(0)
        ),
    };
    kv.insert(
        "capabilities".to_string(),
        json!(vec![format!("{} 图片元信息与可选视觉语义检索", fmt.to_uppercase())]),
    );
    (purpose, file_name.to_string(), None)
}

fn parse_wav_header(bytes: &[u8]) -> Option<(u16, u32, f64)> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return None;
    }
    let mut idx = 12usize;
    let mut channels = None;
    let mut sample_rate = None;
    let mut byte_rate = None;
    let mut data_size = None;
    while idx + 8 <= bytes.len() {
        let id = &bytes[idx..idx + 4];
        let chunk_len = u32::from_le_bytes([bytes[idx + 4], bytes[idx + 5], bytes[idx + 6], bytes[idx + 7]]) as usize;
        let payload = idx + 8;
        if id == b"fmt " && payload + 16 <= bytes.len() {
            channels = Some(u16::from_le_bytes([bytes[payload + 2], bytes[payload + 3]]));
            sample_rate = Some(u32::from_le_bytes([
                bytes[payload + 4],
                bytes[payload + 5],
                bytes[payload + 6],
                bytes[payload + 7],
            ]));
            byte_rate = Some(u32::from_le_bytes([
                bytes[payload + 8],
                bytes[payload + 9],
                bytes[payload + 10],
                bytes[payload + 11],
            ]));
        } else if id == b"data" {
            data_size = Some(chunk_len);
            break;
        }
        let step = 8usize.saturating_add(chunk_len).saturating_add(chunk_len % 2);
        if step == 0 {
            break;
        }
        idx = idx.saturating_add(step);
    }
    let ch = channels?;
    let sr = sample_rate?;
    let dur = match (byte_rate, data_size) {
        (Some(br), Some(ds)) if br > 0 => (ds as f64) / (br as f64),
        _ => 0.0,
    };
    Some((ch, sr, dur))
}

fn parse_mp4_mvhd_duration(bytes: &[u8]) -> Option<f64> {
    let mut idx = 0usize;
    while idx + 8 <= bytes.len() {
        let size = u32::from_be_bytes([bytes[idx], bytes[idx + 1], bytes[idx + 2], bytes[idx + 3]]) as usize;
        let box_type = &bytes[idx + 4..idx + 8];
        if size < 8 || idx + size > bytes.len() {
            break;
        }
        if box_type == b"moov" {
            let inner = &bytes[idx + 8..idx + size];
            let mut j = 0usize;
            while j + 8 <= inner.len() {
                let s2 = u32::from_be_bytes([inner[j], inner[j + 1], inner[j + 2], inner[j + 3]]) as usize;
                let t2 = &inner[j + 4..j + 8];
                if s2 < 8 || j + s2 > inner.len() {
                    break;
                }
                if t2 == b"mvhd" && s2 >= 28 {
                    let version = inner[j + 8];
                    if version == 0 && j + 28 <= inner.len() {
                        let timescale = u32::from_be_bytes([inner[j + 20], inner[j + 21], inner[j + 22], inner[j + 23]]);
                        let duration = u32::from_be_bytes([inner[j + 24], inner[j + 25], inner[j + 26], inner[j + 27]]);
                        if timescale > 0 {
                            return Some(duration as f64 / timescale as f64);
                        }
                    } else if version == 1 && j + 40 <= inner.len() {
                        let timescale = u32::from_be_bytes([inner[j + 28], inner[j + 29], inner[j + 30], inner[j + 31]]);
                        let duration = u64::from_be_bytes([
                            inner[j + 32], inner[j + 33], inner[j + 34], inner[j + 35],
                            inner[j + 36], inner[j + 37], inner[j + 38], inner[j + 39],
                        ]);
                        if timescale > 0 {
                            return Some(duration as f64 / timescale as f64);
                        }
                    }
                }
                j += s2;
            }
        }
        idx += size;
    }
    None
}

fn try_ffprobe_metadata(path: &Path, timeout_ms: u64) -> Option<Value> {
    let p_str = path.to_str()?;
    let out = run_command_bounded(
        "ffprobe",
        &[
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            p_str,
        ],
        timeout_ms,
    )?;
    serde_json::from_str(&out).ok()
}

/// 音视频与媒体资产统一入口（图片委托至 `extract_image_metadata`）
pub fn extract_media_metadata(
    path: &Path,
    snippet: &str,
    ext: &str,
    file_name: &str,
    size_bytes: Option<i64>,
    kv: &mut Map<String, Value>,
    extra_tags: &mut Vec<String>,
) -> (String, String, Option<String>) {
    let modality = infer_modality("media", ext);
    if modality == "image" {
        return extract_image_metadata(path, snippet, ext, file_name, size_bytes, kv, extra_tags);
    }

    let fmt = if ext.is_empty() { "media" } else { ext };
    kv.insert("modality".to_string(), json!(modality));
    kv.insert("media_format".to_string(), json!(fmt));
    if modality != "media" {
        extra_tags.push(modality.to_string());
    }
    extra_tags.push(fmt.to_string());

    let head = read_head_bytes(path, 256 * 1024).unwrap_or_default();
    if fmt == "wav" {
        if let Some((ch, sr, dur)) = parse_wav_header(&head) {
            kv.insert("audio_channels".to_string(), json!(ch));
            kv.insert("sample_rate".to_string(), json!(sr));
            kv.insert("codec".to_string(), json!("pcm_wav"));
            if dur > 0.0 {
                kv.insert("duration_secs".to_string(), json!((dur * 100.0).round() / 100.0));
            }
        }
    } else if matches!(fmt, "mp3" | "flac" | "ogg" | "oga" | "opus") {
        parse_audio_l1a(&head, fmt, size_bytes.unwrap_or(0).max(0) as u64, kv);
    } else if matches!(fmt, "mp4" | "mov" | "m4a") {
        if let Some(dur) = parse_mp4_mvhd_duration(&head) {
            kv.insert("duration_secs".to_string(), json!((dur * 100.0).round() / 100.0));
        }
    }

    // 若配置允许且宿主机存在 ffprobe，则有界超时补充精确编解码器/分辨率/时长
    let (tools_enabled, tool_timeout_ms) = effective_multimodal_tool_policy();
    if tools_enabled && (!kv.contains_key("codec") || (modality == "video" && !kv.contains_key("dimensions"))) {
        if let Some(probe) = try_ffprobe_metadata(path, tool_timeout_ms) {
            if let Some(dur_s) = probe
                .get("format")
                .and_then(|f| f.get("duration"))
                .and_then(|d| d.as_str())
                .and_then(|s| s.parse::<f64>().ok())
            {
                kv.insert("duration_secs".to_string(), json!((dur_s * 100.0).round() / 100.0));
            }
            if let Some(streams) = probe.get("streams").and_then(|s| s.as_array()) {
                if let Some(first_codec) = streams.first().and_then(|s| s.get("codec_name")).and_then(|v| v.as_str()) {
                    kv.insert("codec".to_string(), json!(first_codec));
                }
                for st in streams {
                    if let (Some(w), Some(h)) = (
                        st.get("width").and_then(|v| v.as_u64()),
                        st.get("height").and_then(|v| v.as_u64()),
                    ) {
                        kv.insert("width".to_string(), json!(w));
                        kv.insert("height".to_string(), json!(h));
                        kv.insert("dimensions".to_string(), json!(format!("{}x{}", w, h)));
                        break;
                    }
                }
                for st in streams {
                    if let Some(sr) = st.get("sample_rate").and_then(|v| v.as_str()).and_then(|s| s.parse::<u32>().ok()) {
                        kv.insert("sample_rate".to_string(), json!(sr));
                        break;
                    }
                }
            }
        }
    }

    let dur_part = kv
        .get("duration_secs")
        .and_then(|v| v.as_f64())
        .map(|d| format!(", {:.1}s", d))
        .unwrap_or_default();
    let purpose = format!(
        "{} {} 媒体 ({}{}, {} bytes)",
        fmt.to_uppercase(),
        modality,
        file_name,
        dur_part,
        size_bytes.unwrap_or(0)
    );
    kv.insert("capabilities".to_string(), json!(vec![format!("{} 媒体元信息检索", modality)]));
    (purpose, file_name.to_string(), None)
}

/// PDF L1 元数据提取（魔数校验 + 可选 `pdfinfo`/`pdftotext` + 纯 Rust 回退计数）
pub fn extract_pdf_metadata(
    path: &Path,
    file_name: &str,
    size_bytes: Option<i64>,
    kv: &mut Map<String, Value>,
    extra_tags: &mut Vec<String>,
) -> (String, String, Option<String>) {
    kv.insert("modality".to_string(), json!("pdf"));
    kv.insert("doc_category".to_string(), json!("pdf"));
    extra_tags.push("pdf".to_string());

    let head = read_head_bytes(path, 128 * 1024).unwrap_or_default();
    if head.starts_with(b"%PDF-") {
        let ver: String = head[5..head.len().min(12)]
            .iter()
            .take_while(|&&b| b != b'\r' && b != b'\n' && b != b' ')
            .map(|&b| b as char)
            .collect();
        if !ver.is_empty() {
            kv.insert("pdf_version".to_string(), json!(ver));
        }
    }

    // 受有界超时与配置开关控制的 pdfinfo / pdftotext 探针
    let (tools_enabled, tool_timeout_ms) = effective_multimodal_tool_policy();
    if tools_enabled {
        if let Some(p_str) = path.to_str() {
            if let Some(txt) = run_command_bounded("pdfinfo", &[p_str], tool_timeout_ms) {
                for line in txt.lines() {
                    if let Some((k, v)) = line.split_once(':') {
                        let key = k.trim();
                        let val = v.trim();
                        match key {
                            "Pages" => {
                                if let Ok(p) = val.parse::<usize>() {
                                    kv.insert("page_count".to_string(), json!(p));
                                }
                            }
                            "Title" if !val.is_empty() => {
                                let clean = record_injection_if_any(&SecretRedactor::redact(val), kv, extra_tags);
                                kv.insert("pdf_title".to_string(), json!(truncate_chars(&clean, 80)));
                            }
                            "Author" if !val.is_empty() => {
                                kv.insert("pdf_author".to_string(), json!(truncate_chars(val, 40)));
                            }
                            _ => {}
                        }
                    }
                }
            }
            if let Some(txt) = run_command_bounded("pdftotext", &["-f", "1", "-l", "3", p_str, "-"], tool_timeout_ms) {
                let compact = txt.split_whitespace().collect::<Vec<_>>().join(" ");
                if !compact.is_empty() {
                    let redacted = SecretRedactor::redact(&compact);
                    let safe = record_injection_if_any(&redacted, kv, extra_tags);
                    kv.insert("pdf_excerpt".to_string(), json!(truncate_chars(&safe, 400)));
                }
            }
        }
    }

    // 纯 Rust 回退估算页数
    if !kv.contains_key("page_count") && !head.is_empty() {
        let lossy = String::from_utf8_lossy(&head);
        let page_hits = lossy.matches("/Type /Page").count().saturating_sub(lossy.matches("/Type /Pages").count());
        if page_hits > 0 {
            kv.insert("page_count".to_string(), json!(page_hits));
        }
    }

    let title = kv.get("pdf_title").and_then(|v| v.as_str()).map(|s| s.to_string());
    let pages = kv.get("page_count").and_then(|v| v.as_u64());
    let purpose = match (title, pages) {
        (Some(t), Some(p)) => format!("PDF 文档「{}」({} 页, {})", truncate_chars(&t, 40), p, file_name),
        (Some(t), None) => format!("PDF 文档「{}」({})", truncate_chars(&t, 45), file_name),
        (None, Some(p)) => format!("PDF 文档 ({}, {} 页, {} bytes)", file_name, p, size_bytes.unwrap_or(0)),
        (None, None) => format!("PDF 文档 ({}, {} bytes)", file_name, size_bytes.unwrap_or(0)),
    };
    kv.insert("capabilities".to_string(), json!(vec!["PDF 文本/标题/页数语义检索".to_string()]));
    (purpose, file_name.to_string(), Some(format!("检索 PDF 元数据: {}", file_name)))
}

fn scan_zip_entry_names(path: &Path, max_scan_bytes: usize) -> Vec<String> {
    let bytes = read_head_bytes(path, max_scan_bytes).unwrap_or_default();
    let mut names = Vec::new();
    let mut i = 0usize;
    while i + 30 <= bytes.len() {
        if &bytes[i..i + 4] == b"PK\x03\x04" {
            let comp_size = u32::from_le_bytes([bytes[i + 18], bytes[i + 19], bytes[i + 20], bytes[i + 21]]) as usize;
            let name_len = u16::from_le_bytes([bytes[i + 26], bytes[i + 27]]) as usize;
            let extra_len = u16::from_le_bytes([bytes[i + 28], bytes[i + 29]]) as usize;
            let name_start = i + 30;
            let name_end = name_start.saturating_add(name_len);
            if name_end <= bytes.len() {
                let name = String::from_utf8_lossy(&bytes[name_start..name_end]).to_string();
                if !name.is_empty() && names.len() < 256 {
                    names.push(name);
                }
            }
            let next = name_end.saturating_add(extra_len).saturating_add(comp_size);
            if next <= i {
                break;
            }
            i = next;
        } else {
            i += 1;
        }
    }
    names
}

fn extract_xml_tag_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{}", tag);
    let close = format!("</{}>", tag);
    let s = xml.find(&open)?;
    let after = &xml[s + open.len()..];
    let gt = after.find('>')?;
    let body = &after[gt + 1..];
    let end = body.find(&close)?;
    let val = body[..end].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

/// Office 文档 (docx / pptx / xlsx) L1 元数据提取（ZIP 结构扫描 + 可选 `unzip` 核心 XML 属性）
pub fn extract_office_metadata(
    path: &Path,
    ext: &str,
    file_name: &str,
    size_bytes: Option<i64>,
    kv: &mut Map<String, Value>,
    extra_tags: &mut Vec<String>,
) -> (String, String, Option<String>) {
    kv.insert("modality".to_string(), json!("office"));
    kv.insert("office_format".to_string(), json!(ext));
    kv.insert("doc_category".to_string(), json!("office"));
    extra_tags.push("office".to_string());
    extra_tags.push(ext.to_string());

    let entries = scan_zip_entry_names(path, 2 * 1024 * 1024);
    let slide_count = entries
        .iter()
        .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
        .count();
    let sheet_count = entries
        .iter()
        .filter(|n| n.starts_with("xl/worksheets/sheet") && n.ends_with(".xml"))
        .count();
    let media_count = entries
        .iter()
        .filter(|n| {
            n.starts_with("word/media/") || n.starts_with("ppt/media/") || n.starts_with("xl/media/")
        })
        .count();

    if slide_count > 0 {
        kv.insert("slide_count".to_string(), json!(slide_count));
    }
    if sheet_count > 0 {
        kv.insert("sheet_count".to_string(), json!(sheet_count));
    }
    if media_count > 0 {
        kv.insert("embedded_media_count".to_string(), json!(media_count));
    }

    let (tools_enabled, tool_timeout_ms) = effective_multimodal_tool_policy();
    if tools_enabled {
        if let Some(p_str) = path.to_str() {
            if let Some(xml) = run_command_bounded("unzip", &["-p", p_str, "docProps/core.xml"], tool_timeout_ms) {
                if let Some(title) = extract_xml_tag_value(&xml, "dc:title") {
                    let safe = record_injection_if_any(&SecretRedactor::redact(&title), kv, extra_tags);
                    kv.insert("doc_title".to_string(), json!(truncate_chars(&safe, 80)));
                }
                if let Some(creator) = extract_xml_tag_value(&xml, "dc:creator") {
                    kv.insert("doc_author".to_string(), json!(truncate_chars(&creator, 40)));
                }
            }
        }
    }

    let purpose = match ext {
        "pptx" if slide_count > 0 => format!("PPTX 演示文稿 ({}, {} 页幻灯片)", file_name, slide_count),
        "xlsx" if sheet_count > 0 => format!("XLSX 工作簿 ({}, {} 个工作表)", file_name, sheet_count),
        _ => format!(
            "{} Office 文档 ({}, {} bytes)",
            ext.to_uppercase(),
            file_name,
            size_bytes.unwrap_or(0)
        ),
    };
    kv.insert("capabilities".to_string(), json!(vec![format!("{} 结构与标题检索", ext.to_uppercase())]));
    (purpose, file_name.to_string(), Some(format!("Office 文档检索: {}", file_name)))
}

/// 二进制结构化数据集 L1 元数据提取（SQLite / Parquet / DuckDB，零幻觉真 Schema）
pub fn extract_binary_dataset_metadata(
    path: &Path,
    ext: &str,
    file_name: &str,
    size_bytes: Option<i64>,
    kv: &mut Map<String, Value>,
    extra_tags: &mut Vec<String>,
) -> (String, String, Option<String>) {
    let fmt = if ext.is_empty() { "dataset" } else { ext };
    kv.insert("modality".to_string(), json!("dataset"));
    kv.insert("dataset_format".to_string(), json!(fmt));
    extra_tags.push("dataset".to_string());
    extra_tags.push(fmt.to_string());

    let mut table_names: Vec<String> = Vec::new();
    let mut columns: Vec<String> = Vec::new();
    let mut row_count_estimate: Option<i64> = None;

    if matches!(fmt, "sqlite" | "sqlite3" | "db") {
        let head = read_head_bytes(path, 32).unwrap_or_default();
        if head.starts_with(b"SQLite format 3\0") {
            let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
            if let Ok(conn) = rusqlite::Connection::open_with_flags(path, flags) {
                drop(conn.busy_timeout(std::time::Duration::from_millis(200)));
                if let Ok(mut stmt) = conn.prepare(
                    "SELECT name FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%' ORDER BY name LIMIT 20",
                ) {
                    if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
                        for t in rows.flatten() {
                            table_names.push(t);
                        }
                    }
                }
                let mut total_rows = 0i64;
                let small_enough = size_bytes.unwrap_or(0) <= 64 * 1024 * 1024;
                for t in table_names.iter().take(6) {
                    let escaped = t.replace('"', "\"\"");
                    if columns.len() < 24 {
                        let pragma = format!("PRAGMA table_info(\"{}\")", escaped);
                        if let Ok(mut ps) = conn.prepare(&pragma) {
                            if let Ok(cols) = ps.query_map([], |r| r.get::<_, String>(1)) {
                                for c in cols.flatten() {
                                    let qualified = format!("{}.{}", t, c);
                                    if !columns.contains(&qualified) && columns.len() < 24 {
                                        columns.push(qualified);
                                    }
                                }
                            }
                        }
                    }
                    if small_enough {
                        let cnt_sql = format!("SELECT COUNT(*) FROM \"{}\"", escaped);
                        if let Ok(cnt) = conn.query_row(&cnt_sql, [], |r| r.get::<_, i64>(0)) {
                            total_rows = total_rows.saturating_add(cnt);
                        }
                    }
                }
                if !table_names.is_empty() && small_enough {
                    row_count_estimate = Some(total_rows);
                }
            }
        }
    } else if fmt == "parquet" {
        let sz_u64 = size_bytes.unwrap_or(0).max(0) as u64;
        if let Some((nr, cols, ctypes)) = parse_parquet_footer_l1a(path, sz_u64) {
            row_count_estimate = nr;
            columns = cols;
            if !ctypes.is_empty() {
                kv.insert("column_types".to_string(), json!(ctypes));
            }
        }
        if columns.is_empty() || row_count_estimate.is_none() {
            let head = read_head_bytes(path, 8).unwrap_or_default();
            if head.starts_with(b"PAR1") {
                if let Ok(conn) = duckdb::Connection::open_in_memory() {
                    let p_str = path.to_string_lossy().to_string();
                    if columns.is_empty() {
                        if let Ok(mut stmt) = conn.prepare("DESCRIBE SELECT * FROM read_parquet(?)") {
                            if let Ok(rows) = stmt.query_map([&p_str], |r| r.get::<_, String>(0)) {
                                for c in rows.flatten() {
                                    if columns.len() < 24 && !columns.contains(&c) {
                                        columns.push(c);
                                    }
                                }
                            }
                        }
                    }
                    if row_count_estimate.is_none() && size_bytes.unwrap_or(0) <= 64 * 1024 * 1024 {
                        if let Ok(cnt) = conn.query_row(
                            "SELECT COUNT(*) FROM read_parquet(?)",
                            [&p_str],
                            |r| r.get::<_, i64>(0),
                        ) {
                            row_count_estimate = Some(cnt);
                        }
                    }
                }
            }
        }
    } else if fmt == "duckdb" {
        if let Ok(cfg) = duckdb::Config::default().access_mode(duckdb::AccessMode::ReadOnly) {
            if let Ok(conn) = duckdb::Connection::open_with_flags(path, cfg) {
                if let Ok(mut stmt) = conn.prepare(
                    "SELECT table_name FROM information_schema.tables WHERE table_schema = 'main' ORDER BY table_name LIMIT 20",
                ) {
                    if let Ok(rows) = stmt.query_map([], |r| r.get::<_, String>(0)) {
                        for t in rows.flatten() {
                            table_names.push(t);
                        }
                    }
                }
                if let Ok(mut stmt) = conn.prepare(
                    "SELECT table_name, column_name FROM information_schema.columns WHERE table_schema = 'main' ORDER BY table_name, ordinal_position LIMIT 24",
                ) {
                    if let Ok(rows) = stmt.query_map([], |r| {
                        let t: String = r.get(0)?;
                        let c: String = r.get(1)?;
                        Ok(format!("{}.{}", t, c))
                    }) {
                        for qc in rows.flatten() {
                            columns.push(qc);
                        }
                    }
                }
            }
        }
    }

    if !table_names.is_empty() {
        kv.insert("table_names".to_string(), json!(table_names));
    }
    kv.insert("columns".to_string(), json!(columns));
    if let Some(rc) = row_count_estimate {
        kv.insert("row_count_estimate".to_string(), json!(rc));
    }
    kv.insert("capabilities".to_string(), json!(vec![format!("{} 库表 Schema 与结构检索", fmt)]));

    let purpose = if !table_names.is_empty() {
        format!(
            "{} 数据库，含表 [{}]",
            fmt.to_uppercase(),
            table_names.iter().take(4).cloned().collect::<Vec<_>>().join(", ")
        )
    } else if !columns.is_empty() {
        format!(
            "{} 数据集，字段 [{}]",
            fmt.to_uppercase(),
            columns.iter().take(4).cloned().collect::<Vec<_>>().join(", ")
        )
    } else {
        format!("{} 数据集 ({}, {} bytes)", fmt.to_uppercase(), file_name, size_bytes.unwrap_or(0))
    };

    (purpose, file_name.to_string(), Some(format!("Schema/SQL 查询: {}", file_name)))
}

/// 格式化 `DetailLevel::Standard` 下的多模态紧凑规格摘要
pub fn format_modality_spec(kv: &Map<String, Value>) -> String {
    let Some(mod_str) = kv.get("modality").and_then(|v| v.as_str()) else {
        return String::new();
    };
    let mut parts = vec![format!("modality={}", mod_str)];
    if let Some(dim) = kv.get("dimensions").and_then(|v| v.as_str()) {
        parts.push(dim.to_string());
    }
    if let Some(dur) = kv.get("duration_secs").and_then(|v| v.as_f64()) {
        parts.push(format!("{:.1}s", dur));
    }
    if let Some(pg) = kv.get("page_count").and_then(|v| v.as_u64()) {
        parts.push(format!("{}p", pg));
    }
    if let Some(sl) = kv.get("slide_count").and_then(|v| v.as_u64()) {
        parts.push(format!("{}slides", sl));
    }
    if let Some(sh) = kv.get("sheet_count").and_then(|v| v.as_u64()) {
        parts.push(format!("{}sheets", sh));
    }
    if let Some(tbls) = kv.get("table_names").and_then(|v| v.as_array()) {
        let names: Vec<_> = tbls.iter().filter_map(|x| x.as_str()).take(3).collect();
        if !names.is_empty() {
            parts.push(format!("tables=[{}]", names.join(",")));
        }
    }
    if parts.len() == 1 && matches!(mod_str, "code" | "document" | "config" | "other") {
        return String::new();
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_base64_and_infer_modality() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
        assert_eq!(infer_modality("media", "png"), "image");
        assert_eq!(infer_modality("media", "WAV"), "audio");
        assert_eq!(infer_modality("media", "mov"), "video");
        assert_eq!(infer_modality("document", "pdf"), "pdf");
        assert_eq!(infer_modality("document", "pptx"), "office");
        assert_eq!(infer_modality("dataset", "sqlite"), "dataset");
        assert_eq!(infer_modality("script", "rs"), "code");
    }

    #[test]
    fn test_extract_png_svg_and_wav_headers() {
        let dir = tempdir().expect("tempdir");

        // 1. 构造最小合法 PNG 头 (1280x720 RGBA)
        let png_path = dir.path().join("arch.png");
        let mut png_bytes = Vec::new();
        png_bytes.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        png_bytes.extend_from_slice(&13u32.to_be_bytes());
        png_bytes.extend_from_slice(b"IHDR");
        png_bytes.extend_from_slice(&1280u32.to_be_bytes());
        png_bytes.extend_from_slice(&720u32.to_be_bytes());
        png_bytes.push(8); // bit depth
        png_bytes.push(6); // RGBA
        png_bytes.extend_from_slice(&[0u8; 7]);
        std::fs::write(&png_path, &png_bytes).expect("write png");

        let mut kv = Map::new();
        let mut tags = Vec::new();
        let (purpose, _, _) = extract_image_metadata(&png_path, "", "png", "arch.png", Some(png_bytes.len() as i64), &mut kv, &mut tags);
        assert!(purpose.contains("1280x720"));
        assert_eq!(kv.get("modality").and_then(|v| v.as_str()), Some("image"));
        assert_eq!(kv.get("dimensions").and_then(|v| v.as_str()), Some("1280x720"));
        assert_eq!(kv.get("color_type").and_then(|v| v.as_str()), Some("rgba"));

        // 2. 构造含 <text> 的 SVG 并验证文本抽取与 Prompt 注入防线
        let svg_path = dir.path().join("flow.svg");
        let svg_content = r#"<svg viewBox="0 0 800 600" width="800"><title>Hybrid Search Flow</title><text x="10" y="20">SQLite FTS5 + Vector RRF</text></svg>"#;
        std::fs::write(&svg_path, svg_content).expect("write svg");
        let mut svg_kv = Map::new();
        let mut svg_tags = Vec::new();
        let (svg_purpose, _, _) = extract_image_metadata(&svg_path, svg_content, "svg", "flow.svg", Some(svg_content.len() as i64), &mut svg_kv, &mut svg_tags);
        assert!(svg_purpose.contains("Hybrid Search Flow"));
        assert_eq!(svg_kv.get("viewbox").and_then(|v| v.as_str()), Some("0 0 800 600"));
        let ocr = svg_kv.get("ocr_keywords").and_then(|v| v.as_array()).expect("ocr");
        assert!(ocr.iter().any(|x| x.as_str() == Some("SQLite FTS5 + Vector RRF")));

        // 3. 构造最小合法 WAV 头 (2ch, 44100Hz, 1.0s PCM)
        let wav_path = dir.path().join("voice.wav");
        let mut wav = Vec::new();
        let data_len = 176_400u32; // 44100 * 2ch * 2bytes
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
        wav.extend_from_slice(&2u16.to_le_bytes()); // 2ch
        wav.extend_from_slice(&44100u32.to_le_bytes());
        wav.extend_from_slice(&176400u32.to_le_bytes());
        wav.extend_from_slice(&4u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        std::fs::write(&wav_path, &wav).expect("write wav");

        let mut wav_kv = Map::new();
        let mut wav_tags = Vec::new();
        let (wav_purpose, _, _) = extract_media_metadata(&wav_path, "", "wav", "voice.wav", Some(wav.len() as i64), &mut wav_kv, &mut wav_tags);
        assert!(wav_purpose.contains("1.0s"));
        assert_eq!(wav_kv.get("modality").and_then(|v| v.as_str()), Some("audio"));
        assert_eq!(wav_kv.get("sample_rate").and_then(|v| v.as_u64()), Some(44100));
        assert_eq!(wav_kv.get("audio_channels").and_then(|v| v.as_u64()), Some(2));
    }

    #[test]
    fn test_extract_binary_sqlite_and_office_zip() {
        let dir = tempdir().expect("tempdir");

        // 1. 构造真实 SQLite 数据库（orders 表 + 2 行）
        let db_path = dir.path().join("shop.db");
        {
            let conn = rusqlite::Connection::open(&db_path).expect("open sqlite");
            conn.execute_batch(
                "CREATE TABLE orders (id INTEGER PRIMARY KEY, amount REAL, region TEXT);
                 INSERT INTO orders (amount, region) VALUES (99.5, 'shanghai'), (42.0, 'beijing');",
            )
            .expect("seed sqlite");
        }
        let size = std::fs::metadata(&db_path).expect("meta").len() as i64;
        let mut db_kv = Map::new();
        let mut db_tags = Vec::new();
        let (db_purpose, _, _) = extract_binary_dataset_metadata(&db_path, "db", "shop.db", Some(size), &mut db_kv, &mut db_tags);
        assert!(db_purpose.contains("orders"));
        assert_eq!(db_kv.get("modality").and_then(|v| v.as_str()), Some("dataset"));
        assert_eq!(db_kv.get("row_count_estimate").and_then(|v| v.as_i64()), Some(2));
        let cols = db_kv.get("columns").and_then(|v| v.as_array()).expect("cols");
        assert!(cols.iter().any(|c| c.as_str() == Some("orders.region")));

        // 2. 构造最小 ZIP Local File Header 模拟 .pptx（含 2 张 slide 与 1 个 media）
        let pptx_path = dir.path().join("deck.pptx");
        {
            let mut f = File::create(&pptx_path).expect("create pptx");
            for name in ["ppt/slides/slide1.xml", "ppt/slides/slide2.xml", "ppt/media/image1.png"] {
                let nb = name.as_bytes();
                f.write_all(b"PK\x03\x04").expect("sig");
                f.write_all(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]).expect("hdr");
                f.write_all(&0u32.to_le_bytes()).expect("comp");
                f.write_all(&0u32.to_le_bytes()).expect("uncomp");
                f.write_all(&(nb.len() as u16).to_le_bytes()).expect("nlen");
                f.write_all(&0u16.to_le_bytes()).expect("elen");
                f.write_all(nb).expect("name");
            }
        }
        let mut ppt_kv = Map::new();
        let mut ppt_tags = Vec::new();
        let (ppt_purpose, _, _) = extract_office_metadata(&pptx_path, "pptx", "deck.pptx", Some(256), &mut ppt_kv, &mut ppt_tags);
        assert!(ppt_purpose.contains("2 页幻灯片"));
        assert_eq!(ppt_kv.get("modality").and_then(|v| v.as_str()), Some("office"));
        assert_eq!(ppt_kv.get("slide_count").and_then(|v| v.as_u64()), Some(2));
        assert_eq!(ppt_kv.get("embedded_media_count").and_then(|v| v.as_u64()), Some(1));
    }

    #[test]
    fn test_extract_parquet_and_pdf_fallback() {
        let dir = tempdir().expect("tempdir");

        // 1. 使用 DuckDB 导出真实 Parquet 文件并校验列与行数提取
        let pq_path = dir.path().join("sales.parquet");
        {
            let conn = duckdb::Connection::open_in_memory().expect("duckdb mem");
            let sql = format!(
                "COPY (SELECT 1 AS order_id, 'hz' AS city UNION ALL SELECT 2, 'sh') TO '{}' (FORMAT PARQUET)",
                pq_path.to_string_lossy().replace('\'', "''")
            );
            conn.execute_batch(&sql).expect("export parquet");
        }
        let pq_size = std::fs::metadata(&pq_path).expect("meta").len() as i64;
        let mut pq_kv = Map::new();
        let mut pq_tags = Vec::new();
        let (_, _, _) = extract_binary_dataset_metadata(&pq_path, "parquet", "sales.parquet", Some(pq_size), &mut pq_kv, &mut pq_tags);
        assert_eq!(pq_kv.get("modality").and_then(|v| v.as_str()), Some("dataset"));
        assert_eq!(pq_kv.get("row_count_estimate").and_then(|v| v.as_i64()), Some(2));
        let cols = pq_kv.get("columns").and_then(|v| v.as_array()).expect("cols");
        assert!(cols.iter().any(|c| c.as_str() == Some("order_id")));
        assert!(cols.iter().any(|c| c.as_str() == Some("city")));

        // 2. 合成最小 PDF 并验证纯 Rust 魔数与回退页数识别
        let pdf_path = dir.path().join("spec.pdf");
        let pdf_bytes = b"%PDF-1.7\n1 0 obj\n<< /Type /Pages /Count 2 >>\nendobj\n2 0 obj\n<< /Type /Page >>\nendobj\n3 0 obj\n<< /Type /Page >>\nendobj\n";
        std::fs::write(&pdf_path, pdf_bytes).expect("write pdf");
        let mut pdf_kv = Map::new();
        let mut pdf_tags = Vec::new();
        let (_, _, _) = extract_pdf_metadata(&pdf_path, "spec.pdf", Some(pdf_bytes.len() as i64), &mut pdf_kv, &mut pdf_tags);
        assert_eq!(pdf_kv.get("modality").and_then(|v| v.as_str()), Some("pdf"));
        assert_eq!(pdf_kv.get("pdf_version").and_then(|v| v.as_str()), Some("1.7"));
        assert!(pdf_kv.get("page_count").and_then(|v| v.as_u64()).unwrap_or(0) >= 1);
    }
}
