use super::DuckDbStorageProvider;
use anyhow::{Result};
use duckdb::params;
use crate::storage::models::*;

impl DuckDbStorageProvider {
    pub(super) fn impl_insert_coordination_lock(&self, lock: &CoordinationLock) -> Result<bool> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute_batch("BEGIN TRANSACTION;")?;
        let res: Result<bool> = (|| {
            // 1. 清理该资源已过期的锁
            conn.execute(
                "DELETE FROM coordination_locks
                 WHERE resource_type = ?1 AND resource_id = ?2
                   AND COALESCE(try_cast(expires_at AS TIMESTAMPTZ) <= try_cast(?3 AS TIMESTAMPTZ), expires_at <= ?3)",
                params![lock.resource_type, lock.resource_id, now],
            )?;

            // 2. 检查有效冲突
            let mut stmt = conn.prepare(
                "SELECT lock_id, scope, holder_agent, expires_at FROM coordination_locks
                 WHERE resource_type = ?1 AND resource_id = ?2
                   AND COALESCE(try_cast(expires_at AS TIMESTAMPTZ) > try_cast(?3 AS TIMESTAMPTZ), expires_at > ?3)"
            )?;
            let mut rows = stmt.query(params![lock.resource_type, lock.resource_id, now])?;

            while let Some(row) = rows.next()? {
                let existing_scope: String = row.get(1)?;
                let existing_holder: String = row.get(2)?;

                if existing_holder == lock.holder_agent {
                    continue;
                }

                if lock.scope == "exclusive" || existing_scope == "exclusive" {
                    return Ok(false);
                }
            }
            drop(rows);
            drop(stmt);

            // 3. 插入新锁
            let metadata_str = lock
                .metadata
                .as_ref()
                .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string()))
                .unwrap_or_else(|| "{}".to_string());

            let pid_i64 = lock.holder_pid.map(|p| p as i64);

            conn.execute(
                "INSERT INTO coordination_locks (lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT(lock_id) DO UPDATE SET
                    expires_at = excluded.expires_at,
                    metadata = excluded.metadata,
                    scope = excluded.scope,
                    holder_pid = excluded.holder_pid",
                params![
                    lock.lock_id,
                    lock.resource_type,
                    lock.resource_id,
                    lock.scope,
                    lock.holder_agent,
                    pid_i64,
                    lock.acquired_at,
                    lock.expires_at,
                    metadata_str,
                ],
            )?;
            Ok(true)
        })();
        match res {
            Ok(true) => {
                conn.execute_batch("COMMIT;")?;
                Ok(true)
            }
            Ok(false) => {
                if let Err(rb) = conn.execute_batch("ROLLBACK;") {
                    eprintln!("[DuckDB][WARN] insert_coordination_lock conflict ROLLBACK 失败: {}", rb);
                }
                Ok(false)
            }
            Err(e) => {
                if let Err(rb) = conn.execute_batch("ROLLBACK;") {
                    eprintln!("[DuckDB][WARN] insert_coordination_lock err ROLLBACK 失败: {}", rb);
                }
                Err(e)
            }
        }
    }


    pub(super) fn impl_delete_coordination_lock(&self, lock_id: &str) -> Result<bool> {
        let conn = self.lock_conn()?;
        let affected = conn.execute("DELETE FROM coordination_locks WHERE lock_id = ?1", params![lock_id])?;
        Ok(affected > 0)
    }


    pub(super) fn impl_get_coordination_lock(&self, resource_type: &str, resource_id: &str) -> Result<Option<CoordinationLock>> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata
             FROM coordination_locks
             WHERE resource_type = ?1 AND resource_id = ?2
               AND COALESCE(try_cast(expires_at AS TIMESTAMPTZ) > try_cast(?3 AS TIMESTAMPTZ), expires_at > ?3)
             ORDER BY acquired_at DESC LIMIT 1"
        )?;
        let mut rows = stmt.query(params![resource_type, resource_id, now])?;
        if let Some(row) = rows.next()? {
            let meta_str: String = row.get(8)?;
            let metadata = serde_json::from_str(&meta_str).ok();
            let pid_i64: Option<i64> = row.get(5)?;
            Ok(Some(CoordinationLock {
                lock_id: row.get(0)?,
                resource_type: row.get(1)?,
                resource_id: row.get(2)?,
                scope: row.get(3)?,
                holder_agent: row.get(4)?,
                holder_pid: pid_i64.map(|p| p as u32),
                acquired_at: row.get(6)?,
                expires_at: row.get(7)?,
                metadata,
            }))
        } else {
            Ok(None)
        }
    }


    pub(super) fn impl_list_coordination_locks(&self, resource_type: Option<&str>) -> Result<Vec<CoordinationLock>> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        let sql = match resource_type {
            Some(_) => "SELECT lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata
                        FROM coordination_locks
                        WHERE resource_type = ?1
                          AND COALESCE(try_cast(expires_at AS TIMESTAMPTZ) > try_cast(?2 AS TIMESTAMPTZ), expires_at > ?2)
                        ORDER BY acquired_at DESC",
            None => "SELECT lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata
                     FROM coordination_locks
                     WHERE COALESCE(try_cast(expires_at AS TIMESTAMPTZ) > try_cast(?1 AS TIMESTAMPTZ), expires_at > ?1)
                     ORDER BY acquired_at DESC",
        };
        let mut stmt = conn.prepare(sql)?;
        let mut rows = match resource_type {
            Some(rt) => stmt.query(params![rt, now])?,
            None => stmt.query(params![now])?,
        };

        let mut list = Vec::new();
        while let Some(row) = rows.next()? {
            let meta_str: String = row.get(8)?;
            let metadata = serde_json::from_str(&meta_str).ok();
            let pid_i64: Option<i64> = row.get(5)?;
            list.push(CoordinationLock {
                lock_id: row.get(0)?,
                resource_type: row.get(1)?,
                resource_id: row.get(2)?,
                scope: row.get(3)?,
                holder_agent: row.get(4)?,
                holder_pid: pid_i64.map(|p| p as u32),
                acquired_at: row.get(6)?,
                expires_at: row.get(7)?,
                metadata,
            });
        }
        Ok(list)
    }


    pub(super) fn impl_gc_expired_coordination_locks(&self) -> Result<usize> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        let count = conn.execute(
            "DELETE FROM coordination_locks
             WHERE COALESCE(try_cast(expires_at AS TIMESTAMPTZ) <= try_cast(?1 AS TIMESTAMPTZ), expires_at <= ?1)",
            params![now],
        )?;
        Ok(count)
    }


    pub(super) fn impl_count_expired_coordination_locks(&self) -> Result<usize> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM coordination_locks
             WHERE COALESCE(try_cast(expires_at AS TIMESTAMPTZ) <= try_cast(?1 AS TIMESTAMPTZ), expires_at <= ?1)",
            params![now],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }


    pub(super) fn impl_enqueue_lock_wait(&self, entry: &LockWaitEntry) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO lock_wait_queue (wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.wait_id, entry.resource_type, entry.resource_id, entry.scope,
                entry.waiting_agent, entry.priority, entry.enqueued_at, entry.timeout_at, entry.status
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_list_lock_wait_queue(&self, resource_type: Option<&str>, resource_id: Option<&str>) -> Result<Vec<LockWaitEntry>> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut res = Vec::new();
        match (resource_type, resource_id) {
            (Some(rt), Some(rid)) => {
                let mut stmt = conn.prepare(
                    "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
                     FROM lock_wait_queue
                     WHERE resource_type = ?1 AND resource_id = ?2 AND status IN ('waiting','notified')
                       AND COALESCE(try_cast(timeout_at AS TIMESTAMPTZ) > try_cast(?3 AS TIMESTAMPTZ), timeout_at > ?3)
                     ORDER BY priority ASC, enqueued_at ASC"
                )?;
                let rows = stmt.query_map(params![rt, rid, now], Self::map_wait_row)?;
                for r in rows { res.push(r?); }
            }
            (Some(rt), None) => {
                let mut stmt = conn.prepare(
                    "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
                     FROM lock_wait_queue
                     WHERE resource_type = ?1 AND status IN ('waiting','notified')
                       AND COALESCE(try_cast(timeout_at AS TIMESTAMPTZ) > try_cast(?2 AS TIMESTAMPTZ), timeout_at > ?2)
                     ORDER BY priority ASC, enqueued_at ASC"
                )?;
                let rows = stmt.query_map(params![rt, now], Self::map_wait_row)?;
                for r in rows { res.push(r?); }
            }
            _ => {
                let mut stmt = conn.prepare(
                    "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
                     FROM lock_wait_queue
                     WHERE status IN ('waiting','notified')
                       AND COALESCE(try_cast(timeout_at AS TIMESTAMPTZ) > try_cast(?1 AS TIMESTAMPTZ), timeout_at > ?1)
                     ORDER BY priority ASC, enqueued_at ASC"
                )?;
                let rows = stmt.query_map(params![now], Self::map_wait_row)?;
                for r in rows { res.push(r?); }
            }
        }
        Ok(res)
    }


    pub(super) fn impl_pop_next_lock_waiter(&self, resource_type: &str, resource_id: &str) -> Result<Option<LockWaitEntry>> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
             FROM lock_wait_queue
             WHERE resource_type = ?1 AND resource_id = ?2 AND status = 'waiting'
               AND COALESCE(try_cast(timeout_at AS TIMESTAMPTZ) > try_cast(?3 AS TIMESTAMPTZ), timeout_at > ?3)
             ORDER BY priority ASC, enqueued_at ASC LIMIT 1"
        )?;
        let mut rows = stmt.query_map(params![resource_type, resource_id, now], Self::map_wait_row)?;
        if let Some(first) = rows.next() {
            let mut item = first?;
            drop(rows);
            drop(stmt);
            conn.execute(
                "UPDATE lock_wait_queue SET status = 'notified' WHERE wait_id = ?1",
                params![item.wait_id],
            )?;
            item.status = "notified".to_string();
            Ok(Some(item))
        } else {
            Ok(None)
        }
    }


    pub(super) fn impl_update_lock_wait_status(&self, wait_id: &str, status: &str) -> Result<bool> {
        let conn = self.lock_conn()?;
        let affected = conn.execute(
            "UPDATE lock_wait_queue SET status = ?1 WHERE wait_id = ?2",
            params![status, wait_id],
        )?;
        Ok(affected > 0)
    }


    pub(super) fn impl_gc_expired_lock_waits(&self) -> Result<usize> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        let affected = conn.execute(
            "UPDATE lock_wait_queue SET status = 'expired'
             WHERE status IN ('waiting','notified')
               AND COALESCE(try_cast(timeout_at AS TIMESTAMPTZ) <= try_cast(?1 AS TIMESTAMPTZ), timeout_at <= ?1)",
            params![now],
        )?;
        Ok(affected)
    }


}
