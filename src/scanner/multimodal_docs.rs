use std::path::Path;
use serde_json::{json, Map, Value};
use crate::scanner::metadata::{truncate_chars, SecretRedactor};
use crate::scanner::multimodal::{read_head_bytes, record_injection_if_any};
use crate::scanner::multimodal_ext::{
    effective_multimodal_tool_policy, parse_parquet_footer_l1a, run_command_bounded,
};


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

