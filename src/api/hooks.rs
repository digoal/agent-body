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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostHookResult {
    pub recorded: bool,
    pub message: String,
    pub affected_services: Vec<String>,
}

/// Agent 工作流动作前置与后置 Hook 引擎
pub struct AgentHookEngine {
    storage: Storage,
}

impl AgentHookEngine {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// 前置校验：Agent 执行工具调用前的安全性、端口冲突、文件放置违规诊断
    pub fn pre_tool_call(&self, tool_name: &str, args: &Value) -> PreHookResult {
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

            // A. 反向依赖影响排查
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

        let has_warnings = !warnings.is_empty();

        PreHookResult {
            allowed: true, // 管家默认采取建议式，不直接强制阻断
            has_warnings,
            warnings,
            suggestions,
        }
    }

    /// 后置通知：Agent 工具调用成功后，记录变更并通知管家增量更新拓扑
    pub fn post_tool_call(&self, tool_name: &str, args: &Value, success: bool) -> PostHookResult {
        if !success {
            return PostHookResult {
                recorded: false,
                message: "工具执行失败，忽略后置事实图同步".to_string(),
                affected_services: vec![],
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
}
