//! AOPCC 三态并发分类器 (`ConcurrencyPolicyClassifier`) 与碰壁秒级自愈诊断器 (`OptimisticFailureDiagnoser`)
//!
//! 落地设计规格 `15-adaptive-optimistic-pessimistic-concurrency-spec.md`：
//! 1. **三态并发路由 (`ActionConcurrencyMode`)**：
//!    - `OPTIMISTIC`：只读探测、客户端连接（如 `psql -p 5432` / `curl`）、已授权 CLI、`/root/new/tmp/` 验证——零前置门禁阻塞；
//!    - `CAUTIOUS_CAS`：常规工作区文件新建/编辑——静默本地放置规约校验；
//!    - `PESSIMISTIC_LOCK`：服务端端口绑定、高危系统路径、破坏性命令、服务启停、协作锁资源——强制前置悲观拦截。
//! 2. **PostToolUse 碰壁秒级自愈 (`< 2ms` OCC Abort Recovery)**：
//!    针对 `ECONNREFUSED`、`password authentication failed`、`command not found`、`EADDRINUSE`
//!    四类高频乐观碰壁场景，即时反查宿主机真实端口/工具/服务状态并注入替代处方，同时自动降级失效租约。

use std::time::Instant;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::fact::automation::AutomationTopology;
use crate::fact::capability::CapabilityEngine;
use crate::fact::process_net::ProcessNetTopology;
use crate::storage::models::ChangeRecord;
use crate::storage::Storage;

/// AOPCC 三态动作并发控制模式
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActionConcurrencyMode {
    Optimistic,
    CautiousCas,
    PessimisticLock,
}

impl ActionConcurrencyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Optimistic => "OPTIMISTIC",
            Self::CautiousCas => "CAUTIOUS_CAS",
            Self::PessimisticLock => "PESSIMISTIC_LOCK",
        }
    }
}

impl std::fmt::Display for ActionConcurrencyMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// 动作并发分类结论
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyClassification {
    pub mode: ActionConcurrencyMode,
    pub reason: String,
}

/// 乐观直行失败后的秒级自愈处方
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecoveryPrescription {
    /// `CONNECTION_REFUSED` | `AUTH_ASSUMPTION_FAILED` | `COMMAND_NOT_FOUND` | `PORT_ADDR_IN_USE`
    pub failure_kind: String,
    pub target: String,
    pub root_cause: String,
    pub recovery_actions: Vec<String>,
    pub lease_downgraded: bool,
    pub elapsed_us: u128,
}

pub struct ConcurrencyPolicyClassifier;

impl ConcurrencyPolicyClassifier {
    /// 根据工具名与调用参数确定并发模式 (`OPTIMISTIC` / `CAUTIOUS_CAS` / `PESSIMISTIC_LOCK`)
    pub fn classify(tool_name: &str, args: &Value) -> ConcurrencyClassification {
        let tool_lower = tool_name.trim().to_lowercase();

        // 1. 显式只读工具：直接乐观放行
        if is_read_only_tool(&tool_lower) {
            return ConcurrencyClassification {
                mode: ActionConcurrencyMode::Optimistic,
                reason: "只读探查动作，零副作用，采用乐观直行 (OPTIMISTIC)".to_string(),
            };
        }

        // 2. 命令行执行 (run_command / bash / exec 等)
        if let Some(cmd_str) = args
            .get("command")
            .or_else(|| args.get("cmd"))
            .and_then(|v| v.as_str())
        {
            let norm = cmd_str
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();

            if contains_destructive_pattern(&norm) {
                return ConcurrencyClassification {
                    mode: ActionConcurrencyMode::PessimisticLock,
                    reason: "高危破坏性命令（C_fail 极高），强制悲观前置拦截 (PESSIMISTIC_LOCK)"
                        .to_string(),
                };
            }

            if is_server_listen_command(&norm) {
                return ConcurrencyClassification {
                    mode: ActionConcurrencyMode::PessimisticLock,
                    reason: "检测到服务端监听端口绑定指令，强制悲观端口防撞核验 (PESSIMISTIC_LOCK)"
                        .to_string(),
                };
            }

            if is_optimistic_client_or_dev_command(&norm) {
                return ConcurrencyClassification {
                    mode: ActionConcurrencyMode::Optimistic,
                    reason: "客户端连接/开发调试/只读探测指令（可逆、低 C_fail），采用乐观直行 (OPTIMISTIC)"
                        .to_string(),
                };
            }
        }

        // 3. 显式携带端口参数（bind_port / start_server 等服务端绑定动作）
        if args.get("port").and_then(|v| v.as_u64()).is_some() {
            return ConcurrencyClassification {
                mode: ActionConcurrencyMode::PessimisticLock,
                reason: "涉及服务监听端口分配，强制悲观端口与协作锁核验 (PESSIMISTIC_LOCK)"
                    .to_string(),
            };
        }

        // 4. 服务启停操作
        if args
            .get("service")
            .or_else(|| args.get("service_name"))
            .and_then(|v| v.as_str())
            .is_some()
            || tool_lower.contains("service")
        {
            return ConcurrencyClassification {
                mode: ActionConcurrencyMode::PessimisticLock,
                reason: "涉及宿主机守护服务启停，强制悲观协作锁检查 (PESSIMISTIC_LOCK)".to_string(),
            };
        }

        // 5. 文件写入/编辑路径分级
        if let Some(path_str) = args
            .get("path")
            .or_else(|| args.get("file_path"))
            .or_else(|| args.get("target"))
            .and_then(|v| v.as_str())
        {
            if is_critical_system_path(path_str) {
                return ConcurrencyClassification {
                    mode: ActionConcurrencyMode::PessimisticLock,
                    reason: "命中高危系统/凭据路径，强制悲观安全红线拦截 (PESSIMISTIC_LOCK)"
                        .to_string(),
                };
            }
            if is_sandbox_tmp_path(path_str) {
                return ConcurrencyClassification {
                    mode: ActionConcurrencyMode::Optimistic,
                    reason: "位于临时验证沙盒 (/root/new/tmp/ 或 /tmp/)，采用乐观直写 (OPTIMISTIC)"
                        .to_string(),
                };
            }
            if tool_lower.contains("write")
                || tool_lower.contains("edit")
                || tool_lower.contains("create")
            {
                return ConcurrencyClassification {
                    mode: ActionConcurrencyMode::CautiousCas,
                    reason: "常规工作区文件变更，采用轻量条件 CAS 静态校验 (CAUTIOUS_CAS)"
                        .to_string(),
                };
            }
        }

        ConcurrencyClassification {
            mode: ActionConcurrencyMode::CautiousCas,
            reason: "常规操作采用轻量条件校验 (CAUTIOUS_CAS)".to_string(),
        }
    }
}

pub struct OptimisticFailureDiagnoser;

impl OptimisticFailureDiagnoser {
    /// 当工具乐观直行失败 (`success=false`) 时，在毫秒级内反查宿主机真实事实并注入自愈处方
    pub fn diagnose(
        storage: &Storage,
        tool_name: &str,
        args: &Value,
        error_output: &str,
    ) -> Option<FailureRecoveryPrescription> {
        let t0 = Instant::now();
        let combined_err = build_combined_error_text(args, error_output);
        if combined_err.trim().is_empty() {
            return None;
        }
        let err_lower = combined_err.to_lowercase();
        let cmd_str = args
            .get("command")
            .or_else(|| args.get("cmd"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 模式 4：端口占用冲突 (Address already in use / EADDRINUSE)
        if err_lower.contains("address already in use") || err_lower.contains("eaddrinuse") {
            let port = extract_port_hint(args, &combined_err, cmd_str).unwrap_or(8080);
            let mut actions = Vec::new();
            let root_cause = match ProcessNetTopology::check_port(port, 3) {
                Ok(res) if res.status.eq_ignore_ascii_case("occupied") => {
                    let proc_info = res
                        .occupied_by
                        .as_ref()
                        .and_then(|p| p.process_name.clone())
                        .unwrap_or_else(|| "未知进程".to_string());
                    let pid = res
                        .occupied_by
                        .as_ref()
                        .and_then(|p| p.pid)
                        .unwrap_or(0);
                    actions.push(format!(
                        "推荐直接切换空闲端口重跑: {:?}",
                        res.recommended_free_ports
                    ));
                    actions.push(format!(
                        "或运行 `agent-body env port {}` 查看完整占用上下文",
                        port
                    ));
                    format!(
                        "端口 `{}` 已被 `{}` (PID {}) 占用，导致绑定冲突",
                        port, proc_info, pid
                    )
                }
                Ok(res) => {
                    if !res.recommended_free_ports.is_empty() {
                        actions.push(format!(
                            "建议改用推荐空闲端口: {:?}",
                            res.recommended_free_ports
                        ));
                    }
                    format!("端口 `{}` 发生绑定冲突 (EADDRINUSE)", port)
                }
                Err(_) => {
                    actions.push("请使用 `--port <空闲端口>` 重试或执行 `agent-body platform ports`".to_string());
                    format!("端口 `{}` 发生绑定冲突 (EADDRINUSE)", port)
                }
            };

            return Some(FailureRecoveryPrescription {
                failure_kind: "PORT_ADDR_IN_USE".to_string(),
                target: format!("port:{}", port),
                root_cause,
                recovery_actions: actions,
                lease_downgraded: false,
                elapsed_us: t0.elapsed().as_micros(),
            });
        }

        // 模式 2：认证/权限假设失败 (如“免密直连 PG”假设失效)
        if err_lower.contains("password authentication failed")
            || err_lower.contains("no password supplied")
            || err_lower.contains("fatal: role")
            || err_lower.contains("access denied for user")
            || err_lower.contains("authentication failed")
        {
            let port_opt = extract_port_hint(args, &combined_err, cmd_str);
            let target = match port_opt {
                Some(p) => format!("port:{}", p),
                None if cmd_str.to_lowercase().contains("psql")
                    || err_lower.contains("postgres")
                    || err_lower.contains("role") =>
                {
                    "port:5432".to_string()
                }
                None => format!("auth:{}", tool_name),
            };
            let lease_downgraded = record_optimistic_miss(
                storage,
                &target,
                "AUTH_ASSUMPTION_FAILED",
                &combined_err,
            );

            let mut actions = Vec::new();
            if target.contains("5432") || err_lower.contains("postgres") || cmd_str.contains("psql") {
                actions.push(
                    "本地 peer 认证免密快捷方式: `sudo -u postgres psql -p 5432` 或显式指定 `-U postgres`"
                        .to_string(),
                );
                actions.push(
                    "如需 TCP 口令登录，可检查 `~/.pgpass` 或由 `agent-body service list` 查询凭据环境变量名"
                        .to_string(),
                );
            } else {
                actions.push(
                    "请通过 `agent-body service list` 查看已登记外部服务对应的 `auth_env_var` 环境变量"
                        .to_string(),
                );
            }

            // 查找外部服务通讯录中已脱敏登记的 auth_env_var（严禁返回任何明文密钥）
            if let Ok(svcs) = storage.list_external_services(None) {
                for s in svcs {
                    if let Some(env_var) = s.auth_env_var {
                        let sl = s.service_name.to_lowercase();
                        if err_lower.contains(&sl) || cmd_str.to_lowercase().contains(&sl) || target.contains("5432") && sl.contains("pg") {
                            actions.push(format!(
                                "检测到相关外部服务 `{}` 使用环境变量 `{}` 进行鉴权",
                                s.service_name, env_var
                            ));
                        }
                    }
                }
            }

            return Some(FailureRecoveryPrescription {
                failure_kind: "AUTH_ASSUMPTION_FAILED".to_string(),
                target,
                root_cause: "乐观假设「本地免密/默认角色直连」校验失败，服务要求合法身份认证"
                    .to_string(),
                recovery_actions: actions,
                lease_downgraded,
                elapsed_us: t0.elapsed().as_micros(),
            });
        }

        // 模式 1：服务连接拒绝 (ECONNREFUSED / Connection refused / could not connect to server)
        if err_lower.contains("connection refused")
            || err_lower.contains("econnrefused")
            || err_lower.contains("could not connect to server")
            || err_lower.contains("failed to connect")
            || err_lower.contains("is the server running")
        {
            let port = extract_port_hint(args, &combined_err, cmd_str).unwrap_or_else(|| {
                if cmd_str.contains("psql") || err_lower.contains("postgres") {
                    5432
                } else if err_lower.contains("ollama") {
                    11434
                } else if err_lower.contains("redis") {
                    6379
                } else {
                    5432
                }
            });
            let target = format!("port:{}", port);
            let lease_downgraded = record_optimistic_miss(
                storage,
                &target,
                "CONNECTION_REFUSED",
                &combined_err,
            );

            let mut actions = Vec::new();
            let active_ports = ProcessNetTopology::scan_listening_ports().unwrap_or_default();
            let target_alive = active_ports.iter().any(|p| p.port == port);
            let active_list: Vec<String> = active_ports
                .iter()
                .filter(|p| !p.is_system_daemon)
                .take(6)
                .map(|p| {
                    format!(
                        ":{} ({})",
                        p.port,
                        p.process_name.as_deref().unwrap_or("service")
                    )
                })
                .collect();

            if !active_list.is_empty() {
                actions.push(format!(
                    "宿主机当前实际在听非系统端口: {}",
                    active_list.join(", ")
                ));
            } else {
                actions.push("宿主机当前未发现其它在听应用端口，请确认对应服务是否已拉起".to_string());
            }

            if let Ok(units) = AutomationTopology::scan_systemd_services() {
                let svc_matches: Vec<String> = units
                    .into_iter()
                    .filter(|u| {
                        let nl = u.service_name.to_lowercase();
                        (port == 5432 && nl.contains("postgres"))
                            || (port == 11434 && nl.contains("ollama"))
                            || (port == 6379 && nl.contains("redis"))
                            || (port == 8765 && (nl.contains("agent-body") || nl.contains("steward")))
                    })
                    .take(2)
                    .map(|u| u.service_name)
                    .collect();
                if !svc_matches.is_empty() {
                    actions.push(format!(
                        "发现关联 systemd 单元，可运行 `systemctl status {}` 检查/拉起服务",
                        svc_matches.join(" ")
                    ));
                }
            }
            if port == 5432 && !target_alive {
                actions.push(
                    "如仅需本地轻量 SQL/分析验证，也可直接使用已就绪的 `sqlite3` / `duckdb` 或拉起临时 PG 实例"
                        .to_string(),
                );
            }

            let root_cause = if target_alive {
                format!(
                    "端口 `{}` 虽然存在 LISTEN 进程，但目标连接地址/套接字拒绝连接（请核查 bind_ip 或 unix_socket）",
                    port
                )
            } else {
                format!(
                    "目标端口 `{}` 当前在宿主机 /proc 中无监听进程（服务已离线或未启动），已同步降级该乐观租约",
                    port
                )
            };

            return Some(FailureRecoveryPrescription {
                failure_kind: "CONNECTION_REFUSED".to_string(),
                target,
                root_cause,
                recovery_actions: actions,
                lease_downgraded,
                elapsed_us: t0.elapsed().as_micros(),
            });
        }

        // 模式 3：命令/工具未找到 (command not found / exit 127 / No such file or directory)
        if err_lower.contains("command not found")
            || err_lower.contains("exit code 127")
            || err_lower.contains("exit status 127")
            || (err_lower.contains(": not found") && !cmd_str.is_empty())
            || (err_lower.contains("no such file or directory") && !cmd_str.is_empty())
        {
            let missing_bin = extract_missing_binary(&combined_err, cmd_str)
                .unwrap_or_else(|| tool_name.to_string());
            let target = format!("tool:{}", missing_bin);
            let lease_downgraded = record_optimistic_miss(
                storage,
                &target,
                "COMMAND_NOT_FOUND",
                &combined_err,
            );

            let mut cap = CapabilityEngine::new();
            cap.scan_host_paths_only();
            let search_intent = infer_capability_intent(&missing_bin, &err_lower);
            let qres = cap.query(&search_intent);

            let mut actions = Vec::new();
            for alt in qres
                .all_matching_tools
                .into_iter()
                .filter(|t| t.installed && t.tool_name != missing_bin)
                .take(3)
            {
                actions.push(format!(
                    "已安装的替代工具 `{}` (路径: {}): `{}` — {}",
                    alt.tool_name,
                    alt.path.as_deref().unwrap_or("PATH"),
                    alt.best_practice_args,
                    alt.description
                ));
            }
            if actions.is_empty() {
                actions.push(format!(
                    "可运行 `agent-body fact --fact-type capability {}` 检索宿主已安装同能力工具",
                    search_intent
                ));
            }

            return Some(FailureRecoveryPrescription {
                failure_kind: "COMMAND_NOT_FOUND".to_string(),
                target,
                root_cause: format!(
                    "乐观调用的命令/二进制 `{}` 在当前 PATH 中未找到，已推荐宿主可用替代工具",
                    missing_bin
                ),
                recovery_actions: actions,
                lease_downgraded,
                elapsed_us: t0.elapsed().as_micros(),
            });
        }

        None
    }
}

fn record_optimistic_miss(
    storage: &Storage,
    target: &str,
    failure_kind: &str,
    err_snippet: &str,
) -> bool {
    let preview: String = err_snippet.chars().take(160).collect();
    let rec = ChangeRecord {
        id: None,
        change_type: "optimistic_miss".to_string(),
        target: target.to_string(),
        description: Some(format!("AOPCC 乐观直行碰壁 ({}): {}", failure_kind, preview)),
        details: serde_json::json!({
            "failure_kind": failure_kind,
            "target": target,
        }),
        source: "hook_post".to_string(),
        created_at: None,
    };
    storage.record_change(&rec).is_ok()
}

fn build_combined_error_text(args: &Value, error_output: &str) -> String {
    let mut parts = Vec::new();
    if !error_output.trim().is_empty() {
        parts.push(error_output.trim().to_string());
    }
    for key in ["error_output", "error", "stderr"] {
        if let Some(v) = args.get(key).and_then(|v| v.as_str()) {
            if !v.trim().is_empty() {
                parts.push(v.trim().to_string());
            }
        }
    }
    parts.join("\n")
}

fn is_read_only_tool(tool_lower: &str) -> bool {
    tool_lower.starts_with("read")
        || tool_lower.starts_with("view")
        || tool_lower.starts_with("list")
        || tool_lower.starts_with("get")
        || tool_lower.starts_with("search")
        || tool_lower.starts_with("find")
        || tool_lower.starts_with("grep")
}

fn is_critical_system_path(path_str: &str) -> bool {
    let lower = path_str.replace('\\', "/").to_lowercase();
    [
        "/etc/shadow",
        "/etc/gshadow",
        "/etc/sudoers",
        "/etc/passwd",
        "/.ssh/",
        "/.gnupg/",
        "/boot/",
        "/.agent-body/body.db",
    ]
    .iter()
    .any(|p| lower.contains(p))
}

fn is_sandbox_tmp_path(path_str: &str) -> bool {
    let norm = path_str.replace('\\', "/");
    norm.starts_with("/root/new/tmp/")
        || norm == "/root/new/tmp"
        || norm.starts_with("/tmp/")
        || norm.starts_with("/var/tmp/")
}

fn contains_destructive_pattern(norm: &str) -> bool {
    let check_target = format!("{} ", norm);
    [
        "rm -rf / ",
        "rm -rf /*",
        "rm -fr / ",
        "mkfs.",
        "dd if=",
        "chmod -r 777 /",
        ":(){ :|:& };:",
    ]
    .iter()
    .any(|p| check_target.contains(p))
}

fn is_server_listen_command(norm: &str) -> bool {
    // 若命令行属于明确客户端工具，不误判为端口绑定（例如 psql -p 5432）
    if is_explicit_client_binary(norm) {
        return false;
    }
    let listen_markers = [
        "http.server",
        "uvicorn ",
        "gunicorn ",
        "flask run",
        "streamlit run",
        "next dev",
        "nuxt dev",
        "webpack-dev-server",
        "pg_ctl ",
        "redis-server",
        "agent-body web",
        "docker run",
    ];
    if listen_markers.iter().any(|m| norm.contains(m)) {
        return true;
    }
    (norm.starts_with("nc ") || norm.contains(" nc ") || norm.contains("ncat ") || norm.contains("socat "))
        && (norm.contains(" -l") || norm.contains("-lp") || norm.contains("tcp-listen"))
}

fn is_explicit_client_binary(norm: &str) -> bool {
    let first = norm.split_whitespace().next().unwrap_or("");
    let bin = first.rsplit('/').next().unwrap_or(first);
    matches!(
        bin,
        "psql"
            | "pg_isready"
            | "mysql"
            | "redis-cli"
            | "sqlite3"
            | "duckdb"
            | "curl"
            | "wget"
            | "ssh"
            | "scp"
            | "cargo"
            | "pytest"
            | "git"
            | "codegraph"
            | "pdftotext"
            | "pdftoppm"
            | "ffmpeg"
            | "ffprobe"
            | "jq"
            | "rg"
    )
}

fn is_optimistic_client_or_dev_command(norm: &str) -> bool {
    is_explicit_client_binary(norm)
        || norm.contains(" --version")
        || norm.starts_with("ls ")
        || norm.starts_with("cat ")
        || norm.starts_with("head ")
        || norm.starts_with("tail ")
        || norm.starts_with("ps ")
        || norm.starts_with("which ")
}

fn extract_port_hint(args: &Value, err_text: &str, cmd_str: &str) -> Option<u16> {
    if let Some(p) = args.get("port").and_then(|v| v.as_u64()) {
        if (1..=65535).contains(&p) {
            return Some(p as u16);
        }
    }
    let combined = format!("{} {}", err_text, cmd_str);
    let patterns = [
        r"(?i)port\s+[:=]?\s*(\d{2,5})",
        r"(?i)(?:localhost|127\.0\.0\.1|0\.0\.0\.0):(\d{2,5})",
        r"(?i)(?:--port|-p)\s*[=\s]?\s*(\d{2,5})",
    ];
    for pat in patterns {
        if let Ok(re) = regex::Regex::new(pat) {
            if let Some(cap) = re.captures(&combined) {
                if let Some(m) = cap.get(1) {
                    if let Ok(p) = m.as_str().parse::<u16>() {
                        if p >= 1 {
                            return Some(p);
                        }
                    }
                }
            }
        }
    }
    None
}

fn extract_missing_binary(err_text: &str, cmd_str: &str) -> Option<String> {
    // 典型报错：bash: wkhtmltopdf: command not found / sh: 1: jq: not found
    let patterns = [
        r"(?i)(?:bash|sh|zsh):\s*(?:\d+:\s*)?([a-zA-Z0-9_.\-]+):\s*(?:command )?not found",
        r"(?i)([a-zA-Z0-9_.\-]+):\s*command not found",
    ];
    for pat in patterns {
        if let Ok(re) = regex::Regex::new(pat) {
            if let Some(cap) = re.captures(err_text) {
                if let Some(m) = cap.get(1) {
                    return Some(m.as_str().to_string());
                }
            }
        }
    }
    let first = cmd_str.split_whitespace().next()?;
    let bin = first.rsplit('/').next().unwrap_or(first);
    if bin.is_empty() {
        None
    } else {
        Some(bin.to_string())
    }
}

fn infer_capability_intent(missing_bin: &str, err_lower: &str) -> String {
    let bl = missing_bin.to_lowercase();
    if bl.contains("pdf") || bl.contains("wkhtml") || err_lower.contains("pdf") {
        "pdf".to_string()
    } else if bl.contains("avconv") || bl.contains("mencoder") || bl.contains("ffmpeg") {
        "video".to_string()
    } else if bl.contains("ag") || bl.contains("ack") || bl.contains("rg") {
        "search".to_string()
    } else if bl.contains("jq") || bl.contains("json") {
        "json".to_string()
    } else {
        bl
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concurrency_classifier_distinguishes_client_and_server() {
        let client_psql = ConcurrencyPolicyClassifier::classify(
            "run_command",
            &serde_json::json!({"command": "psql -h 127.0.0.1 -p 5432 -c 'SELECT 1'"}),
        );
        assert_eq!(client_psql.mode, ActionConcurrencyMode::Optimistic);

        let tmp_write = ConcurrencyPolicyClassifier::classify(
            "write_file",
            &serde_json::json!({"path": "/root/new/tmp/probe.sql"}),
        );
        assert_eq!(tmp_write.mode, ActionConcurrencyMode::Optimistic);

        let work_write = ConcurrencyPolicyClassifier::classify(
            "write_file",
            &serde_json::json!({"path": "/root/new/work/app/main.rs"}),
        );
        assert_eq!(work_write.mode, ActionConcurrencyMode::CautiousCas);

        let srv_bind = ConcurrencyPolicyClassifier::classify(
            "run_command",
            &serde_json::json!({"command": "uvicorn main:app --port 8000"}),
        );
        assert_eq!(srv_bind.mode, ActionConcurrencyMode::PessimisticLock);

        let critical = ConcurrencyPolicyClassifier::classify(
            "edit_file",
            &serde_json::json!({"path": "/etc/sudoers"}),
        );
        assert_eq!(critical.mode, ActionConcurrencyMode::PessimisticLock);
    }

    #[test]
    fn test_diagnoser_handles_connrefused_auth_cmd_and_eaddrinuse() {
        let storage = Storage::open_in_memory().expect("in-memory");

        // 1. ECONNREFUSED 诊断并写回 optimistic_miss 降级标记
        let rx_conn = OptimisticFailureDiagnoser::diagnose(
            &storage,
            "run_command",
            &serde_json::json!({"command": "psql -h 127.0.0.1 -p 59999 -c 'select 1'"}),
            "psql: error: connection to server at \"127.0.0.1\", port 59999 failed: Connection refused",
        )
        .expect("应返回 ECONNREFUSED 自愈处方");
        assert_eq!(rx_conn.failure_kind, "CONNECTION_REFUSED");
        assert_eq!(rx_conn.target, "port:59999");
        assert!(rx_conn.lease_downgraded);
        assert!(!rx_conn.recovery_actions.is_empty());

        // 2. 免密假设失败诊断
        let rx_auth = OptimisticFailureDiagnoser::diagnose(
            &storage,
            "run_command",
            &serde_json::json!({"command": "psql -h 127.0.0.1 -p 5432"}),
            "psql: error: FATAL: password authentication failed for user \"root\"",
        )
        .expect("应返回 AUTH_ASSUMPTION_FAILED 自愈处方");
        assert_eq!(rx_auth.failure_kind, "AUTH_ASSUMPTION_FAILED");
        assert!(rx_auth
            .recovery_actions
            .iter()
            .any(|a| a.contains("sudo -u postgres")));

        // 3. 命令缺失诊断
        let rx_cmd = OptimisticFailureDiagnoser::diagnose(
            &storage,
            "run_command",
            &serde_json::json!({"command": "wkhtmltopdf a.html a.pdf"}),
            "bash: wkhtmltopdf: command not found",
        )
        .expect("应返回 COMMAND_NOT_FOUND 自愈处方");
        assert_eq!(rx_cmd.failure_kind, "COMMAND_NOT_FOUND");
        assert_eq!(rx_cmd.target, "tool:wkhtmltopdf");
        assert!(!rx_cmd.recovery_actions.is_empty());

        // 4. EADDRINUSE 诊断（借用实时在听端口验证推荐空闲端口）
        let rx_port = OptimisticFailureDiagnoser::diagnose(
            &storage,
            "run_command",
            &serde_json::json!({"command": "python3 -m http.server 8080"}),
            "OSError: [Errno 98] Address already in use (port 8080)",
        )
        .expect("应返回 PORT_ADDR_IN_USE 自愈处方");
        assert_eq!(rx_port.failure_kind, "PORT_ADDR_IN_USE");
        assert_eq!(rx_port.target, "port:8080");
    }
}
