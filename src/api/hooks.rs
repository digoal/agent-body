use std::path::Path;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::storage::Storage;
use crate::fact::process_net::ProcessNetTopology;
use crate::fact::automation::AutomationTopology;
use crate::placement::engine_v2::PlacementEngineV2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreHookResult {
    pub allowed: bool,
    pub has_warnings: bool,
    pub warnings: Vec<String>,
    pub suggestions: Vec<String>,
    #[serde(default)]
    pub concurrency_mode: String,
    #[serde(default)]
    pub concurrency_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostHookResult {
    pub recorded: bool,
    pub message: String,
    pub affected_services: Vec<String>,
    #[serde(default)]
    pub concurrency_mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_prescription: Option<crate::hooks::FailureRecoveryPrescription>,
}

/// Agent 工作流动作前置与后置 Hook 引擎
pub struct AgentHookEngine {
    storage: Storage,
}

impl AgentHookEngine {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// 前置校验：Agent 执行工具调用前的安全性、端口冲突、文件放置违规诊断（集成 AOPCC 三态路由）
    pub fn pre_tool_call(&self, tool_name: &str, args: &Value) -> PreHookResult {
        use crate::hooks::{ActionConcurrencyMode, ConcurrencyPolicyClassifier};

        let mut class = ConcurrencyPolicyClassifier::classify(tool_name, args);
        let mut warnings = Vec::new();
        let mut suggestions = Vec::new();

        let tool_lower = tool_name.to_lowercase();

        // 1. 端口占用预警 (如 bind_port, run_server, start_daemon 等工具或带 port 参数)
        let port_opt = args.get("port")
            .and_then(|v| v.as_u64())
            .map(|p| p as u16);

        if let Some(port) = port_opt {
            // 物理端口检查
            if let Ok(res) = ProcessNetTopology::check_port(port, 3) {
                if res.status.eq_ignore_ascii_case("occupied") {
                    let proc_name = res.occupied_by.as_ref()
                        .and_then(|f| f.process_name.clone())
                        .unwrap_or_else(|| "未知进程".to_string());
                    let pid = res.occupied_by.as_ref().and_then(|f| f.pid).unwrap_or(0);
                    warnings.push(format!("⚠️ 目标端口 `{}` 目前已被物理占用！占用进程: `{}` (PID: {})", port, proc_name, pid));
                    suggestions.push(res.advice);
                }
            }

            // 多 Agent 协作锁排斥检查
            let port_str = port.to_string();
            let coord = crate::coordination::CoordinationEngine::new(self.storage.provider().clone());
            if let Ok(Some(lock)) = coord.check_resource(&crate::coordination::ResourceType::Port, &port_str) {
                warnings.push(format!("🔒 目标端口 `{}` 已被 Agent `{}` 加协作锁（有效期至 {}）！立即绑定将引发冲突！", port, lock.holder_agent, lock.expires_at));
                suggestions.push(format!("建议先调用 steward_lock_acquire 获取该端口锁，或与 Agent `{}` 协同释放", lock.holder_agent));
            }
        }

        // 2. 关键系统文件修改与爆炸半径排查 (write_file, edit_file 等)
        let path_opt = args.get("path")
            .or_else(|| args.get("file_path"))
            .or_else(|| args.get("target"))
            .and_then(|v| v.as_str());

        if let Some(path_str) = path_opt {
            // 0. 高危系统与凭据路径安全红线检测 (Phase 32.4)
            Self::check_critical_path_guard(&tool_lower, path_str, &mut warnings, &mut suggestions);

            // 协作锁排斥检查（文件锁与父目录锁传递）
            let coord = crate::coordination::CoordinationEngine::new(self.storage.provider().clone());
            if let Ok(Some(lock)) = coord.check_resource(&crate::coordination::ResourceType::File, path_str) {
                warnings.push(format!("🔒 目标文件 `{}` 已被 Agent `{}` 加协作锁（有效期至 {}）！", path_str, lock.holder_agent, lock.expires_at));
                suggestions.push(format!("请等待 Agent `{}` 释放该文件锁，避免覆盖修改", lock.holder_agent));
            }

            let p = Path::new(path_str);
            for ancestor in p.ancestors() {
                let anc_str = ancestor.to_string_lossy();
                if anc_str == "/" || anc_str.is_empty() {
                    continue;
                }
                if let Ok(Some(lock)) = coord.check_resource(&crate::coordination::ResourceType::Directory, &anc_str) {
                    warnings.push(format!("🔒 目标所在父目录 `{}` 已被 Agent `{}` 加协作锁（有效期至 {}）！", anc_str, lock.holder_agent, lock.expires_at));
                    suggestions.push(format!("建议先获取目录锁或与持有 Agent `{}` 协调", lock.holder_agent));
                    break;
                }
            }

            // A. 反向依赖影响排查（乐观直通动作与非绝对路径跳过重磁盘扫描，践行 AOPCC 零无效税原则）
            if class.mode != ActionConcurrencyMode::Optimistic && path_str.starts_with('/') {
                if let Ok(report) = AutomationTopology::find_impact_on_file(path_str) {
                    if !report.impacted_services.is_empty() || !report.impacted_cron_jobs.is_empty() {
                        warnings.push(format!("⚠️ 正在修改具有重要系统依赖的文件 `{}`: {}", path_str, report.advice));
                        for s in report.impacted_services {
                            suggestions.push(format!("需要注意守护服务 `{}` 依赖此文件，改动后须重启服务", s.service_name));
                        }
                        for c in report.impacted_cron_jobs {
                            suggestions.push(format!("需要注意定时调度 [{}] (用户: `{}`) 依赖此文件", c.schedule, c.user));
                        }
                    }
                }
            }

            // B. 用户文件放置规范校验 (如果是在创建新文件)
            if tool_lower.contains("create") || tool_lower.contains("write") {
                let placement_engine = PlacementEngineV2::new(self.storage.clone());
                let p = Path::new(path_str);
                let fname = p.file_name().and_then(|f| f.to_str()).unwrap_or("");
                let parent = p.parent().and_then(|d| d.to_str()).unwrap_or("");

                // 意图推导
                let obj_type = if fname.ends_with(".py") || fname.ends_with(".sh") { "script" }
                    else if fname.ends_with(".md") { "note" }
                    else if fname.ends_with(".rs") { "project" }
                    else { "other" };

                let req = crate::placement::engine_v2::PlacementRequestV2 {
                    object_type: obj_type.to_string(),
                    name: fname.to_string(),
                    intent: format!("创建文件 {}", fname),
                    tags: vec![],
                    security_level: "internal".to_string(),
                    current_workspace: None,
                };

                if let Ok(eval) = placement_engine.suggest(&req) {
                    let sug_parent = eval.suggested_path.parent().and_then(|d| d.to_str()).unwrap_or("");
                    if !sug_parent.is_empty() && !parent.starts_with(sug_parent) {
                        suggestions.push(format!("💡 规范放置建议: 用户偏好将此类文件存放于 `{}` 目录下（当前为 `{}`）", sug_parent, parent));
                    }
                }
            }
        }

        // 3. 服务协作锁排查 (如 restart_service, stop_service 等工具)
        let service_opt = args.get("service")
            .or_else(|| args.get("service_name"))
            .and_then(|v| v.as_str());

        if let Some(svc) = service_opt {
            let coord = crate::coordination::CoordinationEngine::new(self.storage.provider().clone());
            if let Ok(Some(lock)) = coord.check_resource(&crate::coordination::ResourceType::Service, svc) {
                warnings.push(format!("🔒 目标服务 `{}` 已被 Agent `{}` 加排他锁（有效期至 {}）！", svc, lock.holder_agent, lock.expires_at));
                suggestions.push(format!("操作服务可能导致竞争冲突，请等待 Agent `{}` 操作完成", lock.holder_agent));
            }
        }

        // 4. 破坏性命令模式预警 (如 run_command / bash / exec)
        if let Some(cmd_str) = args
            .get("command")
            .or_else(|| args.get("cmd"))
            .and_then(|v| v.as_str())
        {
            Self::check_destructive_command_guard(cmd_str, &mut warnings, &mut suggestions);
        }

        let has_warnings = !warnings.is_empty();
        if has_warnings && class.mode != ActionConcurrencyMode::PessimisticLock {
            class.mode = ActionConcurrencyMode::PessimisticLock;
            class.reason = "命中资源锁/依赖爆炸半径/安全红线冲突，自动升级为悲观强门禁 (PESSIMISTIC_LOCK)".to_string();
        }

        PreHookResult {
            allowed: true, // 管家默认采取建议式，不直接强制阻断
            has_warnings,
            warnings,
            suggestions,
            concurrency_mode: class.mode.as_str().to_string(),
            concurrency_reason: class.reason,
        }
    }

    /// 检查高危系统/凭据路径（表驱动安全红线）
    fn check_critical_path_guard(
        tool_lower: &str,
        path_str: &str,
        warnings: &mut Vec<String>,
        suggestions: &mut Vec<String>,
    ) {
        let is_read_only = tool_lower.starts_with("read")
            || tool_lower.starts_with("view")
            || tool_lower.starts_with("list")
            || tool_lower.starts_with("get");
        if is_read_only {
            return;
        }

        let lower_path = path_str.replace('\\', "/").to_lowercase();
        let critical_rules: &[(&str, &str)] = &[
            ("/etc/shadow", "系统用户密码哈希文件，直接修改可能导致认证瘫痪"),
            ("/etc/gshadow", "系统组密码哈希文件"),
            ("/etc/sudoers", "提权策略文件，语法错误将锁定 sudo 权限"),
            ("/etc/passwd", "系统账户核心文件"),
            ("/.ssh/", "SSH 私钥与 authorized_keys 凭据目录"),
            ("/.gnupg/", "GPG 私钥与信任链目录"),
            ("/boot/", "系统内核与引导分区目录"),
            ("/.steward/steward.db", "大管家主 SQLite 数据库文件，禁止绕过 Steward API 直接写文件"),
        ];

        for (pattern, desc) in critical_rules {
            if lower_path.contains(pattern) {
                warnings.push(format!(
                    "🛑 [CRITICAL · 安全红线] 检测到高危路径操作 `{}` ({})",
                    path_str, desc
                ));
                suggestions.push(
                    "请再次确认人工授权；涉及认证/系统配置建议先备份并在受控变更窗口操作，涉及数据库请通过 steward CLI/MCP 接口写入。".to_string(),
                );
                break;
            }
        }
    }

    /// 检查高危破坏性命令模式
    fn check_destructive_command_guard(
        cmd_str: &str,
        warnings: &mut Vec<String>,
        suggestions: &mut Vec<String>,
    ) {
        let normalized = cmd_str.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase();
        let destructive_patterns: &[(&str, &str)] = &[
            ("rm -rf / ", "根目录递归强制删除风险"),
            ("rm -rf /*", "根目录通配递归强制删除风险"),
            ("rm -fr / ", "根目录递归强制删除风险"),
            ("mkfs.", "磁盘分区格式化指令"),
            ("dd if=", "底层块设备裸写指令 (若 of=/dev/... 将不可逆覆盖磁盘)"),
            ("chmod -r 777 /", "全局放开根目录权限将破坏系统安全模型"),
            (":(){ :|:& };:", "Fork 炸弹拒绝服务模式"),
        ];

        let check_target = format!("{} ", normalized);
        for (pat, desc) in destructive_patterns {
            if check_target.contains(pat) {
                warnings.push(format!(
                    "🛑 [CRITICAL · 安全红线] 检测到高危破坏性命令模式 `{}` ({})",
                    pat.trim(),
                    desc
                ));
                suggestions.push("请先执行只读预览或确认目标路径与设备，切勿在无隔离环境下直接执行全盘破坏性命令。".to_string());
                break;
            }
        }
    }

    /// 后置通知：Agent 工具调用完成（成功入库并同步拓扑；失败时触发 AOPCC <2ms 秒级自愈诊断）
    pub fn post_tool_call(&self, tool_name: &str, args: &Value, success: bool) -> PostHookResult {
        self.post_tool_call_with_error(tool_name, args, success, None)
    }

    /// 后置通知增强版（支持透传错误输出，触发 OptimisticFailureDiagnoser 碰壁自愈）
    pub fn post_tool_call_with_error(
        &self,
        tool_name: &str,
        args: &Value,
        success: bool,
        error_output: Option<&str>,
    ) -> PostHookResult {
        use crate::hooks::{ConcurrencyPolicyClassifier, OptimisticFailureDiagnoser};

        let class = ConcurrencyPolicyClassifier::classify(tool_name, args);
        let mode_str = class.mode.as_str().to_string();

        if !success {
            let err_str = error_output.unwrap_or("");
            if let Some(rx) = OptimisticFailureDiagnoser::diagnose(&self.storage, tool_name, args, err_str) {
                return PostHookResult {
                    recorded: true,
                    message: format!("⚡ [AOPCC 碰壁秒级自愈] {}: {}", rx.failure_kind, rx.root_cause),
                    affected_services: vec![],
                    concurrency_mode: mode_str,
                    recovery_prescription: Some(rx),
                };
            }
            return PostHookResult {
                recorded: false,
                message: "工具执行失败，未匹配特定自愈模式，已忽略后置事实图同步".to_string(),
                affected_services: vec![],
                concurrency_mode: mode_str,
                recovery_prescription: None,
            };
        }

        let path_opt = args.get("path")
            .or_else(|| args.get("file_path"))
            .or_else(|| args.get("target"))
            .and_then(|v| v.as_str());

        let mut affected = Vec::new();

        if let Some(path_str) = path_opt {
            // 1. 记录变更记录
            let change = crate::storage::models::ChangeRecord {
                id: None,
                change_type: format!("agent_tool:{}", tool_name),
                target: path_str.to_string(),
                description: Some(format!("Agent 通过工具 `{}` 进行了物理文件变更", tool_name)),
                details: args.clone(),
                source: "agent_hook".to_string(),
                created_at: None,
            };
            let _ = self.storage.record_change(&change);

            // 2. 检查受影响服务
            if let Ok(report) = AutomationTopology::find_impact_on_file(path_str) {
                for s in report.impacted_services {
                    affected.push(s.service_name);
                }
            }

            // 3. Post-Hook 自动资产建档与记忆沉淀 (AutoSyncEngine)
            let auto_syncer = crate::hooks::AutoSyncEngine::new(self.storage.clone());
            let _ = auto_syncer.sync_file_output(path_str, tool_name);
        }

        PostHookResult {
            recorded: true,
            message: format!("管家已成功捕获工具 `{}` 产生的事实变更，并记录审计流水", tool_name),
            affected_services: affected,
            concurrency_mode: mode_str,
            recovery_prescription: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_pre_hook_port_conflict_warning() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let hook_engine = AgentHookEngine::new(storage);

        // 探测一个系统中通常不占用的超高随机测试端口
        let res_free = hook_engine.pre_tool_call("start_server", &serde_json::json!({"port": 61999}));
        assert!(res_free.allowed);

        // 测试一个正在监听的端口（如果有占用则 warning）
        let ports = ProcessNetTopology::scan_listening_ports().unwrap();
        if let Some(occupied) = ports.first() {
            let res_occ = hook_engine.pre_tool_call("bind_port", &serde_json::json!({"port": occupied.port}));
            assert!(res_occ.has_warnings, "已占用端口必须产生 warning");
            assert!(res_occ.warnings[0].contains("已被物理占用"));
        }
    }

    #[test]
    fn test_agent_post_hook_change_recording() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let hook_engine = AgentHookEngine::new(storage.clone());

        let res = hook_engine.post_tool_call(
            "write_file",
            &serde_json::json!({"path": "/root/new/work/demo.rs", "content": "fn main() {}"}),
            true,
        );
        assert!(res.recorded);

        let changes = storage.list_changes(10).expect("list_changes");
        assert!(!changes.is_empty());
        assert_eq!(changes[0].target, "/root/new/work/demo.rs");
    }

    #[test]
    fn test_agent_pre_hook_lock_conflict_warning() {
        use crate::coordination::{CoordinationEngine, LockRequest, ResourceType, LockScope};
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let coord = CoordinationEngine::new(storage.provider().clone());

        // Agent-1 先给端口 9090 加锁
        let req = LockRequest {
            resource_type: ResourceType::Port,
            resource_id: "9090".to_string(),
            scope: LockScope::Exclusive,
            holder_agent: "agent-1".to_string(),
            holder_pid: None,
            ttl_seconds: 60,
            metadata: None,
        };
        let _ = coord.try_acquire(&req).expect("acquire");

        let hook_engine = AgentHookEngine::new(storage);
        let res = hook_engine.pre_tool_call("bind_port", &serde_json::json!({"port": 9090}));
        assert!(res.has_warnings, "已加锁端口必须产生 warning");
        assert!(res.warnings.iter().any(|w| w.contains("已被 Agent `agent-1` 加协作锁")));
    }

    #[test]
    fn test_agent_pre_hook_directory_lock_propagation() {
        use crate::coordination::{CoordinationEngine, LockRequest, ResourceType, LockScope};
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let coord = CoordinationEngine::new(storage.provider().clone());

        // Agent-Alpha 给父目录 /etc/nginx/ 加锁
        let req = LockRequest {
            resource_type: ResourceType::Directory,
            resource_id: "/etc/nginx/".to_string(),
            scope: LockScope::Exclusive,
            holder_agent: "agent-alpha".to_string(),
            holder_pid: None,
            ttl_seconds: 60,
            metadata: None,
        };
        let _ = coord.try_acquire(&req).expect("acquire");

        let hook_engine = AgentHookEngine::new(storage);
        // 尝试向子路径 /etc/nginx/conf.d/app.conf 写入
        let res = hook_engine.pre_tool_call("write_file", &serde_json::json!({
            "path": "/etc/nginx/conf.d/app.conf",
            "content": "server {}"
        }));
        assert!(res.has_warnings, "父目录被锁时写子路径必须告警");
        assert!(res.warnings.iter().any(|w| w.contains("已被 Agent `agent-alpha` 加协作锁")));
    }

    #[test]
    fn test_agent_pre_hook_critical_path_and_command_guard() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let hook_engine = AgentHookEngine::new(storage);

        // 1. 高危路径写入触发 CRITICAL 告警
        let res_sudo = hook_engine.pre_tool_call("write_file", &serde_json::json!({
            "path": "/etc/sudoers"
        }));
        assert!(res_sudo.has_warnings);
        assert!(res_sudo.warnings.iter().any(|w| w.contains("CRITICAL") && w.contains("/etc/sudoers")));

        let res_ssh = hook_engine.pre_tool_call("edit_file", &serde_json::json!({
            "file_path": "/root/.ssh/authorized_keys"
        }));
        assert!(res_ssh.has_warnings);
        assert!(res_ssh.warnings.iter().any(|w| w.contains("CRITICAL")));

        // 只读工具访问不误报高危写入
        let res_read = hook_engine.pre_tool_call("read_file", &serde_json::json!({
            "path": "/etc/passwd"
        }));
        assert!(!res_read.warnings.iter().any(|w| w.contains("CRITICAL")));

        // 2. 破坏性命令触发 CRITICAL 告警
        let res_cmd = hook_engine.pre_tool_call("run_command", &serde_json::json!({
            "command": "rm -rf / --no-preserve-root"
        }));
        assert!(res_cmd.has_warnings);
        assert!(res_cmd.warnings.iter().any(|w| w.contains("CRITICAL")));
    }

    #[test]
    fn test_agent_aopcc_three_mode_and_post_failure_recovery() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let hook_engine = AgentHookEngine::new(storage.clone());

        // 1. 客户端直连 psql -p 5432 归为 OPTIMISTIC 且不触发端口占用拦截
        let pre_opt = hook_engine.pre_tool_call(
            "run_command",
            &serde_json::json!({"command": "psql -h 127.0.0.1 -p 5432 -c 'SELECT 1'"}),
        );
        assert_eq!(pre_opt.concurrency_mode, "OPTIMISTIC");
        assert!(!pre_opt.has_warnings);

        // 2. 服务端端口绑定 uvicorn --port 8000 归为 PESSIMISTIC_LOCK
        let pre_pess = hook_engine.pre_tool_call(
            "run_command",
            &serde_json::json!({"command": "uvicorn app:main --port 8000"}),
        );
        assert_eq!(pre_pess.concurrency_mode, "PESSIMISTIC_LOCK");

        // 3. 乐观直连失败后，PostToolUse 返回秒级自愈处方并降级租约
        let post_rx = hook_engine.post_tool_call_with_error(
            "run_command",
            &serde_json::json!({"command": "psql -h 127.0.0.1 -p 59876 -c 'SELECT 1'"}),
            false,
            Some("psql: error: connection to server at \"127.0.0.1\", port 59876 failed: Connection refused"),
        );
        assert!(post_rx.recorded);
        let rx = post_rx.recovery_prescription.expect("应返回碰壁自愈处方");
        assert_eq!(rx.failure_kind, "CONNECTION_REFUSED");
        assert!(rx.lease_downgraded);
    }
}
