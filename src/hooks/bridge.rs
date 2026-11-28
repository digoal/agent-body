//! 原生 Rust AGY 生命周期 Hook 单入口桥接器 (`agent-body hooks bridge <event>`)
//!
//! 统一承接 `session_start`、`pre_tool`、`post_tool`、`pre_invocation`、`stop` 五大事件，
//! 对零 DB 事件（`pre_invocation`/`stop`/纯放行 `pre_tool`/常规 `post_tool`）走极速快路径。

use std::fs;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::Result;
use serde_json::{json, Value};

use crate::api::hooks::AgentHookEngine;
use crate::brief::{BriefAssembler, BriefRequest};
use crate::config::{Config, RuntimeMode};
use crate::hooks::{TokenGuardDecision, TokenGuardEngine, TokenGuardPolicy};
use crate::storage::Storage;

const FAILURE_SIGNATURES: &[&str] = &[
    "Connection refused",
    "ECONNREFUSED",
    "could not connect to server",
    "password authentication failed",
    "command not found",
    "Address already in use",
    "EADDRINUSE",
];

const DESTRUCTIVE_PATTERNS: &[&str] = &[
    "rm -rf / ",
    "rm -rf /*",
    "rm -fr / ",
    "rm -fr /*",
    "mkfs.",
    "dd if=",
    "chmod -r 777 /",
    ":(){ :|:& };:",
];

const CRITICAL_PATHS: &[&str] = &[
    "/etc/shadow",
    "/etc/gshadow",
    "/etc/sudoers",
    "/etc/passwd",
    "/.ssh/",
    "~/.ssh/",
    "/.gnupg/",
    "/.agent-body/body.db",
];

/// CLI 零 DB 优先分发入口（仅在必要时懒加载 SQLite `Storage`）
pub fn handle_cli_bridge(event: &str, cfg: &Config) -> Result<()> {
    let mut raw_stdin = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut raw_stdin) {
        eprintln!("[HooksBridge][WARN] 读取 stdin 失败: {}", e);
    }
    let out = evaluate_cli_bridge_with_loader(event, &raw_stdin, cfg, || Storage::from_config(cfg))?;
    println!("{}", serde_json::to_string(&out)?);
    Ok(())
}

/// 已持有 `Storage` 时的桥接入口
pub fn handle_bridge_with_storage(
    event: &str,
    raw_stdin: &str,
    cfg: &Config,
    storage: &Storage,
) -> Result<Value> {
    let ev = event.trim().to_ascii_lowercase();
    if cfg.effective_runtime_mode() == RuntimeMode::Bypass {
        crate::body::ShadowAbEngine::record_bypass_event(
            storage,
            "hook_bridge",
            &ev,
            "hook-bridge",
            "",
        );
        return Ok(bypass_response(&ev));
    }
    let payload: Value = serde_json::from_str(raw_stdin.trim()).unwrap_or_else(|_| json!({}));
    match ev.as_str() {
        "pre_invocation" => Ok(handle_pre_invocation(&payload)),
        "stop" => Ok(handle_stop(&payload)),
        "session_start" => handle_session_start(cfg, storage),
        "pre_tool" => Ok(handle_pre_tool_with_storage(&payload, cfg, storage)),
        "post_tool" => Ok(handle_post_tool_with_storage(&payload, storage)),
        _ => Ok(json!({})),
    }
}

fn evaluate_cli_bridge_with_loader<F>(
    event: &str,
    raw_stdin: &str,
    cfg: &Config,
    load_storage: F,
) -> Result<Value>
where
    F: FnOnce() -> Result<Storage>,
{
    let ev = event.trim().to_ascii_lowercase();
    if cfg.effective_runtime_mode() == RuntimeMode::Bypass {
        return Ok(bypass_response(&ev));
    }

    let payload: Value = serde_json::from_str(raw_stdin.trim()).unwrap_or_else(|_| json!({}));
    match ev.as_str() {
        "pre_invocation" => Ok(handle_pre_invocation(&payload)),
        "stop" => Ok(handle_stop(&payload)),
        "pre_tool" => {
            if let Some(fast) = try_pre_tool_fast(&payload, cfg) {
                return Ok(fast);
            }
            let storage = load_storage()?;
            Ok(handle_pre_tool_with_storage(&payload, cfg, &storage))
        }
        "post_tool" => {
            if let Some(fast) = try_post_tool_fast(&payload) {
                return Ok(fast);
            }
            let storage = load_storage()?;
            Ok(handle_post_tool_with_storage(&payload, &storage))
        }
        "session_start" => {
            let storage = load_storage()?;
            handle_session_start(cfg, &storage)
        }
        _ => Ok(json!({})),
    }
}

fn bypass_response(event: &str) -> Value {
    if event == "pre_tool" {
        json!({"decision": "allow"})
    } else {
        json!({})
    }
}

fn try_pre_tool_fast(payload: &Value, cfg: &Config) -> Option<Value> {
    let tc = payload.get("toolCall")?;
    let tname = tc.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let args = tc.get("args").filter(|v| v.is_object())?;
    let lower = tname.trim().to_ascii_lowercase();

    match lower.as_str() {
        "write_file" | "edit_file" => {
            let path = args
                .get("Path")
                .or_else(|| args.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if is_noise_path(path) {
                return Some(json!({"decision": "allow"}));
            }
            None
        }
        "read_file" | "view_file" | "read" | "grep_search" | "find_by_name" | "grep" | "glob" => {
            let policy = TokenGuardPolicy::from_config(cfg);
            match TokenGuardEngine::evaluate(tname, args, &policy) {
                TokenGuardDecision::Pass => Some(json!({"decision": "allow"})),
                _ => None,
            }
        }
        "run_command" | "bash" | "exec" => {
            let cmd = args
                .get("CommandLine")
                .or_else(|| args.get("command"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if cmd.trim().is_empty() {
                return Some(json!({"decision": "allow"}));
            }
            if extract_listen_port(cmd).is_some() || is_destructive_or_sensitive_command(cmd) {
                return None;
            }
            let policy = TokenGuardPolicy::from_config(cfg);
            match TokenGuardEngine::evaluate(tname, args, &policy) {
                TokenGuardDecision::Pass => Some(json!({"decision": "allow"})),
                _ => None,
            }
        }
        _ => Some(json!({"decision": "allow"})),
    }
}

fn handle_pre_tool_with_storage(payload: &Value, cfg: &Config, storage: &Storage) -> Value {
    let tc = payload.get("toolCall").cloned().unwrap_or_else(|| json!({}));
    let tname = tc.get("name").and_then(|v| v.as_str()).unwrap_or("run_command");
    let mut args = tc.get("args").cloned().unwrap_or_else(|| json!({}));
    if !args.is_object() {
        return json!({"decision": "allow"});
    }

    let lower = tname.trim().to_ascii_lowercase();
    if matches!(lower.as_str(), "write_file" | "edit_file") {
        let path = args
            .get("Path")
            .or_else(|| args.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if is_noise_path(&path) {
            return json!({"decision": "allow"});
        }
        if args.get("path").is_none() {
            if let Some(obj) = args.as_object_mut() {
                obj.insert("path".to_string(), json!(path));
            }
        }
    } else if matches!(lower.as_str(), "run_command" | "bash" | "exec") {
        let cmd = args
            .get("CommandLine")
            .or_else(|| args.get("command"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if let Some(port) = extract_listen_port(&cmd) {
            if args.get("port").is_none() {
                if let Some(obj) = args.as_object_mut() {
                    obj.insert("port".to_string(), json!(port));
                }
            }
        }
        if args.get("command").is_none() {
            if let Some(obj) = args.as_object_mut() {
                obj.insert("command".to_string(), json!(cmd));
            }
        }
    }

    let engine = AgentHookEngine::new(storage.clone());
    let res = engine.pre_tool_call_with_runtime(
        tname,
        &args,
        cfg.effective_runtime_mode(),
        &cfg.runtime.shadow_primary,
    );

    if !res.allowed {
        let reason = res
            .warnings
            .into_iter()
            .chain(res.suggestions)
            .collect::<Vec<_>>()
            .join(" ; ");
        let msg = if reason.is_empty() {
            "[agent-body 阻断] 命中安全/协作锁策略".to_string()
        } else {
            format!("[agent-body 阻断] {}", reason)
        };
        return json!({ "decision": "deny", "reason": msg });
    }
    if res.has_warnings {
        let reason = res
            .warnings
            .into_iter()
            .chain(res.suggestions)
            .collect::<Vec<_>>()
            .join(" ; ");
        return json!({
            "decision": "ask",
            "reason": format!("[agent-body 风险预警] {}", reason)
        });
    }
    if let Some(ow) = res.overwrite {
        return json!({ "decision": "allow", "overwrite": ow });
    }
    json!({ "decision": "allow" })
}

fn try_post_tool_fast(payload: &Value) -> Option<Value> {
    if extract_failure_text(payload).is_some() {
        return None;
    }
    let tc = payload.get("toolCall")?;
    let tname = tc.get("name").and_then(|v| v.as_str()).unwrap_or("");
    if !matches!(tname, "write_file" | "edit_file") {
        return Some(json!({}));
    }
    let args = tc.get("args")?;
    let path = args
        .get("Path")
        .or_else(|| args.get("path"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if is_noise_path(path) {
        return Some(json!({}));
    }
    spawn_async_post_sync(tname, path);
    Some(json!({}))
}

fn handle_post_tool_with_storage(payload: &Value, storage: &Storage) -> Value {
    let tc = payload.get("toolCall").cloned().unwrap_or_else(|| json!({}));
    let tname = tc.get("name").and_then(|v| v.as_str()).unwrap_or("run_command");
    let raw_args = tc.get("args").cloned().unwrap_or_else(|| json!({}));

    if let Some(err_text) = extract_failure_text(payload) {
        let mut body_args = serde_json::Map::new();
        if let Some(cmd) = raw_args
            .get("CommandLine")
            .or_else(|| raw_args.get("command"))
            .and_then(|v| v.as_str())
        {
            if !cmd.is_empty() {
                body_args.insert("command".to_string(), json!(cmd));
            }
        }
        if let Some(path) = raw_args
            .get("Path")
            .or_else(|| raw_args.get("path"))
            .and_then(|v| v.as_str())
        {
            if !path.is_empty() {
                body_args.insert("path".to_string(), json!(path));
            }
        }
        let engine = AgentHookEngine::new(storage.clone());
        let res = engine.post_tool_call_with_error(
            tname,
            &Value::Object(body_args),
            false,
            Some(&err_text),
        );
        if let Some(rx) = res.recovery_prescription {
            let mut hint = format!(
                "[agent-body AOPCC 自愈处方 | {}] {}",
                rx.failure_kind, rx.root_cause
            );
            if !rx.recovery_actions.is_empty() {
                let actions: Vec<String> = rx.recovery_actions.into_iter().take(3).collect();
                hint.push_str(" | 建议动作: ");
                hint.push_str(&actions.join(" ; "));
            }
            write_rx_cache(payload, &hint);
        }
        return json!({});
    }

    if matches!(tname, "write_file" | "edit_file") {
        let path = raw_args
            .get("Path")
            .or_else(|| raw_args.get("path"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !is_noise_path(path) {
            let engine = AgentHookEngine::new(storage.clone());
            let _res = engine.post_tool_call_with_error(
                tname,
                &json!({ "path": path }),
                true,
                None,
            );
        }
    }
    json!({})
}

fn handle_session_start(cfg: &Config, storage: &Storage) -> Result<Value> {
    let provider = cfg.build_memory_provider(storage.clone());
    let assembler = BriefAssembler::new(storage.clone(), Some(provider));
    let out = assembler.assemble(&BriefRequest::default())?;
    let truncated: String = out.markdown.chars().take(2500).collect();
    Ok(json!({
        "injectSteps": [{
            "systemMessage": {
                "systemMessage": format!("[Agent Body 自动开工简报]\n{}", truncated)
            }
        }]
    }))
}

fn handle_pre_invocation(payload: &Value) -> Value {
    let path = rx_cache_path(payload);
    if !path.exists() {
        return json!({});
    }
    let hint = fs::read_to_string(&path).unwrap_or_default();
    if let Err(e) = fs::remove_file(&path) {
        eprintln!("[HooksBridge][WARN] 清理处方缓存失败 ({:?}): {}", path, e);
    }
    let trimmed = hint.trim();
    if trimmed.is_empty() {
        return json!({});
    }
    json!({
        "injectSteps": [{
            "systemMessage": {
                "systemMessage": trimmed
            }
        }]
    })
}

fn handle_stop(payload: &Value) -> Value {
    let reason = payload
        .get("terminationReason")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let conv_id = payload
        .get("conversationId")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");
    if reason == "NO_TOOL_CALL" && !conv_id.is_empty() {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("agent-body"));
        if let Err(e) = Command::new(exe)
            .args(["close", conv_id, "--clean-temp=false"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            eprintln!("[HooksBridge][WARN] 后台派生 close 失败: {}", e);
        }
    }
    json!({})
}

fn spawn_async_post_sync(tool_name: &str, path: &str) {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("agent-body"));
    let args_json = json!({ "path": path }).to_string();
    if let Err(e) = Command::new(exe)
        .args(["hooks", "post", "-t", tool_name, "-a", &args_json, "--success"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        eprintln!("[HooksBridge][WARN] 后台派生 post_sync 失败: {}", e);
    }
}

fn rx_cache_path(payload: &Value) -> PathBuf {
    let conv_id = payload
        .get("conversationId")
        .and_then(|v| v.as_str())
        .unwrap_or("default");
    let safe_id: String = conv_id
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
    let final_id = if safe_id.is_empty() {
        "default".to_string()
    } else {
        safe_id
    };
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home)
        .join(".agent-body")
        .join("tmp")
        .join(format!("agy_rx_{}.txt", final_id))
}

fn write_rx_cache(payload: &Value, hint: &str) {
    let path = rx_cache_path(payload);
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            eprintln!("[HooksBridge][WARN] 创建处方缓存目录失败 ({:?}): {}", parent, e);
            return;
        }
    }
    if let Err(e) = fs::write(&path, hint) {
        eprintln!("[HooksBridge][WARN] 写入处方缓存失败 ({:?}): {}", path, e);
    }
}

fn is_noise_path(path: &str) -> bool {
    if path.trim().is_empty() {
        return true;
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let home_gemini = if home.is_empty() {
        String::new()
    } else {
        format!("{}/.gemini/", home.trim_end_matches('/'))
    };
    let home_body = if home.is_empty() {
        String::new()
    } else {
        format!("{}/.agent-body/", home.trim_end_matches('/'))
    };
    path.starts_with("/tmp/")
        || path.starts_with("/var/tmp/")
        || path.starts_with("/root/.gemini/")
        || path.starts_with("/root/.agent-body/")
        || (!home_gemini.is_empty() && path.starts_with(&home_gemini))
        || (!home_body.is_empty() && path.starts_with(&home_body))
}

fn is_destructive_or_sensitive_command(cmd: &str) -> bool {
    let norm = format!(
        "{} ",
        cmd.split_whitespace().collect::<Vec<_>>().join(" ").to_ascii_lowercase()
    );
    DESTRUCTIVE_PATTERNS.iter().any(|p| norm.contains(p))
        || CRITICAL_PATHS.iter().any(|p| norm.contains(p))
}

fn extract_listen_port(cmd: &str) -> Option<u16> {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    let is_server_cmd = tokens.iter().any(|t| {
        matches!(
            *t,
            "http.server"
                | "uvicorn"
                | "gunicorn"
                | "vite"
                | "webpack-dev-server"
                | "pg_ctl"
                | "nc"
                | "ncat"
                | "socat"
                | "serve"
        )
    }) || cmd.contains("flask run")
        || cmd.contains("streamlit run")
        || cmd.contains("next dev")
        || cmd.contains("nuxt dev")
        || cmd.contains("astro dev");

    if !is_server_cmd {
        return None;
    }
    for pair in tokens.windows(2) {
        if matches!(pair[0], "--port" | "-p" | "-l" | "--listen" | "http.server") {
            let candidate = pair[1].rsplit(':').next().unwrap_or(pair[1]);
            if let Ok(port) = candidate.parse::<u16>() {
                if port >= 1 {
                    return Some(port);
                }
            }
        }
    }
    for tok in &tokens {
        if let Some(val) = tok.strip_prefix("--port=").or_else(|| tok.strip_prefix("-p=")) {
            if let Ok(port) = val.parse::<u16>() {
                if port >= 1 {
                    return Some(port);
                }
            }
        }
    }
    None
}

fn extract_failure_text(payload: &Value) -> Option<String> {
    if let Some(err_obj) = payload.get("error") {
        if !err_obj.is_null() {
            let txt = err_obj
                .as_str()
                .map(|s| s.to_string())
                .or_else(|| err_obj.get("message").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .or_else(|| err_obj.get("error").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .unwrap_or_else(|| err_obj.to_string());
            if !txt.trim().is_empty() {
                return Some(txt.chars().take(800).collect());
            }
        }
    }
    for key in ["toolResult", "response", "result", "output"] {
        let Some(val) = payload.get(key) else {
            continue;
        };
        if let Some(obj) = val.as_object() {
            let code_val = obj
                .get("exitCode")
                .or_else(|| obj.get("returncode"))
                .or_else(|| obj.get("exit_code"));
            let nonzero = code_val
                .and_then(|v| v.as_i64())
                .map(|n| n != 0)
                .unwrap_or(false);
            let txt = obj
                .get("stderr")
                .or_else(|| obj.get("output"))
                .or_else(|| obj.get("error"))
                .or_else(|| obj.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if nonzero || FAILURE_SIGNATURES.iter().any(|sig| txt.contains(sig)) {
                let out = if txt.is_empty() {
                    format!("exitCode={}", code_val.map(|v| v.to_string()).unwrap_or_default())
                } else {
                    txt.chars().take(800).collect()
                };
                return Some(out);
            }
        } else if let Some(s) = val.as_str() {
            if FAILURE_SIGNATURES.iter().any(|sig| s.contains(sig)) {
                return Some(s.chars().take(800).collect());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn test_bridge_pre_tool_fast_pass_and_distill_overwrite() {
        let cfg = Config::default();

        // 简单命令走零 DB 快路径
        let db_loaded = Arc::new(AtomicBool::new(false));
        let flag = db_loaded.clone();
        let res_pass = evaluate_cli_bridge_with_loader(
            "pre_tool",
            r#"{"toolCall":{"name":"run_command","args":{"CommandLine":"git status && ls -la"}}}"#,
            &cfg,
            move || {
                flag.store(true, Ordering::SeqCst);
                Storage::open_in_memory()
            },
        )
        .expect("pre_tool pass");
        assert_eq!(res_pass["decision"], "allow");
        assert!(res_pass.get("overwrite").is_none());
        assert!(!db_loaded.load(Ordering::SeqCst), "纯放行不应打开 SQLite");

        // 复合 cargo test 命令命中 distill overwrite
        let res_distill = evaluate_cli_bridge_with_loader(
            "pre_tool",
            r#"{"toolCall":{"name":"run_command","args":{"CommandLine":"cargo test && make lint-hygiene"}}}"#,
            &cfg,
            Storage::open_in_memory,
        )
        .expect("pre_tool distill");
        assert_eq!(res_distill["decision"], "allow");
        let cmd = res_distill["overwrite"]["CommandLine"].as_str().unwrap_or("");
        assert!(cmd.starts_with("agent-body distill --kind build_test"));
    }

    #[test]
    fn test_bridge_post_tool_error_caches_rx_and_pre_invocation_consumes() {
        let cfg = Config::default();
        let conv = format!("test-bridge-{}", std::process::id());
        let post_payload = json!({
            "conversationId": conv,
            "toolCall": {
                "name": "run_command",
                "args": { "CommandLine": "psql -p 5432" }
            },
            "error": "psql: error: connection to server at \"127.0.0.1\", port 5432 failed: Connection refused"
        })
        .to_string();

        let post_res = evaluate_cli_bridge_with_loader(
            "post_tool",
            &post_payload,
            &cfg,
            Storage::open_in_memory,
        )
        .expect("post_tool");
        assert_eq!(post_res, json!({}));

        let pre_inv_payload = json!({ "conversationId": conv }).to_string();
        let pre_inv_res = evaluate_cli_bridge_with_loader(
            "pre_invocation",
            &pre_inv_payload,
            &cfg,
            || panic!("pre_invocation 不应打开 SQLite"),
        )
        .expect("pre_invocation");
        let msg = pre_inv_res["injectSteps"][0]["systemMessage"]["systemMessage"]
            .as_str()
            .unwrap_or("");
        assert!(msg.contains("AOPCC"), "应回注 AOPCC 自愈处方: {}", msg);

        // 二次消费应为空（缓存文件已删除）
        let second = evaluate_cli_bridge_with_loader(
            "pre_invocation",
            &pre_inv_payload,
            &cfg,
            || panic!("pre_invocation 不应打开 SQLite"),
        )
        .expect("second pre_invocation");
        assert_eq!(second, json!({}));
    }

    #[test]
    fn test_bridge_stop_returns_empty_json_without_db() {
        let cfg = Config::default();
        let res = evaluate_cli_bridge_with_loader(
            "stop",
            r#"{"terminationReason":"MAX_INVOCATIONS","conversationId":""}"#,
            &cfg,
            || panic!("stop 不应打开 SQLite"),
        )
        .expect("stop");
        assert_eq!(res, json!({}));
    }
}
