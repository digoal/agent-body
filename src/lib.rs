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
pub mod fact;
pub mod brief;
pub mod coordination;
pub mod preflight;
pub mod web;
pub mod hooks;
pub mod ecosystem;
pub mod platform;
pub mod mesh;
pub mod demo;
pub mod benchmark;
pub mod feedback;
pub mod audit;
pub mod body;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const BUILD_TIMESTAMP: &str = env!("BUILD_TIMESTAMP");
pub const BUILD_OS: &str = env!("BUILD_OS");
pub const BUILD_ARCH: &str = env!("BUILD_ARCH");
pub const BUILD_GIT_HASH: &str = env!("BUILD_GIT_HASH");

pub fn version_info() -> String {
    format!(
        "v{} ({}, {}/{}, built at {})",
        VERSION, BUILD_GIT_HASH, BUILD_OS, BUILD_ARCH, BUILD_TIMESTAMP
    )
}
