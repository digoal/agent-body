use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, ProcessRefreshKind, RefreshKind, System};
use crate::storage::Storage;
use crate::storage::models::ActiveProcessRecord;
use crate::fact::process_net::ProcessNetTopology;

pub mod port;
pub mod gc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSummary {
    pub os_name: String,
    pub kernel_version: String,
    pub host_name: String,
    pub cpu_cores: usize,
    pub cpu_brand: String,
    pub total_memory_mb: u64,
    pub used_memory_mb: u64,
    pub memory_used_pct: f32,
    pub disks: Vec<DiskInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub mount_point: String,
    pub total_space_gb: f64,
    pub available_space_gb: f64,
}

pub struct EnvCollector {
    storage: Storage,
}

impl EnvCollector {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// 采集系统硬件与基础环境摘要
    pub fn collect_system_summary(&self) -> SystemSummary {
        let mut sys = System::new_with_specifics(
            RefreshKind::new()
                .with_cpu(CpuRefreshKind::everything())
                .with_memory(MemoryRefreshKind::everything()),
        );
        sys.refresh_all();

        let os_name = System::long_os_version().unwrap_or_else(|| "Linux (Unknown)".to_string());
        let kernel_version = System::kernel_version().unwrap_or_else(|| "Unknown".to_string());
        let host_name = System::host_name().unwrap_or_else(|| "localhost".to_string());

        let cpu_cores = sys.cpus().len();
        let cpu_brand = sys.cpus().first().map(|c| c.brand().to_string()).unwrap_or_default();

        let total_memory_mb = sys.total_memory() / (1024 * 1024);
        let used_memory_mb = sys.used_memory() / (1024 * 1024);
        let memory_used_pct = if total_memory_mb > 0 {
            (used_memory_mb as f32 / total_memory_mb as f32) * 100.0
        } else {
            0.0
        };

        let disks_obj = Disks::new_with_refreshed_list();
        let mut disks = Vec::new();
        for d in &disks_obj {
            disks.push(DiskInfo {
                mount_point: d.mount_point().to_string_lossy().to_string(),
                total_space_gb: d.total_space() as f64 / (1024.0 * 1024.0 * 1024.0),
                available_space_gb: d.available_space() as f64 / (1024.0 * 1024.0 * 1024.0),
            });
        }

        SystemSummary {
            os_name,
            kernel_version,
            host_name,
            cpu_cores,
            cpu_brand,
            total_memory_mb,
            used_memory_mb,
            memory_used_pct,
            disks,
        }
    }

    /// 采集并同步动态任务与普通进程 (过滤线程与系统底噪，关联真实监听端口)
    pub fn collect_and_sync_processes(&self) -> Result<Vec<ActiveProcessRecord>> {
        let mut sys = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
        );
        sys.refresh_processes();

        // 预先构建 PID -> 真实监听端口集合映射
        let mut pid_to_ports: HashMap<u32, Vec<u16>> = HashMap::new();
        if let Ok(port_facts) = ProcessNetTopology::scan_listening_ports() {
            for pf in port_facts {
                if let Some(pid) = pf.pid {
                    let entry = pid_to_ports.entry(pid).or_default();
                    if !entry.contains(&pf.port) {
                        entry.push(pf.port);
                    }
                }
            }
            for ports in pid_to_ports.values_mut() {
                ports.sort_unstable();
            }
        }

        let mut active_procs = Vec::new();

        for (pid, proc) in sys.processes() {
            // 过滤 Linux 用户态/内核态子线程（避免 18 线程 daemon 产生 18 条重复记录）
            if proc.thread_kind().is_some() {
                continue;
            }

            let name = proc.name().to_string();
            let cmd_parts: Vec<String> = proc.cmd().iter().map(|s| s.to_string()).collect();
            let raw_command = cmd_parts.join(" ");
            let pid_u32 = pid.as_u32();
            let listening_ports = pid_to_ports.get(&pid_u32).cloned().unwrap_or_default();

            // 过滤系统底噪：只关注应用进程、脚本、构建任务与监听端口的服务
            if listening_ports.is_empty() && is_noise_process(&name, &raw_command) {
                continue;
            }
            let command = normalize_process_command(&raw_command);

            let cpu_pct = proc.cpu_usage();
            let mem_bytes = proc.memory();

            // 提取命令中关联的文件资产路径
            let associated_path = extract_associated_script_path(&cmd_parts);

            let record = ActiveProcessRecord {
                id: format!("proc-{}", pid_u32),
                pid: pid_u32,
                name: name.clone(),
                command,
                owner: infer_process_owner(&name, &cmd_parts),
                associated_path,
                listening_ports,
                status: "running".to_string(),
                cpu_pct,
                mem_bytes,
                gpu_allocated_mb: 0,
                started_at: None,
            };

            active_procs.push(record);
        }

        // 持久化到 Storage
        self.storage.replace_active_processes(&active_procs)?;
        Ok(active_procs)
    }
}

/// 规范化进程命令行：全链路脱敏 + 折叠多行空白 + 截断超长内联脚本（避免 node -e / python3 -c 刷屏简报）
pub fn normalize_process_command(raw: &str) -> String {
    let redacted = crate::scanner::metadata::SecretRedactor::redact_single_line(raw);
    let collapsed = redacted.split_whitespace().collect::<Vec<_>>().join(" ");
    let has_inline_script = collapsed.contains(" -e ")
        || collapsed.contains(" -c ")
        || collapsed.contains(" eval ");
    let char_count = collapsed.chars().count();
    if char_count > 180 || (has_inline_script && char_count > 120) {
        let head: String = collapsed.chars().take(180).collect();
        format!("{} ... [inline-script-truncated]", head)
    } else {
        collapsed
    }
}

fn is_noise_process(name: &str, command: &str) -> bool {
    let lower = name.to_lowercase();
    if command.trim().is_empty() {
        return true;
    }
    if command.contains("CODEGRAPH_NO_WATCHDOG") {
        return true;
    }
    const NOISE_PREFIXES: &[&str] = &[
        "kworker",
        "ksoftirqd",
        "kswapd",
        "kthreadd",
        "migration",
        "rcu_",
        "systemd",
        "tokio-rt-worker",
        "tokio-runtime",
        "libuv-worker",
        "v8worker",
        "asyncio_",
    ];
    if NOISE_PREFIXES.iter().any(|p| lower.starts_with(p)) {
        return true;
    }
    const NOISE_EXACT: &[&str] = &[
        "init",
        "agetty",
        "getty",
        "dbus-daemon",
        "dbus-broker",
        "rsyslogd",
        "irqbalance",
        "polkitd",
        "udisksd",
        "upowerd",
        "accounts-daemon",
        "wpa_supplicant",
        "avahi-daemon",
        "bluetoothd",
        "thermald",
        "acpid",
        "atd",
        "cron",
        "crond",
        "rpcbind",
        "rpcd_lsad",
        "sleep",
    ];
    if NOISE_EXACT.iter().any(|&exact| lower == exact) {
        return true;
    }
    false
}

fn infer_process_owner(_name: &str, cmd_parts: &[String]) -> String {
    for arg in cmd_parts {
        if arg.contains("hermes") || arg.contains("cron") {
            return "agent:hermes_cron".to_string();
        }
    }
    "user:digoal".to_string()
}

fn extract_associated_script_path(cmd_parts: &[String]) -> Option<String> {
    for arg in cmd_parts.iter().skip(1) {
        if (arg.ends_with(".py") || arg.ends_with(".sh") || arg.ends_with(".rs") || arg.ends_with(".js"))
            && (arg.starts_with('/') || arg.starts_with("./")) {
            return Some(arg.clone());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_system_summary() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let collector = EnvCollector::new(storage);
        let summary = collector.collect_system_summary();

        assert!(!summary.os_name.is_empty());
        assert!(summary.cpu_cores > 0);
        assert!(summary.total_memory_mb > 0);
    }

    #[test]
    fn test_collect_and_sync_processes() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let collector = EnvCollector::new(storage.clone());
        let procs = collector.collect_and_sync_processes().expect("collect processes");

        // 验证持久化到了数据库
        let db_procs = storage.list_active_processes().expect("list active procs");
        assert_eq!(procs.len(), db_procs.len());
        // 不应包含线程名底噪
        assert!(
            !procs.iter().any(|p| p.name.starts_with("tokio-rt-worker") || p.name.starts_with("kworker")),
            "过滤线程与内核底噪后不应出现 worker 线程"
        );
    }

    #[test]
    fn test_is_noise_process_filters_system_daemons() {
        assert!(is_noise_process("kworker/u16:2", "kworker"));
        assert!(is_noise_process("systemd-journald", "/lib/systemd/systemd-journald"));
        assert!(is_noise_process("tokio-rt-worker", "/usr/local/bin/agent-body daemon"));
        assert!(is_noise_process("dbus-daemon", "/usr/bin/dbus-daemon --system"));
        assert!(is_noise_process("python3", ""));
        assert!(!is_noise_process("agent-body", "/usr/local/bin/agent-body daemon"));
        assert!(!is_noise_process("python3", "python3 /root/new/shared_proj/bot.py"));
    }

    #[test]
    fn test_process_command_redaction_before_storage() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let sanitized_cmd = crate::scanner::metadata::SecretRedactor::redact_single_line(
            "uvicorn app:main --password=mySecret123 --port 8080",
        );
        let rec = ActiveProcessRecord {
            id: "proc-99999".to_string(),
            pid: 99999,
            name: "uvicorn".to_string(),
            command: sanitized_cmd.clone(),
            owner: "root".to_string(),
            associated_path: None,
            listening_ports: vec![8080],
            status: "running".to_string(),
            cpu_pct: 0.1,
            mem_bytes: 1024,
            gpu_allocated_mb: 0,
            started_at: None,
        };
        storage.replace_active_processes(&[rec]).expect("replace");
        let list = storage.list_active_processes().expect("list");
        assert_eq!(list.len(), 1);
        assert!(!list[0].command.contains("mySecret123"));
        assert!(list[0].command.contains("<REDACTED>"));
    }

    #[test]
    fn test_normalize_process_command_folds_multiline_and_truncates_inline_script() {
        let raw = "node -e \"\nconst fs = require('fs');\nconst token = 'sk-ant-api03-abcdef1234567890abcdef';\n".to_string()
            + &"console.log(1);\n".repeat(20)
            + "\"";
        let norm = normalize_process_command(&raw);
        assert!(!norm.contains('\n'), "不应包含换行符");
        assert!(!norm.contains("sk-ant-api03"), "仍须经过全链路脱敏");
        assert!(norm.contains("[inline-script-truncated]"), "超长内联脚本应被截断");
    }

    #[test]
    fn test_is_noise_process_filters_codegraph_watchdog() {
        assert!(is_noise_process(
            "MainThread",
            "node -e console.log(1) // CODEGRAPH_NO_WATCHDOG=1"
        ));
    }
}
