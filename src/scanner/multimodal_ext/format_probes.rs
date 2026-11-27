use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use serde_json::{json, Map, Value};
use crate::scanner::metadata::{read_head_utf8, truncate_chars};
use super::{sanitize_line, scan_untrusted_text, MAX_FACET_ITEMS};

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
