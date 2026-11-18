//! 跨平台原生探测抽象层 (Phase 18.1)
//!
//! 统一 `PlatformProbe` Trait 屏蔽 Linux (`/proc` + systemd) / macOS (`lsof` + launchd) /
//! Windows (`netstat` + `tasklist` + `schtasks`) 的宿主机事实采集差异。
//!
//! 设计要点：
//! - 所有输出解析函数（`parse_*`）是无副作用的纯函数，**在所有平台均参与编译**，
//!   便于用真实样本 fixture 在 Linux CI 上验证 macOS / Windows 解析逻辑；
//! - 探针选择采用运行时 `std::env::consts::OS`，避免 `cfg` 导致跨平台代码腐烂；
//! - 严格只读：只执行系统自带的只读查询命令，零捏造（规约 #5）。

pub mod linux;
pub mod macos;
pub mod windows;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::fact::process_net::ListeningPortFact;

/// 自动化服务单元事实（跨平台统一视图）
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlatformServiceFact {
    /// 服务/任务名称（systemd unit / launchd Label / Windows 计划任务路径）
    pub name: String,
    /// 启动命令或程序（若可解析）
    pub command: String,
    /// 类别: "systemd" | "launchd" | "schtasks"
    pub kind: String,
    /// 来源（单元文件路径或查询命令）
    pub source: String,
}

/// 平台探针统一接口
pub trait PlatformProbe: Send + Sync {
    /// 平台标识: "linux" | "macos" | "windows"
    fn name(&self) -> &'static str;
    /// 枚举全系统 TCP LISTEN 端口及其进程事实
    fn listening_ports(&self) -> Result<Vec<ListeningPortFact>>;
    /// 枚举自动化服务/计划任务
    fn services(&self) -> Result<Vec<PlatformServiceFact>>;
}

/// 根据运行时 OS 选择当前平台探针
pub fn current_probe() -> Box<dyn PlatformProbe> {
    probe_for_os(std::env::consts::OS)
}

/// 按 OS 标识构造探针（便于测试）
pub fn probe_for_os(os: &str) -> Box<dyn PlatformProbe> {
    match os {
        "macos" => Box::new(macos::MacosProbe),
        "windows" => Box::new(windows::WindowsProbe),
        _ => Box::new(linux::LinuxProbe),
    }
}

/// 判断是否属于常见系统底噪守护进程（跨平台通用名单）
pub fn is_system_daemon_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    const DAEMONS: [&str; 18] = [
        "systemd", "sshd", "chronyd", "dbus-daemon", "rsyslogd", "cupsd", "avahi-daemon",
        "rpcbind", "kernel", "launchd", "mdnsresponder", "rapportd", "controlcenter",
        "svchost", "system", "lsass", "wininit", "services",
    ];
    DAEMONS.iter().any(|d| lower.contains(d))
}

/// 构造一条端口事实（解析器公用）
pub(crate) fn make_port_fact(
    port: u16,
    protocol: &str,
    bind_ip: String,
    pid: Option<u32>,
    process_name: Option<String>,
    user: Option<String>,
) -> ListeningPortFact {
    let is_system = process_name.as_deref().map(is_system_daemon_name).unwrap_or(false);
    ListeningPortFact {
        port,
        protocol: protocol.to_string(),
        bind_ip,
        pid,
        process_name: process_name.clone(),
        command: process_name,
        cwd: None,
        user,
        is_system_daemon: is_system,
        inode: 0,
    }
}

/// 对端口事实按端口排序并去重
pub(crate) fn finalize_ports(mut ports: Vec<ListeningPortFact>) -> Vec<ListeningPortFact> {
    ports.sort_by_key(|p| p.port);
    ports.dedup_by(|a, b| a.port == b.port && a.protocol == b.protocol);
    ports
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_probe_selection_by_os() {
        assert_eq!(probe_for_os("linux").name(), "linux");
        assert_eq!(probe_for_os("macos").name(), "macos");
        assert_eq!(probe_for_os("windows").name(), "windows");
        assert_eq!(probe_for_os("freebsd").name(), "linux");
    }

    #[test]
    fn test_current_probe_matches_host() {
        let p = current_probe();
        let expected = match std::env::consts::OS {
            "macos" => "macos",
            "windows" => "windows",
            _ => "linux",
        };
        assert_eq!(p.name(), expected);
    }
}
