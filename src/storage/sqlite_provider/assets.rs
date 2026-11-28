use super::{SqliteStorageProvider, SCHEMA_SQL};
use anyhow::{Context, Result};
use rusqlite::params;
use crate::storage::models::*;

impl SqliteStorageProvider {
    pub(super) fn impl_init(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute_batch(SCHEMA_SQL)
            .context("执行内置 SQLite Schema 批处理失败")?;
        // 幂等无损迁移兼容已存在历史数据库（自动识别重命名列并补齐缺失列）
        super::schema_reconcile::reconcile_sqlite_schema(&conn)
            .context("执行 SQLite Schema 历史库自愈迁移失败")?;
        conn.execute("CREATE TABLE IF NOT EXISTS scan_jobs (
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
        )", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_scan_jobs_status ON scan_jobs(status)", [])?;

        conn.execute("CREATE TABLE IF NOT EXISTS fact_nodes (
            id          TEXT PRIMARY KEY,
            node_type   TEXT NOT NULL CHECK (node_type IN ('project','tool','port','service','cron','file','authoritative_site','section','domain_term','knowhow')),
            name        TEXT NOT NULL,
            label       TEXT NOT NULL,
            properties  TEXT DEFAULT '{}',
            embedding   TEXT DEFAULT '[]',
            updated_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
        )", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_fact_nodes_type ON fact_nodes(node_type)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_fact_nodes_name ON fact_nodes(name)", [])?;

        let existing_node_ddl: String = conn
            .query_row(
                "SELECT COALESCE(sql, '') FROM sqlite_master WHERE type='table' AND name='fact_nodes'",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default();
        if !existing_node_ddl.is_empty() && !existing_node_ddl.contains("domain_term") {
            if let Err(e) = conn.execute_batch(
                "PRAGMA foreign_keys = OFF;
                 BEGIN IMMEDIATE;
                 CREATE TABLE fact_nodes_v61 (
                     id          TEXT PRIMARY KEY,
                     node_type   TEXT NOT NULL CHECK (node_type IN ('project','tool','port','service','cron','file','authoritative_site','section','domain_term','knowhow')),
                     name        TEXT NOT NULL,
                     label       TEXT NOT NULL,
                     properties  TEXT DEFAULT '{}',
                     embedding   TEXT DEFAULT '[]',
                     updated_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
                 );
                 INSERT OR IGNORE INTO fact_nodes_v61 (id, node_type, name, label, properties, embedding, updated_at)
                 SELECT id, node_type, name, label, properties, COALESCE(embedding, '[]'), updated_at FROM fact_nodes;
                 DROP TABLE fact_nodes;
                 ALTER TABLE fact_nodes_v61 RENAME TO fact_nodes;
                 CREATE INDEX IF NOT EXISTS idx_fact_nodes_type ON fact_nodes(node_type);
                 CREATE INDEX IF NOT EXISTS idx_fact_nodes_name ON fact_nodes(name);
                 COMMIT;
                 PRAGMA foreign_keys = ON;",
            ) {
                eprintln!("[SQLite][WARN] fact_nodes 表结构升级跳过: {}", e);
            }
        }

        conn.execute("CREATE TABLE IF NOT EXISTS fact_edges (
            id             INTEGER PRIMARY KEY AUTOINCREMENT,
            source_id      TEXT NOT NULL,
            target_id      TEXT NOT NULL,
            relation       TEXT NOT NULL CHECK (relation IN ('depends_on','listens_on','scheduled_by','reads','writes','part_of','calls','contains','uses_tool','runs_as','documented_by','semantically_related','mentions','co_occurs_with','causes','mitigates','contrasts_with','applies_when','supersedes')),
            properties     TEXT DEFAULT '{}',
            confidence     REAL DEFAULT 1.0,
            evidence_tier  TEXT NOT NULL DEFAULT 'L1_EXPLICIT',
            weight         REAL NOT NULL DEFAULT 1.0,
            direction_kind TEXT NOT NULL DEFAULT 'forward_dependency',
            updated_at     TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
            UNIQUE(source_id, target_id, relation),
            FOREIGN KEY(source_id) REFERENCES fact_nodes(id) ON DELETE CASCADE,
            FOREIGN KEY(target_id) REFERENCES fact_nodes(id) ON DELETE CASCADE
        )", [])?;

        // 幂等热迁移老库 fact_edges（补充 evidence_tier/weight/direction_kind 并放开 Phase 61 知识网络关系 CHECK 约束）
        let existing_ddl: String = conn
            .query_row(
                "SELECT COALESCE(sql, '') FROM sqlite_master WHERE type='table' AND name='fact_edges'",
                [],
                |row| row.get(0),
            )
            .unwrap_or_default();
        if !existing_ddl.is_empty()
            && (!existing_ddl.contains("evidence_tier")
                || !existing_ddl.contains("semantically_related")
                || !existing_ddl.contains("co_occurs_with"))
        {
            let has_tier_col = existing_ddl.contains("evidence_tier");
            let copy_sql = if has_tier_col {
                "SELECT id, source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at FROM fact_edges"
            } else {
                "SELECT id, source_id, target_id, relation, properties, confidence, 'L1_EXPLICIT', confidence,
                        CASE relation WHEN 'scheduled_by' THEN 'reverse_schedule' WHEN 'part_of' THEN 'derivation_flow' WHEN 'contains' THEN 'derivation_flow' ELSE 'forward_dependency' END,
                        updated_at
                 FROM fact_edges"
            };
            let migration_sql = format!(
                "PRAGMA foreign_keys = OFF;
                 BEGIN IMMEDIATE;
                 CREATE TABLE fact_edges_v61 (
                     id             INTEGER PRIMARY KEY AUTOINCREMENT,
                     source_id      TEXT NOT NULL,
                     target_id      TEXT NOT NULL,
                     relation       TEXT NOT NULL CHECK (relation IN ('depends_on','listens_on','scheduled_by','reads','writes','part_of','calls','contains','uses_tool','runs_as','documented_by','semantically_related','mentions','co_occurs_with','causes','mitigates','contrasts_with','applies_when','supersedes')),
                     properties     TEXT DEFAULT '{{}}',
                     confidence     REAL DEFAULT 1.0,
                     evidence_tier  TEXT NOT NULL DEFAULT 'L1_EXPLICIT',
                     weight         REAL NOT NULL DEFAULT 1.0,
                     direction_kind TEXT NOT NULL DEFAULT 'forward_dependency',
                     updated_at     TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
                     UNIQUE(source_id, target_id, relation),
                     FOREIGN KEY(source_id) REFERENCES fact_nodes(id) ON DELETE CASCADE,
                     FOREIGN KEY(target_id) REFERENCES fact_nodes(id) ON DELETE CASCADE
                 );
                 INSERT OR IGNORE INTO fact_edges_v61 (id, source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at)
                 {};
                 DROP TABLE fact_edges;
                 ALTER TABLE fact_edges_v61 RENAME TO fact_edges;
                 COMMIT;
                 PRAGMA foreign_keys = ON;",
                copy_sql
            );
            if let Err(e) = conn.execute_batch(&migration_sql) {
                eprintln!("[SQLite][WARN] fact_edges 表结构升级跳过: {}", e);
            }
        }

        conn.execute("CREATE INDEX IF NOT EXISTS idx_fact_edges_source ON fact_edges(source_id)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_fact_edges_target ON fact_edges(target_id)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_fact_edges_relation ON fact_edges(relation)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_fact_edges_tier ON fact_edges(evidence_tier)", [])?;

        let base = crate::config::default_workspace_base();
        let home = crate::config::home_dir();
        for r in crate::storage::default_seed_placement_rules(&base) {
            conn.execute(
                "INSERT OR IGNORE INTO placement_rules (id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    r.id,
                    r.rule_name,
                    r.pattern_type,
                    r.target_base_dir,
                    r.naming_pattern,
                    r.description,
                    r.guideline,
                    r.priority,
                ],
            )?;
        }
        for p in crate::storage::default_seed_scan_policies(&base, &home) {
            conn.execute(
                "INSERT OR IGNORE INTO scan_policies (id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    p.id,
                    p.path_pattern,
                    p.tier,
                    p.max_depth,
                    if p.enable_fts { 1 } else { 0 },
                    if p.enable_vec { 1 } else { 0 },
                    if p.enable_graph { 1 } else { 0 },
                    p.notes,
                ],
            )?;
        }

        Ok(())
    }


    pub(super) fn impl_provider_name(&self) -> &'static str {
        "sqlite"
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


    pub(super) fn impl_insert_placement_rule(&self, rule: &PlacementRule) -> Result<()> {
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


    pub(super) fn impl_get_scan_policies(&self) -> Result<Vec<ScanPolicy>> {
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


    pub(super) fn impl_upsert_scan_policy(&self, policy: &ScanPolicy) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "DELETE FROM scan_policies WHERE id = ?1 OR path_pattern = ?2",
            params![policy.id, policy.path_pattern],
        )?;
        conn.execute(
            "INSERT INTO scan_policies (id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                policy.id,
                policy.path_pattern,
                policy.tier,
                policy.max_depth,
                policy.enable_fts as i32,
                policy.enable_vec as i32,
                policy.enable_graph as i32,
                policy.notes,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_upsert_asset(&self, asset: &AssetRecord) -> Result<()> {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::IoSqliteWrite);
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
        let candidates = super::build_fts_query_candidates(query);
        if candidates.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT a.id, a.path, a.type, a.owner, a.security_level, a.scan_tier, a.description, a.desc_kv, a.usage, a.size_bytes, a.file_count, a.tags, a.status
             FROM assets_fts f
             JOIN assets a ON f.rowid = a.rowid
             WHERE assets_fts MATCH ?1 AND a.status = 'active'
             ORDER BY bm25(assets_fts, 4.0, 2.5, 2.0, 1.5, 2.0)
             LIMIT ?2"
        )?;

        let mut merged: Vec<AssetRecord> = Vec::new();
        let mut seen_paths = std::collections::HashSet::new();
        for fts_q in &candidates {
            let Ok(mapped) = stmt.query_map(params![fts_q, limit as i64], |row| {
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
            }) else {
                continue;
            };
            for row in mapped.flatten() {
                if seen_paths.insert(row.path.clone()) {
                    merged.push(row);
                    if merged.len() >= limit {
                        return Ok(merged);
                    }
                }
            }
            if !merged.is_empty() {
                break;
            }
        }

        Ok(merged)
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
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_replace_active_processes(&self, procs: &[ActiveProcessRecord]) -> Result<()> {
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


    pub(super) fn impl_list_active_processes(&self) -> Result<Vec<ActiveProcessRecord>> {
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


    pub(super) fn impl_create_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
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


    pub(super) fn impl_update_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
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


    pub(super) fn impl_get_active_scan_job(&self) -> Result<Option<ScanJobRecord>> {
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


    pub(super) fn impl_update_asset_embedding(&self, path: &str, embedding: &[f32]) -> Result<()> {
        let conn = self.lock_conn()?;
        let emb_str = serde_json::to_string(embedding)?;
        conn.execute(
            "UPDATE assets SET embedding = ?1, updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now') WHERE path = ?2",
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
