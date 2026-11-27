//! 微内核 8 张 `body_cat_*` 系统目录表、扩展 DAG 拓扑排序与生命周期状态机

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use agent_body_protocol::ExtensionControlManifest;
use agent_body_sdk::{AutonomyLevel, ExtensionMeta, ExtensionRegistrar, WorkerTrigger};
use anyhow::{bail, Context};
use chrono::Utc;
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// 8 张核心系统目录表 SQLite DDL（对标 PostgreSQL `pg_catalog`）
pub const CATALOG_SCHEMA_SQL: &str = r#"
PRAGMA foreign_keys = ON;
CREATE TABLE IF NOT EXISTS body_cat_extension (
    ext_id          TEXT PRIMARY KEY,
    version         TEXT NOT NULL,
    kind            TEXT NOT NULL CHECK (kind IN ('builtin_core','compiled_crate','out_of_process')),
    state           TEXT NOT NULL CHECK (state IN ('discovered','verified','installed','enabled','disabled','upgraded','dropped','failed')),
    table_prefix    TEXT NOT NULL UNIQUE,
    requires_json   TEXT NOT NULL DEFAULT '[]',
    control_hash    TEXT NOT NULL,
    installed_at    TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS body_cat_type (
    type_id         TEXT PRIMARY KEY,
    type_kind       TEXT NOT NULL CHECK (type_kind IN ('entity','edge')),
    owner_ext_id    TEXT NOT NULL REFERENCES body_cat_extension(ext_id) ON DELETE CASCADE,
    schema_json     TEXT NOT NULL DEFAULT '{}',
    default_ttl_sec INTEGER NOT NULL DEFAULT 0,
    default_tier    TEXT NOT NULL DEFAULT 'L1_EXPLICIT'
);
CREATE TABLE IF NOT EXISTS body_cat_opclass (
    opclass_id      TEXT PRIMARY KEY,
    am_id           TEXT NOT NULL,
    target_type_id  TEXT NOT NULL,
    strategy_num    INTEGER NOT NULL,
    operator_symbol TEXT NOT NULL,
    owner_ext_id    TEXT NOT NULL REFERENCES body_cat_extension(ext_id) ON DELETE CASCADE,
    UNIQUE(am_id, target_type_id, strategy_num)
);
CREATE TABLE IF NOT EXISTS body_cat_am (
    am_id           TEXT PRIMARY KEY,
    am_kind         TEXT NOT NULL CHECK (am_kind IN ('table','index')),
    owner_ext_id    TEXT NOT NULL REFERENCES body_cat_extension(ext_id) ON DELETE CASCADE,
    default_weight  REAL NOT NULL DEFAULT 1.0,
    capabilities    TEXT NOT NULL DEFAULT '{}'
);
CREATE TABLE IF NOT EXISTS body_cat_fdw (
    fdw_id          TEXT PRIMARY KEY,
    owner_ext_id    TEXT NOT NULL REFERENCES body_cat_extension(ext_id) ON DELETE CASCADE,
    transport       TEXT NOT NULL CHECK (transport IN ('in_process','stdio_jsonrpc','unix_socket')),
    autonomy_level  TEXT NOT NULL CHECK (autonomy_level IN ('L0_OBSERVE','L1_AUTONOMIC')),
    timeout_ms      INTEGER NOT NULL DEFAULT 1500,
    ttl_sec         INTEGER NOT NULL DEFAULT 5
);
CREATE TABLE IF NOT EXISTS body_cat_proc (
    proc_name       TEXT PRIMARY KEY,
    owner_ext_id    TEXT NOT NULL REFERENCES body_cat_extension(ext_id) ON DELETE CASCADE,
    cli_path_json   TEXT NOT NULL,
    rest_method     TEXT NOT NULL,
    rest_path       TEXT NOT NULL UNIQUE,
    profiles_json   TEXT NOT NULL DEFAULT '["full"]',
    volatility      TEXT NOT NULL CHECK (volatility IN ('immutable','stable','volatile')),
    autonomy_level  TEXT NOT NULL CHECK (autonomy_level IN ('L0_OBSERVE','L1_AUTONOMIC','L2_SUPERVISED','L3_FORBIDDEN')),
    input_schema    TEXT NOT NULL,
    provenance_rule TEXT
);
CREATE TABLE IF NOT EXISTS body_cat_hook (
    hook_id         TEXT PRIMARY KEY,
    hook_point      TEXT NOT NULL CHECK (hook_point IN (
        'client_auth','object_access','planner_rewrite',
        'executor_start','executor_end','tuple_redact','emit_audit'
    )),
    owner_ext_id    TEXT NOT NULL REFERENCES body_cat_extension(ext_id) ON DELETE CASCADE,
    priority        INTEGER NOT NULL DEFAULT 100,
    fail_policy     TEXT NOT NULL CHECK (fail_policy IN ('fail_closed','fail_open'))
);
CREATE TABLE IF NOT EXISTS body_cat_bgworker (
    worker_id       TEXT PRIMARY KEY,
    owner_ext_id    TEXT NOT NULL REFERENCES body_cat_extension(ext_id) ON DELETE CASCADE,
    trigger_kind    TEXT NOT NULL CHECK (trigger_kind IN ('interval','event')),
    interval_sec    INTEGER NOT NULL DEFAULT 60,
    autonomy_level  TEXT NOT NULL CHECK (autonomy_level IN ('L0_OBSERVE','L1_AUTONOMIC')),
    mem_budget_mb   INTEGER NOT NULL DEFAULT 16
);
"#;

/// 扩展生命周期状态机
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionState {
    Discovered,
    Verified,
    Installed,
    Enabled,
    Disabled,
    Upgraded,
    Dropped,
    Failed,
}

impl ExtensionState {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Discovered => "discovered",
            Self::Verified => "verified",
            Self::Installed => "installed",
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::Upgraded => "upgraded",
            Self::Dropped => "dropped",
            Self::Failed => "failed",
        }
    }

    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s {
            "discovered" => Ok(Self::Discovered),
            "verified" => Ok(Self::Verified),
            "installed" => Ok(Self::Installed),
            "enabled" => Ok(Self::Enabled),
            "disabled" => Ok(Self::Disabled),
            "upgraded" => Ok(Self::Upgraded),
            "dropped" => Ok(Self::Dropped),
            "failed" => Ok(Self::Failed),
            other => bail!("未知的扩展生命周期状态: '{other}'"),
        }
    }

    /// 校验状态机合法跃迁 (`Discovered -> Verified -> Installed -> Enabled / Disabled / Dropped`)
    pub const fn can_transition_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Discovered, Self::Verified)
                | (Self::Discovered, Self::Failed)
                | (Self::Verified, Self::Installed)
                | (Self::Verified, Self::Failed)
                | (Self::Installed, Self::Enabled)
                | (Self::Installed, Self::Disabled)
                | (Self::Installed, Self::Failed)
                | (Self::Enabled, Self::Disabled)
                | (Self::Enabled, Self::Upgraded)
                | (Self::Enabled, Self::Failed)
                | (Self::Disabled, Self::Enabled)
                | (Self::Disabled, Self::Dropped)
                | (Self::Disabled, Self::Failed)
                | (Self::Upgraded, Self::Enabled)
                | (Self::Upgraded, Self::Failed)
                | (Self::Failed, Self::Discovered)
                | (Self::Failed, Self::Dropped)
                | (Self::Dropped, Self::Discovered)
        )
    }
}

/// `body_cat_extension` 行映射结构
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionRecord {
    pub ext_id: String,
    pub version: String,
    pub kind: String,
    pub state: ExtensionState,
    pub table_prefix: String,
    pub requires: Vec<String>,
    pub control_hash: String,
    pub installed_at: String,
    pub updated_at: String,
}

/// 初始化 8 张 `body_cat_*` 系统目录表
pub fn init_catalog_schema(conn: &Connection) -> anyhow::Result<()> {
    conn.execute_batch(CATALOG_SCHEMA_SQL)
        .context("初始化 8 张 body_cat_* 系统目录表失败")
}

/// 计算 `.control.toml` 原文 SHA-256 指纹
pub fn compute_control_hash(raw_toml: &str) -> String {
    let digest = Sha256::digest(raw_toml.as_bytes());
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

/// 生成确定格式的新目录追踪 ID (UUID v4)
pub fn new_catalog_trace_id() -> String {
    Uuid::new_v4().to_string()
}

/// 校验扩展私有物理表命名空间前缀 (`ext_<ext_id>__<table_name>`)
pub fn validate_scoped_table_name(ext_id: &str, table_name: &str) -> anyhow::Result<()> {
    let pattern = format!(r"^ext_{}__[a-z0-9_]+$", regex::escape(ext_id));
    let re = Regex::new(&pattern).context("构建表名隔离正则失败")?;
    if !re.is_match(table_name) {
        bail!("物理表名 '{table_name}' 未命中扩展 '{ext_id}' 的隔离前缀 'ext_{ext_id}__'");
    }
    Ok(())
}

/// 扩展依赖 DAG 拓扑排序（带缺失依赖校验与环路检测）
pub fn topo_sort_extensions(metas: &[ExtensionMeta]) -> anyhow::Result<Vec<String>> {
    let mut nodes: BTreeMap<String, &ExtensionMeta> = BTreeMap::new();
    for meta in metas {
        if nodes.insert(meta.ext_id.clone(), meta).is_some() {
            bail!("检测到重复注册的扩展 ID: '{}'", meta.ext_id);
        }
    }

    let mut in_degree: BTreeMap<String, usize> = BTreeMap::new();
    let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (ext_id, meta) in &nodes {
        let mut unique_reqs: BTreeSet<&str> = BTreeSet::new();
        for req in &meta.requires {
            if req == ext_id {
                bail!("检测到扩展自依赖环路: '{ext_id}' -> '{ext_id}'");
            }
            if !nodes.contains_key(req) {
                bail!("扩展 '{ext_id}' 依赖的扩展 '{req}' 未找到（缺失依赖）");
            }
            unique_reqs.insert(req.as_str());
        }
        in_degree.insert(ext_id.clone(), unique_reqs.len());
        for req in unique_reqs {
            dependents.entry(req.to_string()).or_default().push(ext_id.clone());
        }
    }

    let mut queue: VecDeque<String> = in_degree
        .iter()
        .filter_map(|(id, deg)| if *deg == 0 { Some(id.clone()) } else { None })
        .collect();
    let mut order: Vec<String> = Vec::with_capacity(nodes.len());

    while let Some(curr) = queue.pop_front() {
        order.push(curr.clone());
        if let Some(children) = dependents.get(&curr) {
            let mut newly_ready = BTreeSet::new();
            for child in children {
                if let Some(deg) = in_degree.get_mut(child) {
                    *deg -= 1;
                    if *deg == 0 {
                        newly_ready.insert(child.clone());
                    }
                }
            }
            for next_id in newly_ready {
                queue.push_back(next_id);
            }
        }
    }

    if order.len() != nodes.len() {
        let cyclic: Vec<String> = in_degree
            .into_iter()
            .filter_map(|(id, deg)| if deg > 0 { Some(id) } else { None })
            .collect();
        bail!("检测到扩展依赖 DAG 环路，涉及扩展: {}", cyclic.join(", "));
    }
    Ok(order)
}

/// 系统目录持久化管理器
pub struct CatalogStore;

impl CatalogStore {
    pub fn init(conn: &Connection) -> anyhow::Result<()> {
        init_catalog_schema(conn)
    }

    /// 解析 `.control.toml` 并注册扩展进入 `Discovered` 状态
    pub fn register_from_control_toml(
        conn: &Connection,
        raw_toml: &str,
    ) -> anyhow::Result<ExtensionMeta> {
        let _parsed_raw: toml::Value = toml::from_str(raw_toml).context("TOML 语法解析失败")?;
        let manifest = ExtensionControlManifest::from_toml_str(raw_toml)?;
        let meta = manifest.to_extension_meta();
        let hash = compute_control_hash(raw_toml);
        Self::register_discovered(conn, &meta, &hash)?;
        Ok(meta)
    }

    /// 登记扩展进入 `Discovered` 初始状态
    pub fn register_discovered(
        conn: &Connection,
        meta: &ExtensionMeta,
        control_hash: &str,
    ) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        let reqs_json = serde_json::to_string(&meta.requires)?;
        conn.execute(
            "INSERT INTO body_cat_extension (
                ext_id, version, kind, state, table_prefix, requires_json, control_hash, installed_at, updated_at
             ) VALUES (?1, ?2, ?3, 'discovered', ?4, ?5, ?6, ?7, ?7)
             ON CONFLICT(ext_id) DO UPDATE SET
                version = excluded.version, kind = excluded.kind, table_prefix = excluded.table_prefix,
                requires_json = excluded.requires_json, control_hash = excluded.control_hash,
                updated_at = excluded.updated_at",
            params![
                meta.ext_id, meta.version, meta.kind.as_str(),
                meta.table_prefix, reqs_json, control_hash, now
            ],
        )
        .with_context(|| format!("写入扩展 '{}' 到 body_cat_extension 失败", meta.ext_id))?;
        Ok(())
    }

    /// 执行扩展生命周期状态跃迁（严格校验状态机合法性）
    pub fn transition_state(
        conn: &Connection,
        ext_id: &str,
        target: ExtensionState,
    ) -> anyhow::Result<ExtensionState> {
        let current_str: Option<String> = conn
            .query_row(
                "SELECT state FROM body_cat_extension WHERE ext_id = ?1",
                params![ext_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(curr_str) = current_str else {
            bail!("扩展 '{ext_id}' 尚未在 body_cat_extension 中登记");
        };
        let curr = ExtensionState::parse(&curr_str)?;
        if !curr.can_transition_to(target) {
            bail!("扩展 '{ext_id}' 不允许从 '{}' 跃迁到 '{}'", curr.as_str(), target.as_str());
        }
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE body_cat_extension SET state = ?1, updated_at = ?2 WHERE ext_id = ?3",
            params![target.as_str(), now, ext_id],
        )?;
        Ok(target)
    }

    /// 将 `ExtensionRegistrar` 中声明的 6 大 Routine 与 7 大 Hook 同步写入 8 张 `body_cat_*` 目录表
    pub fn sync_registrar(
        conn: &Connection,
        reg: &ExtensionRegistrar,
        control_hash: &str,
    ) -> anyhow::Result<()> {
        let ext_id = &reg.meta.ext_id;
        let tx = conn.unchecked_transaction()?;
        Self::register_discovered(&tx, &reg.meta, control_hash)?;

        for tbl in [
            "body_cat_type", "body_cat_opclass", "body_cat_am", "body_cat_fdw",
            "body_cat_proc", "body_cat_hook", "body_cat_bgworker",
        ] {
            tx.execute(&format!("DELETE FROM {tbl} WHERE owner_ext_id = ?1"), params![ext_id])?;
        }
        for ty in &reg.types {
            let tid = ty.type_id();
            let kind = if tid.starts_with("edge:") { "edge" } else { "entity" };
            tx.execute(
                "INSERT INTO body_cat_type (type_id, type_kind, owner_ext_id) VALUES (?1, ?2, ?3)",
                params![tid, kind, ext_id],
            )?;
        }
        for op in &reg.operators {
            let op_id = op.routine.op_id();
            tx.execute(
                "INSERT INTO body_cat_opclass (opclass_id, am_id, target_type_id, strategy_num, operator_symbol, owner_ext_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![op_id, op.am_id, op.target_type_id, op.routine.strategy_num() as i64, op_id, ext_id],
            )?;
        }
        for am in &reg.table_ams {
            tx.execute(
                "INSERT INTO body_cat_am (am_id, am_kind, owner_ext_id, default_weight) VALUES (?1, 'table', ?2, 1.0)",
                params![am.am_id(), ext_id],
            )?;
        }
        for am in &reg.index_ams {
            tx.execute(
                "INSERT INTO body_cat_am (am_id, am_kind, owner_ext_id, default_weight) VALUES (?1, 'index', ?2, ?3)",
                params![am.am_id(), ext_id, am.default_weight()],
            )?;
        }
        for fdw in &reg.fdws {
            let transport = if reg.meta.kind == agent_body_sdk::ExtensionKind::OutOfProcess {
                "stdio_jsonrpc"
            } else {
                "in_process"
            };
            tx.execute(
                "INSERT INTO body_cat_fdw (fdw_id, owner_ext_id, transport, autonomy_level) VALUES (?1, ?2, ?3, 'L0_OBSERVE')",
                params![fdw.fdw_id(), ext_id, transport],
            )?;
        }
        for proc_item in &reg.procs {
            let spec = proc_item.spec();
            tx.execute(
                "INSERT INTO body_cat_proc (
                    proc_name, owner_ext_id, cli_path_json, rest_method, rest_path,
                    profiles_json, volatility, autonomy_level, input_schema, provenance_rule
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    spec.proc_name, ext_id, serde_json::to_string(spec.cli_path)?,
                    spec.rest_route.0, spec.rest_route.1, serde_json::to_string(spec.profiles)?,
                    spec.volatility.as_str(), spec.autonomy_level.as_str(),
                    serde_json::to_string(&spec.input_schema)?, spec.provenance_rule,
                ],
            )?;
        }
        for hook in &reg.hooks {
            tx.execute(
                "INSERT INTO body_cat_hook (hook_id, hook_point, owner_ext_id, priority, fail_policy)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![hook.hook_id(), hook.hook_point(), ext_id, hook.priority(), hook.fail_policy().as_str()],
            )?;
        }
        for worker in &reg.bgworkers {
            let auto = worker.autonomy_level();
            if auto != AutonomyLevel::L0Observe && auto != AutonomyLevel::L1Autonomic {
                bail!("BgWorker '{}' 自治级别 {:?} 非法", worker.worker_id(), auto);
            }
            let (trig_kind, interval_sec) = match worker.trigger() {
                WorkerTrigger::Interval(d) => ("interval", d.as_secs() as i64),
                WorkerTrigger::EventTopics(_) => ("event", 0_i64),
            };
            tx.execute(
                "INSERT INTO body_cat_bgworker (worker_id, owner_ext_id, trigger_kind, interval_sec, autonomy_level)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![worker.worker_id(), ext_id, trig_kind, interval_sec, auto.as_str()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn get_extension(conn: &Connection, ext_id: &str) -> anyhow::Result<Option<ExtensionRecord>> {
        let row = conn
            .query_row(
                "SELECT ext_id, version, kind, state, table_prefix, requires_json, control_hash, installed_at, updated_at
                 FROM body_cat_extension WHERE ext_id = ?1",
                params![ext_id],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?, r.get::<_, String>(4)?, r.get::<_, String>(5)?,
                        r.get::<_, String>(6)?, r.get::<_, String>(7)?, r.get::<_, String>(8)?,
                    ))
                },
            )
            .optional()?;
        let Some((id, ver, kind, st_raw, prefix, req_raw, hash, ins, upd)) = row else {
            return Ok(None);
        };
        Ok(Some(ExtensionRecord {
            ext_id: id,
            version: ver,
            kind,
            state: ExtensionState::parse(&st_raw)?,
            table_prefix: prefix,
            requires: serde_json::from_str(&req_raw)?,
            control_hash: hash,
            installed_at: ins,
            updated_at: upd,
        }))
    }
}
