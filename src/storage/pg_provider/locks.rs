use super::PgStorageProvider;
use anyhow::{Result};
use crate::storage::models::*;

impl PgStorageProvider {
    pub(super) fn impl_insert_coordination_lock(&self, lock: &CoordinationLock) -> Result<bool> {
        let mut client = self.lock_client()?;
        let mut tx = client.transaction()?;
        let now = chrono::Utc::now();

        // 1. 先清理该资源上已过期的锁记录（统一按 timestamptz 瞬间对比，消除 RFC3339 'T' 与 NOW()::text ' ' 字典序偏差）
        tx.execute(
            "DELETE FROM coordination_locks
             WHERE resource_type = $1 AND resource_id = $2
               AND expires_at::timestamptz <= $3",
            &[&lock.resource_type, &lock.resource_id, &now],
        )?;

        // 2. 检查当前资源是否已被有效锁占用
        let rows = tx.query(
            "SELECT lock_id, scope, holder_agent, expires_at FROM coordination_locks
             WHERE resource_type = $1 AND resource_id = $2
               AND expires_at::timestamptz > $3",
            &[&lock.resource_type, &lock.resource_id, &now],
        )?;

        for row in rows {
            let existing_scope: String = row.get(1);
            let existing_holder: String = row.get(2);

            if existing_holder == lock.holder_agent {
                continue;
            }

            if lock.scope == "exclusive" || existing_scope == "exclusive" {
                return Ok(false);
            }
        }

        // 3. 无冲突，插入新锁
        let metadata_val = lock.metadata.clone().unwrap_or(serde_json::json!({}));
        let pid_i32 = lock.holder_pid.map(|p| p as i32);

        tx.execute(
            "INSERT INTO coordination_locks (lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT(lock_id) DO UPDATE SET
                expires_at = EXCLUDED.expires_at,
                metadata = EXCLUDED.metadata,
                scope = EXCLUDED.scope,
                holder_pid = EXCLUDED.holder_pid",
            &[
                &lock.lock_id,
                &lock.resource_type,
                &lock.resource_id,
                &lock.scope,
                &lock.holder_agent,
                &pid_i32,
                &lock.acquired_at,
                &lock.expires_at,
                &metadata_val,
            ],
        )?;
        tx.commit()?;
        Ok(true)
    }


    pub(super) fn impl_delete_coordination_lock(&self, lock_id: &str) -> Result<bool> {
        let mut client = self.lock_client()?;
        let affected = client.execute("DELETE FROM coordination_locks WHERE lock_id = $1", &[&lock_id])?;
        Ok(affected > 0)
    }


    pub(super) fn impl_get_coordination_lock(&self, resource_type: &str, resource_id: &str) -> Result<Option<CoordinationLock>> {
        let mut client = self.lock_client()?;
        let now = chrono::Utc::now();
        let rows = client.query(
            "SELECT lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata
             FROM coordination_locks
             WHERE resource_type = $1 AND resource_id = $2
               AND expires_at::timestamptz > $3
             ORDER BY acquired_at DESC LIMIT 1",
            &[&resource_type, &resource_id, &now],
        )?;

        if let Some(row) = rows.into_iter().next() {
            let pid_i32: Option<i32> = row.get(5);
            Ok(Some(CoordinationLock {
                lock_id: row.get(0),
                resource_type: row.get(1),
                resource_id: row.get(2),
                scope: row.get(3),
                holder_agent: row.get(4),
                holder_pid: pid_i32.map(|p| p as u32),
                acquired_at: row.get(6),
                expires_at: row.get(7),
                metadata: row.get(8),
            }))
        } else {
            Ok(None)
        }
    }


    pub(super) fn impl_list_coordination_locks(&self, resource_type: Option<&str>) -> Result<Vec<CoordinationLock>> {
        let mut client = self.lock_client()?;
        let now = chrono::Utc::now();
        let rows = match resource_type {
            Some(rt) => client.query(
                "SELECT lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata
                 FROM coordination_locks
                 WHERE resource_type = $1 AND expires_at::timestamptz > $2
                 ORDER BY acquired_at DESC",
                &[&rt, &now],
            )?,
            None => client.query(
                "SELECT lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata
                 FROM coordination_locks
                 WHERE expires_at::timestamptz > $1
                 ORDER BY acquired_at DESC",
                &[&now],
            )?,
        };

        let mut list = Vec::new();
        for row in rows {
            let pid_i32: Option<i32> = row.get(5);
            list.push(CoordinationLock {
                lock_id: row.get(0),
                resource_type: row.get(1),
                resource_id: row.get(2),
                scope: row.get(3),
                holder_agent: row.get(4),
                holder_pid: pid_i32.map(|p| p as u32),
                acquired_at: row.get(6),
                expires_at: row.get(7),
                metadata: row.get(8),
            });
        }
        Ok(list)
    }


    pub(super) fn impl_gc_expired_coordination_locks(&self) -> Result<usize> {
        let mut client = self.lock_client()?;
        let now = chrono::Utc::now();
        let affected = client.execute(
            "DELETE FROM coordination_locks WHERE expires_at::timestamptz <= $1",
            &[&now],
        )?;
        Ok(affected as usize)
    }


    pub(super) fn impl_count_expired_coordination_locks(&self) -> Result<usize> {
        let mut client = self.lock_client()?;
        let now = chrono::Utc::now();
        let row = client.query_one(
            "SELECT COUNT(*) FROM coordination_locks WHERE expires_at::timestamptz <= $1",
            &[&now],
        )?;
        let count: i64 = row.get(0);
        Ok(count.max(0) as usize)
    }


    pub(super) fn impl_enqueue_lock_wait(&self, entry: &LockWaitEntry) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO lock_wait_queue (wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
             ON CONFLICT (wait_id) DO UPDATE SET
                scope = EXCLUDED.scope,
                priority = EXCLUDED.priority,
                timeout_at = EXCLUDED.timeout_at,
                status = EXCLUDED.status",
            &[
                &entry.wait_id, &entry.resource_type, &entry.resource_id, &entry.scope,
                &entry.waiting_agent, &entry.priority, &entry.enqueued_at, &entry.timeout_at, &entry.status,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_list_lock_wait_queue(&self, resource_type: Option<&str>, resource_id: Option<&str>) -> Result<Vec<LockWaitEntry>> {
        let mut client = self.lock_client()?;
        let now = chrono::Utc::now();
        let rows = match (resource_type, resource_id) {
            (Some(rt), Some(rid)) => client.query(
                "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
                 FROM lock_wait_queue
                 WHERE resource_type = $1 AND resource_id = $2 AND status IN ('waiting','notified') AND timeout_at::timestamptz > $3
                 ORDER BY priority ASC, enqueued_at ASC",
                &[&rt, &rid, &now],
            )?,
            (Some(rt), None) => client.query(
                "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
                 FROM lock_wait_queue
                 WHERE resource_type = $1 AND status IN ('waiting','notified') AND timeout_at::timestamptz > $2
                 ORDER BY priority ASC, enqueued_at ASC",
                &[&rt, &now],
            )?,
            _ => client.query(
                "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
                 FROM lock_wait_queue
                 WHERE status IN ('waiting','notified') AND timeout_at::timestamptz > $1
                 ORDER BY priority ASC, enqueued_at ASC",
                &[&now],
            )?,
        };
        Ok(rows.into_iter().map(|r| Self::row_to_wait(&r)).collect())
    }


    pub(super) fn impl_pop_next_lock_waiter(&self, resource_type: &str, resource_id: &str) -> Result<Option<LockWaitEntry>> {
        let mut client = self.lock_client()?;
        let now = chrono::Utc::now();
        let rows = client.query(
            "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
             FROM lock_wait_queue
             WHERE resource_type = $1 AND resource_id = $2 AND status = 'waiting' AND timeout_at::timestamptz > $3
             ORDER BY priority ASC, enqueued_at ASC LIMIT 1",
            &[&resource_type, &resource_id, &now],
        )?;
        if let Some(r) = rows.into_iter().next() {
            let mut item = Self::row_to_wait(&r);
            client.execute(
                "UPDATE lock_wait_queue SET status = 'notified' WHERE wait_id = $1",
                &[&item.wait_id],
            )?;
            item.status = "notified".to_string();
            Ok(Some(item))
        } else {
            Ok(None)
        }
    }


    pub(super) fn impl_update_lock_wait_status(&self, wait_id: &str, status: &str) -> Result<bool> {
        let mut client = self.lock_client()?;
        let affected = client.execute(
            "UPDATE lock_wait_queue SET status = $1 WHERE wait_id = $2",
            &[&status, &wait_id],
        )?;
        Ok(affected > 0)
    }


    pub(super) fn impl_gc_expired_lock_waits(&self) -> Result<usize> {
        let mut client = self.lock_client()?;
        let now = chrono::Utc::now();
        let affected = client.execute(
            "UPDATE lock_wait_queue SET status = 'expired' WHERE status IN ('waiting','notified') AND timeout_at::timestamptz <= $1",
            &[&now],
        )?;
        Ok(affected as usize)
    }


}
