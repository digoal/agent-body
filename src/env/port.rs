use std::collections::HashMap;
use std::fs;
use std::net::{TcpListener, SocketAddrV4, Ipv4Addr};
use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessConflict {
    pub pid: u32,
    pub name: String,
    pub command: String,
    pub owner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortCheckResult {
    pub port: u16,
    pub status: String, // "OCCUPIED" | "FREE"
    pub conflict: Option<ProcessConflict>,
    pub recommended_free_ports: Vec<u16>,
    pub advice: String,
}

pub struct PortChecker;

impl PortChecker {
    /// 检查指定端口是否已被占用，并查询占用进程与推荐可用端口
    pub fn check_port(port: u16, suggest_count: usize) -> Result<PortCheckResult> {
        let occupied_ports = Self::get_listening_ports_map()?;

        if let Some((pid, inode)) = occupied_ports.get(&port) {
            // 端口被占用，解析 pid 信息
            let conflict_info = Self::resolve_process_info(*pid);
            let free_ports = Self::find_next_free_ports(port + 1, suggest_count, &occupied_ports);

            let advice = if let Some(ref p) = conflict_info {
                format!(
                    "⚠️ 端口 {} 已被进程 '{}' (PID: {}, 用户: {}) 占用，命令行: {}。为避免服务冲突与踩踏，建议改用推荐空闲端口: {:?}",
                    port, p.name, p.pid, p.owner, p.command, free_ports
                )
            } else {
                format!(
                    "⚠️ 端口 {} 当前处于 LISTEN 状态 (inode: {})。建议改用推荐空闲端口: {:?}",
                    port, inode, free_ports
                )
            };

            Ok(PortCheckResult {
                port,
                status: "OCCUPIED".to_string(),
                conflict: conflict_info,
                recommended_free_ports: free_ports,
                advice,
            })
        } else {
            // 端口当前未见 LISTEN，尝试直接 bind 探测是否可直接使用
            let is_bindable = TcpListener::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port)).is_ok();
            if is_bindable {
                Ok(PortCheckResult {
                    port,
                    status: "FREE".to_string(),
                    conflict: None,
                    recommended_free_ports: vec![],
                    advice: format!("✅ 端口 {} 处于空闲可用状态，可以安全绑定启动服务。", port),
                })
            } else {
                let free_ports = Self::find_next_free_ports(port + 1, suggest_count, &occupied_ports);
                Ok(PortCheckResult {
                    port,
                    status: "OCCUPIED".to_string(),
                    conflict: None,
                    recommended_free_ports: free_ports.clone(),
                    advice: format!("⚠️ 端口 {} 无法直接绑定 (可能处于 TIME_WAIT 或特殊权限受限)。建议改用推荐空闲端口: {:?}", port, free_ports),
                })
            }
        }
    }

    /// 从起始端口开始，寻找接下来的 n 个连续可用空闲端口
    pub fn find_next_free_ports(start_port: u16, count: usize, occupied: &HashMap<u16, (u32, u64)>) -> Vec<u16> {
        let mut result = Vec::new();
        let mut curr = start_port;

        while result.len() < count && curr < 65535 {
            if !occupied.contains_key(&curr) {
                // 尝试绑定测试
                if TcpListener::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), curr)).is_ok() {
                    result.push(curr);
                }
            }
            curr += 1;
        }

        result
    }

    /// 解析 Linux /proc/net/tcp 和 /proc/net/tcp6 获取当前所有处于 LISTEN 状态的端口及 inode
    fn get_listening_ports_map() -> Result<HashMap<u16, (u32, u64)>> {
        let mut inode_to_port: HashMap<u64, u16> = HashMap::new();
        let mut port_map: HashMap<u16, (u32, u64)> = HashMap::new();

        for path in &["/proc/net/tcp", "/proc/net/tcp6"] {
            if let Ok(content) = fs::read_to_string(path) {
                for line in content.lines().skip(1) {
                    let fields: Vec<&str> = line.split_whitespace().collect();
                    if fields.len() >= 10 {
                        let local_addr = fields[1];
                        let state = fields[3];

                        // 0A 表示 TCP_LISTEN 状态
                        if state == "0A" {
                            if let Some(port_hex) = local_addr.split(':').nth(1) {
                                if let Ok(port) = u16::from_str_radix(port_hex, 16) {
                                    if let Ok(inode) = fields[9].parse::<u64>() {
                                        inode_to_port.insert(inode, port);
                                        port_map.insert(port, (0, inode));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 遍历 /proc/[pid]/fd 建立 inode -> pid 的映射
        if let Ok(entries) = fs::read_dir("/proc") {
            for entry in entries.flatten() {
                let file_name = entry.file_name();
                if let Some(name_str) = file_name.to_str() {
                    if let Ok(pid) = name_str.parse::<u32>() {
                        let fd_dir = entry.path().join("fd");
                        if let Ok(fds) = fs::read_dir(fd_dir) {
                            for fd in fds.flatten() {
                                if let Ok(target) = fs::read_link(fd.path()) {
                                    let target_str = target.to_string_lossy();
                                    // 格式形如 "socket:[12345]"
                                    if target_str.starts_with("socket:[") && target_str.ends_with(']') {
                                        let inode_part = &target_str[8..target_str.len() - 1];
                                        if let Ok(inode) = inode_part.parse::<u64>() {
                                            if let Some(&port) = inode_to_port.get(&inode) {
                                                port_map.insert(port, (pid, inode));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(port_map)
    }

    /// 解析指定 pid 的名称、命令行与启动者
    fn resolve_process_info(pid: u32) -> Option<ProcessConflict> {
        if pid == 0 {
            return None;
        }

        let cmdline_path = format!("/proc/{}/cmdline", pid);
        let comm_path = format!("/proc/{}/comm", pid);
        let status_path = format!("/proc/{}/status", pid);

        let name = fs::read_to_string(comm_path)
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|_| "unknown".to_string());

        let command = fs::read_to_string(cmdline_path)
            .map(|s| s.replace('\0', " ").trim().to_string())
            .unwrap_or_else(|_| name.clone());

        // 从 /proc/[pid]/status 解析 UID
        let mut owner = "system".to_string();
        if let Ok(status) = fs::read_to_string(status_path) {
            for line in status.lines() {
                if line.starts_with("Uid:") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        if parts[1] == "0" {
                            owner = "root".to_string();
                        } else {
                            owner = format!("uid:{}", parts[1]);
                        }
                    }
                    break;
                }
            }
        }

        Some(ProcessConflict {
            pid,
            name: name.clone(),
            command: if command.is_empty() { name } else { command },
            owner,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_port_free_and_occupied() {
        // 绑定一个动态临时端口
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let local_port = listener.local_addr().unwrap().port();

        // 探测该端口，应该报 OCCUPIED
        let res = PortChecker::check_port(local_port, 3).expect("check_port");
        assert_eq!(res.status, "OCCUPIED");
        assert!(!res.recommended_free_ports.is_empty());

        // 释放该端口并稍候给内核释放 socket 时间
        drop(listener);
        std::thread::sleep(std::time::Duration::from_millis(20));

        // 再次探测，应该为 FREE
        let res_free = PortChecker::check_port(local_port, 3).expect("check_port free");
        assert_eq!(res_free.status, "FREE");
    }
}
