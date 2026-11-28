//! 原生 PreToolUse Token 守卫引擎 (`TokenGuardEngine`)
//!
//! 在零额外第三方 MCP Server 依赖下，于前置 Hook 链路拦截或自适应裁剪高 Token 消耗工具调用：
//! 1. `read_file` / `view_file` / `read`：阻断无关大目录、锁文件、编译/压缩产物与超长单行文本整读
//!    （显式传入 `<= 200` 行范围可作为逃生口放行）；超出体积/行数阈值的大文件自动注入首屏切片 `overwrite`。
//! 2. `run_command` / `bash` / `exec`：为独立 `rg` 兜底注入 `~/.ripgreprc` 与输出行上限，
//!    为简单递归 `grep` 自动补充 `--exclude-dir` 与输出行上限，为无浅层限制的独立 `find` 补充输出行上限。
//! 3. `grep_search` / `find_by_name` / `grep` / `glob`：拦截直接指向噪音构建/依赖目录的搜索范围。

use std::env;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde_json::{json, Value};

use crate::config::Config;
use crate::hooks::distill::DistillKind;

const NOISE_DIR_SEGMENTS: &[&str] = &[
    "node_modules",
    "__pycache__",
    "dist",
    "build",
    ".venv",
    "venv",
    ".next",
    ".nuxt",
    ".cache",
    "coverage",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".tox",
    ".eggs",
];

const RUST_TARGET_SUBDIRS: &[&str] = &["debug", "release", "doc", "tmp", "llvm-cov-target"];

const NOISE_FILENAMES: &[&str] = &[
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "Cargo.lock",
    "poetry.lock",
    "uv.lock",
    "composer.lock",
    "Gemfile.lock",
    "go.sum",
];

const NOISE_SUFFIXES: &[&str] = &[
    ".min.js",
    ".min.css",
    ".bundle.js",
    ".chunk.js",
    ".map",
    ".pyc",
    ".pyo",
    ".wasm",
];

const BINARY_OR_MEDIA_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "bmp", "ico", "pdf", "mp3", "mp4", "wav", "ogg",
    "webm", "mov", "avi", "zip", "tar", "gz", "xz", "7z", "db", "sqlite", "duckdb",
];

const MINIFIED_CHECK_BYTES: usize = 16 * 1024;
const MAX_EXPLICIT_NOISE_SLICE: i64 = 200;

const GREP_EXCLUDE_ARGS: &str = "--exclude-dir=node_modules --exclude-dir=__pycache__ --exclude-dir=dist --exclude-dir=build --exclude-dir=target --exclude-dir=.git --exclude-dir=.venv --exclude-dir=.next --exclude-dir=.cache --exclude-dir=.codegraph";

/// Token 守卫运行策略（由 `[runtime]` 与 `[assets]` 配置构造）
#[derive(Debug, Clone)]
pub struct TokenGuardPolicy {
    pub enabled: bool,
    pub max_full_read_bytes: usize,
    pub max_full_read_lines: usize,
    pub auto_slice_lines: usize,
    pub search_max_lines: usize,
    pub distill_enabled: bool,
    pub distill_max_lines: usize,
    pub distill_max_bytes: usize,
    pub read_ledger_enabled: bool,
    pub read_ledger_window_steps: usize,
    pub outline_enabled: bool,
    pub outline_max_bytes: usize,
    pub extra_ignore_dirs: Vec<String>,
}

impl Default for TokenGuardPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            max_full_read_bytes: 36 * 1024,
            max_full_read_lines: 600,
            auto_slice_lines: 250,
            search_max_lines: 180,
            distill_enabled: true,
            distill_max_lines: 120,
            distill_max_bytes: 6144,
            read_ledger_enabled: true,
            read_ledger_window_steps: 40,
            outline_enabled: true,
            outline_max_bytes: 1400,
            extra_ignore_dirs: Vec::new(),
        }
    }
}

impl TokenGuardPolicy {
    /// 从全局 `Config` 构造 Token 守卫策略
    pub fn from_config(cfg: &Config) -> Self {
        let mut extra = Vec::new();
        for pat in &cfg.assets.ignore_patterns {
            let trimmed = pat.trim().trim_matches('/');
            if trimmed.is_empty()
                || trimmed.contains('*')
                || trimmed.contains('/')
                || matches!(trimmed, "target" | ".git" | ".agent-body")
                || NOISE_DIR_SEGMENTS.contains(&trimmed)
            {
                continue;
            }
            extra.push(trimmed.to_string());
        }
        Self {
            enabled: cfg.runtime.token_guard_enabled,
            max_full_read_bytes: cfg.runtime.token_guard_max_full_read_bytes.max(4096),
            max_full_read_lines: cfg.runtime.token_guard_max_full_read_lines.max(50),
            auto_slice_lines: cfg.runtime.token_guard_auto_slice_lines.max(20),
            search_max_lines: cfg.runtime.token_guard_search_max_lines.max(20),
            distill_enabled: cfg.runtime.token_guard_distill_enabled,
            distill_max_lines: cfg.runtime.token_guard_distill_max_lines.clamp(20, 2000),
            distill_max_bytes: cfg.runtime.token_guard_distill_max_bytes.clamp(1024, 65536),
            read_ledger_enabled: cfg.runtime.token_guard_read_ledger_enabled,
            read_ledger_window_steps: cfg.runtime.token_guard_read_ledger_window_steps.clamp(5, 500),
            outline_enabled: cfg.runtime.token_guard_outline_enabled,
            outline_max_bytes: cfg.runtime.token_guard_outline_max_bytes.clamp(256, 8192),
            extra_ignore_dirs: extra,
        }
    }
}

/// Token 守卫评估结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenGuardDecision {
    /// 无需拦截或改写，直接放行
    Pass,
    /// 高噪目录 / 锁文件 / 单行压缩文本，建议阻断并提示更省 Token 的替代动作
    Deny {
        reason: String,
        suggestion: String,
        target: String,
    },
    /// 放行但注入参数覆写（如大文件首屏切片、搜索命令追加忽略目录与行数截断）
    Overwrite {
        overwrite: Value,
        suggestion: String,
        target: String,
    },
}

/// 原生 PreToolUse Token 守卫引擎
pub struct TokenGuardEngine;

impl TokenGuardEngine {
    /// 评估工具调用是否需要拦截或注入参数裁剪
    pub fn evaluate(
        tool_name: &str,
        args: &Value,
        policy: &TokenGuardPolicy,
    ) -> TokenGuardDecision {
        if !policy.enabled || is_env_disabled() {
            return TokenGuardDecision::Pass;
        }
        let lower = tool_name.trim().to_ascii_lowercase();
        if matches!(lower.as_str(), "read_file" | "view_file" | "read") {
            return evaluate_read(&lower, args, policy);
        }
        if matches!(lower.as_str(), "run_command" | "bash" | "exec") {
            return evaluate_command(args, policy);
        }
        if matches!(
            lower.as_str(),
            "grep_search" | "find_by_name" | "grep" | "glob"
        ) {
            return evaluate_search_tool(args, policy);
        }
        TokenGuardDecision::Pass
    }
}

fn is_env_disabled() -> bool {
    for key in ["AGENT_BODY_TOKEN_GUARD", "AGY_TOKEN_OPTIMIZER"] {
        if let Ok(val) = env::var(key) {
            let norm = val.trim().to_ascii_lowercase();
            if matches!(norm.as_str(), "0" | "off" | "false" | "no") {
                return true;
            }
        }
    }
    false
}

fn detect_noise_path(raw_path: &str, extra_dirs: &[String]) -> Option<String> {
    if raw_path.trim().is_empty() {
        return None;
    }
    let norm = raw_path.replace('\\', "/");
    if norm.contains("/.git/objects/") || norm.ends_with("/.git/objects") {
        return Some(".git/objects".to_string());
    }
    if norm.contains("/.cargo/registry/") || norm.ends_with("/.cargo/registry") {
        return Some(".cargo/registry".to_string());
    }

    let is_abs = norm.starts_with('/');
    let parts: Vec<&str> = norm
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .collect();

    for (idx, seg) in parts.iter().enumerate() {
        if NOISE_DIR_SEGMENTS.contains(seg) || extra_dirs.iter().any(|d| d == seg) {
            return Some(format!("目录 '{}'", seg));
        }
        if *seg == "target" {
            let next_seg = parts.get(idx + 1).copied().unwrap_or("");
            if RUST_TARGET_SUBDIRS.contains(&next_seg) {
                return Some(format!("构建目录 'target/{}'", next_seg));
            }
            let prefix = parts[..idx].join("/");
            let parent = if is_abs {
                format!("/{}", prefix)
            } else if prefix.is_empty() {
                ".".to_string()
            } else {
                prefix
            };
            if Path::new(&parent).join("Cargo.toml").exists() {
                return Some("Rust 构建目录 'target'".to_string());
            }
        }
    }

    let base = Path::new(&norm)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if NOISE_FILENAMES.contains(&base) {
        return Some(format!("锁文件 '{}'", base));
    }
    let base_lower = base.to_ascii_lowercase();
    for suf in NOISE_SUFFIXES {
        if base_lower.ends_with(suf) {
            return Some(format!("压缩/构建产物 '*{}'", suf));
        }
    }
    None
}

fn extract_int(args: &Value, keys: &[&str]) -> Option<i64> {
    for k in keys {
        if let Some(v) = args.get(*k) {
            if let Some(n) = v.as_i64() {
                return Some(n);
            }
        }
    }
    None
}

fn is_bounded_slice(args: &Value) -> bool {
    let start = extract_int(args, &["StartLine", "start_line", "offset"]);
    let end = extract_int(args, &["EndLine", "end_line"]);
    if let (Some(s), Some(e)) = (start, end) {
        return s >= 1 && e >= s && (e - s) <= MAX_EXPLICIT_NOISE_SLICE;
    }
    if let Some(limit) = extract_int(args, &["limit"]) {
        return limit >= 1 && limit <= MAX_EXPLICIT_NOISE_SLICE;
    }
    false
}

fn has_any_slice_or_offset(args: &Value) -> bool {
    extract_int(
        args,
        &[
            "StartLine",
            "start_line",
            "EndLine",
            "end_line",
            "ByteOffset",
            "byte_offset",
            "offset",
            "limit",
        ],
    )
    .is_some()
}

fn count_lines_exceeds(path: &Path, max_lines: usize) -> std::io::Result<bool> {
    let mut file = File::open(path)?;
    let mut buf = [0u8; 65_536];
    let mut newlines = 0usize;
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        newlines += buf[..n].iter().filter(|&&b| b == b'\n').count();
        if newlines > max_lines {
            return Ok(true);
        }
    }
    Ok(false)
}

fn evaluate_read(_tool_lower: &str, args: &Value, policy: &TokenGuardPolicy) -> TokenGuardDecision {
    let path_str = args
        .get("Path")
        .or_else(|| args.get("AbsolutePath"))
        .or_else(|| args.get("path"))
        .or_else(|| args.get("file_path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if path_str.trim().is_empty() {
        return TokenGuardDecision::Pass;
    }

    let bounded = is_bounded_slice(args);
    if let Some(detail) = detect_noise_path(path_str, &policy.extra_ignore_dirs) {
        if !bounded {
            return TokenGuardDecision::Deny {
                reason: format!(
                    "[token-optimizer] 跳过无关目录/锁文件/压缩产物以节省 Token ({})。",
                    detail
                ),
                suggestion: "请先用 `rg` 定位行号，或显式传入 StartLine/EndLine (<=200 行) 定点读取。"
                    .to_string(),
                target: path_str.to_string(),
            };
        }
    }

    let path = Path::new(path_str);
    if has_any_slice_or_offset(args) || !path.is_file() {
        return TokenGuardDecision::Pass;
    }

    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_ascii_lowercase();
        if BINARY_OR_MEDIA_EXTS.contains(&ext_lower.as_str()) {
            return TokenGuardDecision::Pass;
        }
    }

    let Ok(meta) = path.metadata() else {
        return TokenGuardDecision::Pass;
    };
    let size = meta.len() as usize;

    if size > MINIFIED_CHECK_BYTES {
        if let Ok(mut f) = File::open(path) {
            let mut sample = [0u8; 8192];
            if let Ok(n) = f.read(&mut sample) {
                let slice = &sample[..n];
                if slice.contains(&0u8) {
                    return TokenGuardDecision::Pass;
                }
                let nl_count = slice.iter().filter(|&&b| b == b'\n').count();
                if nl_count < 5 {
                    return TokenGuardDecision::Deny {
                        reason: format!(
                            "[token-optimizer] 文件 ({} bytes) 疑似单行压缩/超长行文本，整读易爆上下文。",
                            size
                        ),
                        suggestion: "请改用 `rg -o` / `jq` / `python3` 提取目标片段。".to_string(),
                        target: path_str.to_string(),
                    };
                }
            }
        }
    }

    let exceeds_lines = count_lines_exceeds(path, policy.max_full_read_lines).unwrap_or(false);
    if size > policy.max_full_read_bytes || exceeds_lines {
        return TokenGuardDecision::Overwrite {
            overwrite: json!({
                "StartLine": 1,
                "EndLine": policy.auto_slice_lines,
            }),
            suggestion: format!(
                "大文件已自动切片为第 1~{} 行（如需后文请先 `rg -n` 定位行号再传 StartLine/EndLine）。",
                policy.auto_slice_lines
            ),
            target: path_str.to_string(),
        };
    }

    TokenGuardDecision::Pass
}

fn has_complex_shell_ops(cmd: &str, allow_pipe: bool) -> bool {
    let basic = ["\n", ";", "&&", "||", ">", "<", "$(", "`"];
    if basic.iter().any(|op| cmd.contains(op)) {
        return true;
    }
    !allow_pipe && cmd.contains('|')
}

fn awk_cap_pipe(max_lines: usize) -> String {
    format!(
        " | awk 'NR<={0}{{print}} END{{if(NR>{0}) printf \"... [token-optimizer: truncated %d extra lines, narrow query or path]\\n\", NR-{0}}}'",
        max_lines
    )
}

fn starts_with_word(cmd: &str, word: &str) -> bool {
    if let Some(rest) = cmd.strip_prefix(word) {
        rest.is_empty() || rest.starts_with(char::is_whitespace)
    } else {
        false
    }
}

fn has_token(cmd: &str, candidates: &[&str]) -> bool {
    cmd.split_whitespace()
        .any(|tok| candidates.contains(&tok))
}

fn is_recursive_grep_cmd(cmd: &str) -> bool {
    cmd.split_whitespace().skip(1).any(|tok| {
        tok == "--recursive"
            || (tok.starts_with('-')
                && !tok.starts_with("--")
                && (tok.contains('r') || tok.contains('R')))
    })
}

fn has_shallow_maxdepth(cmd: &str) -> bool {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    for pair in tokens.windows(2) {
        if pair[0] == "-maxdepth" && matches!(pair[1], "0" | "1" | "2") {
            return true;
        }
    }
    false
}

fn ripgreprc_exists() -> bool {
    if let Ok(home) = env::var("HOME").or_else(|_| env::var("USERPROFILE")) {
        let trimmed = home.trim();
        if !trimmed.is_empty() {
            return Path::new(trimmed).join(".ripgreprc").exists();
        }
    }
    false
}

fn is_distill_env_disabled() -> bool {
    if let Ok(val) = env::var("AGENT_BODY_DISTILL") {
        let norm = val.trim().to_ascii_lowercase();
        if matches!(norm.as_str(), "0" | "off" | "false" | "no") {
            return true;
        }
    }
    false
}

fn is_daemon_invocation(args: &Value) -> bool {
    args.get("IsDaemon").and_then(|v| v.as_bool()) == Some(true)
}

fn should_skip_distill(cmd: &str) -> bool {
    let lower = cmd.to_ascii_lowercase();
    if lower.contains("agent-body distill") || lower.contains("agent-body hooks") {
        return true;
    }
    if cmd.contains("<<") {
        return true;
    }
    let trimmed = cmd.trim_end();
    if trimmed.ends_with('&') && !trimmed.ends_with("&&") {
        let bytes = trimmed.as_bytes();
        let before = if bytes.len() >= 2 { bytes[bytes.len() - 2] } else { b' ' };
        if before.is_ascii_whitespace() {
            return true;
        }
    }
    for tok in cmd.split_whitespace() {
        let clean = tok.trim_matches(|c: char| matches!(c, ';' | '&' | '|'));
        if matches!(
            clean,
            "vim" | "vi" | "nano" | "less" | "more" | "top" | "htop" | "tmux"
        ) {
            return true;
        }
    }
    false
}

fn strip_leading_env_assignments(segment: &str) -> &str {
    let mut cur = segment.trim_start();
    loop {
        let Some(first) = cur.split_whitespace().next() else {
            return "";
        };
        if let Some((k, _)) = first.split_once('=') {
            if !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                cur = cur[first.len()..].trim_start();
                continue;
            }
        }
        return cur;
    }
}

fn split_shell_segments(cmd: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let bytes = cmd.as_bytes();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut in_single = false;
    let mut in_double = false;

    while i < bytes.len() {
        let b = bytes[i];
        if b == b'\'' && !in_double {
            in_single = !in_single;
            i += 1;
            continue;
        }
        if b == b'"' && !in_single {
            in_double = !in_double;
            i += 1;
            continue;
        }
        if !in_single && !in_double {
            let is_two = i + 1 < bytes.len()
                && ((b == b'&' && bytes[i + 1] == b'&') || (b == b'|' && bytes[i + 1] == b'|'));
            if is_two {
                segments.push(&cmd[start..i]);
                i += 2;
                start = i;
                continue;
            }
            if matches!(b, b';' | b'|' | b'\n') {
                segments.push(&cmd[start..i]);
                i += 1;
                start = i;
                continue;
            }
        }
        i += 1;
    }
    if start <= cmd.len() {
        segments.push(&cmd[start..]);
    }
    segments
}

fn detect_distill_target(cmd: &str) -> Option<DistillKind> {
    let is_compound = ["\n", ";", "&&", "||"]
        .iter()
        .any(|op| cmd.contains(op));

    for seg in split_shell_segments(cmd) {
        let s = strip_leading_env_assignments(seg);
        if s.is_empty() {
            continue;
        }
        if starts_with_word(s, "cargo") {
            let sub = s["cargo".len()..].trim_start();
            if ["test", "nextest", "build", "check", "clippy"]
                .iter()
                .any(|w| starts_with_word(sub, w))
            {
                return Some(DistillKind::BuildTest);
            }
        }
        if starts_with_word(s, "make")
            || starts_with_word(s, "pytest")
            || starts_with_word(s, "python3 -m pytest")
            || (starts_with_word(s, "go") && starts_with_word(s["go".len()..].trim_start(), "test"))
            || (starts_with_word(s, "npm") && starts_with_word(s["npm".len()..].trim_start(), "test"))
            || (starts_with_word(s, "pnpm") && starts_with_word(s["pnpm".len()..].trim_start(), "test"))
            || (starts_with_word(s, "yarn") && starts_with_word(s["yarn".len()..].trim_start(), "test"))
        {
            return Some(DistillKind::BuildTest);
        }
        if starts_with_word(s, "git") {
            let sub = s["git".len()..].trim_start();
            if ["diff", "show", "log"]
                .iter()
                .any(|w| starts_with_word(sub, w))
            {
                return Some(DistillKind::GitDiff);
            }
        }
        if starts_with_word(s, "curl") || starts_with_word(s, "wget") {
            return Some(DistillKind::HttpFetch);
        }
        if starts_with_word(s, "codegraph")
            && starts_with_word(s["codegraph".len()..].trim_start(), "explore")
        {
            return Some(DistillKind::SearchList);
        }
        if is_compound
            && ["rg", "grep", "egrep", "fgrep", "find"]
                .iter()
                .any(|w| starts_with_word(s, w))
        {
            return Some(DistillKind::SearchList);
        }
    }
    None
}

fn shell_single_quote(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\\''"))
}

fn wrap_with_distill(cmd: &str, kind: DistillKind, max_lines: usize, max_bytes: usize) -> String {
    format!(
        "agent-body distill --kind {} --max-lines {} --max-bytes {} --shell {}",
        kind.as_str(),
        max_lines,
        max_bytes,
        shell_single_quote(cmd)
    )
}

fn evaluate_command(args: &Value, policy: &TokenGuardPolicy) -> TokenGuardDecision {
    let cmd_key = if args.get("CommandLine").is_some() {
        "CommandLine"
    } else if args.get("command").is_some() {
        "command"
    } else {
        "cmd"
    };
    let cmd = args
        .get("CommandLine")
        .or_else(|| args.get("command"))
        .or_else(|| args.get("cmd"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let stripped = cmd.trim();
    if stripped.is_empty() {
        return TokenGuardDecision::Pass;
    }

    let cap_pipe = awk_cap_pipe(policy.search_max_lines);
    let has_compound = ["\n", ";", "&&", "||"]
        .iter()
        .any(|sep| stripped.contains(sep));

    if starts_with_word(stripped, "rg") && !has_compound {
        let mut new_cmd = stripped.to_string();
        let env_has_rc = env::var("RIPGREP_CONFIG_PATH")
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false);
        if !env_has_rc
            && !new_cmd.contains("RIPGREP_CONFIG_PATH")
            && !new_cmd.contains("--no-config")
            && ripgreprc_exists()
        {
            new_cmd = format!("RIPGREP_CONFIG_PATH=\"$HOME/.ripgreprc\" {}", new_cmd);
        }

        let is_simple = !has_complex_shell_ops(stripped, false);
        let has_summary = has_token(
            stripped,
            &[
                "--files",
                "--count",
                "-c",
                "-l",
                "--files-with-matches",
                "--json",
            ],
        );
        if is_simple && !has_summary {
            new_cmd.push_str(&cap_pipe);
        }
        if new_cmd != stripped {
            return TokenGuardDecision::Overwrite {
                overwrite: json!({ cmd_key: new_cmd }),
                suggestion: "已为 `rg` 注入全局忽略配置与输出行数上限。".to_string(),
                target: stripped.chars().take(120).collect(),
            };
        }
        return TokenGuardDecision::Pass;
    }

    let grep_bin = ["grep", "egrep", "fgrep"]
        .iter()
        .copied()
        .find(|b| starts_with_word(stripped, b));
    if let Some(bin) = grep_bin {
        if !has_compound
            && !stripped.contains("$(")
            && !stripped.contains('`')
            && is_recursive_grep_cmd(stripped)
            && !stripped.contains("--exclude-dir")
        {
            let rest = stripped[bin.len()..].trim_start();
            let mut new_cmd = format!("{} {} {}", bin, GREP_EXCLUDE_ARGS, rest);
            let is_simple = !has_complex_shell_ops(stripped, false);
            let has_summary = has_token(stripped, &["-c", "--count", "-l", "--files-with-matches"]);
            if is_simple && !has_summary {
                new_cmd.push_str(&cap_pipe);
            }
            return TokenGuardDecision::Overwrite {
                overwrite: json!({ cmd_key: new_cmd }),
                suggestion: "已为递归 `grep` 自动注入噪音目录排除与输出行数上限。".to_string(),
                target: stripped.chars().take(120).collect(),
            };
        }
        if !has_compound {
            return TokenGuardDecision::Pass;
        }
    }

    if starts_with_word(stripped, "find") && !has_complex_shell_ops(stripped, false) {
        if has_token(
            stripped,
            &["-exec", "-execdir", "-delete", "-ok", "-okdir"],
        ) || has_shallow_maxdepth(stripped)
        {
            return TokenGuardDecision::Pass;
        }
        let new_cmd = format!("{}{}", stripped, cap_pipe);
        return TokenGuardDecision::Overwrite {
            overwrite: json!({ cmd_key: new_cmd }),
            suggestion: "已为独立 `find` 自动追加输出行数上限保护。".to_string(),
            target: stripped.chars().take(120).collect(),
        };
    }

    if policy.distill_enabled
        && !is_distill_env_disabled()
        && !is_daemon_invocation(args)
        && !should_skip_distill(stripped)
    {
        if let Some(kind) = detect_distill_target(stripped) {
            let wrapped = wrap_with_distill(
                stripped,
                kind,
                policy.distill_max_lines,
                policy.distill_max_bytes,
            );
            return TokenGuardDecision::Overwrite {
                overwrite: json!({ cmd_key: wrapped }),
                suggestion: format!(
                    "已自动包装高噪/复合命令为 `agent-body distill` ({}) 流式输出蒸馏（无损落盘 + 退出码保真）。",
                    kind.as_str()
                ),
                target: stripped.chars().take(120).collect(),
            };
        }
    }

    TokenGuardDecision::Pass
}

fn evaluate_search_tool(args: &Value, policy: &TokenGuardPolicy) -> TokenGuardDecision {
    let target = args
        .get("SearchPath")
        .or_else(|| args.get("SearchDirectory"))
        .or_else(|| args.get("AbsolutePath"))
        .or_else(|| args.get("Directory"))
        .or_else(|| args.get("path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if target.trim().is_empty() {
        return TokenGuardDecision::Pass;
    }
    if let Some(detail) = detect_noise_path(target, &policy.extra_ignore_dirs) {
        return TokenGuardDecision::Deny {
            reason: format!(
                "[token-optimizer] 跳过在无关目录中搜索 ({}) 以节省 Token。",
                detail
            ),
            suggestion: "请缩小 SearchPath 至源码目录。".to_string(),
            target: target.to_string(),
        };
    }
    TokenGuardDecision::Pass
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_read_file_noise_dir_and_lockfile_escape_hatch() {
        let dir = tempdir().expect("tempdir");
        let nm = dir.path().join("node_modules");
        fs::create_dir_all(&nm).expect("mkdir");
        let nm_file = nm.join("a.js");
        fs::write(&nm_file, "const a = 1;\n").expect("write");
        let lock_file = dir.path().join("Cargo.lock");
        fs::write(&lock_file, "# lock\n").expect("write");

        let policy = TokenGuardPolicy::default();

        let d1 = TokenGuardEngine::evaluate(
            "read_file",
            &json!({"Path": nm_file.to_string_lossy()}),
            &policy,
        );
        assert!(matches!(d1, TokenGuardDecision::Deny { .. }));

        let d2 = TokenGuardEngine::evaluate(
            "read_file",
            &json!({"Path": lock_file.to_string_lossy()}),
            &policy,
        );
        assert!(matches!(d2, TokenGuardDecision::Deny { .. }));

        let d2_sliced = TokenGuardEngine::evaluate(
            "read_file",
            &json!({"Path": lock_file.to_string_lossy(), "StartLine": 1, "EndLine": 50}),
            &policy,
        );
        assert_eq!(d2_sliced, TokenGuardDecision::Pass);
    }

    #[test]
    fn test_read_file_small_large_and_minified() {
        let dir = tempdir().expect("tempdir");
        let small = dir.path().join("small.rs");
        fs::write(
            &small,
            (1..=300).map(|i| format!("let x{} = {};\n", i, i)).collect::<String>(),
        )
        .expect("write small");

        let large = dir.path().join("large.rs");
        fs::write(
            &large,
            (1..=900).map(|i| format!("let x{} = {};\n", i, i)).collect::<String>(),
        )
        .expect("write large");

        let minified = dir.path().join("bundle.txt");
        fs::write(&minified, "a=1;".repeat(6500)).expect("write minified");

        let policy = TokenGuardPolicy::default();

        assert_eq!(
            TokenGuardEngine::evaluate(
                "read_file",
                &json!({"Path": small.to_string_lossy()}),
                &policy
            ),
            TokenGuardDecision::Pass
        );

        let d_large = TokenGuardEngine::evaluate(
            "read_file",
            &json!({"Path": large.to_string_lossy()}),
            &policy,
        );
        assert_eq!(
            d_large,
            TokenGuardDecision::Overwrite {
                overwrite: json!({"StartLine": 1, "EndLine": 250}),
                suggestion: "大文件已自动切片为第 1~250 行（如需后文请先 `rg -n` 定位行号再传 StartLine/EndLine）。".to_string(),
                target: large.to_string_lossy().to_string(),
            }
        );

        let d_min = TokenGuardEngine::evaluate(
            "read_file",
            &json!({"Path": minified.to_string_lossy()}),
            &policy,
        );
        assert!(matches!(d_min, TokenGuardDecision::Deny { .. }));
    }

    #[test]
    fn test_run_command_rg_grep_find_and_compound_passthrough() {
        let policy = TokenGuardPolicy::default();

        let d_grep = TokenGuardEngine::evaluate(
            "run_command",
            &json!({"CommandLine": "grep -rn needle ."}),
            &policy,
        );
        match d_grep {
            TokenGuardDecision::Overwrite { overwrite, .. } => {
                let cmd = overwrite["CommandLine"].as_str().unwrap_or("");
                assert!(cmd.contains("--exclude-dir=node_modules"));
                assert!(cmd.contains("awk 'NR<=180"));
            }
            other => panic!("expected Overwrite, got {:?}", other),
        }

        let d_rg = TokenGuardEngine::evaluate(
            "run_command",
            &json!({"CommandLine": "rg needle src/"}),
            &policy,
        );
        match d_rg {
            TokenGuardDecision::Overwrite { overwrite, .. } => {
                let cmd = overwrite["CommandLine"].as_str().unwrap_or("");
                assert!(cmd.contains("awk 'NR<=180"));
            }
            other => panic!("expected Overwrite, got {:?}", other),
        }

        let d_find = TokenGuardEngine::evaluate(
            "run_command",
            &json!({"CommandLine": "find . -name '*.rs'"}),
            &policy,
        );
        assert!(matches!(d_find, TokenGuardDecision::Overwrite { .. }));

        let d_find_shallow = TokenGuardEngine::evaluate(
            "run_command",
            &json!({"CommandLine": "find . -maxdepth 2 -type f"}),
            &policy,
        );
        assert_eq!(d_find_shallow, TokenGuardDecision::Pass);

        let d_compound = TokenGuardEngine::evaluate(
            "run_command",
            &json!({"CommandLine": "git status && ls -la"}),
            &policy,
        );
        assert_eq!(d_compound, TokenGuardDecision::Pass);
    }

    #[test]
    fn test_search_tool_and_rust_target_detection() {
        let dir = tempdir().expect("tempdir");
        fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"x\"\n").expect("Cargo.toml");
        let target_file = dir.path().join("target").join("foo.rs");
        let policy = TokenGuardPolicy::default();

        let d_target = TokenGuardEngine::evaluate(
            "read_file",
            &json!({"Path": target_file.to_string_lossy()}),
            &policy,
        );
        assert!(matches!(d_target, TokenGuardDecision::Deny { .. }));

        let d_search = TokenGuardEngine::evaluate(
            "grep_search",
            &json!({"SearchPath": "/repo/__pycache__"}),
            &policy,
        );
        assert!(matches!(d_search, TokenGuardDecision::Deny { .. }));
    }

    #[test]
    fn test_distill_wrapping_for_compound_and_high_noise_commands() {
        let policy = TokenGuardPolicy::default();

        let d_cargo = TokenGuardEngine::evaluate(
            "run_command",
            &json!({"CommandLine": "RUST_TEST_THREADS=1 cargo test && make lint-hygiene"}),
            &policy,
        );
        match d_cargo {
            TokenGuardDecision::Overwrite { overwrite, .. } => {
                let cmd = overwrite["CommandLine"].as_str().unwrap_or("");
                assert!(cmd.starts_with("agent-body distill --kind build_test"));
                assert!(cmd.contains("--shell "));
            }
            other => panic!("expected distill Overwrite for cargo test, got {:?}", other),
        }

        let d_curl = TokenGuardEngine::evaluate(
            "run_command",
            &json!({"CommandLine": "curl -sSf https://example.com/a.json"}),
            &policy,
        );
        match d_curl {
            TokenGuardDecision::Overwrite { overwrite, .. } => {
                let cmd = overwrite["CommandLine"].as_str().unwrap_or("");
                assert!(cmd.contains("--kind http_fetch"));
            }
            other => panic!("expected distill Overwrite for curl, got {:?}", other),
        }

        let d_compound_rg = TokenGuardEngine::evaluate(
            "run_command",
            &json!({"CommandLine": "rg -n foo src/ && echo done"}),
            &policy,
        );
        match d_compound_rg {
            TokenGuardDecision::Overwrite { overwrite, .. } => {
                let cmd = overwrite["CommandLine"].as_str().unwrap_or("");
                assert!(cmd.contains("--kind search_list"));
            }
            other => panic!("expected distill Overwrite for compound rg, got {:?}", other),
        }

        // 防递归跳过、守护进程跳过
        let d_skip_self = TokenGuardEngine::evaluate(
            "run_command",
            &json!({"CommandLine": "agent-body distill --kind auto --shell 'cargo test'"}),
            &policy,
        );
        assert_eq!(d_skip_self, TokenGuardDecision::Pass);

        let d_daemon = TokenGuardEngine::evaluate(
            "run_command",
            &json!({"CommandLine": "cargo test", "IsDaemon": true}),
            &policy,
        );
        assert_eq!(d_daemon, TokenGuardDecision::Pass);
    }
}
