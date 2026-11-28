//! Agent Body（宿主机具身底座）旗舰运行时 Crate。
//!
//! 秉持「脑体分离 × 递弱代偿 × 微内核防腐」原则，组织为 `27` 个顶层模块、
//! `43` 个 1:1:1 同步原生 MCP 工具、`95` 方法三后端存储 SPI 与 `28` 张核心表，
//! 并与 `kernel/`（`3` Crates）、`contrib/`（`11` 周边五件套，含 `mem_pg_tx` /
//! `obs_telemetry`）及 `distributions/`（`3` 精简发行版）协同演化。

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
