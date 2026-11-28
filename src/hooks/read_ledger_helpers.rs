//! `Session Read Ledger` 确定性纯函数辅助集
//!
//! 负责 Transcript CHECKPOINT 增量扫描、路径词法归一化、区间归并、重叠裁剪计算、
//! 文件版本号读取、未读关键符号导航拼接与旧账本目录清理。

use std::env;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::hooks::outline::extract_file_outline;

const TRANSCRIPT_SCAN_TAIL_BYTES: u64 = 4 * 1024 * 1024;
const MAX_LINE_COUNT_SCAN_BYTES: usize = 2 * 1024 * 1024;
const OVERLAP_CONTEXT_LINES: usize = 5;
const MIN_OVERLAP_LINES_TO_TRIM: usize = 40;
const MIN_UNCOVERED_LINES_TO_TRIM: usize = 10;
const MIN_SAVED_LINES_TO_TRIM: usize = 30;

const BINARY_OR_MEDIA_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "pdf", "mp3", "mp4", "wav", "ogg",
    "webm", "mov", "avi", "zip", "tar", "gz", "xz", "7z", "db", "sqlite", "duckdb",
];

pub(crate) fn scan_transcript_for_checkpoint(
    tp: &Path,
    old_offset: u64,
    len: u64,
) -> (bool, u64) {
    let Ok(mut f) = File::open(tp) else {
        return (false, old_offset);
    };
    let delta = len.saturating_sub(old_offset);
    let (scan_start, skip_first_partial) = if delta > TRANSCRIPT_SCAN_TAIL_BYTES {
        (len - TRANSCRIPT_SCAN_TAIL_BYTES, true)
    } else {
        (old_offset, false)
    };
    if f.seek(SeekFrom::Start(scan_start)).is_err() {
        return (false, old_offset);
    }
    let mut reader = BufReader::new(f);
    let mut found = false;
    let mut consumed = scan_start;
    let mut first = true;
    let mut buf = String::new();
    loop {
        buf.clear();
        let Ok(n) = reader.read_line(&mut buf) else {
            break;
        };
        if n == 0 {
            break;
        }
        consumed += n as u64;
        if first && skip_first_partial {
            first = false;
            continue;
        }
        first = false;
        if !buf.contains("\"CHECKPOINT\"") {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<Value>(buf.trim()) {
            if v.get("type").and_then(|t| t.as_str()) == Some("CHECKPOINT") {
                found = true;
            }
        }
    }
    (found, consumed.min(len))
}

pub(crate) fn prune_old_ledgers(dir: &Path, keep_max: usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(SystemTime, PathBuf)> = entries
        .filter_map(|res| res.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("jsonl"))
        .map(|p| {
            let mtime = p
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(UNIX_EPOCH);
            (mtime, p)
        })
        .collect();
    if files.len() < keep_max {
        return;
    }
    files.sort_by_key(|(t, _)| *t);
    let remove_count = files.len().saturating_sub(keep_max.saturating_sub(1));
    for (_, path) in files.into_iter().take(remove_count) {
        if let Err(e) = fs::remove_file(&path) {
            eprintln!("[ReadLedger][WARN] 淘汰旧账本失败 ({:?}): {}", path, e);
        }
    }
}

pub(crate) fn sanitize_conv_id(conv_id: &str) -> String {
    let safe: String = conv_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    if safe.is_empty() {
        "default".to_string()
    } else {
        safe
    }
}

pub(crate) fn is_read_ledger_env_disabled() -> bool {
    for key in [
        "AGENT_BODY_READ_LEDGER",
        "AGENT_BODY_TOKEN_GUARD",
        "AGY_TOKEN_OPTIMIZER",
    ] {
        if let Ok(val) = env::var(key) {
            let norm = val.trim().to_ascii_lowercase();
            if matches!(norm.as_str(), "0" | "off" | "false" | "no") {
                return true;
            }
        }
    }
    false
}

pub(crate) fn has_byte_offset(args: &Value) -> bool {
    args.get("ByteOffset")
        .or_else(|| args.get("byte_offset"))
        .and_then(|v| v.as_i64())
        .map(|n| n > 0)
        .unwrap_or(false)
}

pub(crate) fn extract_read_path(args: &Value) -> Option<&str> {
    args.get("Path")
        .or_else(|| args.get("AbsolutePath"))
        .or_else(|| args.get("path"))
        .or_else(|| args.get("file_path"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

pub(crate) fn normalize_path(raw: &str, cwd: Option<&str>) -> String {
    let expanded = if let Some(rest) = raw.strip_prefix("~/") {
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .unwrap_or_else(|_| "/tmp".to_string());
        PathBuf::from(home).join(rest)
    } else {
        let p = Path::new(raw);
        if p.is_absolute() {
            p.to_path_buf()
        } else if let Some(base) = cwd.map(str::trim).filter(|s| !s.is_empty()) {
            Path::new(base).join(p)
        } else {
            p.to_path_buf()
        }
    };

    let mut clean = PathBuf::new();
    for comp in expanded.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other.as_os_str()),
        }
    }
    clean.to_string_lossy().to_string()
}

pub(crate) fn is_binary_ext(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| BINARY_OR_MEDIA_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub(crate) fn meta_version(meta: &fs::Metadata) -> (u64, u32, u64) {
    let size = meta.len();
    match meta.modified().and_then(|t| {
        t.duration_since(UNIX_EPOCH)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
    }) {
        Ok(dur) => (dur.as_secs(), dur.subsec_nanos(), size),
        Err(_) => (0, 0, size),
    }
}

fn count_file_lines(path: &Path) -> Option<usize> {
    let mut f = File::open(path).ok()?;
    let mut buf = Vec::with_capacity(64 * 1024);
    (&mut f)
        .take(MAX_LINE_COUNT_SCAN_BYTES as u64)
        .read_to_end(&mut buf)
        .ok()?;
    if buf.is_empty() {
        return Some(1);
    }
    let newlines = buf.iter().filter(|&&b| b == b'\n').count();
    if buf.last() == Some(&b'\n') {
        Some(newlines.max(1))
    } else {
        Some(newlines + 1)
    }
}

fn extract_int_arg(v: &Value, keys: &[&str]) -> Option<usize> {
    for k in keys {
        if let Some(n) = v.get(*k).and_then(|x| x.as_i64()) {
            if n >= 1 {
                return Some(n as usize);
            }
        }
    }
    None
}

pub(crate) fn resolve_effective_range(
    path: &Path,
    args: &Value,
    upstream_overwrite: Option<&Value>,
) -> Option<(usize, usize, usize)> {
    let total_lines = count_file_lines(path)?;
    if let Some(ow) = upstream_overwrite {
        if let (Some(s), Some(e)) = (
            extract_int_arg(ow, &["StartLine", "start_line"]),
            extract_int_arg(ow, &["EndLine", "end_line"]),
        ) {
            return Some((s, e.min(total_lines).max(s), total_lines));
        }
    }
    let start = extract_int_arg(args, &["StartLine", "start_line", "offset"])
        .unwrap_or(1)
        .min(total_lines);
    let end = if let Some(e) = extract_int_arg(args, &["EndLine", "end_line"]) {
        e.min(total_lines).max(start)
    } else if let Some(limit) = extract_int_arg(args, &["limit"]) {
        start.saturating_add(limit).saturating_sub(1).min(total_lines)
    } else {
        total_lines.max(start)
    };
    Some((start, end, total_lines))
}

pub(crate) fn merge_intervals(raw: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if raw.is_empty() {
        return Vec::new();
    }
    let mut sorted = raw.to_vec();
    sorted.sort_unstable_by_key(|&(s, e)| (s, e));
    let mut merged: Vec<(usize, usize)> = Vec::with_capacity(sorted.len());
    for (s, e) in sorted {
        if let Some(last) = merged.last_mut() {
            if s <= last.1.saturating_add(1) {
                last.1 = last.1.max(e);
                continue;
            }
        }
        merged.push((s, e));
    }
    merged
}

pub(crate) fn is_fully_covered(
    covered: &[(usize, usize)],
    req_start: usize,
    req_end: usize,
) -> bool {
    covered
        .iter()
        .any(|&(cs, ce)| cs <= req_start && ce >= req_end)
}

pub(crate) fn compute_overlap_trim(
    covered: &[(usize, usize)],
    req_start: usize,
    req_end: usize,
) -> Option<(usize, usize, usize)> {
    if covered.is_empty() || req_end <= req_start {
        return None;
    }
    let mut uncovered: Vec<(usize, usize)> = Vec::new();
    let mut cur = req_start;
    for &(cs, ce) in covered {
        if ce < cur {
            continue;
        }
        if cs > req_end {
            break;
        }
        if cs > cur {
            uncovered.push((cur, cs - 1));
        }
        if ce >= req_end {
            cur = req_end + 1;
            break;
        }
        cur = ce + 1;
    }
    if cur <= req_end {
        uncovered.push((cur, req_end));
    }
    if uncovered.len() != 1 {
        return None;
    }
    let (us, ue) = uncovered[0];
    let req_len = req_end - req_start + 1;
    let unc_len = ue - us + 1;
    let overlap_lines = req_len.saturating_sub(unc_len);
    if overlap_lines < MIN_OVERLAP_LINES_TO_TRIM || unc_len < MIN_UNCOVERED_LINES_TO_TRIM {
        return None;
    }
    let trim_start = us.saturating_sub(OVERLAP_CONTEXT_LINES).max(req_start);
    let trim_end = ue.saturating_add(OVERLAP_CONTEXT_LINES).min(req_end);
    let trimmed_len = trim_end.saturating_sub(trim_start) + 1;
    let saved_lines = req_len.saturating_sub(trimmed_len);
    if saved_lines < MIN_SAVED_LINES_TO_TRIM {
        return None;
    }
    Some((trim_start, trim_end, saved_lines))
}

pub(crate) fn format_intervals(covered: &[(usize, usize)]) -> String {
    covered
        .iter()
        .map(|(s, e)| format!("L{}~{}", s, e))
        .collect::<Vec<_>>()
        .join(", ")
}

pub(crate) fn base_name(path: &str) -> &str {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

pub(crate) fn build_unread_symbol_nav(
    path: &Path,
    covered: &[(usize, usize)],
    max_items: usize,
    max_bytes: usize,
) -> Option<String> {
    let outline = extract_file_outline(path, 120).ok()?;
    let unread: Vec<String> = outline
        .items
        .into_iter()
        .filter(|it| !covered.iter().any(|&(cs, ce)| it.line >= cs && it.line <= ce))
        .take(max_items)
        .map(|it| format!("- L{} {}", it.line, it.label))
        .collect();
    if unread.is_empty() {
        return None;
    }
    let mut out = String::from("未读关键符号导航（可直接按 StartLine/EndLine 跳转）：");
    for line in unread {
        if out.len() + line.len() + 1 > max_bytes {
            break;
        }
        out.push('\n');
        out.push_str(&line);
    }
    Some(out)
}
