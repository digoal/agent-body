pub(crate) const AGY_HOOKS_JSON_TEMPLATE: &str = r#"{
  "agent-body": {
    "enabled": true,
    "SessionStart": [
      {
        "type": "command",
        "command": "agent-body hooks bridge session_start",
        "timeout": 5
      }
    ],
    "PreToolUse": [
      {
        "matcher": "read_file|view_file|write_file|edit_file|run_command|grep_search|find_by_name",
        "hooks": [
          {
            "type": "command",
            "command": "agent-body hooks bridge pre_tool",
            "timeout": 3
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "write_file|edit_file|run_command",
        "hooks": [
          {
            "type": "command",
            "command": "agent-body hooks bridge post_tool",
            "timeout": 3
          }
        ]
      }
    ],
    "PreInvocation": [
      {
        "type": "command",
        "command": "agent-body hooks bridge pre_invocation",
        "timeout": 2
      }
    ],
    "Stop": [
      {
        "type": "command",
        "command": "agent-body hooks bridge stop",
        "timeout": 2
      }
    ]
  }
}
"#;

pub(crate) const AGY_BRIDGE_SCRIPT_TEMPLATE: &str = r##"#!/usr/bin/env python3
"""
Antigravity (agy) <-> Agent Body (CLI) 生命周期硬 Hook 桥接脚本
由 `agent-body hooks generate --client agy` 自动生成。

用法（由 ~/.gemini/config/hooks.json 调用）:
  body_bridge.py session_start
  body_bridge.py pre_tool
  body_bridge.py post_tool
  body_bridge.py pre_invocation
  body_bridge.py stop
"""

import json
import os
import re
import shutil
import subprocess
import sys
from typing import Any, Dict, Optional

BODY_BIN = shutil.which("agent-body") or "/usr/local/bin/agent-body"

# 仅匹配明确“启动监听服务/端口绑定”的命令，避免误拦 psql -p 5432 / curl :8080 等客户端命令
LISTEN_PORT_PATTERNS = [
    re.compile(r"\bhttp\.server\s+(\d{2,5})\b"),
    re.compile(r"\b(?:uvicorn|gunicorn|flask\s+run|vite|webpack-dev-server|streamlit\s+run)\b[^\n]*?(?:--port|-p)[=\s]+(\d{2,5})\b"),
    re.compile(r"\b(?:next\s+dev|nuxt\s+dev|astro\s+dev)\b[^\n]*?(?:--port|-p)[=\s]+(\d{2,5})\b"),
    re.compile(r"\bdocker\s+run\b[^\n]*?(?:-p|--publish)[=\s]+(?:[\d.]+:)?(\d{2,5}):\d{2,5}\b"),
    re.compile(r"\bpg_ctl\b[^\n]*?\bstart\b[^\n]*?-p\s+(\d{2,5})\b"),
    re.compile(r"\b(?:nc|ncat|socat)\b[^\n]*?-[a-zA-Z]*l[a-zA-Z]*[^\n]*?\s(\d{2,5})\b"),
    re.compile(r"\bserve\b[^\n]*?(?:-l|--listen)[=\s]+(?:tcp://[^:]+:)?(\d{2,5})\b"),
]

NOISE_PATH_PREFIXES = (
    "/tmp/",
    "/var/tmp/",
    "/root/.gemini/",
    "/root/.agent-body/",
)

DESTRUCTIVE_CMD_PATTERNS = (
    "rm -rf / ",
    "rm -rf /*",
    "rm -fr / ",
    "rm -fr /*",
    "mkfs.",
    "dd if=",
    "chmod -r 777 /",
    ":(){ :|:& };:",
)

CRITICAL_PATH_PATTERNS = (
    "/etc/shadow",
    "/etc/gshadow",
    "/etc/sudoers",
    "/etc/passwd",
    "/.ssh/",
    "~/.ssh/",
    "/.gnupg/",
    "/.agent-body/body.db",
)


def is_destructive_or_sensitive_command(cmd: str) -> bool:
    norm = " ".join(cmd.split()).lower() + " "
    if any(pat in norm for pat in DESTRUCTIVE_CMD_PATTERNS):
        return True
    return any(pat in norm for pat in CRITICAL_PATH_PATTERNS)


def read_stdin_json() -> Dict[str, Any]:
    try:
        raw = sys.stdin.read()
        if not raw.strip():
            return {}
        return json.loads(raw)
    except Exception:
        return {}


def emit(obj: Dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def is_noise_path(path: str) -> bool:
    if not path:
        return True
    home_gemini = os.path.expanduser("~/.gemini/")
    home_body = os.path.expanduser("~/.agent-body/")
    prefixes = NOISE_PATH_PREFIXES + (home_gemini, home_body)
    return any(path.startswith(prefix) for prefix in prefixes)


def extract_listen_port(cmd: str) -> Optional[int]:
    for pat in LISTEN_PORT_PATTERNS:
        m = pat.search(cmd)
        if m:
            try:
                port = int(m.group(1))
                if 1 <= port <= 65535:
                    return port
            except ValueError:
                pass
    return None


def handle_session_start() -> None:
    if not os.path.exists(BODY_BIN):
        emit({})
        return
    try:
        proc = subprocess.run(
            [BODY_BIN, "brief"],
            capture_output=True,
            text=True,
            timeout=2.5,
            check=False,
        )
        out = (proc.stdout or "").strip()
        if proc.returncode == 0 and out:
            if len(out) > 2500:
                out = out[:2500] + "\n... (truncated)"
            emit(
                {
                    "injectSteps": [
                        {
                            "systemMessage": {
                                "systemMessage": f"[Agent Body 自动开工简报]\n{out}"
                            }
                        }
                    ]
                }
            )
            return
    except Exception:
        pass
    emit({})


def handle_pre_tool() -> None:
    payload = read_stdin_json()
    if not os.path.exists(BODY_BIN):
        emit({"decision": "allow"})
        return

    tool_call = payload.get("toolCall") or {}
    tool_name = str(tool_call.get("name") or "")
    raw_args = tool_call.get("args") or {}
    if not isinstance(raw_args, dict):
        emit({"decision": "allow"})
        return

    body_args: Dict[str, Any] = {}
    if tool_name in ("write_file", "edit_file"):
        path = str(raw_args.get("Path") or raw_args.get("path") or "")
        if is_noise_path(path):
            emit({"decision": "allow"})
            return
        body_args["path"] = path
    elif tool_name == "run_command":
        cmd = str(raw_args.get("CommandLine") or raw_args.get("command") or "")
        if not cmd:
            emit({"decision": "allow"})
            return
        port = extract_listen_port(cmd)
        if port is None and not is_destructive_or_sensitive_command(cmd):
            emit({"decision": "allow"})
            return
        body_args["command"] = cmd
        if port is not None:
            body_args["port"] = port
    else:
        emit({"decision": "allow"})
        return

    try:
        proc = subprocess.run(
            [
                BODY_BIN,
                "hooks",
                "pre",
                "-t",
                tool_name,
                "-a",
                json.dumps(body_args, ensure_ascii=False),
            ],
            capture_output=True,
            text=True,
            timeout=2.0,
            check=False,
        )
        if proc.returncode != 0 or not proc.stdout.strip():
            emit({"decision": "allow"})
            return

        res = json.loads(proc.stdout)
        allowed = res.get("allowed", True)
        has_warnings = bool(res.get("has_warnings", False))
        warnings = res.get("warnings") or []
        suggestions = res.get("suggestions") or []

        if not allowed:
            reason_parts = warnings + suggestions
            reason = "[agent-body 阻断] " + " ; ".join(reason_parts) if reason_parts else "[agent-body 阻断] 命中安全/协作锁策略"
            emit({"decision": "deny", "reason": reason})
            return

        if has_warnings:
            reason_parts = warnings + suggestions
            reason = "[agent-body 风险预警] " + " ; ".join(reason_parts)
            emit({"decision": "ask", "reason": reason})
            return
    except Exception:
        pass

    emit({"decision": "allow"})


def extract_failure_text(payload: Dict[str, Any]) -> Optional[str]:
    err_obj = payload.get("error")
    if err_obj:
        if isinstance(err_obj, dict):
            msg = err_obj.get("message") or err_obj.get("error") or json.dumps(err_obj, ensure_ascii=False)
            return str(msg)[:800]
        return str(err_obj)[:800]

    for key in ("toolResult", "response", "result", "output"):
        val = payload.get(key)
        if not val:
            continue
        if isinstance(val, dict):
            code = val.get("exitCode") or val.get("returncode") or val.get("exit_code")
            txt = str(val.get("stderr") or val.get("output") or val.get("error") or val.get("content") or "")
            if (code is not None and str(code) not in ("0", "")) or any(
                m in txt
                for m in (
                    "Connection refused",
                    "ECONNREFUSED",
                    "could not connect to server",
                    "password authentication failed",
                    "command not found",
                    "Address already in use",
                    "EADDRINUSE",
                )
            ):
                return txt[:800] if txt else f"exitCode={code}"
        elif isinstance(val, str) and any(
            m in val
            for m in (
                "Connection refused",
                "ECONNREFUSED",
                "could not connect to server",
                "password authentication failed",
                "command not found",
                "Address already in use",
                "EADDRINUSE",
            )
        ):
            return val[:800]
    return None


def handle_post_tool() -> None:
    payload = read_stdin_json()
    if not os.path.exists(BODY_BIN):
        emit({})
        return

    tool_call = payload.get("toolCall") or {}
    tool_name = str(tool_call.get("name") or "")
    raw_args = tool_call.get("args") or {}
    if not isinstance(raw_args, dict):
        raw_args = {}

    # AOPCC 失败早回退（OCC Abort Recovery）：工具执行出错时 1.5s 内同步诊断并回注自愈处方
    err_text = extract_failure_text(payload)
    if err_text:
        body_args: Dict[str, Any] = {}
        cmd = raw_args.get("CommandLine") or raw_args.get("command") or ""
        path = raw_args.get("Path") or raw_args.get("path") or ""
        if cmd:
            body_args["command"] = str(cmd)
        if path:
            body_args["path"] = str(path)
        try:
            proc = subprocess.run(
                [
                    BODY_BIN,
                    "hooks",
                    "post",
                    "-t",
                    tool_name or "run_command",
                    "-a",
                    json.dumps(body_args, ensure_ascii=False),
                    "--success=false",
                    "--error-output",
                    err_text,
                ],
                capture_output=True,
                text=True,
                timeout=1.5,
                check=False,
            )
            if proc.returncode == 0 and proc.stdout.strip():
                res = json.loads(proc.stdout)
                rx = res.get("recovery_prescription")
                if isinstance(rx, dict):
                    actions = rx.get("recovery_actions") or []
                    hint = (
                        f"[agent-body AOPCC 自愈处方 | {rx.get('failure_kind', '')}] "
                        f"{rx.get('root_cause', '')}"
                    )
                    if actions:
                        hint += " | 建议动作: " + " ; ".join(str(a) for a in actions[:3])
                    # agy PostToolUse 仅接收 {}，暂存处方供下一轮 PreInvocation injectSteps 注入
                    _write_rx_cache(payload, hint)
                    emit({})
                    return
        except Exception:
            pass
        emit({})
        return

    # 正常乐观执行的 run_command 零阻塞退出，仅 write_file/edit_file 走异步资产建档
    if tool_name not in ("write_file", "edit_file"):
        emit({})
        return

    path = str(raw_args.get("Path") or raw_args.get("path") or "")
    if is_noise_path(path):
        emit({})
        return

    body_args = {"path": path}
    try:
        subprocess.Popen(
            [
                BODY_BIN,
                "hooks",
                "post",
                "-t",
                tool_name,
                "-a",
                json.dumps(body_args, ensure_ascii=False),
                "--success",
            ],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
    except Exception:
        pass

    emit({})


def _rx_cache_path(payload: Dict[str, Any]) -> str:
    conv_id = str(payload.get("conversationId") or "default")
    safe_id = re.sub(r"[^a-zA-Z0-9._-]", "_", conv_id)[:64] or "default"
    return os.path.expanduser(f"~/.agent-body/tmp/agy_rx_{safe_id}.txt")


def _write_rx_cache(payload: Dict[str, Any], hint: str) -> None:
    try:
        p = _rx_cache_path(payload)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "w", encoding="utf-8") as f:
            f.write(hint)
    except Exception:
        pass


def handle_pre_invocation() -> None:
    payload = read_stdin_json()
    p = _rx_cache_path(payload)
    if not os.path.exists(p):
        emit({})
        return
    try:
        with open(p, "r", encoding="utf-8") as f:
            hint = f.read().strip()
        try:
            os.remove(p)
        except Exception:
            pass
        if hint:
            emit(
                {
                    "injectSteps": [
                        {
                            "systemMessage": {
                                "systemMessage": hint
                            }
                        }
                    ]
                }
            )
            return
    except Exception:
        pass
    emit({})


def handle_stop() -> None:
    payload = read_stdin_json()
    reason = str(payload.get("terminationReason") or "")
    conv_id = str(payload.get("conversationId") or "").strip()
    if reason == "NO_TOOL_CALL" and conv_id and os.path.exists(BODY_BIN):
        try:
            subprocess.Popen(
                [BODY_BIN, "close", conv_id, "--clean-temp=false"],
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True,
            )
        except Exception:
            pass
    emit({})


def _is_bypass_mode() -> bool:
    env_mode = os.environ.get("AGENT_BODY_MODE", "").strip().lower()
    if env_mode in ("bypass", "off", "passthrough"):
        return True
    if env_mode in ("body_only", "body", "shadow_ab", "shadow"):
        return False
    for name in ("runtime_mode.override", "execution_mode.override"):
        p = os.path.expanduser(f"~/.agent-body/{name}")
        if os.path.exists(p):
            try:
                raw = open(p, "r", encoding="utf-8").read().strip().lower()
                if raw in ("bypass", "off", "passthrough"):
                    return True
                if raw in ("body_only", "body", "shadow_ab", "shadow"):
                    return False
            except Exception:
                pass
    return False


def main() -> None:
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    if _is_bypass_mode():
        emit({"decision": "allow"} if mode == "pre_tool" else {})
        return
    if mode == "session_start":
        handle_session_start()
    elif mode == "pre_tool":
        handle_pre_tool()
    elif mode == "post_tool":
        handle_post_tool()
    elif mode == "pre_invocation":
        handle_pre_invocation()
    elif mode == "stop":
        handle_stop()
    else:
        emit({})


if __name__ == "__main__":
    main()
"##;

pub(crate) const CLAUDE_SETTINGS_JSON_TEMPLATE: &str = r#"{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          {
            "type": "command",
            "command": "python3 ./.claude/hooks/body_claude_bridge.py session_start",
            "timeout": 5
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "Write|Edit|MultiEdit|Bash|Read|Grep|Glob",
        "hooks": [
          {
            "type": "command",
            "command": "python3 ./.claude/hooks/body_claude_bridge.py pre_tool",
            "timeout": 5
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Write|Edit|MultiEdit|Bash",
        "hooks": [
          {
            "type": "command",
            "command": "python3 ./.claude/hooks/body_claude_bridge.py post_tool",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
"#;

pub(crate) const CLAUDE_BRIDGE_SCRIPT_TEMPLATE: &str = r##"#!/usr/bin/env python3
"""
Claude Code <-> Agent Body (CLI) lifecycle hook bridge.
Generated by `agent-body hooks generate --client claude`.
"""

import json
import os
import shutil
import subprocess
import sys
from typing import Any, Dict

BODY_BIN = shutil.which("agent-body") or "/usr/local/bin/agent-body"


def read_stdin_json() -> Dict[str, Any]:
    try:
        raw = sys.stdin.read()
        return json.loads(raw) if raw.strip() else {}
    except Exception:
        return {}


def emit(obj: Dict[str, Any]) -> None:
    sys.stdout.write(json.dumps(obj, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def _is_bypass_mode() -> bool:
    env_mode = os.environ.get("AGENT_BODY_MODE", "").strip().lower()
    if env_mode in ("bypass", "off", "passthrough"):
        return True
    if env_mode in ("body_only", "body", "shadow_ab", "shadow"):
        return False
    for name in ("runtime_mode.override", "execution_mode.override"):
        p = os.path.expanduser(f"~/.agent-body/{name}")
        if os.path.exists(p):
            try:
                raw = open(p, "r", encoding="utf-8").read().strip().lower()
                if raw in ("bypass", "off", "passthrough"):
                    return True
                if raw in ("body_only", "body", "shadow_ab", "shadow"):
                    return False
            except Exception:
                pass
    return False


def main() -> None:
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    if _is_bypass_mode():
        emit({})
        return
    if not os.path.exists(BODY_BIN):
        emit({})
        return
    payload = read_stdin_json()
    tool_name = str(payload.get("tool_name") or payload.get("toolName") or "")
    tool_input = payload.get("tool_input") or payload.get("toolInput") or {}
    if not isinstance(tool_input, dict):
        tool_input = {}

    if mode == "session_start":
        try:
            proc = subprocess.run([BODY_BIN, "brief"], capture_output=True, text=True, timeout=2.5, check=False)
            out = (proc.stdout or "").strip()
            if proc.returncode == 0 and out:
                emit({"hookSpecificOutput": {"hookEventName": "SessionStart", "additionalContext": out[:2500]}})
                return
        except Exception:
            pass
        emit({})
        return

    tool_map = {
        "Write": "write_file",
        "Edit": "edit_file",
        "MultiEdit": "edit_file",
        "Bash": "run_command",
        "Read": "read_file",
        "Grep": "grep_search",
        "Glob": "find_by_name",
    }
    mapped_name = tool_map.get(tool_name, tool_name)
    args_dict = dict(tool_input)
    if "file_path" in tool_input and "path" not in args_dict:
        args_dict["path"] = tool_input["file_path"]
    if "command" in tool_input and "cmd" not in args_dict:
        args_dict["cmd"] = tool_input["command"]

    if mode == "pre_tool":
        try:
            proc = subprocess.run(
                [BODY_BIN, "hooks", "pre", "--tool-name", mapped_name, "--args", json.dumps(args_dict, ensure_ascii=False)],
                capture_output=True,
                text=True,
                timeout=3.0,
                check=False,
            )
            data = json.loads(proc.stdout) if proc.returncode == 0 and proc.stdout.strip() else {}
            if not data.get("allowed", True) or str(data.get("action") or "").upper() == "BLOCK":
                warnings = data.get("warnings") or []
                suggestions = data.get("suggestions") or []
                reason = " ; ".join(warnings + suggestions) or data.get("message") or "Blocked by Agent Body pre-hook"
                emit({
                    "hookSpecificOutput": {
                        "hookEventName": "PreToolUse",
                        "permissionDecision": "deny",
                        "permissionDecisionReason": reason,
                    }
                })
                return
        except Exception:
            pass
        emit({})
        return

    if mode == "post_tool":
        tool_resp = payload.get("tool_response") or {}
        err_txt = ""
        success = True
        if isinstance(tool_resp, dict) and tool_resp.get("is_error"):
            success = False
            err_txt = str(tool_resp.get("stderr") or tool_resp.get("content") or "")
        cmd = [
            BODY_BIN,
            "hooks",
            "post",
            "--tool-name",
            mapped_name,
            "--args",
            json.dumps(args_dict, ensure_ascii=False),
            "--success=" + ("true" if success else "false"),
        ]
        if not success and err_txt:
            cmd.extend(["--error-output", err_txt[:400]])
        try:
            subprocess.Popen(cmd, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        except Exception:
            pass
        emit({})
        return

    emit({})


if __name__ == "__main__":
    main()
"##;

pub(crate) const HERMES_BRIDGE_SCRIPT_TEMPLATE: &str = r##"#!/usr/bin/env python3
"""
Hermes <-> Agent Body (CLI) lifecycle hook bridge.
Generated by `agent-body hooks generate --client hermes`.
"""

import json
import os
import shutil
import subprocess
import sys

BODY_BIN = shutil.which("agent-body") or "/usr/local/bin/agent-body"


def run_cli(args):
    if not os.path.exists(BODY_BIN):
        return "{}"
    proc = subprocess.run([BODY_BIN] + args, capture_output=True, text=True, timeout=3.0, check=False)
    return (proc.stdout or "").strip() or "{}"


def _is_bypass_mode() -> bool:
    env_mode = os.environ.get("AGENT_BODY_MODE", "").strip().lower()
    if env_mode in ("bypass", "off", "passthrough"):
        return True
    if env_mode in ("body_only", "body", "shadow_ab", "shadow"):
        return False
    for name in ("runtime_mode.override", "execution_mode.override"):
        p = os.path.expanduser(f"~/.agent-body/{name}")
        if os.path.exists(p):
            try:
                raw = open(p, "r", encoding="utf-8").read().strip().lower()
                if raw in ("bypass", "off", "passthrough"):
                    return True
                if raw in ("body_only", "body", "shadow_ab", "shadow"):
                    return False
            except Exception:
                pass
    return False


def main() -> None:
    mode = sys.argv[1] if len(sys.argv) > 1 else "brief"
    if _is_bypass_mode():
        print("{}")
        return
    if mode == "brief":
        print(run_cli(["brief"]))
    elif mode == "pre":
        tool_name = sys.argv[2] if len(sys.argv) > 2 else "run_command"
        tool_args = sys.argv[3] if len(sys.argv) > 3 else "{}"
        print(run_cli(["hooks", "pre", "--tool-name", tool_name, "--args", tool_args]))
    elif mode == "post":
        tool_name = sys.argv[2] if len(sys.argv) > 2 else "write_file"
        tool_args = sys.argv[3] if len(sys.argv) > 3 else "{}"
        print(run_cli(["hooks", "post", "--tool-name", tool_name, "--args", tool_args]))
    else:
        print(json.dumps({"status": "noop", "mode": mode}))


if __name__ == "__main__":
    main()
"##;

