use super::{PgStorageProvider, PG_SCHEMA_SQL};
use anyhow::{Context, Result};
use crate::storage::models::*;

impl PgStorageProvider {
    pub(super) fn impl_init(&self) -> Result<()> {
        let mut client = self.lock_client()?;
        client.batch_execute(PG_SCHEMA_SQL)
            .context("执行内置 PostgreSQL Schema 批处理失败")?;
        if let Err(e) = client.batch_execute(
            "ALTER TABLE assets ADD COLUMN IF NOT EXISTS embedding TEXT DEFAULT '[]';
             ALTER TABLE memories ADD COLUMN IF NOT EXISTS sensitive BOOLEAN DEFAULT FALSE;
             ALTER TABLE fact_nodes ADD COLUMN IF NOT EXISTS embedding TEXT DEFAULT '[]';
             ALTER TABLE fact_edges ADD COLUMN IF NOT EXISTS evidence_tier TEXT NOT NULL DEFAULT 'L1_EXPLICIT';
             ALTER TABLE fact_edges ADD COLUMN IF NOT EXISTS weight REAL NOT NULL DEFAULT 1.0;
             ALTER TABLE fact_edges ADD COLUMN IF NOT EXISTS direction_kind TEXT NOT NULL DEFAULT 'forward_dependency';
             CREATE INDEX IF NOT EXISTS idx_pg_fact_edges_tier ON fact_edges(evidence_tier);
             DO $$
             DECLARE
                 old_col TEXT;
             BEGIN
                 IF NOT EXISTS (
                     SELECT 1 FROM information_schema.columns
                     WHERE table_schema = current_schema() AND table_name = 'benchmark_runs' AND column_name = 'total_body_tokens'
                 ) THEN
                     SELECT column_name INTO old_col FROM information_schema.columns
                     WHERE table_schema = current_schema() AND table_name = 'benchmark_runs'
                       AND column_name LIKE 'total_%_tokens' AND column_name <> 'total_baseline_tokens'
                     LIMIT 1;
                     IF old_col IS NOT NULL THEN
                         EXECUTE format('ALTER TABLE benchmark_runs RENAME COLUMN %I TO total_body_tokens', old_col);
                     END IF;
                 END IF;
                 IF NOT EXISTS (
                     SELECT 1 FROM information_schema.columns
                     WHERE table_schema = current_schema() AND table_name = 'effectiveness_ledger' AND column_name = 'body_tokens'
                 ) THEN
                     SELECT column_name INTO old_col FROM information_schema.columns
                     WHERE table_schema = current_schema() AND table_name = 'effectiveness_ledger'
                       AND column_name LIKE '%_tokens' AND column_name <> 'baseline_tokens'
                     LIMIT 1;
                     IF old_col IS NOT NULL THEN
                         EXECUTE format('ALTER TABLE effectiveness_ledger RENAME COLUMN %I TO body_tokens', old_col);
                     END IF;
                 END IF;
                 IF NOT EXISTS (
                     SELECT 1 FROM information_schema.columns
                     WHERE table_schema = current_schema() AND table_name = 'effectiveness_ledger' AND column_name = 'body_rounds'
                 ) THEN
                     SELECT column_name INTO old_col FROM information_schema.columns
                     WHERE table_schema = current_schema() AND table_name = 'effectiveness_ledger'
                       AND column_name LIKE '%_rounds' AND column_name <> 'baseline_rounds'
                     LIMIT 1;
                     IF old_col IS NOT NULL THEN
                         EXECUTE format('ALTER TABLE effectiveness_ledger RENAME COLUMN %I TO body_rounds', old_col);
                     END IF;
                 END IF;
                 ALTER TABLE body_checkpoints ADD COLUMN IF NOT EXISTS tx_epoch BIGINT NOT NULL DEFAULT 0;
                 ALTER TABLE body_checkpoints ADD COLUMN IF NOT EXISTS fast_weight_snapshot_id TEXT DEFAULT NULL;
                 ALTER TABLE body_checkpoints ADD COLUMN IF NOT EXISTS fast_weight_base_model TEXT DEFAULT NULL;
             END $$;",
        ) { eprintln!("[PG][WARN] 增量列/索引热迁移告警: {}", e); }

        let base = crate::config::default_workspace_base();
        let home = crate::config::home_dir();

        // 预置放置规范规则
        for r in crate::storage::default_seed_placement_rules(&base) {
            client.execute(
                "INSERT INTO placement_rules (id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                 ON CONFLICT (id) DO NOTHING",
                &[
                    &r.id,
                    &r.rule_name,
                    &r.pattern_type,
                    &r.target_base_dir,
                    &r.naming_pattern,
                    &r.description,
                    &r.guideline,
                    &r.priority,
                ],
            )?;
        }

        // 预置扫描策略
        for p in crate::storage::default_seed_scan_policies(&base, &home) {
            client.execute(
                "INSERT INTO scan_policies (id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                 ON CONFLICT (id) DO NOTHING",
                &[
                    &p.id,
                    &p.path_pattern,
                    &p.tier,
                    &p.max_depth,
                    &p.enable_fts,
                    &p.enable_vec,
                    &p.enable_graph,
                    &p.notes,
                ],
            )?;
        }

        Ok(())
    }


    pub(super) fn impl_provider_name(&self) -> &'static str {
        "postgres"
    }


    pub(super) fn impl_list_created_at_map(&self, table: &str) -> Result<std::collections::HashMap<String, String>> {
        let mut client = self.lock_client()?;
        let sql = match table {
            "assets" => {
                "SELECT id, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')
                 FROM assets WHERE created_at IS NOT NULL"
            }
            "memories" => {
                "SELECT id, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')
                 FROM memories WHERE created_at IS NOT NULL"
            }
            _ => anyhow::bail!("不支持批量提取 created_at 的表: {}", table),
        };
        let rows = client.query(sql, &[])?;
        let mut map = std::collections::HashMap::new();
        for row in rows {
            let id: String = row.get(0);
            let ts: String = row.get(1);
            map.insert(id, ts);
        }
        Ok(map)
    }


    pub(super) fn impl_get_placement_rules(&self) -> Result<Vec<PlacementRule>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority 
             FROM placement_rules ORDER BY priority ASC",
            &[],
        )?;

        let mut rules = Vec::new();
        for row in rows {
            rules.push(PlacementRule {
                id: row.get(0),
                rule_name: row.get(1),
                pattern_type: row.get(2),
                target_base_dir: row.get(3),
                naming_pattern: row.get(4),
                description: row.get(5),
                guideline: row.get(6),
                priority: row.get(7),
            });
        }
        Ok(rules)
    }


    pub(super) fn impl_insert_placement_rule(&self, rule: &PlacementRule) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO placement_rules (id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (rule_name) DO UPDATE SET
                pattern_type = EXCLUDED.pattern_type,
                target_base_dir = EXCLUDED.target_base_dir,
                naming_pattern = EXCLUDED.naming_pattern,
                description = EXCLUDED.description,
                guideline = EXCLUDED.guideline,
                priority = EXCLUDED.priority,
                updated_at = NOW()",
            &[
                &rule.id,
                &rule.rule_name,
                &rule.pattern_type,
                &rule.target_base_dir,
                &rule.naming_pattern,
                &rule.description,
                &rule.guideline,
                &rule.priority,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_scan_policies(&self) -> Result<Vec<ScanPolicy>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes 
             FROM scan_policies",
            &[],
        )?;

        let mut policies = Vec::new();
        for row in rows {
            policies.push(ScanPolicy {
                id: row.get(0),
                path_pattern: row.get(1),
                tier: row.get(2),
                max_depth: row.get(3),
                enable_fts: row.get(4),
                enable_vec: row.get(5),
                enable_graph: row.get(6),
                notes: row.get(7),
            });
        }
        Ok(policies)
    }


    pub(super) fn impl_upsert_scan_policy(&self, policy: &ScanPolicy) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "DELETE FROM scan_policies WHERE id = $1 OR path_pattern = $2",
            &[&policy.id, &policy.path_pattern],
        )?;
        client.execute(
            "INSERT INTO scan_policies (id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            &[
                &policy.id,
                &policy.path_pattern,
                &policy.tier,
                &policy.max_depth,
                &policy.enable_fts,
                &policy.enable_vec,
                &policy.enable_graph,
                &policy.notes,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_upsert_asset(&self, asset: &AssetRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        let invalidate_embedding = asset
            .desc_kv
            .get("embedding_status")
            .and_then(|v| v.as_str())
            == Some("stale")
            || asset.status == "missing";
        client.execute(
            "INSERT INTO assets (id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, NOW())
             ON CONFLICT (path) DO UPDATE SET
                type = EXCLUDED.type,
                owner = EXCLUDED.owner,
                security_level = EXCLUDED.security_level,
                scan_tier = EXCLUDED.scan_tier,
                description = EXCLUDED.description,
                desc_kv = EXCLUDED.desc_kv,
                usage = EXCLUDED.usage,
                size_bytes = EXCLUDED.size_bytes,
                file_count = EXCLUDED.file_count,
                tags = EXCLUDED.tags,
                status = EXCLUDED.status,
                embedding = CASE WHEN $14 THEN '[]' ELSE assets.embedding END,
                content_text = CASE WHEN EXCLUDED.status = 'missing' THEN '' ELSE assets.content_text END,
                updated_at = NOW()",
            &[
                &asset.id,
                &asset.path,
                &asset.asset_type,
                &asset.owner,
                &asset.security_level,
                &asset.scan_tier,
                &asset.description,
                &asset.desc_kv,
                &asset.usage,
                &asset.size_bytes,
                &asset.file_count,
                &serde_json::to_value(&asset.tags)?,
                &asset.status,
                &invalidate_embedding,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_asset_by_path(&self, path: &str) -> Result<Option<AssetRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets WHERE path = $1",
            &[&path],
        )?;

        if let Some(row) = rows.first() {
            let tags_val: serde_json::Value = row.get(11);
            let tags: Vec<String> = serde_json::from_value(tags_val).unwrap_or_default();

            Ok(Some(AssetRecord {
                id: row.get(0),
                path: row.get(1),
                asset_type: row.get(2),
                owner: row.get(3),
                security_level: row.get(4),
                scan_tier: row.get(5),
                description: row.get(6),
                desc_kv: row.get(7),
                usage: row.get(8),
                size_bytes: row.get(9),
                file_count: row.get(10),
                tags,
                status: row.get(12),
            }))
        } else {
            Ok(None)
        }
    }


    pub(super) fn impl_search_assets_fts(&self, query: &str, limit: usize) -> Result<Vec<AssetRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets
             WHERE status = 'active' AND (
                 fts_doc @@ plainto_tsquery('simple', $1)
                 OR path ILIKE '%' || $1 || '%'
                 OR description ILIKE '%' || $1 || '%'
                 OR usage ILIKE '%' || $1 || '%'
             )
             ORDER BY 
                CASE WHEN fts_doc @@ plainto_tsquery('simple', $1) 
                     THEN ts_rank(fts_doc, plainto_tsquery('simple', $1)) + 1.0 
                     ELSE 0.5 
                END DESC,
                updated_at DESC
             LIMIT $2",
            &[&query, &(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let tags_val: serde_json::Value = row.get(11);
            let tags: Vec<String> = serde_json::from_value(tags_val).unwrap_or_default();

            results.push(AssetRecord {
                id: row.get(0),
                path: row.get(1),
                asset_type: row.get(2),
                owner: row.get(3),
                security_level: row.get(4),
                scan_tier: row.get(5),
                description: row.get(6),
                desc_kv: row.get(7),
                usage: row.get(8),
                size_bytes: row.get(9),
                file_count: row.get(10),
                tags,
                status: row.get(12),
            });
        }
        Ok(results)
    }


    pub(super) fn impl_list_assets(&self, limit: usize) -> Result<Vec<AssetRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets
             ORDER BY updated_at DESC
             LIMIT $1",
            &[&(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let tags_val: serde_json::Value = row.get(11);
            let tags: Vec<String> = serde_json::from_value(tags_val).unwrap_or_default();

            results.push(AssetRecord {
                id: row.get(0),
                path: row.get(1),
                asset_type: row.get(2),
                owner: row.get(3),
                security_level: row.get(4),
                scan_tier: row.get(5),
                description: row.get(6),
                desc_kv: row.get(7),
                usage: row.get(8),
                size_bytes: row.get(9),
                file_count: row.get(10),
                tags,
                status: row.get(12),
            });
        }
        Ok(results)
    }


    pub(super) fn impl_replace_active_processes(&self, procs: &[ActiveProcessRecord]) -> Result<()> {
        let mut client = self.lock_client()?;
        let mut tx = client.transaction()?;

        tx.execute("DELETE FROM active_processes", &[])?;

        for p in procs {
            tx.execute(
                "INSERT INTO active_processes (id, pid, name, command, owner, associated_path, listening_ports, status, cpu_pct, mem_bytes, gpu_allocated_mb, started_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)",
                &[
                    &p.id,
                    &(p.pid as i32),
                    &p.name,
                    &p.command,
                    &p.owner,
                    &p.associated_path,
                    &serde_json::to_value(&p.listening_ports)?,
                    &p.status,
                    &p.cpu_pct,
                    &(p.mem_bytes as i64),
                    &(p.gpu_allocated_mb as i32),
                    &p.started_at,
                ],
            )?;
        }

        tx.commit()?;
        Ok(())
    }


    pub(super) fn impl_list_active_processes(&self) -> Result<Vec<ActiveProcessRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, pid, name, command, owner, associated_path, listening_ports, status, cpu_pct, mem_bytes, gpu_allocated_mb, started_at
             FROM active_processes ORDER BY cpu_pct DESC",
            &[],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let pid_i32: i32 = row.get(1);
            let ports_val: serde_json::Value = row.get(6);
            let listening_ports: Vec<u16> = serde_json::from_value(ports_val).unwrap_or_default();
            let mem_bytes_i64: i64 = row.get(9);
            let gpu_mb: i32 = row.get(10);

            results.push(ActiveProcessRecord {
                id: row.get(0),
                pid: pid_i32 as u32,
                name: row.get(2),
                command: row.get(3),
                owner: row.get(4),
                associated_path: row.get(5),
                listening_ports,
                status: row.get(7),
                cpu_pct: row.get(8),
                mem_bytes: mem_bytes_i64 as u64,
                gpu_allocated_mb: gpu_mb as u32,
                started_at: row.get(11),
            });
        }
        Ok(results)
    }


    pub(super) fn impl_create_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO scan_jobs (id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, NOW()::text, NOW()::text)",
            &[
                &job.id,
                &job.root_path,
                &job.status,
                &job.last_path,
                &job.scanned_dirs,
                &job.scanned_files,
                &job.total_assets,
                &job.shallow_blocked,
                &job.error_message,
                &job.started_at,
                &job.paused_at,
                &job.completed_at,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_update_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "UPDATE scan_jobs SET
                status = $2,
                last_path = $3,
                scanned_dirs = $4,
                scanned_files = $5,
                total_assets = $6,
                shallow_blocked = $7,
                error_message = $8,
                started_at = $9,
                paused_at = $10,
                completed_at = $11,
                updated_at = NOW()::text
             WHERE id = $1",
            &[
                &job.id,
                &job.status,
                &job.last_path,
                &job.scanned_dirs,
                &job.scanned_files,
                &job.total_assets,
                &job.shallow_blocked,
                &job.error_message,
                &job.started_at,
                &job.paused_at,
                &job.completed_at,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_scan_job(&self, id: &str) -> Result<Option<ScanJobRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs WHERE id = $1",
            &[&id],
        )?;

        if let Some(row) = rows.first() {
            Ok(Some(ScanJobRecord {
                id: row.get(0),
                root_path: row.get(1),
                status: row.get(2),
                last_path: row.get(3),
                scanned_dirs: row.get(4),
                scanned_files: row.get(5),
                total_assets: row.get(6),
                shallow_blocked: row.get(7),
                error_message: row.get(8),
                started_at: row.get(9),
                paused_at: row.get(10),
                completed_at: row.get(11),
                created_at: row.get(12),
                updated_at: row.get(13),
            }))
        } else {
            Ok(None)
        }
    }


    pub(super) fn impl_list_scan_jobs(&self, limit: usize) -> Result<Vec<ScanJobRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs ORDER BY created_at DESC LIMIT $1",
            &[&(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            results.push(ScanJobRecord {
                id: row.get(0),
                root_path: row.get(1),
                status: row.get(2),
                last_path: row.get(3),
                scanned_dirs: row.get(4),
                scanned_files: row.get(5),
                total_assets: row.get(6),
                shallow_blocked: row.get(7),
                error_message: row.get(8),
                started_at: row.get(9),
                paused_at: row.get(10),
                completed_at: row.get(11),
                created_at: row.get(12),
                updated_at: row.get(13),
            });
        }
        Ok(results)
    }


    pub(super) fn impl_get_active_scan_job(&self) -> Result<Option<ScanJobRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, root_path, status, last_path, scanned_dirs, scanned_files, total_assets, shallow_blocked, error_message, started_at, paused_at, completed_at, created_at, updated_at
             FROM scan_jobs WHERE status IN ('pending', 'running', 'paused') ORDER BY updated_at DESC LIMIT 1",
            &[],
        )?;

        if let Some(row) = rows.first() {
            Ok(Some(ScanJobRecord {
                id: row.get(0),
                root_path: row.get(1),
                status: row.get(2),
                last_path: row.get(3),
                scanned_dirs: row.get(4),
                scanned_files: row.get(5),
                total_assets: row.get(6),
                shallow_blocked: row.get(7),
                error_message: row.get(8),
                started_at: row.get(9),
                paused_at: row.get(10),
                completed_at: row.get(11),
                created_at: row.get(12),
                updated_at: row.get(13),
            }))
        } else {
            Ok(None)
        }
    }


    pub(super) fn impl_update_asset_embedding(&self, path: &str, embedding: &[f32]) -> Result<()> {
        let mut client = self.lock_client()?;
        let emb_str = serde_json::to_string(embedding)?;
        client.execute(
            "UPDATE assets SET embedding = $1, updated_at = NOW() WHERE path = $2",
            &[&emb_str, &path],
        )?;
        Ok(())
    }


    pub(super) fn impl_get_asset_embedding(&self, path: &str) -> Result<Vec<f32>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT COALESCE(embedding, '[]') FROM assets WHERE path = $1",
            &[&path],
        )?;
        if let Some(row) = rows.first() {
            let s: String = row.get(0);
            Ok(serde_json::from_str(&s).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }


}
