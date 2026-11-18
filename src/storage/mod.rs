pub mod models;

use std::path::Path;
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use models::*;

pub const SCHEMA_SQL: &str = include_str!("schema.sql");

#[derive(Clone)]
pub struct Storage {
    conn: Arc<Mutex<Connection>>,
}

impl Storage {
    /// 打开或创建 SQLite 数据库，并自动执行 Schema 初始化
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("创建数据库目录失败: {:?}", parent))?;
        }
        let conn = Connection::open(path)
            .with_context(|| format!("打开 SQLite 数据库失败: {:?}", path))?;
        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        storage.init_schema()?;
        Ok(storage)
    }

    /// 打开内存数据库（用于单元测试）
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()
            .context("打开 SQLite 内存数据库失败")?;
        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        storage.init_schema()?;
        Ok(storage)
    }

    /// 初始化 Schema
    pub fn init_schema(&self) -> Result<()> {
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

    // ─── Placement Rules ─────────────────────────────────────────────

    pub fn get_placement_rules(&self) -> Result<Vec<PlacementRule>> {
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

    pub fn insert_placement_rule(&self, rule: &PlacementRule) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO placement_rules 
             (id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))",
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

    // ─── Scan Policies ───────────────────────────────────────────────

    pub fn get_scan_policies(&self) -> Result<Vec<ScanPolicy>> {
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

    // ─── Assets ──────────────────────────────────────────────────────

    pub fn upsert_asset(&self, asset: &AssetRecord) -> Result<()> {
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

    pub fn get_asset_by_path(&self, path: &str) -> Result<Option<AssetRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets WHERE path = ?1"
        )?;

        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            let desc_kv_str: String = row.get(7)?;
            let desc_kv: serde_json::Value = serde_json::from_str(&desc_kv_str).unwrap_or(serde_json::Value::Object(Default::default()));
            let tags_str: String = row.get(11)?;
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

    pub fn search_assets_fts(&self, query: &str, limit: usize) -> Result<Vec<AssetRecord>> {
        let conn = self.conn.lock().unwrap();
        let fts_query = if query.contains('*') || query.contains('"') {
            query.to_string()
        } else {
            format!("\"{}\"*", query)
        };

        let mut stmt = conn.prepare(
            "SELECT a.id, a.path, a.type, a.owner, a.security_level, a.scan_tier, a.description, a.desc_kv, a.usage, a.size_bytes, a.file_count, a.tags, a.status
             FROM assets_fts f
             JOIN assets a ON a.rowid = f.rowid
             WHERE assets_fts MATCH ?1
             LIMIT ?2"
        )?;

        let mut rows: Vec<AssetRecord> = stmt.query_map(params![fts_query, limit as i64], |row| {
            let desc_kv_str: String = row.get(7)?;
            let desc_kv: serde_json::Value = serde_json::from_str(&desc_kv_str).unwrap_or(serde_json::Value::Object(Default::default()));
            let tags_str: String = row.get(11)?;
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
        })?.filter_map(|r| r.ok()).collect();

        // 若 FTS5 未直接命中，回退到 LIKE 模糊匹配保障无分词中文子串召回
        if rows.is_empty() {
            let like_pattern = format!("%{}%", query);
            let mut fallback_stmt = conn.prepare(
                "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
                 FROM assets
                 WHERE path LIKE ?1 OR description LIKE ?1 OR tags LIKE ?1
                 LIMIT ?2"
            )?;
            rows = fallback_stmt.query_map(params![like_pattern, limit as i64], |row| {
                let desc_kv_str: String = row.get(7)?;
                let desc_kv: serde_json::Value = serde_json::from_str(&desc_kv_str).unwrap_or(serde_json::Value::Object(Default::default()));
                let tags_str: String = row.get(11)?;
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
            })?.filter_map(|r| r.ok()).collect();
        }

        Ok(rows)
    }

    pub fn list_assets(&self, limit: usize) -> Result<Vec<AssetRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets ORDER BY updated_at DESC LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let desc_kv_str: String = row.get(7)?;
            let desc_kv: serde_json::Value = serde_json::from_str(&desc_kv_str).unwrap_or(serde_json::Value::Object(Default::default()));
            let tags_str: String = row.get(11)?;
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

    // ─── Active Processes ────────────────────────────────────────────

    pub fn replace_active_processes(&self, procs: &[ActiveProcessRecord]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM active_processes", [])?;

        for p in procs {
            let ports_str = serde_json::to_string(&p.listening_ports)?;
            tx.execute(
                "INSERT INTO active_processes (id, pid, name, command, owner, associated_path, listening_ports, status, cpu_pct, mem_bytes, gpu_allocated_mb, started_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                params![
                    p.id,
                    p.pid as i64,
                    p.name,
                    p.command,
                    p.owner,
                    p.associated_path,
                    ports_str,
                    p.status,
                    p.cpu_pct as f64,
                    p.mem_bytes as i64,
                    p.gpu_allocated_mb as i64,
                    p.started_at,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn list_active_processes(&self) -> Result<Vec<ActiveProcessRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, pid, name, command, owner, associated_path, listening_ports, status, cpu_pct, mem_bytes, gpu_allocated_mb, started_at
             FROM active_processes ORDER BY cpu_pct DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            let ports_str: String = row.get(6)?;
            let listening_ports: Vec<u16> = serde_json::from_str(&ports_str).unwrap_or_default();

            Ok(ActiveProcessRecord {
                id: row.get(0)?,
                pid: row.get::<_, i64>(1)? as u32,
                name: row.get(2)?,
                command: row.get(3)?,
                owner: row.get(4)?,
                associated_path: row.get(5)?,
                listening_ports,
                status: row.get(7)?,
                cpu_pct: row.get::<_, f64>(8)? as f32,
                mem_bytes: row.get::<_, i64>(9)? as u64,
                gpu_allocated_mb: row.get::<_, i64>(10)? as u32,
                started_at: row.get(11)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    // ─── Memories ────────────────────────────────────────────────────

    pub fn insert_memory(&self, mem: &MemoryRecord) -> Result<()> {
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
                mem.weight as f64,
                if mem.pinned { 1 } else { 0 },
                mem.status,
                emb_str,
            ],
        )?;
        Ok(())
    }

    pub fn search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT m.id, m.type, m.content, m.summary, m.source_agent, m.source_session, m.tags, m.weight, m.pinned, m.status, m.embedding
             FROM memories_fts f
             JOIN memories m ON m.rowid = f.rowid
             WHERE memories_fts MATCH ?1
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![query, limit as i64], |row| {
            let tags_str: String = row.get(6)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let emb_str: String = row.get(10).unwrap_or_else(|_| "[]".to_string());
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();

            Ok(MemoryRecord {
                id: row.get(0)?,
                memory_type: row.get(1)?,
                content: row.get(2)?,
                summary: row.get(3)?,
                source_agent: row.get(4)?,
                source_session: row.get(5)?,
                tags,
                weight: row.get::<_, f64>(7)? as f32,
                pinned: row.get::<_, i32>(8)? != 0,
                status: row.get(9)?,
                embedding,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    pub fn list_all_active_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             WHERE status = 'active'
             ORDER BY created_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let tags_str: String = row.get(6)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let emb_str: String = row.get(10).unwrap_or_else(|_| "[]".to_string());
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();

            Ok(MemoryRecord {
                id: row.get(0)?,
                memory_type: row.get(1)?,
                content: row.get(2)?,
                summary: row.get(3)?,
                source_agent: row.get(4)?,
                source_session: row.get(5)?,
                tags,
                weight: row.get::<_, f64>(7)? as f32,
                pinned: row.get::<_, i32>(8)? != 0,
                status: row.get(9)?,
                embedding,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    pub fn update_memory_status(&self, memory_id: &str, status: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "UPDATE memories SET status = ?1 WHERE id = ?2",
            params![status, memory_id],
        )?;
        Ok(count)
    }

    // ─── Changes ─────────────────────────────────────────────────────

    pub fn record_change(&self, change: &ChangeRecord) -> Result<()> {
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

    pub fn list_changes(&self, limit: usize) -> Result<Vec<ChangeRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, type, target, description, details, source, created_at
             FROM changes ORDER BY created_at DESC LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let details_str: String = row.get(4)?;
            let details: serde_json::Value = serde_json::from_str(&details_str).unwrap_or_default();

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

    // ─── Links ───────────────────────────────────────────────────────

    pub fn add_link(&self, link: &LinkRecord) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO links (source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
             ON CONFLICT(source_type, source_id, target_type, target_id, relation) DO UPDATE SET
                confidence = excluded.confidence,
                auto_discovered = excluded.auto_discovered,
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
                if link.auto_discovered { 1 } else { 0 },
                link.line_number,
                link.snippet,
                link.provenance.as_deref().unwrap_or("static_scan"),
            ],
        )?;
        Ok(())
    }

    pub fn delete_link(&self, id: i64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM links WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delete_links_for_target(&self, target_id: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute("DELETE FROM links WHERE target_id = ?1", params![target_id])?;
        Ok(count)
    }

    pub fn update_link_target(&self, source_id: &str, old_target: &str, new_target: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap();
        let count = conn.execute(
            "UPDATE links SET target_id = ?1 WHERE source_id = ?2 AND target_id = ?3",
            params![new_target, source_id, old_target],
        )?;
        Ok(count)
    }

    pub fn query_links_for_node(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             WHERE (source_type = ?1 AND source_id = ?2) OR (target_type = ?1 AND target_id = ?2)
             ORDER BY confidence DESC, created_at DESC"
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

    pub fn list_all_links(&self, limit: usize) -> Result<Vec<LinkRecord>> {
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

    // ─── Scan Jobs ───────────────────────────────────────────────────

    pub fn create_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
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

    pub fn update_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
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

    pub fn get_scan_job(&self, id: &str) -> Result<Option<ScanJobRecord>> {
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

    pub fn list_scan_jobs(&self, limit: usize) -> Result<Vec<ScanJobRecord>> {
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

    pub fn get_active_scan_job(&self) -> Result<Option<ScanJobRecord>> {
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

    // ─── Fact Property Graph ──────────────────────────────────────────

    pub fn upsert_fact_node(&self, node: &FactNodeRecord) -> Result<()> {
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

    pub fn upsert_fact_edge(&self, edge: &FactEdgeRecord) -> Result<()> {
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

    pub fn get_fact_node(&self, id: &str) -> Result<Option<FactNodeRecord>> {
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

    pub fn find_fact_nodes_by_name(&self, name: &str) -> Result<Vec<FactNodeRecord>> {
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

    /// 基于广度优先遍历从指定 root_id 提取 N 跳子图
    pub fn traverse_fact_graph(&self, root_id: &str, max_hops: usize) -> Result<SubGraph> {
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

        // 收集所有已访问节点的详细信息
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_init_and_defaults() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let rules = storage.get_placement_rules().expect("get_placement_rules");
        assert_eq!(rules.len(), 8, "预置放置规则应有 8 条");

        let policies = storage.get_scan_policies().expect("get_scan_policies");
        assert_eq!(policies.len(), 10, "预置扫描策略应有 10 条");
    }

    #[test]
    fn test_assets_crud_and_fts() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let asset = AssetRecord {
            id: "test-asset-1".to_string(),
            path: "/root/new/shared_proj/sec_fetcher".to_string(),
            asset_type: "script".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("SEC EDGAR financial scraper tool".to_string()),
            desc_kv: serde_json::json!({"purpose": "finance_tool", "domain": "sec"}),
            usage: Some("python main.py".to_string()),
            size_bytes: Some(1024),
            file_count: Some(1),
            tags: vec!["python".to_string(), "sec".to_string()],
            status: "active".to_string(),
        };

        storage.upsert_asset(&asset).expect("upsert_asset");

        let fetched = storage.get_asset_by_path("/root/new/shared_proj/sec_fetcher")
            .expect("get_asset_by_path")
            .expect("should exist");
        assert_eq!(fetched.owner, "user:digoal");
        assert_eq!(fetched.desc_kv["purpose"], "finance_tool");

        let search_res = storage.search_assets_fts("scraper", 10).expect("search_assets_fts");
        assert_eq!(search_res.len(), 1);
        assert_eq!(search_res[0].id, "test-asset-1");
    }

    #[test]
    fn test_active_processes() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let proc = ActiveProcessRecord {
            id: "proc-1".to_string(),
            pid: 14208,
            name: "python3".to_string(),
            command: "python scan_sec.py".to_string(),
            owner: "agent:hermes_cron".to_string(),
            associated_path: Some("/root/new/shared_proj/scan_sec.py".to_string()),
            listening_ports: vec![8080],
            status: "running".to_string(),
            cpu_pct: 95.5,
            mem_bytes: 1024 * 1024 * 50,
            gpu_allocated_mb: 0,
            started_at: Some("2026-11-15T08:30:00Z".to_string()),
        };

        storage.replace_active_processes(&[proc]).expect("replace_active_processes");
        let list = storage.list_active_processes().expect("list_active_processes");
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].pid, 14208);
        assert_eq!(list[0].owner, "agent:hermes_cron");
    }

    #[test]
    fn test_scan_jobs_crud() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let job = ScanJobRecord {
            id: "job-test-1".to_string(),
            root_path: "/root/new/work".to_string(),
            status: "pending".to_string(),
            last_path: None,
            scanned_dirs: 0,
            scanned_files: 0,
            total_assets: 0,
            shallow_blocked: 0,
            error_message: None,
            started_at: None,
            paused_at: None,
            completed_at: None,
            created_at: None,
            updated_at: None,
        };

        storage.create_scan_job(&job).expect("create_scan_job");

        let fetched = storage.get_scan_job("job-test-1").expect("get_scan_job").expect("must exist");
        assert_eq!(fetched.status, "pending");
        assert_eq!(fetched.root_path, "/root/new/work");

        let mut updated = fetched;
        updated.status = "running".to_string();
        updated.last_path = Some("/root/new/work/proj1".to_string());
        updated.scanned_dirs = 5;
        updated.scanned_files = 20;
        updated.total_assets = 25;
        storage.update_scan_job(&updated).expect("update_scan_job");

        let active = storage.get_active_scan_job().expect("get_active").expect("should have active");
        assert_eq!(active.id, "job-test-1");
        assert_eq!(active.status, "running");
        assert_eq!(active.scanned_dirs, 5);
        assert_eq!(active.last_path.as_deref(), Some("/root/new/work/proj1"));

        let list = storage.list_scan_jobs(10).expect("list_scan_jobs");
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn test_property_graph_crud_and_traversal() {
        let storage = Storage::open_in_memory().expect("open_in_memory");

        // 1. 创建节点: 项目 -> 服务 -> 端口
        let p_node = FactNodeRecord {
            id: "proj:agent-steward".to_string(),
            node_type: "project".to_string(),
            name: "agent-steward".to_string(),
            label: "宿主环境大管家工程".to_string(),
            properties: serde_json::json!({"path": "/root/new/work/agent-steward"}),
            updated_at: None,
        };
        let s_node = FactNodeRecord {
            id: "service:agent-steward.service".to_string(),
            node_type: "service".to_string(),
            name: "agent-steward.service".to_string(),
            label: "大管家守护服务".to_string(),
            properties: serde_json::json!({"unit": "/etc/systemd/system/agent-steward.service"}),
            updated_at: None,
        };
        let port_node = FactNodeRecord {
            id: "port:8080".to_string(),
            node_type: "port".to_string(),
            name: "8080".to_string(),
            label: "TCP 8080端口".to_string(),
            properties: serde_json::json!({"proto": "tcp"}),
            updated_at: None,
        };

        storage.upsert_fact_node(&p_node).expect("upsert proj");
        storage.upsert_fact_node(&s_node).expect("upsert service");
        storage.upsert_fact_node(&port_node).expect("upsert port");

        // 2. 创建边
        let edge1 = FactEdgeRecord {
            id: None,
            source_id: "service:agent-steward.service".to_string(),
            target_id: "proj:agent-steward".to_string(),
            relation: "depends_on".to_string(),
            properties: serde_json::json!({}),
            confidence: 1.0,
            updated_at: None,
        };
        let edge2 = FactEdgeRecord {
            id: None,
            source_id: "service:agent-steward.service".to_string(),
            target_id: "port:8080".to_string(),
            relation: "listens_on".to_string(),
            properties: serde_json::json!({}),
            confidence: 1.0,
            updated_at: None,
        };

        storage.upsert_fact_edge(&edge1).expect("edge 1");
        storage.upsert_fact_edge(&edge2).expect("edge 2");

        // 3. 验证从服务节点出发的 1 跳子图
        let sub1 = storage.traverse_fact_graph("service:agent-steward.service", 1).expect("subgraph 1 hop");
        assert_eq!(sub1.nodes.len(), 3, "1跳应该连同项目和端口全部捕获");
        assert_eq!(sub1.edges.len(), 2, "应包含2条边");

        // 4. 验证根据名字模糊查找节点
        let found = storage.find_fact_nodes_by_name("steward").expect("find nodes");
        assert_eq!(found.len(), 2);
    }
}
