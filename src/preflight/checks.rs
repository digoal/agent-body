use std::fs;
use std::path::Path;
use std::time::Duration;

use super::*;

impl PreflightChecker {
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
                    remedy: Some("如需函数级代码联邦调用分析，请安装 codegraph 索引工具并加入 PATH".to_string()),
                });
            }
        }
    }

    /// 检查宿主机 SKILL 目录可用性 (Phase 16 生态目录)
    pub fn check_ecosystem_skills(report: &mut PreflightReport) {
        let candidates = crate::config::Config::load_effective_or_default().effective_skills_dirs();
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
                remedy: Some("可将常用技能放入 [workspace].skills_dirs 配置目录并执行 agent-body skill scan".to_string()),
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
                        remedy: Some(format!("如需小模型语义标注，请启动 Ollama 并拉取 {}", cfg.model)),
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

    /// 检查后端存储配置（校验 sqlite / duckdb 可用性，如配置 postgres 则脱敏后探活连通性与版本）
    pub fn check_storage_backend(report: &mut PreflightReport, config: &crate::config::Config) {
        let provider = config.storage.provider.trim().to_lowercase();
        match provider.as_str() {
            "sqlite" | "" => {
                let db_path = &config.db_path;
                let parent_ok = db_path
                    .parent()
                    .map(|p| p.exists() || fs::create_dir_all(p).is_ok())
                    .unwrap_or(true);
                if parent_ok {
                    report.add(CheckItem {
                        name: "主存储后端 (Storage Backend)".to_string(),
                        category: CheckCategory::Required,
                        status: CheckStatus::Pass,
                        message: format!("provider=sqlite，目标路径 {:?}", db_path),
                        remedy: None,
                    });
                } else {
                    report.add(CheckItem {
                        name: "主存储后端 (Storage Backend)".to_string(),
                        category: CheckCategory::Required,
                        status: CheckStatus::Fail,
                        message: format!("provider=sqlite，父目录无法创建或访问: {:?}", db_path),
                        remedy: Some("检查 db_path 配置及目录写权限".to_string()),
                    });
                }
            }
            "duckdb" => {
                let duck_path = config
                    .storage
                    .duckdb_path
                    .clone()
                    .unwrap_or_else(|| config.data_dir.join("body.duckdb"));
                let parent_ok = duck_path
                    .parent()
                    .map(|p| p.exists() || fs::create_dir_all(p).is_ok())
                    .unwrap_or(true);
                if !parent_ok {
                    report.add(CheckItem {
                        name: "主存储后端 (Storage Backend)".to_string(),
                        category: CheckCategory::Required,
                        status: CheckStatus::Fail,
                        message: format!("provider=duckdb，父目录不可写: {:?}", duck_path),
                        remedy: Some("检查 storage.duckdb_path 与目录权限".to_string()),
                    });
                    return;
                }
                match crate::storage::DuckDbStorageProvider::open_in_memory() {
                    Ok(_) => {
                        report.add(CheckItem {
                            name: "主存储后端 (Storage Backend)".to_string(),
                            category: CheckCategory::Required,
                            status: CheckStatus::Pass,
                            message: format!("provider=duckdb，目标路径 {:?}（引擎就绪）", duck_path),
                            remedy: None,
                        });
                    }
                    Err(e) => {
                        report.add(CheckItem {
                            name: "主存储后端 (Storage Backend)".to_string(),
                            category: CheckCategory::Required,
                            status: CheckStatus::Fail,
                            message: format!("provider=duckdb 初始化探测失败: {}", e),
                            remedy: Some("检查 DuckDB 磁盘空间、文件锁或 C++ 运行时库".to_string()),
                        });
                    }
                }
            }
            "postgres" | "pg" | "postgresql" => {
                let raw_url = config
                    .storage
                    .pg_url
                    .as_deref()
                    .unwrap_or("postgresql://postgres@127.0.0.1:5432/agent_body");
                let safe_url = crate::scanner::metadata::SecretRedactor::redact_single_line(raw_url);
                let url_owned = raw_url.to_string();
                let probe_res = std::thread::spawn(move || -> anyhow::Result<String> {
                    use std::str::FromStr;
                    let mut pg_cfg = postgres::Config::from_str(&url_owned)?;
                    pg_cfg.connect_timeout(Duration::from_secs(2));
                    let mut client = pg_cfg.connect(postgres::NoTls)?;
                    let row = client.query_one("SELECT current_setting('server_version')", &[])?;
                    let ver: String = row.get(0);
                    Ok(ver)
                })
                .join();

                match probe_res {
                    Ok(Ok(ver)) => {
                        report.add(CheckItem {
                            name: "主存储后端 (Storage Backend)".to_string(),
                            category: CheckCategory::Required,
                            status: CheckStatus::Pass,
                            message: format!(
                                "provider=postgres ({}) 已连通，PostgreSQL server_version = {}",
                                safe_url, ver
                            ),
                            remedy: None,
                        });
                    }
                    Ok(Err(e)) => {
                        let safe_err =
                            crate::scanner::metadata::SecretRedactor::redact_single_line(&e.to_string());
                        report.add(CheckItem {
                            name: "主存储后端 (Storage Backend)".to_string(),
                            category: CheckCategory::Required,
                            status: CheckStatus::Fail,
                            message: format!("provider=postgres ({}) 连接失败: {}", safe_url, safe_err),
                            remedy: Some(
                                "检查 PostgreSQL 服务状态、pg_hba 鉴权规则及 storage.pg_url 配置"
                                    .to_string(),
                            ),
                        });
                    }
                    Err(_) => {
                        report.add(CheckItem {
                            name: "主存储后端 (Storage Backend)".to_string(),
                            category: CheckCategory::Required,
                            status: CheckStatus::Fail,
                            message: format!("provider=postgres ({}) 探活线程异常终止", safe_url),
                            remedy: Some("检查 storage.pg_url 连接串格式及网络可达性".to_string()),
                        });
                    }
                }
            }
            other => {
                report.add(CheckItem {
                    name: "主存储后端 (Storage Backend)".to_string(),
                    category: CheckCategory::Required,
                    status: CheckStatus::Fail,
                    message: format!("未知存储后端 provider='{}'", other),
                    remedy: Some(
                        "请将 storage.provider 配置为 sqlite、postgres 或 duckdb 之一".to_string(),
                    ),
                });
            }
        }
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
        let shared_git_dirs = crate::config::Config::load_effective_or_default().effective_shared_git_dirs();
        let shared_git_refs: Vec<&Path> = shared_git_dirs.iter().map(|p| p.as_path()).collect();
        let redundant = crate::env::gc::HostGarbageCollector::scan_redundant_build_caches(&shared_git_refs);
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
                    remedy: Some("执行 agent-body env gc --dry-run 查看清理建议，同步镜像请用 make sync-mirror (--exclude 'target/')".to_string()),
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
        match std::env::consts::OS {
            "macos" => {
                report.add(CheckItem {
                    name: "macOS 文件事件与句柄机制".to_string(),
                    category: CheckCategory::Informational,
                    status: CheckStatus::Pass,
                    message: "使用 FSEvents 内核事件流（无需 Linux inotify max_user_watches 配置）".to_string(),
                    remedy: None,
                });
            }
            "windows" => {
                report.add(CheckItem {
                    name: "Windows 文件事件机制".to_string(),
                    category: CheckCategory::Informational,
                    status: CheckStatus::Pass,
                    message: "使用 ReadDirectoryChangesW 文件监控机制".to_string(),
                    remedy: None,
                });
            }
            _ => {
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
            }
        }

        // 二进制版本与编译环境
        report.add(CheckItem {
            name: "Agent Body 软件包版本".to_string(),
            category: CheckCategory::Informational,
            status: CheckStatus::Pass,
            message: crate::version_info(),
            remedy: None,
        });
    }
}
