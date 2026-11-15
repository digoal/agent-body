pub mod config;
pub mod storage;
pub mod placement;
pub mod scanner;
pub mod env;
pub mod memory;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
