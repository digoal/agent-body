pub mod core;
pub mod governance;
pub mod ops;

use anyhow::Result;
use agent_body::config::Config;
use agent_body::storage::Storage;
use crate::cli_args::Commands;

pub fn dispatch(cmd: Commands, cfg: &Config, storage: Storage) -> Result<()> {
    match &cmd {
        Commands::Status
        | Commands::Tui { .. }
        | Commands::Brief { .. }
        | Commands::Place { .. }
        | Commands::Asset { .. }
        | Commands::Env { .. }
        | Commands::Memory { .. }
        | Commands::Mcp { .. }
        | Commands::Daemon
        | Commands::Links { .. }
        | Commands::Changes { .. }
        | Commands::Close { .. }
        | Commands::Gc { .. }
        | Commands::Fact { .. } => core::handle_core(cmd, cfg, storage),
        Commands::Storage { .. }
        | Commands::Coordination { .. }
        | Commands::Preflight { .. }
        | Commands::Web { .. }
        | Commands::Hooks { .. }
        | Commands::Analytics { .. }
        | Commands::McpCatalog { .. }
        | Commands::Skill { .. }
        | Commands::Service { .. }
        | Commands::Mesh { .. }
        | Commands::Platform { .. }
        | Commands::Demo { .. } => ops::handle_ops(cmd, cfg, storage),
        Commands::Bench { .. }
        | Commands::Feedback { .. }
        | Commands::Audit { .. }
        | Commands::Body { .. } => governance::handle_governance(cmd, cfg, storage),
    }
}
