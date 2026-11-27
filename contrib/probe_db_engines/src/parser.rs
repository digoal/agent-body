//! 跨平台纯函数数据库配置解析器：PostgreSQL / 达梦 (Dameng) / 人大金仓 (KingbaseES)

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use agent_body_sdk::{types::rfc3339_beijing_now, EvidenceTuple, RowFilterPredicate};

/// 支持探测的数据库引擎种类
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbEngineKind {
    Postgresql,
    Dameng,
    Kingbase,
}

impl DbEngineKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Postgresql => "postgresql",
            Self::Dameng => "dameng",
            Self::Kingbase => "kingbase",
        }
    }
}

/// 单数据库实例配置提取摘要
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbInstanceConfig {
    pub engine: DbEngineKind,
    pub config_path: PathBuf,
    pub port: Option<u16>,
    pub data_dir: Option<String>,
    pub memory_params: BTreeMap<String, String>,
    pub wal_params: BTreeMap<String, String>,
    pub referenced_files: Vec<String>,
    pub hba_rules: Vec<String>,
    pub all_params: BTreeMap<String, String>,
}

/// 扫描可注入根目录列表 `&[PathBuf]`，发现并解析 PG / 达梦 / 人大金仓配置文件
pub fn scan_db_configs_in_roots(roots: &[PathBuf]) -> anyhow::Result<Vec<DbInstanceConfig>> {
    let mut discovered = Vec::new();
    let mut visited_dirs = BTreeSet::new();

    for root in roots {
        if !root.exists() {
            continue;
        }
        collect_config_files(root, 0, &mut visited_dirs, &mut discovered)?;
    }
    discovered.sort();

    let mut instances = Vec::new();
    for cfg_path in discovered {
        let Some(fname) = cfg_path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        let parsed = match fname {
            "postgresql.conf" => parse_pg_like_instance(DbEngineKind::Postgresql, &cfg_path, "pg_hba.conf")?,
            "kingbase.conf" => parse_pg_like_instance(DbEngineKind::Kingbase, &cfg_path, "sys_hba.conf")?,
            "dm.ini" => parse_dameng_instance(&cfg_path)?,
            _ => continue,
        };
        instances.push(parsed);
    }
    Ok(instances)
}

/// 将扫描得到的数据库实例配置封装为带 SHA-256 指纹的 `EvidenceTuple`
pub fn probe_roots_to_tuples(
    roots: &[PathBuf],
    host_id: &str,
    rls: Option<&RowFilterPredicate>,
) -> anyhow::Result<Vec<EvidenceTuple>> {
    let instances = scan_db_configs_in_roots(roots)?;
    let collected_at = rfc3339_beijing_now();
    let mut out = Vec::with_capacity(instances.len());

    for (idx, inst) in instances.into_iter().enumerate() {
        let cfg_str = inst.config_path.to_string_lossy().to_string();
        let entity_id = format!("db:{}:{cfg_str}", inst.engine.as_str());
        let mut labels = BTreeMap::new();
        labels.insert("engine".to_string(), inst.engine.as_str().to_string());
        if let Some(p) = inst.port {
            labels.insert("port".to_string(), p.to_string());
        }

        let payload = serde_json::json!({
            "engine": inst.engine.as_str(),
            "config_path": cfg_str,
            "port": inst.port,
            "data_dir": inst.data_dir,
            "memory_params": inst.memory_params,
            "wal_params": inst.wal_params,
            "referenced_files": inst.referenced_files,
            "hba_rules": inst.hba_rules,
            "params": inst.all_params,
        });

        let tuple = EvidenceTuple {
            evidence_id: format!("ev-db-{}-{idx}", inst.engine.as_str()),
            host_id: host_id.to_string(),
            host_labels: labels,
            entity_type: "entity:db_instance".to_string(),
            entity_id,
            source_ext: "probe_db_engines".to_string(),
            evidence_tier: "L1_EXPLICIT".to_string(),
            collected_at: collected_at.clone(),
            ttl_sec: 60,
            payload,
            redacted: false,
            evidence_sha256: String::new(),
        }
        .with_computed_sha256();

        if rls.map(|pred| pred.matches_tuple(&tuple)).unwrap_or(true) {
            out.push(tuple);
        }
    }
    Ok(out)
}

fn collect_config_files(
    dir: &Path,
    depth: usize,
    visited_dirs: &mut BTreeSet<PathBuf>,
    out: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    if depth > 8 {
        return Ok(());
    }
    let canon = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if !visited_dirs.insert(canon) {
        return Ok(());
    }
    if dir.is_file() {
        if is_target_config_file(dir) {
            out.push(dir.to_path_buf());
        }
        return Ok(());
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(());
    };
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
        } else if path.is_file() && is_target_config_file(&path) {
            out.push(path);
        }
    }
    subdirs.sort();
    for sub in subdirs {
        collect_config_files(&sub, depth + 1, visited_dirs, out)?;
    }
    Ok(())
}

fn is_target_config_file(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|s| s.to_str()),
        Some("postgresql.conf" | "kingbase.conf" | "dm.ini")
    )
}

/// 解析 PG / KingbaseES 风格配置文件（支持 `include` / `include_if_exists` / `include_dir` 及 HBA 文件）
fn parse_pg_like_instance(
    engine: DbEngineKind,
    cfg_path: &Path,
    hba_filename: &str,
) -> anyhow::Result<DbInstanceConfig> {
    let mut all_params = BTreeMap::new();
    let mut referenced = BTreeSet::new();
    let mut visited_files = BTreeSet::new();

    parse_pg_conf_recursive(cfg_path, &mut visited_files, &mut referenced, &mut all_params)?;

    let base_dir = cfg_path.parent().unwrap_or_else(|| Path::new("."));
    let mut hba_rules = Vec::new();
    let hba_path = all_params
        .get("hba_file")
        .map(|raw| resolve_rel_path(base_dir, raw))
        .unwrap_or_else(|| base_dir.join(hba_filename));

    if hba_path.is_file() {
        referenced.insert(hba_path.to_string_lossy().to_string());
        hba_rules = parse_hba_file(&hba_path)?;
    }

    let port = all_params
        .get("port")
        .and_then(|v| v.parse::<u16>().ok());
    let data_dir = all_params.get("data_directory").cloned().or_else(|| {
        base_dir
            .join("PG_VERSION")
            .is_file()
            .then(|| base_dir.to_string_lossy().to_string())
    });

    let memory_keys = [
        "shared_buffers",
        "work_mem",
        "maintenance_work_mem",
        "effective_cache_size",
        "temp_buffers",
        "wal_buffers",
        "huge_pages",
    ];
    let wal_keys = [
        "wal_level",
        "max_wal_size",
        "min_wal_size",
        "wal_compression",
        "archive_mode",
        "archive_command",
        "synchronous_commit",
        "checkpoint_timeout",
    ];

    let memory_params = pick_keys(&all_params, &memory_keys);
    let wal_params = pick_keys(&all_params, &wal_keys);

    Ok(DbInstanceConfig {
        engine,
        config_path: cfg_path.to_path_buf(),
        port,
        data_dir,
        memory_params,
        wal_params,
        referenced_files: referenced.into_iter().collect(),
        hba_rules,
        all_params,
    })
}

fn parse_pg_conf_recursive(
    file_path: &Path,
    visited: &mut BTreeSet<PathBuf>,
    referenced: &mut BTreeSet<String>,
    params: &mut BTreeMap<String, String>,
) -> anyhow::Result<()> {
    let canon = fs::canonicalize(file_path).unwrap_or_else(|_| file_path.to_path_buf());
    if !visited.insert(canon) {
        return Ok(());
    }
    if !file_path.is_file() {
        return Ok(());
    }
    referenced.insert(file_path.to_string_lossy().to_string());

    let content = fs::read_to_string(file_path)?;
    let parent = file_path.parent().unwrap_or_else(|| Path::new("."));

    for raw_line in content.lines() {
        let line = strip_hash_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, val)) = parse_kv_line(line) else {
            continue;
        };
        let key_lower = key.to_ascii_lowercase();
        match key_lower.as_str() {
            "include" | "include_if_exists" => {
                let inc_path = resolve_rel_path(parent, &val);
                if inc_path.is_file() {
                    parse_pg_conf_recursive(&inc_path, visited, referenced, params)?;
                }
            }
            "include_dir" => {
                let inc_dir = resolve_rel_path(parent, &val);
                if inc_dir.is_dir() {
                    if let Ok(entries) = fs::read_dir(&inc_dir) {
                        let mut conf_files: Vec<PathBuf> = entries
                            .flatten()
                            .map(|e| e.path())
                            .filter(|p| {
                                p.is_file()
                                    && p.extension().and_then(|s| s.to_str()) == Some("conf")
                            })
                            .collect();
                        conf_files.sort();
                        for cf in conf_files {
                            parse_pg_conf_recursive(&cf, visited, referenced, params)?;
                        }
                    }
                }
            }
            _ => {
                params.insert(key_lower, val);
            }
        }
    }
    Ok(())
}

/// 解析达梦 `dm.ini` 配置文件
fn parse_dameng_instance(cfg_path: &Path) -> anyhow::Result<DbInstanceConfig> {
    let content = fs::read_to_string(cfg_path)?;
    let mut all_params = BTreeMap::new();
    let mut referenced = vec![cfg_path.to_string_lossy().to_string()];

    for raw_line in content.lines() {
        let line = strip_hash_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some((key, val)) = parse_kv_line(line) {
            all_params.insert(key.to_ascii_uppercase(), val);
        }
    }

    let port = all_params
        .get("PORT_NUM")
        .and_then(|v| v.parse::<u16>().ok());
    let ctl_path = all_params.get("CTL_PATH").cloned();
    if let Some(ref ctl) = ctl_path {
        referenced.push(ctl.clone());
    }
    let data_dir = all_params
        .get("SYSTEM_PATH")
        .cloned()
        .or_else(|| {
            ctl_path
                .as_deref()
                .and_then(|p| Path::new(p).parent())
                .map(|p| p.to_string_lossy().to_string())
        })
        .or(ctl_path);

    let memory_keys = ["MEMORY_POOL", "MEMORY_TARGET", "BUFFER", "MAX_OS_MEMORY"];
    let wal_keys = ["RLOG_BUF_SIZE", "RLOG_POOL_SIZE", "ARCHIVE_MODE", "ARCH_INI"];

    let memory_params = pick_keys(&all_params, &memory_keys);
    let wal_params = pick_keys(&all_params, &wal_keys);

    Ok(DbInstanceConfig {
        engine: DbEngineKind::Dameng,
        config_path: cfg_path.to_path_buf(),
        port,
        data_dir,
        memory_params,
        wal_params,
        referenced_files: referenced,
        hba_rules: Vec::new(),
        all_params,
    })
}

fn parse_hba_file(path: &Path) -> anyhow::Result<Vec<String>> {
    let content = fs::read_to_string(path)?;
    let mut rules = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let normalized = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
        rules.push(normalized);
    }
    Ok(rules)
}

fn pick_keys(source: &BTreeMap<String, String>, keys: &[&str]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for k in keys {
        if let Some(v) = source.get(*k) {
            out.insert((*k).to_string(), v.clone());
        }
    }
    out
}

fn resolve_rel_path(base_dir: &Path, raw: &str) -> PathBuf {
    let p = Path::new(raw);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base_dir.join(p)
    }
}

fn strip_hash_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    for (idx, ch) in line.char_indices() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '#' if !in_single && !in_double => return &line[..idx],
            _ => {}
        }
    }
    line
}

fn parse_kv_line(line: &str) -> Option<(String, String)> {
    let (raw_k, raw_v) = if let Some((k, v)) = line.split_once('=') {
        (k, v)
    } else {
        line.split_once(char::is_whitespace)?
    };
    let key = raw_k.trim();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.') {
        return None;
    }
    let val = unquote_value(raw_v.trim());
    Some((key.to_string(), val))
}

fn unquote_value(raw: &str) -> String {
    if raw.len() >= 2
        && ((raw.starts_with('\'') && raw.ends_with('\''))
            || (raw.starts_with('"') && raw.ends_with('"')))
    {
        raw[1..raw.len() - 1].to_string()
    } else {
        raw.to_string()
    }
}
