use std::path::Path;
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use crate::storage::models::*;
use crate::storage::spi::StorageProvider;

pub const SCHEMA_SQL: &str = include_str!("schema.sql");

#[derive(Clone)]
pub struct SqliteStorageProvider {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteStorageProvider {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建数据库目录失败: {:?}", parent))?;
            crate::config::restrict_private_permissions(parent, true);
        }
        let conn = Connection::open(path)
            .with_context(|| format!("打开 SQLite 数据库失败: {:?}", path))?;
        crate::config::restrict_private_permissions(path, false);
        let provider = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        provider.init()?;
        Ok(provider)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("打开 SQLite 内存数据库失败")?;
        let provider = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        provider.init()?;
        Ok(provider)
    }

    pub fn conn(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| anyhow::anyhow!("SQLite 连接锁异常 (mutex poisoned): {}", e))
    }
}

impl StorageProvider for SqliteStorageProvider {
    fn init(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute_batch(SCHEMA_SQL)
            .context("执行内置 SQLite Schema 批处理失败")?;
        // 增量热迁移兼容已存在数据库
        let _ = conn.execute("ALTER TABLE memories ADD COLUMN embedding TEXT DEFAULT '[]'", []);
        let _ = conn.execute("ALTER TABLE assets ADD COLUMN embedding TEXT DEFAULT '[]'", []);
        let _ = conn.execute("ALTER TABLE fact_nodes ADD COLUMN embedding TEXT DEFAULT '[]'", []);
        let _ = conn.execute("ALTER TABLE links ADD COLUMN line_number INTEGER DEFAULT NULL", []);
        let _ = conn.execute("ALTER TABLE links ADD COLUMN snippet TEXT DEFAULT NULL", []);
        let _ = conn.execute("ALTER TABLE links ADD COLUMN provenance TEXT DEFAULT 'static_scan'", []);
        let _ = conn.execute("CREATE TABLE IF NOT EXISTS scan_jobs (
            id              TEXT PRIMARY KEY,
            root_path       TEXT NOT NULL,
            status          TEXT NOT NULL CHECK (status IN ('pending','running','paused','completed','failed')),
            last_path       TEXT,
            scanned_dirs    INTEGER DEFAULT 0,
            scanned_files   INTEGER DEFAULT 0,
            total_assets    INTEGER DEFAULT 0,
            shallow_blocked INTEGER DEFAULT 0,
            error_message   TEXT,
            started_at      TEXT,
            paused_at       TEXT,
            completed_at    TEXT,
            created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
            updated_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
        )", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_scan_jobs_status ON scan_jobs(status)", []);

        let _ = conn.execute("CREATE TABLE IF NOT EXISTS fact_nodes (
            id          TEXT PRIMARY KEY,
            node_type   TEXT NOT NULL CHECK (node_type IN ('project','tool','port','service','cron','file','authoritative_site')),
            name        TEXT NOT NULL,
            label       TEXT NOT NULL,
            properties  TEXT DEFAULT '{}',
            updated_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
        )", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_fact_nodes_type ON fact_nodes(node_type)", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_fact_nodes_name ON fact_nodes(name)", []);

        let _ = conn.execute("CREATE TABLE IF NOT EXISTS fact_edges (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id   TEXT NOT NULL,
            target_id   TEXT NOT NULL,
            relation    TEXT NOT NULL CHECK (relation IN ('depends_on','listens_on','scheduled_by','reads','writes','part_of','calls','contains')),
            properties  TEXT DEFAULT '{}',
            confidence  REAL DEFAULT 1.0,
            updated_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
            UNIQUE(source_id, target_id, relation),
            FOREIGN KEY(source_id) REFERENCES fact_nodes(id) ON DELETE CASCADE,
            FOREIGN KEY(target_id) REFERENCES fact_nodes(id) ON DELETE CASCADE
        )", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_fact_edges_source ON fact_edges(source_id)", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_fact_edges_target ON fact_edges(target_id)", []);
        let _ = conn.execute("CREATE INDEX IF NOT EXISTS idx_fact_edges_relation ON fact_edges(relation)", []);
        Ok(())
    }

    fn provider_name(&self) -> &'static str {
        "sqlite"
    }

    fn get_placement_rules(&self) -> Result<Vec<PlacementRule>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority 
             FROM placement_rules ORDER BY priority ASC"
        )?;

        let rules = stmt.query_map([], |row| {
            Ok(PlacementRule {
                id: row.get(0)?,
                rule_name: row.get(1)?,
                pattern_type: row.get(2)?,
                target_base_dir: row.get(3)?,
                naming_pattern: row.get(4)?,
                description: row.get(5)?,
                guideline: row.get(6)?,
                priority: row.get(7)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rules)
    }

    fn insert_placement_rule(&self, rule: &PlacementRule) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO placement_rules (id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(rule_name) DO UPDATE SET
                pattern_type = excluded.pattern_type,
                target_base_dir = excluded.target_base_dir,
                naming_pattern = excluded.naming_pattern,
                description = excluded.description,
                guideline = excluded.guideline,
                priority = excluded.priority",
            params![
                rule.id,
                rule.rule_name,
                rule.pattern_type,
                rule.target_base_dir,
                rule.naming_pattern,
                rule.description,
                rule.guideline,
                rule.priority,
            ],
        )?;
        Ok(())
    }

    fn get_scan_policies(&self) -> Result<Vec<ScanPolicy>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes 
             FROM scan_policies"
        )?;

        let policies = stmt.query_map([], |row| {
            Ok(ScanPolicy {
                id: row.get(0)?,
                path_pattern: row.get(1)?,
                tier: row.get(2)?,
                max_depth: row.get(3)?,
                enable_fts: row.get::<_, i32>(4)? != 0,
                enable_vec: row.get::<_, i32>(5)? != 0,
                enable_graph: row.get::<_, i32>(6)? != 0,
                notes: row.get(7)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(policies)
    }

    fn upsert_asset(&self, asset: &AssetRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let desc_kv_str = serde_json::to_string(&asset.desc_kv)?;
        let tags_str = serde_json::to_string(&asset.tags)?;
        let invalidate_embedding: i32 = if asset
            .desc_kv
            .get("embedding_status")
            .and_then(|v| v.as_str())
            == Some("stale")
            || asset.status == "missing"
        {
            1
        } else {
            0
        };

        conn.execute(
            "INSERT INTO assets (id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
             ON CONFLICT(path) DO UPDATE SET
                type = excluded.type,
                owner = excluded.owner,
                security_level = excluded.security_level,
                scan_tier = excluded.scan_tier,
                description = excluded.description,
                desc_kv = excluded.desc_kv,
                usage = excluded.usage,
                size_bytes = excluded.size_bytes,
                file_count = excluded.file_count,
                tags = excluded.tags,
                status = excluded.status,
                embedding = CASE WHEN ?14 != 0 THEN '[]' ELSE assets.embedding END,
                updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now')",
            params![
                asset.id,
                asset.path,
                asset.asset_type,
                asset.owner,
                asset.security_level,
                asset.scan_tier,
                asset.description,
                desc_kv_str,
                asset.usage,
                asset.size_bytes,
                asset.file_count,
                tags_str,
                asset.status,
                invalidate_embedding,
            ],
        )?;
        Ok(())
    }

    fn get_asset_by_path(&self, path: &str) -> Result<Option<AssetRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets WHERE path = ?1"
        )?;

        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            let desc_kv_str: String = row.get(7)?;
            let tags_str: String = row.get(11)?;
            let desc_kv: serde_json::Value = serde_json::from_str(&desc_kv_str).unwrap_or(serde_json::json!({}));
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

            Ok(Some(AssetRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                asset_type: row.get(2)?,
                owner: row.get(3)?,
                security_level: row.get(4)?,
                scan_tier: row.get(5)?,
                description: row.get(6)?,
                desc_kv,
                usage: row.get(8)?,
                size_bytes: row.get(9)?,
                file_count: row.get(10)?,
                tags,
                status: row.get(12)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn search_assets_fts(&self, query: &str, limit: usize) -> Result<Vec<AssetRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT a.id, a.path, a.type, a.owner, a.security_level, a.scan_tier, a.description, a.desc_kv, a.usage, a.size_bytes, a.file_count, a.tags, a.status
             FROM assets_fts f
             JOIN assets a ON f.rowid = a.rowid
             WHERE assets_fts MATCH ?1 AND a.status = 'active'
             ORDER BY rank
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![query, limit as i64], |row| {
            let desc_kv_str: String = row.get(7)?;
            let tags_str: String = row.get(11)?;
            let desc_kv: serde_json::Value = serde_json::from_str(&desc_kv_str).unwrap_or(serde_json::json!({}));
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

            Ok(AssetRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                asset_type: row.get(2)?,
                owner: row.get(3)?,
                security_level: row.get(4)?,
                scan_tier: row.get(5)?,
                description: row.get(6)?,
                desc_kv,
                usage: row.get(8)?,
                size_bytes: row.get(9)?,
                file_count: row.get(10)?,
                tags,
                status: row.get(12)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn list_assets(&self, limit: usize) -> Result<Vec<AssetRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets
             ORDER BY updated_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let desc_kv_str: String = row.get(7)?;
            let tags_str: String = row.get(11)?;
            let desc_kv: serde_json::Value = serde_json::from_str(&desc_kv_str).unwrap_or(serde_json::json!({}));
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

            Ok(AssetRecord {
                id: row.get(0)?,
                path: row.get(1)?,
                asset_type: row.get(2)?,
                owner: row.get(3)?,
                security_level: row.get(4)?,
                scan_tier: row.get(5)?,
                description: row.get(6)?,
                desc_kv,
                usage: row.get(8)?,
                size_bytes: row.get(9)?,
                file_count: row.get(10)?,
                tags,
                status: row.get(12)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn replace_active_processes(&self, procs: &[ActiveProcessRecord]) -> Result<()> {
        let mut conn = self.lock_conn()?;
        let tx = conn.transaction()?;

        tx.execute("DELETE FROM active_processes", [])?;

        {
            let mut stmt = tx.prepare(
                "INSERT INTO active_processes (id, pid, name, command, owner, associated_path, listening_ports, status, cpu_pct, mem_bytes, gpu_allocated_mb, started_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)"
            )?;

            for p in procs {
                let ports_str = serde_json::to_string(&p.listening_ports)?;
                stmt.execute(params![
                    p.id,
                    p.pid,
                    p.name,
                    p.command,
                    p.owner,
                    p.associated_path,
                    ports_str,
                    p.status,
                    p.cpu_pct,
                    p.mem_bytes as i64,
                    p.gpu_allocated_mb,
                    p.started_at,
                ])?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    fn list_active_processes(&self) -> Result<Vec<ActiveProcessRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, pid, name, command, owner, associated_path, listening_ports, status, cpu_pct, mem_bytes, gpu_allocated_mb, started_at
             FROM active_processes ORDER BY cpu_pct DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            let ports_str: String = row.get(6)?;
            let listening_ports: Vec<u16> = serde_json::from_str(&ports_str).unwrap_or_default();
            let mem_bytes_i64: i64 = row.get(9)?;

            Ok(ActiveProcessRecord {
                id: row.get(0)?,
                pid: row.get(1)?,
                name: row.get(2)?,
                command: row.get(3)?,
                owner: row.get(4)?,
                associated_path: row.get(5)?,
                listening_ports,
                status: row.get(7)?,
                cpu_pct: row.get(8)?,
                mem_bytes: mem_bytes_i64 as u64,
                gpu_allocated_mb: row.get(10)?,
                started_at: row.get(11)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn insert_memory(&self, mem: &MemoryRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let tags_str = serde_json::to_string(&mem.tags)?;
        let emb_str = serde_json::to_string(&mem.embedding)?;

        conn.execute(
            "INSERT INTO memories (id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                mem.id,
                mem.memory_type,
                mem.content,
                mem.summary,
                mem.source_agent,
                mem.source_session,
                tags_str,
                mem.weight,
                mem.pinned,
                mem.status,
                emb_str,
            ],
        )?;
        Ok(())
    }

    fn search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT m.id, m.type, m.content, m.summary, m.source_agent, m.source_session, m.tags, m.weight, m.pinned, m.status, m.embedding
             FROM memories_fts f
             JOIN memories m ON f.rowid = m.rowid
             WHERE memories_fts MATCH ?1
             ORDER BY rank
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![query, limit as i64], |row| {
            let tags_str: String = row.get(6)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let emb_str: String = row.get(10)?;
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();

            Ok(MemoryRecord {
                id: row.get(0)?,
                memory_type: row.get(1)?,
                content: row.get(2)?,
                summary: row.get(3)?,
                source_agent: row.get(4)?,
                source_session: row.get(5)?,
                tags,
                weight: row.get(7)?,
                pinned: row.get(8)?,
                status: row.get(9)?,
                embedding,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn list_all_active_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             WHERE status = 'active'
             ORDER BY updated_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let tags_str: String = row.get(6)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let emb_str: String = row.get(10)?;
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();

            Ok(MemoryRecord {
                id: row.get(0)?,
                memory_type: row.get(1)?,
                content: row.get(2)?,
                summary: row.get(3)?,
                source_agent: row.get(4)?,
                source_session: row.get(5)?,
                tags,
                weight: row.get(7)?,
                pinned: row.get(8)?,
                status: row.get(9)?,
                embedding,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn update_memory_status(&self, memory_id: &str, status: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let rows = conn.execute(
            "UPDATE memories SET status = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now') WHERE id = ?1",
            params![memory_id, status],
        )?;
        Ok(rows)
    }

    fn record_change(&self, change: &ChangeRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let details_str = serde_json::to_string(&change.details)?;
        conn.execute(
            "INSERT INTO changes (type, target, description, details, source)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                change.change_type,
                change.target,
                change.description,
                details_str,
                change.source,
            ],
        )?;
        Ok(())
    }

    fn list_changes(&self, limit: usize) -> Result<Vec<ChangeRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, type, target, description, details, source, created_at
             FROM changes ORDER BY created_at DESC LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let details_str: String = row.get(4)?;
            let details: serde_json::Value = serde_json::from_str(&details_str).unwrap_or(serde_json::json!({}));
            Ok(ChangeRecord {
                id: Some(row.get(0)?),
                change_type: row.get(1)?,
                target: row.get(2)?,
                description: row.get(3)?,
                details,
                source: row.get(5)?,
                created_at: Some(row.get(6)?),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn add_link(&self, link: &LinkRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO links (source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(source_type, source_id, target_type, target_id, relation) DO UPDATE SET
                confidence = excluded.confidence,
                line_number = excluded.line_number,
                snippet = excluded.snippet,
                provenance = excluded.provenance",
            params![
                link.source_type,
                link.source_id,
                link.target_type,
                link.target_id,
                link.relation,
                link.confidence,
                link.auto_discovered as i32,
                link.line_number,
                link.snippet,
                link.provenance.as_deref().unwrap_or("static_scan"),
            ],
        )?;
        Ok(())
    }

    fn delete_link(&self, id: i64) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM links WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn delete_links_for_target(&self, target_id: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count = conn.execute("DELETE FROM links WHERE target_id = ?1", params![target_id])?;
        Ok(count)
    }

    fn delete_auto_links_for_source(&self, source_id: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count = conn.execute(
            "DELETE FROM links WHERE source_id = ?1 AND auto_discovered = 1",
            params![source_id],
        )?;
        Ok(count)
    }

    fn update_link_target(&self, source_id: &str, old_target: &str, new_target: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count = conn.execute(
            "UPDATE links SET target_id = ?3 WHERE source_id = ?1 AND target_id = ?2",
            params![source_id, old_target, new_target],
        )?;
        Ok(count)
    }

    fn query_links_for_node(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             WHERE (source_type = ?1 AND source_id = ?2) OR (target_type = ?1 AND target_id = ?2)
             ORDER BY created_at DESC"
        )?;

        let rows = stmt.query_map(params![node_type, node_id], |row| {
            let auto_disc: i32 = row.get(7)?;
            Ok(LinkRecord {
                id: Some(row.get(0)?),
                source_type: row.get(1)?,
                source_id: row.get(2)?,
                target_type: row.get(3)?,
                target_id: row.get(4)?,
                relation: row.get(5)?,
                confidence: row.get(6)?,
                auto_discovered: auto_disc != 0,
                line_number: row.get(8)?,
                snippet: row.get(9)?,
                provenance: row.get(10)?,
                created_at: Some(row.get(11)?),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn list_all_links(&self, limit: usize) -> Result<Vec<LinkRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             ORDER BY created_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let auto_disc: i32 = row.get(7)?;
            Ok(LinkRecord {
                id: Some(row.get(0)?),
                source_type: row.get(1)?,
                source_id: row.get(2)?,
                target_type: row.get(3)?,
                target_id: row.get(4)?,
                relation: row.get(5)?,
                confidence: row.get(6)?,
                auto_discovered: auto_disc != 0,
                line_number: row.get(8)?,
                snippet: row.get(9)?,
                provenance: row.get(10)?,
                created_at: Some(row.get(11)?),
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn create_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO scan_jobs (id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'), strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))",
            params![
                job.id,
                job.root_path,
                job.status,
                job.last_path,
                job.scanned_dirs,
                job.scanned_files,
                job.total_assets,
                job.shallow_blocked,
                job.error_message,
                job.started_at,
                job.paused_at,
                job.completed_at,
            ],
        )?;
        Ok(())
    }

    fn update_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "UPDATE scan_jobs SET
                status = ?2,
                last_path = ?3,
                scanned_dirs = ?4,
                scanned_files = ?5,
                total_assets = ?6,
                shallow_blocked = ?7,
                error_message = ?8,
                started_at = ?9,
                paused_at = ?10,
                completed_at = ?11,
                updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now')
             WHERE id = ?1",
            params![
                job.id,
                job.status,
                job.last_path,
                job.scanned_dirs,
                job.scanned_files,
                job.total_assets,
                job.shallow_blocked,
                job.error_message,
                job.started_at,
                job.paused_at,
                job.completed_at,
            ],
        )?;
        Ok(())
    }

    fn get_scan_job(&self, id: &str) -> Result<Option<ScanJobRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs WHERE id = ?1"
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ScanJobRecord {
                id: row.get(0)?,
                root_path: row.get(1)?,
                status: row.get(2)?,
                last_path: row.get(3)?,
                scanned_dirs: row.get(4)?,
                scanned_files: row.get(5)?,
                total_assets: row.get(6)?,
                shallow_blocked: row.get(7)?,
                error_message: row.get(8)?,
                started_at: row.get(9)?,
                paused_at: row.get(10)?,
                completed_at: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn list_scan_jobs(&self, limit: usize) -> Result<Vec<ScanJobRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs ORDER BY created_at DESC LIMIT ?1"
        )?;

        let jobs = stmt.query_map(params![limit as i64], |row| {
            Ok(ScanJobRecord {
                id: row.get(0)?,
                root_path: row.get(1)?,
                status: row.get(2)?,
                last_path: row.get(3)?,
                scanned_dirs: row.get(4)?,
                scanned_files: row.get(5)?,
                total_assets: row.get(6)?,
                shallow_blocked: row.get(7)?,
                error_message: row.get(8)?,
                started_at: row.get(9)?,
                paused_at: row.get(10)?,
                completed_at: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(jobs)
    }

    fn get_active_scan_job(&self) -> Result<Option<ScanJobRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs WHERE status IN ('pending', 'running', 'paused') ORDER BY updated_at DESC LIMIT 1"
        )?;

        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? {
            Ok(Some(ScanJobRecord {
                id: row.get(0)?,
                root_path: row.get(1)?,
                status: row.get(2)?,
                last_path: row.get(3)?,
                scanned_dirs: row.get(4)?,
                scanned_files: row.get(5)?,
                total_assets: row.get(6)?,
                shallow_blocked: row.get(7)?,
                error_message: row.get(8)?,
                started_at: row.get(9)?,
                paused_at: row.get(10)?,
                completed_at: row.get(11)?,
                created_at: row.get(12)?,
                updated_at: row.get(13)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn upsert_fact_node(&self, node: &FactNodeRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let props_str = serde_json::to_string(&node.properties)?;
        conn.execute(
            "INSERT INTO fact_nodes (id, node_type, name, label, properties, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
             ON CONFLICT(id) DO UPDATE SET
                node_type = excluded.node_type,
                name = excluded.name,
                label = excluded.label,
                properties = excluded.properties,
                updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now')",
            params![node.id, node.node_type, node.name, node.label, props_str],
        )?;
        Ok(())
    }

    fn upsert_fact_edge(&self, edge: &FactEdgeRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let props_str = serde_json::to_string(&edge.properties)?;
        conn.execute(
            "INSERT INTO fact_edges (source_id, target_id, relation, properties, confidence, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
             ON CONFLICT(source_id, target_id, relation) DO UPDATE SET
                properties = excluded.properties,
                confidence = excluded.confidence,
                updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now')",
            params![edge.source_id, edge.target_id, edge.relation, props_str, edge.confidence],
        )?;
        Ok(())
    }

    fn get_fact_node(&self, id: &str) -> Result<Option<FactNodeRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes WHERE id = ?1"
        )?;

        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
            Ok(Some(FactNodeRecord {
                id: row.get(0)?,
                node_type: row.get(1)?,
                name: row.get(2)?,
                label: row.get(3)?,
                properties,
                updated_at: row.get(5)?,
            }))
        } else {
            Ok(None)
        }
    }

    fn find_fact_nodes_by_name(&self, name: &str) -> Result<Vec<FactNodeRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes 
             WHERE name LIKE ?1 OR label LIKE ?1 LIMIT 50"
        )?;

        let pattern = format!("%{}%", name);
        let nodes = stmt.query_map(params![pattern], |row| {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
            Ok(FactNodeRecord {
                id: row.get(0)?,
                node_type: row.get(1)?,
                name: row.get(2)?,
                label: row.get(3)?,
                properties,
                updated_at: row.get(5)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(nodes)
    }

    fn traverse_fact_graph(&self, root_id: &str, max_hops: usize) -> Result<SubGraph> {
        let root = match self.get_fact_node(root_id)? {
            Some(r) => r,
            None => return Ok(SubGraph::default()),
        };

        let conn = self.lock_conn()?;
        let mut visited_node_ids = std::collections::HashSet::new();
        visited_node_ids.insert(root_id.to_string());

        let mut current_frontier = vec![root_id.to_string()];
        let mut collected_edges = Vec::new();

        for _ in 0..max_hops {
            if current_frontier.is_empty() {
                break;
            }
            let mut next_frontier = Vec::new();

            for node_id in &current_frontier {
                let mut stmt = conn.prepare(
                    "SELECT id, source_id, target_id, relation, properties, confidence, updated_at
                     FROM fact_edges
                     WHERE source_id = ?1 OR target_id = ?1"
                )?;

                let edges = stmt.query_map(params![node_id], |row| {
                    let props_str: String = row.get(4)?;
                    let props = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
                    Ok(FactEdgeRecord {
                        id: Some(row.get(0)?),
                        source_id: row.get(1)?,
                        target_id: row.get(2)?,
                        relation: row.get(3)?,
                        properties: props,
                        confidence: row.get(5)?,
                        updated_at: row.get(6)?,
                    })
                })?.collect::<Result<Vec<_>, _>>()?;

                for e in edges {
                    let other_id = if &e.source_id == node_id {
                        &e.target_id
                    } else {
                        &e.source_id
                    };

                    if !visited_node_ids.contains(other_id) {
                        visited_node_ids.insert(other_id.clone());
                        next_frontier.push(other_id.clone());
                    }
                    if !collected_edges.iter().any(|existing: &FactEdgeRecord| {
                        existing.source_id == e.source_id && existing.target_id == e.target_id && existing.relation == e.relation
                    }) {
                        collected_edges.push(e);
                    }
                }
            }
            current_frontier = next_frontier;
        }

        let mut collected_nodes = Vec::new();
        for nid in &visited_node_ids {
            let mut stmt = conn.prepare(
                "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes WHERE id = ?1"
            )?;
            let mut rows = stmt.query(params![nid])?;
            if let Some(row) = rows.next()? {
                let props_str: String = row.get(4)?;
                let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
                collected_nodes.push(FactNodeRecord {
                    id: row.get(0)?,
                    node_type: row.get(1)?,
                    name: row.get(2)?,
                    label: row.get(3)?,
                    properties,
                    updated_at: row.get(5)?,
                });
            }
        }

        Ok(SubGraph {
            root_node: Some(root),
            nodes: collected_nodes,
            edges: collected_edges,
        })
    }

    fn list_all_fact_nodes(&self, limit: usize) -> Result<Vec<FactNodeRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes ORDER BY id LIMIT ?1"
        )?;
        let nodes = stmt.query_map(params![limit as i64], |row| {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
            Ok(FactNodeRecord {
                id: row.get(0)?,
                node_type: row.get(1)?,
                name: row.get(2)?,
                label: row.get(3)?,
                properties,
                updated_at: row.get(5)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(nodes)
    }

    fn list_all_fact_edges(&self, limit: usize) -> Result<Vec<FactEdgeRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, source_id, target_id, relation, properties, confidence, updated_at FROM fact_edges ORDER BY id LIMIT ?1"
        )?;
        let edges = stmt.query_map(params![limit as i64], |row| {
            let props_str: String = row.get(4)?;
            let properties: serde_json::Value = serde_json::from_str(&props_str).unwrap_or(serde_json::json!({}));
            Ok(FactEdgeRecord {
                id: Some(row.get(0)?),
                source_id: row.get(1)?,
                target_id: row.get(2)?,
                relation: row.get(3)?,
                properties,
                confidence: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;
        Ok(edges)
    }

    fn delete_fact_node(&self, id: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        conn.execute(
            "DELETE FROM fact_edges WHERE source_id = ?1 OR target_id = ?1",
            params![id],
        )?;
        let affected = conn.execute("DELETE FROM fact_nodes WHERE id = ?1", params![id])?;
        Ok(affected)
    }

    fn update_asset_embedding(&self, path: &str, embedding: &[f32]) -> Result<()> {
        let conn = self.lock_conn()?;
        let emb_str = serde_json::to_string(embedding)?;
        conn.execute(
            "UPDATE assets SET embedding = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now') WHERE path = ?2",
            params![emb_str, path],
        )?;
        Ok(())
    }

    fn get_asset_embedding(&self, path: &str) -> Result<Vec<f32>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT COALESCE(embedding, '[]') FROM assets WHERE path = ?1")?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            let s: String = row.get(0)?;
            Ok(serde_json::from_str(&s).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    fn update_fact_node_embedding(&self, id: &str, embedding: &[f32]) -> Result<()> {
        let conn = self.lock_conn()?;
        let emb_str = serde_json::to_string(embedding)?;
        conn.execute(
            "UPDATE fact_nodes SET embedding = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now') WHERE id = ?2",
            params![emb_str, id],
        )?;
        Ok(())
    }

    fn get_fact_node_embedding(&self, id: &str) -> Result<Vec<f32>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT COALESCE(embedding, '[]') FROM fact_nodes WHERE id = ?1")?;
        let mut rows = stmt.query(params![id])?;
        if let Some(row) = rows.next()? {
            let s: String = row.get(0)?;
            Ok(serde_json::from_str(&s).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    // ─── Multi-Agent Coordination Locks (多 Agent 协作锁) ────────────
    fn insert_coordination_lock(&self, lock: &CoordinationLock) -> Result<bool> {
        let conn = self.lock_conn()?;
        // 1. 先清理该资源上已过期的锁记录
        conn.execute(
            "DELETE FROM coordination_locks WHERE resource_type = ?1 AND resource_id = ?2 AND expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            params![lock.resource_type, lock.resource_id],
        )?;

        // 2. 检查当前资源是否已被有效锁占用
        let mut stmt = conn.prepare(
            "SELECT lock_id, scope, holder_agent, expires_at FROM coordination_locks 
             WHERE resource_type = ?1 AND resource_id = ?2 AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"
        )?;
        let mut rows = stmt.query(params![lock.resource_type, lock.resource_id])?;

        while let Some(row) = rows.next()? {
            let existing_scope: String = row.get(1)?;
            let existing_holder: String = row.get(2)?;

            // 若持有者是自己，允许同一 Agent 刷新/重入
            if existing_holder == lock.holder_agent {
                continue;
            }

            // 排他锁冲突规则：只要有任一方为 exclusive，即冲突
            if lock.scope == "exclusive" || existing_scope == "exclusive" {
                return Ok(false);
            }
        }

        // 3. 无冲突，插入新锁
        let metadata_str = lock.metadata.as_ref()
            .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string()))
            .unwrap_or_else(|| "{}".to_string());

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
                lock.holder_pid,
                lock.acquired_at,
                lock.expires_at,
                metadata_str,
            ],
        )?;
        Ok(true)
    }

    fn delete_coordination_lock(&self, lock_id: &str) -> Result<bool> {
        let conn = self.lock_conn()?;
        let affected = conn.execute("DELETE FROM coordination_locks WHERE lock_id = ?1", params![lock_id])?;
        Ok(affected > 0)
    }

    fn get_coordination_lock(&self, resource_type: &str, resource_id: &str) -> Result<Option<CoordinationLock>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata
             FROM coordination_locks
             WHERE resource_type = ?1 AND resource_id = ?2 AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             ORDER BY acquired_at DESC LIMIT 1"
        )?;
        let mut rows = stmt.query(params![resource_type, resource_id])?;
        if let Some(row) = rows.next()? {
            let meta_str: String = row.get(8)?;
            let metadata = serde_json::from_str(&meta_str).ok();
            Ok(Some(CoordinationLock {
                lock_id: row.get(0)?,
                resource_type: row.get(1)?,
                resource_id: row.get(2)?,
                scope: row.get(3)?,
                holder_agent: row.get(4)?,
                holder_pid: row.get(5)?,
                acquired_at: row.get(6)?,
                expires_at: row.get(7)?,
                metadata,
            }))
        } else {
            Ok(None)
        }
    }

    fn list_coordination_locks(&self, resource_type: Option<&str>) -> Result<Vec<CoordinationLock>> {
        let conn = self.lock_conn()?;
        let sql = match resource_type {
            Some(_) => "SELECT lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata
                        FROM coordination_locks
                        WHERE resource_type = ?1 AND expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                        ORDER BY acquired_at DESC",
            None => "SELECT lock_id, resource_type, resource_id, scope, holder_agent, holder_pid, acquired_at, expires_at, metadata
                     FROM coordination_locks
                     WHERE expires_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     ORDER BY acquired_at DESC",
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = match resource_type {
            Some(rt) => stmt.query_map(params![rt], Self::map_lock_row)?,
            None => stmt.query_map([], Self::map_lock_row)?,
        };
        let mut result = Vec::new();
        for r in rows {
            result.push(r?);
        }
        Ok(result)
    }

    fn gc_expired_coordination_locks(&self) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count = conn.execute(
            "DELETE FROM coordination_locks WHERE expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            [],
        )?;
        Ok(count)
    }

    fn upsert_mcp_catalog(&self, rec: &McpCatalogRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let tools_str = serde_json::to_string(&rec.tools_json)?;
        conn.execute(
            "INSERT INTO mcp_catalog (id, server_name, client_source, transport, command_or_url, tools_json, tool_count, est_total_tokens, status, last_scanned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
             ON CONFLICT(server_name) DO UPDATE SET
               client_source=excluded.client_source,
               transport=excluded.transport,
               command_or_url=excluded.command_or_url,
               tools_json=excluded.tools_json,
               tool_count=excluded.tool_count,
               est_total_tokens=excluded.est_total_tokens,
               status=excluded.status,
               last_scanned_at=strftime('%Y-%m-%dT%H:%M:%f+00:00','now')",
            params![
                rec.id, rec.server_name, rec.client_source, rec.transport,
                rec.command_or_url, tools_str, rec.tool_count as i64,
                rec.est_total_tokens as i64, rec.status
            ],
        )?;
        Ok(())
    }

    fn list_mcp_catalog(&self) -> Result<Vec<McpCatalogRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, server_name, client_source, transport, command_or_url, tools_json, tool_count, est_total_tokens, status, last_scanned_at
             FROM mcp_catalog ORDER BY server_name ASC"
        )?;
        let rows = stmt.query_map([], Self::map_mcp_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }

    fn search_mcp_catalog(&self, query: &str, limit: usize) -> Result<Vec<McpCatalogRecord>> {
        let conn = self.lock_conn()?;
        let kw = format!("%{}%", query.to_lowercase());
        let mut stmt = conn.prepare(
            "SELECT id, server_name, client_source, transport, command_or_url, tools_json, tool_count, est_total_tokens, status, last_scanned_at
             FROM mcp_catalog
             WHERE lower(server_name) LIKE ?1 OR lower(tools_json) LIKE ?1 OR lower(client_source) LIKE ?1
             ORDER BY tool_count DESC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![kw, limit as i64], Self::map_mcp_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }

    fn upsert_skill_catalog(&self, rec: &SkillCatalogRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let triggers_str = serde_json::to_string(&rec.triggers)?;
        conn.execute(
            "INSERT INTO skills_catalog (id, skill_name, root_source, skill_path, description, triggers_json, est_tokens, has_scripts, has_references, status, last_scanned_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
             ON CONFLICT(skill_name) DO UPDATE SET
               root_source=excluded.root_source,
               skill_path=excluded.skill_path,
               description=excluded.description,
               triggers_json=excluded.triggers_json,
               est_tokens=excluded.est_tokens,
               has_scripts=excluded.has_scripts,
               has_references=excluded.has_references,
               status=excluded.status,
               last_scanned_at=strftime('%Y-%m-%dT%H:%M:%f+00:00','now')",
            params![
                rec.id, rec.skill_name, rec.root_source, rec.skill_path,
                rec.description, triggers_str, rec.est_tokens as i64,
                rec.has_scripts as i32, rec.has_references as i32, rec.status
            ],
        )?;
        Ok(())
    }

    fn list_skills_catalog(&self) -> Result<Vec<SkillCatalogRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, skill_name, root_source, skill_path, description, triggers_json, est_tokens, has_scripts, has_references, status, last_scanned_at
             FROM skills_catalog ORDER BY skill_name ASC"
        )?;
        let rows = stmt.query_map([], Self::map_skill_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }

    fn search_skills_catalog(&self, query: &str, limit: usize) -> Result<Vec<SkillCatalogRecord>> {
        let conn = self.lock_conn()?;
        let clean_q: String = query
            .chars()
            .map(|c| if c.is_alphanumeric() || c as u32 > 0x4E00 || c == ' ' { c } else { ' ' })
            .collect();
        let fts_tokens: Vec<String> = clean_q.split_whitespace().map(|w| format!("\"{}\"", w)).collect();
        if !fts_tokens.is_empty() {
            let fts_expr = fts_tokens.join(" OR ");
            if let Ok(mut stmt) = conn.prepare(
                "SELECT s.id, s.skill_name, s.root_source, s.skill_path, s.description, s.triggers_json, s.est_tokens, s.has_scripts, s.has_references, s.status, s.last_scanned_at
                 FROM skills_catalog s
                 JOIN skills_catalog_fts f ON s.rowid = f.rowid
                 WHERE skills_catalog_fts MATCH ?1
                 ORDER BY rank LIMIT ?2"
            ) {
                if let Ok(rows) = stmt.query_map(params![fts_expr, limit as i64], Self::map_skill_row) {
                    let mut out = Vec::new();
                    for r in rows {
                        if let Ok(item) = r { out.push(item); }
                    }
                    if !out.is_empty() {
                        return Ok(out);
                    }
                }
            }
        }
        let kw = format!("%{}%", query.to_lowercase());
        let mut stmt = conn.prepare(
            "SELECT id, skill_name, root_source, skill_path, description, triggers_json, est_tokens, has_scripts, has_references, status, last_scanned_at
             FROM skills_catalog
             WHERE lower(skill_name) LIKE ?1 OR lower(description) LIKE ?1 OR lower(triggers_json) LIKE ?1
             ORDER BY skill_name ASC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![kw, limit as i64], Self::map_skill_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }

    fn upsert_external_service(&self, rec: &ExternalServiceRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO external_services (id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, COALESCE(?12, strftime('%Y-%m-%dT%H:%M:%f+00:00','now')))
             ON CONFLICT(service_name) DO UPDATE SET
               category=excluded.category,
               endpoint=excluded.endpoint,
               protocol=excluded.protocol,
               capabilities=excluded.capabilities,
               auth_type=excluded.auth_type,
               auth_env_var=excluded.auth_env_var,
               health_endpoint=excluded.health_endpoint,
               usage_example=excluded.usage_example,
               status=excluded.status,
               last_checked_at=COALESCE(excluded.last_checked_at, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))",
            params![
                rec.id, rec.service_name, rec.category, rec.endpoint, rec.protocol,
                rec.capabilities, rec.auth_type, rec.auth_env_var, rec.health_endpoint,
                rec.usage_example, rec.status, rec.last_checked_at
            ],
        )?;
        Ok(())
    }

    fn list_external_services(&self, category: Option<&str>) -> Result<Vec<ExternalServiceRecord>> {
        let conn = self.lock_conn()?;
        let mut res = Vec::new();
        match category {
            Some(cat) => {
                let mut stmt = conn.prepare(
                    "SELECT id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at
                     FROM external_services WHERE category = ?1 ORDER BY service_name ASC"
                )?;
                let rows = stmt.query_map(params![cat], Self::map_ext_svc_row)?;
                for r in rows { res.push(r?); }
            }
            None => {
                let mut stmt = conn.prepare(
                    "SELECT id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at
                     FROM external_services ORDER BY service_name ASC"
                )?;
                let rows = stmt.query_map([], Self::map_ext_svc_row)?;
                for r in rows { res.push(r?); }
            }
        }
        Ok(res)
    }

    fn search_external_services(&self, query: &str, limit: usize) -> Result<Vec<ExternalServiceRecord>> {
        let conn = self.lock_conn()?;
        let kw = format!("%{}%", query.to_lowercase());
        let mut stmt = conn.prepare(
            "SELECT id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at
             FROM external_services
             WHERE lower(service_name) LIKE ?1 OR lower(category) LIKE ?1 OR lower(capabilities) LIKE ?1
             ORDER BY service_name ASC LIMIT ?2"
        )?;
        let rows = stmt.query_map(params![kw, limit as i64], Self::map_ext_svc_row)?;
        let mut res = Vec::new();
        for r in rows { res.push(r?); }
        Ok(res)
    }

    fn enqueue_lock_wait(&self, entry: &LockWaitEntry) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO lock_wait_queue (wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                entry.wait_id, entry.resource_type, entry.resource_id, entry.scope,
                entry.waiting_agent, entry.priority, entry.enqueued_at, entry.timeout_at, entry.status
            ],
        )?;
        Ok(())
    }

    fn list_lock_wait_queue(&self, resource_type: Option<&str>, resource_id: Option<&str>) -> Result<Vec<LockWaitEntry>> {
        let conn = self.lock_conn()?;
        let mut res = Vec::new();
        match (resource_type, resource_id) {
            (Some(rt), Some(rid)) => {
                let mut stmt = conn.prepare(
                    "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
                     FROM lock_wait_queue
                     WHERE resource_type = ?1 AND resource_id = ?2 AND status IN ('waiting','notified')
                       AND timeout_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     ORDER BY priority ASC, enqueued_at ASC"
                )?;
                let rows = stmt.query_map(params![rt, rid], Self::map_wait_row)?;
                for r in rows { res.push(r?); }
            }
            (Some(rt), None) => {
                let mut stmt = conn.prepare(
                    "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
                     FROM lock_wait_queue
                     WHERE resource_type = ?1 AND status IN ('waiting','notified')
                       AND timeout_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     ORDER BY priority ASC, enqueued_at ASC"
                )?;
                let rows = stmt.query_map(params![rt], Self::map_wait_row)?;
                for r in rows { res.push(r?); }
            }
            _ => {
                let mut stmt = conn.prepare(
                    "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
                     FROM lock_wait_queue
                     WHERE status IN ('waiting','notified')
                       AND timeout_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                     ORDER BY priority ASC, enqueued_at ASC"
                )?;
                let rows = stmt.query_map([], Self::map_wait_row)?;
                for r in rows { res.push(r?); }
            }
        }
        Ok(res)
    }

    fn pop_next_lock_waiter(&self, resource_type: &str, resource_id: &str) -> Result<Option<LockWaitEntry>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT wait_id, resource_type, resource_id, scope, waiting_agent, priority, enqueued_at, timeout_at, status
             FROM lock_wait_queue
             WHERE resource_type = ?1 AND resource_id = ?2 AND status = 'waiting'
               AND timeout_at > strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             ORDER BY priority ASC, enqueued_at ASC LIMIT 1"
        )?;
        let mut rows = stmt.query_map(params![resource_type, resource_id], Self::map_wait_row)?;
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

    fn update_lock_wait_status(&self, wait_id: &str, status: &str) -> Result<bool> {
        let conn = self.lock_conn()?;
        let affected = conn.execute(
            "UPDATE lock_wait_queue SET status = ?1 WHERE wait_id = ?2",
            params![status, wait_id],
        )?;
        Ok(affected > 0)
    }

    fn gc_expired_lock_waits(&self) -> Result<usize> {
        let conn = self.lock_conn()?;
        let affected = conn.execute(
            "UPDATE lock_wait_queue SET status = 'expired'
             WHERE status IN ('waiting','notified') AND timeout_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            [],
        )?;
        Ok(affected)
    }

    fn insert_benchmark_run(&self, run: &BenchmarkRunRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let host_fp = serde_json::to_string(&run.host_fingerprint)?;
        let res_json = serde_json::to_string(&run.results_json)?;
        conn.execute(
            "INSERT OR REPLACE INTO benchmark_runs (
                run_id, suite_name, git_commit, app_version, host_fingerprint,
                workload_count, total_baseline_tokens, total_steward_tokens,
                overall_token_savings_pct, overall_speedup_factor,
                total_hazards_intercepted, total_noise_blocked, sla_passed,
                results_json, report_sha256, duration_ms, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)",
            params![
                run.run_id,
                run.suite_name,
                run.git_commit,
                run.app_version,
                host_fp,
                run.workload_count as i64,
                run.total_baseline_tokens,
                run.total_steward_tokens,
                run.overall_token_savings_pct,
                run.overall_speedup_factor,
                run.total_hazards_intercepted,
                run.total_noise_blocked,
                if run.sla_passed { 1i32 } else { 0i32 },
                res_json,
                run.report_sha256,
                run.duration_ms,
                run.created_at,
            ],
        )?;
        Ok(())
    }

    fn get_benchmark_run(&self, run_id: &str) -> Result<Option<BenchmarkRunRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT run_id, suite_name, git_commit, app_version, host_fingerprint,
                    workload_count, total_baseline_tokens, total_steward_tokens,
                    overall_token_savings_pct, overall_speedup_factor,
                    total_hazards_intercepted, total_noise_blocked, sla_passed,
                    results_json, report_sha256, duration_ms, created_at
             FROM benchmark_runs WHERE run_id = ?1",
        )?;
        let mut rows = stmt.query(params![run_id])?;
        if let Some(row) = rows.next()? {
            Ok(Some(Self::map_benchmark_run_row(row)?))
        } else {
            Ok(None)
        }
    }

    fn list_benchmark_runs(&self, limit: usize) -> Result<Vec<BenchmarkRunRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT run_id, suite_name, git_commit, app_version, host_fingerprint,
                    workload_count, total_baseline_tokens, total_steward_tokens,
                    overall_token_savings_pct, overall_speedup_factor,
                    total_hazards_intercepted, total_noise_blocked, sla_passed,
                    results_json, report_sha256, duration_ms, created_at
             FROM benchmark_runs ORDER BY created_at DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map(params![limit as i64], Self::map_benchmark_run_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    fn insert_effectiveness_event(&self, event: &EffectivenessLedgerRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO effectiveness_ledger (
                event_id, session_id, source_channel, tool_or_action, provenance_rule,
                baseline_tokens, steward_tokens, tokens_saved,
                baseline_rounds, steward_rounds, rounds_saved,
                hazard_intercepted, noise_blocked, elapsed_us, evidence_summary, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
            params![
                event.event_id,
                event.session_id,
                event.source_channel,
                event.tool_or_action,
                event.provenance_rule,
                event.baseline_tokens,
                event.steward_tokens,
                event.tokens_saved,
                event.baseline_rounds,
                event.steward_rounds,
                event.rounds_saved,
                event.hazard_intercepted,
                event.noise_blocked,
                event.elapsed_us,
                event.evidence_summary,
                event.created_at,
            ],
        )?;
        Ok(())
    }

    fn query_effectiveness_ledger(
        &self,
        window: &str,
        tool_filter: Option<&str>,
        limit: usize,
    ) -> Result<EffectivenessSummary> {
        let conn = self.lock_conn()?;
        let cutoff = crate::storage::window_to_cutoff_iso(window).unwrap_or_default();
        let tool = tool_filter.unwrap_or("").to_string();

        let (total_events, total_base, total_stew, total_saved, total_rounds, total_haz, total_noise, avg_us): (
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            i64,
            f64,
        ) = conn.query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(baseline_tokens), 0),
                    COALESCE(SUM(steward_tokens), 0),
                    COALESCE(SUM(tokens_saved), 0),
                    COALESCE(SUM(rounds_saved), 0),
                    COALESCE(SUM(hazard_intercepted), 0),
                    COALESCE(SUM(noise_blocked), 0),
                    COALESCE(AVG(elapsed_us), 0.0)
             FROM effectiveness_ledger
             WHERE (?1 = '' OR created_at >= ?1)
               AND (?2 = '' OR tool_or_action = ?2)",
            params![cutoff, tool],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            },
        )?;

        let mut rule_stmt = conn.prepare(
            "SELECT provenance_rule,
                    COUNT(*),
                    COALESCE(SUM(tokens_saved), 0),
                    COALESCE(SUM(rounds_saved), 0),
                    COALESCE(SUM(hazard_intercepted), 0),
                    COALESCE(SUM(noise_blocked), 0)
             FROM effectiveness_ledger
             WHERE (?1 = '' OR created_at >= ?1)
               AND (?2 = '' OR tool_or_action = ?2)
             GROUP BY provenance_rule
             ORDER BY SUM(tokens_saved) DESC, COUNT(*) DESC",
        )?;
        let by_rule = rule_stmt
            .query_map(params![cutoff, tool], |r| {
                Ok(RuleAggregate {
                    provenance_rule: r.get(0)?,
                    event_count: r.get::<_, i64>(1)? as usize,
                    tokens_saved: r.get(2)?,
                    rounds_saved: r.get(3)?,
                    hazards_intercepted: r.get(4)?,
                    noise_blocked: r.get(5)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let mut ev_stmt = conn.prepare(
            "SELECT event_id, session_id, source_channel, tool_or_action, provenance_rule,
                    baseline_tokens, steward_tokens, tokens_saved,
                    baseline_rounds, steward_rounds, rounds_saved,
                    hazard_intercepted, noise_blocked, elapsed_us, evidence_summary, created_at
             FROM effectiveness_ledger
             WHERE (?1 = '' OR created_at >= ?1)
               AND (?2 = '' OR tool_or_action = ?2)
             ORDER BY created_at DESC LIMIT ?3",
        )?;
        let recent_events = ev_stmt
            .query_map(params![cutoff, tool, limit as i64], |r| {
                Ok(EffectivenessLedgerRecord {
                    event_id: r.get(0)?,
                    session_id: r.get(1)?,
                    source_channel: r.get(2)?,
                    tool_or_action: r.get(3)?,
                    provenance_rule: r.get(4)?,
                    baseline_tokens: r.get(5)?,
                    steward_tokens: r.get(6)?,
                    tokens_saved: r.get(7)?,
                    baseline_rounds: r.get(8)?,
                    steward_rounds: r.get(9)?,
                    rounds_saved: r.get(10)?,
                    hazard_intercepted: r.get(11)?,
                    noise_blocked: r.get(12)?,
                    elapsed_us: r.get(13)?,
                    evidence_summary: r.get(14)?,
                    created_at: r.get(15)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        let token_savings_pct = if total_base > 0 {
            (total_saved as f64 / total_base as f64) * 100.0
        } else {
            0.0
        };

        Ok(EffectivenessSummary {
            window: window.to_string(),
            total_events: total_events as usize,
            total_baseline_tokens: total_base,
            total_steward_tokens: total_stew,
            total_tokens_saved: total_saved,
            token_savings_pct,
            total_rounds_saved: total_rounds,
            total_hazards_intercepted: total_haz,
            total_noise_blocked: total_noise,
            avg_elapsed_us: avg_us,
            by_rule,
            recent_events,
        })
    }

    fn delete_bench_sandbox_records(&self) -> Result<usize> {
        let conn = self.lock_conn()?;
        let mut removed = 0usize;
        removed += conn.execute(
            "DELETE FROM coordination_locks WHERE holder_agent LIKE 'bench-%' OR resource_id LIKE '%bench:%'",
            [],
        )?;
        removed += conn.execute(
            "DELETE FROM lock_wait_queue WHERE waiting_agent LIKE 'bench-%' OR resource_id LIKE '%bench:%'",
            [],
        )?;
        removed += conn.execute(
            "DELETE FROM effectiveness_ledger WHERE session_id LIKE 'bench:%' OR tool_or_action LIKE 'bench:%'",
            [],
        )?;
        removed += conn.execute(
            "DELETE FROM benchmark_runs WHERE run_id LIKE 'bench:%' OR suite_name LIKE 'bench:%'",
            [],
        )?;
        Ok(removed)
    }
}

impl SqliteStorageProvider {
    fn map_benchmark_run_row(row: &rusqlite::Row) -> rusqlite::Result<BenchmarkRunRecord> {
        let host_str: String = row.get(4)?;
        let res_str: String = row.get(13)?;
        Ok(BenchmarkRunRecord {
            run_id: row.get(0)?,
            suite_name: row.get(1)?,
            git_commit: row.get(2)?,
            app_version: row.get(3)?,
            host_fingerprint: serde_json::from_str(&host_str).unwrap_or_else(|_| serde_json::json!({})),
            workload_count: row.get::<_, i64>(5)? as usize,
            total_baseline_tokens: row.get(6)?,
            total_steward_tokens: row.get(7)?,
            overall_token_savings_pct: row.get(8)?,
            overall_speedup_factor: row.get(9)?,
            total_hazards_intercepted: row.get(10)?,
            total_noise_blocked: row.get(11)?,
            sla_passed: row.get::<_, i32>(12)? != 0,
            results_json: serde_json::from_str(&res_str).unwrap_or_else(|_| serde_json::json!([])),
            report_sha256: row.get(14)?,
            duration_ms: row.get(15)?,
            created_at: row.get(16)?,
        })
    }
    fn map_wait_row(row: &rusqlite::Row) -> rusqlite::Result<LockWaitEntry> {
        Ok(LockWaitEntry {
            wait_id: row.get(0)?,
            resource_type: row.get(1)?,
            resource_id: row.get(2)?,
            scope: row.get(3)?,
            waiting_agent: row.get(4)?,
            priority: row.get(5)?,
            enqueued_at: row.get(6)?,
            timeout_at: row.get(7)?,
            status: row.get(8)?,
        })
    }
    fn map_mcp_row(row: &rusqlite::Row) -> rusqlite::Result<McpCatalogRecord> {
        let tools_str: String = row.get(5)?;
        let tools_json = serde_json::from_str(&tools_str).unwrap_or_else(|_| serde_json::json!([]));
        Ok(McpCatalogRecord {
            id: row.get(0)?,
            server_name: row.get(1)?,
            client_source: row.get(2)?,
            transport: row.get(3)?,
            command_or_url: row.get(4)?,
            tools_json,
            tool_count: row.get::<_, i64>(6)? as usize,
            est_total_tokens: row.get::<_, i64>(7)? as usize,
            status: row.get(8)?,
            last_scanned_at: row.get(9)?,
        })
    }

    fn map_skill_row(row: &rusqlite::Row) -> rusqlite::Result<SkillCatalogRecord> {
        let triggers_str: String = row.get(5)?;
        let triggers: Vec<String> = serde_json::from_str(&triggers_str).unwrap_or_default();
        Ok(SkillCatalogRecord {
            id: row.get(0)?,
            skill_name: row.get(1)?,
            root_source: row.get(2)?,
            skill_path: row.get(3)?,
            description: row.get(4)?,
            triggers,
            est_tokens: row.get::<_, i64>(6)? as usize,
            has_scripts: row.get::<_, i32>(7)? != 0,
            has_references: row.get::<_, i32>(8)? != 0,
            status: row.get(9)?,
            last_scanned_at: row.get(10)?,
        })
    }

    fn map_ext_svc_row(row: &rusqlite::Row) -> rusqlite::Result<ExternalServiceRecord> {
        Ok(ExternalServiceRecord {
            id: row.get(0)?,
            service_name: row.get(1)?,
            category: row.get(2)?,
            endpoint: row.get(3)?,
            protocol: row.get(4)?,
            capabilities: row.get(5)?,
            auth_type: row.get(6)?,
            auth_env_var: row.get(7)?,
            health_endpoint: row.get(8)?,
            usage_example: row.get(9)?,
            status: row.get(10)?,
            last_checked_at: row.get(11)?,
        })
    }
    fn map_lock_row(row: &rusqlite::Row) -> rusqlite::Result<CoordinationLock> {
        let meta_str: String = row.get(8)?;
        let metadata = serde_json::from_str(&meta_str).ok();
        Ok(CoordinationLock {
            lock_id: row.get(0)?,
            resource_type: row.get(1)?,
            resource_id: row.get(2)?,
            scope: row.get(3)?,
            holder_agent: row.get(4)?,
            holder_pid: row.get(5)?,
            acquired_at: row.get(6)?,
            expires_at: row.get(7)?,
            metadata,
        })
    }
}
