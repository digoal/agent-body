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

    // 极速零 DB 快路径：distill 包装器与 hooks bridge 无 DB 事件不打开 SQLite
    match &cli.command {
        Commands::Distill {
            kind,
            max_lines,
            max_bytes,
            shell,
            cmd,
        } => {
            let code = agent_body::hooks::run_distill_cli(
                kind,
                *max_lines,
                *max_bytes,
                shell.as_deref(),
                cmd,
            )?;
            if code != 0 {
                std::process::exit(code);
            }
            return Ok(());
        }
        Commands::Hooks {
            action: HooksCommands::Bridge { event },
        } => {
            return agent_body::hooks::bridge::handle_cli_bridge(event, &cfg);
        }
        Commands::Hooks {
            action:
                HooksCommands::Outline {
                    path,
                    max_items,
                    after_line,
                    max_bytes,
                    json,
                },
        } => {
            return agent_body::hooks::bridge::handle_cli_outline(
                path,
                *max_items,
                *after_line,
                *max_bytes,
                *json,
            );
        }
        Commands::Hooks {
            action:
                HooksCommands::ReadLedger {
                    conversation_id,
                    clear,
                    json,
                },
        } => {
            return agent_body::hooks::bridge::handle_cli_read_ledger(
                conversation_id,
                *clear,
                *json,
            );
        }
        _ => {}
    }

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
        Commands::Distill { .. } => "distill",
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
        assert!(Cli::try_parse_from(["agent-body", "asset", "register", "--path", "/root/new/tmp/a.py", "--type", "script"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "hooks", "pre", "--tool-name", "bind_port", "--args", "{\"port\":8080}"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "hooks", "post", "--tool-name", "write_file", "--args", "{\"path\":\"/tmp/a\"}"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "hooks", "bridge", "pre_invocation"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "distill", "--kind", "build_test", "--max-lines", "80", "--shell", "cargo test"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "coordination", "acquire", "-t", "port", "-i", "9090"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "analytics", "heatmap", "--days", "7"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "analytics", "trend", "--metric", "assets", "--days", "14"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "analytics", "export", "--table", "assets", "--output-path", "/root/new/tmp/a.parquet"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "close", "sess-1", "--dry-run"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "mcp-catalog", "scan"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "skill", "route", "PostgreSQL调优", "--budget-tokens", "500"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "service", "bootstrap"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "coordination", "wait", "-t", "port", "-i", "8080"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "coordination", "deadlock"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "mesh", "port", "8080"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "mesh", "acquire", "--peer", "http://10.0.0.2:8756", "-t", "port", "-i", "9000"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "platform", "ports"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "demo", "run", "scene-1"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "feedback", "list"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "feedback", "sync"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "feedback", "reconcile"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "feedback", "probes", "--auto-heal"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "brief"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "brief", "--query", "postgres", "--budget-tokens", "1200"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "brief", "--legacy"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "audit", "query", "--domain", "OUT_OF_AGENT"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "audit", "summary", "--window", "7d"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "audit", "evolve", "--auto-feedback"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "body", "perf-snapshot", "--label", "gold", "--set-baseline"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "body", "perf-awr", "--vs-baseline", "true"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "body", "perf-tune"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "body", "manifest"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "body", "vitals"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "place", "suggest", "-t", "script", "-n", "demo", "--intent", "test"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "place", "rules"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "place", "learn"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "env", "show"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "env", "port", "8080"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "memory", "audit"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "links", "audit"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "links", "graph"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "changes", "--since", "2026-11-15T00:00:00"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "gc", "--max-age-hours", "72"]).is_ok());
        assert!(Cli::try_parse_from(["agent-body", "bench", "report"]).is_ok());

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
