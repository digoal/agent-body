use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use crate::scanner::redactor::{is_sensitive_file, SecretRedactor};

/// ISSUE #56: 为数据库侧模糊/BM25 全文检索安全提取并脱敏文本资产正文 (`content_text`)。
/// 严格执行白黑名单与脱敏红线：
/// - `shallow` 黑名单及 `is_sensitive_file` 文件严禁读取任何字节，直接返回 `None`；
/// - 跳过二进制/图片/音视频/压缩包/数据库等非纯文本文件与 >20MB 巨型文件；
/// - 超出 `max_bytes` 时自动采集首部 3/4 + 尾部 1/4，并经 `SecretRedactor::redact` 脱敏。
pub fn extract_indexable_content_text(
    path: &Path,
    scan_tier: &str,
    max_bytes: usize,
) -> Option<String> {
    if scan_tier == "shallow" || is_sensitive_file(path) || !path.is_file() || max_bytes == 0 {
        return None;
    }

    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();
    if matches!(
        ext.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "ico"
            | "bmp"
            | "tiff"
            | "pdf"
            | "doc"
            | "docx"
            | "xls"
            | "xlsx"
            | "ppt"
            | "pptx"
            | "zip"
            | "tar"
            | "gz"
            | "tgz"
            | "bz2"
            | "xz"
            | "7z"
            | "zst"
            | "rar"
            | "db"
            | "sqlite"
            | "sqlite3"
            | "duckdb"
            | "parquet"
            | "so"
            | "dylib"
            | "dll"
            | "a"
            | "o"
            | "rlib"
            | "rmeta"
            | "wasm"
            | "bin"
            | "exe"
            | "mp3"
            | "mp4"
            | "wav"
            | "flac"
            | "ogg"
            | "avi"
            | "mov"
            | "mkv"
            | "pyc"
            | "pyo"
            | "class"
            | "jar"
            | "woff"
            | "woff2"
            | "ttf"
            | "otf"
            | "eot"
            | "pdb"
            | "pack"
            | "idx"
    ) {
        return None;
    }

    let meta = path.metadata().ok()?;
    let file_len = meta.len();
    if file_len == 0 || file_len > 20 * 1024 * 1024 {
        return None;
    }

    let mut file = File::open(path).ok()?;
    let raw_text = if (file_len as usize) <= max_bytes {
        let mut buf = Vec::with_capacity(file_len as usize);
        file.read_to_end(&mut buf).ok()?;
        let probe_len = buf.len().min(4096);
        if buf[..probe_len].contains(&0) {
            return None;
        }
        String::from_utf8_lossy(&buf).to_string()
    } else {
        let head_cap = (max_bytes * 3) / 4;
        let tail_cap = max_bytes.saturating_sub(head_cap);
        let mut head_buf = vec![0u8; head_cap];
        let hn = file.read(&mut head_buf).ok()?;
        head_buf.truncate(hn);
        let probe_len = head_buf.len().min(4096);
        if head_buf[..probe_len].contains(&0) {
            return None;
        }
        let mut combined = String::from_utf8_lossy(&head_buf).to_string();
        if tail_cap > 0 && file_len > head_cap as u64 {
            let tail_start = file_len.saturating_sub(tail_cap as u64);
            if file.seek(SeekFrom::Start(tail_start)).is_ok() {
                let mut tail_buf = vec![0u8; tail_cap];
                if let Ok(tn) = file.read(&mut tail_buf) {
                    tail_buf.truncate(tn);
                    if !tail_buf.contains(&0) {
                        combined.push_str("\n...[truncated]...\n");
                        combined.push_str(&String::from_utf8_lossy(&tail_buf));
                    }
                }
            }
        }
        combined
    };

    if raw_text.trim().is_empty() {
        return None;
    }
    Some(SecretRedactor::redact(&raw_text))
}
