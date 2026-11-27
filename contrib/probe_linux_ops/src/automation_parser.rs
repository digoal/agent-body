//! Systemd 全路径 + Drop-in + 实时运行态 & RHEL/麒麟/openEuler/Debian Cron 纯函数解析

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemdRuntimeState {
    pub active_state: String,
    pub sub_state: String,
    pub main_pid: Option<u32>,
    pub exec_main_start_ts: Option<String>,
    pub result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemdUnitInfo {
    pub service_name: String,
    pub unit_file_path: String,
    pub drop_in_files: Vec<String>,
    pub description: String,
    pub exec_start: String,
    pub working_directory: Option<String>,
    pub user: Option<String>,
    pub environment_files: Vec<String>,
    pub referenced_files: Vec<String>,
    pub runtime_state: Option<SystemdRuntimeState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronEntryInfo {
    pub schedule: String,
    pub user: String,
    pub command: String,
    pub target_script: Option<String>,
    pub source_file: String,
    pub kind: String,
}

/// 扫描全路径 systemd 单元、`*.service.d/*.conf` drop-in 与可选实时运行态文件
pub fn scan_systemd_units(root: &Path) -> Vec<SystemdUnitInfo> {
    let unit_dirs = [
        root.join("etc/systemd/system"),
        root.join("run/systemd/system"),
        root.join("usr/lib/systemd/system"),
        root.join("lib/systemd/system"),
    ];
    let mut by_name: BTreeMap<String, SystemdUnitInfo> = BTreeMap::new();
    let abs_re = Regex::new(r"(/[a-zA-Z0-9_\.\-]+(?:/[a-zA-Z0-9_\.\-]+)+)").ok();

    for dir in &unit_dirs {
        if !dir.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if !p.is_file() {
                continue;
            }
            let fname = p.file_name().unwrap_or_default().to_string_lossy().to_string();
            if !fname.ends_with(".service") || fname.contains(".bak") || fname.starts_with('.') {
                continue;
            }
            if by_name.contains_key(&fname) {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&p) {
                let mut info = SystemdUnitInfo {
                    service_name: fname.clone(),
                    unit_file_path: p.to_string_lossy().to_string(),
                    drop_in_files: Vec::new(),
                    description: String::new(),
                    exec_start: String::new(),
                    working_directory: None,
                    user: None,
                    environment_files: Vec::new(),
                    referenced_files: Vec::new(),
                    runtime_state: None,
                };
                apply_unit_content(&content, &mut info, abs_re.as_ref());

                // 合并同名 `<unit>.d/*.conf` drop-in 覆盖
                for d_parent in &unit_dirs {
                    let drop_dir = d_parent.join(format!("{fname}.d"));
                    if drop_dir.is_dir() {
                        if let Ok(d_entries) = fs::read_dir(&drop_dir) {
                            let mut confs: Vec<_> = d_entries
                                .flatten()
                                .map(|e| e.path())
                                .filter(|cp| {
                                    cp.is_file()
                                        && cp.extension().and_then(|s| s.to_str()) == Some("conf")
                                })
                                .collect();
                            confs.sort();
                            for cp in confs {
                                if let Ok(d_txt) = fs::read_to_string(&cp) {
                                    info.drop_in_files.push(cp.to_string_lossy().to_string());
                                    apply_unit_content(&d_txt, &mut info, abs_re.as_ref());
                                }
                            }
                        }
                    }
                }

                // 读取可选运行时状态目录 `<root>/run/systemd/runtime_state/<service>.state`
                let state_path = root
                    .join("run/systemd/runtime_state")
                    .join(format!("{fname}.state"));
                if let Ok(st_txt) = fs::read_to_string(&state_path) {
                    info.runtime_state = Some(parse_systemd_state_text(&st_txt));
                }

                info.referenced_files.sort();
                info.referenced_files.dedup();
                by_name.insert(fname, info);
            }
        }
    }

    by_name.into_values().collect()
}

fn apply_unit_content(content: &str, info: &mut SystemdUnitInfo, abs_re: Option<&Regex>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        if let Some(v) = trimmed.strip_prefix("Description=") {
            info.description = v.trim().to_string();
        } else if let Some(v) = trimmed.strip_prefix("ExecStart=") {
            let val = v.trim();
            if !val.is_empty() {
                info.exec_start = val.to_string();
                if let Some(re) = abs_re {
                    for cap in re.captures_iter(val) {
                        if let Some(m) = cap.get(1) {
                            info.referenced_files.push(m.as_str().to_string());
                        }
                    }
                }
            }
        } else if let Some(v) = trimmed.strip_prefix("WorkingDirectory=") {
            let val = v.trim();
            if !val.is_empty() {
                info.working_directory = Some(val.to_string());
            }
        } else if let Some(v) = trimmed.strip_prefix("User=") {
            let val = v.trim();
            if !val.is_empty() {
                info.user = Some(val.to_string());
            }
        } else if let Some(v) = trimmed.strip_prefix("EnvironmentFile=") {
            let env_file = v.trim().trim_start_matches('-').to_string();
            if !env_file.is_empty() {
                info.environment_files.push(env_file.clone());
                info.referenced_files.push(env_file);
            }
        }
    }
}

/// 解析 `systemctl show` 风格键值状态文本
pub fn parse_systemd_state_text(txt: &str) -> SystemdRuntimeState {
    let mut state = SystemdRuntimeState::default();
    for line in txt.lines() {
        let Some((k, v)) = line.trim().split_once('=') else {
            continue;
        };
        let val = v.trim();
        match k.trim() {
            "ActiveState" => state.active_state = val.to_string(),
            "SubState" => state.sub_state = val.to_string(),
            "MainPID" => state.main_pid = val.parse::<u32>().ok().filter(|&p| p > 0),
            "ExecMainStartTimestamp" if !val.is_empty() => {
                state.exec_main_start_ts = Some(val.to_string());
            }
            "Result" if !val.is_empty() => {
                state.result = Some(val.to_string());
            }
            _ => {}
        }
    }
    state
}

/// 扫描 `/etc/crontab`, `/etc/cron.d/*`, Debian `crontabs/*`, RHEL/麒麟/openEuler `/var/spool/cron/*` 及 `*.timer`
pub fn scan_cron_all(root: &Path) -> Vec<CronEntryInfo> {
    let mut out = Vec::new();
    let env_assign_re = Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*\s*=").ok();
    let abs_re = Regex::new(r"(/[a-zA-Z0-9_\.\-]+(?:/[a-zA-Z0-9_\.\-]+)+)").ok();

    let etc_crontab = root.join("etc/crontab");
    if etc_crontab.is_file() {
        if let Ok(txt) = fs::read_to_string(&etc_crontab) {
            parse_cron_lines(
                &txt,
                &etc_crontab.to_string_lossy(),
                true,
                None,
                env_assign_re.as_ref(),
                abs_re.as_ref(),
                &mut out,
            );
        }
    }

    let cron_d = root.join("etc/cron.d");
    if cron_d.is_dir() {
        if let Ok(entries) = fs::read_dir(&cron_d) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && !p.file_name().unwrap_or_default().to_string_lossy().starts_with('.') {
                    if let Ok(txt) = fs::read_to_string(&p) {
                        parse_cron_lines(
                            &txt,
                            &p.to_string_lossy(),
                            true,
                            None,
                            env_assign_re.as_ref(),
                            abs_re.as_ref(),
                            &mut out,
                        );
                    }
                }
            }
        }
    }

    for spool_dir in [
        root.join("var/spool/cron/crontabs"),
        root.join("var/spool/cron"),
    ] {
        if !spool_dir.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&spool_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_file() {
                    continue;
                }
                let uname = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                if uname.starts_with('.') {
                    continue;
                }
                if let Ok(txt) = fs::read_to_string(&p) {
                    parse_cron_lines(
                        &txt,
                        &p.to_string_lossy(),
                        false,
                        Some(&uname),
                        env_assign_re.as_ref(),
                        abs_re.as_ref(),
                        &mut out,
                    );
                }
            }
        }
    }

    for timer_dir in [
        root.join("etc/systemd/system"),
        root.join("usr/lib/systemd/system"),
    ] {
        if !timer_dir.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&timer_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if !p.is_file() {
                    continue;
                }
                let fname = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                if !fname.ends_with(".timer") {
                    continue;
                }
                if let Ok(txt) = fs::read_to_string(&p) {
                    let mut sched = String::new();
                    let mut unit = fname.replace(".timer", ".service");
                    for line in txt.lines() {
                        let t = line.trim();
                        if let Some(v) = t.strip_prefix("OnCalendar=") {
                            sched = v.trim().to_string();
                        } else if let Some(v) = t.strip_prefix("Unit=") {
                            unit = v.trim().to_string();
                        }
                    }
                    if !sched.is_empty() {
                        out.push(CronEntryInfo {
                            schedule: sched,
                            user: "root".to_string(),
                            command: unit.clone(),
                            target_script: Some(unit),
                            source_file: p.to_string_lossy().to_string(),
                            kind: "systemd_timer".to_string(),
                        });
                    }
                }
            }
        }
    }

    out
}

fn parse_cron_lines(
    txt: &str,
    source_file: &str,
    has_user_col: bool,
    fixed_user: Option<&str>,
    env_assign_re: Option<&Regex>,
    abs_re: Option<&Regex>,
    out: &mut Vec<CronEntryInfo>,
) {
    for line in txt.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if env_assign_re.map(|re| re.is_match(trimmed)).unwrap_or(false) {
            continue;
        }
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let (schedule, user, cmd_slice) = if trimmed.starts_with('@') {
            if has_user_col && parts.len() >= 3 {
                (parts[0].to_string(), parts[1].to_string(), &parts[2..])
            } else if !has_user_col && parts.len() >= 2 {
                (
                    parts[0].to_string(),
                    fixed_user.unwrap_or("root").to_string(),
                    &parts[1..],
                )
            } else {
                continue;
            }
        } else if has_user_col && parts.len() >= 7 {
            (parts[0..5].join(" "), parts[5].to_string(), &parts[6..])
        } else if !has_user_col && parts.len() >= 6 {
            (
                parts[0..5].join(" "),
                fixed_user.unwrap_or("root").to_string(),
                &parts[5..],
            )
        } else {
            continue;
        };

        let command = cmd_slice.join(" ");
        let target_script = abs_re
            .and_then(|re| re.captures(&command))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string());
        out.push(CronEntryInfo {
            schedule,
            user,
            command,
            target_script,
            source_file: source_file.to_string(),
            kind: "cron".to_string(),
        });
    }
}
