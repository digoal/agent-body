use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};


mod checks;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckCategory {
    Required,      // 必须满足，否则关键功能不可用，启动将阻断
    Optional,      // 可选增强，不满足时系统自动降级运行
    Informational, // 环境与运行时信息，用于排障诊断
}

impl std::fmt::Display for CheckCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckCategory::Required => write!(f, "REQUIRED"),
            CheckCategory::Optional => write!(f, "OPTIONAL"),
            CheckCategory::Informational => write!(f, "INFO"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
}

impl std::fmt::Display for CheckStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckStatus::Pass => write!(f, "PASS"),
            CheckStatus::Warn => write!(f, "WARN"),
            CheckStatus::Fail => write!(f, "FAIL"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckItem {
    pub name: String,
    pub category: CheckCategory,
    pub status: CheckStatus,
    pub message: String,
    pub remedy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreflightReport {
    pub timestamp: String,
    pub os_info: String,
    pub arch: String,
    pub items: Vec<CheckItem>,
    pub passed_required: bool,
}

impl PreflightReport {
    pub fn new() -> Self {
        let os_info = format!("{}-{}", std::env::consts::OS, std::env::consts::FAMILY);
        let arch = std::env::consts::ARCH.to_string();
        Self {
            timestamp: chrono::Local::now().to_rfc3339(),
            os_info,
            arch,
            items: Vec::new(),
            passed_required: true,
        }
    }

    pub fn add(&mut self, item: CheckItem) {
        if item.category == CheckCategory::Required && item.status == CheckStatus::Fail {
            self.passed_required = false;
        }
        self.items.push(item);
    }

    pub fn format_table(&self) -> String {
        let mut out = String::new();
        out.push_str("╔══════════════════════════════════════════════════════════════════════════════════════════╗\n");
        out.push_str("║                   Agent Body — 运行环境与依赖预检报告 (Preflight)                ║\n");
        out.push_str("╚══════════════════════════════════════════════════════════════════════════════════════════╝\n");
        out.push_str(&format!("系统架构: {} ({})  |  检查时间: {}\n", self.os_info, self.arch, self.timestamp));
        out.push_str("--------------------------------------------------------------------------------------------\n");
        out.push_str(&format!("{:<6} {:<12} {:<24} {}\n", "状态", "类型", "检查项", "详情 / 修复指引"));
        out.push_str("--------------------------------------------------------------------------------------------\n");

        for item in &self.items {
            let status_badge = match item.status {
                CheckStatus::Pass => "✅ PASS",
                CheckStatus::Warn => "⚠️ WARN",
                CheckStatus::Fail => "❌ FAIL",
            };
            let remedy_str = if let Some(ref r) = item.remedy {
                format!(" => 修复建议: {}", r)
            } else {
                String::new()
            };
            out.push_str(&format!(
                "{:<8} {:<12} {:<24} {}{}\n",
                status_badge,
                item.category.to_string(),
                item.name,
                item.message,
                remedy_str
            ));
        }

        out.push_str("--------------------------------------------------------------------------------------------\n");
        if self.passed_required {
            out.push_str("🚀 结论: 所有 REQUIRED 基础环境依赖全部满足，Agent 本体可稳定上线运行。\n");
        } else {
            out.push_str("🛑 结论: 存在 REQUIRED 基础依赖未满足项！Agent 本体关键功能将受限或无法启动，请参考修复建议。\n");
        }
        out
    }
}

impl Default for PreflightReport {
    fn default() -> Self {
        Self::new()
    }
}

pub struct PreflightChecker;

impl PreflightChecker {
    /// 执行全量运行环境依赖检查
    pub fn run_all(data_dir: &Path, config: Option<&crate::config::Config>) -> PreflightReport {
        let mut report = PreflightReport::new();

        // 1. 必选依赖检查 (Required)
        Self::check_procfs(&mut report);
        Self::check_inotify(&mut report);
        Self::check_data_dir(&mut report, data_dir);
        Self::check_sqlite(&mut report);

        // 2. 可选依赖检查 (Optional)
        Self::check_duckdb(&mut report);
        Self::check_systemd(&mut report);
        Self::check_codegraph(&mut report);
        Self::check_ecosystem_skills(&mut report);
        Self::check_disk_watermark(&mut report);
        Self::check_multimodal_tools(&mut report);
        Self::check_security_posture(&mut report, data_dir, config);

        if let Some(cfg) = config {
            Self::check_ollama(&mut report, &cfg.embedding.ollama_url, &cfg.embedding.model, cfg.embedding.fallback_fts_only);
            Self::check_annotation_model(&mut report, &cfg.annotation);
            Self::check_storage_backend(&mut report, cfg);
            Self::check_memory_backend(&mut report, cfg);
            Self::check_relate_graph_probe(&mut report, cfg);
        } else {
            let default_emb = crate::config::EmbeddingConfig::default();
            Self::check_ollama(
                &mut report,
                &default_emb.ollama_url,
                &default_emb.model,
                default_emb.fallback_fts_only,
            );
            Self::check_annotation_model(&mut report, &crate::config::AnnotationConfig::default());
        }

        // 3. 信息项 (Informational)
        Self::collect_sys_info(&mut report);

        report
    }

    /// 检查宿主机只读多模态解析工具 (pdfinfo / pdftotext / ffprobe)
    pub fn check_multimodal_tools(report: &mut PreflightReport) {
        let probes = ["pdfinfo", "pdftotext", "ffprobe"];
        let found: Vec<&str> = probes
            .iter()
            .copied()
            .filter(|p| Self::resolve_binary_path(p).is_some())
            .collect();
        let missing: Vec<&str> = probes
            .iter()
            .copied()
            .filter(|p| !found.contains(p))
            .collect();
        let message = if missing.is_empty() {
            format!("多模态辅助探针就绪: {}", found.join(", "))
        } else if found.is_empty() {
            "未检测到 pdfinfo/pdftotext/ffprobe，自动降级为纯 Rust L1A 多模态解析".to_string()
        } else {
            format!(
                "已就绪 [{}]，缺失 [{}] (自动回退纯 Rust 解析)",
                found.join(", "),
                missing.join(", ")
            )
        };
        report.add(CheckItem {
            name: "多模态只读探针工具".to_string(),
            category: CheckCategory::Optional,
            status: CheckStatus::Pass,
            message,
            remedy: if missing.is_empty() {
                None
            } else {
                Some("如需 PDF 深层正文与音视频流精确探测，可安装 poppler 与 ffmpeg".to_string())
            },
        });
    }

    /// 检查本地存储最小权限 (0700/0600) 与接口暴露安全基线
    pub fn check_security_posture(
        report: &mut PreflightReport,
        data_dir: &Path,
        config: Option<&crate::config::Config>,
    ) {
        let mut issues: Vec<String> = Vec::new();
        let mut remedies: Vec<String> = Vec::new();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if data_dir.exists() {
                if let Ok(meta) = fs::metadata(data_dir) {
                    let mode = meta.permissions().mode() & 0o777;
                    if (mode & 0o077) != 0 {
                        issues.push(format!("数据目录 {} 权限过宽 ({:03o})", data_dir.display(), mode));
                        remedies.push(format!("chmod 700 {}", data_dir.display()));
                    }
                }
            }

            let config_file = data_dir.join("config.toml");
            let db_file = config
                .map(|c| c.db_path.clone())
                .unwrap_or_else(|| data_dir.join("body.db"));
            for path in [&config_file, &db_file] {
                if path.exists() {
                    if let Ok(meta) = fs::metadata(path) {
                        let mode = meta.permissions().mode() & 0o777;
                        if (mode & 0o077) != 0 {
                            issues.push(format!("敏感文件 {} 权限过宽 ({:03o})", path.display(), mode));
                            remedies.push(format!("chmod 600 {}", path.display()));
                        }
                    }
                }
            }

            let inbox_dir = config
                .and_then(|c| c.mesh.inbox_dir.as_deref())
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| data_dir.join("mesh_inbox"));
            if inbox_dir.exists() {
                if let Ok(meta) = fs::metadata(&inbox_dir) {
                    let mode = meta.permissions().mode() & 0o777;
                    if (mode & 0o077) != 0 {
                        issues.push(format!("Mesh 收件箱 {} 权限过宽 ({:03o})", inbox_dir.display(), mode));
                        remedies.push(format!("chmod 700 {}", inbox_dir.display()));
                    }
                }
            }
        }

        if let Some(cfg) = config {
            let non_loopback = !matches!(cfg.web.host.trim(), "127.0.0.1" | "localhost" | "::1");
            let empty_token = cfg
                .web
                .auth_token
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true);
            if cfg.web.enabled && non_loopback && empty_token {
                issues.push(format!("Web 服务监听非回环地址 `{}` 且未配置 auth_token", cfg.web.host));
                remedies.push("在 config.toml 的 [web] 下配置强 auth_token，或将 host 收敛回 127.0.0.1".to_string());
            }
            let empty_mesh_secret = cfg
                .mesh
                .shared_secret
                .as_deref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
                && cfg.mesh.peer_keys.is_empty()
                && empty_token;
            if cfg.mesh.enabled && empty_mesh_secret {
                issues.push("Peer Mesh 已启用但未配置 [mesh].shared_secret / peer_keys / [web].auth_token".to_string());
                remedies.push("在 config.toml 的 [mesh] 下配置 shared_secret 或 peer_keys".to_string());
            }
        }

        if issues.is_empty() {
            report.add(CheckItem {
                name: "安全基线与文件权限".to_string(),
                category: CheckCategory::Optional,
                status: CheckStatus::Pass,
                message: "本地存储权限 (0700/0600) 与接口暴露策略合规".to_string(),
                remedy: None,
            });
        } else {
            report.add(CheckItem {
                name: "安全基线与文件权限".to_string(),
                category: CheckCategory::Optional,
                status: CheckStatus::Warn,
                message: issues.join("; "),
                remedy: Some(remedies.join(" && ")),
            });
        }
    }

    /// 检查 /proc 虚拟文件系统 (Linux 进程与端口拓扑依赖)
    pub fn check_procfs(report: &mut PreflightReport) {
        // Phase 18：非 Linux 平台使用 PlatformProbe，不要求 /proc
        if std::env::consts::OS != "linux" {
            let probe = crate::platform::current_probe();
            report.add(CheckItem {
                name: "平台原生探针".to_string(),
                category: CheckCategory::Informational,
                status: CheckStatus::Pass,
                message: format!("当前 OS 为 {}，使用 `{}` 平台探针（lsof / netstat 等系统自带命令）", std::env::consts::OS, probe.name()),
                remedy: None,
            });
            return;
        }

        let proc_tcp = Path::new("/proc/net/tcp");
        let proc_root = Path::new("/proc");

        if !proc_root.exists() {
            report.add(CheckItem {
                name: "Linux /proc 文件系统".to_string(),
                category: CheckCategory::Required,
                status: CheckStatus::Fail,
                message: "/proc 目录不存在，非 Linux 系统或运行在受限容器内".to_string(),
                remedy: Some("若在 Docker 中运行，请添加挂载卷 -v /proc:/proc:ro 或 --pid=host".to_string()),
            });
            return;
        }

        if proc_tcp.exists() {
            match fs::read_to_string(proc_tcp) {
                Ok(_) => {
                    report.add(CheckItem {
                        name: "Linux /proc 进程与网络接口".to_string(),
                        category: CheckCategory::Required,
                        status: CheckStatus::Pass,
                        message: "/proc/net/tcp 及进程空间可正常读取".to_string(),
                        remedy: None,
                    });
                }
                Err(e) => {
                    report.add(CheckItem {
                        name: "Linux /proc 进程与网络接口".to_string(),
                        category: CheckCategory::Required,
                        status: CheckStatus::Fail,
                        message: format!("/proc/net/tcp 存在但读取失败: {}", e),
                        remedy: Some("检查当前用户读取 /proc 权限或容器 capabilities (CAP_SYS_PTRACE/CAP_NET_ADMIN)".to_string()),
                    });
                }
            }
        } else {
            report.add(CheckItem {
                name: "Linux /proc 进程与网络接口".to_string(),
                category: CheckCategory::Required,
                status: CheckStatus::Fail,
                message: "/proc/net/tcp 不存在，无法采集系统监听端口".to_string(),
                remedy: Some("请在支持 procfs 的 Linux 内核环境下运行".to_string()),
            });
        }
    }

    /// 检查文件系统实时监听内核事件机制 (Linux: inotify / macOS: FSEvents / Windows: ReadDirectoryChangesW)
    pub fn check_inotify(report: &mut PreflightReport) {
        use notify::{Config as NotifyConfig, RecommendedWatcher, Watcher};

        let os = std::env::consts::OS;
        let check_name = match os {
            "macos" => "FSEvents 实时文件系统监听",
            "windows" => "ReadDirectoryChangesW 实时文件系统监听",
            _ => "inotify 实时文件系统监听",
        }
        .to_string();
        let pass_msg = match os {
            "macos" => "macOS FSEvents 文件系统事件机制就绪",
            "windows" => "Windows 文件系统事件机制就绪",
            _ => "内核 inotify 事件机制就绪",
        }
        .to_string();
        let watch_remedy = match os {
            "macos" => "请检查进程文件描述符上限 (ulimit -n) 或目录权限",
            "windows" => "请检查目标工作区目录权限",
            _ => "请检查 /proc/sys/fs/inotify/max_user_watches 是否耗尽",
        }
        .to_string();
        let fail_remedy = match os {
            "macos" => "确保 macOS FSEvents 服务可用且未触发文件描述符上限",
            "windows" => "确保目标卷支持目录变更通知机制",
            _ => "确保内核启用 CONFIG_INOTIFY_USER，且未超出系统 max_user_instances 限制",
        }
        .to_string();

        let temp_dir = std::env::temp_dir();
        let (tx, _rx) = std::sync::mpsc::channel();

        match RecommendedWatcher::new(tx, NotifyConfig::default()) {
            Ok(mut watcher) => {
                if let Err(e) = watcher.watch(&temp_dir, notify::RecursiveMode::NonRecursive) {
                    report.add(CheckItem {
                        name: check_name,
                        category: CheckCategory::Required,
                        status: CheckStatus::Warn,
                        message: format!("Watcher 初始化成功但监听临时目录失败: {}", e),
                        remedy: Some(watch_remedy),
                    });
                } else {
                    if let Err(e) = watcher.unwatch(&temp_dir) {
                        eprintln!("[Preflight][WARN] 取消临时目录监听失败: {}", e);
                    }
                    report.add(CheckItem {
                        name: check_name,
                        category: CheckCategory::Required,
                        status: CheckStatus::Pass,
                        message: pass_msg,
                        remedy: None,
                    });
                }
            }
            Err(e) => {
                report.add(CheckItem {
                    name: check_name,
                    category: CheckCategory::Required,
                    status: CheckStatus::Fail,
                    message: format!("创建文件系统 Watcher 失败: {}", e),
                    remedy: Some(fail_remedy),
                });
            }
        }
    }

    /// 检查数据存储目录可写性
    pub fn check_data_dir(report: &mut PreflightReport, data_dir: &Path) {
        if !data_dir.exists() {
            if let Err(e) = fs::create_dir_all(data_dir) {
                report.add(CheckItem {
                    name: "数据存储目录写入权限".to_string(),
                    category: CheckCategory::Required,
                    status: CheckStatus::Fail,
                    message: format!("无法创建数据目录 {:?}: {}", data_dir, e),
                    remedy: Some(format!("请检查父目录写权限: mkdir -p {:?}", data_dir)),
                });
                return;
            }
        }

        // 测试创建并删除临时标记文件
        let test_file = data_dir.join(".preflight_write_test");
        match fs::write(&test_file, b"ok") {
            Ok(_) => {
                if let Err(e) = fs::remove_file(&test_file) {
                    eprintln!("[Preflight][WARN] 清理预检临时文件失败: {}", e);
                }
                report.add(CheckItem {
                    name: "数据存储目录写入权限".to_string(),
                    category: CheckCategory::Required,
                    status: CheckStatus::Pass,
                    message: format!("数据目录 {:?} 可读写", data_dir),
                    remedy: None,
                });
            }
            Err(e) => {
                report.add(CheckItem {
                    name: "数据存储目录写入权限".to_string(),
                    category: CheckCategory::Required,
                    status: CheckStatus::Fail,
                    message: format!("数据目录 {:?} 无法写入: {}", data_dir, e),
                    remedy: Some(format!("修改目录属主或权限: chmod u+w {:?}", data_dir)),
                });
            }
        }
    }

    /// 检查内置 SQLite 引擎 (含 FTS5)
    pub fn check_sqlite(report: &mut PreflightReport) {
        match rusqlite::Connection::open_in_memory() {
            Ok(conn) => {
                // 测试 FTS5 是否编译在内
                let fts_check = conn.execute("CREATE VIRTUAL TABLE _test_fts USING fts5(content);", []);
                match fts_check {
                    Ok(_) => {
                        report.add(CheckItem {
                            name: "SQLite3 + FTS5 存储引擎".to_string(),
                            category: CheckCategory::Required,
                            status: CheckStatus::Pass,
                            message: "嵌入式 SQLite 及 FTS5 全文检索模块运行正常".to_string(),
                            remedy: None,
                        });
                    }
                    Err(e) => {
                        report.add(CheckItem {
                            name: "SQLite3 + FTS5 存储引擎".to_string(),
                            category: CheckCategory::Required,
                            status: CheckStatus::Fail,
                            message: format!("SQLite 已加载但 FTS5 不可用: {}", e),
                            remedy: Some("请确保 Cargo.toml 中 rusqlite 包含 features = ['bundled']".to_string()),
                        });
                    }
                }
            }
            Err(e) => {
                report.add(CheckItem {
                    name: "SQLite3 + FTS5 存储引擎".to_string(),
                    category: CheckCategory::Required,
                    status: CheckStatus::Fail,
                    message: format!("内存 SQLite 启动失败: {}", e),
                    remedy: Some("检查动态链接库 libc / libm 是否完整".to_string()),
                });
            }
        }
    }

    /// 检查嵌入式 DuckDB OLAP 引擎
    pub fn check_duckdb(report: &mut PreflightReport) {
        match duckdb::Connection::open_in_memory() {
            Ok(conn) => {
                let test_query: duckdb::Result<i64> = conn.query_row("SELECT 1 + 1", [], |row| row.get(0));
                match test_query {
                    Ok(2) => {
                        report.add(CheckItem {
                            name: "DuckDB 列存分析引擎".to_string(),
                            category: CheckCategory::Optional,
                            status: CheckStatus::Pass,
                            message: "嵌入式 DuckDB 内存引擎就绪".to_string(),
                            remedy: None,
                        });
                    }
                    _ => {
                        report.add(CheckItem {
                            name: "DuckDB 列存分析引擎".to_string(),
                            category: CheckCategory::Optional,
                            status: CheckStatus::Warn,
                            message: "DuckDB 启动异常".to_string(),
                            remedy: Some("检查 C++ 标准运行时库 libstdc++.so.6".to_string()),
                        });
                    }
                }
            }
            Err(e) => {
                report.add(CheckItem {
                    name: "DuckDB 列存分析引擎".to_string(),
                    category: CheckCategory::Optional,
                    status: CheckStatus::Warn,
                    message: format!("DuckDB 初始化失败: {}", e),
                    remedy: Some("确保宿主安装了 g++ 和 libstdc++6".to_string()),
                });
            }
        }
    }

    /// 检查宿主后台守护系统 (Linux Systemd / macOS LaunchAgent / Windows Service)
    pub fn check_systemd(report: &mut PreflightReport) {
        match std::env::consts::OS {
            "macos" => {
                let launchctl_ready = Path::new("/bin/launchctl").exists();
                report.add(CheckItem {
                    name: "macOS LaunchAgent 守护环境".to_string(),
                    category: CheckCategory::Optional,
                    status: CheckStatus::Pass,
                    message: if launchctl_ready {
                        "系统由 launchd/LaunchAgent 纳管，支持后台常驻守护与自启".to_string()
                    } else {
                        "macOS 支持后台守护进程模式 (agent-body daemon)".to_string()
                    },
                    remedy: None,
                });
            }
            "windows" => {
                report.add(CheckItem {
                    name: "Windows 守护服务环境".to_string(),
                    category: CheckCategory::Optional,
                    status: CheckStatus::Pass,
                    message: "支持 Windows 任务计划与后台守护进程运行".to_string(),
                    remedy: None,
                });
            }
            _ => {
                let systemd_dir = Path::new("/run/systemd/system");
                if systemd_dir.exists() {
                    report.add(CheckItem {
                        name: "Systemd 守护环境".to_string(),
                        category: CheckCategory::Optional,
                        status: CheckStatus::Pass,
                        message: "系统由 Systemd 纳管，支持自动化拓扑采集与 service 后台运行".to_string(),
                        remedy: None,
                    });
                } else {
                    report.add(CheckItem {
                        name: "Systemd 守护环境".to_string(),
                        category: CheckCategory::Optional,
                        status: CheckStatus::Warn,
                        message: "/run/systemd/system 不存在，当前系统可能使用 OpenRC、SysVinit 或处于 Docker 容器内".to_string(),
                        remedy: Some("若无需 Systemd 拓扑扫描可忽略此告警；后台运行建议采用 Docker 或 nohup/supervisor".to_string()),
                    });
                }
            }
        }
    }

    /// 按优先级解析宿主机可执行文件路径（兼容 Systemd 最小 PATH 下定位 NVM / Cargo / local 工具链）
    pub fn resolve_binary_path(bin_name: &str) -> Option<PathBuf> {
        if let Some(path_env) = std::env::var_os("PATH") {
            for dir in std::env::split_paths(&path_env) {
                let cand = dir.join(bin_name);
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }

        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let mut candidate_dirs = vec![
            PathBuf::from("/usr/local/bin"),
            PathBuf::from("/usr/bin"),
            PathBuf::from(&home).join(".cargo/bin"),
            PathBuf::from(&home).join(".local/bin"),
        ];

        let nvm_versions = PathBuf::from(&home).join(".nvm/versions/node");
        if let Ok(entries) = fs::read_dir(&nvm_versions) {
            let mut nvm_bins: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path().join("bin"))
                .filter(|p| p.is_dir())
                .collect();
            nvm_bins.sort();
            nvm_bins.reverse();
            candidate_dirs.extend(nvm_bins);
        }

        for dir in candidate_dirs {
            let cand = dir.join(bin_name);
            if cand.is_file() {
                return Some(cand);
            }
        }
        None
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preflight_procfs_and_sqlite() {
        let mut report = PreflightReport::new();
        PreflightChecker::check_procfs(&mut report);
        PreflightChecker::check_sqlite(&mut report);

        assert!(!report.items.is_empty());
        if std::env::consts::OS == "linux" {
            let procfs_item = report.items.iter().find(|i| i.name.contains("/proc")).unwrap();
            assert_eq!(procfs_item.category, CheckCategory::Required);
        } else {
            let probe_item = report.items.iter().find(|i| i.name.contains("平台原生探针")).unwrap();
            assert_eq!(probe_item.status, CheckStatus::Pass);
        }

        let sqlite_item = report.items.iter().find(|i| i.name.contains("SQLite3")).unwrap();
        assert_eq!(sqlite_item.status, CheckStatus::Pass);
    }

    #[test]
    fn test_preflight_report_table_format() {
        let mut report = PreflightReport::new();
        report.add(CheckItem {
            name: "测试组件".to_string(),
            category: CheckCategory::Required,
            status: CheckStatus::Pass,
            message: "工作正常".to_string(),
            remedy: None,
        });
        report.add(CheckItem {
            name: "可选组件".to_string(),
            category: CheckCategory::Optional,
            status: CheckStatus::Warn,
            message: "未安装，已降级".to_string(),
            remedy: Some("apt install xxx".to_string()),
        });

        let table = report.format_table();
        assert!(table.contains("测试组件"));
        assert!(table.contains("可选组件"));
        assert!(table.contains("PASS"));
        assert!(table.contains("WARN"));
        assert!(table.contains("apt install xxx"));
    }

    #[cfg(unix)]
    #[test]
    fn test_preflight_security_posture_detects_wide_permissions_and_passes_strict() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("tempdir");
        let data_dir = dir.path().join(".agent-body");
        fs::create_dir_all(&data_dir).expect("mkdir");
        let db_file = data_dir.join("body.db");
        fs::write(&db_file, b"").expect("write db");

        // 1. 故意放宽权限到 0755 / 0644 -> 应检测为 Warn
        fs::set_permissions(&data_dir, fs::Permissions::from_mode(0o755)).expect("chmod dir");
        fs::set_permissions(&db_file, fs::Permissions::from_mode(0o644)).expect("chmod db");

        let mut report_warn = PreflightReport::new();
        PreflightChecker::check_security_posture(&mut report_warn, &data_dir, None);
        let item_warn = report_warn.items.iter().find(|i| i.name.contains("安全基线")).expect("item");
        assert_eq!(item_warn.status, CheckStatus::Warn);
        assert!(item_warn.remedy.as_deref().unwrap_or("").contains("chmod 600"));

        // 2. 收敛权限为 0700 / 0600 -> 应检测为 Pass
        crate::config::restrict_private_permissions(&data_dir, true);
        crate::config::restrict_private_permissions(&db_file, false);

        let mut report_ok = PreflightReport::new();
        PreflightChecker::check_security_posture(&mut report_ok, &data_dir, None);
        let item_ok = report_ok.items.iter().find(|i| i.name.contains("安全基线")).expect("item");
        assert_eq!(item_ok.status, CheckStatus::Pass);
    }

    #[test]
    fn test_resolve_binary_path_finds_sh_and_handles_missing() {
        assert!(PreflightChecker::resolve_binary_path("sh").is_some());
        assert!(PreflightChecker::resolve_binary_path("__nonexistent_body_bin_999__").is_none());
    }

    #[test]
    fn test_check_storage_backend_variants() {
        let dir = tempfile::tempdir().expect("tempdir");

        // 1. sqlite
        let mut cfg_sqlite = crate::config::Config::default();
        cfg_sqlite.storage.provider = "sqlite".to_string();
        cfg_sqlite.db_path = dir.path().join("body.db");
        let mut rep_sqlite = PreflightReport::new();
        PreflightChecker::check_storage_backend(&mut rep_sqlite, &cfg_sqlite);
        assert!(rep_sqlite.passed_required);
        let item_sq = rep_sqlite
            .items
            .iter()
            .find(|i| i.name.contains("Storage Backend"))
            .expect("sqlite item");
        assert_eq!(item_sq.status, CheckStatus::Pass);

        // 2. duckdb
        let mut cfg_duck = crate::config::Config::default();
        cfg_duck.storage.provider = "duckdb".to_string();
        cfg_duck.storage.duckdb_path = Some(dir.path().join("body.duckdb"));
        let mut rep_duck = PreflightReport::new();
        PreflightChecker::check_storage_backend(&mut rep_duck, &cfg_duck);
        assert!(rep_duck.passed_required);
        assert_eq!(rep_duck.items[0].status, CheckStatus::Pass);

        // 3. invalid provider
        let mut cfg_bad = crate::config::Config::default();
        cfg_bad.storage.provider = "redis".to_string();
        let mut rep_bad = PreflightReport::new();
        PreflightChecker::check_storage_backend(&mut rep_bad, &cfg_bad);
        assert!(!rep_bad.passed_required);
        assert_eq!(rep_bad.items[0].status, CheckStatus::Fail);

        // 4. unreachable postgres must fail and redact password
        let mut cfg_pg = crate::config::Config::default();
        cfg_pg.storage.provider = "postgres".to_string();
        cfg_pg.storage.pg_url = Some("postgres://user:SuperSecret99@127.0.0.1:1/db".to_string());
        let mut rep_pg = PreflightReport::new();
        PreflightChecker::check_storage_backend(&mut rep_pg, &cfg_pg);
        assert!(!rep_pg.passed_required);
        assert_eq!(rep_pg.items[0].status, CheckStatus::Fail);
        assert!(
            !rep_pg.items[0].message.contains("SuperSecret99"),
            "PG 错误信息禁止泄露明文密码: {}",
            rep_pg.items[0].message
        );
    }

    #[test]
    fn test_preflight_platform_watcher_daemon_and_sysinfo_branching() {
        let mut report = PreflightReport::new();
        PreflightChecker::check_inotify(&mut report);
        PreflightChecker::check_systemd(&mut report);
        PreflightChecker::collect_sys_info(&mut report);

        if std::env::consts::OS == "macos" {
            let fs_item = report
                .items
                .iter()
                .find(|i| i.name.contains("FSEvents"))
                .expect("FSEvents item");
            assert_eq!(fs_item.status, CheckStatus::Pass);
            let daemon_item = report
                .items
                .iter()
                .find(|i| i.name.contains("LaunchAgent"))
                .expect("LaunchAgent item");
            assert_eq!(daemon_item.status, CheckStatus::Pass);
            assert!(
                !report.items.iter().any(|i| i.name.contains("Systemd")),
                "macOS 不应输出 Systemd WARN"
            );
        }
        assert!(report.items.iter().any(|i| i.name.contains("Agent Body")));
    }
}
