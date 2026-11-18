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
        }
        let conn = Connection::open(path)
            .with_context(|| format!("打开 SQLite 数据库失败: {:?}", path))?;
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
}

impl StorageProvider for SqliteStorageProvider {
    fn init(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(SCHEMA_SQL)
            .context("执行内置 SQLite Schema 批处理失败")?;
        // 增量热迁移兼容已存在数据库
        let _ = conn.execute("ALTER TABLE memories ADD COLUMN embedding TEXT DEFAULT '[]'", []);
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        let desc_kv_str = serde_json::to_string(&asset.desc_kv)?;
        let tags_str = serde_json::to_string(&asset.tags)?;

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
            ],
        )?;
        Ok(())
    }

    fn get_asset_by_path(&self, path: &str) -> Result<Option<AssetRecord>> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT a.id, a.path, a.type, a.owner, a.security_level, a.scan_tier, a.description, a.desc_kv, a.usage, a.size_bytes, a.file_count, a.tags, a.status
             FROM assets_fts f
             JOIN assets a ON f.rowid = a.rowid
             WHERE assets_fts MATCH ?1
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
        let conn = self.conn.lock().unwrap();
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
        let mut conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute(
            "UPDATE memories SET status = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now') WHERE id = ?1",
            params![memory_id, status],
        )?;
        Ok(rows)
    }

    fn record_change(&self, change: &ChangeRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM links WHERE id = ?1", params![id])?;
        Ok(())
    }

    fn delete_links_for_target(&self, target_id: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM links WHERE target_id = ?1", params![target_id])?;
        Ok(count)
    }

    fn update_link_target(&self, source_id: &str, old_target: &str, new_target: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "UPDATE links SET target_id = ?3 WHERE source_id = ?1 AND target_id = ?2",
            params![source_id, old_target, new_target],
        )?;
        Ok(count)
    }

    fn query_links_for_node(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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

        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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

    // ─── Multi-Agent Coordination Locks (多 Agent 协作锁) ────────────
    fn insert_coordination_lock(&self, lock: &CoordinationLock) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        let affected = conn.execute("DELETE FROM coordination_locks WHERE lock_id = ?1", params![lock_id])?;
        Ok(affected > 0)
    }

    fn get_coordination_lock(&self, resource_type: &str, resource_id: &str) -> Result<Option<CoordinationLock>> {
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
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
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "DELETE FROM coordination_locks WHERE expires_at <= strftime('%Y-%m-%dT%H:%M:%fZ', 'now')",
            [],
        )?;
        Ok(count)
    }
}

impl SqliteStorageProvider {
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
