//! 确定性命令输出流式蒸馏引擎 (`OutputDistiller`)
//!
//! 为 `agent-body distill` 及 `TokenGuardEngine` 复合命令包装提供零 LLM 调用、
//! 100% 退出码保真与无损落盘可追溯的输出压缩能力。

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

const DEFAULT_MAX_LOG_FILES: usize = 200;

/// 命令输出蒸馏分类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistillKind {
    Auto,
    BuildTest,
    GitDiff,
    SearchList,
    HttpFetch,
    Generic,
}

impl DistillKind {
    /// 从用户输入字符串宽松解析蒸馏分类
    pub fn from_str_loose(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "build" | "test" | "build_test" | "build-test" => Self::BuildTest,
            "git" | "diff" | "git_diff" | "git-diff" => Self::GitDiff,
            "search" | "list" | "search_list" | "search-list" => Self::SearchList,
            "http" | "fetch" | "http_fetch" | "http-fetch" => Self::HttpFetch,
            "generic" => Self::Generic,
            _ => Self::Auto,
        }
    }

    /// 返回规范化分类标识字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::BuildTest => "build_test",
            Self::GitDiff => "git_diff",
            Self::SearchList => "search_list",
            Self::HttpFetch => "http_fetch",
            Self::Generic => "generic",
        }
    }

    /// 根据命令行内容自动推断输出类型
    pub fn detect_from_command(cmd: &str) -> Self {
        let lower = cmd.to_ascii_lowercase();
        if contains_any(
            &lower,
            &[
                "cargo test",
                "cargo nextest",
                "cargo build",
                "cargo check",
                "cargo clippy",
                "pytest",
                "go test",
                "npm test",
                "pnpm test",
                "yarn test",
                "make ",
            ],
        ) || lower.trim() == "make"
        {
            return Self::BuildTest;
        }
        if contains_any(&lower, &["git diff", "git show", "git log", "git status"]) {
            return Self::GitDiff;
        }
        if contains_any(
            &lower,
            &[
                "rg ", "grep ", "egrep ", "fgrep ", "find ", "fd ", "ls ",
            ],
        ) || matches!(lower.trim(), "rg" | "find" | "fd" | "ls")
        {
            return Self::SearchList;
        }
        if contains_any(&lower, &["curl ", "wget "]) {
            return Self::HttpFetch;
        }
        Self::Generic
    }
}

fn contains_any(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.contains(n))
}

/// 蒸馏处理结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DistillOutput {
    pub text: String,
    pub distilled: bool,
    pub raw_bytes: usize,
    pub raw_lines: usize,
    pub out_bytes: usize,
    pub out_lines: usize,
    pub kind: DistillKind,
}

/// 清洗 ANSI 转义序列与 `\r` 进度条覆盖行
pub fn strip_ansi_and_progress(raw: &str) -> String {
    let mut no_ansi = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] == 0x1B {
            if i + 1 < bytes.len() && bytes[i + 1] == b'[' {
                i += 2;
                while i < bytes.len() {
                    let b = bytes[i];
                    i += 1;
                    if (0x40..=0x7E).contains(&b) {
                        break;
                    }
                }
                continue;
            }
            if i + 1 < bytes.len() && bytes[i + 1] == b']' {
                i += 2;
                while i < bytes.len() {
                    if bytes[i] == 0x07 {
                        i += 1;
                        break;
                    }
                    if bytes[i] == 0x1B && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
                continue;
            }
            i += 1;
            continue;
        }
        // 安全消费合法 UTF-8 字符
        if let Some(ch) = raw[i..].chars().next() {
            no_ansi.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }

    let had_trailing_nl = no_ansi.ends_with('\n');
    let mut out_lines: Vec<&str> = Vec::new();
    for line in no_ansi.split('\n') {
        if line.contains('\r') {
            let last_non_empty = line
                .split('\r')
                .rev()
                .find(|seg| !seg.trim().is_empty())
                .unwrap_or("");
            out_lines.push(last_non_empty);
        } else {
            out_lines.push(line);
        }
    }

    // `split('\n')` 在末尾为 `\n` 时会多一个空尾元素，还原语义避免虚增空行
    if had_trailing_nl && out_lines.last() == Some(&"") {
        out_lines.pop();
    }

    let mut joined = out_lines.join("\n");
    if had_trailing_nl && !joined.is_empty() {
        joined.push('\n');
    }
    joined
}

/// 纯函数确定性命令输出蒸馏
pub fn distill_output(
    raw: &str,
    kind: DistillKind,
    exit_code: i32,
    max_lines: usize,
    max_bytes: usize,
) -> DistillOutput {
    let max_lines = max_lines.max(12);
    let max_bytes = max_bytes.max(512);
    let resolved_kind = if kind == DistillKind::Auto {
        DistillKind::Generic
    } else {
        kind
    };

    let cleaned = strip_ansi_and_progress(raw);
    let lines: Vec<&str> = cleaned.lines().collect();
    let raw_lines = lines.len();
    let raw_bytes = raw.len();

    if raw_lines <= max_lines && cleaned.len() <= max_bytes {
        return DistillOutput {
            out_bytes: cleaned.len(),
            out_lines: raw_lines,
            text: cleaned,
            distilled: false,
            raw_bytes,
            raw_lines,
            kind: resolved_kind,
        };
    }

    let distilled_lines = match resolved_kind {
        DistillKind::BuildTest => distill_build_test(&lines, exit_code, max_lines),
        DistillKind::GitDiff => distill_git_diff(&lines, max_lines),
        DistillKind::SearchList => distill_search_list(&lines, max_lines),
        DistillKind::HttpFetch | DistillKind::Generic | DistillKind::Auto => {
            distill_head_tail(&lines, max_lines)
        }
    };

    let mut final_text = cap_bytes_utf8(&distilled_lines.join("\n"), max_bytes);
    if !final_text.ends_with('\n') && !final_text.is_empty() {
        final_text.push('\n');
    }

    let out_lines = final_text.lines().count();
    let out_bytes = final_text.len();

    DistillOutput {
        text: final_text,
        distilled: true,
        raw_bytes,
        raw_lines,
        out_bytes,
        out_lines,
        kind: resolved_kind,
    }
}

fn is_build_progress_line(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("Compiling ")
        || t.starts_with("Downloading ")
        || t.starts_with("Downloaded ")
        || t.starts_with("Fetching ")
        || t.starts_with("Updating ")
        || t.starts_with("Checking ")
        || t.starts_with("Blocking waiting ")
        || t.starts_with("Locking ")
        || t.starts_with("Fresh ")
}

fn is_passing_test_line(line: &str) -> bool {
    let t = line.trim();
    (t.starts_with("test ") && t.ends_with("... ok"))
        || (t.starts_with("--- PASS:") || t.starts_with("PASS "))
}

fn is_build_summary_line(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("test result:")
        || t.starts_with("Finished ")
        || t.starts_with("Running ")
        || t.starts_with("running ")
        || t.starts_with("Doc-tests ")
        || t.starts_with("✅")
        || t.starts_with("❌")
        || t.contains("passed") && t.contains("failed")
        || t.starts_with("warning:")
}

fn is_failure_anchor(line: &str) -> bool {
    let t = line.trim();
    t.contains("FAILED")
        || t.contains("panicked at")
        || t.starts_with("error[")
        || t.starts_with("error:")
        || t.contains("assertion")
        || t == "failures:"
        || t.starts_with("---- ")
        || t.contains("Error:")
        || t.contains("Traceback (most recent call last)")
        || t.contains("exit status")
}

fn distill_build_test(lines: &[&str], exit_code: i32, max_lines: usize) -> Vec<String> {
    let mut compiled_folded = 0usize;
    let mut passed_folded = 0usize;

    if exit_code != 0 {
        let mut keep = vec![false; lines.len()];
        for (idx, line) in lines.iter().enumerate() {
            if is_build_progress_line(line) {
                compiled_folded += 1;
                continue;
            }
            if is_passing_test_line(line) {
                passed_folded += 1;
                continue;
            }
            if is_failure_anchor(line) || is_build_summary_line(line) {
                let start = idx.saturating_sub(2);
                let end = (idx + 3).min(lines.len());
                for slot in &mut keep[start..end] {
                    *slot = true;
                }
            }
        }

        let mut kept: Vec<String> = Vec::new();
        if compiled_folded > 0 || passed_folded > 0 {
            kept.push(format!(
                "... [build_test: 折叠 {} 条编译进度行, {} 条通过用例行]",
                compiled_folded, passed_folded
            ));
        }
        for (idx, line) in lines.iter().enumerate() {
            if keep[idx] && !is_build_progress_line(line) && !is_passing_test_line(line) {
                kept.push((*line).to_string());
            }
        }
        if kept.len() <= 1 {
            return distill_head_tail(lines, max_lines);
        }
        if kept.len() > max_lines {
            let refs: Vec<&str> = kept.iter().map(|s| s.as_str()).collect();
            return distill_head_tail(&refs, max_lines);
        }
        return kept;
    }

    // exit_code == 0：折叠编译进度和逐行通过项，保留关键汇总、去重 warning 与尾部窗口
    let mut warnings_seen: HashSet<&str> = HashSet::new();
    let mut kept: Vec<String> = Vec::new();
    let mut other_lines: Vec<&str> = Vec::new();

    for line in lines {
        if is_build_progress_line(line) {
            compiled_folded += 1;
            continue;
        }
        if is_passing_test_line(line) {
            passed_folded += 1;
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("warning:") {
            if warnings_seen.len() < 20 && warnings_seen.insert(trimmed) {
                kept.push((*line).to_string());
            }
            continue;
        }
        if is_build_summary_line(line) {
            kept.push((*line).to_string());
        } else {
            other_lines.push(line);
        }
    }

    let mut out: Vec<String> = Vec::new();
    if compiled_folded > 0 || passed_folded > 0 {
        out.push(format!(
            "... [build_test: 已折叠 {} 条编译进度行, {} 条通过用例行]",
            compiled_folded, passed_folded
        ));
    }

    let budget_for_other = max_lines.saturating_sub(out.len() + kept.len() + 2);
    if other_lines.len() <= budget_for_other {
        for l in other_lines {
            out.push(l.to_string());
        }
    } else if budget_for_other > 0 {
        let skipped = other_lines.len() - budget_for_other;
        out.push(format!("... [省略 {} 条中间输出行] ...", skipped));
        for l in &other_lines[other_lines.len() - budget_for_other..] {
            out.push((*l).to_string());
        }
    }

    out.extend(kept);
    if out.len() > max_lines {
        let refs: Vec<&str> = out.iter().map(|s| s.as_str()).collect();
        return distill_head_tail(&refs, max_lines);
    }
    out
}

fn distill_git_diff(lines: &[&str], max_lines: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut file_hunk_lines = 0usize;
    let mut file_folded = 0usize;
    let per_file_cap = 20usize;

    for line in lines {
        let is_header = line.starts_with("diff --git ")
            || line.starts_with("index ")
            || line.starts_with("--- ")
            || line.starts_with("+++ ")
            || line.starts_with("commit ")
            || line.starts_with("Author:")
            || line.starts_with("Date:");

        if is_header {
            if file_folded > 0 {
                out.push(format!("... [当前文件折叠 {} 行 diff] ...", file_folded));
                file_folded = 0;
            }
            if line.starts_with("diff --git ") {
                file_hunk_lines = 0;
            }
            out.push((*line).to_string());
        } else if file_hunk_lines < per_file_cap {
            file_hunk_lines += 1;
            out.push((*line).to_string());
        } else {
            file_folded += 1;
        }

        if out.len() >= max_lines.saturating_sub(2) {
            break;
        }
    }

    if file_folded > 0 {
        out.push(format!("... [当前文件折叠 {} 行 diff] ...", file_folded));
    }
    if out.len() < lines.len() && out.len() <= max_lines {
        let omitted = lines.len().saturating_sub(out.len());
        if omitted > 0 && out.len() < max_lines {
            out.push(format!("... [git_diff 共计 {} 行，已按文件限流折叠] ...", lines.len()));
        }
    }
    out
}

fn distill_search_list(lines: &[&str], max_lines: usize) -> Vec<String> {
    let head_cap = max_lines.saturating_sub(10).max(10).min(lines.len());
    let mut out: Vec<String> = lines[..head_cap].iter().map(|s| (*s).to_string()).collect();

    if head_cap >= lines.len() {
        return out;
    }

    let tail = &lines[head_cap..];
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for line in tail {
        if let Some((path, rest)) = line.split_once(':') {
            let first_seg = rest.split(':').next().unwrap_or("");
            if !path.is_empty() && first_seg.chars().all(|c| c.is_ascii_digit()) {
                *counts.entry(path).or_insert(0) += 1;
            }
        }
    }

    out.push(format!("... [省略后续 {} 行匹配] ...", tail.len()));
    if !counts.is_empty() {
        let mut ranked: Vec<(&str, usize)> = counts.into_iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
        let summary: Vec<String> = ranked
            .into_iter()
            .take(5)
            .map(|(p, c)| format!("{} ({})", p, c))
            .collect();
        out.push(format!("... [折叠命中的高频文件 Top5: {}]", summary.join(", ")));
    }
    out
}

fn distill_head_tail(lines: &[&str], max_lines: usize) -> Vec<String> {
    if lines.len() <= max_lines {
        return lines.iter().map(|s| (*s).to_string()).collect();
    }
    let usable = max_lines.saturating_sub(1).max(2);
    let head_n = (usable * 60) / 100;
    let tail_n = usable.saturating_sub(head_n);
    let skipped = lines.len().saturating_sub(head_n + tail_n);

    let mut out: Vec<String> = Vec::with_capacity(max_lines);
    for l in &lines[..head_n] {
        out.push((*l).to_string());
    }
    out.push(format!("... [省略 {} 行] ...", skipped));
    if tail_n > 0 {
        for l in &lines[lines.len() - tail_n..] {
            out.push((*l).to_string());
        }
    }
    out
}

fn cap_bytes_utf8(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text.to_string();
    }
    let mut end = 0usize;
    for (idx, ch) in text.char_indices() {
        let next = idx + ch.len_utf8();
        if next > max_bytes {
            break;
        }
        end = next;
    }
    if let Some(last_nl) = text[..end].rfind('\n') {
        if last_nl > end / 2 {
            return text[..last_nl].to_string();
        }
    }
    text[..end].to_string()
}

fn distill_log_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join(".agent-body").join("tmp").join("distill")
}

fn persist_full_output(raw: &str, cmd_hint: &str) -> Option<PathBuf> {
    let dir = distill_log_dir();
    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("[Distill][WARN] 创建落盘目录失败 ({:?}): {}", dir, e);
        return None;
    }
    prune_old_distill_logs(&dir, DEFAULT_MAX_LOG_FILES);

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let pid = std::process::id();
    let hash = fnv1a_32(cmd_hint.as_bytes());
    let file_path = dir.join(format!("{}_{}_{:08x}.log", ts, pid, hash));

    if let Err(e) = fs::write(&file_path, raw) {
        eprintln!("[Distill][WARN] 写入无损日志失败 ({:?}): {}", file_path, e);
        return None;
    }
    Some(file_path)
}

fn prune_old_distill_logs(dir: &Path, max_files: usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|ent| {
            let path = ent.path();
            if path.is_file() {
                let name = ent.file_name().to_string_lossy().to_string();
                Some((name, path))
            } else {
                None
            }
        })
        .collect();
    if files.len() < max_files {
        return;
    }
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let to_remove = files.len().saturating_sub(max_files.saturating_sub(1));
    for (_, path) in files.into_iter().take(to_remove) {
        if let Err(e) = fs::remove_file(&path) {
            eprintln!("[Distill][WARN] 清理旧落盘日志失败 ({:?}): {}", path, e);
        }
    }
}

fn fnv1a_32(data: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for &b in data {
        hash ^= b as u32;
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash
}

/// 执行命令并对超阈值输出执行确定性蒸馏 + 无损落盘，100% 透传原始退出码
pub fn run_distill_cli(
    kind_str: &str,
    max_lines: usize,
    max_bytes: usize,
    shell: Option<&str>,
    cmd: &[String],
) -> Result<i32> {
    let (mut command, cmd_label) = if let Some(sh) = shell.filter(|s| !s.trim().is_empty()) {
        let mut c = Command::new("bash");
        c.arg("-c").arg(sh);
        (c, sh.to_string())
    } else if !cmd.is_empty() {
        let mut c = Command::new(&cmd[0]);
        if cmd.len() > 1 {
            c.args(&cmd[1..]);
        }
        (c, cmd.join(" "))
    } else {
        anyhow::bail!("请通过 --shell <命令> 或尾随位置参数提供需要执行的命令");
    };

    let output = command
        .output()
        .with_context(|| format!("执行命令失败: {}", cmd_label))?;

    let exit_code = output.status.code().unwrap_or(1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    let mut combined = String::with_capacity(stdout.len() + stderr.len() + 2);
    combined.push_str(&stdout);
    if !stderr.is_empty() {
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&stderr);
    }

    let parsed_kind = DistillKind::from_str_loose(kind_str);
    let effective_kind = if parsed_kind == DistillKind::Auto {
        DistillKind::detect_from_command(&cmd_label)
    } else {
        parsed_kind
    };

    let res = distill_output(&combined, effective_kind, exit_code, max_lines, max_bytes);

    if !res.distilled {
        if !res.text.is_empty() {
            print!("{}", res.text);
        }
        return Ok(exit_code);
    }

    let log_path_disp = persist_full_output(&combined, &cmd_label)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "(persist_failed)".to_string());
    let saved_pct = if res.raw_bytes > 0 {
        100usize.saturating_sub((res.out_bytes * 100) / res.raw_bytes)
    } else {
        0
    };

    print!("{}", res.text);
    println!(
        "[agent-body distill: {}B ({}L) -> {}B ({}L), -{}% | kind={} | exit={} | full: {}]",
        res.raw_bytes,
        res.raw_lines,
        res.out_bytes,
        res.out_lines,
        saved_pct,
        res.kind.as_str(),
        exit_code,
        log_path_disp
    );

    Ok(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_small_output_passes_through_without_distill() {
        let raw = "line 1\nline 2\nok\n";
        let res = distill_output(raw, DistillKind::BuildTest, 0, 120, 6144);
        assert!(!res.distilled);
        assert_eq!(res.text, raw);
    }

    #[test]
    fn test_strip_ansi_and_progress_bars() {
        let raw = "\x1b[32mOK\x1b[0m\r\n  10% foo\r  100% done\nnext\n";
        let cleaned = strip_ansi_and_progress(raw);
        assert!(!cleaned.contains("\x1b["));
        assert!(!cleaned.contains("10% foo"));
        assert!(cleaned.contains("100% done"));
        assert!(cleaned.contains("OK"));
    }

    #[test]
    fn test_build_test_success_folds_compiling_and_passing_tests() {
        let mut lines = Vec::new();
        for i in 0..80 {
            lines.push(format!("   Compiling crate_{} v0.1.0", i));
        }
        for i in 0..120 {
            lines.push(format!("test module::tests::case_{} ... ok", i));
        }
        lines.push("test result: ok. 120 passed; 0 failed; 0 ignored".to_string());
        lines.push("Finished `test` profile in 4.2s".to_string());
        let raw = lines.join("\n");

        let res = distill_output(&raw, DistillKind::BuildTest, 0, 60, 4096);
        assert!(res.distilled);
        assert!(res.text.contains("test result: ok. 120 passed"));
        assert!(res.text.contains("已折叠 80 条编译进度行, 120 条通过用例行"));
        assert!(res.out_bytes * 10 < res.raw_bytes * 3, "压缩比应 > 70%");
    }

    #[test]
    fn test_build_test_failure_preserves_panic_and_assertion_context() {
        let mut lines = Vec::new();
        for i in 0..150 {
            lines.push(format!("test ok_case_{} ... ok", i));
        }
        lines.push("test broken_case ... FAILED".to_string());
        lines.push("thread 'broken_case' panicked at src/foo.rs:42:9:".to_string());
        lines.push("assertion `left == right` failed".to_string());
        lines.push("  left: 1".to_string());
        lines.push(" right: 2".to_string());
        lines.push("test result: FAILED. 150 passed; 1 failed".to_string());
        let raw = lines.join("\n");

        let res = distill_output(&raw, DistillKind::BuildTest, 101, 60, 4096);
        assert!(res.distilled);
        assert!(res.text.contains("broken_case ... FAILED"));
        assert!(res.text.contains("panicked at src/foo.rs:42:9"));
        assert!(res.text.contains("assertion `left == right` failed"));
    }

    #[test]
    fn test_search_list_summarizes_top_files() {
        let mut lines = Vec::new();
        for i in 1..=160 {
            let file = if i % 2 == 0 { "src/a.rs" } else { "src/b.rs" };
            lines.push(format!("{}:{}:fn match_{}()", file, i, i));
        }
        let raw = lines.join("\n");

        let res = distill_output(&raw, DistillKind::SearchList, 0, 40, 4096);
        assert!(res.distilled);
        assert!(res.text.contains("高频文件 Top5"));
        assert!(res.text.contains("src/a.rs"));
    }

    #[test]
    fn test_git_diff_folds_excess_hunk_lines() {
        let mut lines = vec![
            "diff --git a/src/foo.rs b/src/foo.rs".to_string(),
            "--- a/src/foo.rs".to_string(),
            "+++ b/src/foo.rs".to_string(),
        ];
        for i in 0..100 {
            lines.push(format!("+let line_{} = {};", i, i));
        }
        let raw = lines.join("\n");

        let res = distill_output(&raw, DistillKind::GitDiff, 0, 50, 4096);
        assert!(res.distilled);
        assert!(res.text.contains("diff --git a/src/foo.rs b/src/foo.rs"));
        assert!(res.text.contains("折叠"));
    }

    #[test]
    fn test_run_distill_cli_preserves_nonzero_exit_code() {
        let code = run_distill_cli("auto", 80, 4096, Some("echo fail_line; exit 7"), &[])
            .expect("run_distill_cli should execute");
        assert_eq!(code, 7);
    }
}
