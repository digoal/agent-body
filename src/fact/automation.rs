use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Systemd 服务客观依赖事实
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemdServiceFact {
    /// 服务名（如 agent-steward.service, powercontext.service）
    pub service_name: String,
    /// 单元文件物理绝对路径（如 /etc/systemd/system/agent-steward.service）
    pub unit_file_path: String,
    /// 描述文本
    pub description: String,
    /// 启动执行命令 (ExecStart)
    pub exec_start: String,
    /// 工作目录 (WorkingDirectory)
    pub working_directory: Option<String>,
    /// 执行用户 (User)
    pub user: Option<String>,
    /// 依赖的 EnvironmentFile 配置文件绝对路径列表
    pub environment_files: Vec<String>,
    /// 在单元文件或 ExecStart 中引用的其它文件物理路径列表
    pub referenced_files: Vec<String>,
}

/// 定时任务自动化事实
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CronJobFact {
    /// 调度表达式（如 "*/5 * * * *" 或 "@daily"）
    pub schedule: String,
    /// 完整执行命令
    pub command: String,
    /// 运行用户
    pub user: String,
    /// 从命令行中提炼出的主脚本/程序物理绝对路径
    pub target_script: Option<String>,
    /// 来源文件路径（如 /etc/crontab 或 /etc/cron.d/sysstat）
    pub source_file: String,
}

/// 文件改动反向影响面报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileImpactReport {
    /// 被查询的目标文件路径
    pub target_file: String,
    /// 受其影响的 Systemd 服务
    pub impacted_services: Vec<SystemdServiceFact>,
    /// 受其影响的定时 Cron 任务
    pub impacted_cron_jobs: Vec<CronJobFact>,
    /// 架构师级避坑与协同建议
    pub advice: String,
}

/// 系统自动化拓扑分析器
pub struct AutomationTopology;

impl AutomationTopology {
    /// 扫描全系统主要 Systemd 服务及其配置依赖拓扑
    pub fn scan_systemd_services() -> Result<Vec<SystemdServiceFact>> {
        let scan_dirs = ["/etc/systemd/system", "/run/systemd/system"];
        let mut services = Vec::new();

        for dir in scan_dirs {
            let path = Path::new(dir);
            if !path.exists() {
                continue;
            }

            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() {
                        let fname = p.file_name().unwrap_or_default().to_string_lossy();
                        // 过滤非 .service 文件及备份文件 (.bak, .old)
                        if fname.ends_with(".service") && !fname.contains(".bak") && !fname.starts_with('.') {
                            if let Ok(fact) = Self::parse_service_unit(&p) {
                                services.push(fact);
                            }
                        }
                    }
                }
            }
        }

        services.sort_by(|a, b| a.service_name.cmp(&b.service_name));
        Ok(services)
    }

    /// 解析单个 systemd .service 单元内容
    pub fn parse_service_unit(unit_path: &Path) -> Result<SystemdServiceFact> {
        let content = fs::read_to_string(unit_path)?;
        let service_name = unit_path.file_name().unwrap_or_default().to_string_lossy().to_string();

        let mut description = String::new();
        let mut exec_start = String::new();
        let mut working_directory = None;
        let mut user = None;
        let mut environment_files = Vec::new();
        let mut referenced_files = Vec::new();

        // 提取绝对路径正则（例如 /root/... 或 /etc/...）
        let abs_path_re = Regex::new(r"(/[a-zA-Z0-9_\.\-]+(/[a-zA-Z0-9_\.\-]+)+)")
            .map_err(|e| anyhow::anyhow!("编译 abs_path_re 失败: {}", e))?;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }

            if let Some(val) = trimmed.strip_prefix("Description=") {
                description = val.trim().to_string();
            } else if let Some(val) = trimmed.strip_prefix("ExecStart=") {
                exec_start = val.trim().to_string();
                // 从 ExecStart 中提炼引用文件
                for cap in abs_path_re.captures_iter(&exec_start) {
                    if let Some(m) = cap.get(1) {
                        referenced_files.push(m.as_str().to_string());
                    }
                }
            } else if let Some(val) = trimmed.strip_prefix("WorkingDirectory=") {
                let cwd = val.trim().to_string();
                if !cwd.is_empty() {
                    working_directory = Some(cwd);
                }
            } else if let Some(val) = trimmed.strip_prefix("User=") {
                user = Some(val.trim().to_string());
            } else if let Some(val) = trimmed.strip_prefix("EnvironmentFile=") {
                let env_file = val.trim().trim_start_matches('-').to_string();
                if !env_file.is_empty() {
                    environment_files.push(env_file.clone());
                    referenced_files.push(env_file);
                }
            } else if trimmed.contains("Documentation=file://") {
                if let Some(idx) = trimmed.find("Documentation=file://") {
                    let doc_path = &trimmed[idx + 21..];
                    referenced_files.push(doc_path.to_string());
                }
            }
        }

        referenced_files.sort();
        referenced_files.dedup();

        Ok(SystemdServiceFact {
            service_name,
            unit_file_path: unit_path.to_string_lossy().to_string(),
            description,
            exec_start,
            working_directory,
            user,
            environment_files,
            referenced_files,
        })
    }

    /// 扫描全系统与当前用户定时任务 (Cron Jobs)
    pub fn scan_cron_jobs() -> Result<Vec<CronJobFact>> {
        let mut cron_jobs = Vec::new();
        let abs_path_re = Regex::new(r"(/[a-zA-Z0-9_\.\-]+(/[a-zA-Z0-9_\.\-]+)+)")
            .map_err(|e| anyhow::anyhow!("编译 abs_path_re 失败: {}", e))?;

        // 1. /etc/crontab
        let etc_crontab = Path::new("/etc/crontab");
        if etc_crontab.exists() {
            if let Ok(content) = fs::read_to_string(etc_crontab) {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.contains('=') {
                        continue;
                    }
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 7 {
                        let schedule = parts[0..5].join(" ");
                        let user = parts[5].to_string();
                        let command = parts[6..].join(" ");
                        let target_script = abs_path_re
                            .captures(&command)
                            .and_then(|c| c.get(1))
                            .map(|m| m.as_str().to_string());

                        cron_jobs.push(CronJobFact {
                            schedule,
                            command,
                            user,
                            target_script,
                            source_file: "/etc/crontab".to_string(),
                        });
                    }
                }
            }
        }

        // 2. /etc/cron.d/*
        let cron_d = Path::new("/etc/cron.d");
        if cron_d.exists() {
            if let Ok(entries) = fs::read_dir(cron_d) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if p.is_file() && !p.file_name().unwrap_or_default().to_string_lossy().starts_with('.') {
                        if let Ok(content) = fs::read_to_string(&p) {
                            for line in content.lines() {
                                let trimmed = line.trim();
                                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.contains('=') {
                                    continue;
                                }
                                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                                if parts.len() >= 7 {
                                    let schedule = parts[0..5].join(" ");
                                    let user = parts[5].to_string();
                                    let command = parts[6..].join(" ");
                                    let target_script = abs_path_re
                                        .captures(&command)
                                        .and_then(|c| c.get(1))
                                        .map(|m| m.as_str().to_string());

                                    cron_jobs.push(CronJobFact {
                                        schedule,
                                        command,
                                        user,
                                        target_script,
                                        source_file: p.to_string_lossy().to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. /var/spool/cron/crontabs/*
        let user_cron_dir = Path::new("/var/spool/cron/crontabs");
        if user_cron_dir.exists() {
            if let Ok(entries) = fs::read_dir(user_cron_dir) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    let username = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                    if let Ok(content) = fs::read_to_string(&p) {
                        for line in content.lines() {
                            let trimmed = line.trim();
                            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.contains('=') {
                                continue;
                            }
                            let parts: Vec<&str> = trimmed.split_whitespace().collect();
                            if parts.len() >= 6 {
                                let schedule = parts[0..5].join(" ");
                                let command = parts[5..].join(" ");
                                let target_script = abs_path_re
                                    .captures(&command)
                                    .and_then(|c| c.get(1))
                                    .map(|m| m.as_str().to_string());

                                cron_jobs.push(CronJobFact {
                                    schedule,
                                    command,
                                    user: username.clone(),
                                    target_script,
                                    source_file: p.to_string_lossy().to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(cron_jobs)
    }

    /// 反查指定文件路径的逆向依赖与爆炸半径（若改动该文件，会影响哪些后台自动化系统）
    pub fn find_impact_on_file(file_path: &str) -> Result<FileImpactReport> {
        let abs_path = if file_path.starts_with('/') {
            file_path.to_string()
        } else {
            fs::canonicalize(file_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| file_path.to_string())
        };

        let services = Self::scan_systemd_services()?;
        let cron_jobs = Self::scan_cron_jobs()?;

        let mut impacted_services = Vec::new();
        let mut impacted_cron_jobs = Vec::new();

        for s in services {
            let matches_unit = s.unit_file_path == abs_path;
            let matches_ref = s.referenced_files.iter().any(|r| r == &abs_path || abs_path.ends_with(r) || r.ends_with(&abs_path));
            let matches_cwd = s.working_directory.as_deref() == Some(&abs_path);

            if matches_unit || matches_ref || matches_cwd {
                impacted_services.push(s);
            }
        }

        for c in cron_jobs {
            let matches_script = c.target_script.as_deref() == Some(&abs_path);
            let matches_cmd = c.command.contains(&abs_path);

            if matches_script || matches_cmd {
                impacted_cron_jobs.push(c);
            }
        }

        let advice = if !impacted_services.is_empty() || !impacted_cron_jobs.is_empty() {
            format!(
                "⚠️ 文件 `{}` 存在强依赖：受影响 Systemd 服务 {} 个，Cron 定时任务 {} 个！改动后可能引发生产服务异常或需要执行 `systemctl daemon-reload && systemctl restart <service>`。",
                abs_path,
                impacted_services.len(),
                impacted_cron_jobs.len()
            )
        } else {
            format!("✅ 文件 `{}` 未被检测到强关联的 Systemd 服务或系统级 Cron 任务依赖，可安全编辑。", abs_path)
        };

        Ok(FileImpactReport {
            target_file: abs_path,
            impacted_services,
            impacted_cron_jobs,
            advice,
        })
    }

    /// 格式化全系统自动化任务总览报告
    pub fn format_automation_overview(
        services: &[SystemdServiceFact],
        cron_jobs: &[CronJobFact],
    ) -> String {
        let mut out = String::new();
        out.push_str("⚙️ **宿主机自动化拓扑全景网络 (Systemd & Cron)**\n\n");

        out.push_str(&format!("### 1. 活跃与托管 Systemd 守护服务 (共 {} 个)\n\n", services.len()));
        if services.is_empty() {
            out.push_str("未发现托管服务。\n\n");
        } else {
            out.push_str("| 服务名 | 描述 | 启动命令 (ExecStart) | 依赖配置/文件数 |\n");
            out.push_str("|:---|:---|:---|:---:|\n");
            for s in services {
                let desc = if s.description.is_empty() { "-" } else { &s.description };
                let cmd_trunc = if s.exec_start.len() > 50 {
                    format!("{}...", &s.exec_start[..47])
                } else {
                    s.exec_start.clone()
                };
                out.push_str(&format!(
                    "| `{}` | {} | `{}` | {} |\n",
                    s.service_name, desc, cmd_trunc, s.referenced_files.len()
                ));
            }
            out.push_str("\n");
        }

        out.push_str(&format!("### 2. 后台定时自动化调度 (Cron Jobs 共 {} 个)\n\n", cron_jobs.len()));
        if cron_jobs.is_empty() {
            out.push_str("未发现后台 cron 定时任务。\n\n");
        } else {
            out.push_str("| 调度周期 | 执行用户 | 目标脚本/程序 | 来源文件 |\n");
            out.push_str("|:---|:---:|:---|:---|\n");
            for c in cron_jobs {
                let script_str = c.target_script.as_deref().unwrap_or(&c.command);
                let script_trunc = if script_str.len() > 40 {
                    format!("{}...", &script_str[..37])
                } else {
                    script_str.to_string()
                };
                out.push_str(&format!(
                    "| `{}` | `{}` | `{}` | `{}` |\n",
                    c.schedule, c.user, script_trunc, c.source_file
                ));
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_systemd_services_real_host() {
        let services = AutomationTopology::scan_systemd_services().expect("扫描 systemd 应该成功");
        assert!(!services.is_empty(), "宿主机上应该至少存在 systemd 单元");
        
        // 验证 agent-steward.service 或 ollama.service 等真实单元是否被捕获
        let found_steward = services.iter().any(|s| s.service_name.contains("steward"));
        println!("扫描到 systemd 服务数: {}, 是否包含 steward: {}", services.len(), found_steward);
    }

    #[test]
    fn test_scan_cron_jobs_real_host() {
        let cron_jobs = AutomationTopology::scan_cron_jobs().expect("扫描 cron 应该成功");
        println!("扫描到 cron 任务数: {}", cron_jobs.len());
        for c in &cron_jobs {
            println!("Cron: {} user: {} target: {:?}", c.schedule, c.user, c.target_script);
        }
    }

    #[test]
    fn test_find_impact_on_file() {
        // 测试反查 agent-steward 的二进制文件或其配置
        let impact = AutomationTopology::find_impact_on_file("/usr/local/bin/agent-steward").expect("反查影响面成功");
        println!("影响服务数: {}", impact.impacted_services.len());
        assert!(impact.advice.contains("agent-steward"));
    }

    #[test]
    fn test_format_automation_overview() {
        let services = vec![SystemdServiceFact {
            service_name: "test.service".to_string(),
            unit_file_path: "/etc/systemd/system/test.service".to_string(),
            description: "Test service".to_string(),
            exec_start: "/usr/bin/test-app --config /etc/test.conf".to_string(),
            working_directory: Some("/root".to_string()),
            user: Some("root".to_string()),
            environment_files: vec!["/etc/default/test".to_string()],
            referenced_files: vec!["/etc/test.conf".to_string()],
        }];

        let cron_jobs = vec![CronJobFact {
            schedule: "*/5 * * * *".to_string(),
            command: "/root/script.sh".to_string(),
            user: "root".to_string(),
            target_script: Some("/root/script.sh".to_string()),
            source_file: "/etc/crontab".to_string(),
        }];

        let overview = AutomationTopology::format_automation_overview(&services, &cron_jobs);
        assert!(overview.contains("test.service"));
        assert!(overview.contains("*/5 * * * *"));
    }
}
