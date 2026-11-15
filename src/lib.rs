pub mod config;
pub mod storage;
pub mod placement;
pub mod scanner;
pub mod env;
pub mod memory;
pub mod api;
pub mod watcher;
pub mod daemon;
pub mod links;
pub mod changes;
pub mod session;
pub mod tui;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
