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
    Codex,
    Hermes,
    Git,
}

impl std::str::FromStr for ClientType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "agy" | "antigravity" => Ok(ClientType::Agy),
            "cursor" => Ok(ClientType::Cursor),
            "claude" | "claude-code" | "claudecode" => Ok(ClientType::Claude),
            "codex" | "openai-codex" => Ok(ClientType::Codex),
            "hermes" | "hermes-agent" => Ok(ClientType::Hermes),
            "git" | "githook" | "git-hook" => Ok(ClientType::Git),
            other => anyhow::bail!(
                "不支持的客户端类型 `{}`。支持的类型包括: agy, cursor, claude, codex, hermes, git",
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
            ClientType::Codex => write!(f, "codex"),
            ClientType::Hermes => write!(f, "hermes"),
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

use super::client_templates::{
    AGY_BRIDGE_SCRIPT_TEMPLATE, AGY_HOOKS_JSON_TEMPLATE, CLAUDE_BRIDGE_SCRIPT_TEMPLATE,
    CLAUDE_SETTINGS_JSON_TEMPLATE, HERMES_BRIDGE_SCRIPT_TEMPLATE,
};
use super::token_guard_templates::{AGY_TOKEN_OPTIMIZER_SCRIPT_TEMPLATE, RIPGREPRC_TEMPLATE};

/// Agent 客户端 Hook 自动生成器 (规约 #13 MCP 与 CLI 1:1 映射)
pub struct ClientHookGenerator;

impl ClientHookGenerator {
    /// 根据客户端类型生成默认相对文件名
    pub fn default_relative_path(client: ClientType) -> &'static str {
        match client {
            ClientType::Agy => ".gemini/config/hooks.json",
            ClientType::Cursor => ".cursorrules",
            ClientType::Claude => "CLAUDE.md",
            ClientType::Codex => "AGENTS.md",
            ClientType::Hermes => ".hermes/agent-body-rules.md",
            ClientType::Git => ".git/hooks/post-commit",
        }
    }

    /// 配套生成文件清单（如 AGY / Claude / Hermes 的配置与桥接脚本）
    pub fn companion_templates(client: ClientType) -> Vec<(&'static str, &'static str, bool)> {
        match client {
            ClientType::Agy => vec![
                (
                    ".gemini/config/hooks/body_bridge.py",
                    AGY_BRIDGE_SCRIPT_TEMPLATE,
                    true,
                ),
                (
                    ".gemini/config/hooks/token_optimizer.py",
                    AGY_TOKEN_OPTIMIZER_SCRIPT_TEMPLATE,
                    true,
                ),
                (".ripgreprc", RIPGREPRC_TEMPLATE, false),
            ],
            ClientType::Claude => vec![
                (".claude/settings.json", CLAUDE_SETTINGS_JSON_TEMPLATE, false),
                (
                    ".claude/hooks/body_claude_bridge.py",
                    CLAUDE_BRIDGE_SCRIPT_TEMPLATE,
                    true,
                ),
            ],
            ClientType::Hermes => vec![(
                ".hermes/hooks/body_bridge.py",
                HERMES_BRIDGE_SCRIPT_TEMPLATE,
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
- Always respect the workspace directory hierarchy (`agent-body place rules`):
  - Code/Projects: `<workspace_base>/work/` or `<workspace_base>/src/`
  - Shared Tools: `<workspace_base>/shared_proj/`
  - Temporary files: `<temp_dir>/` (auto-cleaned)

# 2. Automated Hook Integration & Token Guard
- When writing significant architecture/design files, ensure you invoke agent-body's hook:
  MCP tool: `body_hook_post` with the target file path.
- Token Hygiene: prefer `codegraph explore` / `body_asset_search` / `rg` (with `.ripgreprc`) and slice large reads (`StartLine`/`EndLine`); set `mcp_profile = "dev"` (18 tools) or `"core"` (10 tools) in `[runtime]` to trim context schema overhead.
- Avoid committing or tracking noisy directories: `node_modules`, `target`, `.git`, `.venv`, `dist`, `build`.
"#,
                false,
            ),
            ClientType::Claude => (
                r#"# CLAUDE.md - Agent Body Integration Guidelines

## System Rules & Constraints
- **Agent Body Integration**: This host runs `agent-body` to manage environment facts, locks, continuity capsules, and assets.
- **Claude Code Hooks**: Automated `SessionStart`, `PreToolUse`, and `PostToolUse` hooks are wired in `.claude/settings.json` -> `.claude/hooks/body_claude_bridge.py`.
- **Port & Concurrency Safety**: Before binding any network port or launching background services, check for conflicts using `body_port_check` or run `agent-body env port <port>` / `agent-body platform ports`.
- **Cross-Brain Continuity & Vitals**: Check `agent-body body manifest` / `agent-body body vitals` or `body_state` when resuming across sessions.
- **Pre-Hook & Token Guard Discipline**:
  - Run `body_hook_pre` before major workspace modifications to verify locks and Token Guard (`node_modules`/`dist`/`target`/lockfiles blocking & large-file slicing).
  - Run `body_hook_post` after creating key artifacts to automatically index assets and register long-term memory.
  - Set `[runtime].mcp_profile = "dev"` (18 tools) or `"core"` (10 tools) and configure `~/.ripgreprc` to minimize token tax.
- **Temporary Files**: Put throwaway files in your configured temp sandbox (`agent-body place suggest --type tmp` or `/tmp/`). Never pollute root project directories with ad-hoc logs.
"#,
                false,
            ),
            ClientType::Codex => (
                r#"# AGENTS.md - Agent Body Integration Guidelines (OpenAI Codex)

## Host Reality & Embodiment Protocol
- **Session Bootstrap**: Start tasks by reading host context with `agent-body brief` (or MCP `body_brief`) instead of blindly re-scanning the filesystem.
- **Port & Concurrency Safety**: Before starting listeners or background daemons, check ports with `agent-body env port <port>` / `agent-body locks list` (or MCP `body_port_check` / `body_lock_acquire`).
- **Write Guards & Post-Hooks**: Run `agent-body hooks pre --tool-name write_file --args '{"path":"<path>"}'` before risky writes, and `agent-body hooks post --tool-name write_file --args '{"path":"<path>"}'` afterwards so write-through invalidation stays current.
- **Cross-Brain Handoff**: Save/resume continuity capsules across brain switches via `agent-body body handoff` and `agent-body body resume`.
- **Code, Token Guard & Placement Discipline**: Check `.codegraph/` with `codegraph explore` first; use `rg` with `.ripgreprc` and bounded slices for large files; set `mcp_profile = "dev"`/`"core"` to cut MCP schema tokens; verify target directories with `agent-body place suggest`; keep temporary artifacts in your configured temp sandbox (`agent-body gc --max-age-hours 72`).
"#,
                false,
            ),
            ClientType::Hermes => (
                r#"# Agent Body Integration Rules for Hermes Agent

## Runtime Embodiment Integration
- **Startup Brief**: Invoke `python3 .hermes/hooks/body_bridge.py brief` or `agent-body brief` at session start to obtain host facts, optimistic AOPCC leases, and active continuity capsules.
- **Pre/Post Guards & Token Guard**:
  - Pre-check writes/binds/reads: `python3 .hermes/hooks/body_bridge.py pre <tool_name> '<args_json>'`
  - Post-sync changes & AOPCC failure recovery: `python3 .hermes/hooks/body_bridge.py post <tool_name> '<args_json>'`
  - Configure `[runtime].mcp_profile = "dev"`/`"core"` and `~/.ripgreprc` to avoid scanning `node_modules`/`dist`/`target`.
- **Multi-Agent Coordination**: Acquire/release locks via `agent-body coordination acquire` / `agent-body coordination release` (`body_lock_acquire` / `body_lock_release`).
- **Telemetry & AWR Diagnostics**: Inspect host health with `agent-body body vitals` and `agent-body body perf-awr`.
"#,
                false,
            ),
            ClientType::Git => (
                r#"#!/usr/bin/env bash
# Git post-commit hook generated by agent-body
# Automatically indexes newly committed assets and emits on_file_modified events.

set -e

BODY_BIN=$(which agent-body 2>/dev/null || echo "$HOME/.cargo/bin/agent-body")

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
                    if let Err(e) = fs::set_permissions(&full_path, perms) {
                        eprintln!("[Hooks][WARN] 设置脚本权限 {:?} 失败: {}", full_path, e);
                    }
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
                        if let Err(e) = fs::set_permissions(&comp_full, perms) {
                            eprintln!("[Hooks][WARN] 设置配套脚本权限 {:?} 失败: {}", comp_full, e);
                        }
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
        assert_eq!("codex".parse::<ClientType>().unwrap(), ClientType::Codex);
        assert_eq!("hermes".parse::<ClientType>().unwrap(), ClientType::Hermes);
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
        assert_eq!(
            parsed["agent-body"]["PreToolUse"][0]["matcher"],
            "read_file|view_file|write_file|edit_file|run_command|grep_search|find_by_name"
        );
        assert_eq!(
            parsed["agent-body"]["PreToolUse"][0]["hooks"][0]["command"],
            "agent-body hooks bridge pre_tool"
        );
        assert!(parsed["agent-body"]["PostToolUse"].is_array());
        assert_eq!(
            parsed["agent-body"]["PostToolUse"][0]["matcher"],
            "write_file|edit_file|run_command"
        );
        assert_eq!(
            parsed["agent-body"]["PostToolUse"][0]["hooks"][0]["command"],
            "agent-body hooks bridge post_tool"
        );
        assert_eq!(
            parsed["agent-body"]["SessionStart"][0]["command"],
            "agent-body hooks bridge session_start"
        );
        assert_eq!(
            parsed["agent-body"]["PreInvocation"][0]["command"],
            "agent-body hooks bridge pre_invocation"
        );
        assert_eq!(
            parsed["agent-body"]["Stop"][0]["command"],
            "agent-body hooks bridge stop"
        );
        assert!(parsed.get("token-optimizer").is_none());
        assert_eq!(res.companion_files.len(), 3);
        assert_eq!(
            res.companion_files[0].relative_path,
            ".gemini/config/hooks/body_bridge.py"
        );
        assert!(res.companion_files[0].content.contains("extract_failure_text"));
        assert!(res.companion_files[0].content.contains("--error-output"));
        assert!(res.companion_files[0].content.contains("is_destructive_or_sensitive_command"));
        assert!(res.companion_files[0].content.contains("handle_pre_invocation"));
        assert!(res.companion_files[0].content.contains("handle_stop"));
        assert!(res.companion_files[0].executable);
        assert_eq!(
            res.companion_files[1].relative_path,
            ".gemini/config/hooks/token_optimizer.py"
        );
        assert!(res.companion_files[1].content.contains("handle_read_file"));
        assert!(res.companion_files[1].executable);
        assert_eq!(res.companion_files[2].relative_path, ".ripgreprc");
        assert!(res.companion_files[2].content.contains("--max-columns=300"));
        assert!(!res.companion_files[2].executable);
        assert!(res.written_path.is_none());
        assert!(res.is_dry_run);
    }

    #[test]
    fn test_generate_agy_write_to_directory() {
        let dir = tempdir().unwrap();
        let res = ClientHookGenerator::generate(ClientType::Agy, Some(dir.path()), false).unwrap();
        assert_eq!(res.client, "agy");
        assert!(res.written_path.is_some());
        assert_eq!(res.companion_files.len(), 3);
        assert!(res.companion_files[0].written_path.is_some());
        assert!(res.companion_files[1].written_path.is_some());
        assert!(res.companion_files[2].written_path.is_some());

        let hooks_file = dir.path().join(".gemini/config/hooks.json");
        let bridge_file = dir.path().join(".gemini/config/hooks/body_bridge.py");
        let opt_file = dir.path().join(".gemini/config/hooks/token_optimizer.py");
        let rg_file = dir.path().join(".ripgreprc");
        assert!(hooks_file.exists());
        assert!(bridge_file.exists());
        assert!(opt_file.exists());
        assert!(rg_file.exists());

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&hooks_file).unwrap()).unwrap();
        assert_eq!(parsed["agent-body"]["enabled"], true);
        assert_eq!(
            parsed["agent-body"]["PreToolUse"][0]["hooks"][0]["command"],
            "agent-body hooks bridge pre_tool"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&bridge_file).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "body_bridge.py 必须具有可执行权限");
            let opt_mode = fs::metadata(&opt_file).unwrap().permissions().mode();
            assert_eq!(opt_mode & 0o111, 0o111, "token_optimizer.py 必须具有可执行权限");
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

    #[test]
    fn test_generate_codex_hermes_and_claude_settings() {
        let dir = tempdir().unwrap();

        let claude = ClientHookGenerator::generate(ClientType::Claude, Some(dir.path()), false).unwrap();
        assert_eq!(claude.relative_path, "CLAUDE.md");
        assert_eq!(claude.companion_files.len(), 2);
        let settings_path = dir.path().join(".claude/settings.json");
        let bridge_path = dir.path().join(".claude/hooks/body_claude_bridge.py");
        assert!(settings_path.exists());
        assert!(bridge_path.exists());
        let settings_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&settings_path).unwrap()).expect("valid claude settings.json");
        assert!(settings_json["hooks"]["PreToolUse"].is_array());
        assert!(settings_json["hooks"]["PostToolUse"].is_array());

        let codex = ClientHookGenerator::generate(ClientType::Codex, Some(dir.path()), false).unwrap();
        assert_eq!(codex.relative_path, "AGENTS.md");
        assert!(dir.path().join("AGENTS.md").exists());
        assert!(codex.content.contains("agent-body brief"));

        let hermes = ClientHookGenerator::generate(ClientType::Hermes, Some(dir.path()), false).unwrap();
        assert_eq!(hermes.relative_path, ".hermes/agent-body-rules.md");
        assert_eq!(hermes.companion_files.len(), 1);
        assert!(dir.path().join(".hermes/agent-body-rules.md").exists());
        assert!(dir.path().join(".hermes/hooks/body_bridge.py").exists());
    }
}
