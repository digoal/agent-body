use serde::{Deserialize, Serialize};
use anyhow::Result;
use sysinfo::{CpuRefreshKind, Disks, MemoryRefreshKind, ProcessRefreshKind, RefreshKind, System};
use crate::storage::Storage;
use crate::storage::models::ActiveProcessRecord;

pub mod port;

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

    /// 采集并同步动态任务与普通进程 (过滤系统底噪，纳管活跃任务)
    pub fn collect_and_sync_processes(&self) -> Result<Vec<ActiveProcessRecord>> {
        let mut sys = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
        );
        sys.refresh_processes();

        let mut active_procs = Vec::new();

        for (pid, proc) in sys.processes() {
            let name = proc.name().to_string();
            let cmd_parts: Vec<String> = proc.cmd().iter().map(|s| s.to_string()).collect();
            let command = cmd_parts.join(" ");

            // 过滤系统底噪：只关注常见解释器、构建工具、服务与任务
            if is_noise_process(&name, &command) {
                continue;
            }

            let cpu_pct = proc.cpu_usage();
            let mem_bytes = proc.memory();
            let pid_u32 = pid.as_u32();

            // 提取命令中关联的文件资产路径
            let associated_path = extract_associated_script_path(&cmd_parts);

            let record = ActiveProcessRecord {
                id: format!("proc-{}", pid_u32),
                pid: pid_u32,
                name: name.clone(),
                command,
                owner: infer_process_owner(&name, &cmd_parts),
                associated_path,
                listening_ports: vec![],
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

fn is_noise_process(name: &str, command: &str) -> bool {
    let lower = name.to_lowercase();
    if lower.starts_with("kworker") || lower.starts_with("systemd") || lower == "init" {
        return true;
    }
    if command.is_empty() {
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
    }
}
