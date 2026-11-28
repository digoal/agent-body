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

    /// 检查事务级记忆后端 (`pg_tx` / `builtin` 等) 与 Phase 78 旁路快权重控制面 (`fast_weight_runtime_url`)
    pub fn check_memory_backend(report: &mut PreflightReport, config: &crate::config::Config) {
        let mem_cfg = &config.memory;
        if let Err(err) = mem_cfg.validate(config.storage.pg_url.as_deref()) {
            let safe_err =
                crate::scanner::metadata::SecretRedactor::redact_single_line(&err.to_string());
            report.add(CheckItem {
                name: "事务级记忆后端 (Memory Backend)".to_string(),
                category: CheckCategory::Required,
                status: CheckStatus::Fail,
                message: format!("[memory] 配置校验失败: {}", safe_err),
                remedy: Some("请检查 config.toml 中 [memory] 字段取值范围与 URL 格式".to_string()),
            });
            let fw_url = mem_cfg.fast_weight_runtime_url.trim();
            let fw_scheme_invalid = !fw_url.is_empty()
                && !(fw_url.starts_with("http://") || fw_url.starts_with("https://"));
            if (mem_cfg.fast_weight_enabled || !fw_url.is_empty()) && !fw_scheme_invalid {
                Self::check_fast_weight_runtime(report, mem_cfg);
            }
            return;
        }

        let provider = mem_cfg.provider.trim();
        if provider.eq_ignore_ascii_case("pg_tx") {
            Self::check_pg_tx_memory_probe(report, config);
        } else {
            report.add(CheckItem {
                name: "长期记忆后端 (Memory Backend)".to_string(),
                category: CheckCategory::Informational,
                status: CheckStatus::Pass,
                message: format!(
                    "provider='{}'（本地 SQLite / 外部适配器模式，未启用独立 pg_tx 事务引擎）",
                    provider
                ),
                remedy: None,
            });
        }

        if mem_cfg.fast_weight_enabled || !mem_cfg.fast_weight_runtime_url.trim().is_empty() {
            Self::check_fast_weight_runtime(report, mem_cfg);
        }
    }

    /// 探活 `pg_tx` 事务记忆引擎与 `ext_mem_pg_tx__*` 表状态（连接错误全程脱敏）
    fn check_pg_tx_memory_probe(report: &mut PreflightReport, config: &crate::config::Config) {
        let mem_cfg = &config.memory;
        let category = if mem_cfg.fallback_to_sqlite {
            CheckCategory::Optional
        } else {
            CheckCategory::Required
        };
        let degraded_status = if mem_cfg.fallback_to_sqlite {
            CheckStatus::Warn
        } else {
            CheckStatus::Fail
        };
        let fallback_hint = if mem_cfg.fallback_to_sqlite {
            "（已开启 fallback_to_sqlite=true，将自动无损降级至本地 SQLite）"
        } else {
            "（fallback_to_sqlite=false，记忆写入/检索将被阻断）"
        };

        let resolved_url = mem_cfg
            .pg_url
            .clone()
            .filter(|s| !s.trim().is_empty())
            .or_else(|| config.storage.pg_url.clone().filter(|s| !s.trim().is_empty()))
            .or_else(|| {
                std::env::var("AGENT_BODY_MEMORY_PG_URL")
                    .ok()
                    .filter(|s| !s.trim().is_empty())
            })
            .unwrap_or_default();

        if resolved_url.trim().is_empty() {
            report.add(CheckItem {
                name: "事务级记忆后端 (Memory Backend)".to_string(),
                category,
                status: degraded_status,
                message: format!(
                    "provider='pg_tx'，但 memory.pg_url / storage.pg_url / AGENT_BODY_MEMORY_PG_URL 均未配置{}",
                    fallback_hint
                ),
                remedy: Some(
                    "在 config.toml 配置 [memory].pg_url 或设置环境变量 AGENT_BODY_MEMORY_PG_URL"
                        .to_string(),
                ),
            });
            return;
        }

        let safe_url = crate::scanner::metadata::SecretRedactor::redact_single_line(&resolved_url);
        let url_owned = resolved_url.clone();
        let probe_res = std::thread::spawn(move || -> anyhow::Result<(String, bool)> {
            use std::str::FromStr;
            let mut pg_cfg = postgres::Config::from_str(&url_owned)?;
            pg_cfg.connect_timeout(Duration::from_secs(2));
            let mut client = pg_cfg.connect(postgres::NoTls)?;
            let row = client.query_one(
                "SELECT current_setting('server_version'), to_regclass('ext_mem_pg_tx__events') IS NOT NULL",
                &[],
            )?;
            let ver: String = row.get(0);
            let has_schema: bool = row.get(1);
            Ok((ver, has_schema))
        })
        .join();

        match probe_res {
            Ok(Ok((ver, has_schema))) => {
                let schema_note = if has_schema {
                    "ext_mem_pg_tx__* 已初始化"
                } else {
                    "首次运行时将自动幂等迁移"
                };
                report.add(CheckItem {
                    name: "事务级记忆后端 (Memory Backend)".to_string(),
                    category,
                    status: CheckStatus::Pass,
                    message: format!(
                        "provider=pg_tx ({}) 已连通，PostgreSQL {}，scope='{}'，vector='{}'（{}）",
                        safe_url, ver, mem_cfg.pg_tenant_scope, mem_cfg.vector_backend, schema_note
                    ),
                    remedy: None,
                });
            }
            Ok(Err(e)) => {
                let safe_err =
                    crate::scanner::metadata::SecretRedactor::redact_single_line(&e.to_string());
                report.add(CheckItem {
                    name: "事务级记忆后端 (Memory Backend)".to_string(),
                    category,
                    status: degraded_status,
                    message: format!(
                        "provider=pg_tx ({}) 连接失败{}: {}",
                        safe_url, fallback_hint, safe_err
                    ),
                    remedy: Some(
                        "检查 PostgreSQL 服务状态、pg_hba 鉴权规则及 [memory].pg_url 配置"
                            .to_string(),
                    ),
                });
            }
            Err(_) => {
                report.add(CheckItem {
                    name: "事务级记忆后端 (Memory Backend)".to_string(),
                    category,
                    status: degraded_status,
                    message: format!("provider=pg_tx ({}) 探活线程异常终止{}", safe_url, fallback_hint),
                    remedy: Some("检查 [memory].pg_url 连接串格式及网络可达性".to_string()),
                });
            }
        }
    }

    /// 探活 `ext-graph-relate` (`librelate_ffi.so` v2) 嵌入式图引擎可用性
    pub fn check_relate_graph_probe(report: &mut PreflightReport, config: &crate::config::Config) {
        if !config.knowledge_graph.relate_enabled {
            return;
        }
        let kg = &config.knowledge_graph;
        let probe = ext_graph_relate::RelateGraphEngine::probe_library(kg.relate_lib_path.as_deref());
        if probe.available {
            report.add(CheckItem {
                name: "Relate 图引擎 (librelate_ffi)".to_string(),
                category: CheckCategory::Optional,
                status: CheckStatus::Pass,
                message: probe.detail,
                remedy: None,
            });
        } else {
            let (category, status) = if kg.relate_fallback_to_sqlite {
                (CheckCategory::Optional, CheckStatus::Warn)
            } else {
                (CheckCategory::Required, CheckStatus::Fail)
            };
            let suffix = if kg.relate_fallback_to_sqlite {
                "（已开启 relate_fallback_to_sqlite=true，将自动无损回退 SQLite/PG）"
            } else {
                "（relate_fallback_to_sqlite=false，Relate 图查询将被阻断）"
            };
            report.add(CheckItem {
                name: "Relate 图引擎 (librelate_ffi)".to_string(),
                category,
                status,
                message: format!("{}{}", probe.detail, suffix),
                remedy: Some(
                    "构建安装 /usr/local/lib/librelate_ffi.so 或设置环境变量 RELATE_LIB"
                        .to_string(),
                ),
            });
        }
    }

    /// 探活 Phase 78 旁路快权重 Model Runtime JSON-RPC 2.0 端点
    fn check_fast_weight_runtime(
        report: &mut PreflightReport,
        mem_cfg: &crate::config::MemoryConfig,
    ) {
        let fw_url = mem_cfg.fast_weight_runtime_url.trim();
        if fw_url.is_empty() {
            report.add(CheckItem {
                name: "旁路快权重控制面 (Fast-Weight)".to_string(),
                category: CheckCategory::Optional,
                status: CheckStatus::Pass,
                message: "已启用 fast_weight_enabled，当前运行于纯元数据血缘模式（fast_weight_runtime_url 留空，零内置训练/推理依赖）".to_string(),
                remedy: None,
            });
            return;
        }

        let safe_fw_url = crate::scanner::metadata::SecretRedactor::redact_single_line(fw_url);
        if !(fw_url.starts_with("http://") || fw_url.starts_with("https://")) {
            let (category, status) = if mem_cfg.fast_weight_enabled {
                (CheckCategory::Required, CheckStatus::Fail)
            } else {
                (CheckCategory::Optional, CheckStatus::Warn)
            };
            report.add(CheckItem {
                name: "旁路快权重控制面 (Fast-Weight)".to_string(),
                category,
                status,
                message: format!(
                    "memory.fast_weight_runtime_url ('{}') 必须以 http:// 或 https:// 开头",
                    safe_fw_url
                ),
                remedy: Some(
                    "修正 [memory].fast_weight_runtime_url 或置空以仅使用元数据血缘控制面"
                        .to_string(),
                ),
            });
            return;
        }

        let rpc_body = serde_json::json!({
            "jsonrpc": "2.0",
            "id": "preflight",
            "method": "fast_weight.health",
            "params": {}
        });
        let client = match reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(1500))
            .build()
        {
            Ok(cli) => cli,
            Err(e) => {
                let safe_err = crate::scanner::metadata::SecretRedactor::redact_single_line(
                    &e.to_string(),
                );
                report.add(CheckItem {
                    name: "旁路快权重控制面 (Fast-Weight)".to_string(),
                    category: CheckCategory::Optional,
                    status: CheckStatus::Warn,
                    message: format!("旁路 HTTP 客户端构建失败: {}", safe_err),
                    remedy: Some("检查系统 TLS/HTTP 运行环境及 [memory].fast_weight_runtime_url".to_string()),
                });
                return;
            }
        };

        match client.post(fw_url).json(&rpc_body).send() {
            Ok(resp) if resp.status().is_success() => {
                let status_code = resp.status();
                match resp.json::<serde_json::Value>() {
                    Ok(payload) => {
                        if let Some(err_val) = payload.get("error").filter(|v| !v.is_null()) {
                            let safe_err = crate::scanner::metadata::SecretRedactor::redact_single_line(
                                &err_val.to_string(),
                            );
                            report.add(CheckItem {
                                name: "旁路快权重控制面 (Fast-Weight)".to_string(),
                                category: CheckCategory::Optional,
                                status: CheckStatus::Warn,
                                message: format!(
                                    "旁路服务 ({}) 返回 JSON-RPC 错误（快照血缘控制面仍可安全运行于元数据/回退模式）: {}",
                                    safe_fw_url, safe_err
                                ),
                                remedy: Some("检查外部 Model Runtime JSON-RPC 接口及 [memory].fast_weight_runtime_url".to_string()),
                            });
                            return;
                        }
                        let result_obj = payload.get("result");
                        let runtime_kind = result_obj
                            .and_then(|r| r.get("runtime_kind"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("http_jsonrpc");
                        let models: Vec<&str> = result_obj
                            .and_then(|r| r.get("active_base_models"))
                            .and_then(|v| v.as_array())
                            .map(|arr| arr.iter().filter_map(|m| m.as_str()).collect())
                            .unwrap_or_default();
                        let models_note = if models.is_empty() {
                            "models=未声明".to_string()
                        } else {
                            format!("models=[{}]", models.join(", "))
                        };
                        report.add(CheckItem {
                            name: "旁路快权重控制面 (Fast-Weight)".to_string(),
                            category: CheckCategory::Optional,
                            status: CheckStatus::Pass,
                            message: format!(
                                "旁路 JSON-RPC 探活就绪 ({}, HTTP {}, runtime_kind='{}', {})",
                                safe_fw_url,
                                status_code.as_u16(),
                                runtime_kind,
                                models_note
                            ),
                            remedy: None,
                        });
                    }
                    Err(e) => {
                        let safe_err = crate::scanner::metadata::SecretRedactor::redact_single_line(
                            &e.to_string(),
                        );
                        report.add(CheckItem {
                            name: "旁路快权重控制面 (Fast-Weight)".to_string(),
                            category: CheckCategory::Optional,
                            status: CheckStatus::Warn,
                            message: format!(
                                "旁路服务 ({}) 响应非合法 JSON-RPC 载荷（快照血缘控制面仍可安全运行于元数据/回退模式）: {}",
                                safe_fw_url, safe_err
                            ),
                            remedy: Some("确认 [memory].fast_weight_runtime_url 指向实现 fast_weight.health 的 JSON-RPC 2.0 端点".to_string()),
                        });
                    }
                }
            }
            Ok(resp) => {
                report.add(CheckItem {
                    name: "旁路快权重控制面 (Fast-Weight)".to_string(),
                    category: CheckCategory::Optional,
                    status: CheckStatus::Warn,
                    message: format!(
                        "旁路服务 ({}) 返回 HTTP {}，快照血缘控制面仍可安全运行于元数据/回退模式",
                        safe_fw_url,
                        resp.status()
                    ),
                    remedy: Some("检查外部快权重服务状态及 [memory].fast_weight_runtime_url".to_string()),
                });
            }
            Err(e) => {
                let safe_err = crate::scanner::metadata::SecretRedactor::redact_single_line(
                    &e.to_string(),
                );
                report.add(CheckItem {
                    name: "旁路快权重控制面 (Fast-Weight)".to_string(),
                    category: CheckCategory::Optional,
                    status: CheckStatus::Warn,
                    message: format!(
                        "旁路服务 ({}) 暂不可达，快照血缘控制面仍可安全运行于元数据/回退模式: {}",
                        safe_fw_url, safe_err
                    ),
                    remedy: Some("如需神经 Compaction，请启动外部服务并核对 [memory].fast_weight_runtime_url".to_string()),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn test_check_memory_backend_builtin_and_pg_tx_redaction() {
        // 1. 默认 builtin -> Informational Pass，零网络开销
        let mut rep_builtin = PreflightReport::new();
        let cfg_builtin = crate::config::Config::default();
        PreflightChecker::check_memory_backend(&mut rep_builtin, &cfg_builtin);
        assert!(rep_builtin.passed_required);
        let item_builtin = rep_builtin
            .items
            .iter()
            .find(|i| i.name.contains("Memory Backend"))
            .expect("builtin memory item");
        assert_eq!(item_builtin.category, CheckCategory::Informational);
        assert_eq!(item_builtin.status, CheckStatus::Pass);
        assert!(item_builtin.message.contains("builtin"));

        // 2. pg_tx + 不可达 URL + fallback_to_sqlite=true -> Optional Warn + 密码脱敏
        let mut cfg_warn = crate::config::Config::default();
        cfg_warn.memory.provider = "pg_tx".to_string();
        cfg_warn.memory.fallback_to_sqlite = true;
        cfg_warn.memory.pg_url = Some("postgres://user:TopSecretFw99@127.0.0.1:1/db".to_string());
        let mut rep_warn = PreflightReport::new();
        PreflightChecker::check_memory_backend(&mut rep_warn, &cfg_warn);
        assert!(rep_warn.passed_required);
        let item_warn = rep_warn
            .items
            .iter()
            .find(|i| i.name.contains("Memory Backend"))
            .expect("pg_tx warn item");
        assert_eq!(item_warn.category, CheckCategory::Optional);
        assert_eq!(item_warn.status, CheckStatus::Warn);
        assert!(
            !item_warn.message.contains("TopSecretFw99"),
            "pg_tx 错误信息禁止泄露明文密码: {}",
            item_warn.message
        );

        // 3. pg_tx + 不可达 URL + fallback_to_sqlite=false -> Required Fail + 密码脱敏
        let mut cfg_fail = crate::config::Config::default();
        cfg_fail.memory.provider = "pg_tx".to_string();
        cfg_fail.memory.fallback_to_sqlite = false;
        cfg_fail.memory.pg_url = Some("postgres://user:TopSecretFw99@127.0.0.1:1/db".to_string());
        let mut rep_fail = PreflightReport::new();
        PreflightChecker::check_memory_backend(&mut rep_fail, &cfg_fail);
        assert!(!rep_fail.passed_required);
        let item_fail = rep_fail
            .items
            .iter()
            .find(|i| i.name.contains("Memory Backend"))
            .expect("pg_tx fail item");
        assert_eq!(item_fail.category, CheckCategory::Required);
        assert_eq!(item_fail.status, CheckStatus::Fail);
        assert!(
            !item_fail.message.contains("TopSecretFw99"),
            "pg_tx 阻断信息禁止泄露明文密码: {}",
            item_fail.message
        );

        // 4. pg_tx + 空 URL + fallback_to_sqlite=false -> Required Fail
        let mut cfg_empty = crate::config::Config::default();
        cfg_empty.memory.provider = "pg_tx".to_string();
        cfg_empty.memory.fallback_to_sqlite = false;
        cfg_empty.memory.pg_url = None;
        cfg_empty.storage.pg_url = None;
        let mut rep_empty = PreflightReport::new();
        PreflightChecker::check_memory_backend(&mut rep_empty, &cfg_empty);
        assert!(!rep_empty.passed_required);
        let item_empty = rep_empty
            .items
            .iter()
            .find(|i| i.name.contains("Memory Backend"))
            .expect("pg_tx empty url item");
        assert_eq!(item_empty.category, CheckCategory::Required);
        assert_eq!(item_empty.status, CheckStatus::Fail);
    }

    #[test]
    fn test_check_memory_backend_fast_weight_runtime_modes() {
        // 1. fast_weight_enabled=true + 空 URL -> Optional Pass (纯元数据血缘模式)
        let mut cfg_meta = crate::config::Config::default();
        cfg_meta.memory.fast_weight_enabled = true;
        cfg_meta.memory.fast_weight_runtime_url.clear();
        let mut rep_meta = PreflightReport::new();
        PreflightChecker::check_memory_backend(&mut rep_meta, &cfg_meta);
        assert!(rep_meta.passed_required);
        let item_meta = rep_meta
            .items
            .iter()
            .find(|i| i.name.contains("Fast-Weight"))
            .expect("fast-weight meta item");
        assert_eq!(item_meta.category, CheckCategory::Optional);
        assert_eq!(item_meta.status, CheckStatus::Pass);
        assert!(item_meta.message.contains("纯元数据血缘模式"));

        // 2. fast_weight_enabled=true + 不可达 URL -> Optional Warn
        let mut cfg_unreach = crate::config::Config::default();
        cfg_unreach.memory.fast_weight_enabled = true;
        cfg_unreach.memory.fast_weight_runtime_url = "http://127.0.0.1:1/rpc".to_string();
        let mut rep_unreach = PreflightReport::new();
        PreflightChecker::check_memory_backend(&mut rep_unreach, &cfg_unreach);
        assert!(rep_unreach.passed_required);
        let item_unreach = rep_unreach
            .items
            .iter()
            .find(|i| i.name.contains("Fast-Weight"))
            .expect("fast-weight unreachable item");
        assert_eq!(item_unreach.category, CheckCategory::Optional);
        assert_eq!(item_unreach.status, CheckStatus::Warn);

        // 3. 本地临时 TcpListener 响应有效 JSON-RPC 2.0 -> Optional Pass
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local mock listener");
        let addr = listener.local_addr().expect("listener addr");
        let handle = std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0u8; 2048];
                let _read_res = stream.read(&mut buf);
                let body = r#"{"jsonrpc":"2.0","id":"preflight","result":{"runtime_kind":"mock","endpoint":"http://127.0.0.1","reachable":true,"active_base_models":["qwen3-coder-next"],"detail":"ok"}}"#;
                let resp = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _write_res = stream.write_all(resp.as_bytes());
                let _flush_res = stream.flush();
            }
        });

        let mut cfg_ok = crate::config::Config::default();
        cfg_ok.memory.fast_weight_enabled = true;
        cfg_ok.memory.fast_weight_runtime_url = format!("http://{}/rpc", addr);
        let mut rep_ok = PreflightReport::new();
        PreflightChecker::check_memory_backend(&mut rep_ok, &cfg_ok);
        handle.join().expect("mock server thread join");

        assert!(rep_ok.passed_required);
        let item_ok = rep_ok
            .items
            .iter()
            .find(|i| i.name.contains("Fast-Weight"))
            .expect("fast-weight rpc pass item");
        assert_eq!(item_ok.category, CheckCategory::Optional);
        assert_eq!(item_ok.status, CheckStatus::Pass);
        assert!(item_ok.message.contains("mock"));
        assert!(item_ok.message.contains("qwen3-coder-next"));
    }
}
