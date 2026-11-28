//! 会话级读账本 (`Session Read Ledger`)
//!
//! 基于 Append-Only JSONL (`~/.agent-body/tmp/read_ledger/<conv_id>.jsonl`) 追踪同会话内
//! 已读文件的行区间与文件版本 (`mtime + size`)，在零 SQLite 打开的前提下：
//! 1. 拦截重复完整读或被既有上下文完全覆盖的子集读（支持 3 步内 Repeat-Confirm 自动放行）；
//! 2. 对部分重叠的翻页读取自动增量裁剪，保留 5 行边界衔接上下文；
//! 3. 联动 `PostToolUse(write_file|edit_file)` 与 `transcript.jsonl` `CHECKPOINT` 自动失效/清空，
//!    防止 auto-compact 后上下文丢失却被误拦。

use std::collections::HashSet;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::hooks::outline::{extract_file_outline, format_outline_compact};
use crate::hooks::read_ledger_helpers::{
    base_name, build_unread_symbol_nav, compute_overlap_trim, extract_read_path, format_intervals,
    has_byte_offset, is_binary_ext, is_fully_covered, is_read_ledger_env_disabled, merge_intervals,
    meta_version, normalize_path, prune_old_ledgers, resolve_effective_range, sanitize_conv_id,
    scan_transcript_for_checkpoint,
};
use crate::hooks::token_guard::TokenGuardPolicy;

const MAX_SESSION_FILES: usize = 100;
const TRANSCRIPT_CURSOR_STEP_BYTES: u64 = 32 * 1024;
const REPEAT_CONFIRM_STEP_WINDOW: u64 = 3;

/// 单条会话读账本事件（Append-Only JSONL）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum LedgerEvent {
    Compact {
        epoch: u64,
        transcript_offset: u64,
        step: u64,
    },
    Cursor {
        epoch: u64,
        transcript_offset: u64,
    },
    Read {
        path: String,
        start: usize,
        end: usize,
        total_lines: usize,
        mtime_secs: u64,
        mtime_nanos: u32,
        size: u64,
        step: u64,
        epoch: u64,
    },
    Invalidate {
        path: String,
        step: u64,
        epoch: u64,
    },
    Deny {
        path: String,
        start: usize,
        end: usize,
        mtime_secs: u64,
        mtime_nanos: u32,
        size: u64,
        step: u64,
        epoch: u64,
    },
    OutlineSent {
        path: String,
        mtime_secs: u64,
        mtime_nanos: u32,
        size: u64,
        epoch: u64,
    },
}

/// 预读评估输入参数
pub struct ReadLedgerRequest<'a> {
    pub conversation_id: &'a str,
    pub transcript_path: Option<&'a str>,
    pub cwd: Option<&'a str>,
    pub step_idx: u64,
    pub args: &'a Value,
    pub upstream_overwrite: Option<&'a Value>,
    pub policy: &'a TokenGuardPolicy,
}

/// 预读账本判定结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadLedgerDecision {
    Pass,
    AllowRecord {
        effective_overwrite: Option<Value>,
        outline_hint: Option<String>,
        range: (usize, usize),
    },
    TrimOverlap {
        overwrite: Value,
        saved_lines: usize,
        orig_range: (usize, usize),
        trimmed_range: (usize, usize),
    },
    DenyDuplicate {
        reason: String,
        saved_lines: usize,
        covered_summary: String,
    },
}

/// CLI / 诊断用的单文件覆盖摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReadCoverageSummary {
    pub path: String,
    pub total_lines: usize,
    pub covered_ranges: Vec<(usize, usize)>,
    pub last_step: u64,
}

/// CLI / 诊断用的会话读账本快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLedgerSummary {
    pub conversation_id: String,
    pub ledger_path: String,
    pub epoch: u64,
    pub transcript_offset: u64,
    pub total_reads: usize,
    pub total_denies: usize,
    pub estimated_saved_lines: usize,
    pub files: Vec<FileReadCoverageSummary>,
}

#[derive(Debug, Clone)]
struct ReadRecord {
    path: String,
    start: usize,
    end: usize,
    total_lines: usize,
    mtime_secs: u64,
    mtime_nanos: u32,
    size: u64,
    step: u64,
    epoch: u64,
}

#[derive(Debug, Clone)]
struct DenyRecord {
    path: String,
    start: usize,
    end: usize,
    mtime_secs: u64,
    mtime_nanos: u32,
    size: u64,
    step: u64,
    epoch: u64,
}

#[derive(Debug, Default)]
struct LedgerState {
    epoch: u64,
    transcript_offset: u64,
    last_step: u64,
    reads: Vec<ReadRecord>,
    denies: Vec<DenyRecord>,
    outlines_sent: HashSet<(String, u64, u32, u64)>,
    total_reads: usize,
    total_denies: usize,
    estimated_saved_lines: usize,
}

/// 会话级读账本管理器（支持自定义测试根目录）
#[derive(Debug, Clone)]
pub struct ReadLedgerStore {
    base_dir: PathBuf,
}

impl ReadLedgerStore {
    /// 构造宿主机默认目录 (`~/.agent-body/tmp/read_ledger`)
    pub fn default_host() -> Self {
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .unwrap_or_else(|_| "/tmp".to_string());
        Self {
            base_dir: PathBuf::from(home)
                .join(".agent-body")
                .join("tmp")
                .join("read_ledger"),
        }
    }

    /// 指定自定义存储目录（测试隔离用）
    pub fn with_dir<P: Into<PathBuf>>(dir: P) -> Self {
        Self {
            base_dir: dir.into(),
        }
    }

    /// 对应会话的 JSONL 账本路径
    pub fn ledger_path(&self, conversation_id: &str) -> PathBuf {
        self.base_dir
            .join(format!("{}.jsonl", sanitize_conv_id(conversation_id)))
    }

    /// 评估 `read_file` 是否重复读/部分重叠/需要注入首屏结构骨架
    pub fn evaluate_pre_read(&self, req: &ReadLedgerRequest<'_>) -> ReadLedgerDecision {
        if has_byte_offset(req.args) {
            return ReadLedgerDecision::Pass;
        }
        let Some(raw_path) = extract_read_path(req.args) else {
            return ReadLedgerDecision::Pass;
        };
        let norm_path = normalize_path(raw_path, req.cwd);
        let path_obj = Path::new(&norm_path);
        if is_binary_ext(path_obj) || !path_obj.is_file() {
            return ReadLedgerDecision::Pass;
        }

        let Ok(meta) = fs::metadata(path_obj) else {
            return ReadLedgerDecision::Pass;
        };
        let (mtime_secs, mtime_nanos, size) = meta_version(&meta);

        let ledger_disabled = !req.policy.read_ledger_enabled
            || is_read_ledger_env_disabled()
            || req.conversation_id.trim().is_empty();

        if ledger_disabled {
            return self.handle_outline_only_if_needed(
                path_obj,
                req.upstream_overwrite,
                req.policy,
            );
        }

        let ledger_path = self.ledger_path(req.conversation_id);
        let mut state = load_state(&ledger_path);
        self.sync_compact_from_transcript(&ledger_path, &mut state, req.transcript_path, req.step_idx);

        let Some((req_start, req_end, total_lines)) =
            resolve_effective_range(path_obj, req.args, req.upstream_overwrite)
        else {
            return ReadLedgerDecision::Pass;
        };

        let matching: Vec<&ReadRecord> = state
            .reads
            .iter()
            .filter(|r| {
                r.epoch == state.epoch
                    && r.path == norm_path
                    && r.mtime_secs == mtime_secs
                    && r.mtime_nanos == mtime_nanos
                    && r.size == size
                    && req.step_idx.saturating_sub(r.step) <= req.policy.read_ledger_window_steps as u64
            })
            .collect();

        let last_step = matching.iter().map(|r| r.step).max().unwrap_or(0);
        let intervals: Vec<(usize, usize)> = matching.iter().map(|r| (r.start, r.end)).collect();
        let covered = merge_intervals(&intervals);

        let recently_denied = state.denies.iter().any(|d| {
            d.epoch == state.epoch
                && d.path == norm_path
                && d.start == req_start
                && d.end == req_end
                && d.mtime_secs == mtime_secs
                && d.mtime_nanos == mtime_nanos
                && d.size == size
                && req.step_idx.saturating_sub(d.step) <= REPEAT_CONFIRM_STEP_WINDOW
        });

        if !recently_denied && is_fully_covered(&covered, req_start, req_end) {
            let saved_lines = req_end.saturating_sub(req_start) + 1;
            let covered_summary = format_intervals(&covered);
            self.append_event(
                &ledger_path,
                &LedgerEvent::Deny {
                    path: norm_path.clone(),
                    start: req_start,
                    end: req_end,
                    mtime_secs,
                    mtime_nanos,
                    size,
                    step: req.step_idx,
                    epoch: state.epoch,
                },
            );
            let mut reason = format!(
                "[agent-body ReadLedger] 文件 {} L{}~{} 已于第 {} 步读取且内容未变更（已读区间: {}）。请直接复用上下文中的内容；若确需重读原文，立即再次调用相同参数将自动放行。",
                base_name(&norm_path),
                req_start,
                req_end,
                last_step,
                covered_summary
            );
            if req.policy.outline_enabled && total_lines > 250 {
                if let Some(unread_nav) =
                    build_unread_symbol_nav(path_obj, &covered, 10, 600)
                {
                    reason.push('\n');
                    reason.push_str(&unread_nav);
                }
            }
            return ReadLedgerDecision::DenyDuplicate {
                reason,
                saved_lines,
                covered_summary,
            };
        }

        if !recently_denied && req.upstream_overwrite.is_none() {
            if let Some((trim_start, trim_end, saved_lines)) =
                compute_overlap_trim(&covered, req_start, req_end)
            {
                self.append_event(
                    &ledger_path,
                    &LedgerEvent::Read {
                        path: norm_path,
                        start: trim_start,
                        end: trim_end,
                        total_lines,
                        mtime_secs,
                        mtime_nanos,
                        size,
                        step: req.step_idx,
                        epoch: state.epoch,
                    },
                );
                return ReadLedgerDecision::TrimOverlap {
                    overwrite: json!({
                        "StartLine": trim_start,
                        "EndLine": trim_end,
                    }),
                    saved_lines,
                    orig_range: (req_start, req_end),
                    trimmed_range: (trim_start, trim_end),
                };
            }
        }

        self.append_event(
            &ledger_path,
            &LedgerEvent::Read {
                path: norm_path.clone(),
                start: req_start,
                end: req_end,
                total_lines,
                mtime_secs,
                mtime_nanos,
                size,
                step: req.step_idx,
                epoch: state.epoch,
            },
        );

        let outline_key = (norm_path.clone(), mtime_secs, mtime_nanos, size);
        let outline_hint = if req.policy.outline_enabled
            && req.upstream_overwrite.is_some()
            && !state.outlines_sent.contains(&outline_key)
        {
            if let Some(hint) = extract_file_outline(path_obj, 80)
                .ok()
                .and_then(|o| format_outline_compact(&o, Some(req_end), req.policy.outline_max_bytes))
            {
                self.append_event(
                    &ledger_path,
                    &LedgerEvent::OutlineSent {
                        path: norm_path,
                        mtime_secs,
                        mtime_nanos,
                        size,
                        epoch: state.epoch,
                    },
                );
                Some(hint)
            } else {
                None
            }
        } else {
            None
        };

        ReadLedgerDecision::AllowRecord {
            effective_overwrite: req.upstream_overwrite.cloned(),
            outline_hint,
            range: (req_start, req_end),
        }
    }

    /// 文件写入/编辑后使对应路径读账本失效
    pub fn invalidate_path(
        &self,
        conversation_id: &str,
        raw_path: &str,
        cwd: Option<&str>,
        step_idx: u64,
    ) {
        if conversation_id.trim().is_empty() || raw_path.trim().is_empty() {
            return;
        }
        let ledger_path = self.ledger_path(conversation_id);
        if !ledger_path.exists() {
            return;
        }
        let state = load_state(&ledger_path);
        let norm_path = normalize_path(raw_path, cwd);
        self.append_event(
            &ledger_path,
            &LedgerEvent::Invalidate {
                path: norm_path,
                step: step_idx,
                epoch: state.epoch,
            },
        );
    }

    /// 查看会话读账本统计快照
    pub fn inspect_session(&self, conversation_id: &str) -> SessionLedgerSummary {
        let ledger_path = self.ledger_path(conversation_id);
        let state = load_state(&ledger_path);
        let mut by_file: std::collections::BTreeMap<String, (usize, Vec<(usize, usize)>, u64)> =
            std::collections::BTreeMap::new();
        for r in &state.reads {
            if r.epoch != state.epoch {
                continue;
            }
            let entry = by_file
                .entry(r.path.clone())
                .or_insert_with(|| (r.total_lines, Vec::new(), 0));
            entry.0 = entry.0.max(r.total_lines);
            entry.1.push((r.start, r.end));
            entry.2 = entry.2.max(r.step);
        }
        let files = by_file
            .into_iter()
            .map(|(path, (total_lines, raw_ranges, last_step))| FileReadCoverageSummary {
                path,
                total_lines,
                covered_ranges: merge_intervals(&raw_ranges),
                last_step,
            })
            .collect();

        SessionLedgerSummary {
            conversation_id: conversation_id.to_string(),
            ledger_path: ledger_path.to_string_lossy().to_string(),
            epoch: state.epoch,
            transcript_offset: state.transcript_offset,
            total_reads: state.total_reads,
            total_denies: state.total_denies,
            estimated_saved_lines: state.estimated_saved_lines,
            files,
        }
    }

    /// 清空指定会话的读账本文件
    pub fn clear_session(&self, conversation_id: &str) -> std::io::Result<bool> {
        let p = self.ledger_path(conversation_id);
        if p.exists() {
            fs::remove_file(&p)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn handle_outline_only_if_needed(
        &self,
        path_obj: &Path,
        upstream_overwrite: Option<&Value>,
        policy: &TokenGuardPolicy,
    ) -> ReadLedgerDecision {
        if !policy.outline_enabled {
            return ReadLedgerDecision::Pass;
        }
        let Some(ow) = upstream_overwrite else {
            return ReadLedgerDecision::Pass;
        };
        let start = ow
            .get("StartLine")
            .and_then(|v| v.as_i64())
            .unwrap_or(1)
            .max(1) as usize;
        let end = ow
            .get("EndLine")
            .and_then(|v| v.as_i64())
            .unwrap_or(policy.auto_slice_lines as i64)
            .max(start as i64) as usize;
        let outline_hint = extract_file_outline(path_obj, 80)
            .ok()
            .and_then(|o| format_outline_compact(&o, Some(end), policy.outline_max_bytes));
        ReadLedgerDecision::AllowRecord {
            effective_overwrite: Some(ow.clone()),
            outline_hint,
            range: (start, end),
        }
    }

    fn sync_compact_from_transcript(
        &self,
        ledger_path: &Path,
        state: &mut LedgerState,
        transcript_path: Option<&str>,
        step_idx: u64,
    ) {
        // 仅当发生大幅步号归零重置（如 >=10 退回 <=3）时才无 transcript 触发 compact，避免并行工具乱序误清空
        if state.last_step >= 10 && step_idx > 0 && step_idx <= 3 {
            state.epoch += 1;
            clear_epoch_state(state);
            self.append_event(
                ledger_path,
                &LedgerEvent::Compact {
                    epoch: state.epoch,
                    transcript_offset: state.transcript_offset,
                    step: step_idx,
                },
            );
        }

        let Some(tp_raw) = transcript_path.map(str::trim).filter(|s| !s.is_empty()) else {
            return;
        };
        let tp = Path::new(tp_raw);
        let Ok(meta) = fs::metadata(tp) else {
            return;
        };
        let len = meta.len();
        if len < state.transcript_offset {
            state.epoch += 1;
            state.transcript_offset = 0;
            clear_epoch_state(state);
            self.append_event(
                ledger_path,
                &LedgerEvent::Compact {
                    epoch: state.epoch,
                    transcript_offset: 0,
                    step: step_idx,
                },
            );
            return;
        }
        if len == state.transcript_offset {
            return;
        }

        let (found_checkpoint, new_offset) = scan_transcript_for_checkpoint(tp, state.transcript_offset, len);
        if found_checkpoint {
            state.epoch += 1;
            state.transcript_offset = new_offset;
            clear_epoch_state(state);
            self.append_event(
                ledger_path,
                &LedgerEvent::Compact {
                    epoch: state.epoch,
                    transcript_offset: new_offset,
                    step: step_idx,
                },
            );
        } else if new_offset.saturating_sub(state.transcript_offset) >= TRANSCRIPT_CURSOR_STEP_BYTES {
            state.transcript_offset = new_offset;
            self.append_event(
                ledger_path,
                &LedgerEvent::Cursor {
                    epoch: state.epoch,
                    transcript_offset: new_offset,
                },
            );
        }
    }

    fn append_event(&self, ledger_path: &Path, ev: &LedgerEvent) {
        let is_new = !ledger_path.exists();
        if let Some(parent) = ledger_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                eprintln!("[ReadLedger][WARN] 创建账本目录失败 ({:?}): {}", parent, e);
                return;
            }
            if is_new {
                prune_old_ledgers(parent, MAX_SESSION_FILES);
            }
        }
        let Ok(mut line) = serde_json::to_string(ev) else {
            return;
        };
        line.push('\n');
        match OpenOptions::new().create(true).append(true).open(ledger_path) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(line.as_bytes()) {
                    eprintln!("[ReadLedger][WARN] 写入账本失败 ({:?}): {}", ledger_path, e);
                }
            }
            Err(e) => {
                eprintln!("[ReadLedger][WARN] 打开账本失败 ({:?}): {}", ledger_path, e);
            }
        }
    }
}

fn clear_epoch_state(state: &mut LedgerState) {
    state.reads.clear();
    state.denies.clear();
    state.outlines_sent.clear();
    state.last_step = 0;
}

fn load_state(ledger_path: &Path) -> LedgerState {
    let mut state = LedgerState::default();
    let Ok(f) = File::open(ledger_path) else {
        return state;
    };
    let reader = BufReader::new(f);
    for line in reader.lines().map_while(Result::ok) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(ev) = serde_json::from_str::<LedgerEvent>(trimmed) else {
            continue;
        };
        match ev {
            LedgerEvent::Compact {
                epoch,
                transcript_offset,
                ..
            } => {
                state.epoch = epoch;
                state.transcript_offset = transcript_offset;
                clear_epoch_state(&mut state);
            }
            LedgerEvent::Cursor {
                epoch,
                transcript_offset,
            } => {
                if epoch == state.epoch {
                    state.transcript_offset = state.transcript_offset.max(transcript_offset);
                }
            }
            LedgerEvent::Read {
                path,
                start,
                end,
                total_lines,
                mtime_secs,
                mtime_nanos,
                size,
                step,
                epoch,
            } => {
                if epoch == state.epoch {
                    state.last_step = state.last_step.max(step);
                    state.total_reads += 1;
                    state.reads.push(ReadRecord {
                        path,
                        start,
                        end,
                        total_lines,
                        mtime_secs,
                        mtime_nanos,
                        size,
                        step,
                        epoch,
                    });
                }
            }
            LedgerEvent::Invalidate { path, epoch, .. } => {
                if epoch == state.epoch {
                    state.reads.retain(|r| r.path != path);
                    state.denies.retain(|d| d.path != path);
                    state.outlines_sent.retain(|(p, _, _, _)| p != &path);
                }
            }
            LedgerEvent::Deny {
                path,
                start,
                end,
                mtime_secs,
                mtime_nanos,
                size,
                step,
                epoch,
            } => {
                if epoch == state.epoch {
                    state.last_step = state.last_step.max(step);
                    state.total_denies += 1;
                    state.estimated_saved_lines += end.saturating_sub(start) + 1;
                    state.denies.push(DenyRecord {
                        path,
                        start,
                        end,
                        mtime_secs,
                        mtime_nanos,
                        size,
                        step,
                        epoch,
                    });
                }
            }
            LedgerEvent::OutlineSent {
                path,
                mtime_secs,
                mtime_nanos,
                size,
                epoch,
            } => {
                if epoch == state.epoch {
                    state.outlines_sent.insert((path, mtime_secs, mtime_nanos, size));
                }
            }
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_sample_file(dir: &Path, name: &str, total_lines: usize) -> PathBuf {
        let p = dir.join(name);
        let mut content = String::new();
        for i in 1..=total_lines {
            if i % 50 == 1 {
                content.push_str(&format!("pub fn func_{}() {{}}\n", i));
            } else {
                content.push_str(&format!("// line {}\n", i));
            }
        }
        fs::write(&p, content).expect("write sample");
        p
    }

    #[test]
    fn test_read_ledger_duplicate_subset_and_repeat_confirm() {
        let tmp = TempDir::new().expect("tempdir");
        let store = ReadLedgerStore::with_dir(tmp.path().join("ledger"));
        let file = make_sample_file(tmp.path(), "mod.rs", 300);
        let policy = TokenGuardPolicy::default();
        let conv = "conv-dup-1";

        let args1 = json!({ "Path": file, "StartLine": 1, "EndLine": 120 });
        let d1 = store.evaluate_pre_read(&ReadLedgerRequest {
            conversation_id: conv,
            transcript_path: None,
            cwd: None,
            step_idx: 1,
            args: &args1,
            upstream_overwrite: None,
            policy: &policy,
        });
        assert!(matches!(d1, ReadLedgerDecision::AllowRecord { .. }));

        // 完全重复拦截
        let d2 = store.evaluate_pre_read(&ReadLedgerRequest {
            conversation_id: conv,
            transcript_path: None,
            cwd: None,
            step_idx: 2,
            args: &args1,
            upstream_overwrite: None,
            policy: &policy,
        });
        assert!(matches!(d2, ReadLedgerDecision::DenyDuplicate { .. }));

        // Repeat-Confirm 二次立即放行
        let d3 = store.evaluate_pre_read(&ReadLedgerRequest {
            conversation_id: conv,
            transcript_path: None,
            cwd: None,
            step_idx: 3,
            args: &args1,
            upstream_overwrite: None,
            policy: &policy,
        });
        assert!(matches!(d3, ReadLedgerDecision::AllowRecord { .. }));

        // 子集拦截 (L20~80 被 L1~120 覆盖)
        let args_sub = json!({ "Path": file, "StartLine": 20, "EndLine": 80 });
        let d4 = store.evaluate_pre_read(&ReadLedgerRequest {
            conversation_id: conv,
            transcript_path: None,
            cwd: None,
            step_idx: 4,
            args: &args_sub,
            upstream_overwrite: None,
            policy: &policy,
        });
        assert!(matches!(d4, ReadLedgerDecision::DenyDuplicate { .. }));
    }

    #[test]
    fn test_read_ledger_interval_merge_and_overlap_trim() {
        let tmp = TempDir::new().expect("tempdir");
        let store = ReadLedgerStore::with_dir(tmp.path().join("ledger"));
        let file = make_sample_file(tmp.path(), "big.rs", 400);
        let policy = TokenGuardPolicy::default();
        let conv = "conv-merge-1";

        let a1 = json!({ "Path": file, "StartLine": 1, "EndLine": 100 });
        let a2 = json!({ "Path": file, "StartLine": 101, "EndLine": 200 });
        assert!(matches!(
            store.evaluate_pre_read(&ReadLedgerRequest {
                conversation_id: conv,
                transcript_path: None,
                cwd: None,
                step_idx: 1,
                args: &a1,
                upstream_overwrite: None,
                policy: &policy,
            }),
            ReadLedgerDecision::AllowRecord { .. }
        ));
        assert!(matches!(
            store.evaluate_pre_read(&ReadLedgerRequest {
                conversation_id: conv,
                transcript_path: None,
                cwd: None,
                step_idx: 2,
                args: &a2,
                upstream_overwrite: None,
                policy: &policy,
            }),
            ReadLedgerDecision::AllowRecord { .. }
        ));

        // 归并后 1~200 完整覆盖 50~150
        let a_span = json!({ "Path": file, "StartLine": 50, "EndLine": 150 });
        assert!(matches!(
            store.evaluate_pre_read(&ReadLedgerRequest {
                conversation_id: conv,
                transcript_path: None,
                cwd: None,
                step_idx: 3,
                args: &a_span,
                upstream_overwrite: None,
                policy: &policy,
            }),
            ReadLedgerDecision::DenyDuplicate { .. }
        ));

        // 盲翻页 120~280 与已读 1~200 重叠 81 行，应裁剪为 196~280
        let a_overlap = json!({ "Path": file, "StartLine": 120, "EndLine": 280 });
        let trimmed = store.evaluate_pre_read(&ReadLedgerRequest {
            conversation_id: conv,
            transcript_path: None,
            cwd: None,
            step_idx: 4,
            args: &a_overlap,
            upstream_overwrite: None,
            policy: &policy,
        });
        match trimmed {
            ReadLedgerDecision::TrimOverlap {
                overwrite,
                trimmed_range,
                saved_lines,
                ..
            } => {
                assert_eq!(trimmed_range, (196, 280));
                assert_eq!(overwrite["StartLine"], 196);
                assert_eq!(overwrite["EndLine"], 280);
                assert!(saved_lines >= 70);
            }
            other => panic!("expected TrimOverlap, got {:?}", other),
        }
    }

    #[test]
    fn test_read_ledger_invalidation_window_and_compact_checkpoint() {
        let tmp = TempDir::new().expect("tempdir");
        let store = ReadLedgerStore::with_dir(tmp.path().join("ledger"));
        let file = make_sample_file(tmp.path(), "edit.rs", 100);
        let transcript = tmp.path().join("transcript.jsonl");
        fs::write(&transcript, "{\"type\":\"USER_INPUT\",\"content\":\"contains CHECKPOINT word\"}\n")
            .expect("write t1");

        let mut policy = TokenGuardPolicy::default();
        policy.read_ledger_window_steps = 10;
        let conv = "conv-inv-1";
        let args = json!({ "Path": file, "StartLine": 1, "EndLine": 60 });

        assert!(matches!(
            store.evaluate_pre_read(&ReadLedgerRequest {
                conversation_id: conv,
                transcript_path: transcript.to_str(),
                cwd: None,
                step_idx: 1,
                args: &args,
                upstream_overwrite: None,
                policy: &policy,
            }),
            ReadLedgerDecision::AllowRecord { .. }
        ));

        // 含 CHECKPOINT 字面量但 type!=CHECKPOINT 不应误触发 compact 清空
        assert!(matches!(
            store.evaluate_pre_read(&ReadLedgerRequest {
                conversation_id: conv,
                transcript_path: transcript.to_str(),
                cwd: None,
                step_idx: 2,
                args: &args,
                upstream_overwrite: None,
                policy: &policy,
            }),
            ReadLedgerDecision::DenyDuplicate { .. }
        ));

        // 写入真实 CHECKPOINT 后应清零 epoch 并放行
        let mut tf = OpenOptions::new().append(true).open(&transcript).expect("open");
        writeln!(tf, "{{\"type\":\"CHECKPOINT\",\"step_index\":3}}").expect("append");
        assert!(matches!(
            store.evaluate_pre_read(&ReadLedgerRequest {
                conversation_id: conv,
                transcript_path: transcript.to_str(),
                cwd: None,
                step_idx: 4,
                args: &args,
                upstream_overwrite: None,
                policy: &policy,
            }),
            ReadLedgerDecision::AllowRecord { .. }
        ));

        // PostToolUse write 失效后放行
        store.invalidate_path(conv, file.to_str().unwrap_or(""), None, 5);
        assert!(matches!(
            store.evaluate_pre_read(&ReadLedgerRequest {
                conversation_id: conv,
                transcript_path: transcript.to_str(),
                cwd: None,
                step_idx: 6,
                args: &args,
                upstream_overwrite: None,
                policy: &policy,
            }),
            ReadLedgerDecision::AllowRecord { .. }
        ));

        // 超过滑窗 (step 6 -> step 20, window=10) 后放行
        assert!(matches!(
            store.evaluate_pre_read(&ReadLedgerRequest {
                conversation_id: conv,
                transcript_path: transcript.to_str(),
                cwd: None,
                step_idx: 20,
                args: &args,
                upstream_overwrite: None,
                policy: &policy,
            }),
            ReadLedgerDecision::AllowRecord { .. }
        ));

        // 磁盘内容变化 (size/mtime 变更) 后自动放行
        fs::write(&file, "// modified file\npub fn changed() {}\n").expect("rewrite");
        assert!(matches!(
            store.evaluate_pre_read(&ReadLedgerRequest {
                conversation_id: conv,
                transcript_path: transcript.to_str(),
                cwd: None,
                step_idx: 21,
                args: &json!({ "Path": file, "StartLine": 1, "EndLine": 2 }),
                upstream_overwrite: None,
                policy: &policy,
            }),
            ReadLedgerDecision::AllowRecord { .. }
        ));
    }

    #[test]
    fn test_read_ledger_concurrent_append_integrity() {
        let tmp = TempDir::new().expect("tempdir");
        let ledger_dir = tmp.path().join("ledger");
        let policy = TokenGuardPolicy::default();
        let conv = "conv-concurrent";

        let files: Vec<PathBuf> = (0..6)
            .map(|i| make_sample_file(tmp.path(), &format!("f_{}.rs", i), 80))
            .collect();

        std::thread::scope(|s| {
            for (idx, f) in files.iter().enumerate() {
                let d = ledger_dir.clone();
                let pol = policy.clone();
                s.spawn(move || {
                    let st = ReadLedgerStore::with_dir(d);
                    let args = json!({ "Path": f, "StartLine": 1, "EndLine": 40 });
                    let res = st.evaluate_pre_read(&ReadLedgerRequest {
                        conversation_id: conv,
                        transcript_path: None,
                        cwd: None,
                        step_idx: (idx + 1) as u64,
                        args: &args,
                        upstream_overwrite: None,
                        policy: &pol,
                    });
                    assert!(matches!(res, ReadLedgerDecision::AllowRecord { .. }));
                });
            }
        });

        let summary = ReadLedgerStore::with_dir(ledger_dir).inspect_session(conv);
        assert_eq!(summary.total_reads, 6);
        assert_eq!(summary.files.len(), 6);
    }
}
