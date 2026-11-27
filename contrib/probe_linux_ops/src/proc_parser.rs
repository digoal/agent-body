//! Linux `/proc` 深度进程、套接字与 Capabilities 纯函数解析器（支持可注入根目录）

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::Path;

use chrono::{DateTime, FixedOffset, Utc};
use serde::{Deserialize, Serialize};

pub const DEFAULT_CMDLINE_MAX_LEN: usize = 4096;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructuredCmdline {
    pub full_command: String,
    pub truncated: bool,
    pub jar_path: Option<String>,
    pub spring_config_location: Option<String>,
    pub spring_profiles_active: Option<String>,
    pub server_port: Option<u16>,
    pub config_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TcpSocketEntry {
    pub protocol: String,
    pub state: String,
    pub local_ip: String,
    pub local_port: u16,
    pub remote_ip: String,
    pub remote_port: u16,
    pub inode: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnixSocketEntry {
    pub state: String,
    pub inode: u64,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinuxProcessFact {
    pub pid: u32,
    pub ppid: Option<u32>,
    pub comm: String,
    pub started_at: Option<String>,
    pub cwd: Option<String>,
    pub cmdline: StructuredCmdline,
    pub listening_sockets: Vec<TcpSocketEntry>,
    pub established_peers: Vec<TcpSocketEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapCheckResult {
    pub cap_eff_hex: Option<String>,
    pub has_dac_read_search: bool,
    pub has_sys_ptrace: bool,
    pub is_root: bool,
    pub degraded_reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LinuxProcSnapshot {
    pub processes: Vec<LinuxProcessFact>,
    pub sockets: Vec<TcpSocketEntry>,
    pub unix_sockets: Vec<UnixSocketEntry>,
    pub cap_check: Option<CapCheckResult>,
}

/// 解析 `/proc/[pid]/stat` 提取 `ppid` 与 RFC3339 (+08:00) `started_at`
pub fn parse_proc_stat_started_at(
    stat_line: &str,
    btime_secs: u64,
    clk_tck: u64,
) -> Option<(u32, String)> {
    let close_paren = stat_line.rfind(')')?;
    let rest = stat_line.get(close_paren + 1..)?.trim_start();
    let fields: Vec<&str> = rest.split_whitespace().collect();
    // 括号后第 0 项为 state，第 1 项为 ppid，第 19 项为 starttime (原第 22 字段)
    let ppid: u32 = fields.get(1)?.parse().ok()?;
    let starttime_ticks: u64 = fields.get(19)?.parse().ok()?;
    let hz = clk_tck.max(1);
    let started_secs = btime_secs.saturating_add(starttime_ticks / hz);
    let rem_nanos = ((starttime_ticks % hz) * 1_000_000_000 / hz) as u32;
    let dt_utc = DateTime::<Utc>::from_timestamp(started_secs as i64, rem_nanos)?;
    let ts = match FixedOffset::east_opt(8 * 3600) {
        Some(off) => dt_utc.with_timezone(&off).to_rfc3339(),
        None => dt_utc.to_rfc3339(),
    };
    Some((ppid, ts))
}

/// 从 `/proc/stat` 解析系统启动时间戳 `btime`
pub fn parse_proc_stat_btime(stat_text: &str) -> Option<u64> {
    for line in stat_text.lines() {
        if let Some(rest) = line.strip_prefix("btime ") {
            if let Ok(v) = rest.trim().parse::<u64>() {
                return Some(v);
            }
        }
    }
    None
}

/// 解析超长命令行并结构化提炼 Java/服务关键参数
pub fn extract_structured_cmdline(raw_cmd: &str, max_len: usize) -> StructuredCmdline {
    let normalized = raw_cmd.split_whitespace().collect::<Vec<_>>().join(" ");
    let cap = if max_len == 0 {
        DEFAULT_CMDLINE_MAX_LEN
    } else {
        max_len
    };
    let char_cnt = normalized.chars().count();
    let truncated = char_cnt > cap;
    let full_command = if truncated {
        let head: String = normalized.chars().take(cap).collect();
        format!("{head} ...[truncated:{char_cnt}]")
    } else {
        normalized.clone()
    };

    let tokens: Vec<&str> = normalized.split_whitespace().collect();
    let mut jar_path = None;
    let mut spring_config_location = None;
    let mut spring_profiles_active = None;
    let mut server_port = None;
    let mut config_path = None;

    let mut i = 0;
    while i < tokens.len() {
        let tok = tokens[i];
        let next = tokens.get(i + 1).copied();
        if tok == "-jar" {
            if let Some(v) = next {
                jar_path = Some(v.to_string());
                i += 2;
                continue;
            }
        } else if let Some(v) = tok.strip_prefix("--spring.config.location=") {
            spring_config_location = Some(v.to_string());
        } else if tok == "--spring.config.location" {
            if let Some(v) = next {
                spring_config_location = Some(v.to_string());
                i += 2;
                continue;
            }
        } else if let Some(v) = tok.strip_prefix("--spring.profiles.active=") {
            spring_profiles_active = Some(v.to_string());
        } else if let Some(v) = tok.strip_prefix("--server.port=") {
            server_port = v.parse::<u16>().ok();
        } else if tok == "--server.port" {
            if let Some(v) = next {
                server_port = v.parse::<u16>().ok();
                i += 2;
                continue;
            }
        } else if let Some(v) = tok.strip_prefix("--config=") {
            config_path = Some(v.to_string());
        } else if tok == "--config" || tok == "-c" {
            if let Some(v) = next {
                config_path = Some(v.to_string());
                i += 2;
                continue;
            }
        }
        i += 1;
    }

    StructuredCmdline {
        full_command,
        truncated,
        jar_path,
        spring_config_location,
        spring_profiles_active,
        server_port,
        config_path,
    }
}

/// 解析 `/proc/net/tcp` 或 `/proc/net/tcp6`（同时捕获 `0A` LISTEN 与 `01` ESTABLISHED）
pub fn parse_proc_net_tcp(content: &str, protocol: &str) -> Vec<TcpSocketEntry> {
    let mut entries = Vec::new();
    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }
        let state_label = match fields[3] {
            "0A" => "LISTEN",
            "01" => "ESTABLISHED",
            _ => continue,
        };
        let Some((local_ip, local_port)) = parse_hex_ip_port(fields[1], protocol) else {
            continue;
        };
        let Some((remote_ip, remote_port)) = parse_hex_ip_port(fields[2], protocol) else {
            continue;
        };
        let inode: u64 = fields[9].parse().unwrap_or(0);
        entries.push(TcpSocketEntry {
            protocol: protocol.to_string(),
            state: state_label.to_string(),
            local_ip,
            local_port,
            remote_ip,
            remote_port,
            inode,
        });
    }
    entries
}

fn parse_hex_ip_port(hex: &str, protocol: &str) -> Option<(String, u16)> {
    let (ip_hex, port_hex) = hex.split_once(':')?;
    let port = u16::from_str_radix(port_hex, 16).ok()?;
    if protocol == "tcp" {
        let ip_num = u32::from_str_radix(ip_hex, 16).ok()?;
        Some((Ipv4Addr::from(ip_num.to_be()).to_string(), port))
    } else if ip_hex.len() == 32 {
        let mut bytes = [0u8; 16];
        for (idx, chunk) in bytes.chunks_mut(4).enumerate() {
            let word = u32::from_str_radix(&ip_hex[idx * 8..idx * 8 + 8], 16).ok()?;
            chunk.copy_from_slice(&word.to_be_bytes());
        }
        Some((Ipv6Addr::from(bytes).to_string(), port))
    } else {
        Some(("::".to_string(), port))
    }
}

/// 解析 `/proc/net/unix` 监听套接字
pub fn parse_proc_net_unix(content: &str) -> Vec<UnixSocketEntry> {
    let mut out = Vec::new();
    for line in content.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 7 {
            continue;
        }
        let state_code = fields[5];
        if state_code != "01" && state_code != "03" {
            continue;
        }
        let inode: u64 = fields[6].parse().unwrap_or(0);
        let path = fields.get(7).copied().unwrap_or("").to_string();
        if !path.is_empty() {
            out.push(UnixSocketEntry {
                state: if state_code == "01" {
                    "LISTEN".to_string()
                } else {
                    "CONNECTED".to_string()
                },
                inode,
                path,
            });
        }
    }
    out
}

/// 解析 `/proc/self/status` 中的 `Uid:` 与 `CapEff:` 检测最小 Linux Capabilities
pub fn check_capabilities_from_status(status_text: &str) -> CapCheckResult {
    let mut is_root = false;
    let mut cap_eff_hex = None;
    for line in status_text.lines() {
        if let Some(rest) = line.strip_prefix("Uid:") {
            let euid = rest.split_whitespace().nth(1).unwrap_or("");
            if euid == "0" {
                is_root = true;
            }
        } else if let Some(rest) = line.strip_prefix("CapEff:") {
            cap_eff_hex = Some(rest.trim().to_string());
        }
    }
    let mask = cap_eff_hex
        .as_deref()
        .and_then(|h| u64::from_str_radix(h.trim_start_matches("0x"), 16).ok())
        .unwrap_or(0);
    let has_dac_read_search = is_root || (mask & (1u64 << 2)) != 0;
    let has_sys_ptrace = is_root || (mask & (1u64 << 19)) != 0;

    let degraded_reason = if !has_dac_read_search || !has_sys_ptrace {
        let mut missing = Vec::new();
        if !has_dac_read_search {
            missing.push("CAP_DAC_READ_SEARCH");
        }
        if !has_sys_ptrace {
            missing.push("CAP_SYS_PTRACE");
        }
        Some(format!(
            "非 root 运行且缺少 [{}]，跨用户 /proc/[pid]/fd 与 cwd 关联可能降级",
            missing.join(", ")
        ))
    } else {
        None
    };

    CapCheckResult {
        cap_eff_hex,
        has_dac_read_search,
        has_sys_ptrace,
        is_root,
        degraded_reason,
    }
}

/// 扫描可注入根目录下的 `<root>/proc` 全景（保留同端口多监听进程与 ESTABLISHED 连接）
pub fn scan_proc_root(root: &Path, max_cmd_len: usize) -> LinuxProcSnapshot {
    let proc_dir = root.join("proc");
    if !proc_dir.is_dir() {
        return LinuxProcSnapshot::default();
    }

    let btime = fs::read_to_string(proc_dir.join("stat"))
        .ok()
        .and_then(|s| parse_proc_stat_btime(&s))
        .unwrap_or(0);

    let mut sockets = Vec::new();
    if let Ok(v4) = fs::read_to_string(proc_dir.join("net/tcp")) {
        sockets.extend(parse_proc_net_tcp(&v4, "tcp"));
    }
    if let Ok(v6) = fs::read_to_string(proc_dir.join("net/tcp6")) {
        sockets.extend(parse_proc_net_tcp(&v6, "tcp6"));
    }
    let unix_sockets = fs::read_to_string(proc_dir.join("net/unix"))
        .map(|s| parse_proc_net_unix(&s))
        .unwrap_or_default();
    let cap_check = fs::read_to_string(proc_dir.join("self/status"))
        .ok()
        .map(|s| check_capabilities_from_status(&s));

    let mut inode_to_sockets: BTreeMap<u64, Vec<TcpSocketEntry>> = BTreeMap::new();
    for s in &sockets {
        inode_to_sockets.entry(s.inode).or_default().push(s.clone());
    }

    let mut processes = Vec::new();
    let Ok(entries) = fs::read_dir(&proc_dir) else {
        return LinuxProcSnapshot {
            processes,
            sockets,
            unix_sockets,
            cap_check,
        };
    };

    let mut pids: Vec<(u32, std::path::PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let fname = entry.file_name();
        if let Ok(pid) = fname.to_string_lossy().parse::<u32>() {
            pids.push((pid, entry.path()));
        }
    }
    pids.sort_by_key(|(pid, _)| *pid);

    for (pid, pdir) in pids {
        let comm = fs::read_to_string(pdir.join("comm"))
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let raw_cmd = fs::read(pdir.join("cmdline"))
            .map(|bytes| {
                bytes
                    .split(|&b| b == 0)
                    .filter(|p| !p.is_empty())
                    .map(|p| String::from_utf8_lossy(p).to_string())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        let stat_info = fs::read_to_string(pdir.join("stat"))
            .ok()
            .and_then(|s| parse_proc_stat_started_at(&s, btime, 100));
        let cwd = fs::read_link(pdir.join("cwd"))
            .ok()
            .map(|p| p.to_string_lossy().to_string())
            .or_else(|| {
                fs::read_to_string(pdir.join("cwd.txt"))
                    .ok()
                    .map(|s| s.trim().to_string())
            });

        let mut listening_sockets = Vec::new();
        let mut established_peers = Vec::new();
        let mut seen_sock_keys = BTreeSet::new();
        collect_pid_socket_inodes(&pdir, &inode_to_sockets, &mut |sock| {
            let key = (
                sock.protocol.clone(),
                sock.state.clone(),
                sock.local_ip.clone(),
                sock.local_port,
                sock.remote_ip.clone(),
                sock.remote_port,
            );
            if seen_sock_keys.insert(key) {
                if sock.state == "LISTEN" {
                    listening_sockets.push(sock.clone());
                } else if sock.state == "ESTABLISHED" {
                    established_peers.push(sock.clone());
                }
            }
        });

        let cmd_text = if raw_cmd.is_empty() {
            comm.clone()
        } else {
            raw_cmd
        };
        if cmd_text.trim().is_empty() && listening_sockets.is_empty() {
            continue;
        }
        processes.push(LinuxProcessFact {
            pid,
            ppid: stat_info.as_ref().map(|(ppid, _)| *ppid),
            comm,
            started_at: stat_info.map(|(_, ts)| ts),
            cwd,
            cmdline: extract_structured_cmdline(&cmd_text, max_cmd_len),
            listening_sockets,
            established_peers,
        });
    }

    LinuxProcSnapshot {
        processes,
        sockets,
        unix_sockets,
        cap_check,
    }
}

fn collect_pid_socket_inodes(
    pdir: &Path,
    inode_to_sockets: &BTreeMap<u64, Vec<TcpSocketEntry>>,
    on_sock: &mut dyn FnMut(&TcpSocketEntry),
) {
    let fd_dir = pdir.join("fd");
    if let Ok(fds) = fs::read_dir(&fd_dir) {
        for fd in fds.flatten() {
            let link_target = fs::read_link(fd.path())
                .ok()
                .map(|p| p.to_string_lossy().to_string())
                .or_else(|| fs::read_to_string(fd.path()).ok());
            if let Some(target) = link_target {
                let trimmed = target.trim();
                if let Some(inner) = trimmed
                    .strip_prefix("socket:[")
                    .and_then(|s| s.strip_suffix(']'))
                {
                    if let Ok(inode) = inner.parse::<u64>() {
                        if let Some(list) = inode_to_sockets.get(&inode) {
                            for s in list {
                                on_sock(s);
                            }
                        }
                    }
                }
            }
        }
    }
}
