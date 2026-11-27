use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use super::StewardEvent;

/// 钩子动作类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum HookAction {
    /// 执行本机命令或脚本 (如 command = "steward asset scan {path}")
    Exec {
        command: String,
    },
    /// 向外部 HTTP 端点投递 Webhook
    Webhook {
        url: String,
        #[serde(default = "default_webhook_timeout_ms")]
        timeout_ms: u64,
    },
}

fn default_webhook_timeout_ms() -> u64 {
    3000
}

/// 钩子多维触发过滤器
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct HookFilter {
    /// 路径前缀匹配
    pub path_prefix: Option<String>,
    /// 文件后缀匹配列表 (例如 [".md", ".rs"])
    pub extensions: Option<Vec<String>>,
    /// 包含的标签列表 (适用于 memory 等事件)
    pub tags_contain: Option<Vec<String>>,
}

impl HookFilter {
    /// 评估事件是否满足过滤规则
    pub fn matches(&self, event: &StewardEvent) -> bool {
        // 1. 检查路径前缀
        if let Some(ref prefix) = self.path_prefix {
            if let Some(path) = event.target_path() {
                if !path.starts_with(prefix) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // 2. 检查扩展名
        if let Some(ref exts) = self.extensions {
            if let Some(path) = event.target_path() {
                let matches_ext = exts.iter().any(|ext| path.ends_with(ext));
                if !matches_ext {
                    return false;
                }
            } else {
                return false;
            }
        }

        // 3. 检查记忆标签
        if let Some(ref filter_tags) = self.tags_contain {
            let event_tags = match event {
                StewardEvent::MemoryAdded { tags, .. } | StewardEvent::MemoryUpdated { tags, .. } => tags,
                _ => return false,
            };
            let has_match = filter_tags.iter().any(|ft| event_tags.contains(ft));
            if !has_match {
                return false;
            }
        }

        true
    }
}

/// 单条钩子规则
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HookRule {
    /// 事件类型 (如 "on_file_created", "on_memory_added", "*" 表示通配)
    pub event: String,
    /// 过滤条件 (可选)
    #[serde(default)]
    pub filter: Option<HookFilter>,
    /// 触发的具体动作
    #[serde(flatten)]
    pub action: HookAction,
}

/// POSIX Shell 单引号安全转义：将任意文本包裹为安全的单引号字面量，防止 sh -c 命令注入
pub fn shell_escape_posix(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    out.push('\'');
    for ch in input.chars() {
        if ch == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

/// 安全插值 Hook 执行命令（拒绝控制字符，并对 `{path}` / `{event}` 执行 POSIX 单引号转义）
pub fn interpolate_hook_command(template: &str, event: &StewardEvent) -> anyhow::Result<String> {
    let mut result = template.to_string();
    if let Some(path) = event.target_path() {
        if path.chars().any(|c| c == '\0' || c == '\n' || c == '\r') {
            anyhow::bail!("安全拦截：Hook 目标路径包含非法控制字符，已拒绝插值执行");
        }
        let escaped_path = shell_escape_posix(path);
        result = result.replace("{path}", &escaped_path);
    }
    let escaped_event = shell_escape_posix(event.event_type());
    result = result.replace("{event}", &escaped_event);
    Ok(result)
}

/// 钩子分发器
pub struct HookDispatcher {
    rules: Vec<HookRule>,
    blacklist_patterns: Vec<String>,
}

impl HookDispatcher {
    pub fn new(rules: Vec<HookRule>, blacklist_patterns: Vec<String>) -> Self {
        Self {
            rules,
            blacklist_patterns,
        }
    }

    /// 评估路径是否命中黑名单目录（规约 #6 白黑名单深度法则：黑名单目录严禁触发外部钩子）
    pub fn is_blacklisted(&self, path_str: &str) -> bool {
        let p = Path::new(path_str);
        for comp in p.components() {
            let comp_str = comp.as_os_str().to_string_lossy();
            for pattern in &self.blacklist_patterns {
                if pattern.trim_matches('/').trim_matches('*') == comp_str {
                    return true;
                }
            }
        }
        false
    }

    /// 针对特定事件进行分发处理 (异步非阻塞触发)
    pub async fn dispatch(&self, event: Arc<StewardEvent>) -> Vec<String> {
        let mut executed_actions = Vec::new();

        // 检查黑名单
        if let Some(target) = event.target_path() {
            if self.is_blacklisted(target) {
                return executed_actions;
            }
        }

        let event_type_str = event.event_type();

        for rule in &self.rules {
            if rule.event != "*" && rule.event != event_type_str {
                continue;
            }

            if let Some(ref filter) = rule.filter {
                if !filter.matches(&event) {
                    continue;
                }
            }

            // 执行 action
            match &rule.action {
                HookAction::Exec { command } => {
                    let Ok(cmd_interpolated) = interpolate_hook_command(command, &event) else {
                        continue;
                    };
                    executed_actions.push(format!("exec: {}", cmd_interpolated));

                    // 异步在后台执行，避免阻塞
                    let cmd_to_run = cmd_interpolated.clone();
                    tokio::task::spawn_blocking(move || {
                        let _wait = crate::body::WaitEventGuard::enter(
                            crate::body::WaitEventId::IpcHookSubprocess,
                        );
                        if let Err(e) = Command::new("sh")
                            .arg("-c")
                            .arg(&cmd_to_run)
                            .output()
                        {
                            eprintln!("[Hooks][WARN] 异步执行 Hook 命令失败: {}", e);
                        }
                    });
                }
                HookAction::Webhook { url, timeout_ms } => {
                    executed_actions.push(format!("webhook: {}", url));
                    let url_cloned = url.clone();
                    let ev_cloned = (*event).clone();
                    let to_ms = *timeout_ms;

                    // 异步发出 Webhook HTTP POST (基于 WebhookPusher 指数退避重试通道)
                    tokio::spawn(async move {
                        let pusher = super::push::WebhookPusher::default();
                        let report = pusher.send_event(&url_cloned, &ev_cloned, to_ms).await;
                        if !report.success {
                            eprintln!(
                                "[Hooks][WARN] Webhook 推送 {} 失败 (attempts={}): {}",
                                url_cloned,
                                report.attempts,
                                report.error_message.as_deref().unwrap_or("unknown error")
                            );
                        }
                    });
                }
            }
        }

        executed_actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Local;

    #[test]
    fn test_blacklist_filtering() {
        let dispatcher = HookDispatcher::new(
            vec![],
            vec![".git".to_string(), "node_modules".to_string(), ".cache".to_string()],
        );

        assert!(dispatcher.is_blacklisted("/root/new/work/.git/COMMIT_EDITMSG"));
        assert!(dispatcher.is_blacklisted("/root/new/work/frontend/node_modules/pkg/index.js"));
        assert!(!dispatcher.is_blacklisted("/root/new/work/my_project/src/main.rs"));
    }

    #[tokio::test]
    async fn test_dispatcher_matching_and_filter() {
        let rule = HookRule {
            event: "on_file_created".to_string(),
            filter: Some(HookFilter {
                path_prefix: Some("/root/new/work".to_string()),
                extensions: Some(vec![".md".to_string()]),
                tags_contain: None,
            }),
            action: HookAction::Exec {
                command: "echo file {path}".to_string(),
            },
        };

        let dispatcher = HookDispatcher::new(vec![rule], vec!["target".to_string()]);

        // 1. 匹配成功 (.md + /root/new/work)
        let ev1 = Arc::new(StewardEvent::FileCreated {
            path: "/root/new/work/notes.md".to_string(),
            source: "agent".to_string(),
            timestamp: Local::now().to_rfc3339(),
        });
        let actions1 = dispatcher.dispatch(ev1).await;
        assert_eq!(actions1.len(), 1);
        assert_eq!(actions1[0], "exec: echo file '/root/new/work/notes.md'");

        // 2. 扩展名不匹配 (.rs)
        let ev2 = Arc::new(StewardEvent::FileCreated {
            path: "/root/new/work/main.rs".to_string(),
            source: "agent".to_string(),
            timestamp: Local::now().to_rfc3339(),
        });
        let actions2 = dispatcher.dispatch(ev2).await;
        assert!(actions2.is_empty());

        // 3. 黑名单目录被阻断
        let ev3 = Arc::new(StewardEvent::FileCreated {
            path: "/root/new/work/target/out.md".to_string(),
            source: "agent".to_string(),
            timestamp: Local::now().to_rfc3339(),
        });
        let actions3 = dispatcher.dispatch(ev3).await;
        assert!(actions3.is_empty());
    }

    #[tokio::test]
    async fn test_dispatcher_escapes_shell_injection_in_path() {
        let marker = std::env::temp_dir().join(format!("steward_pwn_{}", uuid::Uuid::new_v4()));
        let malicious_path = format!("/root/new/work/evil'; touch {}; echo '.md", marker.display());
        let ev = StewardEvent::FileCreated {
            path: malicious_path,
            source: "test".to_string(),
            timestamp: Local::now().to_rfc3339(),
        };
        let cmd = interpolate_hook_command("echo {path}", &ev).expect("should escape");
        let out = Command::new("sh").arg("-c").arg(&cmd).output().expect("run sh");
        assert!(out.status.success());
        assert!(!marker.exists(), "恶意注入的 touch 命令绝不能被执行！");

        // 含换行控制字符应被直接拒绝
        let ev_nl = StewardEvent::FileCreated {
            path: "/root/new/work/bad\ntouch /tmp/x.md".to_string(),
            source: "test".to_string(),
            timestamp: Local::now().to_rfc3339(),
        };
        assert!(interpolate_hook_command("echo {path}", &ev_nl).is_err());
    }
}
