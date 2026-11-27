//! `agent-body-dba` CLI 入口（PostgreSQL / 达梦 / 人大金仓专项诊断发行版）

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use agent_body_dba::{build_dba_runtime, ingest_tuples, run_self_check, DbaSentinelConfig};
use agent_body_sdk::{ClientAuthContext, ProbeContext, TransportKind};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "agent-body-dba",
    version,
    about = "Agent Body 跨平台数据库诊断哨兵发行版"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 输出数据库诊断发行版状态摘要
    Status,
    /// 列出已加载扩展清单与四端算子矩阵
    Catalog,
    /// 对指定配置与系统根目录执行单次只读数据库 + OS 探针采集
    Probe {
        #[arg(long, default_value = "/")]
        db_root: PathBuf,
        #[arg(long, default_value = "/")]
        os_root: PathBuf,
    },
    /// 启动 stdio MCP JSON-RPC 2.0 派发循环
    Mcp,
    /// 输出 Prometheus / OpenMetrics 或 JSON 格式实时内核指标
    Metrics {
        #[arg(long, default_value = "prometheus")]
        format: String,
    },
    /// 执行内置异常规则与热点链路自诊断
    Diagnose {
        #[arg(long, default_value_t = 20)]
        limit: u64,
    },
    /// 执行 PG / 达梦 / 人大金仓配置解析与合规脱敏离线自检
    SelfCheck,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Status => cmd_status(),
        Commands::Catalog => cmd_catalog(),
        Commands::Probe { db_root, os_root } => cmd_probe(db_root, os_root),
        Commands::Mcp => cmd_mcp(),
        Commands::Metrics { format } => cmd_metrics(format),
        Commands::Diagnose { limit } => cmd_diagnose(limit),
        Commands::SelfCheck => run_self_check(),
    }
}

fn loopback_auth(transport: TransportKind, method: &str, path: &str, read_only: bool) -> ClientAuthContext {
    ClientAuthContext {
        transport,
        cert: None,
        bearer_token: None,
        method: method.to_string(),
        path: path.to_string(),
        is_loopback: true,
        is_read_only: read_only,
    }
}

fn cmd_status() -> anyhow::Result<()> {
    let cfg = DbaSentinelConfig::default();
    let dba_rt = build_dba_runtime(&cfg, None)?;
    let out = serde_json::json!({
        "distribution": "dba-sentinel",
        "extensions": dba_rt.runtime.installed_metas().len(),
        "parity_procs": dba_rt.runtime.dispatcher().parity_matrix().len(),
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn cmd_catalog() -> anyhow::Result<()> {
    let cfg = DbaSentinelConfig::default();
    let dba_rt = build_dba_runtime(&cfg, None)?;
    let out = serde_json::json!({
        "extensions": dba_rt.runtime.installed_metas(),
        "parity_matrix": dba_rt.runtime.dispatcher().parity_matrix(),
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn cmd_probe(db_root: PathBuf, os_root: PathBuf) -> anyhow::Result<()> {
    let cfg = DbaSentinelConfig {
        db_roots: vec![db_root],
        os_root,
        ..Default::default()
    };
    let dba_rt = build_dba_runtime(&cfg, None)?;
    let auth = loopback_auth(TransportKind::Cli, "GET", "/api/v1/fdw", true);
    let mut all = Vec::new();
    for fdw_id in ["fdw:db_engines", "fdw:linux_ops"] {
        let mut ctx = ProbeContext {
            target_host: Some(cfg.host_id.clone()),
            timeout_ms: 3000,
            ..Default::default()
        };
        let batch = dba_rt.runtime.scan_fdw(fdw_id, None, &mut ctx, &auth)?;
        all.extend(batch);
    }
    ingest_tuples(&dba_rt, &all)?;
    println!("{}", serde_json::to_string_pretty(&all)?);
    Ok(())
}

fn cmd_mcp() -> anyhow::Result<()> {
    let cfg = DbaSentinelConfig::default();
    let dba_rt = build_dba_runtime(&cfg, None)?;
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line_res in stdin.lock().lines() {
        let line = line_res?;
        if line.trim().is_empty() {
            continue;
        }
        let req_val: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(err) => {
                let err_resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": 0,
                    "error": { "code": -32700, "message": format!("parse error: {err}") }
                });
                writeln!(stdout, "{}", serde_json::to_string(&err_resp)?)?;
                stdout.flush()?;
                continue;
            }
        };
        let auth = loopback_auth(TransportKind::StdioMcp, "POST", "/mcp", false);
        let resp = dba_rt.runtime.handle_mcp(&req_val, &auth);
        writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
        stdout.flush()?;
    }
    Ok(())
}

fn cmd_metrics(format: String) -> anyhow::Result<()> {
    let cfg = DbaSentinelConfig::default();
    let dba_rt = build_dba_runtime(&cfg, None)?;
    let auth = loopback_auth(TransportKind::Cli, "GET", "/api/v1/obs/metrics", true);
    let ok = dba_rt
        .runtime
        .handle_cli(
            &["obs".to_string(), "metrics".to_string()],
            Some(serde_json::json!({ "format": format })),
            &auth,
        )
        .map_err(|e| anyhow::anyhow!("metrics 查询失败: {}", e.message()))?;
    if format.eq_ignore_ascii_case("json") {
        println!("{}", serde_json::to_string_pretty(&ok.data)?);
    } else if let Some(txt) = ok.data["metrics_text"].as_str() {
        print!("{txt}");
    } else {
        println!("{}", serde_json::to_string_pretty(&ok.data)?);
    }
    Ok(())
}

fn cmd_diagnose(limit: u64) -> anyhow::Result<()> {
    let cfg = DbaSentinelConfig::default();
    let dba_rt = build_dba_runtime(&cfg, None)?;
    let auth = loopback_auth(TransportKind::Cli, "POST", "/api/v1/obs/diagnose", true);
    let ok = dba_rt
        .runtime
        .handle_cli(
            &["obs".to_string(), "diagnose".to_string()],
            Some(serde_json::json!({ "limit": limit })),
            &auth,
        )
        .map_err(|e| anyhow::anyhow!("diagnose 失败: {}", e.message()))?;
    println!("{}", serde_json::to_string_pretty(&ok.data)?);
    Ok(())
}
