use std::fs;
use std::path::Path;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// 支持的 Agent 客户端类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientType {
    Agy,
    Cursor,
    Claude,
    Git,
}

impl std::str::FromStr for ClientType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "agy" | "antigravity" => Ok(ClientType::Agy),
            "cursor" => Ok(ClientType::Cursor),
            "claude" | "claude-code" | "claudecode" => Ok(ClientType::Claude),
            "git" | "githook" | "git-hook" => Ok(ClientType::Git),
            other => anyhow::bail!(
                "不支持的客户端类型 `{}`。支持的类型包括: agy, cursor, claude, git",
                other
            ),
        }
    }
}

impl std::fmt::Display for ClientType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientType::Agy => write!(f, "agy"),
            ClientType::Cursor => write!(f, "cursor"),
            ClientType::Claude => write!(f, "claude"),
            ClientType::Git => write!(f, "git"),
        }
    }
}

/// 配套生成文件（例如 AGY hooks.json 对应的桥接脚本）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedCompanionFile {
    pub relative_path: String,
    pub content: String,
    pub executable: bool,
    pub written_path: Option<String>,
}

/// 客户端 Hook 配置生成结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedHookConfig {
    pub client: String,
    pub relative_path: String,
    pub content: String,
    pub executable: bool,
    pub is_dry_run: bool,
    pub written_path: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub companion_files: Vec<GeneratedCompanionFile>,
}

const AGY_HOOKS_JSON_TEMPLATE: &str = r#"{
  "agent-body": {
    "enabled": true,
    "SessionStart": [
      {
        "type": "command",
        "command": "python3 ./hooks/body_bridge.py session_start",
        "timeout": 5
      }
    ],
    "PreToolUse": [
      {
        "matcher": "write_file|edit_file|run_command",
        "hooks": [
          {
            "type": "command",
            "command": "python3 ./hooks/body_bridge.py pre_tool",
            "timeout": 5
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
            "command": "python3 ./hooks/body_bridge.py post_tool",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
"#;

const AGY_BRIDGE_SCRIPT_TEMPLATE: &str = r##"#!/usr/bin/env python3
"""
Antigravity (agy) <-> Agent Body (CLI) 生命周期硬 Hook 桥接脚本
由 `agent-body hooks generate --client agy` 自动生成。

用法（由 ~/.gemini/config/hooks.json 调用）:
  body_bridge.py session_start
  body_bridge.py pre_tool
  body_bridge.py post_tool
"""

import json
import os
import re
import shutil
import subprocess
import sys
from typing import Any, Dict, Optional

STEWARD_BIN = shutil.which("agent-body") or "/usr/local/bin/agent-body"

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
    home_steward = os.path.expanduser("~/.agent-body/")
    prefixes = NOISE_PATH_PREFIXES + (home_gemini, home_steward)
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
    if not os.path.exists(STEWARD_BIN):
        emit({})
        return
    try:
        proc = subprocess.run(
            [STEWARD_BIN, "brief"],
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
    if not os.path.exists(STEWARD_BIN):
        emit({"decision": "allow"})
        return

    tool_call = payload.get("toolCall") or {}
    tool_name = str(tool_call.get("name") or "")
    raw_args = tool_call.get("args") or {}
    if not isinstance(raw_args, dict):
        emit({"decision": "allow"})
        return

    steward_args: Dict[str, Any] = {}
    if tool_name in ("write_file", "edit_file"):
        path = str(raw_args.get("Path") or raw_args.get("path") or "")
        if is_noise_path(path):
            emit({"decision": "allow"})
            return
        steward_args["path"] = path
    elif tool_name == "run_command":
        cmd = str(raw_args.get("CommandLine") or raw_args.get("command") or "")
        if not cmd:
            emit({"decision": "allow"})
            return
        port = extract_listen_port(cmd)
        if port is None and not is_destructive_or_sensitive_command(cmd):
            emit({"decision": "allow"})
            return
        steward_args["command"] = cmd
        if port is not None:
            steward_args["port"] = port
    else:
        emit({"decision": "allow"})
        return

    try:
        proc = subprocess.run(
            [
                STEWARD_BIN,
                "hooks",
                "pre",
                "-t",
                tool_name,
                "-a",
                json.dumps(steward_args, ensure_ascii=False),
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
    if not os.path.exists(STEWARD_BIN):
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
        steward_args: Dict[str, Any] = {}
        cmd = raw_args.get("CommandLine") or raw_args.get("command") or ""
        path = raw_args.get("Path") or raw_args.get("path") or ""
        if cmd:
            steward_args["command"] = str(cmd)
        if path:
            steward_args["path"] = str(path)
        try:
            proc = subprocess.run(
                [
                    STEWARD_BIN,
                    "hooks",
                    "post",
                    "-t",
                    tool_name or "run_command",
                    "-a",
                    json.dumps(steward_args, ensure_ascii=False),
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
                    emit({"additionalContext": hint})
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

    steward_args = {"path": path}
    try:
        subprocess.Popen(
            [
                STEWARD_BIN,
                "hooks",
                "post",
                "-t",
                tool_name,
                "-a",
                json.dumps(steward_args, ensure_ascii=False),
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


def main() -> None:
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    if mode == "session_start":
        handle_session_start()
    elif mode == "pre_tool":
        handle_pre_tool()
    elif mode == "post_tool":
        handle_post_tool()
    else:
        emit({})


if __name__ == "__main__":
    main()
"##;

/// Agent 客户端 Hook 自动生成器 (规约 #13 MCP 与 CLI 1:1 映射)
pub struct ClientHookGenerator;

impl ClientHookGenerator {
    /// 根据客户端类型生成默认相对文件名
    pub fn default_relative_path(client: ClientType) -> &'static str {
        match client {
            ClientType::Agy => ".gemini/config/hooks.json",
            ClientType::Cursor => ".cursorrules",
            ClientType::Claude => "CLAUDE.md",
            ClientType::Git => ".git/hooks/post-commit",
        }
    }

    /// 配套生成文件清单（如 AGY 的桥接脚本）
    pub fn companion_templates(client: ClientType) -> Vec<(&'static str, &'static str, bool)> {
        match client {
            ClientType::Agy => vec![(
                ".gemini/config/hooks/body_bridge.py",
                AGY_BRIDGE_SCRIPT_TEMPLATE,
                true,
            )],
            _ => Vec::new(),
        }
    }

    /// 生成配置内容模板
    pub fn render_template(client: ClientType) -> (&'static str, bool) {
        match client {
            ClientType::Agy => (AGY_HOOKS_JSON_TEMPLATE, false),
            ClientType::Cursor => (
                r#"# Cursor Rules for Agent Body

# 1. Host Awareness & Safety
- Before starting services or tests, verify ports using agent-body:
  Command: `agent-body env port <port>` (or `agent-body fact "<port>"`) or check MCP `body_port_check`.
- Always respect the directory hierarchy:
  - Code/Projects: `/root/new/work/` or `/root/new/src/`
  - Shared Tools: `/root/new/shared_proj/`
  - Temporary files: `/root/new/tmp/` (auto-cleaned)

# 2. Automated Hook Integration
- When writing significant architecture/design files, ensure you invoke agent-body's hook:
  MCP tool: `body_hook_post` with the target file path.
- Avoid committing or tracking noisy directories: `node_modules`, `target`, `.git`, `.venv`.
"#,
                false,
            ),
            ClientType::Claude => (
                r#"# CLAUDE.md - Agent Body Integration Guidelines

## System Rules & Constraints
- **Agent Body Integration**: This host runs `agent-body` to manage environment facts, locks, and assets.
- **Port & Concurrency Safety**: Before binding any network port or launching background services, check for conflicts using `body_port_check` or run `agent-body env port <port>` / `agent-body platform ports`.
- **Pre-Hook & Post-Hook Discipline**:
  - Run `body_hook_pre` before major workspace modifications to verify locks.
  - Run `body_hook_post` after creating key artifacts to automatically index assets and register long-term memory.
- **Temporary Files**: Put throwaway files in `/root/new/tmp/`. Never pollute root project directories with ad-hoc logs.
"#,
                false,
            ),
            ClientType::Git => (
                r#"#!/usr/bin/env bash
# Git post-commit hook generated by agent-body
# Automatically indexes newly committed assets and emits on_file_modified events.

set -e

BODY_BIN=$(which agent-body 2>/dev/null || echo "/root/.cargo/bin/agent-body")

if [ -x "$BODY_BIN" ]; then
    # Trigger lightweight scan on current repo root without blocking commit flow
    REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
    "$BODY_BIN" asset scan "$REPO_ROOT" >/dev/null 2>&1 &
fi

exit 0
"#,
                true,
            ),
        }
    }

    /// 执行 Hook 配置生成
    pub fn generate(
        client: ClientType,
        target_dir: Option<&Path>,
        dry_run: bool,
    ) -> Result<GeneratedHookConfig> {
        let relative_path = Self::default_relative_path(client);
        let (content, executable) = Self::render_template(client);

        let mut written_path = None;

        if let Some(base_dir) = target_dir {
            let full_path = base_dir.join(relative_path);
            if !dry_run {
                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent)
                        .with_context(|| format!("创建目标目录失败: {:?}", parent))?;
                }
                fs::write(&full_path, content)
                    .with_context(|| format!("写入 Hook 配置文件失败: {:?}", full_path))?;

                #[cfg(unix)]
                if executable {
                    use std::os::unix::fs::PermissionsExt;
                    let perms = fs::Permissions::from_mode(0o755);
                    let _ = fs::set_permissions(&full_path, perms);
                }
            }
            written_path = Some(full_path.to_string_lossy().to_string());
        }

        let mut companion_files = Vec::new();
        for (comp_rel, comp_content, comp_exec) in Self::companion_templates(client) {
            let mut comp_written = None;
            if let Some(base_dir) = target_dir {
                let comp_full = base_dir.join(comp_rel);
                if !dry_run {
                    if let Some(parent) = comp_full.parent() {
                        fs::create_dir_all(parent)
                            .with_context(|| format!("创建配套脚本目录失败: {:?}", parent))?;
                    }
                    fs::write(&comp_full, comp_content)
                        .with_context(|| format!("写入配套 Hook 脚本失败: {:?}", comp_full))?;

                    #[cfg(unix)]
                    if comp_exec {
                        use std::os::unix::fs::PermissionsExt;
                        let perms = fs::Permissions::from_mode(0o755);
                        let _ = fs::set_permissions(&comp_full, perms);
                    }
                }
                comp_written = Some(comp_full.to_string_lossy().to_string());
            }
            companion_files.push(GeneratedCompanionFile {
                relative_path: comp_rel.to_string(),
                content: comp_content.to_string(),
                executable: comp_exec,
                written_path: comp_written,
            });
        }

        Ok(GeneratedHookConfig {
            client: client.to_string(),
            relative_path: relative_path.to_string(),
            content: content.to_string(),
            executable,
            is_dry_run: dry_run,
            written_path,
            companion_files,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_client_type_from_str() {
        assert_eq!("agy".parse::<ClientType>().unwrap(), ClientType::Agy);
        assert_eq!("cursor".parse::<ClientType>().unwrap(), ClientType::Cursor);
        assert_eq!("claude".parse::<ClientType>().unwrap(), ClientType::Claude);
        assert_eq!("git".parse::<ClientType>().unwrap(), ClientType::Git);
        assert!("unknown".parse::<ClientType>().is_err());
    }

    #[test]
    fn test_generate_dry_run() {
        let res = ClientHookGenerator::generate(ClientType::Agy, None, true).unwrap();
        assert_eq!(res.client, "agy");
        assert_eq!(res.relative_path, ".gemini/config/hooks.json");
        let parsed: serde_json::Value =
            serde_json::from_str(&res.content).expect("AGY hooks.json 必须是合法 JSON");
        assert!(parsed["agent-body"]["PreToolUse"].is_array());
        assert!(parsed["agent-body"]["PostToolUse"].is_array());
        assert_eq!(
            parsed["agent-body"]["PostToolUse"][0]["matcher"],
            "write_file|edit_file|run_command"
        );
        assert!(parsed["agent-body"]["SessionStart"].is_array());
        assert_eq!(res.companion_files.len(), 1);
        assert_eq!(
            res.companion_files[0].relative_path,
            ".gemini/config/hooks/body_bridge.py"
        );
        assert!(res.companion_files[0].content.contains("extract_failure_text"));
        assert!(res.companion_files[0].content.contains("--error-output"));
        assert!(res.companion_files[0].content.contains("is_destructive_or_sensitive_command"));
        assert!(res.companion_files[0].executable);
        assert!(res.written_path.is_none());
        assert!(res.is_dry_run);
    }

    #[test]
    fn test_generate_agy_write_to_directory() {
        let dir = tempdir().unwrap();
        let res = ClientHookGenerator::generate(ClientType::Agy, Some(dir.path()), false).unwrap();
        assert_eq!(res.client, "agy");
        assert!(res.written_path.is_some());
        assert_eq!(res.companion_files.len(), 1);
        assert!(res.companion_files[0].written_path.is_some());

        let hooks_file = dir.path().join(".gemini/config/hooks.json");
        let bridge_file = dir.path().join(".gemini/config/hooks/body_bridge.py");
        assert!(hooks_file.exists());
        assert!(bridge_file.exists());

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&hooks_file).unwrap()).unwrap();
        assert_eq!(parsed["agent-body"]["enabled"], true);

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&bridge_file).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "body_bridge.py 必须具有可执行权限");
        }
    }

    #[test]
    fn test_generate_write_to_directory() {
        let dir = tempdir().unwrap();
        let res = ClientHookGenerator::generate(ClientType::Git, Some(dir.path()), false).unwrap();
        assert_eq!(res.client, "git");
        assert!(res.executable);
        assert!(res.written_path.is_some());

        let target_file = dir.path().join(".git/hooks/post-commit");
        assert!(target_file.exists());
        let read_back = fs::read_to_string(&target_file).unwrap();
        assert!(read_back.contains("asset scan"));
        assert!(read_back.contains("BODY_BIN"));
        assert!(!read_back.contains("--depth"));

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = fs::metadata(&target_file).unwrap();
            let mode = meta.permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "Git hook 必须具有可执行权限");
        }
    }
}
