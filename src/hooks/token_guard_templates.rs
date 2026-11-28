//! Antigravity 客户端 Token Optimizer Hook 与全局 `.ripgreprc` 模板

pub(crate) const RIPGREPRC_TEMPLATE: &str = r#"# Global token-saving defaults for ripgrep (rg)
--smart-case
--max-columns=300
--max-columns-preview
--glob=!.git/
--glob=!node_modules/
--glob=!__pycache__/
--glob=!dist/
--glob=!build/
--glob=!target/
--glob=!.venv/
--glob=!venv/
--glob=!.next/
--glob=!.nuxt/
--glob=!.cache/
--glob=!coverage/
--glob=!.pytest_cache/
--glob=!.mypy_cache/
--glob=!.ruff_cache/
--glob=!.tox/
--glob=!.codegraph/
--glob=!*.min.js
--glob=!*.min.css
--glob=!*.bundle.js
--glob=!*.map
--glob=!*.pyc
--glob=!*.lock
--glob=!package-lock.json
--glob=!yarn.lock
--glob=!pnpm-lock.yaml
--glob=!Cargo.lock
--glob=!poetry.lock
--glob=!uv.lock
--glob=!go.sum
"#;

pub(crate) const AGY_TOKEN_OPTIMIZER_SCRIPT_TEMPLATE: &str = r##"#!/usr/bin/env python3
"""
Antigravity (agy) 原生 PreToolUse Token 优化守卫
由 `agent-body hooks generate --client agy` 自动生成。

功能:
  1. 拦截无关大目录/锁文件/压缩产物及单行超长文本的整读 (支持 <=200 行切片逃生口);
  2. 为 >36KB 或 >600 行的普通大文件自动注入 StartLine=1, EndLine=250 切片;
  3. 为未加过滤或无上限的 rg / grep -r / find 自动补充忽略目录与 180 行输出上限。

环境变量开关:
  AGY_TOKEN_OPTIMIZER=0|off|false|no  或  AGENT_BODY_TOKEN_GUARD=0  -> 立即旁路（Fail-open）
"""

import json
import os
import re
import shlex
import sys
from datetime import datetime, timezone
from typing import Any, Dict, Optional

EVENTS_LOG_PATH = os.path.expanduser(
    "~/.gemini/antigravity-cli/cache/token_optimizer_events.jsonl"
)
MAX_LOG_BYTES = 1024 * 1024

NOISE_DIR_SEGMENTS = {
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
}

RUST_TARGET_SUBDIRS = {"debug", "release", "doc", "tmp", "llvm-cov-target"}

NOISE_FILENAMES = {
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "Cargo.lock",
    "poetry.lock",
    "uv.lock",
    "composer.lock",
    "Gemfile.lock",
    "go.sum",
}

NOISE_SUFFIXES = (
    ".min.js",
    ".min.css",
    ".bundle.js",
    ".chunk.js",
    ".map",
    ".pyc",
    ".pyo",
    ".wasm",
)

BINARY_OR_MEDIA_EXTS = {
    ".png",
    ".jpg",
    ".jpeg",
    ".gif",
    ".webp",
    ".bmp",
    ".ico",
    ".pdf",
    ".mp3",
    ".mp4",
    ".wav",
    ".ogg",
    ".webm",
    ".mov",
    ".avi",
    ".zip",
    ".tar",
    ".gz",
    ".xz",
    ".7z",
    ".db",
    ".sqlite",
    ".duckdb",
}

MAX_FULL_READ_BYTES = 36 * 1024
MAX_FULL_READ_LINES = 600
AUTO_SLICE_END_LINE = 250
MINIFIED_CHECK_BYTES = 16 * 1024
MAX_EXPLICIT_NOISE_SLICE = 200
SEARCH_MAX_LINES = 180

GREP_EXCLUDE_ARGS = (
    "--exclude-dir=node_modules --exclude-dir=__pycache__ --exclude-dir=dist "
    "--exclude-dir=build --exclude-dir=target --exclude-dir=.git "
    "--exclude-dir=.venv --exclude-dir=.next --exclude-dir=.cache "
    "--exclude-dir=.codegraph"
)

AWK_CAP_PIPE = (
    " | awk 'NR<="
    + str(SEARCH_MAX_LINES)
    + "{print} END{if(NR>"
    + str(SEARCH_MAX_LINES)
    + ') printf "... [token-optimizer: truncated %d extra lines, narrow query or path]\\n", NR-'
    + str(SEARCH_MAX_LINES)
    + "}'"
)


def emit(obj: Dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def read_stdin_json() -> Dict[str, Any]:
    try:
        raw = sys.stdin.read()
        if not raw.strip():
            return {}
        data = json.loads(raw)
        return data if isinstance(data, dict) else {}
    except Exception:
        return {}


def log_event(tool: str, action: str, reason: str, target: str) -> None:
    try:
        os.makedirs(os.path.dirname(EVENTS_LOG_PATH), exist_ok=True)
        if os.path.exists(EVENTS_LOG_PATH) and os.path.getsize(EVENTS_LOG_PATH) > MAX_LOG_BYTES:
            with open(EVENTS_LOG_PATH, "w", encoding="utf-8"):
                pass
        rec = {
            "ts": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "tool": tool,
            "action": action,
            "reason": reason[:160],
            "target": target[:200],
        }
        with open(EVENTS_LOG_PATH, "a", encoding="utf-8") as f:
            f.write(json.dumps(rec, ensure_ascii=False) + "\n")
    except Exception:
        pass


def detect_noise_path(raw_path: str) -> Optional[str]:
    if not raw_path:
        return None
    norm = os.path.normpath(raw_path).replace("\\", "/")
    if "/.git/objects/" in norm or norm.endswith("/.git/objects"):
        return ".git/objects"
    if "/.cargo/registry/" in norm or norm.endswith("/.cargo/registry"):
        return ".cargo/registry"

    parts = [p for p in norm.split("/") if p and p != "."]
    for idx, seg in enumerate(parts):
        if seg in NOISE_DIR_SEGMENTS:
            return f"目录 '{seg}'"
        if seg == "target":
            next_seg = parts[idx + 1] if idx + 1 < len(parts) else ""
            if next_seg in RUST_TARGET_SUBDIRS:
                return f"构建目录 'target/{next_seg}'"
            prefix_parts = parts[:idx]
            target_parent = ("/" + "/".join(prefix_parts)) if norm.startswith("/") else ("/".join(prefix_parts) or ".")
            if os.path.exists(os.path.join(target_parent, "Cargo.toml")):
                return "Rust 构建目录 'target'"

    base = os.path.basename(norm)
    if base in NOISE_FILENAMES:
        return f"锁文件 '{base}'"
    base_lower = base.lower()
    for suf in NOISE_SUFFIXES:
        if base_lower.endswith(suf):
            return f"压缩/构建产物 '*{suf}'"
    return None


def is_bounded_int_slice(start_line: Any, end_line: Any) -> bool:
    if isinstance(start_line, bool) or isinstance(end_line, bool):
        return False
    if not (isinstance(start_line, int) and isinstance(end_line, int)):
        return False
    return start_line >= 1 and 0 <= (end_line - start_line) <= MAX_EXPLICIT_NOISE_SLICE


def count_lines_exceeds(path: str, max_lines: int) -> bool:
    newlines = 0
    with open(path, "rb") as f:
        while True:
            chunk = f.read(65536)
            if not chunk:
                break
            newlines += chunk.count(b"\n")
            if newlines > max_lines:
                return True
    return False


def handle_read_file(tool_name: str, args: Dict[str, Any]) -> Dict[str, Any]:
    path = str(args.get("Path") or args.get("AbsolutePath") or args.get("path") or "")
    if not path:
        return {"decision": "allow"}

    start_line = args.get("StartLine")
    end_line = args.get("EndLine")
    byte_offset = args.get("ByteOffset")
    bounded = is_bounded_int_slice(start_line, end_line)

    noise_detail = detect_noise_path(path)
    if noise_detail and not bounded:
        reason = (
            f"[token-optimizer] 跳过无关目录/锁文件/压缩产物以节省 Token ({noise_detail})。"
            "若确需查看，请先用 rg 定位，或显式传入 StartLine/EndLine (<=200 行) 定点读取。"
        )
        log_event(tool_name, "deny", reason, path)
        return {"decision": "deny", "reason": reason}

    if start_line is None and end_line is None and byte_offset is None and os.path.isfile(path):
        ext = os.path.splitext(path)[1].lower()
        if ext in BINARY_OR_MEDIA_EXTS:
            return {"decision": "allow"}

        try:
            size = os.path.getsize(path)
        except OSError:
            return {"decision": "allow"}

        if size > MINIFIED_CHECK_BYTES:
            try:
                with open(path, "rb") as f:
                    sample = f.read(8192)
                if b"\x00" in sample:
                    return {"decision": "allow"}
                if sample.count(b"\n") < 5:
                    reason = (
                        f"[token-optimizer] 文件 ({size} bytes) 疑似单行压缩/超长行文本，整读易爆上下文；"
                        "请改用 rg -o / jq / python3 提取目标片段。"
                    )
                    log_event(tool_name, "deny", reason, path)
                    return {"decision": "deny", "reason": reason}
            except OSError:
                return {"decision": "allow"}

        try:
            if size > MAX_FULL_READ_BYTES or count_lines_exceeds(path, MAX_FULL_READ_LINES):
                log_event(
                    tool_name,
                    "overwrite",
                    f"auto slice 1..{AUTO_SLICE_END_LINE} (size={size})",
                    path,
                )
                return {
                    "decision": "allow",
                    "overwrite": {"StartLine": 1, "EndLine": AUTO_SLICE_END_LINE},
                }
        except OSError:
            return {"decision": "allow"}

    return {"decision": "allow"}


def has_complex_shell_ops(cmd: str, allow_pipe: bool = False) -> bool:
    bad_tokens = ["\n", ";", "&&", "||", ">", "<", "$(", "`"]
    if not allow_pipe:
        bad_tokens.append("|")
    return any(tok in cmd for tok in bad_tokens)


def is_recursive_grep(tokens: list) -> bool:
    for tok in tokens[1:]:
        if tok == "--recursive":
            return True
        if tok.startswith("-") and not tok.startswith("--") and ("r" in tok or "R" in tok):
            return True
    return False


def handle_run_command(args: Dict[str, Any]) -> Dict[str, Any]:
    cmd_key = "CommandLine" if "CommandLine" in args else "command"
    cmd = str(args.get("CommandLine") or args.get("command") or "")
    stripped = cmd.strip()
    if not stripped:
        return {"decision": "allow"}

    if re.match(r"^rg(?:\s+|$)", stripped):
        if not any(sep in stripped for sep in ("\n", ";", "&&", "||")):
            new_cmd = stripped
            rc_path = os.path.expanduser("~/.ripgreprc")
            if (
                "RIPGREP_CONFIG_PATH" not in os.environ
                and "RIPGREP_CONFIG_PATH" not in new_cmd
                and "--no-config" not in new_cmd
                and os.path.exists(rc_path)
            ):
                new_cmd = 'RIPGREP_CONFIG_PATH="$HOME/.ripgreprc" ' + new_cmd

            is_simple = not has_complex_shell_ops(stripped, allow_pipe=False)
            has_summary_flag = bool(
                re.search(
                    r"(?:^|\s)(?:--files|--count|-c|-l|--files-with-matches|--json)(?:\s|$)",
                    stripped,
                )
            )
            if is_simple and not has_summary_flag:
                new_cmd = new_cmd + AWK_CAP_PIPE

            if new_cmd != stripped:
                log_event("run_command", "overwrite", "rg config/cap", stripped)
                return {"decision": "allow", "overwrite": {cmd_key: new_cmd}}
        return {"decision": "allow"}

    m_grep = re.match(r"^(grep|egrep|fgrep)\s+(.+)$", stripped)
    if m_grep and not any(sep in stripped for sep in ("\n", ";", "&&", "||", "$(", "`")):
        try:
            tokens = shlex.split(stripped)
        except ValueError:
            tokens = stripped.split()
        if is_recursive_grep(tokens) and "--exclude-dir" not in stripped:
            binary = m_grep.group(1)
            rest = m_grep.group(2)
            new_cmd = f"{binary} {GREP_EXCLUDE_ARGS} {rest}"
            is_simple = not has_complex_shell_ops(stripped, allow_pipe=False)
            has_summary_flag = bool(
                re.search(
                    r"(?:^|\s)(?:-c|--count|-l|--files-with-matches)(?:\s|$)",
                    stripped,
                )
            )
            if is_simple and not has_summary_flag:
                new_cmd = new_cmd + AWK_CAP_PIPE
            log_event("run_command", "overwrite", "grep exclude/cap", stripped)
            return {"decision": "allow", "overwrite": {cmd_key: new_cmd}}
        return {"decision": "allow"}

    if re.match(r"^find(?:\s+|$)", stripped) and not has_complex_shell_ops(stripped, allow_pipe=False):
        if re.search(r"(?:^|\s)(?:-exec|-execdir|-delete|-ok|-okdir)(?:\s|$)", stripped):
            return {"decision": "allow"}
        if re.search(r"-maxdepth\s+[012]\b", stripped):
            return {"decision": "allow"}
        new_cmd = stripped + AWK_CAP_PIPE
        log_event("run_command", "overwrite", "find output cap", stripped)
        return {"decision": "allow", "overwrite": {cmd_key: new_cmd}}

    return {"decision": "allow"}


def handle_search_tool(tool_name: str, args: Dict[str, Any]) -> Dict[str, Any]:
    target = str(
        args.get("SearchPath")
        or args.get("SearchDirectory")
        or args.get("AbsolutePath")
        or args.get("Directory")
        or args.get("path")
        or ""
    )
    if not target:
        return {"decision": "allow"}
    noise_detail = detect_noise_path(target)
    if noise_detail:
        reason = (
            f"[token-optimizer] 跳过在无关目录中搜索 ({noise_detail}) 以节省 Token；"
            "请缩小 SearchPath 至源码目录。"
        )
        log_event(tool_name, "deny", reason, target)
        return {"decision": "deny", "reason": reason}
    return {"decision": "allow"}


def main() -> None:
    for env_key in ("AGY_TOKEN_OPTIMIZER", "AGENT_BODY_TOKEN_GUARD"):
        if os.environ.get(env_key, "").strip().lower() in ("0", "off", "false", "no"):
            emit({"decision": "allow"})
            return

    payload = read_stdin_json()
    tool_call = payload.get("toolCall") or {}
    tool_name = str(tool_call.get("name") or "")
    raw_args = tool_call.get("args") or {}
    if not isinstance(raw_args, dict):
        emit({"decision": "allow"})
        return

    if tool_name in ("read_file", "view_file"):
        emit(handle_read_file(tool_name, raw_args))
    elif tool_name == "run_command":
        emit(handle_run_command(raw_args))
    elif tool_name in ("grep_search", "find_by_name"):
        emit(handle_search_tool(tool_name, raw_args))
    else:
        emit({"decision": "allow"})


if __name__ == "__main__":
    try:
        main()
    except Exception:
        emit({"decision": "allow"})
"##;
