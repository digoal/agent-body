use super::{DuckDbStorageProvider, DUCKDB_SCHEMA_SQL};
use anyhow::{Context, Result};
use duckdb::params;
use crate::storage::models::*;

impl DuckDbStorageProvider {
    pub(super) fn impl_init(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute_batch(DUCKDB_SCHEMA_SQL)
            .context("执行 DuckDB Schema 批处理失败")?;
        conn.execute_batch(
            "ALTER TABLE assets ADD COLUMN IF NOT EXISTS embedding VARCHAR DEFAULT '[]';
             ALTER TABLE memories ADD COLUMN IF NOT EXISTS sensitive BOOLEAN DEFAULT FALSE;
             ALTER TABLE fact_nodes ADD COLUMN IF NOT EXISTS embedding VARCHAR DEFAULT '[]';
             ALTER TABLE fact_edges ADD COLUMN IF NOT EXISTS evidence_tier VARCHAR DEFAULT 'L1_EXPLICIT';
             ALTER TABLE fact_edges ADD COLUMN IF NOT EXISTS weight FLOAT DEFAULT 1.0;
             ALTER TABLE fact_edges ADD COLUMN IF NOT EXISTS direction_kind VARCHAR DEFAULT 'forward_dependency';",
        )
        .context("执行 DuckDB 增量列迁移失败")?;

        let now = chrono::Utc::now().to_rfc3339();
        let base = crate::config::default_workspace_base();
        let home = crate::config::home_dir();

        // 预置放置规则（使用 ON CONFLICT DO NOTHING 避免覆写用户自定义规则）
        for r in crate::storage::default_seed_placement_rules(&base) {
            conn.execute(
                "INSERT INTO placement_rules (id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
                 ON CONFLICT DO NOTHING",
                params![
                    r.id,
                    r.rule_name,
                    r.pattern_type,
                    r.target_base_dir,
                    r.naming_pattern,
                    r.description,
                    r.guideline,
                    r.priority,
                    now
                ],
            )?;
        }

        // 预置扫描策略（同样仅缺失时补齐，绝不覆盖用户策略）
        for p in crate::storage::default_seed_scan_policies(&base, &home) {
            conn.execute(
                "INSERT INTO scan_policies (id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 ON CONFLICT DO NOTHING",
                params![
                    p.id,
                    p.path_pattern,
                    p.tier,
                    p.max_depth,
                    p.enable_fts,
                    p.enable_vec,
                    p.enable_graph,
                    p.notes,
                    now
                ],
            )?;
        }

        Ok(())
    }


    pub(super) fn impl_provider_name(&self) -> &'static str {
        "duckdb"
    }


    pub(super) fn impl_list_created_at_map(&self, table: &str) -> Result<std::collections::HashMap<String, String>> {
        let conn = self.lock_conn()?;
        let sql = match table {
            "assets" => "SELECT id, created_at FROM assets WHERE created_at IS NOT NULL",
            "memories" => "SELECT id, created_at FROM memories WHERE created_at IS NOT NULL",
            _ => anyhow::bail!("不支持批量提取 created_at 的表: {}", table),
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
        let mut map = std::collections::HashMap::new();
        for r in rows {
            let (id, ts) = r?;
            map.insert(id, ts);
        }
        Ok(map)
    }


    pub(super) fn impl_get_placement_rules(&self) -> Result<Vec<PlacementRule>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority 
             FROM placement_rules ORDER BY priority ASC"
        )?;

        let rows = stmt.query_map([], |row| {
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
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_insert_placement_rule(&self, rule: &PlacementRule) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO placement_rules (id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, strftime(now(), '%Y-%m-%dT%H:%M:%SZ'), strftime(now(), '%Y-%m-%dT%H:%M:%SZ'))
             ON CONFLICT(rule_name) DO UPDATE SET
                pattern_type = excluded.pattern_type,
                target_base_dir = excluded.target_base_dir,
                naming_pattern = excluded.naming_pattern,
                description = excluded.description,
                guideline = excluded.guideline,
                priority = excluded.priority,
                updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ')",
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


    pub(super) fn impl_get_scan_policies(&self) -> Result<Vec<ScanPolicy>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes 
             FROM scan_policies"
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ScanPolicy {
                id: row.get(0)?,
                path_pattern: row.get(1)?,
                tier: row.get(2)?,
                max_depth: row.get(3)?,
                enable_fts: row.get::<_, bool>(4)?,
                enable_vec: row.get::<_, bool>(5)?,
                enable_graph: row.get::<_, bool>(6)?,
                notes: row.get(7)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_upsert_scan_policy(&self, policy: &ScanPolicy) -> Result<()> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute(
            "DELETE FROM scan_policies WHERE id = ?1 OR path_pattern = ?2",
            params![policy.id, policy.path_pattern],
        )?;
        conn.execute(
            "INSERT INTO scan_policies (id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                policy.id,
                policy.path_pattern,
                policy.tier,
                policy.max_depth,
                policy.enable_fts,
                policy.enable_vec,
                policy.enable_graph,
                policy.notes,
                now,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_upsert_asset(&self, asset: &AssetRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let desc_kv_str = serde_json::to_string(&asset.desc_kv)?;
        let tags_str = serde_json::to_string(&asset.tags)?;
        let invalidate_embedding = asset
            .desc_kv
            .get("embedding_status")
            .and_then(|v| v.as_str())
            == Some("stale")
            || asset.status == "missing";
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO assets (id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?15, ?15)
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
                embedding = CASE WHEN ?14 THEN '[]' ELSE assets.embedding END,
                updated_at = excluded.updated_at",
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
                now,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_asset_by_path(&self, path: &str) -> Result<Option<AssetRecord>> {
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


    pub(super) fn impl_search_assets_fts(&self, query: &str, limit: usize) -> Result<Vec<AssetRecord>> {
        let conn = self.lock_conn()?;
        let query_pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets
             WHERE status = 'active' AND (path ILIKE ?1 OR description ILIKE ?1 OR usage ILIKE ?1 OR tags ILIKE ?1)
             ORDER BY updated_at DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![query_pattern, limit as i64], |row| {
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
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_list_assets(&self, limit: usize) -> Result<Vec<AssetRecord>> {
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
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_replace_active_processes(&self, procs: &[ActiveProcessRecord]) -> Result<()> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute_batch("BEGIN TRANSACTION;")?;
        let res: Result<()> = (|| {
            conn.execute("DELETE FROM active_processes", [])?;
            for p in procs {
                let ports_str = serde_json::to_string(&p.listening_ports)?;
                conn.execute(
                    "INSERT INTO active_processes (id, pid, name, command, owner, associated_path, listening_ports, status, cpu_pct, mem_bytes, gpu_allocated_mb, started_at, last_seen_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                    params![
                        p.id,
                        p.pid as i32,
                        p.name,
                        p.command,
                        p.owner,
                        p.associated_path,
                        ports_str,
                        p.status,
                        p.cpu_pct as f64,
                        p.mem_bytes as i64,
                        p.gpu_allocated_mb as i32,
                        p.started_at,
                        now,
                    ],
                )?;
            }
            Ok(())
        })();
        match res {
            Ok(()) => {
                conn.execute_batch("COMMIT;")?;
                Ok(())
            }
            Err(e) => {
                if let Err(rb) = conn.execute_batch("ROLLBACK;") {
                    eprintln!("[DuckDB][WARN] replace_active_processes ROLLBACK 失败: {}", rb);
                }
                Err(e)
            }
        }
    }


    pub(super) fn impl_list_active_processes(&self) -> Result<Vec<ActiveProcessRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, pid, name, command, owner, associated_path, listening_ports, status, cpu_pct, mem_bytes, gpu_allocated_mb, started_at, last_seen_at
             FROM active_processes ORDER BY pid ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            let ports_str: String = row.get(6)?;
            let ports: Vec<u16> = serde_json::from_str(&ports_str).unwrap_or_default();
            let pid_i: i32 = row.get(1)?;
            let cpu_f: f64 = row.get(8)?;
            let mem_i: i64 = row.get(9)?;
            let gpu_i: i32 = row.get(10)?;

            Ok(ActiveProcessRecord {
                id: row.get(0)?,
                pid: pid_i as u32,
                name: row.get(2)?,
                command: row.get(3)?,
                owner: row.get(4)?,
                associated_path: row.get(5)?,
                listening_ports: ports,
                status: row.get(7)?,
                cpu_pct: cpu_f as f32,
                mem_bytes: mem_i as u64,
                gpu_allocated_mb: gpu_i as u32,
                started_at: row.get(11)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_create_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO scan_jobs (id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
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
                job.created_at,
                job.updated_at,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_update_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "UPDATE scan_jobs SET
                status = ?1,
                last_path = ?2,
                scanned_dirs = ?3,
                scanned_files = ?4,
                total_assets = ?5,
                shallow_blocked = ?6,
                error_message = ?7,
                started_at = ?8,
                paused_at = ?9,
                completed_at = ?10,
                updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ')
             WHERE id = ?11",
            params![
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
                job.id,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_scan_job(&self, id: &str) -> Result<Option<ScanJobRecord>> {
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


    pub(super) fn impl_list_scan_jobs(&self, limit: usize) -> Result<Vec<ScanJobRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs ORDER BY created_at DESC LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
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
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_get_active_scan_job(&self) -> Result<Option<ScanJobRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs WHERE status IN ('running', 'paused') ORDER BY created_at DESC LIMIT 1"
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


    pub(super) fn impl_update_asset_embedding(&self, path: &str, embedding: &[f32]) -> Result<()> {
        let conn = self.lock_conn()?;
        let emb_str = serde_json::to_string(embedding)?;
        conn.execute(
            "UPDATE assets SET embedding = ?1, updated_at = strftime(now(), '%Y-%m-%dT%H:%M:%SZ') WHERE path = ?2",
            params![emb_str, path],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_asset_embedding(&self, path: &str) -> Result<Vec<f32>> {
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


}
