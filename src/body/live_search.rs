use std::collections::{BTreeMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::benchmark::provenance::estimate_tokens;
use crate::scanner::importance::path_matches_ignore_pattern;
use crate::scanner::metadata::{is_sensitive_file, SecretRedactor};

/// 原生基线检索（模拟不使用 agent-body 时的 rg / grep / WalkDir 全文模糊检索）单条命中
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSearchHit {
    pub path: String,
    pub line_number: Option<usize>,
    pub snippet: String,
    pub match_score: usize,
    pub match_kind: String, // "content" | "path" | "multi_token"
}

/// 原生基线检索完整运行摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSearchRun {
    pub engine: String, // "ripgrep" | "grep" | "walkdir_full"
    pub hits: Vec<LiveSearchHit>,
    pub scanned_files: usize,
    pub elapsed_us: i64,
    pub tokens: i64,
    pub timed_out: bool,
}

fn truncate_line(line: &str, max_chars: usize) -> String {
    let clean = SecretRedactor::redact_single_line(line.trim());
    let mut chars = clean.chars();
    let taken: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", taken)
    } else {
        taken
    }
}

fn tool_available(bin: &str) -> bool {
    Command::new(bin)
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn append_ignore_globs(cmd: &mut Command, ignore_patterns: &[String]) {
    for pat in ignore_patterns {
        let p = pat.trim().trim_matches('/');
        if !p.is_empty() {
            cmd.arg("--glob").arg(format!("!**/{}/**", p));
            cmd.arg("--glob").arg(format!("!{}", p));
        }
    }
    for sens in [
        "!**/.ssh/**",
        "!**/.gnupg/**",
        "!.env",
        "!.env.*",
        "!*.pem",
        "!*.key",
        "!*.p12",
        "!*.pfx",
    ] {
        cmd.arg("--glob").arg(sens);
    }
}

fn collect_rg_stream(
    mut cmd: Command,
    limit_lines: usize,
    deadline: Instant,
) -> (Vec<(String, Option<usize>, String)>, bool) {
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    let Ok(mut child) = cmd.spawn() else {
        return (Vec::new(), false);
    };
    let mut rows = Vec::new();
    let mut timed_out = false;
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line_res in reader.lines() {
            if Instant::now() >= deadline {
                timed_out = true;
                break;
            }
            let Ok(line) = line_res else {
                break;
            };
            if let Some((path, ln, text)) = parse_grep_line(&line) {
                rows.push((path, Some(ln), text));
                if rows.len() >= limit_lines {
                    break;
                }
            }
        }
    }
    if timed_out || rows.len() >= limit_lines {
        if child.kill().is_err() {
            // child may have exited cleanly already
        }
    }
    if child.wait().is_err() {
        // ignore wait error after kill
    }
    (rows, timed_out)
}

fn parse_grep_line(line: &str) -> Option<(String, usize, String)> {
    // Format: /abs/or/rel/path:123:line text
    let first_colon = line.find(':')?;
    let path = &line[..first_colon];
    let rest = &line[first_colon + 1..];
    let second_colon = rest.find(':')?;
    let ln = rest[..second_colon].parse::<usize>().ok()?;
    let text = rest[second_colon + 1..].to_string();
    Some((path.to_string(), ln, text))
}

fn collect_rg_filename_hits(
    roots: &[PathBuf],
    ignore_patterns: &[String],
    query_lower: &str,
    limit: usize,
    deadline: Instant,
) -> Vec<String> {
    let mut cmd = Command::new("rg");
    cmd.arg("--files").arg("--hidden").arg("--no-messages");
    append_ignore_globs(&mut cmd, ignore_patterns);
    for r in roots {
        cmd.arg(r);
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    let Ok(mut child) = cmd.spawn() else {
        return Vec::new();
    };
    let mut matched = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let reader = BufReader::new(stdout);
        for line_res in reader.lines() {
            if Instant::now() >= deadline || matched.len() >= limit {
                break;
            }
            let Ok(p_str) = line_res else {
                break;
            };
            let p = Path::new(&p_str);
            if is_sensitive_file(p) {
                continue;
            }
            if p_str.to_ascii_lowercase().contains(query_lower) {
                matched.push(p_str);
            }
        }
    }
    if child.kill().is_err() {
        // child already exited
    }
    if child.wait().is_err() {
        // ignore
    }
    matched
}

fn run_ripgrep_baseline(
    roots: &[PathBuf],
    ignore_patterns: &[String],
    query: &str,
    limit: usize,
    deadline: Instant,
) -> (Vec<LiveSearchHit>, bool) {
    let mut by_path: BTreeMap<String, LiveSearchHit> = BTreeMap::new();
    let mut timed_out = false;
    let q_trim = query.trim();
    let q_lower = q_trim.to_ascii_lowercase();

    // Pass 1: exact fixed-string match across full files (same as rg -n -F)
    let mut cmd = Command::new("rg");
    cmd.args([
        "--line-number",
        "--no-heading",
        "--no-messages",
        "--color",
        "never",
        "--smart-case",
        "--fixed-strings",
        "--max-count",
        "1",
        "--max-filesize",
        "20M",
    ]);
    append_ignore_globs(&mut cmd, ignore_patterns);
    cmd.arg("--").arg(q_trim);
    for r in roots {
        cmd.arg(r);
    }
    let (rows, to1) = collect_rg_stream(cmd, limit * 4, deadline);
    timed_out |= to1;
    for (p, ln, text) in rows {
        if is_sensitive_file(Path::new(&p)) {
            continue;
        }
        let path_bonus = usize::from(p.to_ascii_lowercase().contains(&q_lower));
        by_path.entry(p.clone()).or_insert(LiveSearchHit {
            path: p,
            line_number: ln,
            snippet: truncate_line(&text, 140),
            match_score: 4 + path_bonus * 2,
            match_kind: "content".to_string(),
        });
    }

    // Pass 2: multi-word queries (all whitespace tokens present across file)
    let tokens: Vec<String> = q_trim
        .split_whitespace()
        .filter(|s| s.chars().count() >= 2)
        .map(str::to_string)
        .collect();
    if tokens.len() > 1 && by_path.len() < limit && Instant::now() < deadline {
        let mut intersection: Option<HashSet<String>> = None;
        let mut first_snippets: BTreeMap<String, (Option<usize>, String)> = BTreeMap::new();
        for (idx, tok) in tokens.iter().enumerate() {
            if Instant::now() >= deadline {
                timed_out = true;
                break;
            }
            let mut tcmd = Command::new("rg");
            tcmd.args([
                "--line-number",
                "--no-heading",
                "--no-messages",
                "--color",
                "never",
                "--smart-case",
                "--fixed-strings",
                "--max-count",
                "1",
                "--max-filesize",
                "20M",
            ]);
            append_ignore_globs(&mut tcmd, ignore_patterns);
            tcmd.arg("--").arg(tok);
            for r in roots {
                tcmd.arg(r);
            }
            let (trows, to_t) = collect_rg_stream(tcmd, (limit * 6).max(60), deadline);
            timed_out |= to_t;
            let mut cur_set = HashSet::new();
            for (p, ln, txt) in trows {
                if is_sensitive_file(Path::new(&p)) {
                    continue;
                }
                if idx == 0 {
                    first_snippets
                        .entry(p.clone())
                        .or_insert((ln, truncate_line(&txt, 140)));
                }
                cur_set.insert(p);
            }
            intersection = Some(match intersection {
                None => cur_set,
                Some(prev) => prev.intersection(&cur_set).cloned().collect(),
            });
        }
        if let Some(both) = intersection {
            for p in both {
                if !by_path.contains_key(&p) {
                    let (ln, snip) = first_snippets
                        .get(&p)
                        .cloned()
                        .unwrap_or((None, String::new()));
                    by_path.insert(
                        p.clone(),
                        LiveSearchHit {
                            path: p,
                            line_number: ln,
                            snippet: snip,
                            match_score: 2,
                            match_kind: "multi_token".to_string(),
                        },
                    );
                }
            }
        }
    }

    // Pass 3: filename/path substring matches (same as find/rg --files)
    if by_path.len() < limit && Instant::now() < deadline {
        for p in collect_rg_filename_hits(roots, ignore_patterns, &q_lower, limit * 2, deadline) {
            by_path.entry(p.clone()).or_insert(LiveSearchHit {
                path: p,
                line_number: None,
                snippet: "[filename match]".to_string(),
                match_score: 3,
                match_kind: "path".to_string(),
            });
        }
    }

    let mut out: Vec<LiveSearchHit> = by_path.into_values().collect();
    out.sort_by(|a, b| {
        b.match_score
            .cmp(&a.match_score)
            .then_with(|| a.path.cmp(&b.path))
    });
    out.truncate(limit);
    (out, timed_out)
}

fn run_grep_baseline(
    roots: &[PathBuf],
    ignore_patterns: &[String],
    query: &str,
    limit: usize,
    deadline: Instant,
) -> (Vec<LiveSearchHit>, bool) {
    let mut cmd = Command::new("grep");
    cmd.args(["-rnIFm", "1"]);
    for pat in ignore_patterns {
        let p = pat.trim().trim_matches('/');
        if !p.is_empty() {
            cmd.arg(format!("--exclude-dir={}", p));
        }
    }
    for exc in ["*.env*", "*.pem", "*.key", "*.p12", "*.pfx"] {
        cmd.arg(format!("--exclude={}", exc));
    }
    cmd.arg("--").arg(query.trim());
    for r in roots {
        cmd.arg(r);
    }
    let (rows, timed_out) = collect_rg_stream(cmd, limit * 3, deadline);
    let mut seen = HashSet::new();
    let mut hits = Vec::new();
    for (p, ln, text) in rows {
        if is_sensitive_file(Path::new(&p)) || !seen.insert(p.clone()) {
            continue;
        }
        hits.push(LiveSearchHit {
            path: p,
            line_number: ln,
            snippet: truncate_line(&text, 140),
            match_score: 4,
            match_kind: "content".to_string(),
        });
        if hits.len() >= limit {
            break;
        }
    }
    (hits, timed_out)
}

fn run_walkdir_full_baseline(
    roots: &[PathBuf],
    ignore_patterns: &[String],
    query: &str,
    limit: usize,
    deadline: Instant,
) -> (Vec<LiveSearchHit>, usize, bool) {
    let q_trim = query.trim();
    let q_lower = q_trim.to_ascii_lowercase();
    let tokens: Vec<String> = q_lower
        .split_whitespace()
        .filter(|t| t.len() >= 2)
        .map(str::to_string)
        .collect();
    let search_terms = if tokens.is_empty() {
        vec![q_lower.clone()]
    } else {
        tokens
    };

    let mut candidates = Vec::new();
    let mut scanned_files = 0usize;
    let mut timed_out = false;

    'outer: for root in roots {
        if !root.exists() {
            continue;
        }
        let mut it = WalkDir::new(root).into_iter();
        while let Some(entry_res) = it.next() {
            if Instant::now() >= deadline {
                timed_out = true;
                break 'outer;
            }
            let Ok(entry) = entry_res else {
                continue;
            };
            let p = entry.path();
            if entry.file_type().is_dir() {
                if ignore_patterns.iter().any(|pat| path_matches_ignore_pattern(p, pat)) {
                    it.skip_current_dir();
                }
                continue;
            }
            if !entry.file_type().is_file() || is_sensitive_file(p) {
                continue;
            }
            scanned_files += 1;
            let path_str = p.to_string_lossy().to_string();
            let path_hit = path_str.to_ascii_lowercase().contains(&q_lower);
            let mut score = usize::from(path_hit) * 3;
            let mut matched_ln = None;
            let mut snippet = String::new();

            if let Ok(bytes) = std::fs::read(p) {
                if bytes.len() <= 5 * 1024 * 1024 && !bytes[..bytes.len().min(2048)].contains(&0) {
                    let body = String::from_utf8_lossy(&bytes);
                    let body_lower = body.to_ascii_lowercase();
                    if body_lower.contains(&q_lower) {
                        score += 4;
                        if let Some((idx, line)) = body
                            .lines()
                            .enumerate()
                            .find(|(_, l)| l.to_ascii_lowercase().contains(&q_lower))
                        {
                            matched_ln = Some(idx + 1);
                            snippet = truncate_line(line, 140);
                        }
                    } else if search_terms.len() > 1
                        && search_terms.iter().all(|t| body_lower.contains(t.as_str()))
                    {
                        score += 2;
                        if let Some((idx, line)) = body
                            .lines()
                            .enumerate()
                            .find(|(_, l)| l.to_ascii_lowercase().contains(&search_terms[0]))
                        {
                            matched_ln = Some(idx + 1);
                            snippet = truncate_line(line, 140);
                        }
                    }
                }
            }

            if score > 0 {
                candidates.push(LiveSearchHit {
                    path: path_str,
                    line_number: matched_ln,
                    snippet,
                    match_score: score,
                    match_kind: if matched_ln.is_some() {
                        "content".to_string()
                    } else {
                        "path".to_string()
                    },
                });
            }
        }
    }

    candidates.sort_by(|a, b| {
        b.match_score
            .cmp(&a.match_score)
            .then_with(|| a.path.cmp(&b.path))
    });
    candidates.truncate(limit);
    (candidates, scanned_files, timed_out)
}

/// 运行模拟“完全不用 agent-body”的真实工作区模糊全文检索基线 (`rg` -> `grep` -> `walkdir_full`)
pub fn run_baseline_workspace_search(
    roots: &[PathBuf],
    ignore_patterns: &[String],
    query: &str,
    limit: usize,
    max_ms: u64,
) -> LiveSearchRun {
    let t0 = Instant::now();
    let valid_roots: Vec<PathBuf> = roots.iter().filter(|r| r.exists()).cloned().collect();
    if query.trim().is_empty() || limit == 0 || valid_roots.is_empty() {
        return LiveSearchRun {
            engine: "ripgrep".to_string(),
            hits: Vec::new(),
            scanned_files: 0,
            elapsed_us: t0.elapsed().as_micros() as i64,
            tokens: 0,
            timed_out: false,
        };
    }

    let deadline = t0 + Duration::from_millis(max_ms.max(100));
    let (engine, hits, scanned_files, timed_out) = if tool_available("rg") {
        let (h, to) = run_ripgrep_baseline(&valid_roots, ignore_patterns, query, limit, deadline);
        ("ripgrep".to_string(), h, 0, to)
    } else if tool_available("grep") {
        let (h, to) = run_grep_baseline(&valid_roots, ignore_patterns, query, limit, deadline);
        ("grep".to_string(), h, 0, to)
    } else {
        let (h, sf, to) =
            run_walkdir_full_baseline(&valid_roots, ignore_patterns, query, limit, deadline);
        ("walkdir_full".to_string(), h, sf, to)
    };

    let rendered = format_live_search_hits(&hits, query);
    let tokens = if hits.is_empty() {
        0
    } else {
        estimate_tokens(&rendered)
    };
    LiveSearchRun {
        engine,
        hits,
        scanned_files,
        elapsed_us: t0.elapsed().as_micros() as i64,
        tokens,
        timed_out,
    }
}

pub fn format_live_search_hits(hits: &[LiveSearchHit], query: &str) -> String {
    let mut out = format!(
        "原生实时工作区搜索结果 (query='{}', 共 {} 条):\n",
        query,
        hits.len()
    );
    for h in hits {
        let ln_tag = h
            .line_number
            .map(|n| format!(":{}", n))
            .unwrap_or_default();
        if h.snippet.is_empty() {
            out.push_str(&format!("  - {}{} (score={})\n", h.path, ln_tag, h.match_score));
        } else {
            out.push_str(&format!(
                "  - {}{} (score={}) | {}\n",
                h.path, ln_tag, h.match_score, h.snippet
            ));
        }
    }
    out
}
