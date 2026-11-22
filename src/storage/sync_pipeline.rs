use std::sync::Arc;
use std::time::Instant;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use crate::storage::duckdb_provider::DuckDbStorageProvider;
use crate::storage::spi::StorageProvider;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncReport {
    pub assets_synced: usize,
    pub changes_synced: usize,
    pub memories_synced: usize,
    pub fact_nodes_synced: usize,
    pub fact_edges_synced: usize,
    #[serde(default)]
    pub links_synced: usize,
    #[serde(default)]
    pub active_processes_synced: usize,
    #[serde(default)]
    pub body_checkpoints_synced: usize,
    #[serde(default)]
    pub audit_events_synced: usize,
    #[serde(default)]
    pub effectiveness_ledger_synced: usize,
    pub duration_ms: u128,
}

/// 主存储 (SQLite / PG) → DuckDB 分析副本的同步管道
pub struct SyncPipeline {
    source: Arc<dyn StorageProvider>,
    target: Arc<DuckDbStorageProvider>,
}

impl SyncPipeline {
    pub fn new(source: Arc<dyn StorageProvider>, target: Arc<DuckDbStorageProvider>) -> Self {
        Self { source, target }
    }

    /// 执行全量同步（使用 DuckDB 原生 Appender + 单事务批量装载，保留真实历史 created_at 并更新水位线）
    pub fn sync_all(&self) -> Result<SyncReport> {
        use std::collections::HashSet;
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::LockDuckDbSync);
        let start = Instant::now();
        let now_str = chrono::Utc::now().to_rfc3339();

        let assets = self.source.list_assets(500_000)?;
        let asset_ts = self.source.list_created_at_map("assets").unwrap_or_default();
        let changes = self.source.list_changes(500_000)?;
        let memories = self.source.list_all_memories(500_000)?;
        let mem_ts = self.source.list_created_at_map("memories").unwrap_or_default();
        let nodes = self.source.list_all_fact_nodes(500_000)?;
        let edges = self.source.list_all_fact_edges(500_000)?;
        let links = self.source.list_all_links(500_000).unwrap_or_default();
        let procs = self.source.list_active_processes().unwrap_or_default();
        let checkpoints = self
            .source
            .list_body_checkpoints(None, 10_000)
            .unwrap_or_default();
        let audits = self
            .source
            .query_audit_events(&crate::storage::models::AuditQueryFilter {
                limit: 100_000,
                ..Default::default()
            })
            .unwrap_or_default();
        let ledger_events = self
            .source
            .query_effectiveness_ledger("all", None, 100_000)
            .map(|s| s.recent_events)
            .unwrap_or_default();

        let conn_arc = self.target.conn();
        let conn = conn_arc
            .lock()
            .map_err(|e| anyhow::anyhow!("DuckDB 连接锁异常: {}", e))?;

        conn.execute_batch(
            "BEGIN TRANSACTION;
             DELETE FROM assets;
             DELETE FROM changes;
             DELETE FROM memories;
             DELETE FROM fact_nodes;
             DELETE FROM fact_edges;
             DELETE FROM links;
             DELETE FROM active_processes;
             DELETE FROM body_checkpoints;
             DELETE FROM audit_events;
             DELETE FROM effectiveness_ledger;",
        )?;

        struct SyncCounts {
            assets: usize,
            changes: usize,
            memories: usize,
            nodes: usize,
            edges: usize,
            links: usize,
            procs: usize,
            checkpoints: usize,
            audits: usize,
            ledger: usize,
        }

        let write_res: Result<SyncCounts> = (|| {
            // 1. assets (18 cols)
            let mut assets_synced = 0usize;
            {
                let mut seen_ids = HashSet::with_capacity(assets.len());
                let mut seen_paths = HashSet::with_capacity(assets.len());
                let mut app = conn.appender("assets")?;
                for a in &assets {
                    if !seen_ids.insert(a.id.as_str()) || !seen_paths.insert(a.path.as_str()) {
                        continue;
                    }
                    let desc_kv = serde_json::to_string(&a.desc_kv).unwrap_or_else(|_| "{}".into());
                    let tags = serde_json::to_string(&a.tags).unwrap_or_else(|_| "[]".into());
                    let created_ts = asset_ts.get(&a.id).map(String::as_str).unwrap_or(&now_str);
                    app.append_row(duckdb::params![
                        a.id,
                        a.path,
                        a.asset_type,
                        a.owner,
                        a.security_level,
                        a.scan_tier,
                        a.description,
                        desc_kv,
                        a.usage,
                        a.size_bytes,
                        a.file_count,
                        tags,
                        a.status,
                        "[]",
                        now_str,
                        now_str,
                        created_ts,
                        now_str
                    ])?;
                    assets_synced += 1;
                }
                app.flush()?;
            }

            // 2. changes (7 cols)
            let mut changes_synced = 0usize;
            {
                let mut seen_ids = HashSet::with_capacity(changes.len());
                let mut next_id: i64 = 1;
                let mut app = conn.appender("changes")?;
                for c in &changes {
                    let cid = match c.id {
                        Some(id) if seen_ids.insert(id) => {
                            if id >= next_id {
                                next_id = id + 1;
                            }
                            id
                        }
                        _ => {
                            while !seen_ids.insert(next_id) {
                                next_id += 1;
                            }
                            let id = next_id;
                            next_id += 1;
                            id
                        }
                    };
                    let details = serde_json::to_string(&c.details).unwrap_or_else(|_| "{}".into());
                    let created_at = c.created_at.as_deref().unwrap_or(&now_str);
                    app.append_row(duckdb::params![
                        cid,
                        c.change_type,
                        c.target,
                        c.description,
                        details,
                        c.source,
                        created_at
                    ])?;
                    changes_synced += 1;
                }
                app.flush()?;
            }

            // 3. memories (16 cols: 第 11 列保留真实 embedding，第 12 列保留真实 created_at，第 16 列 sensitive)
            let mut memories_synced = 0usize;
            {
                let mut seen_ids = HashSet::with_capacity(memories.len());
                let mut app = conn.appender("memories")?;
                for m in &memories {
                    if !seen_ids.insert(m.id.as_str()) {
                        continue;
                    }
                    let tags = serde_json::to_string(&m.tags).unwrap_or_else(|_| "[]".into());
                    let emb = serde_json::to_string(&m.embedding).unwrap_or_else(|_| "[]".into());
                    let created_ts = mem_ts.get(&m.id).map(String::as_str).unwrap_or(&now_str);
                    app.append_row(duckdb::params![
                        m.id,
                        m.memory_type,
                        m.content,
                        m.summary,
                        m.source_agent,
                        m.source_session,
                        tags,
                        m.weight,
                        m.pinned,
                        m.status,
                        emb,
                        created_ts,
                        now_str,
                        now_str,
                        0i32,
                        false
                    ])?;
                    memories_synced += 1;
                }
                app.flush()?;
            }

            // 4. fact_nodes (7 cols)
            let mut nodes_synced = 0usize;
            {
                let mut seen_ids = HashSet::with_capacity(nodes.len());
                let mut app = conn.appender("fact_nodes")?;
                for n in &nodes {
                    if !seen_ids.insert(n.id.as_str()) {
                        continue;
                    }
                    let props = serde_json::to_string(&n.properties).unwrap_or_else(|_| "{}".into());
                    let updated = n.updated_at.as_deref().unwrap_or(&now_str);
                    app.append_row(duckdb::params![
                        n.id,
                        n.node_type,
                        n.name,
                        n.label,
                        props,
                        "[]",
                        updated
                    ])?;
                    nodes_synced += 1;
                }
                app.flush()?;
            }

            // 5. fact_edges (10 cols)
            let mut edges_synced = 0usize;
            {
                let mut seen_ids = HashSet::with_capacity(edges.len());
                let mut next_id: i64 = 1;
                let mut app = conn.appender("fact_edges")?;
                for e in &edges {
                    let eid = match e.id {
                        Some(id) if seen_ids.insert(id) => {
                            if id >= next_id {
                                next_id = id + 1;
                            }
                            id
                        }
                        _ => {
                            while !seen_ids.insert(next_id) {
                                next_id += 1;
                            }
                            let id = next_id;
                            next_id += 1;
                            id
                        }
                    };
                    let props = serde_json::to_string(&e.properties).unwrap_or_else(|_| "{}".into());
                    let updated = e.updated_at.as_deref().unwrap_or(&now_str);
                    app.append_row(duckdb::params![
                        eid,
                        e.source_id,
                        e.target_id,
                        e.relation,
                        props,
                        e.confidence,
                        e.evidence_tier,
                        e.weight,
                        e.direction_kind,
                        updated
                    ])?;
                    edges_synced += 1;
                }
                app.flush()?;
            }

            // 6. links (12 cols)
            let mut links_synced = 0usize;
            {
                let mut seen_ids = HashSet::with_capacity(links.len());
                let mut next_id: i64 = 1;
                let mut app = conn.appender("links")?;
                for l in &links {
                    let lid = match l.id {
                        Some(id) if seen_ids.insert(id) => {
                            if id >= next_id {
                                next_id = id + 1;
                            }
                            id
                        }
                        _ => {
                            while !seen_ids.insert(next_id) {
                                next_id += 1;
                            }
                            let id = next_id;
                            next_id += 1;
                            id
                        }
                    };
                    let prov = l.provenance.as_deref().unwrap_or("static_scan");
                    let created_at = l.created_at.as_deref().unwrap_or(&now_str);
                    app.append_row(duckdb::params![
                        lid,
                        l.source_type,
                        l.source_id,
                        l.target_type,
                        l.target_id,
                        l.relation,
                        l.confidence,
                        l.auto_discovered,
                        l.line_number,
                        l.snippet,
                        prov,
                        created_at
                    ])?;
                    links_synced += 1;
                }
                app.flush()?;
            }

            // 7. active_processes (13 cols)
            let mut active_processes_synced = 0usize;
            {
                let mut seen_ids = HashSet::with_capacity(procs.len());
                let mut seen_pids = HashSet::with_capacity(procs.len());
                let mut app = conn.appender("active_processes")?;
                for p in &procs {
                    if !seen_ids.insert(p.id.as_str()) || !seen_pids.insert(p.pid) {
                        continue;
                    }
                    let ports = serde_json::to_string(&p.listening_ports).unwrap_or_else(|_| "[]".into());
                    app.append_row(duckdb::params![
                        p.id,
                        p.pid as i32,
                        p.name,
                        p.command,
                        p.owner,
                        p.associated_path,
                        ports,
                        p.status,
                        p.cpu_pct,
                        p.mem_bytes as i64,
                        p.gpu_allocated_mb as i32,
                        p.started_at,
                        now_str
                    ])?;
                    active_processes_synced += 1;
                }
                app.flush()?;
            }

            // 8. body_checkpoints (16 cols)
            let mut body_checkpoints_synced = 0usize;
            {
                let mut seen_ids = HashSet::with_capacity(checkpoints.len());
                let mut app = conn.appender("body_checkpoints")?;
                for cp in &checkpoints {
                    if !seen_ids.insert(cp.checkpoint_id.as_str()) {
                        continue;
                    }
                    let active_files = serde_json::to_string(&cp.active_files).unwrap_or_else(|_| "[]".into());
                    let active_locks = serde_json::to_string(&cp.active_locks).unwrap_or_else(|_| "[]".into());
                    let verified_facts = serde_json::to_string(&cp.verified_facts).unwrap_or_else(|_| "[]".into());
                    let open_blockers = serde_json::to_string(&cp.open_blockers).unwrap_or_else(|_| "[]".into());
                    app.append_row(duckdb::params![
                        cp.checkpoint_id,
                        cp.body_id,
                        cp.from_brain,
                        cp.session_id,
                        cp.goal_summary,
                        active_files,
                        active_locks,
                        verified_facts,
                        open_blockers,
                        cp.next_step_hint,
                        cp.token_budget as i32,
                        cp.status,
                        cp.evidence_sha256,
                        cp.created_at,
                        cp.resumed_by_brain,
                        cp.resumed_at
                    ])?;
                    body_checkpoints_synced += 1;
                }
                app.flush()?;
            }

            // 9. audit_events (17 cols)
            let mut audit_events_synced = 0usize;
            {
                let mut seen_ids = HashSet::with_capacity(audits.len());
                let mut app = conn.appender("audit_events")?;
                for aud in &audits {
                    if !seen_ids.insert(aud.event_id.as_str()) {
                        continue;
                    }
                    let delta = serde_json::to_string(&aud.before_after_delta).unwrap_or_else(|_| "{}".into());
                    app.append_row(duckdb::params![
                        aud.event_id,
                        aud.trace_id,
                        aud.session_id,
                        aud.domain,
                        aud.actor_kind,
                        aud.actor_id,
                        aud.source_channel,
                        aud.category,
                        aud.action,
                        aud.target_type,
                        aud.target_id,
                        aud.concurrency_mode,
                        aud.outcome,
                        delta,
                        aud.elapsed_us,
                        aud.evidence_sha256,
                        aud.created_at
                    ])?;
                    audit_events_synced += 1;
                }
                app.flush()?;
            }

            // 10. effectiveness_ledger (16 cols)
            let mut effectiveness_ledger_synced = 0usize;
            {
                let mut seen_ids = HashSet::with_capacity(ledger_events.len());
                let mut app = conn.appender("effectiveness_ledger")?;
                for ev in &ledger_events {
                    if !seen_ids.insert(ev.event_id.as_str()) {
                        continue;
                    }
                    app.append_row(duckdb::params![
                        ev.event_id,
                        ev.session_id,
                        ev.source_channel,
                        ev.tool_or_action,
                        ev.provenance_rule,
                        ev.baseline_tokens,
                        ev.steward_tokens,
                        ev.tokens_saved,
                        ev.baseline_rounds,
                        ev.steward_rounds,
                        ev.rounds_saved,
                        ev.hazard_intercepted,
                        ev.noise_blocked,
                        ev.elapsed_us,
                        ev.evidence_summary,
                        ev.created_at
                    ])?;
                    effectiveness_ledger_synced += 1;
                }
                app.flush()?;
            }

            // 11. 写入 sync_watermarks 水位线追踪记录
            let watermarks = [
                ("assets", assets_synced),
                ("changes", changes_synced),
                ("memories", memories_synced),
                ("fact_nodes", nodes_synced),
                ("fact_edges", edges_synced),
                ("links", links_synced),
                ("active_processes", active_processes_synced),
                ("body_checkpoints", body_checkpoints_synced),
                ("audit_events", audit_events_synced),
                ("effectiveness_ledger", effectiveness_ledger_synced),
            ];
            for (table_name, rows) in watermarks {
                conn.execute(
                    "INSERT INTO sync_watermarks (table_name, last_sync_ts, synced_rows, updated_at)
                     VALUES (?1, ?2, ?3, ?2)
                     ON CONFLICT(table_name) DO UPDATE SET
                        last_sync_ts = excluded.last_sync_ts,
                        synced_rows = excluded.synced_rows,
                        updated_at = excluded.updated_at",
                    duckdb::params![table_name, now_str, rows as i64],
                )?;
            }

            Ok(SyncCounts {
                assets: assets_synced,
                changes: changes_synced,
                memories: memories_synced,
                nodes: nodes_synced,
                edges: edges_synced,
                links: links_synced,
                procs: active_processes_synced,
                checkpoints: body_checkpoints_synced,
                audits: audit_events_synced,
                ledger: effectiveness_ledger_synced,
            })
        })();

        let counts = match write_res {
            Ok(counts) => {
                conn.execute_batch("COMMIT;")?;
                counts
            }
            Err(e) => {
                if let Err(rb_err) = conn.execute_batch("ROLLBACK;") {
                    eprintln!("[SyncPipeline][WARN] sync_all ROLLBACK 失败: {}", rb_err);
                }
                return Err(e);
            }
        };

        let duration_ms = start.elapsed().as_millis();
        Ok(SyncReport {
            assets_synced: counts.assets,
            changes_synced: counts.changes,
            memories_synced: counts.memories,
            fact_nodes_synced: counts.nodes,
            fact_edges_synced: counts.edges,
            links_synced: counts.links,
            active_processes_synced: counts.procs,
            body_checkpoints_synced: counts.checkpoints,
            audit_events_synced: counts.audits,
            effectiveness_ledger_synced: counts.ledger,
            duration_ms,
        })
    }

    /// 轻量单表同步：仅通过 DuckDB Appender 装载双域审计表（< 20ms）
    pub fn sync_audit_events_only(&self) -> Result<usize> {
        use std::collections::HashSet;
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::LockDuckDbSync);
        let audits = self
            .source
            .query_audit_events(&crate::storage::models::AuditQueryFilter {
                limit: 100_000,
                ..Default::default()
            })
            .unwrap_or_default();
        let conn_arc = self.target.conn();
        let conn = conn_arc
            .lock()
            .map_err(|e| anyhow::anyhow!("DuckDB 连接锁异常: {}", e))?;
        conn.execute_batch("BEGIN TRANSACTION; DELETE FROM audit_events;")?;
        let mut synced = 0usize;
        let res: Result<()> = (|| {
            let mut seen_ids = HashSet::with_capacity(audits.len());
            let mut app = conn.appender("audit_events")?;
            for aud in &audits {
                if !seen_ids.insert(aud.event_id.as_str()) {
                    continue;
                }
                let delta = serde_json::to_string(&aud.before_after_delta).unwrap_or_else(|_| "{}".into());
                app.append_row(duckdb::params![
                    aud.event_id,
                    aud.trace_id,
                    aud.session_id,
                    aud.domain,
                    aud.actor_kind,
                    aud.actor_id,
                    aud.source_channel,
                    aud.category,
                    aud.action,
                    aud.target_type,
                    aud.target_id,
                    aud.concurrency_mode,
                    aud.outcome,
                    delta,
                    aud.elapsed_us,
                    aud.evidence_sha256,
                    aud.created_at
                ])?;
                synced += 1;
            }
            app.flush()?;
            let now_str = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO sync_watermarks (table_name, last_sync_ts, synced_rows, updated_at)
                 VALUES ('audit_events', ?1, ?2, ?1)
                 ON CONFLICT(table_name) DO UPDATE SET
                    last_sync_ts = excluded.last_sync_ts,
                    synced_rows = excluded.synced_rows,
                    updated_at = excluded.updated_at",
                duckdb::params![now_str, synced as i64],
            )?;
            Ok(())
        })();
        match res {
            Ok(()) => {
                conn.execute_batch("COMMIT;")?;
                Ok(synced)
            }
            Err(e) => {
                if let Err(rb_err) = conn.execute_batch("ROLLBACK;") {
                    eprintln!("[SyncPipeline][WARN] sync_audit_events_only ROLLBACK 失败: {}", rb_err);
                }
                Err(e)
            }
        }
    }
}
