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
        let mut stmt = conn.prepare(
            "SELECT a.id, a.path, a.type, a.owner, a.security_level, a.scan_tier, a.description, a.desc_kv, a.usage, a.size_bytes, a.file_count, a.tags, a.status
             FROM assets_fts f
             JOIN assets a ON a.rowid = f.rowid
             WHERE assets_fts MATCH ?1
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![query, limit as i64], |row| {
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

        conn.execute(
            "INSERT INTO memories (id, type, content, summary, source_agent, source_session, tags, weight, pinned, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
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
            ],
        )?;
        Ok(())
    }

    pub fn search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT m.id, m.type, m.content, m.summary, m.source_agent, m.source_session, m.tags, m.weight, m.pinned, m.status
             FROM memories_fts f
             JOIN memories m ON m.rowid = f.rowid
             WHERE memories_fts MATCH ?1
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![query, limit as i64], |row| {
            let tags_str: String = row.get(6)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();

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
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
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
}
