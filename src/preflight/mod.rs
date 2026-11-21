use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use serde::{Deserialize, Serialize};

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
        out.push_str("║                   Agent Host Steward — 运行环境与依赖预检报告 (Preflight)                ║\n");
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
            out.push_str("🚀 结论: 所有 REQUIRED 基础环境依赖全部满足，大管家可稳定上线运行。\n");
        } else {
            out.push_str("🛑 结论: 存在 REQUIRED 基础依赖未满足项！大管家关键功能将受限或无法启动，请参考修复建议。\n");
        }
        out
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
        Self::check_security_posture(&mut report, data_dir, config);

        if let Some(cfg) = config {
            Self::check_ollama(&mut report, &cfg.embedding.ollama_url, &cfg.embedding.model, cfg.embedding.fallback_fts_only);
            Self::check_annotation_model(&mut report, &cfg.annotation);
            Self::check_storage_backend(&mut report, cfg);
        } else {
            Self::check_ollama(&mut report, "http://localhost:11434", "nomic-embed-text", true);
            Self::check_annotation_model(&mut report, &crate::config::AnnotationConfig::default());
        }

        // 3. 信息项 (Informational)
        Self::collect_sys_info(&mut report);

        report
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
                .unwrap_or_else(|| data_dir.join("steward.db"));
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

    /// 检查 inotify 内核事件机制
    pub fn check_inotify(report: &mut PreflightReport) {
        use notify::{Config as NotifyConfig, RecommendedWatcher, Watcher};

        let temp_dir = std::env::temp_dir();
        let (tx, _rx) = std::sync::mpsc::channel();

        match RecommendedWatcher::new(tx, NotifyConfig::default()) {
            Ok(mut watcher) => {
                if let Err(e) = watcher.watch(&temp_dir, notify::RecursiveMode::NonRecursive) {
                    report.add(CheckItem {
                        name: "inotify 实时文件系统监听".to_string(),
                        category: CheckCategory::Required,
                        status: CheckStatus::Warn,
                        message: format!("Watcher 初始化成功但监听临时目录失败: {}", e),
                        remedy: Some("请检查 /proc/sys/fs/inotify/max_user_watches 是否耗尽".to_string()),
                    });
                } else {
                    let _ = watcher.unwatch(&temp_dir);
                    report.add(CheckItem {
                        name: "inotify 实时文件系统监听".to_string(),
                        category: CheckCategory::Required,
                        status: CheckStatus::Pass,
                        message: "内核 inotify 事件机制就绪".to_string(),
                        remedy: None,
                    });
                }
            }
            Err(e) => {
                report.add(CheckItem {
                    name: "inotify 实时文件系统监听".to_string(),
                    category: CheckCategory::Required,
                    status: CheckStatus::Fail,
                    message: format!("创建 inotify Watcher 失败: {}", e),
                    remedy: Some("确保内核启用 CONFIG_INOTIFY_USER，且未超出系统 max_user_instances 限制".to_string()),
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
                let _ = fs::remove_file(test_file);
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
                    Ok(val) if val == 2 => {
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

    /// 检查 Systemd 守护系统
    pub fn check_systemd(report: &mut PreflightReport) {
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

    /// 检查 CodeGraph 宏微联邦支持
    pub fn check_codegraph(report: &mut PreflightReport) {
        if let Some(path) = Self::resolve_binary_path("codegraph") {
            report.add(CheckItem {
                name: "CodeGraph 宏微联邦引擎".to_string(),
                category: CheckCategory::Optional,
                status: CheckStatus::Pass,
                message: format!("已安装 codegraph CLI: {}", path.display()),
                remedy: None,
            });
            return;
        }

        let which_codegraph = std::process::Command::new("which")
            .arg("codegraph")
            .output();

        match which_codegraph {
            Ok(output) if output.status.success() => {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                report.add(CheckItem {
                    name: "CodeGraph 宏微联邦引擎".to_string(),
                    category: CheckCategory::Optional,
                    status: CheckStatus::Pass,
                    message: format!("已安装 codegraph CLI: {}", path),
                    remedy: None,
                });
            }
            _ => {
                report.add(CheckItem {
                    name: "CodeGraph 宏微联邦引擎".to_string(),
                    category: CheckCategory::Optional,
                    status: CheckStatus::Warn,
                    message: "未在 PATH 中检测到 codegraph 命令，微观代码符号穿透将降级为文件级依赖".to_string(),
                    remedy: Some("如需函数级代码联邦调用分析，请安装 codegraph (https://github.com/digoal)".to_string()),
                });
            }
        }
    }

    /// 检查宿主机 SKILL 目录可用性 (Phase 16 生态目录)
    pub fn check_ecosystem_skills(report: &mut PreflightReport) {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let candidates = [
            std::path::PathBuf::from("/root/new/skills"),
            std::path::PathBuf::from(&home).join(".gemini/antigravity-cli/skills"),
        ];
        let existing: Vec<String> = candidates
            .iter()
            .filter(|p| p.is_dir())
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        if existing.is_empty() {
            report.add(CheckItem {
                name: "宿主机 SKILL 生态目录".to_string(),
                category: CheckCategory::Optional,
                status: CheckStatus::Warn,
                message: "未发现默认 SKILL 目录，Context 按需路由器将保持空闲".to_string(),
                remedy: Some("可将常用技能放入 /root/new/skills/ 并执行 agent-steward skill scan".to_string()),
            });
        } else {
            report.add(CheckItem {
                name: "宿主机 SKILL 生态目录".to_string(),
                category: CheckCategory::Optional,
                status: CheckStatus::Pass,
                message: format!("已发现 {} 个 SKILL 根目录: {}", existing.len(), existing.join(", ")),
                remedy: None,
            });
        }
    }

    /// 检查 Ollama 本地元数据自动标注小模型 (Phase 23)
    pub fn check_annotation_model(report: &mut PreflightReport, cfg: &crate::config::AnnotationConfig) {
        if !cfg.enabled {
            report.add(CheckItem {
                name: "Ollama 元数据标注模型".to_string(),
                category: CheckCategory::Optional,
                status: CheckStatus::Warn,
                message: "已手动禁用 [annotation].enabled，将仅使用 L1 确定性语法提取".to_string(),
                remedy: None,
            });
            return;
        }
        let tags_url = format!("{}/api/tags", cfg.ollama_url.trim_end_matches('/'));
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(1500))
            .build();
        match client {
            Ok(cli) => match cli.get(&tags_url).send() {
                Ok(resp) if resp.status().is_success() => {
                    let text = resp.text().unwrap_or_default();
                    if text.contains(&cfg.model) {
                        report.add(CheckItem {
                            name: "Ollama 元数据标注模型".to_string(),
                            category: CheckCategory::Optional,
                            status: CheckStatus::Pass,
                            message: format!("服务正常且已拉取小模型 `{}` (L2 自动语义标注就绪)", cfg.model),
                            remedy: None,
                        });
                    } else {
                        report.add(CheckItem {
                            name: "Ollama 元数据标注模型".to_string(),
                            category: CheckCategory::Optional,
                            status: CheckStatus::Warn,
                            message: format!("Ollama 在线，但未找到标注模型 `{}` (自动降级为 L1 确定性提取)", cfg.model),
                            remedy: Some(format!("执行: ollama pull {}", cfg.model)),
                        });
                    }
                }
                _ => {
                    report.add(CheckItem {
                        name: "Ollama 元数据标注模型".to_string(),
                        category: CheckCategory::Optional,
                        status: CheckStatus::Warn,
                        message: format!("无法连接 Ollama ({})，已自动降级为 L1 确定性元数据提取", cfg.ollama_url),
                        remedy: Some("如需小模型语义标注，请启动 Ollama 并拉取 qwen3.5:0.8b".to_string()),
                    });
                }
            },
            Err(e) => {
                report.add(CheckItem {
                    name: "Ollama 元数据标注模型".to_string(),
                    category: CheckCategory::Optional,
                    status: CheckStatus::Warn,
                    message: format!("HTTP 客户端构建失败: {}", e),
                    remedy: None,
                });
            }
        }
    }

    /// 检查 Ollama 本地向量服务
    pub fn check_ollama(report: &mut PreflightReport, url: &str, model: &str, fallback_fts: bool) {
        let tags_url = format!("{}/api/tags", url.trim_end_matches('/'));
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(1500))
            .build();

        match client {
            Ok(cli) => match cli.get(&tags_url).send() {
                Ok(resp) if resp.status().is_success() => {
                    let text = resp.text().unwrap_or_default();
                    if text.contains(model) {
                        report.add(CheckItem {
                            name: "Ollama 向量服务与模型".to_string(),
                            category: CheckCategory::Optional,
                            status: CheckStatus::Pass,
                            message: format!("服务正常且已拉取嵌入模型 `{}`", model),
                            remedy: None,
                        });
                    } else {
                        report.add(CheckItem {
                            name: "Ollama 向量服务与模型".to_string(),
                            category: CheckCategory::Optional,
                            status: CheckStatus::Warn,
                            message: format!("Ollama 在线，但未找到模型 `{}` (自动降级为 FTS5 检索)", model),
                            remedy: Some(format!("执行: ollama pull {}", model)),
                        });
                    }
                }
                Ok(resp) => {
                    report.add(CheckItem {
                        name: "Ollama 向量服务与模型".to_string(),
                        category: CheckCategory::Optional,
                        status: CheckStatus::Warn,
                        message: format!("Ollama 返回 HTTP 状态码: {} (自动降级为 FTS5 检索)", resp.status()),
                        remedy: Some("检查 Ollama 服务状态: systemctl status ollama".to_string()),
                    });
                }
                Err(_) => {
                    let fallback_msg = if fallback_fts {
                        "已自动降级为本地 SQLite FTS5 纯全文检索"
                    } else {
                        "注意: fallback_fts_only 未开启，向量检索可能直接报错"
                    };
                    report.add(CheckItem {
                        name: "Ollama 向量服务与模型".to_string(),
                        category: CheckCategory::Optional,
                        status: CheckStatus::Warn,
                        message: format!("无法连接至 Ollama ({}) - {}", url, fallback_msg),
                        remedy: Some("如需语义混合检索，启动 Ollama: ollama serve".to_string()),
                    });
                }
            },
            Err(e) => {
                report.add(CheckItem {
                    name: "Ollama 向量服务与模型".to_string(),
                    category: CheckCategory::Optional,
                    status: CheckStatus::Warn,
                    message: format!("HTTP 客户端构建失败: {}", e),
                    remedy: None,
                });
            }
        }
    }

    /// 检查后端存储配置（如配置了 PostgreSQL，验证其连通性与 pgvector）
    pub fn check_storage_backend(_report: &mut PreflightReport, _config: &crate::config::Config) {
        // 若未来支持从 config 直接读 storage_backend，在此做连接探活
    }

    /// 检查根分区磁盘水位与冗余镜像构建缓存（CON-06 防满盘预警）
    pub fn check_disk_watermark(report: &mut PreflightReport) {
        let disks = sysinfo::Disks::new_with_refreshed_list();
        let mut root_used_pct: Option<f64> = None;
        let mut root_avail_gb: f64 = 0.0;
        for d in disks.list() {
            if d.mount_point() == Path::new("/") {
                let total = d.total_space();
                let avail = d.available_space();
                if total > 0 {
                    let used = total.saturating_sub(avail);
                    root_used_pct = Some((used as f64 / total as f64) * 100.0);
                    root_avail_gb = avail as f64 / (1024.0 * 1024.0 * 1024.0);
                }
                break;
            }
        }
        let redundant = crate::env::gc::HostGarbageCollector::scan_redundant_build_caches(&[
            Path::new("/root/new/shared_git"),
        ]);
        let redundant_mb: u64 = redundant.iter().map(|c| c.size_mb).sum();

        match root_used_pct {
            Some(pct) if pct >= 85.0 || redundant_mb > 512 => {
                report.add(CheckItem {
                    name: "Disk Watermark".to_string(),
                    category: CheckCategory::Optional,
                    status: CheckStatus::Warn,
                    message: format!(
                        "根分区使用率 {:.1}% (可用 {:.1} GB)，检出 {} 处镜像冗余构建缓存 (约 {} MB)",
                        pct,
                        root_avail_gb,
                        redundant.len(),
                        redundant_mb
                    ),
                    remedy: Some("执行 agent-steward env gc --dry-run 查看清理建议，同步镜像请用 make sync-mirror (--exclude 'target/')".to_string()),
                });
            }
            Some(pct) => {
                report.add(CheckItem {
                    name: "Disk Watermark".to_string(),
                    category: CheckCategory::Optional,
                    status: CheckStatus::Pass,
                    message: format!("根分区使用率 {:.1}% (可用 {:.1} GB)", pct, root_avail_gb),
                    remedy: None,
                });
            }
            None => {}
        }
    }

    /// 收集系统信息
    pub fn collect_sys_info(report: &mut PreflightReport) {
        // 读取 inotify max_user_watches
        let inotify_watches = fs::read_to_string("/proc/sys/fs/inotify/max_user_watches")
            .unwrap_or_else(|_| "未知".to_string())
            .trim()
            .to_string();

        report.add(CheckItem {
            name: "系统内核 inotify watch 限制".to_string(),
            category: CheckCategory::Informational,
            status: CheckStatus::Pass,
            message: format!("max_user_watches = {} (建议大项目 >= 65536)", inotify_watches),
            remedy: None,
        });

        // 二进制版本与编译环境
        report.add(CheckItem {
            name: "Agent Steward 软件包版本".to_string(),
            category: CheckCategory::Informational,
            status: CheckStatus::Pass,
            message: crate::version_info(),
            remedy: None,
        });
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
        // 在常规 Linux 环境下应通过
        let procfs_item = report.items.iter().find(|i| i.name.contains("/proc")).unwrap();
        assert_eq!(procfs_item.category, CheckCategory::Required);

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
        let data_dir = dir.path().join(".steward");
        fs::create_dir_all(&data_dir).expect("mkdir");
        let db_file = data_dir.join("steward.db");
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
        assert!(PreflightChecker::resolve_binary_path("__nonexistent_steward_bin_999__").is_none());
    }
}
