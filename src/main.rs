use std::path::PathBuf;
use clap::{Parser, Subcommand};
use anyhow::Result;
use agent_steward::config::Config;
use agent_steward::storage::Storage;
use agent_steward::placement::{PlacementEngine, PlacementRequest};
use agent_steward::scanner::Scanner;
use agent_steward::env::EnvCollector;
use agent_steward::memory::MemoryManager;

#[derive(Parser)]
#[command(name = "steward", version = agent_steward::VERSION, about = "Agent Host Steward — 宿主环境大管家 (Rust 版)")]
struct Cli {
    #[arg(short, long, help = "自定义配置文件路径")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(about = "查看大管家运行状态与统计")]
    Status,

    #[command(about = "一键获取宿主环境与上下文简报")]
    Brief,

    #[command(about = "新对象放置与目录位置建议")]
    Place {
        #[command(subcommand)]
        action: PlaceCommands,
    },

    #[command(about = "资产分级纳管与检索")]
    Asset {
        #[command(subcommand)]
        action: AssetCommands,
    },

    #[command(about = "宿主系统环境与动态进程")]
    Env {
        #[command(subcommand)]
        action: EnvCommands,
    },

    #[command(about = "跨会话长期记忆管理")]
    Memory {
        #[command(subcommand)]
        action: MemoryCommands,
    },
}

#[derive(Subcommand)]
enum PlaceCommands {
    #[command(about = "获取新对象规范存放位置建议")]
    Suggest {
        #[arg(short, long, help = "对象类别 (project/script/skill/note/tmp/src)")]
        r#type: String,

        #[arg(short, long, help = "对象名称")]
        name: String,

        #[arg(short, long, help = "对象用途与预期生命周期")]
        intent: String,

        #[arg(short, long, help = "保密级别 (public/internal/confidential/secret)", default_value = "internal")]
        security: String,
    },

    #[command(about = "列出当前生效的宿主机目录放置规范")]
    Rules,
}

#[derive(Subcommand)]
enum AssetCommands {
    #[command(about = "执行白黑名单分级扫描")]
    Scan {
        #[arg(help = "目标目录路径")]
        path: PathBuf,
    },

    #[command(about = "全文与语义搜索资产")]
    Search {
        #[arg(help = "搜索关键词")]
        query: String,

        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },

    #[command(about = "列出最近索引的资产")]
    List {
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum EnvCommands {
    #[command(about = "查看操作系统与硬件规格")]
    Show,

    #[command(about = "查看活跃动态任务与普通进程 (过滤系统底噪)")]
    Processes,
}

#[derive(Subcommand)]
enum MemoryCommands {
    #[command(about = "写入一条新记忆")]
    Add {
        #[arg(short, long, help = "记忆类型 (decision/task/experience/fact/note)", default_value = "decision")]
        r#type: String,

        #[arg(short, long, help = "记忆正文内容")]
        content: String,

        #[arg(short, long, help = "一句话摘要")]
        summary: Option<String>,
    },

    #[command(about = "搜索记忆")]
    Search {
        #[arg(help = "搜索关键词")]
        query: String,

        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config::load_or_default(cli.config.as_deref())?;
    let storage = Storage::open(&cfg.db_path)?;

    match cli.command {
        Commands::Status => {
            println!("╔═══════════════════════════════════════════════════════╗");
            println!("║      Agent Host Steward (Rust 版) — 系统状态          ║");
            println!("╚═══════════════════════════════════════════════════════╝");
            println!("版本:           v{}", agent_steward::VERSION);
            println!("数据目录:       {:?}", cfg.data_dir);
            println!("数据库文件:     {:?}", cfg.db_path);
            let assets = storage.list_assets(1000)?;
            let rules = storage.get_placement_rules()?;
            let policies = storage.get_scan_policies()?;
            let procs = storage.list_active_processes()?;
            println!("已纳管资产数:   {}", assets.len());
            println!("放置规范规则:   {} 条", rules.len());
            println!("分级扫描策略:   {} 条", policies.len());
            println!("当前动态进程:   {} 个", procs.len());
        }

        Commands::Brief => {
            let env_collector = EnvCollector::new(storage.clone());
            let summary = env_collector.collect_system_summary();
            let procs = env_collector.collect_and_sync_processes()?;
            let assets = storage.list_assets(5)?;

            println!("=== Steward Brief 上下文简报 ===");
            println!("系统: {} | 内核: {} | 内存使用率: {:.1}%", summary.os_name, summary.kernel_version, summary.memory_used_pct);
            println!("活跃动态任务进程 (前 5):");
            for p in procs.iter().take(5) {
                let mem_mb = p.mem_bytes as f64 / (1024.0 * 1024.0);
                println!("  - [PID {}] {} (CPU {:.1}%, 内存 {:.1}MB) -> {}", p.pid, p.name, p.cpu_pct, mem_mb, p.command);
            }
            println!("最近活跃资产:");
            for a in assets {
                println!("  - [{}] {} ({})", a.asset_type, a.path, a.scan_tier);
            }
        }

        Commands::Place { action } => {
            let engine = PlacementEngine::new(storage.clone());
            match action {
                PlaceCommands::Suggest { r#type, name, intent, security } => {
                    let req = PlacementRequest {
                        object_type: r#type,
                        name,
                        intent,
                        tags: vec![],
                        security_level: security,
                    };
                    let suggestion = engine.suggest(&req)?;
                    println!("╔═══════════════════════════════════════════════════════╗");
                    println!("║             Steward 对象放置与路径建议                ║");
                    println!("╚═══════════════════════════════════════════════════════╝");
                    println!("推荐目标路径:   {:?}", suggestion.suggested_path);
                    println!("命中规范规则:   {}", suggestion.rule_matched);
                    println!("规则理由:       {}", suggestion.reason);
                    println!("命名建议:       {}", suggestion.naming_convention);
                    println!("冲突状态:       {}", if suggestion.conflict_detected { "⚠️ 路径已存在冲突！" } else { "✅ 路径空闲可用" });
                    println!("伴随开发指引:");
                    for g in suggestion.guidelines {
                        println!("  * {}", g);
                    }
                }
                PlaceCommands::Rules => {
                    let rules = storage.get_placement_rules()?;
                    println!("当前生效的宿主机目录放置规范 (共 {} 条):", rules.len());
                    for r in rules {
                        println!("  - [{}] {} -> {} (优先级 {})", r.rule_name, r.pattern_type, r.target_base_dir, r.priority);
                        if let Some(desc) = r.description {
                            println!("    说明: {}", desc);
                        }
                    }
                }
            }
        }

        Commands::Asset { action } => {
            match action {
                AssetCommands::Scan { path } => {
                    println!("正在执行分级颗粒度扫描: {:?}", path);
                    let scanner = Scanner::new(storage)?;
                    let report = scanner.scan_path(&path)?;
                    println!("扫描完成报告:");
                    println!("  - 扫描目录数:       {}", report.scanned_dirs);
                    println!("  - 深层索引文件数:   {}", report.deep_indexed_files);
                    println!("  - 黑名单阻断目录数: {}", report.shallow_blocked_dirs.len());
                    for b in report.shallow_blocked_dirs {
                        println!("    ★ [Shallow Tier 阻断] {} ({})", b.path, b.reason);
                    }
                }
                AssetCommands::Search { query, limit } => {
                    let results = storage.search_assets_fts(&query, limit)?;
                    println!("资产搜索匹配结果 (共 {} 条):", results.len());
                    for a in results {
                        println!("  - [{}] {} ({:?})", a.asset_type, a.path, a.description);
                    }
                }
                AssetCommands::List { limit } => {
                    let results = storage.list_assets(limit)?;
                    println!("最近资产清单 (前 {} 条):", results.len());
                    for a in results {
                        println!("  - [{:8}] {:6} | {}", a.asset_type, a.scan_tier, a.path);
                    }
                }
            }
        }

        Commands::Env { action } => {
            let collector = EnvCollector::new(storage);
            match action {
                EnvCommands::Show => {
                    let summary = collector.collect_system_summary();
                    println!("=== 系统与硬件规格 ===");
                    println!("OS:       {}", summary.os_name);
                    println!("Kernel:   {}", summary.kernel_version);
                    println!("Host:     {}", summary.host_name);
                    println!("CPU:      {} 核心 ({})", summary.cpu_cores, summary.cpu_brand);
                    println!("Memory:   {} MB 总量 (已用: {} MB, {:.1}%)", summary.total_memory_mb, summary.used_memory_mb, summary.memory_used_pct);
                    println!("Disks:");
                    for d in summary.disks {
                        println!("  - {:<10} 总计 {:.1} GB / 可用 {:.1} GB", d.mount_point, d.total_space_gb, d.available_space_gb);
                    }
                }
                EnvCommands::Processes => {
                    let procs = collector.collect_and_sync_processes()?;
                    println!("活跃动态任务进程 (共 {} 个):", procs.len());
                    println!("{:<8} {:<10} {:<6} {:<10} {:<18} {}", "PID", "NAME", "CPU%", "MEM(MB)", "OWNER", "COMMAND");
                    for p in procs {
                        let mem_mb = p.mem_bytes as f64 / (1024.0 * 1024.0);
                        let cmd_trunc = if p.command.len() > 60 { format!("{}...", &p.command[..60]) } else { p.command };
                        println!("{:<8} {:<10} {:<6.1} {:<10.1} {:<18} {}", p.pid, p.name, p.cpu_pct, mem_mb, p.owner, cmd_trunc);
                    }
                }
            }
        }

        Commands::Memory { action } => {
            let mm = MemoryManager::new(storage);
            match action {
                MemoryCommands::Add { r#type, content, summary } => {
                    let id = mm.add(&r#type, &content, summary.as_deref(), Some("cli"), vec![])?;
                    println!("✅ 记忆已保存, ID: {}", id);
                }
                MemoryCommands::Search { query, limit } => {
                    let results = mm.search(&query, limit)?;
                    println!("记忆搜索结果 (共 {} 条):", results.len());
                    for m in results {
                        println!("  - [{}] {} (ID: {})", m.memory_type, m.content, m.id);
                    }
                }
            }
        }
    }

    Ok(())
}
