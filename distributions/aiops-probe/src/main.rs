//! `agent-body-probe` CLI 入口（政务云 AIOps 极简探针）

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use agent_body_probe::{build_aiops_runtime, ingest_tuples, run_self_check, AiopsProbeConfig};
use agent_body_sdk::{ClientAuthContext, ProbeContext, TransportKind};
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "agent-body-probe",
    version,
    about = "Agent Body 政务云 AIOps 极简探针发行版"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 输出探针运行态与装配扩展摘要
    Status,
    /// 列出已装配扩展与四端派生算子矩阵
    Catalog,
    /// 对指定 rootfs 目录执行单次只读探针扫描与脱敏验证
    Probe {
        #[arg(long, default_value = "/")]
        root: PathBuf,
    },
    /// 启动 stdio MCP JSON-RPC 2.0 服务循环
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
    /// 执行端到端离线自检（401 鉴权 / Linux / JVM / 轨 B 沙盒 / 脱敏 / 证据包 / 可观测）
    SelfCheck,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Status => cmd_status(),
        Commands::Catalog => cmd_catalog(),
        Commands::Probe { root } => cmd_probe(root),
        Commands::Mcp => cmd_mcp(),
        Commands::Metrics { format } => cmd_metrics(format),
        Commands::Diagnose { limit } => cmd_diagnose(limit),
        Commands::SelfCheck => run_self_check(),
    }
}

fn default_auth(transport: TransportKind, method: &str, path: &str, read_only: bool) -> ClientAuthContext {
    ClientAuthContext {
        transport,
        cert: None,
        bearer_token: Some("local-probe-token".to_string()),
        method: method.to_string(),
        path: path.to_string(),
        is_loopback: true,
        is_read_only: read_only,
    }
}

fn cmd_status() -> anyhow::Result<()> {
    let cfg = AiopsProbeConfig::default();
    let probe_rt = build_aiops_runtime(&cfg, None)?;
    let out = serde_json::json!({
        "distribution": "aiops-probe",
        "extensions": probe_rt.runtime.installed_metas().len(),
        "duckdb_enabled": false,
        "cpp_runtime_deps": false,
        "parity_procs": probe_rt.runtime.dispatcher().parity_matrix().len(),
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn cmd_catalog() -> anyhow::Result<()> {
    let cfg = AiopsProbeConfig::default();
    let probe_rt = build_aiops_runtime(&cfg, None)?;
    let out = serde_json::json!({
        "extensions": probe_rt.runtime.installed_metas(),
        "parity_matrix": probe_rt.runtime.dispatcher().parity_matrix(),
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn cmd_probe(root: PathBuf) -> anyhow::Result<()> {
    let cfg = AiopsProbeConfig {
        root_path: root,
        ..Default::default()
    };
    let probe_rt = build_aiops_runtime(&cfg, None)?;
    let auth = default_auth(TransportKind::Cli, "GET", "/api/v1/fdw", true);
    let mut collected = Vec::new();
    for fdw_id in ["fdw:linux_ops", "fdw:jvm_middleware"] {
        let mut pctx = ProbeContext {
            target_host: Some(cfg.host_id.clone()),
            timeout_ms: 3000,
            ..Default::default()
        };
        let batch = probe_rt.runtime.scan_fdw(fdw_id, None, &mut pctx, &auth)?;
        collected.extend(batch);
    }
    ingest_tuples(&probe_rt, &collected)?;
    println!("{}", serde_json::to_string_pretty(&collected)?);
    Ok(())
}

fn cmd_mcp() -> anyhow::Result<()> {
    let cfg = AiopsProbeConfig::default();
    let probe_rt = build_aiops_runtime(&cfg, None)?;
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
        let auth = default_auth(TransportKind::StdioMcp, "POST", "/mcp", false);
        let resp = probe_rt.runtime.handle_mcp(&req_val, &auth);
        writeln!(stdout, "{}", serde_json::to_string(&resp)?)?;
        stdout.flush()?;
    }
    Ok(())
}

fn cmd_metrics(format: String) -> anyhow::Result<()> {
    let cfg = AiopsProbeConfig::default();
    let probe_rt = build_aiops_runtime(&cfg, None)?;
    let auth = default_auth(TransportKind::Cli, "GET", "/api/v1/obs/metrics", true);
    let ok = probe_rt
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
    let cfg = AiopsProbeConfig::default();
    let probe_rt = build_aiops_runtime(&cfg, None)?;
    let auth = default_auth(TransportKind::Cli, "POST", "/api/v1/obs/diagnose", true);
    let ok = probe_rt
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
