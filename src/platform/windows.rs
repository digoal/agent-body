use std::collections::HashMap;
use std::process::Command;
use anyhow::{Context, Result};
use crate::fact::process_net::ListeningPortFact;
use super::{finalize_ports, make_port_fact, PlatformProbe, PlatformServiceFact};

/// Windows 探针：`netstat -ano` + `tasklist` + `schtasks`
///
/// 已知限制：`netstat` 的 `LISTENING` 关键字在非英文本地化系统中可能不同，
/// 此时返回空列表（不会捏造数据）。
pub struct WindowsProbe;

impl PlatformProbe for WindowsProbe {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn listening_ports(&self) -> Result<Vec<ListeningPortFact>> {
        let netstat = Command::new("netstat")
            .args(["-ano", "-p", "TCP"])
            .output()
            .context("执行 netstat 失败")?;
        let names = Command::new("tasklist")
            .args(["/FO", "CSV", "/NH"])
            .output()
            .map(|o| parse_tasklist_csv(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default();
        Ok(parse_netstat_ano(&String::from_utf8_lossy(&netstat.stdout), &names))
    }

    fn services(&self) -> Result<Vec<PlatformServiceFact>> {
        let out = Command::new("schtasks")
            .args(["/Query", "/FO", "CSV", "/NH"])
            .output()
            .context("执行 schtasks 失败")?;
        Ok(parse_schtasks_csv(&String::from_utf8_lossy(&out.stdout)))
    }
}

/// 解析 `netstat -ano` 输出，仅保留 LISTENING 行；`pid_names` 用于补全进程名
pub fn parse_netstat_ano(text: &str, pid_names: &HashMap<u32, String>) -> Vec<ListeningPortFact> {
    let mut ports = Vec::new();
    for line in text.lines() {
        let t: Vec<&str> = line.split_whitespace().collect();
        if t.len() < 5 || !t[0].eq_ignore_ascii_case("TCP") || !t[3].eq_ignore_ascii_case("LISTENING") {
            continue;
        }
        let (ip_raw, port_str) = match t[1].rsplit_once(':') {
            Some(v) => v,
            None => continue,
        };
        let port: u16 = match port_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };
        let pid: Option<u32> = t[4].parse().ok();
        let ip_clean = ip_raw.trim_matches(|c| c == '[' || c == ']');
        let is_v6 = ip_clean.contains(':');
        let name = pid.and_then(|p| pid_names.get(&p).cloned());
        ports.push(make_port_fact(
            port,
            if is_v6 { "tcp6" } else { "tcp" },
            ip_clean.to_string(),
            pid,
            name,
            None,
        ));
    }
    finalize_ports(ports)
}

/// 解析 `tasklist /FO CSV /NH` 输出为 PID -> 进程名映射
pub fn parse_tasklist_csv(text: &str) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    for line in text.lines() {
        let cols = split_csv_line(line);
        if cols.len() >= 2 {
            if let Ok(pid) = cols[1].parse::<u32>() {
                map.insert(pid, cols[0].clone());
            }
        }
    }
    map
}

/// 解析 `schtasks /Query /FO CSV /NH` 输出
pub fn parse_schtasks_csv(text: &str) -> Vec<PlatformServiceFact> {
    let mut res = Vec::new();
    for line in text.lines() {
        let cols = split_csv_line(line);
        if cols.len() >= 3 && !cols[0].is_empty() && cols[0].starts_with('\\') {
            res.push(PlatformServiceFact {
                name: cols[0].clone(),
                command: format!("状态: {} / 下次运行: {}", cols[2], cols[1]),
                kind: "schtasks".to_string(),
                source: "schtasks /Query /FO CSV /NH".to_string(),
            });
        }
    }
    res.sort_by(|a, b| a.name.cmp(&b.name));
    res.dedup_by(|a, b| a.name == b.name);
    res
}

/// 轻量 CSV 行拆分（仅支持双引号包裹字段，满足 tasklist/schtasks 输出）
fn split_csv_line(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('"') {
        return Vec::new();
    }
    trimmed
        .trim_matches('"')
        .split("\",\"")
        .map(|s| s.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const NETSTAT_FIXTURE: &str = "
Active Connections

  Proto  Local Address          Foreign Address        State           PID
  TCP    0.0.0.0:135            0.0.0.0:0              LISTENING       1048
  TCP    0.0.0.0:8080           0.0.0.0:0              LISTENING       9120
  TCP    127.0.0.1:5432         0.0.0.0:0              LISTENING       4412
  TCP    192.168.1.5:50234      142.250.1.1:443        ESTABLISHED     7788
  TCP    [::]:445               [::]:0                 LISTENING       4
";

    const TASKLIST_FIXTURE: &str = "\"svchost.exe\",\"1048\",\"Services\",\"0\",\"12,345 K\"\n\"node.exe\",\"9120\",\"Console\",\"1\",\"80,000 K\"\n\"postgres.exe\",\"4412\",\"Services\",\"0\",\"20,000 K\"\n\"System\",\"4\",\"Services\",\"0\",\"140 K\"\n";

    #[test]
    fn test_parse_netstat_with_tasklist_names() {
        let names = parse_tasklist_csv(TASKLIST_FIXTURE);
        assert_eq!(names.get(&9120).map(String::as_str), Some("node.exe"));

        let ports = parse_netstat_ano(NETSTAT_FIXTURE, &names);
        // 135 / 445(v6) / 5432 / 8080，ESTABLISHED 行被过滤
        assert_eq!(ports.len(), 4);
        assert!(ports.iter().all(|p| p.port != 50234));

        let web = ports.iter().find(|p| p.port == 8080).expect("8080");
        assert_eq!(web.pid, Some(9120));
        assert_eq!(web.process_name.as_deref(), Some("node.exe"));
        assert!(!web.is_system_daemon);

        let smb = ports.iter().find(|p| p.port == 445).expect("445");
        assert_eq!(smb.protocol, "tcp6");
        assert_eq!(smb.bind_ip, "::");
        assert!(smb.is_system_daemon, "System(PID 4) 属于系统底噪");

        let rpc = ports.iter().find(|p| p.port == 135).expect("135");
        assert!(rpc.is_system_daemon, "svchost 属于系统底噪");
    }

    #[test]
    fn test_parse_schtasks_csv() {
        let csv = "\"\\Microsoft\\Windows\\Defrag\\ScheduledDefrag\",\"11/20/2026 1:00:00 AM\",\"Ready\"\n\"\\MyBackup\",\"N/A\",\"Disabled\"\n\"INFO: some header\"\n";
        let tasks = parse_schtasks_csv(csv);
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].name, "\\Microsoft\\Windows\\Defrag\\ScheduledDefrag");
        assert!(tasks[1].command.contains("Disabled"));
        assert_eq!(tasks[0].kind, "schtasks");
    }

    #[test]
    fn test_parse_netstat_empty_on_localized_output() {
        // 非英文本地化时不会误报/捏造
        let localized = "  TCP    0.0.0.0:135    0.0.0.0:0    侦听    1048\n";
        assert!(parse_netstat_ano(localized, &HashMap::new()).is_empty());
    }
}
