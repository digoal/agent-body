//! SQLite 基表 DDL、RLS SQL 下推辅助与行解码

use std::collections::BTreeMap;

use agent_body_sdk::{EvidenceTuple, KernelEventRecord, RowFilterPredicate};
use rusqlite::{params, types::Value as SqlValue, Connection};

/// 5 张内核通用基表与 FTS5 核心检索虚表 DDL
pub const BASE_TABLES_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS kernel_entities (
    entity_type      TEXT NOT NULL,
    entity_id        TEXT NOT NULL,
    evidence_id      TEXT NOT NULL,
    host_id          TEXT NOT NULL,
    host_labels_json TEXT NOT NULL DEFAULT '{}',
    tenant_id        TEXT,
    project_id       TEXT,
    env              TEXT,
    source_ext       TEXT NOT NULL,
    evidence_tier    TEXT NOT NULL,
    collected_at     TEXT NOT NULL,
    ttl_sec          INTEGER NOT NULL DEFAULT 0,
    payload_json     TEXT NOT NULL DEFAULT '{}',
    redacted         INTEGER NOT NULL DEFAULT 0,
    evidence_sha256  TEXT NOT NULL,
    updated_at       TEXT NOT NULL,
    PRIMARY KEY (entity_type, entity_id)
);
CREATE INDEX IF NOT EXISTS idx_kernel_entities_type_ext
    ON kernel_entities(entity_type, source_ext);
CREATE INDEX IF NOT EXISTS idx_kernel_entities_rls
    ON kernel_entities(tenant_id, project_id, env, host_id);

CREATE VIRTUAL TABLE IF NOT EXISTS kernel_entities_fts USING fts5(
    entity_type,
    entity_id,
    source_ext,
    payload_text
);

CREATE TABLE IF NOT EXISTS kernel_edges (
    edge_id         TEXT PRIMARY KEY,
    src_id          TEXT NOT NULL,
    dst_id          TEXT NOT NULL,
    edge_type       TEXT NOT NULL,
    weight          REAL NOT NULL DEFAULT 1.0,
    evidence_tier   TEXT NOT NULL,
    source_ext      TEXT NOT NULL,
    collected_at    TEXT NOT NULL,
    payload_json    TEXT NOT NULL DEFAULT '{}'
);
CREATE INDEX IF NOT EXISTS idx_kernel_edges_src ON kernel_edges(src_id);
CREATE INDEX IF NOT EXISTS idx_kernel_edges_dst ON kernel_edges(dst_id);

CREATE TABLE IF NOT EXISTS kernel_event_log (
    epoch_seq       INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id        TEXT NOT NULL UNIQUE,
    stream          TEXT NOT NULL,
    event_type      TEXT NOT NULL,
    source_ext      TEXT NOT NULL,
    entity_id       TEXT,
    occurred_at     TEXT NOT NULL,
    payload_json    TEXT NOT NULL DEFAULT '{}',
    evidence_sha256 TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_kernel_event_log_stream_time
    ON kernel_event_log(stream, occurred_at);

CREATE TABLE IF NOT EXISTS kernel_leases (
    lock_id         TEXT PRIMARY KEY,
    resource_type   TEXT NOT NULL,
    resource_id     TEXT NOT NULL,
    holder_id       TEXT NOT NULL,
    mode            TEXT NOT NULL CHECK (mode IN ('Shared', 'Exclusive')),
    acquired_at     TEXT NOT NULL,
    expires_at      TEXT NOT NULL,
    fence_token     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_kernel_leases_res
    ON kernel_leases(resource_type, resource_id);

CREATE TABLE IF NOT EXISTS kernel_checkpoints (
    namespace       TEXT NOT NULL,
    k               TEXT NOT NULL,
    val_json        TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    PRIMARY KEY (namespace, k)
);
"#;

pub(crate) fn append_rls_sql_clauses(
    sql: &mut String,
    params_vec: &mut Vec<SqlValue>,
    rls: Option<&RowFilterPredicate>,
    alias: &str,
) {
    let Some(pred) = rls else {
        return;
    };
    if let Some(ref tid) = pred.tenant_id {
        sql.push_str(&format!(" AND {alias}tenant_id = ?"));
        params_vec.push(SqlValue::Text(tid.clone()));
    }
    if let Some(ref pid) = pred.project_id {
        sql.push_str(&format!(" AND {alias}project_id = ?"));
        params_vec.push(SqlValue::Text(pid.clone()));
    }
    if let Some(ref env) = pred.env {
        sql.push_str(&format!(" AND {alias}env = ?"));
        params_vec.push(SqlValue::Text(env.clone()));
    }
    if let Some(ref hosts) = pred.allowed_host_ids {
        if hosts.is_empty() {
            sql.push_str(" AND 1=0");
        } else {
            let placeholders = vec!["?"; hosts.len()].join(",");
            sql.push_str(&format!(" AND {alias}host_id IN ({placeholders})"));
            for h in hosts {
                params_vec.push(SqlValue::Text(h.clone()));
            }
        }
    }
}

pub(crate) fn decode_entity_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<EvidenceTuple> {
    let labels_raw: String = r.get(4)?;
    let payload_raw: String = r.get(9)?;
    let host_labels: BTreeMap<String, String> =
        serde_json::from_str(&labels_raw).unwrap_or_default();
    let payload: serde_json::Value =
        serde_json::from_str(&payload_raw).unwrap_or(serde_json::Value::Null);
    Ok(EvidenceTuple {
        entity_type: r.get(0)?,
        entity_id: r.get(1)?,
        evidence_id: r.get(2)?,
        host_id: r.get(3)?,
        host_labels,
        source_ext: r.get(5)?,
        evidence_tier: r.get(6)?,
        collected_at: r.get(7)?,
        ttl_sec: r.get::<_, i64>(8)?.max(0) as u64,
        payload,
        redacted: r.get::<_, i64>(10)? != 0,
        evidence_sha256: r.get(11)?,
    })
}

pub(crate) fn build_safe_fts_query(raw: &str) -> String {
    let tokens: Vec<String> = raw
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|s| !s.is_empty())
        .map(|s| format!("\"{}\"", s.replace('"', "")))
        .collect();
    tokens.join(" OR ")
}

pub(crate) fn rls_allows_event(
    conn: &Connection,
    ev: &KernelEventRecord,
    rls: Option<&RowFilterPredicate>,
) -> anyhow::Result<bool> {
    let Some(pred) = rls else {
        return Ok(true);
    };
    let has_filter = pred.tenant_id.is_some()
        || pred.project_id.is_some()
        || pred.env.is_some()
        || pred.allowed_host_ids.is_some()
        || !pred.custom_labels.is_empty();
    if !has_filter {
        return Ok(true);
    }
    if let Some(ref eid) = ev.entity_id {
        let mut stmt = conn.prepare(
            "SELECT entity_type, entity_id, evidence_id, host_id, host_labels_json,
                    source_ext, evidence_tier, collected_at, ttl_sec, payload_json,
                    redacted, evidence_sha256
             FROM kernel_entities WHERE entity_id = ?1",
        )?;
        let rows = stmt.query_map(params![eid], decode_entity_row)?;
        for row in rows {
            if pred.matches_tuple(&row?) {
                return Ok(true);
            }
        }
        return Ok(false);
    }
    let label_match = |key: &str, expected: &Option<String>| -> bool {
        match expected {
            None => true,
            Some(v) => ev.payload.get(key).and_then(|x| x.as_str()) == Some(v.as_str()),
        }
    };
    Ok(label_match("tenant_id", &pred.tenant_id)
        && label_match("project_id", &pred.project_id)
        && label_match("env", &pred.env))
}
