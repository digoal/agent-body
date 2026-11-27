//! 纯内存无盘 `TableAmRoutine` 实现 (`MemoryTableAm`)，供极简无盘探针与单元测试使用

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use agent_body_sdk::{
    EntityScanFilter, EventQueryFilter, EvidenceTuple, KernelEdge, KernelEventRecord, KernelLease,
    KernelSubGraph, LeaseAcquireReq, LeaseMode, LeaseOutcome, RowFilterPredicate, TableAmRoutine,
};
use anyhow::{anyhow, bail};
use chrono::{Duration as ChronoDuration, Utc};
use uuid::Uuid;

use crate::scoped_sql::ScopedSqlExecutor;

#[derive(Default)]
struct MemoryStore {
    initialized: bool,
    entities: BTreeMap<(String, String), EvidenceTuple>,
    edges: BTreeMap<String, KernelEdge>,
    events: Vec<(u64, KernelEventRecord)>,
    next_epoch_seq: u64,
    leases: BTreeMap<String, KernelLease>,
    fence_counters: BTreeMap<(String, String), u64>,
    kv: BTreeMap<(String, String), serde_json::Value>,
    migrations: BTreeMap<(String, String), Vec<String>>,
}

/// 纯内存无盘 `TableAmRoutine` 实现
#[derive(Clone, Default)]
pub struct MemoryTableAm {
    store: Arc<RwLock<MemoryStore>>,
}

impl MemoryTableAm {
    pub fn new() -> Self {
        Self::default()
    }

    fn read_store(&self) -> anyhow::Result<RwLockReadGuard<'_, MemoryStore>> {
        self.store
            .read()
            .map_err(|_| anyhow!("MemoryTableAm 读锁已中毒"))
    }

    fn write_store(&self) -> anyhow::Result<RwLockWriteGuard<'_, MemoryStore>> {
        self.store
            .write()
            .map_err(|_| anyhow!("MemoryTableAm 写锁已中毒"))
    }

    pub fn current_epoch(&self) -> anyhow::Result<u64> {
        let guard = self.read_store()?;
        Ok(guard.next_epoch_seq)
    }

    pub fn applied_migrations(&self) -> anyhow::Result<Vec<(String, String)>> {
        let guard = self.read_store()?;
        Ok(guard.migrations.keys().cloned().collect())
    }
}

impl TableAmRoutine for MemoryTableAm {
    fn am_id(&self) -> &'static str {
        "am:memory"
    }

    fn init_catalog_and_base_tables(&self) -> anyhow::Result<()> {
        let mut guard = self.write_store()?;
        guard.initialized = true;
        Ok(())
    }

    fn entity_upsert(&self, tuple: &EvidenceTuple) -> anyhow::Result<()> {
        let mut guard = self.write_store()?;
        guard.entities.insert(
            (tuple.entity_type.clone(), tuple.entity_id.clone()),
            tuple.clone(),
        );
        Ok(())
    }

    fn entity_get(
        &self,
        entity_type: &str,
        entity_id: &str,
        rls: Option<&RowFilterPredicate>,
    ) -> anyhow::Result<Option<EvidenceTuple>> {
        let guard = self.read_store()?;
        let found = guard
            .entities
            .get(&(entity_type.to_string(), entity_id.to_string()))
            .filter(|t| rls.map(|p| p.matches_tuple(t)).unwrap_or(true))
            .cloned();
        Ok(found)
    }

    fn entity_scan(
        &self,
        filter: &EntityScanFilter,
        rls: Option<&RowFilterPredicate>,
    ) -> anyhow::Result<Vec<EvidenceTuple>> {
        let guard = self.read_store()?;
        let matched: Vec<EvidenceTuple> = guard
            .entities
            .values()
            .filter(|t| {
                if let Some(ref et) = filter.entity_type {
                    if &t.entity_type != et {
                        return false;
                    }
                }
                if let Some(ref ext) = filter.source_ext {
                    if &t.source_ext != ext {
                        return false;
                    }
                }
                if let Some(ref hid) = filter.host_id {
                    if &t.host_id != hid {
                        return false;
                    }
                }
                let labels_ok = filter
                    .label_filters
                    .iter()
                    .all(|(k, v)| t.host_labels.get(k) == Some(v));
                labels_ok && rls.map(|p| p.matches_tuple(t)).unwrap_or(true)
            })
            .cloned()
            .collect();

        let start = filter.offset.min(matched.len());
        let end = if filter.limit == 0 {
            matched.len()
        } else {
            start.saturating_add(filter.limit).min(matched.len())
        };
        Ok(matched[start..end].to_vec())
    }

    fn entity_delete_by_source(&self, source_ext: &str, path_or_key: &str) -> anyhow::Result<usize> {
        let mut guard = self.write_store()?;
        let before = guard.entities.len();
        guard.entities.retain(|(_, eid), tuple| {
            if tuple.source_ext != source_ext {
                return true;
            }
            if path_or_key.is_empty() {
                false
            } else {
                eid != path_or_key
            }
        });
        Ok(before.saturating_sub(guard.entities.len()))
    }

    fn edge_upsert(&self, edge: &KernelEdge) -> anyhow::Result<()> {
        let mut guard = self.write_store()?;
        guard.edges.insert(edge.edge_id.clone(), edge.clone());
        Ok(())
    }

    fn edge_traverse(
        &self,
        root_id: &str,
        max_hops: usize,
        min_weight: f32,
        rls: Option<&RowFilterPredicate>,
    ) -> anyhow::Result<KernelSubGraph> {
        let guard = self.read_store()?;
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
            for edge in guard.edges.values() {
                if edge.weight < min_weight {
                    continue;
                }
                if edge.src_id != curr && edge.dst_id != curr {
                    continue;
                }
                let next = if edge.src_id == curr {
                    edge.dst_id.clone()
                } else {
                    edge.src_id.clone()
                };
                if seen_edges.insert(edge.edge_id.clone()) {
                    collected_edges.push(edge.clone());
                }
                if visited.insert(next.clone()) {
                    queue.push_back((next, hop + 1));
                }
            }
        }

        let mut nodes = Vec::new();
        let mut allowed_ids = BTreeSet::new();
        for tuple in guard.entities.values() {
            if visited.contains(&tuple.entity_id) && rls.map(|p| p.matches_tuple(tuple)).unwrap_or(true)
            {
                allowed_ids.insert(tuple.entity_id.clone());
                nodes.push(tuple.clone());
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

    fn event_append(&self, stream: &str, event: &KernelEventRecord) -> anyhow::Result<()> {
        let mut guard = self.write_store()?;
        guard.next_epoch_seq += 1;
        let seq = guard.next_epoch_seq;
        let mut rec = event.clone();
        if rec.stream.is_empty() {
            rec.stream = stream.to_string();
        }
        guard.events.push((seq, rec));
        Ok(())
    }

    fn event_query(
        &self,
        filter: &EventQueryFilter,
        rls: Option<&RowFilterPredicate>,
    ) -> anyhow::Result<Vec<KernelEventRecord>> {
        let guard = self.read_store()?;
        let mut out = Vec::new();
        for (_, ev) in &guard.events {
            if let Some(ref st) = filter.stream {
                if &ev.stream != st {
                    continue;
                }
            }
            if let Some(ref et) = filter.event_type {
                if &ev.event_type != et {
                    continue;
                }
            }
            if let Some(ref ext) = filter.source_ext {
                if &ev.source_ext != ext {
                    continue;
                }
            }
            if let Some(ref since) = filter.since_iso {
                if ev.occurred_at.as_str() < since.as_str() {
                    continue;
                }
            }
            if let Some(ref until) = filter.until_iso {
                if ev.occurred_at.as_str() > until.as_str() {
                    continue;
                }
            }
            if let Some(pred) = rls {
                if let Some(ref eid) = ev.entity_id {
                    let ok = guard
                        .entities
                        .values()
                        .any(|t| &t.entity_id == eid && pred.matches_tuple(t));
                    if !ok {
                        continue;
                    }
                }
            }
            out.push(ev.clone());
            if filter.limit > 0 && out.len() >= filter.limit {
                break;
            }
        }
        Ok(out)
    }

    fn event_prune_before(&self, stream: &str, before_iso: &str) -> anyhow::Result<usize> {
        let mut guard = self.write_store()?;
        let before = guard.events.len();
        guard
            .events
            .retain(|(_, ev)| !(ev.stream == stream && ev.occurred_at.as_str() < before_iso));
        Ok(before.saturating_sub(guard.events.len()))
    }

    fn lease_try_acquire(&self, req: &LeaseAcquireReq) -> anyhow::Result<LeaseOutcome> {
        if req.ttl_sec == 0 {
            bail!("租约 ttl_sec 必须大于 0");
        }
        let mut guard = self.write_store()?;
        let now = Utc::now();
        let now_iso = now.to_rfc3339();
        let expires_iso = (now + ChronoDuration::seconds(req.ttl_sec as i64)).to_rfc3339();

        for lease in guard.leases.values() {
            if lease.resource_type == req.resource_type
                && lease.resource_id == req.resource_id
                && lease.expires_at > now_iso
                && lease.holder_id != req.holder_id
                && (req.mode == LeaseMode::Exclusive || lease.mode == LeaseMode::Exclusive)
            {
                return Ok(LeaseOutcome::Busy {
                    holder_id: lease.holder_id.clone(),
                    expires_at: lease.expires_at.clone(),
                });
            }
        }

        let res_key = (req.resource_type.clone(), req.resource_id.clone());
        let next_fence = guard.fence_counters.get(&res_key).copied().unwrap_or(0) + 1;
        guard.fence_counters.insert(res_key, next_fence);

        let lock_id = Uuid::new_v4().to_string();
        let lease = KernelLease {
            lock_id: lock_id.clone(),
            resource_type: req.resource_type.clone(),
            resource_id: req.resource_id.clone(),
            holder_id: req.holder_id.clone(),
            mode: req.mode,
            acquired_at: now_iso,
            expires_at: expires_iso,
            fence_token: next_fence,
        };
        guard.leases.insert(lock_id, lease.clone());
        Ok(LeaseOutcome::Acquired(lease))
    }

    fn lease_release(&self, lock_id: &str) -> anyhow::Result<bool> {
        let mut guard = self.write_store()?;
        Ok(guard.leases.remove(lock_id).is_some())
    }

    fn lease_list(&self, resource_type: Option<&str>) -> anyhow::Result<Vec<KernelLease>> {
        let guard = self.read_store()?;
        let now_iso = Utc::now().to_rfc3339();
        let mut out: Vec<KernelLease> = guard
            .leases
            .values()
            .filter(|l| {
                l.expires_at > now_iso
                    && resource_type
                        .map(|rt| l.resource_type == rt)
                        .unwrap_or(true)
            })
            .cloned()
            .collect();
        out.sort_by_key(|l| l.fence_token);
        Ok(out)
    }

    fn lease_gc_expired(&self) -> anyhow::Result<usize> {
        let mut guard = self.write_store()?;
        let now_iso = Utc::now().to_rfc3339();
        let before = guard.leases.len();
        guard.leases.retain(|_, l| l.expires_at > now_iso);
        Ok(before.saturating_sub(guard.leases.len()))
    }

    fn kv_put(&self, namespace: &str, key: &str, val: &serde_json::Value) -> anyhow::Result<()> {
        let mut guard = self.write_store()?;
        guard
            .kv
            .insert((namespace.to_string(), key.to_string()), val.clone());
        Ok(())
    }

    fn kv_get(&self, namespace: &str, key: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let guard = self.read_store()?;
        Ok(guard
            .kv
            .get(&(namespace.to_string(), key.to_string()))
            .cloned())
    }

    fn exec_scoped_migration(
        &self,
        ext_id: &str,
        table_prefix: &str,
        version: &str,
        sql: &str,
    ) -> anyhow::Result<()> {
        let stmts = ScopedSqlExecutor::validate_scoped_sql(ext_id, table_prefix, sql)?;
        let mut guard = self.write_store()?;
        guard
            .migrations
            .insert((ext_id.to_string(), version.to_string()), stmts);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_am_crud_traversal_and_scoped_migration() {
        let am = MemoryTableAm::new();
        am.init_catalog_and_base_tables().unwrap();

        let mut labels = BTreeMap::new();
        labels.insert("tenant_id".to_string(), "t1".to_string());
        let t = EvidenceTuple {
            evidence_id: "ev-1".to_string(),
            host_id: "h1".to_string(),
            host_labels: labels,
            entity_type: "entity:node".to_string(),
            entity_id: "a".to_string(),
            source_ext: "demo".to_string(),
            evidence_tier: "L1_EXPLICIT".to_string(),
            collected_at: Utc::now().to_rfc3339(),
            ttl_sec: 0,
            payload: serde_json::json!({"ok": true}),
            redacted: false,
            evidence_sha256: String::new(),
        }
        .with_computed_sha256();
        am.entity_upsert(&t).unwrap();

        let scanned = am
            .entity_scan(&EntityScanFilter::default(), None)
            .unwrap();
        assert_eq!(scanned.len(), 1);
        assert_eq!(am.entity_delete_by_source("demo", "a").unwrap(), 1);

        am.exec_scoped_migration(
            "mem_ext",
            "ext_mem_ext__",
            "0.1.0",
            "CREATE TABLE ext_mem_ext__state (k TEXT PRIMARY KEY);",
        )
        .unwrap();
        assert_eq!(am.applied_migrations().unwrap().len(), 1);
    }
}
