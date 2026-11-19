use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, TcpListener};
use std::path::{Path, PathBuf};

/// 单个监听端口的客观事实实体
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListeningPortFact {
    /// 端口号
    pub port: u16,
    /// 协议 ("tcp" | "tcp6")
    pub protocol: String,
    /// 绑定 IP（如 "0.0.0.0", "127.0.0.1", "::"）
    pub bind_ip: String,
    /// 占用进程 PID
    pub pid: Option<u32>,
    /// 进程名称
    pub process_name: Option<String>,
    /// 完整启动命令行
    pub command: Option<String>,
    /// 进程工作目录 (Cwd)
    pub cwd: Option<String>,
    /// 进程所属用户
    pub user: Option<String>,
    /// 是否属于系统级底噪守护进程（如 sshd, systemd-resolved 等）
    pub is_system_daemon: bool,
    /// 套接字 inode
    pub inode: u64,
}

/// 端口查询与防撞诊断结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortQueryFactResult {
    pub port: u16,
    pub status: String, // "OCCUPIED" | "FREE"
    pub occupied_by: Option<ListeningPortFact>,
    pub recommended_free_ports: Vec<u16>,
    pub advice: String,
}

/// 运行时进程与网络拓扑事实引擎
pub struct ProcessNetTopology;

impl ProcessNetTopology {
    /// 扫描全系统所有处于 LISTEN 状态的网络端口及其物理进程
    /// (Phase 18：按运行时 OS 经 `PlatformProbe` 分发；Linux 仍走 `/proc` 微秒级实现)
    pub fn scan_listening_ports() -> Result<Vec<ListeningPortFact>> {
        crate::platform::current_probe().listening_ports()
    }

    /// Linux `/proc` 原生实现：扫描 TCP LISTEN 端口及进程上下文
    pub fn scan_listening_ports_procfs() -> Result<Vec<ListeningPortFact>> {
        let mut port_facts = Vec::new();

        // 1. 扫描 TCP IPv4
        if let Ok(v4_entries) = Self::parse_proc_net_tcp("/proc/net/tcp", "tcp") {
            port_facts.extend(v4_entries);
        }

        // 2. 扫描 TCP IPv6
        if let Ok(v6_entries) = Self::parse_proc_net_tcp("/proc/net/tcp6", "tcp6") {
            port_facts.extend(v6_entries);
        }

        // 3. 构建 socket inode -> pid 映射表
        let inode_to_pid = Self::build_inode_to_pid_map();

        // 4. 填充进程详细上下文与底噪判定
        for fact in &mut port_facts {
            if let Some(&pid) = inode_to_pid.get(&fact.inode) {
                fact.pid = Some(pid);
                if let Some(proc_info) = Self::resolve_proc_details(pid) {
                    fact.process_name = Some(proc_info.name.clone());
                    fact.command = Some(proc_info.command);
                    fact.cwd = proc_info.cwd;
                    fact.user = Some(proc_info.user);
                    fact.is_system_daemon = Self::is_system_daemon_process(&proc_info.name);
                }
            }
        }

        // 按端口号排序
        port_facts.sort_by_key(|p| p.port);
        // 去重 (相同端口和协议)
        port_facts.dedup_by(|a, b| a.port == b.port && a.protocol == b.protocol);

        Ok(port_facts)
    }

    /// 诊断指定端口是否已被占用，并返回客观进程事实与空闲推荐
    pub fn check_port(port: u16, suggest_count: usize) -> Result<PortQueryFactResult> {
        let all_ports = Self::scan_listening_ports()?;
        let occupied = all_ports.iter().find(|p| p.port == port).cloned();

        if let Some(fact) = occupied {
            let occupied_ports_set: Vec<u16> = all_ports.iter().map(|p| p.port).collect();
            let free_ports = Self::find_next_free_ports(port + 1, suggest_count, &occupied_ports_set);

            let advice = if let Some(ref proc_name) = fact.process_name {
                format!(
                    "⚠️ 端口 {} 已被进程 '{}' (PID: {}, 用户: {}) 占用，启动命令: {}，工作目录: {}。为避免服务冲突与踩踏，建议改用推荐空闲端口: {:?}",
                    port,
                    proc_name,
                    fact.pid.unwrap_or(0),
                    fact.user.as_deref().unwrap_or("未知"),
                    fact.command.as_deref().unwrap_or("未知"),
                    fact.cwd.as_deref().unwrap_or("未知"),
                    free_ports
                )
            } else {
                format!(
                    "⚠️ 端口 {} 当前处于 LISTEN 状态 (inode: {})。建议改用推荐空闲端口: {:?}",
                    port, fact.inode, free_ports
                )
            };

            Ok(PortQueryFactResult {
                port,
                status: "OCCUPIED".to_string(),
                occupied_by: Some(fact),
                recommended_free_ports: free_ports,
                advice,
            })
        } else {
            // 尝试物理 bind 测试是否确实可用
            let is_bindable = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port)).is_ok();
            if is_bindable {
                Ok(PortQueryFactResult {
                    port,
                    status: "FREE".to_string(),
                    occupied_by: None,
                    recommended_free_ports: vec![],
                    advice: format!("✅ 端口 {} 当前空闲未被占用，可安全绑定启动服务。", port),
                })
            } else {
                let occupied_ports_set: Vec<u16> = all_ports.iter().map(|p| p.port).collect();
                let free_ports = Self::find_next_free_ports(port + 1, suggest_count, &occupied_ports_set);
                Ok(PortQueryFactResult {
                    port,
                    status: "OCCUPIED".to_string(),
                    occupied_by: None,
                    recommended_free_ports: free_ports.clone(),
                    advice: format!("⚠️ 端口 {} 无法绑定（可能受权限限制或处于半关闭状态），建议改用推荐端口: {:?}", port, free_ports),
                })
            }
        }
    }

    /// 解析 /proc/net/tcp 或 /proc/net/tcp6
    fn parse_proc_net_tcp(file_path: &str, protocol: &str) -> Result<Vec<ListeningPortFact>> {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => return Ok(Vec::new()),
        };

        let mut entries = Vec::new();

        for line in content.lines().skip(1) {
            let fields: Vec<&str> = line.split_whitespace().collect();
            if fields.len() < 10 {
                continue;
            }

            // state "0A" 表示 TCP_LISTEN (sl=0, local_address=1, rem_address=2, st=3)
            let state = fields[3];
            if state != "0A" {
                continue;
            }

            let local_address = fields[1];
            let inode_str = fields[9];
            let inode: u64 = inode_str.parse().unwrap_or(0);

            if let Some((bind_ip, port)) = Self::parse_ip_port_hex(local_address, protocol) {
                entries.push(ListeningPortFact {
                    port,
                    protocol: protocol.to_string(),
                    bind_ip,
                    pid: None,
                    process_name: None,
                    command: None,
                    cwd: None,
                    user: None,
                    is_system_daemon: false,
                    inode,
                });
            }
        }

        Ok(entries)
    }

    /// 将 16 进制 IP:PORT 解析为常规字符串
    fn parse_ip_port_hex(hex_str: &str, protocol: &str) -> Option<(String, u16)> {
        let parts: Vec<&str> = hex_str.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let port = u16::from_str_radix(parts[1], 16).ok()?;

        if protocol == "tcp" {
            let ip_num = u32::from_str_radix(parts[0], 16).ok()?;
            // Linux /proc/net/tcp IP 为网络字节序存储在 host int 中
            let ip = Ipv4Addr::from(ip_num.to_be());
            Some((ip.to_string(), port))
        } else {
            // IPv6
            if parts[0].len() == 32 {
                let mut bytes = [0u8; 16];
                for i in 0..16 {
                    bytes[i] = u8::from_str_radix(&parts[0][i * 2..i * 2 + 2], 16).ok()?;
                }
                let ip = Ipv6Addr::from(bytes);
                Some((ip.to_string(), port))
            } else {
                Some(("::".to_string(), port))
            }
        }
    }

    /// 扫描 /proc/[pid]/fd 构建 socket inode 到 PID 的映射
    fn build_inode_to_pid_map() -> HashMap<u64, u32> {
        let mut map = HashMap::new();
        let proc_dir = Path::new("/proc");

        if let Ok(entries) = fs::read_dir(proc_dir) {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                let pid_str = file_name.to_string_lossy();
                if let Ok(pid) = pid_str.parse::<u32>() {
                    let fd_dir = entry.path().join("fd");
                    if let Ok(fd_entries) = fs::read_dir(fd_dir) {
                        for fd in fd_entries.flatten() {
                            if let Ok(link) = fs::read_link(fd.path()) {
                                let link_str = link.to_string_lossy();
                                if link_str.starts_with("socket:[") && link_str.ends_with(']') {
                                    let inode_str = &link_str[8..link_str.len() - 1];
                                    if let Ok(inode) = inode_str.parse::<u64>() {
                                        map.insert(inode, pid);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        map
    }

    /// 解析指定 PID 的详细信息（名称、命令、Cwd、所有者）
    fn resolve_proc_details(pid: u32) -> Option<ProcessDetailInfo> {
        let pid_dir = PathBuf::from(format!("/proc/{}", pid));
        if !pid_dir.exists() {
            return None;
        }

        // 1. 读取 comm
        let comm_path = pid_dir.join("comm");
        let name = fs::read_to_string(comm_path)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        // 2. 读取 cmdline (以 \0 分隔)
        let cmdline_path = pid_dir.join("cmdline");
        let command = fs::read(cmdline_path)
            .map(|bytes| {
                let s = bytes.split(|&b| b == 0)
                    .filter(|part| !part.is_empty())
                    .map(|part| String::from_utf8_lossy(part).to_string())
                    .collect::<Vec<String>>()
                    .join(" ");
                if s.is_empty() { name.clone() } else { s }
            })
            .unwrap_or_else(|_| name.clone());

        // 3. 读取 cwd 软链接
        let cwd_path = pid_dir.join("cwd");
        let cwd = fs::read_link(cwd_path)
            .ok()
            .map(|p| p.to_string_lossy().to_string());

        // 4. 解析所属用户 (通过 status 中的 Uid)
        let mut user = "unknown".to_string();
        let status_path = pid_dir.join("status");
        if let Ok(status_str) = fs::read_to_string(status_path) {
            for line in status_str.lines() {
                if line.starts_with("Uid:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() > 1 {
                        let uid = parts[1];
                        user = if uid == "0" { "root".to_string() } else { format!("uid-{}", uid) };
                    }
                    break;
                }
            }
        }

        Some(ProcessDetailInfo {
            name,
            command,
            cwd,
            user,
        })
    }

    /// 判断是否属于常见系统底噪进程
    fn is_system_daemon_process(name: &str) -> bool {
        let daemons = [
            "systemd", "sshd", "systemd-resolved", "chronyd", "dbus-daemon",
            "rsyslogd", "cupsd", "avahi-daemon", "rpcbind", "kernel",
        ];
        daemons.iter().any(|&d| name.contains(d))
    }

    /// 寻找下一个连续推荐的可用空闲端口
    fn find_next_free_ports(start_port: u16, count: usize, occupied_set: &[u16]) -> Vec<u16> {
        let mut free = Vec::new();
        let mut curr = start_port;

        while free.len() < count && curr < 65535 {
            if !occupied_set.contains(&curr) {
                // 尝试能否直接 bind
                if TcpListener::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), curr)).is_ok() {
                    free.push(curr);
                }
            }
            curr += 1;
        }

        free
    }

    /// 将端口事实列表格式化为面向 Agent 汇报的自然语言摘要
    pub fn format_listening_report(facts: &[ListeningPortFact], filter_system: bool) -> String {
        let mut out = String::new();
        out.push_str("📡 **宿主机监听端口与活跃服务拓扑图谱**\n\n");

        let filtered: Vec<&ListeningPortFact> = facts.iter()
            .filter(|f| !filter_system || !f.is_system_daemon)
            .collect();

        if filtered.is_empty() {
            out.push_str("当前宿主机无非系统监听端口。\n");
            return out;
        }

        out.push_str("| 端口 | 协议 | 绑定 IP | PID | 进程名 | 工作目录 (Cwd) | 启动命令 |\n");
        out.push_str("|:---:|:---:|:---:|:---:|:---:|:---|:---|\n");

        for f in filtered {
            let pid_str = f.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".to_string());
            let name_str = f.process_name.as_deref().unwrap_or("-");
            let cwd_str = f.cwd.as_deref().unwrap_or("-");
            let cmd_str = f.command.as_deref().unwrap_or("-");
            // 截断超长命令防撑爆表格
            let truncated_cmd = if cmd_str.chars().count() > 60 {
                let t: String = cmd_str.chars().take(57).collect();
                format!("{}...", t)
            } else {
                cmd_str.to_string()
            };

            out.push_str(&format!(
                "| `{}` | {} | `{}` | `{}` | `{}` | `{}` | `{}` |\n",
                f.port, f.protocol, f.bind_ip, pid_str, name_str, cwd_str, truncated_cmd
            ));
        }

        out
    }
}

struct ProcessDetailInfo {
    name: String,
    command: String,
    cwd: Option<String>,
    user: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_listening_ports() {
        let ports = ProcessNetTopology::scan_listening_ports();
        assert!(ports.is_ok(), "扫描监听端口应该成功返回");
        let list = ports.unwrap();
        // Linux 环境下一般至少有 SSH(22) 或其它监听端口
        println!("扫描到监听端口数量: {}", list.len());
        for p in list.iter().take(5) {
            println!("Port: {}/{} PID: {:?}", p.port, p.protocol, p.pid);
        }
    }

    #[test]
    fn test_check_port_free_and_conflict() {
        // 动态起一个临时 listener
        let listener = TcpListener::bind("127.0.0.1:0").expect("应该能成功绑定临时端口");
        let occupied_port = listener.local_addr().unwrap().port();

        // 检查该端口
        let res = ProcessNetTopology::check_port(occupied_port, 3).expect("检查被占用端口成功");
        assert_eq!(res.status, "OCCUPIED");
        assert!(!res.recommended_free_ports.is_empty());
        assert!(res.advice.contains(&occupied_port.to_string()));

        // 释放该端口
        drop(listener);

        // 再次检查应该为 FREE
        let res_free = ProcessNetTopology::check_port(occupied_port, 3).expect("检查空闲端口成功");
        assert_eq!(res_free.status, "FREE");
        assert!(res_free.advice.contains("空闲未被占用"));
    }

    #[test]
    fn test_format_listening_report() {
        let dummy = vec![
            ListeningPortFact {
                port: 8080,
                protocol: "tcp".to_string(),
                bind_ip: "0.0.0.0".to_string(),
                pid: Some(18493),
                process_name: Some("python3".to_string()),
                command: Some("uvicorn main:app --port 8080".to_string()),
                cwd: Some("/root/new/work/api-service".to_string()),
                user: Some("root".to_string()),
                is_system_daemon: false,
                inode: 123456,
            }
        ];

        let report = ProcessNetTopology::format_listening_report(&dummy, false);
        assert!(report.contains("8080"));
        assert!(report.contains("python3"));
        assert!(report.contains("/root/new/work/api-service"));
    }
}
