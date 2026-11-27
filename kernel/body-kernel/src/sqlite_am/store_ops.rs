//! SQLite 事件流、协作租约、KV 胶囊与扩展私有 DDL 操作子模块

use std::collections::{BTreeSet, VecDeque};

use agent_body_sdk::{
    EventQueryFilter, KernelEdge, KernelEventRecord, KernelLease, KernelSubGraph,
    LeaseAcquireReq, LeaseMode, LeaseOutcome, RowFilterPredicate,
};
use anyhow::bail;
use chrono::{Duration as ChronoDuration, Utc};
use rusqlite::{params, types::Value as SqlValue, Connection, OptionalExtension};
use uuid::Uuid;

use super::ddl::{decode_entity_row, rls_allows_event};
use crate::scoped_sql::ScopedSqlExecutor;

pub(crate) fn edge_traverse(
    conn: &Connection,
    root_id: &str,
    max_hops: usize,
    min_weight: f32,
    rls: Option<&RowFilterPredicate>,
) -> anyhow::Result<KernelSubGraph> {
    let mut visited: BTreeSet<String> = BTreeSet::new();
    let mut seen_edges: BTreeSet<String> = BTreeSet::new();
    let mut collected_edges: Vec<KernelEdge> = Vec::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    visited.insert(root_id.to_string());
    queue.push_back((root_id.to_string(), 0));

    while let Some((curr, hop)) = queue.pop_front() {
        if hop >= max_hops {
            continue;
        }
        let mut stmt = conn.prepare(
            "SELECT edge_id, src_id, dst_id, edge_type, weight, evidence_tier,
                    source_ext, collected_at, payload_json
             FROM kernel_edges WHERE (src_id = ?1 OR dst_id = ?1) AND weight >= ?2",
        )?;
        let rows = stmt.query_map(params![curr, f64::from(min_weight)], |r| {
            let payload_raw: String = r.get(8)?;
            Ok(KernelEdge {
                edge_id: r.get(0)?,
                src_id: r.get(1)?,
                dst_id: r.get(2)?,
                edge_type: r.get(3)?,
                weight: r.get::<_, f64>(4)? as f32,
                evidence_tier: r.get(5)?,
                source_ext: r.get(6)?,
                collected_at: r.get(7)?,
                payload: serde_json::from_str(&payload_raw).unwrap_or(serde_json::Value::Null),
            })
        })?;
        for row in rows {
            let edge = row?;
            let next_node = if edge.src_id == curr {
                edge.dst_id.clone()
            } else {
                edge.src_id.clone()
            };
            if seen_edges.insert(edge.edge_id.clone()) {
                collected_edges.push(edge);
            }
            if visited.insert(next_node.clone()) {
                queue.push_back((next_node, hop + 1));
            }
        }
    }

    let mut nodes = Vec::new();
    let mut allowed_ids = BTreeSet::new();
    for nid in &visited {
        let mut stmt = conn.prepare(
            "SELECT entity_type, entity_id, evidence_id, host_id, host_labels_json,
                    source_ext, evidence_tier, collected_at, ttl_sec, payload_json,
                    redacted, evidence_sha256
             FROM kernel_entities WHERE entity_id = ?1",
        )?;
        let rows = stmt.query_map(params![nid], decode_entity_row)?;
        for row in rows {
            let tuple = row?;
            if rls.map(|p| p.matches_tuple(&tuple)).unwrap_or(true) {
                allowed_ids.insert(tuple.entity_id.clone());
                nodes.push(tuple);
            }
        }
    }
    if rls.is_some() {
        collected_edges
            .retain(|e| allowed_ids.contains(&e.src_id) && allowed_ids.contains(&e.dst_id));
    }
    Ok(KernelSubGraph {
        root_id: root_id.to_string(),
        nodes,
        edges: collected_edges,
        truncated: false,
    })
}

pub(crate) fn event_append(
    conn: &Connection,
    stream: &str,
    event: &KernelEventRecord,
) -> anyhow::Result<()> {
    let payload_json = serde_json::to_string(&event.payload)?;
    let target_stream = if event.stream.is_empty() {
        stream
    } else {
        event.stream.as_str()
    };
    conn.execute(
        "INSERT INTO kernel_event_log (
            event_id, stream, event_type, source_ext, entity_id, occurred_at, payload_json, evidence_sha256
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            event.event_id,
            target_stream,
            event.event_type,
            event.source_ext,
            event.entity_id,
            event.occurred_at,
            payload_json,
            event.evidence_sha256
        ],
    )?;
    Ok(())
}

pub(crate) fn event_query(
    conn: &Connection,
    filter: &EventQueryFilter,
    rls: Option<&RowFilterPredicate>,
) -> anyhow::Result<Vec<KernelEventRecord>> {
    let mut sql = String::from(
        "SELECT event_id, stream, event_type, source_ext, entity_id, occurred_at, payload_json, evidence_sha256
         FROM kernel_event_log WHERE 1=1",
    );
    let mut params_vec: Vec<SqlValue> = Vec::new();
    if let Some(ref st) = filter.stream {
        sql.push_str(" AND stream = ?");
        params_vec.push(SqlValue::Text(st.clone()));
    }
    if let Some(ref et) = filter.event_type {
        sql.push_str(" AND event_type = ?");
        params_vec.push(SqlValue::Text(et.clone()));
    }
    if let Some(ref ext) = filter.source_ext {
        sql.push_str(" AND source_ext = ?");
        params_vec.push(SqlValue::Text(ext.clone()));
    }
    if let Some(ref since) = filter.since_iso {
        sql.push_str(" AND occurred_at >= ?");
        params_vec.push(SqlValue::Text(since.clone()));
    }
    if let Some(ref until) = filter.until_iso {
        sql.push_str(" AND occurred_at <= ?");
        params_vec.push(SqlValue::Text(until.clone()));
    }
    sql.push_str(" ORDER BY epoch_seq ASC");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter()), |r| {
        let payload_raw: String = r.get(6)?;
        Ok(KernelEventRecord {
            event_id: r.get(0)?,
            stream: r.get(1)?,
            event_type: r.get(2)?,
            source_ext: r.get(3)?,
            entity_id: r.get(4)?,
            occurred_at: r.get(5)?,
            payload: serde_json::from_str(&payload_raw).unwrap_or(serde_json::Value::Null),
            evidence_sha256: r.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        let ev = row?;
        if rls_allows_event(conn, &ev, rls)? {
            out.push(ev);
            if filter.limit > 0 && out.len() >= filter.limit {
                break;
            }
        }
    }
    Ok(out)
}

pub(crate) fn event_prune_before(
    conn: &Connection,
    stream: &str,
    before_iso: &str,
) -> anyhow::Result<usize> {
    let affected = conn.execute(
        "DELETE FROM kernel_event_log WHERE stream = ?1 AND occurred_at < ?2",
        params![stream, before_iso],
    )?;
    Ok(affected)
}

pub(crate) fn lease_try_acquire(
    conn: &Connection,
    req: &LeaseAcquireReq,
) -> anyhow::Result<LeaseOutcome> {
    if req.ttl_sec == 0 {
        bail!("租约 ttl_sec 必须大于 0");
    }
    let now = Utc::now();
    let now_iso = now.to_rfc3339();
    let expires_iso = (now + ChronoDuration::seconds(req.ttl_sec as i64)).to_rfc3339();
    let tx = conn.unchecked_transaction()?;

    let mut stmt = tx.prepare(
        "SELECT holder_id, mode, expires_at, fence_token
         FROM kernel_leases
         WHERE resource_type = ?1 AND resource_id = ?2 AND expires_at > ?3",
    )?;
    let active: Vec<(String, String, String, i64)> = stmt
        .query_map(params![req.resource_type, req.resource_id, now_iso], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    for (holder, mode_str, exp, _) in &active {
        if holder != &req.holder_id
            && (req.mode == LeaseMode::Exclusive || mode_str == "Exclusive")
        {
            tx.commit()?;
            return Ok(LeaseOutcome::Busy {
                holder_id: holder.clone(),
                expires_at: exp.clone(),
            });
        }
    }

    let max_fence: Option<i64> = tx.query_row(
        "SELECT MAX(fence_token) FROM kernel_leases WHERE resource_type = ?1 AND resource_id = ?2",
        params![req.resource_type, req.resource_id],
        |r| r.get(0),
    )?;
    let next_fence = max_fence.unwrap_or(0).max(0) as u64 + 1;
    let lock_id = Uuid::new_v4().to_string();
    let mode_label = match req.mode {
        LeaseMode::Shared => "Shared",
        LeaseMode::Exclusive => "Exclusive",
    };
    tx.execute(
        "INSERT INTO kernel_leases (
            lock_id, resource_type, resource_id, holder_id, mode, acquired_at, expires_at, fence_token
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            lock_id,
            req.resource_type,
            req.resource_id,
            req.holder_id,
            mode_label,
            now_iso,
            expires_iso,
            next_fence as i64
        ],
    )?;
    tx.commit()?;
    Ok(LeaseOutcome::Acquired(KernelLease {
        lock_id,
        resource_type: req.resource_type.clone(),
        resource_id: req.resource_id.clone(),
        holder_id: req.holder_id.clone(),
        mode: req.mode,
        acquired_at: now_iso,
        expires_at: expires_iso,
        fence_token: next_fence,
    }))
}

pub(crate) fn lease_release(conn: &Connection, lock_id: &str) -> anyhow::Result<bool> {
    let count = conn.execute("DELETE FROM kernel_leases WHERE lock_id = ?1", params![lock_id])?;
    Ok(count > 0)
}

pub(crate) fn lease_list(
    conn: &Connection,
    resource_type: Option<&str>,
) -> anyhow::Result<Vec<KernelLease>> {
    let now_iso = Utc::now().to_rfc3339();
    let mut sql = String::from(
        "SELECT lock_id, resource_type, resource_id, holder_id, mode, acquired_at, expires_at, fence_token
         FROM kernel_leases WHERE expires_at > ?1",
    );
    let mut args = vec![SqlValue::Text(now_iso)];
    if let Some(rt) = resource_type {
        sql.push_str(" AND resource_type = ?2");
        args.push(SqlValue::Text(rt.to_string()));
    }
    sql.push_str(" ORDER BY fence_token ASC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |r| {
        let mode_str: String = r.get(4)?;
        Ok(KernelLease {
            lock_id: r.get(0)?,
            resource_type: r.get(1)?,
            resource_id: r.get(2)?,
            holder_id: r.get(3)?,
            mode: if mode_str == "Exclusive" {
                LeaseMode::Exclusive
            } else {
                LeaseMode::Shared
            },
            acquired_at: r.get(5)?,
            expires_at: r.get(6)?,
            fence_token: r.get::<_, i64>(7)?.max(0) as u64,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub(crate) fn lease_gc_expired(conn: &Connection) -> anyhow::Result<usize> {
    let now_iso = Utc::now().to_rfc3339();
    let count = conn.execute(
        "DELETE FROM kernel_leases WHERE expires_at <= ?1",
        params![now_iso],
    )?;
    Ok(count)
}

pub(crate) fn kv_put(
    conn: &Connection,
    namespace: &str,
    key: &str,
    val: &serde_json::Value,
) -> anyhow::Result<()> {
    let val_json = serde_json::to_string(val)?;
    let now = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO kernel_checkpoints (namespace, k, val_json, updated_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(namespace, k) DO UPDATE SET
            val_json = excluded.val_json,
            updated_at = excluded.updated_at",
        params![namespace, key, val_json, now],
    )?;
    Ok(())
}

pub(crate) fn kv_get(
    conn: &Connection,
    namespace: &str,
    key: &str,
) -> anyhow::Result<Option<serde_json::Value>> {
    let raw: Option<String> = conn
        .query_row(
            "SELECT val_json FROM kernel_checkpoints WHERE namespace = ?1 AND k = ?2",
            params![namespace, key],
            |r| r.get(0),
        )
        .optional()?;
    raw.map(|s| serde_json::from_str(&s).map_err(anyhow::Error::from))
        .transpose()
}

pub(crate) fn exec_scoped_migration(
    conn: &Connection,
    ext_id: &str,
    table_prefix: &str,
    version: &str,
    sql: &str,
) -> anyhow::Result<()> {
    ScopedSqlExecutor::execute_on_sqlite(conn, ext_id, table_prefix, sql)?;
    let key = format!("{ext_id}:{version}");
    let marker = serde_json::json!({
        "ext_id": ext_id,
        "version": version,
        "table_prefix": table_prefix,
        "applied_at": Utc::now().to_rfc3339(),
    });
    kv_put(conn, "ext_migrations", &key, &marker)
}
