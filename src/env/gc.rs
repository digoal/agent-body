use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};
use serde::{Deserialize, Serialize};
use anyhow::{bail, Result};
use sysinfo::{ProcessRefreshKind, RefreshKind, System};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TmpFileCandidate {
    pub path: String,
    pub size_bytes: u64,
    pub age_hours: f64,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrphanProcessCandidate {
    pub pid: u32,
    pub name: String,
    pub command: String,
    pub memory_mb: u64,
    pub running_secs: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildCacheCandidate {
    pub path: String,
    pub kind: String,
    pub size_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostGcReport {
    pub tmp_dir_scanned: String,
    pub candidate_files_count: usize,
    pub candidate_bytes_reclaimed: u64,
    pub deleted_files_count: usize,
    pub deleted_bytes_reclaimed: u64,
    pub tmp_files: Vec<TmpFileCandidate>,
    pub orphan_processes: Vec<OrphanProcessCandidate>,
    #[serde(default)]
    pub build_caches: Vec<BuildCacheCandidate>,
    pub dry_run: bool,
}

pub struct HostGarbageCollector;

impl HostGarbageCollector {
    /// 执行宿主垃圾回收（安全清理 /root/new/tmp/ 超期文件与诊断孤儿进程）
    pub fn run_gc(
        tmp_dir: &Path,
        max_age_hours: u64,
        dry_run: bool,
    ) -> Result<HostGcReport> {
        // 安全红线校验: 严禁将非临时目录作为 GC 目标
        let tmp_str = tmp_dir.to_string_lossy().to_string();
        if !tmp_str.contains("/tmp") && !tmp_str.contains("temp") {
            bail!("安全红线阻断: GC 目标路径必须包含 '/tmp' 或 'temp'，严禁扫描生产工作目录: {}", tmp_str);
        }

        let now = SystemTime::now();
        let max_age = Duration::from_secs(max_age_hours * 3600);

        let mut candidate_files = Vec::new();
        let mut candidate_bytes = 0u64;
        let mut deleted_files_count = 0usize;
        let mut deleted_bytes_reclaimed = 0u64;

        if tmp_dir.exists() {
            for entry in WalkDir::new(tmp_dir)
                .min_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                // 忽略当前目录本身
                if path == tmp_dir {
                    continue;
                }

                if let Ok(metadata) = entry.metadata() {
                    let is_dir = metadata.is_dir();
                    let size = if is_dir { 0 } else { metadata.len() };

                    let mtime = metadata.modified().unwrap_or(now);
                    let elapsed = now.duration_since(mtime).unwrap_or(Duration::ZERO);

                    if elapsed >= max_age {
                        let age_hours = elapsed.as_secs_f64() / 3600.0;
                        candidate_files.push(TmpFileCandidate {
                            path: path.to_string_lossy().to_string(),
                            size_bytes: size,
                            age_hours,
                            is_dir,
                        });
                        candidate_bytes += size;

                        if !dry_run && !is_dir {
                            if fs::remove_file(path).is_ok() {
                                deleted_files_count += 1;
                                deleted_bytes_reclaimed += size;
                            }
                        }
                    }
                }
            }

            // 清理空的超期子目录 (仅在真实执行且目录为空时)
            if !dry_run {
                for cand in candidate_files.iter().rev() {
                    if cand.is_dir {
                        let _ = fs::remove_dir(Path::new(&cand.path));
                    }
                }
            }
        }

        // 诊断孤儿进程 (PPID == 1 或 PPID 不在活跃列表中的 Agent Worker)
        let orphan_processes = Self::diagnose_orphan_processes();
        // 只读巡检镜像仓库中的冗余构建缓存（target/、node_modules/），绝不自动删除
        let build_caches = Self::scan_redundant_build_caches(&[Path::new("/root/new/shared_git")]);

        let candidate_files_count = candidate_files.len();
        Ok(HostGcReport {
            tmp_dir_scanned: tmp_str,
            candidate_files_count,
            candidate_bytes_reclaimed: candidate_bytes,
            deleted_files_count,
            deleted_bytes_reclaimed,
            tmp_files: candidate_files,
            orphan_processes,
            build_caches,
            dry_run,
        })
    }

    /// 只读巡检给定根目录下各子项目的冗余构建缓存（target/、node_modules/），返回可回收体积建议
    pub fn scan_redundant_build_caches(roots: &[&Path]) -> Vec<BuildCacheCandidate> {
        let mut results = Vec::new();
        for root in roots {
            if !root.exists() || !root.is_dir() {
                continue;
            }
            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    let proj = entry.path();
                    if !proj.is_dir() {
                        continue;
                    }
                    for (sub_name, kind) in [("target", "rust_target"), ("node_modules", "node_modules")] {
                        let cache_dir = proj.join(sub_name);
                        if cache_dir.is_dir() {
                            let bytes: u64 = WalkDir::new(&cache_dir)
                                .max_depth(6)
                                .into_iter()
                                .filter_map(|e| e.ok())
                                .filter_map(|e| e.metadata().ok())
                                .filter(|m| m.is_file())
                                .map(|m| m.len())
                                .sum();
                            results.push(BuildCacheCandidate {
                                path: cache_dir.to_string_lossy().to_string(),
                                kind: kind.to_string(),
                                size_mb: bytes / (1024 * 1024),
                            });
                        }
                    }
                }
            }
        }
        results
    }

    /// 诊断宿主机孤儿/失控测试与 Worker 进程
    pub fn diagnose_orphan_processes() -> Vec<OrphanProcessCandidate> {
        let mut sys = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::everything()),
        );
        sys.refresh_processes();

        let mut orphans = Vec::new();
        let agent_keywords = ["python", "node", "pytest", "cargo", "worker", "runner"];

        for (pid, proc_) in sys.processes() {
            let cmd_line = proc_.cmd().join(" ");
            let cmd_lower = cmd_line.to_lowercase();
            let proc_name = proc_.name().to_lowercase();

            // 仅审查 Agent 相关的脚本或构建测试进程
            let is_agent_process = agent_keywords.iter().any(|kw| {
                cmd_lower.contains(kw) || proc_name.contains(kw)
            });

            if !is_agent_process {
                continue;
            }

            // 孤儿判定: 父进程 PID 为 1 (已被 systemd/init 托管接管) 或父进程已不存在
            let parent_pid_opt = proc_.parent();
            let is_orphan = match parent_pid_opt {
                None => true,
                Some(ppid) => ppid.as_u32() == 1,
            };

            // 运行时间超过 3 小时 (10800s) 的孤儿 Worker
            let run_time_secs = proc_.run_time();
            if is_orphan && run_time_secs > 10800 {
                let memory_mb = proc_.memory() / (1024 * 1024);
                orphans.push(OrphanProcessCandidate {
                    pid: pid.as_u32(),
                    name: proc_.name().to_string(),
                    command: cmd_line,
                    memory_mb,
                    running_secs: run_time_secs,
                    reason: "父进程已退出(PPID=1)且长周期挂起无应答".to_string(),
                });
            }
        }

        orphans
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_gc_safety_check_blocks_dangerous_path() {
        let res = HostGarbageCollector::run_gc(Path::new("/root/new/work"), 24, true);
        assert!(res.is_err());
    }

    #[test]
    fn test_gc_tmp_files_dry_run_and_cleanup() -> Result<()> {
        let dir = tempdir()?;
        let tmp_sub = dir.path().join("tmp_test_area");
        fs::create_dir_all(&tmp_sub)?;

        let old_file = tmp_sub.join("expired_data.log");
        let fresh_file = tmp_sub.join("fresh_data.log");

        fs::write(&old_file, "old log content to be cleaned")?;
        fs::write(&fresh_file, "fresh log content")?;

        // 1. Dry run 测试 (0 小时判定为超期)
        let rep_dry = HostGarbageCollector::run_gc(&tmp_sub, 0, true)?;
        assert_eq!(rep_dry.candidate_files_count, 2);
        assert_eq!(rep_dry.deleted_files_count, 0);
        assert!(old_file.exists());
        assert!(fresh_file.exists());

        // 2. Real run 测试
        let rep_real = HostGarbageCollector::run_gc(&tmp_sub, 0, false)?;
        assert_eq!(rep_real.deleted_files_count, 2);
        assert!(!old_file.exists());
        assert!(!fresh_file.exists());

        Ok(())
    }
}
