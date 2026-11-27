use clap::Parser;
use anyhow::Result;
use agent_body::config::Config;
use agent_body::storage::Storage;

mod cli_args;
mod cli_subcommands;
use cli_args::*;

mod cli_handlers;

fn main() -> Result<()> {
    #[cfg(unix)]
    unsafe {
        extern "C" {
            fn signal(sig: i32, handler: usize) -> usize;
        }
        // 恢复 SIGPIPE 默认行为，避免输出被管道下游（如 | head）提前关闭时触发 Broken pipe panic
        signal(13, 0);
    }

    let cli = Cli::parse();
    let cfg = Config::load_or_default(cli.config.as_deref())?;
    let storage = Storage::from_config(&cfg)?;
    let audit_storage = storage.clone();
    let cli_t0 = std::time::Instant::now();
    let cli_cmd_name = cli_command_label(&cli.command);
    let skip_cli_audit = matches!(
        cli.command,
        Commands::Mcp { .. }
            | Commands::Daemon
            | Commands::Web { .. }
            | Commands::Tui { .. }
            | Commands::Bench { .. }
            | Commands::Demo { .. }
    );

    let exec_res: Result<()> = (|| -> Result<()> {
        cli_handlers::dispatch(cli.command, &cfg, storage)?;
        Ok(())
    })();

    if !skip_cli_audit {
        let outcome = if exec_res.is_ok() { "success" } else { "failed" };
        agent_body::audit::AuditLogger::record_best_effort(
            &audit_storage,
            agent_body::audit::AuditEventInput::in_agent("cli", "cli_command", cli_cmd_name)
                .with_target("command", cli_cmd_name)
                .with_outcome(outcome)
                .with_elapsed_us(cli_t0.elapsed().as_micros() as i64)
                .with_delta(serde_json::json!({
                    "ok": exec_res.is_ok(),
                    "error": exec_res.as_ref().err().map(|e| e.to_string()),
                })),
        );
    }

    exec_res
}

fn cli_command_label(cmd: &Commands) -> &'static str {
    match cmd {
        Commands::Status => "status",
        Commands::Tui { .. } => "tui",
        Commands::Brief { .. } => "brief",
        Commands::Place { .. } => "place",
        Commands::Asset { .. } => "asset",
        Commands::Env { .. } => "env",
        Commands::Fact { .. } => "fact",
        Commands::Memory { .. } => "memory",
        Commands::Links { .. } => "links",
        Commands::Changes { .. } => "changes",
        Commands::Close { .. } => "close",
        Commands::Gc { .. } => "gc",
        Commands::Mcp { .. } => "mcp",
        Commands::Daemon => "daemon",
        Commands::Preflight { .. } => "preflight",
        Commands::Web { .. } => "web",
        Commands::Hooks { .. } => "hooks",
        Commands::Storage { .. } => "storage",
        Commands::Coordination { .. } => "coordination",
        Commands::Analytics { .. } => "analytics",
        Commands::McpCatalog { .. } => "mcp-catalog",
        Commands::Skill { .. } => "skill",
        Commands::Service { .. } => "service",
        Commands::Mesh { .. } => "mesh",
        Commands::Platform { .. } => "platform",
        Commands::Demo { .. } => "demo",
        Commands::Bench { .. } => "bench",
        Commands::Feedback { .. } => "feedback",
        Commands::Audit { .. } => "audit",
        Commands::Body { .. } => "body",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_1to1_mcp_subcommands_parse() {
        assert!(Cli::try_parse_from(["steward", "asset", "register", "--path", "/root/new/tmp/a.py", "--type", "script"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "hooks", "pre", "--tool-name", "bind_port", "--args", "{\"port\":8080}"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "hooks", "post", "--tool-name", "write_file", "--args", "{\"path\":\"/tmp/a\"}"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "coordination", "acquire", "-t", "port", "-i", "9090"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "analytics", "heatmap", "--days", "7"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "analytics", "trend", "--metric", "assets", "--days", "14"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "analytics", "export", "--table", "assets", "--output-path", "/root/new/tmp/a.parquet"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "close", "sess-1", "--dry-run"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "mcp-catalog", "scan"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "skill", "route", "PostgreSQL调优", "--budget-tokens", "500"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "service", "bootstrap"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "coordination", "wait", "-t", "port", "-i", "8080"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "coordination", "deadlock"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "mesh", "port", "8080"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "mesh", "acquire", "--peer", "http://10.0.0.2:8756", "-t", "port", "-i", "9000"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "platform", "ports"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "demo", "run", "scene-1"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "feedback", "list"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "feedback", "sync"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "feedback", "reconcile"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "feedback", "probes", "--auto-heal"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "brief"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "brief", "--query", "postgres", "--budget-tokens", "1200"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "brief", "--legacy"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "audit", "query", "--domain", "OUT_OF_AGENT"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "audit", "summary", "--window", "7d"]).is_ok());
        assert!(Cli::try_parse_from(["steward", "audit", "evolve", "--auto-feedback"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "body", "perf-snapshot", "--label", "gold", "--set-baseline"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "body", "perf-awr", "--vs-baseline", "true"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "body", "perf-tune"]).is_ok());

        // 验证 Clap 4 布尔标志支持显式 false 与隐式 true 写法
        let c1 = Cli::try_parse_from(["agent-body", "memory", "search", "pg", "--hybrid=false"]).expect("hybrid=false");
        assert!(matches!(c1.command, Commands::Memory { action: MemoryCommands::Search { hybrid: false, .. } }));
        let c2 = Cli::try_parse_from(["agent-body", "memory", "search", "pg", "--hybrid", "false"]).expect("hybrid false");
        assert!(matches!(c2.command, Commands::Memory { action: MemoryCommands::Search { hybrid: false, .. } }));
        let c3 = Cli::try_parse_from(["agent-body", "memory", "search", "pg", "--hybrid"]).expect("hybrid implicit true");
        assert!(matches!(c3.command, Commands::Memory { action: MemoryCommands::Search { hybrid: true, .. } }));
        let c4 = Cli::try_parse_from(["agent-body", "close", "s1", "--clean-temp=false"]).expect("clean-temp=false");
        assert!(matches!(c4.command, Commands::Close { clean_temp: false, .. }));
    }
}
