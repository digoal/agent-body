//! SQLite 核心表存储访问方法 (`SqliteCoreTableAm`)：5 张基表 + FTS5 + RLS 下推

pub mod ddl;
mod store_ops;

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use agent_body_sdk::{
    EntityScanFilter, EventQueryFilter, EvidenceTuple, KernelEdge, KernelEventRecord, KernelLease,
    KernelSubGraph, LeaseAcquireReq, LeaseOutcome, RowFilterPredicate, TableAmRoutine,
};
use anyhow::{anyhow, Context};
use chrono::Utc;
use rusqlite::{params, types::Value as SqlValue, Connection, OptionalExtension};

use crate::catalog::init_catalog_schema;
pub use crate::scoped_sql::ScopedSqlExecutor;
pub use ddl::BASE_TABLES_SQL;
use ddl::{append_rls_sql_clauses, build_safe_fts_query, decode_entity_row};

/// SQLite 微内核默认 `TableAmRoutine` 实现
pub struct SqliteCoreTableAm {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteCoreTableAm {
    pub fn from_shared_conn(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }

    pub fn open(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("打开 SQLite 内核库失败: {}", path.as_ref().display()))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn open_in_memory() -> anyhow::Result<Self> {
        let conn = Connection::open_in_memory().context("创建内存 SQLite 连接失败")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn shared_conn(&self) -> Arc<Mutex<Connection>> {
        Arc::clone(&self.conn)
    }

    fn lock_conn(&self) -> anyhow::Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| anyhow!("SQLite 连接互斥锁已中毒"))
    }

    /// 读取当前全局 `kernel_event_log.epoch_seq` 最大水位线
    pub fn current_epoch(&self) -> anyhow::Result<u64> {
        let conn = self.lock_conn()?;
        let max_seq: Option<i64> = conn.query_row(
            "SELECT MAX(epoch_seq) FROM kernel_event_log",
            [],
            |r| r.get(0),
        )?;
        Ok(max_seq.unwrap_or(0).max(0) as u64)
    }

    /// 基于核心 FTS5 虚表执行带 RLS 下推的实体全文检索
    pub fn fts_search(
        &self,
        query: &str,
        rls: Option<&RowFilterPredicate>,
        limit: usize,
    ) -> anyhow::Result<Vec<(EvidenceTuple, f64)>> {
        let fts_query = build_safe_fts_query(query);
        if fts_query.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.lock_conn()?;
        let cap = if limit == 0 { 20 } else { limit };
        let mut sql = String::from(
            "SELECT e.entity_type, e.entity_id, e.evidence_id, e.host_id, e.host_labels_json,
                    e.source_ext, e.evidence_tier, e.collected_at, e.ttl_sec, e.payload_json,
                    e.redacted, e.evidence_sha256, bm25(kernel_entities_fts) AS rank_score
             FROM kernel_entities_fts f
             JOIN kernel_entities e
               ON e.entity_type = f.entity_type AND e.entity_id = f.entity_id
             WHERE kernel_entities_fts MATCH ?1",
        );
        let mut params_vec: Vec<SqlValue> = vec![SqlValue::Text(fts_query)];
        append_rls_sql_clauses(&mut sql, &mut params_vec, rls, "e.");
        sql.push_str(" ORDER BY rank_score ASC LIMIT ?");
        params_vec.push(SqlValue::Integer(cap as i64));

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter()), |r| {
            let tuple = decode_entity_row(r)?;
            let bm25: f64 = r.get(12)?;
            Ok((tuple, (-bm25).max(0.0001)))
        })?;
        let mut out = Vec::new();
        for item in rows {
            let (tuple, score) = item?;
            if rls.map(|p| p.matches_tuple(&tuple)).unwrap_or(true) {
                out.push((tuple, score));
            }
        }
        Ok(out)
    }
}

impl TableAmRoutine for SqliteCoreTableAm {
    fn am_id(&self) -> &'static str {
        "am:sqlite_core"
    }

    fn init_catalog_and_base_tables(&self) -> anyhow::Result<()> {
        let conn = self.lock_conn()?;
        init_catalog_schema(&conn)?;
        conn.execute_batch(BASE_TABLES_SQL)
            .context("初始化 5 张微内核通用基表与 FTS5 虚表失败")
    }

    fn entity_upsert(&self, tuple: &EvidenceTuple) -> anyhow::Result<()> {
        let conn = self.lock_conn()?;
        let labels_json = serde_json::to_string(&tuple.host_labels)?;
        let payload_json = serde_json::to_string(&tuple.payload)?;
        let tenant_id = tuple.host_labels.get("tenant_id").cloned();
        let project_id = tuple.host_labels.get("project_id").cloned();
        let env = tuple.host_labels.get("env").cloned();
        let now = Utc::now().to_rfc3339();
        let fts_text = format!(
            "{} {} {} {}",
            tuple.entity_type, tuple.entity_id, tuple.host_id, payload_json
        );

        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO kernel_entities (
                entity_type, entity_id, evidence_id, host_id, host_labels_json,
                tenant_id, project_id, env, source_ext, evidence_tier,
                collected_at, ttl_sec, payload_json, redacted, evidence_sha256, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
             ON CONFLICT(entity_type, entity_id) DO UPDATE SET
                evidence_id = excluded.evidence_id, host_id = excluded.host_id,
                host_labels_json = excluded.host_labels_json, tenant_id = excluded.tenant_id,
                project_id = excluded.project_id, env = excluded.env, source_ext = excluded.source_ext,
                evidence_tier = excluded.evidence_tier, collected_at = excluded.collected_at,
                ttl_sec = excluded.ttl_sec, payload_json = excluded.payload_json,
                redacted = excluded.redacted, evidence_sha256 = excluded.evidence_sha256,
                updated_at = excluded.updated_at",
            params![
                tuple.entity_type,
                tuple.entity_id,
                tuple.evidence_id,
                tuple.host_id,
                labels_json,
                tenant_id,
                project_id,
                env,
                tuple.source_ext,
                tuple.evidence_tier,
                tuple.collected_at,
                tuple.ttl_sec as i64,
                payload_json,
                i64::from(tuple.redacted),
                tuple.evidence_sha256,
                now
            ],
        )?;
        tx.execute(
            "DELETE FROM kernel_entities_fts WHERE entity_type = ?1 AND entity_id = ?2",
            params![tuple.entity_type, tuple.entity_id],
        )?;
        tx.execute(
            "INSERT INTO kernel_entities_fts (entity_type, entity_id, source_ext, payload_text)
             VALUES (?1, ?2, ?3, ?4)",
            params![tuple.entity_type, tuple.entity_id, tuple.source_ext, fts_text],
        )?;
        tx.commit()?;
        Ok(())
    }

    fn entity_get(
        &self,
        entity_type: &str,
        entity_id: &str,
        rls: Option<&RowFilterPredicate>,
    ) -> anyhow::Result<Option<EvidenceTuple>> {
        let conn = self.lock_conn()?;
        let mut sql = String::from(
            "SELECT entity_type, entity_id, evidence_id, host_id, host_labels_json,
                    source_ext, evidence_tier, collected_at, ttl_sec, payload_json,
                    redacted, evidence_sha256
             FROM kernel_entities
             WHERE entity_type = ?1 AND entity_id = ?2",
        );
        let mut params_vec = vec![
            SqlValue::Text(entity_type.to_string()),
            SqlValue::Text(entity_id.to_string()),
        ];
        append_rls_sql_clauses(&mut sql, &mut params_vec, rls, "");
        let found = conn
            .query_row(&sql, rusqlite::params_from_iter(params_vec.iter()), decode_entity_row)
            .optional()?;
        Ok(found.filter(|t| rls.map(|p| p.matches_tuple(t)).unwrap_or(true)))
    }

    fn entity_scan(
        &self,
        filter: &EntityScanFilter,
        rls: Option<&RowFilterPredicate>,
    ) -> anyhow::Result<Vec<EvidenceTuple>> {
        let conn = self.lock_conn()?;
        let mut sql = String::from(
            "SELECT entity_type, entity_id, evidence_id, host_id, host_labels_json,
                    source_ext, evidence_tier, collected_at, ttl_sec, payload_json,
                    redacted, evidence_sha256
             FROM kernel_entities WHERE 1=1",
        );
        let mut params_vec: Vec<SqlValue> = Vec::new();
        if let Some(ref et) = filter.entity_type {
            sql.push_str(" AND entity_type = ?");
            params_vec.push(SqlValue::Text(et.clone()));
        }
        if let Some(ref ext) = filter.source_ext {
            sql.push_str(" AND source_ext = ?");
            params_vec.push(SqlValue::Text(ext.clone()));
        }
        if let Some(ref hid) = filter.host_id {
            sql.push_str(" AND host_id = ?");
            params_vec.push(SqlValue::Text(hid.clone()));
        }
        append_rls_sql_clauses(&mut sql, &mut params_vec, rls, "");
        sql.push_str(" ORDER BY entity_type ASC, entity_id ASC");

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec.iter()), decode_entity_row)?;
        let mut matched = Vec::new();
        for row in rows {
            let tuple = row?;
            let labels_ok = filter
                .label_filters
                .iter()
                .all(|(k, v)| tuple.host_labels.get(k) == Some(v));
            let rls_ok = rls.map(|p| p.matches_tuple(&tuple)).unwrap_or(true);
            if labels_ok && rls_ok {
                matched.push(tuple);
            }
        }
        let start = filter.offset.min(matched.len());
        let end = if filter.limit == 0 {
            matched.len()
        } else {
            start.saturating_add(filter.limit).min(matched.len())
        };
        Ok(matched[start..end].to_vec())
    }

    fn entity_delete_by_source(&self, source_ext: &str, path_or_key: &str) -> anyhow::Result<usize> {
        let conn = self.lock_conn()?;
        let tx = conn.unchecked_transaction()?;
        let affected = if path_or_key.is_empty() {
            tx.execute("DELETE FROM kernel_entities WHERE source_ext = ?1", params![source_ext])?
        } else {
            tx.execute(
                "DELETE FROM kernel_entities WHERE source_ext = ?1 AND entity_id = ?2",
                params![source_ext, path_or_key],
            )?
        };
        if path_or_key.is_empty() {
            tx.execute("DELETE FROM kernel_entities_fts WHERE source_ext = ?1", params![source_ext])?;
        } else {
            tx.execute(
                "DELETE FROM kernel_entities_fts WHERE source_ext = ?1 AND entity_id = ?2",
                params![source_ext, path_or_key],
            )?;
        }
        tx.commit()?;
        Ok(affected)
    }

    fn edge_upsert(&self, edge: &KernelEdge) -> anyhow::Result<()> {
        let conn = self.lock_conn()?;
        let payload_json = serde_json::to_string(&edge.payload)?;
        conn.execute(
            "INSERT INTO kernel_edges (
                edge_id, src_id, dst_id, edge_type, weight, evidence_tier,
                source_ext, collected_at, payload_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(edge_id) DO UPDATE SET
                src_id = excluded.src_id, dst_id = excluded.dst_id, edge_type = excluded.edge_type,
                weight = excluded.weight, evidence_tier = excluded.evidence_tier,
                source_ext = excluded.source_ext, collected_at = excluded.collected_at,
                payload_json = excluded.payload_json",
            params![
                edge.edge_id,
                edge.src_id,
                edge.dst_id,
                edge.edge_type,
                f64::from(edge.weight),
                edge.evidence_tier,
                edge.source_ext,
                edge.collected_at,
                payload_json
            ],
        )?;
        Ok(())
    }

    fn edge_traverse(
        &self,
        root_id: &str,
        max_hops: usize,
        min_weight: f32,
        rls: Option<&RowFilterPredicate>,
    ) -> anyhow::Result<KernelSubGraph> {
        store_ops::edge_traverse(&*self.lock_conn()?, root_id, max_hops, min_weight, rls)
    }

    fn event_append(&self, stream: &str, event: &KernelEventRecord) -> anyhow::Result<()> {
        store_ops::event_append(&*self.lock_conn()?, stream, event)
    }

    fn event_query(
        &self,
        filter: &EventQueryFilter,
        rls: Option<&RowFilterPredicate>,
    ) -> anyhow::Result<Vec<KernelEventRecord>> {
        store_ops::event_query(&*self.lock_conn()?, filter, rls)
    }

    fn event_prune_before(&self, stream: &str, before_iso: &str) -> anyhow::Result<usize> {
        store_ops::event_prune_before(&*self.lock_conn()?, stream, before_iso)
    }

    fn lease_try_acquire(&self, req: &LeaseAcquireReq) -> anyhow::Result<LeaseOutcome> {
        store_ops::lease_try_acquire(&*self.lock_conn()?, req)
    }

    fn lease_release(&self, lock_id: &str) -> anyhow::Result<bool> {
        store_ops::lease_release(&*self.lock_conn()?, lock_id)
    }

    fn lease_list(&self, resource_type: Option<&str>) -> anyhow::Result<Vec<KernelLease>> {
        store_ops::lease_list(&*self.lock_conn()?, resource_type)
    }

    fn lease_gc_expired(&self) -> anyhow::Result<usize> {
        store_ops::lease_gc_expired(&*self.lock_conn()?)
    }

    fn kv_put(&self, namespace: &str, key: &str, val: &serde_json::Value) -> anyhow::Result<()> {
        store_ops::kv_put(&*self.lock_conn()?, namespace, key, val)
    }

    fn kv_get(&self, namespace: &str, key: &str) -> anyhow::Result<Option<serde_json::Value>> {
        store_ops::kv_get(&*self.lock_conn()?, namespace, key)
    }

    fn exec_scoped_migration(
        &self,
        ext_id: &str,
        table_prefix: &str,
        version: &str,
        sql: &str,
    ) -> anyhow::Result<()> {
        store_ops::exec_scoped_migration(&*self.lock_conn()?, ext_id, table_prefix, version, sql)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use agent_body_sdk::{LeaseMode, LeaseOutcome};

    fn sample_tuple(id: &str, tenant: &str, env: &str, host: &str, msg: &str) -> EvidenceTuple {
        let mut labels = BTreeMap::new();
        labels.insert("tenant_id".to_string(), tenant.to_string());
        labels.insert("env".to_string(), env.to_string());
        EvidenceTuple {
            evidence_id: format!("ev-{id}"),
            host_id: host.to_string(),
            host_labels: labels,
            entity_type: "entity:node".to_string(),
            entity_id: id.to_string(),
            source_ext: "core_test".to_string(),
            evidence_tier: "L1_EXPLICIT".to_string(),
            collected_at: Utc::now().to_rfc3339(),
            ttl_sec: 60,
            payload: serde_json::json!({"summary": msg}),
            redacted: false,
            evidence_sha256: String::new(),
        }
        .with_computed_sha256()
    }

    #[test]
    fn sqlite_am_base_tables_rls_fts_and_leases_work() {
        let am = SqliteCoreTableAm::open_in_memory().unwrap();
        am.init_catalog_and_base_tables().unwrap();

        am.entity_upsert(&sample_tuple("n1", "t1", "prod", "h1", "kernel memory snapshot"))
            .unwrap();
        am.entity_upsert(&sample_tuple("n2", "t2", "staging", "h2", "kernel memory snapshot"))
            .unwrap();

        let rls_t1 = RowFilterPredicate {
            tenant_id: Some("t1".to_string()),
            env: Some("prod".to_string()),
            ..Default::default()
        };
        assert!(am.entity_get("entity:node", "n1", Some(&rls_t1)).unwrap().is_some());
        assert!(am.entity_get("entity:node", "n2", Some(&rls_t1)).unwrap().is_none());

        let fts = am.fts_search("kernel snapshot", Some(&rls_t1), 10).unwrap();
        assert_eq!(fts.len(), 1);
        assert_eq!(fts[0].0.entity_id, "n1");

        let l1 = am
            .lease_try_acquire(&LeaseAcquireReq {
                resource_type: "node".to_string(),
                resource_id: "n1".to_string(),
                holder_id: "worker-a".to_string(),
                mode: LeaseMode::Exclusive,
                ttl_sec: 30,
            })
            .unwrap();
        assert!(matches!(l1, LeaseOutcome::Acquired(_)));
        let l2 = am
            .lease_try_acquire(&LeaseAcquireReq {
                resource_type: "node".to_string(),
                resource_id: "n1".to_string(),
                holder_id: "worker-b".to_string(),
                mode: LeaseMode::Shared,
                ttl_sec: 30,
            })
            .unwrap();
        assert!(matches!(l2, LeaseOutcome::Busy { .. }));

        am.kv_put("fdw_cursor", "probe_x", &serde_json::json!({"epoch": 3}))
            .unwrap();
        assert_eq!(
            am.kv_get("fdw_cursor", "probe_x").unwrap(),
            Some(serde_json::json!({"epoch": 3}))
        );
    }

    #[test]
    fn sqlite_am_scoped_migration_enforces_prefix() {
        let am = SqliteCoreTableAm::open_in_memory().unwrap();
        am.init_catalog_and_base_tables().unwrap();

        am.exec_scoped_migration(
            "demo_ext",
            "ext_demo_ext__",
            "0.1.0",
            "CREATE TABLE IF NOT EXISTS ext_demo_ext__items (id TEXT PRIMARY KEY);",
        )
        .unwrap();

        let err = am.exec_scoped_migration(
            "demo_ext",
            "ext_demo_ext__",
            "0.1.1",
            "DROP TABLE kernel_entities;",
        );
        assert!(err.is_err());
    }
}
