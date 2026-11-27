//! Java/SpringBoot/Tomcat/东方通命令行 & Nginx `include`/`upstream` 递归解析器

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JvmAppInfo {
    pub pid: u32,
    pub middleware_kind: String,
    pub heap_xms: Option<String>,
    pub heap_xmx: Option<String>,
    pub jar_or_main: Option<String>,
    pub spring_configs: Vec<String>,
    pub sys_props: BTreeMap<String, String>,
    pub listen_port: Option<u16>,
    pub home_dir: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NginxUpstreamInfo {
    pub upstream_name: String,
    pub backends: Vec<String>,
    pub source_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NginxProxyRoute {
    pub listen_ports: Vec<u16>,
    pub server_names: Vec<String>,
    pub location: String,
    pub proxy_pass: String,
    pub resolved_backends: Vec<String>,
    pub source_file: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NginxTopology {
    pub upstreams: Vec<NginxUpstreamInfo>,
    pub proxy_routes: Vec<NginxProxyRoute>,
    pub visited_files: Vec<String>,
}

/// 解析 JVM / Tomcat / 东方通命令行
pub fn parse_jvm_cmdline(pid: u32, cmdline: &str) -> Option<JvmAppInfo> {
    let tokens: Vec<&str> = cmdline.split_whitespace().collect();
    let first = tokens.first()?.to_ascii_lowercase();
    let looks_jvm = first.ends_with("java")
        || first.contains("/java")
        || cmdline.contains(" -jar ")
        || cmdline.contains("catalina.base")
        || cmdline.contains("tongweb");
    if !looks_jvm {
        return None;
    }

    let mut heap_xms = None;
    let mut heap_xmx = None;
    let mut jar_or_main = None;
    let mut spring_configs = Vec::new();
    let mut sys_props = BTreeMap::new();
    let mut listen_port = None;
    let mut home_dir = None;

    let mut i = 0;
    while i < tokens.len() {
        let tok = tokens[i];
        let next = tokens.get(i + 1).copied();
        if let Some(v) = tok.strip_prefix("-Xms") {
            heap_xms = Some(v.to_string());
        } else if let Some(v) = tok.strip_prefix("-Xmx") {
            heap_xmx = Some(v.to_string());
        } else if let Some(prop) = tok.strip_prefix("-D") {
            if let Some((k, v)) = prop.split_once('=') {
                sys_props.insert(k.to_string(), v.to_string());
                if k == "catalina.base" || k == "tongweb.home" {
                    home_dir = Some(v.to_string());
                } else if k == "server.port" {
                    listen_port = v.parse::<u16>().ok();
                }
            }
        } else if tok == "-jar" {
            if let Some(v) = next {
                jar_or_main = Some(v.to_string());
                i += 2;
                continue;
            }
        } else if let Some(v) = tok
            .strip_prefix("--spring.config.location=")
            .or_else(|| tok.strip_prefix("--spring.config.additional-location="))
        {
            spring_configs.push(v.to_string());
        } else if (tok == "--spring.config.location"
            || tok == "--spring.config.additional-location")
            && next.is_some()
        {
            if let Some(v) = next {
                spring_configs.push(v.to_string());
                i += 2;
                continue;
            }
        } else if let Some(v) = tok.strip_prefix("--server.port=") {
            listen_port = v.parse::<u16>().ok();
        } else if tok == "--server.port" {
            if let Some(v) = next {
                listen_port = v.parse::<u16>().ok();
                i += 2;
                continue;
            }
        }
        i += 1;
    }

    let middleware_kind = if cmdline.to_ascii_lowercase().contains("tongweb") {
        "tongweb".to_string()
    } else if sys_props.contains_key("catalina.base") {
        "tomcat".to_string()
    } else if !spring_configs.is_empty() || sys_props.keys().any(|k| k.starts_with("spring.")) {
        "spring_boot".to_string()
    } else {
        "jvm".to_string()
    };

    Some(JvmAppInfo {
        pid,
        middleware_kind,
        heap_xms,
        heap_xmx,
        jar_or_main,
        spring_configs,
        sys_props,
        listen_port,
        home_dir,
    })
}

/// 递归解析 Nginx 主配置及其 `include` 子配置文件，抽取 `upstream` 与 `proxy_pass` 路由
pub fn parse_nginx_hierarchy(conf_path: &Path, root: &Path) -> NginxTopology {
    let mut visited = BTreeSet::new();
    let mut upstreams_map: BTreeMap<String, NginxUpstreamInfo> = BTreeMap::new();
    let mut raw_routes: Vec<NginxProxyRoute> = Vec::new();

    walk_nginx_file(
        conf_path,
        root,
        &mut visited,
        &mut upstreams_map,
        &mut raw_routes,
    );

    for route in &mut raw_routes {
        let trimmed = route
            .proxy_pass
            .trim()
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        let host_part = trimmed.split('/').next().unwrap_or("");
        if let Some(up) = upstreams_map.get(host_part) {
            route.resolved_backends = up.backends.clone();
        } else if !host_part.is_empty() {
            route.resolved_backends = vec![host_part.to_string()];
        }
    }

    NginxTopology {
        upstreams: upstreams_map.into_values().collect(),
        proxy_routes: raw_routes,
        visited_files: visited.into_iter().collect(),
    }
}

fn walk_nginx_file(
    conf_path: &Path,
    root: &Path,
    visited: &mut BTreeSet<String>,
    upstreams: &mut BTreeMap<String, NginxUpstreamInfo>,
    routes: &mut Vec<NginxProxyRoute>,
) {
    let key = conf_path.to_string_lossy().to_string();
    if !visited.insert(key.clone()) || !conf_path.is_file() {
        return;
    }
    let Ok(txt) = fs::read_to_string(conf_path) else {
        return;
    };
    let base_dir = conf_path.parent().unwrap_or(root);

    let mut cur_upstream: Option<String> = None;
    let mut cur_backends: Vec<String> = Vec::new();
    let mut in_server = false;
    let mut server_brace_depth: i32 = 0;
    let mut server_ports: Vec<u16> = Vec::new();
    let mut server_names: Vec<String> = Vec::new();
    let mut cur_location: Option<String> = None;

    for raw_line in txt.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        let open_braces = line.matches('{').count() as i32;
        let close_braces = line.matches('}').count() as i32;

        if let Some(rest) = line.strip_prefix("include ") {
            let pat = rest.trim_end_matches(';').trim();
            for inc_path in resolve_include_pattern(base_dir, root, pat) {
                walk_nginx_file(&inc_path, root, visited, upstreams, routes);
            }
            continue;
        }

        if cur_upstream.is_none() && line.starts_with("upstream ") && line.contains('{') {
            let name = line
                .trim_start_matches("upstream ")
                .split('{')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !name.is_empty() {
                cur_upstream = Some(name);
                cur_backends.clear();
            }
            continue;
        }

        if let Some(ref up_name) = cur_upstream {
            if line.starts_with("server ") {
                let target = line
                    .trim_start_matches("server ")
                    .trim_end_matches(';')
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();
                if !target.is_empty() {
                    cur_backends.push(target);
                }
            }
            if close_braces > 0 {
                upstreams.insert(
                    up_name.clone(),
                    NginxUpstreamInfo {
                        upstream_name: up_name.clone(),
                        backends: cur_backends.clone(),
                        source_file: key.clone(),
                    },
                );
                cur_upstream = None;
                cur_backends.clear();
            }
            continue;
        }

        if !in_server && (line == "server {" || line.starts_with("server {")) {
            in_server = true;
            server_brace_depth = open_braces - close_braces;
            server_ports.clear();
            server_names.clear();
            cur_location = None;
            continue;
        }

        if in_server {
            server_brace_depth += open_braces - close_braces;
            if let Some(rest) = line.strip_prefix("listen ") {
                if let Some(tok) = rest.trim_end_matches(';').split_whitespace().next() {
                    let port_str = tok.rsplit(':').next().unwrap_or(tok);
                    if let Ok(p) = port_str.parse::<u16>() {
                        server_ports.push(p);
                    }
                }
            } else if let Some(rest) = line.strip_prefix("server_name ") {
                for tok in rest.trim_end_matches(';').split_whitespace() {
                    server_names.push(tok.to_string());
                }
            } else if line.starts_with("location ") && line.contains('{') {
                let loc = line
                    .trim_start_matches("location ")
                    .split('{')
                    .next()
                    .unwrap_or("/")
                    .trim()
                    .to_string();
                cur_location = Some(loc);
            } else if let Some(rest) = line.strip_prefix("proxy_pass ") {
                let target = rest.trim_end_matches(';').trim().to_string();
                routes.push(NginxProxyRoute {
                    listen_ports: server_ports.clone(),
                    server_names: server_names.clone(),
                    location: cur_location.clone().unwrap_or_else(|| "/".to_string()),
                    proxy_pass: target,
                    resolved_backends: Vec::new(),
                    source_file: key.clone(),
                });
            }

            if close_braces > 0 && cur_location.is_some() && server_brace_depth == 1 {
                cur_location = None;
            }
            if server_brace_depth <= 0 {
                in_server = false;
                server_brace_depth = 0;
            }
        }
    }
}

fn resolve_include_pattern(base_dir: &Path, root: &Path, pat: &str) -> Vec<PathBuf> {
    let candidate = if let Some(stripped) = pat.strip_prefix('/') {
        root.join(stripped)
    } else {
        base_dir.join(pat)
    };
    let s = candidate.to_string_lossy();
    if let Some((dir_part, file_glob)) = s.rsplit_once('/') {
        if file_glob.contains('*') {
            let dir = Path::new(dir_part);
            let suffix = file_glob.trim_start_matches('*');
            let mut out = Vec::new();
            if let Ok(entries) = fs::read_dir(dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.is_file() && p.file_name().unwrap_or_default().to_string_lossy().ends_with(suffix) {
                        out.push(p);
                    }
                }
            }
            out.sort();
            return out;
        }
    }
    vec![candidate]
}
