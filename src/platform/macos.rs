use std::path::PathBuf;
use std::process::Command;
use anyhow::{Context, Result};
use crate::fact::process_net::ListeningPortFact;
use super::{finalize_ports, make_port_fact, PlatformProbe, PlatformServiceFact};

/// macOS 探针：`lsof` 枚举监听端口 + launchd plist 解析
pub struct MacosProbe;

impl PlatformProbe for MacosProbe {
    fn name(&self) -> &'static str {
        "macos"
    }

    fn listening_ports(&self) -> Result<Vec<ListeningPortFact>> {
        let out = Command::new("lsof")
            .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-F", "pcLn"])
            .output()
            .context("执行 lsof 失败 (macOS 需系统自带 lsof)")?;
        Ok(parse_lsof_f_output(&String::from_utf8_lossy(&out.stdout)))
    }

    fn services(&self) -> Result<Vec<PlatformServiceFact>> {
        let home = std::env::var("HOME").unwrap_or_default();
        let dirs = [
            PathBuf::from(&home).join("Library/LaunchAgents"),
            PathBuf::from("/Library/LaunchAgents"),
            PathBuf::from("/Library/LaunchDaemons"),
        ];
        let mut res = Vec::new();
        for dir in dirs {
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let p = entry.path();
                if p.extension().and_then(|e| e.to_str()) != Some("plist") {
                    continue;
                }
                if let Ok(text) = std::fs::read_to_string(&p) {
                    if let Some((label, cmd)) = parse_plist_label_and_command(&text) {
                        res.push(PlatformServiceFact {
                            name: label,
                            command: cmd,
                            kind: "launchd".to_string(),
                            source: p.to_string_lossy().to_string(),
                        });
                    }
                }
            }
        }
        res.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(res)
    }
}

/// 解析 `lsof -F pcLn` 机器可读输出
/// 行前缀: p=PID, c=命令名, L=登录用户, n=网络名 (如 `*:8080` / `127.0.0.1:631` / `[::1]:8080`)
pub fn parse_lsof_f_output(text: &str) -> Vec<ListeningPortFact> {
    let mut ports = Vec::new();
    let mut pid: Option<u32> = None;
    let mut cmd: Option<String> = None;
    let mut user: Option<String> = None;

    for line in text.lines() {
        let (tag, val) = match line.chars().next() {
            Some(c) => (c, &line[c.len_utf8()..]),
            None => continue,
        };
        match tag {
            'p' => {
                pid = val.trim().parse().ok();
                cmd = None;
                user = None;
            }
            'c' => cmd = Some(val.trim().to_string()),
            'L' => user = Some(val.trim().to_string()),
            'n' => {
                if let Some((ip_raw, port_str)) = val.rsplit_once(':') {
                    if let Ok(port) = port_str.trim().parse::<u16>() {
                        let ip_clean = ip_raw.trim_matches(|c| c == '[' || c == ']');
                        let is_v6 = ip_clean.contains(':');
                        let bind_ip = if ip_clean == "*" { "0.0.0.0".to_string() } else { ip_clean.to_string() };
                        ports.push(make_port_fact(
                            port,
                            if is_v6 { "tcp6" } else { "tcp" },
                            bind_ip,
                            pid,
                            cmd.clone(),
                            user.clone(),
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    finalize_ports(ports)
}

/// 从 launchd plist 文本中提取 Label 与启动命令 (ProgramArguments / Program)
pub fn parse_plist_label_and_command(text: &str) -> Option<(String, String)> {
    let label = string_after_key(text, "Label")?;

    let command = if let Some(idx) = text.find("<key>ProgramArguments</key>") {
        let rest = &text[idx..];
        let end = rest.find("</array>").unwrap_or(rest.len());
        let block = &rest[..end];
        let mut args = Vec::new();
        let mut cursor = block;
        while let Some(s) = cursor.find("<string>") {
            let after = &cursor[s + 8..];
            if let Some(e) = after.find("</string>") {
                args.push(after[..e].trim().to_string());
                cursor = &after[e + 9..];
            } else {
                break;
            }
        }
        args.join(" ")
    } else {
        string_after_key(text, "Program").unwrap_or_default()
    };

    Some((label, command))
}

fn string_after_key(text: &str, key: &str) -> Option<String> {
    let marker = format!("<key>{}</key>", key);
    let idx = text.find(&marker)?;
    let rest = &text[idx + marker.len()..];
    let s = rest.find("<string>")?;
    let after = &rest[s + 8..];
    let e = after.find("</string>")?;
    Some(after[..e].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LSOF_FIXTURE: &str = "p412\ncControlCenter\nLdigoal\nf8u\nn*:7000\nf9u\nn[::]:7000\np5120\ncpostgres\nLdigoal\nf6u\nn127.0.0.1:5432\np7788\ncnode\nLdigoal\nf20u\nn[::1]:3000\n";

    #[test]
    fn test_parse_lsof_f_output_fixture() {
        let ports = parse_lsof_f_output(LSOF_FIXTURE);
        // 7000 (tcp / tcp6 各一条)、5432、3000
        assert_eq!(ports.len(), 4);
        let pg = ports.iter().find(|p| p.port == 5432).expect("pg");
        assert_eq!(pg.pid, Some(5120));
        assert_eq!(pg.process_name.as_deref(), Some("postgres"));
        assert_eq!(pg.bind_ip, "127.0.0.1");
        assert_eq!(pg.protocol, "tcp");
        assert_eq!(pg.user.as_deref(), Some("digoal"));
        assert!(!pg.is_system_daemon);

        let node = ports.iter().find(|p| p.port == 3000).expect("node");
        assert_eq!(node.protocol, "tcp6");
        assert_eq!(node.bind_ip, "::1");

        let cc = ports.iter().find(|p| p.port == 7000 && p.protocol == "tcp").expect("cc");
        assert_eq!(cc.bind_ip, "0.0.0.0");
        assert!(cc.is_system_daemon, "ControlCenter 属于 macOS 系统底噪");
    }

    #[test]
    fn test_parse_plist_label_and_command() {
        let plist = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.digoal.steward</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/agent-body</string>
        <string>daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>"#;
        let (label, cmd) = parse_plist_label_and_command(plist).expect("parse");
        assert_eq!(label, "com.digoal.steward");
        assert_eq!(cmd, "/usr/local/bin/agent-body daemon");
    }

    #[test]
    fn test_parse_plist_program_fallback_and_missing_label() {
        let plist = "<dict><key>Label</key><string>x.y</string><key>Program</key><string>/bin/foo</string></dict>";
        let (l, c) = parse_plist_label_and_command(plist).expect("parse");
        assert_eq!((l.as_str(), c.as_str()), ("x.y", "/bin/foo"));
        assert!(parse_plist_label_and_command("<dict></dict>").is_none());
    }
}
