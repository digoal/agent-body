use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use postgres::{Client, NoTls};
use crate::storage::models::*;
use crate::storage::spi::StorageProvider;

pub const PG_SCHEMA_SQL: &str = include_str!("pg_schema.sql");

#[derive(Clone)]
pub struct PgStorageProvider {
    client: Arc<Mutex<Client>>,
}

impl PgStorageProvider {
    /// 连接到 PostgreSQL 数据库并自动执行 Schema 初始化
    pub fn connect(url: &str) -> Result<Self> {
        let url_owned = url.to_string();
        let client = std::thread::spawn(move || {
            Client::connect(&url_owned, NoTls)
        })
        .join()
        .map_err(|_| anyhow::anyhow!("连接 PostgreSQL 线程异常退出"))?
        .with_context(|| format!("连接 PostgreSQL 数据库失败: {}", url))?;

        let provider = Self {
            client: Arc::new(Mutex::new(client)),
        };
        provider.init()?;
        Ok(provider)
    }

    /// 基于已建立的 Client 构造
    pub fn from_client(client: Client) -> Result<Self> {
        let provider = Self {
            client: Arc::new(Mutex::new(client)),
        };
        provider.init()?;
        Ok(provider)
    }

    fn lock_client(&self) -> Result<std::sync::MutexGuard<'_, Client>> {
        self.client
            .lock()
            .map_err(|e| anyhow::anyhow!("PostgreSQL 连接锁异常 (mutex poisoned): {}", e))
    }
}

impl StorageProvider for PgStorageProvider {
    fn init(&self) -> Result<()> {
        let mut client = self.lock_client()?;
        client.batch_execute(PG_SCHEMA_SQL)
            .context("执行内置 PostgreSQL Schema 批处理失败")?;
        let _ = client.batch_execute(
            "ALTER TABLE assets ADD COLUMN IF NOT EXISTS embedding TEXT DEFAULT '[]';
             ALTER TABLE memories ADD COLUMN IF NOT EXISTS sensitive BOOLEAN DEFAULT FALSE;
             ALTER TABLE fact_nodes ADD COLUMN IF NOT EXISTS embedding TEXT DEFAULT '[]';
             ALTER TABLE fact_edges ADD COLUMN IF NOT EXISTS evidence_tier TEXT NOT NULL DEFAULT 'L1_EXPLICIT';
             ALTER TABLE fact_edges ADD COLUMN IF NOT EXISTS weight REAL NOT NULL DEFAULT 1.0;
             ALTER TABLE fact_edges ADD COLUMN IF NOT EXISTS direction_kind TEXT NOT NULL DEFAULT 'forward_dependency';
             CREATE INDEX IF NOT EXISTS idx_pg_fact_edges_tier ON fact_edges(evidence_tier);"
        );

        let base = crate::config::default_workspace_base();
        let home = crate::config::home_dir();

        // 预置放置规范规则
        for r in crate::storage::default_seed_placement_rules(&base) {
            let _ = client.execute(
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
            );
        }

        // 预置扫描策略
        for p in crate::storage::default_seed_scan_policies(&base, &home) {
            let _ = client.execute(
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
            );
        }

        Ok(())
    }

    fn provider_name(&self) -> &'static str {
        "postgres"
    }

    fn list_created_at_map(&self, table: &str) -> Result<std::collections::HashMap<String, String>> {
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

    fn get_placement_rules(&self) -> Result<Vec<PlacementRule>> {
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

    fn insert_placement_rule(&self, rule: &PlacementRule) -> Result<()> {
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

    fn get_scan_policies(&self) -> Result<Vec<ScanPolicy>> {
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

    fn upsert_scan_policy(&self, policy: &ScanPolicy) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO scan_policies (id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
             ON CONFLICT (path_pattern) DO UPDATE SET
                tier = EXCLUDED.tier,
                max_depth = EXCLUDED.max_depth,
                enable_fts = EXCLUDED.enable_fts,
                enable_vec = EXCLUDED.enable_vec,
                enable_graph = EXCLUDED.enable_graph,
                notes = EXCLUDED.notes",
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

    fn upsert_asset(&self, asset: &AssetRecord) -> Result<()> {
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

    fn get_asset_by_path(&self, path: &str) -> Result<Option<AssetRecord>> {
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

    fn search_assets_fts(&self, query: &str, limit: usize) -> Result<Vec<AssetRecord>> {
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

    fn list_assets(&self, limit: usize) -> Result<Vec<AssetRecord>> {
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

    fn replace_active_processes(&self, procs: &[ActiveProcessRecord]) -> Result<()> {
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

    fn list_active_processes(&self) -> Result<Vec<ActiveProcessRecord>> {
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

    fn insert_memory(&self, mem: &MemoryRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO memories (id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
             ON CONFLICT (id) DO UPDATE SET
                type = EXCLUDED.type,
                content = EXCLUDED.content,
                summary = EXCLUDED.summary,
                source_agent = COALESCE(EXCLUDED.source_agent, memories.source_agent),
                source_session = COALESCE(EXCLUDED.source_session, memories.source_session),
                tags = EXCLUDED.tags,
                weight = EXCLUDED.weight,
                pinned = EXCLUDED.pinned,
                status = EXCLUDED.status,
                embedding = CASE WHEN EXCLUDED.embedding != '[]' THEN EXCLUDED.embedding ELSE memories.embedding END,
                updated_at = NOW()",
            &[
                &mem.id,
                &mem.memory_type,
                &mem.content,
                &mem.summary,
                &mem.source_agent,
                &mem.source_session,
                &serde_json::to_value(&mem.tags)?,
                &mem.weight,
                &mem.pinned,
                &mem.status,
                &serde_json::to_string(&mem.embedding)?,
            ],
        )?;
        Ok(())
    }

    fn search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             WHERE (fts_doc @@ plainto_tsquery('simple', $1)
                OR content ILIKE '%' || $1 || '%'
                OR summary ILIKE '%' || $1 || '%')
               AND status = 'active'
             ORDER BY 
                CASE WHEN fts_doc @@ plainto_tsquery('simple', $1)
                     THEN ts_rank(fts_doc, plainto_tsquery('simple', $1)) + 1.0
                     ELSE 0.5
                END DESC,
                weight DESC
             LIMIT $2",
            &[&query, &(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let tags_val: serde_json::Value = row.get(6);
            let tags: Vec<String> = serde_json::from_value(tags_val).unwrap_or_default();
            let emb_str: String = row.get(10);
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();

            results.push(MemoryRecord {
                id: row.get(0),
                memory_type: row.get(1),
                content: row.get(2),
                summary: row.get(3),
                source_agent: row.get(4),
                source_session: row.get(5),
                tags,
                weight: row.get(7),
                pinned: row.get(8),
                status: row.get(9),
                embedding,
            });
        }
        Ok(results)
    }

    fn list_all_active_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             WHERE status = 'active'
             ORDER BY updated_at DESC
             LIMIT $1",
            &[&(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let tags_val: serde_json::Value = row.get(6);
            let tags: Vec<String> = serde_json::from_value(tags_val).unwrap_or_default();
            let emb_str: String = row.get(10);
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();

            results.push(MemoryRecord {
                id: row.get(0),
                memory_type: row.get(1),
                content: row.get(2),
                summary: row.get(3),
                source_agent: row.get(4),
                source_session: row.get(5),
                tags,
                weight: row.get(7),
                pinned: row.get(8),
                status: row.get(9),
                embedding,
            });
        }
        Ok(results)
    }

    fn list_all_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             ORDER BY updated_at DESC
             LIMIT $1",
            &[&(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let tags_val: serde_json::Value = row.get(6);
            let tags: Vec<String> = serde_json::from_value(tags_val).unwrap_or_default();
            let emb_str: String = row.get(10);
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();

            results.push(MemoryRecord {
                id: row.get(0),
                memory_type: row.get(1),
                content: row.get(2),
                summary: row.get(3),
                source_agent: row.get(4),
                source_session: row.get(5),
                tags,
                weight: row.get(7),
                pinned: row.get(8),
                status: row.get(9),
                embedding,
            });
        }
        Ok(results)
    }

    fn update_memory_status(&self, memory_id: &str, status: &str) -> Result<usize> {
        let mut client = self.lock_client()?;
        let count = client.execute(
            "UPDATE memories SET status = $2, updated_at = NOW() WHERE id = $1",
            &[&memory_id, &status],
        )?;
        Ok(count as usize)
    }

    fn record_change(&self, change: &ChangeRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        let created_ts = change
            .created_at
            .as_deref()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(chrono::Utc::now);
        client.execute(
            "INSERT INTO changes (type, target, description, details, source, created_at)
             VALUES ($1, $2, $3, $4, $5, $6)",
            &[
                &change.change_type,
                &change.target,
                &change.description,
                &change.details,
                &change.source,
                &created_ts,
            ],
        )?;
        Ok(())
    }

    fn list_changes(&self, limit: usize) -> Result<Vec<ChangeRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, type, target, description, details, source, created_at
             FROM changes ORDER BY created_at DESC LIMIT $1",
            &[&(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let id_i64: i64 = row.get(0);
            let created: Option<chrono::DateTime<chrono::Utc>> = row.get(6);
            results.push(ChangeRecord {
                id: Some(id_i64),
                change_type: row.get(1),
                target: row.get(2),
                description: row.get(3),
                details: row.get(4),
                source: row.get(5),
                created_at: created.map(|c| c.to_rfc3339()),
            });
        }
        Ok(results)
    }

    fn add_link(&self, link: &LinkRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO links (source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             ON CONFLICT (source_type, source_id, target_type, target_id, relation) DO UPDATE SET
                confidence = EXCLUDED.confidence,
                line_number = EXCLUDED.line_number,
                snippet = EXCLUDED.snippet,
                provenance = EXCLUDED.provenance",
            &[
                &link.source_type,
                &link.source_id,
                &link.target_type,
                &link.target_id,
                &link.relation,
                &link.confidence,
                &link.auto_discovered,
                &link.line_number,
                &link.snippet,
                &link.provenance.as_deref().unwrap_or("static_scan"),
            ],
        )?;
        Ok(())
    }

    fn delete_link(&self, id: i64) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute("DELETE FROM links WHERE id = $1", &[&id])?;
        Ok(())
    }

    fn delete_links_for_target(&self, target_id: &str) -> Result<usize> {
        let mut client = self.lock_client()?;
        let count = client.execute("DELETE FROM links WHERE target_id = $1", &[&target_id])?;
        Ok(count as usize)
    }

    fn delete_auto_links_for_source(&self, source_id: &str) -> Result<usize> {
        let mut client = self.lock_client()?;
        let count = client.execute(
            "DELETE FROM links WHERE source_id = $1 AND auto_discovered = TRUE",
            &[&source_id],
        )?;
        Ok(count as usize)
    }

    fn update_link_target(&self, source_id: &str, old_target: &str, new_target: &str) -> Result<usize> {
        let mut client = self.lock_client()?;
        let count = client.execute(
            "UPDATE links SET target_id = $3 WHERE source_id = $1 AND target_id = $2",
            &[&source_id, &old_target, &new_target],
        )?;
        Ok(count as usize)
    }

    fn query_links_for_node(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             WHERE (source_type = $1 AND source_id = $2) OR (target_type = $1 AND target_id = $2)
             ORDER BY created_at DESC",
            &[&node_type, &node_id],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let id_i64: i64 = row.get(0);
            let created: Option<chrono::DateTime<chrono::Utc>> = row.get(11);
            results.push(LinkRecord {
                id: Some(id_i64),
                source_type: row.get(1),
                source_id: row.get(2),
                target_type: row.get(3),
                target_id: row.get(4),
                relation: row.get(5),
                confidence: row.get(6),
                auto_discovered: row.get(7),
                line_number: row.get(8),
                snippet: row.get(9),
                provenance: row.get(10),
                created_at: created.map(|c| c.to_rfc3339()),
            });
        }
        Ok(results)
    }

    fn list_all_links(&self, limit: usize) -> Result<Vec<LinkRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, source_type, source_id, target_type, target_id, relation, confidence, auto_discovered, line_number, snippet, provenance, created_at
             FROM links
             ORDER BY created_at DESC
             LIMIT $1",
            &[&(limit as i64)],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let id_i64: i64 = row.get(0);
            let created: Option<chrono::DateTime<chrono::Utc>> = row.get(11);
            results.push(LinkRecord {
                id: Some(id_i64),
                source_type: row.get(1),
                source_id: row.get(2),
                target_type: row.get(3),
                target_id: row.get(4),
                relation: row.get(5),
                confidence: row.get(6),
                auto_discovered: row.get(7),
                line_number: row.get(8),
                snippet: row.get(9),
                provenance: row.get(10),
                created_at: created.map(|c| c.to_rfc3339()),
            });
        }
        Ok(results)
    }

    fn create_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
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

    fn update_scan_job(&self, job: &ScanJobRecord) -> Result<()> {
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

    fn get_scan_job(&self, id: &str) -> Result<Option<ScanJobRecord>> {
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

    fn list_scan_jobs(&self, limit: usize) -> Result<Vec<ScanJobRecord>> {
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

    fn get_active_scan_job(&self) -> Result<Option<ScanJobRecord>> {
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

    fn upsert_fact_node(&self, node: &FactNodeRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO fact_nodes (id, node_type, name, label, properties, updated_at)
             VALUES ($1, $2, $3, $4, $5, NOW())
             ON CONFLICT (id) DO UPDATE SET
                node_type = EXCLUDED.node_type,
                name = EXCLUDED.name,
                label = EXCLUDED.label,
                properties = EXCLUDED.properties,
                updated_at = NOW()",
            &[&node.id, &node.node_type, &node.name, &node.label, &node.properties],
        )?;
        Ok(())
    }

    fn upsert_fact_edge(&self, edge: &FactEdgeRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO fact_edges (source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, NOW())
             ON CONFLICT (source_id, target_id, relation) DO UPDATE SET
                properties = EXCLUDED.properties,
                confidence = EXCLUDED.confidence,
                evidence_tier = EXCLUDED.evidence_tier,
                weight = EXCLUDED.weight,
                direction_kind = EXCLUDED.direction_kind,
                updated_at = NOW()",
            &[
                &edge.source_id,
                &edge.target_id,
                &edge.relation,
                &edge.properties,
                &edge.confidence,
                &edge.evidence_tier,
                &edge.weight,
                &edge.direction_kind,
            ],
        )?;
        Ok(())
    }

    fn get_fact_node(&self, id: &str) -> Result<Option<FactNodeRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes WHERE id = $1",
            &[&id],
        )?;

        if let Some(row) = rows.first() {
            let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(5);
            Ok(Some(FactNodeRecord {
                id: row.get(0),
                node_type: row.get(1),
                name: row.get(2),
                label: row.get(3),
                properties: row.get(4),
                updated_at: updated.map(|u| u.to_rfc3339()),
            }))
        } else {
            Ok(None)
        }
    }

    fn find_fact_nodes_by_name(&self, name: &str) -> Result<Vec<FactNodeRecord>> {
        let mut client = self.lock_client()?;
        let pattern = format!("%{}%", name);
        let rows = client.query(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes 
             WHERE name ILIKE $1 OR label ILIKE $1 LIMIT 50",
            &[&pattern],
        )?;

        let mut results = Vec::new();
        for row in rows {
            let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(5);
            results.push(FactNodeRecord {
                id: row.get(0),
                node_type: row.get(1),
                name: row.get(2),
                label: row.get(3),
                properties: row.get(4),
                updated_at: updated.map(|u| u.to_rfc3339()),
            });
        }
        Ok(results)
    }

    fn traverse_fact_graph(&self, root_id: &str, max_hops: usize) -> Result<SubGraph> {
        let root = match self.get_fact_node(root_id)? {
            Some(r) => r,
            None => return Ok(SubGraph::default()),
        };

        let mut client = self.lock_client()?;
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
                let rows = client.query(
                    "SELECT id, source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at
                     FROM fact_edges
                     WHERE source_id = $1 OR target_id = $1",
                    &[&node_id],
                )?;

                for row in rows {
                    let id_i64: i64 = row.get(0);
                    let src: String = row.get(1);
                    let tgt: String = row.get(2);
                    let rel: String = row.get(3);
                    let props: serde_json::Value = row.get(4);
                    let conf: f32 = row.get(5);
                    let tier: String = row.get(6);
                    let weight: f32 = row.get(7);
                    let dir: String = row.get(8);
                    let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(9);

                    let other_id = if &src == node_id {
                        tgt.clone()
                    } else {
                        src.clone()
                    };

                    if !visited_node_ids.contains(&other_id) {
                        visited_node_ids.insert(other_id.clone());
                        next_frontier.push(other_id);
                    }

                    if !collected_edges.iter().any(|existing: &FactEdgeRecord| {
                        existing.source_id == src && existing.target_id == tgt && existing.relation == rel
                    }) {
                        collected_edges.push(FactEdgeRecord {
                            id: Some(id_i64),
                            source_id: src,
                            target_id: tgt,
                            relation: rel,
                            properties: props,
                            confidence: conf,
                            evidence_tier: tier,
                            weight,
                            direction_kind: dir,
                            updated_at: updated.map(|u| u.to_rfc3339()),
                        });
                    }
                }
            }
            current_frontier = next_frontier;
        }

        let mut collected_nodes = Vec::new();
        for nid in &visited_node_ids {
            let rows = client.query(
                "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes WHERE id = $1",
                &[&nid],
            )?;
            if let Some(row) = rows.first() {
                let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(5);
                collected_nodes.push(FactNodeRecord {
                    id: row.get(0),
                    node_type: row.get(1),
                    name: row.get(2),
                    label: row.get(3),
                    properties: row.get(4),
                    updated_at: updated.map(|u| u.to_rfc3339()),
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
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, node_type, name, label, properties, updated_at FROM fact_nodes ORDER BY id LIMIT $1",
            &[&(limit as i64)],
        )?;
        let mut nodes = Vec::new();
        for row in rows {
            let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(5);
            nodes.push(FactNodeRecord {
                id: row.get(0),
                node_type: row.get(1),
                name: row.get(2),
                label: row.get(3),
                properties: row.get(4),
                updated_at: updated.map(|u| u.to_rfc3339()),
            });
        }
        Ok(nodes)
    }

    fn list_all_fact_edges(&self, limit: usize) -> Result<Vec<FactEdgeRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, source_id, target_id, relation, properties, confidence, evidence_tier, weight, direction_kind, updated_at FROM fact_edges ORDER BY id LIMIT $1",
            &[&(limit as i64)],
        )?;
        let mut edges = Vec::new();
        for row in rows {
            let id_i64: i64 = row.get(0);
            let updated: Option<chrono::DateTime<chrono::Utc>> = row.get(9);
            edges.push(FactEdgeRecord {
                id: Some(id_i64),
                source_id: row.get(1),
                target_id: row.get(2),
                relation: row.get(3),
                properties: row.get(4),
                confidence: row.get(5),
                evidence_tier: row.get(6),
                weight: row.get(7),
                direction_kind: row.get(8),
                updated_at: updated.map(|u| u.to_rfc3339()),
            });
        }
        Ok(edges)
    }

    fn delete_fact_node(&self, id: &str) -> Result<usize> {
        let mut client = self.lock_client()?;
        client.execute(
            "DELETE FROM fact_edges WHERE source_id = $1 OR target_id = $1",
            &[&id],
        )?;
        let affected = client.execute("DELETE FROM fact_nodes WHERE id = $1", &[&id])?;
        Ok(affected as usize)
    }

    fn delete_fact_edges_by_tier(&self, tier: &str) -> Result<usize> {
        let mut client = self.lock_client()?;
        let affected = client.execute(
            "DELETE FROM fact_edges WHERE evidence_tier = $1",
            &[&tier],
        )?;
        Ok(affected as usize)
    }

    fn update_asset_embedding(&self, path: &str, embedding: &[f32]) -> Result<()> {
        let mut client = self.lock_client()?;
        let emb_str = serde_json::to_string(embedding)?;
        client.execute(
            "UPDATE assets SET embedding = $1, updated_at = NOW() WHERE path = $2",
            &[&emb_str, &path],
        )?;
        Ok(())
    }

    fn get_asset_embedding(&self, path: &str) -> Result<Vec<f32>> {
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

    fn update_fact_node_embedding(&self, id: &str, embedding: &[f32]) -> Result<()> {
        let mut client = self.lock_client()?;
        let emb_str = serde_json::to_string(embedding)?;
        client.execute(
            "UPDATE fact_nodes SET embedding = $1, updated_at = NOW() WHERE id = $2",
            &[&emb_str, &id],
        )?;
        Ok(())
    }

    fn get_fact_node_embedding(&self, id: &str) -> Result<Vec<f32>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT COALESCE(embedding, '[]') FROM fact_nodes WHERE id = $1",
            &[&id],
        )?;
        if let Some(row) = rows.first() {
            let s: String = row.get(0);
            Ok(serde_json::from_str(&s).unwrap_or_default())
        } else {
            Ok(Vec::new())
        }
    }

    // ─── Multi-Agent Coordination Locks (多 Agent 协作锁) ────────────
    fn insert_coordination_lock(&self, lock: &CoordinationLock) -> Result<bool> {
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

    fn delete_coordination_lock(&self, lock_id: &str) -> Result<bool> {
        let mut client = self.lock_client()?;
        let affected = client.execute("DELETE FROM coordination_locks WHERE lock_id = $1", &[&lock_id])?;
        Ok(affected > 0)
    }

    fn get_coordination_lock(&self, resource_type: &str, resource_id: &str) -> Result<Option<CoordinationLock>> {
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

    fn list_coordination_locks(&self, resource_type: Option<&str>) -> Result<Vec<CoordinationLock>> {
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

    fn gc_expired_coordination_locks(&self) -> Result<usize> {
        let mut client = self.lock_client()?;
        let now = chrono::Utc::now();
        let affected = client.execute(
            "DELETE FROM coordination_locks WHERE expires_at::timestamptz <= $1",
            &[&now],
        )?;
        Ok(affected as usize)
    }

    fn count_expired_coordination_locks(&self) -> Result<usize> {
        let mut client = self.lock_client()?;
        let now = chrono::Utc::now();
        let row = client.query_one(
            "SELECT COUNT(*) FROM coordination_locks WHERE expires_at::timestamptz <= $1",
            &[&now],
        )?;
        let count: i64 = row.get(0);
        Ok(count.max(0) as usize)
    }

    fn upsert_mcp_catalog(&self, rec: &McpCatalogRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO mcp_catalog (id, server_name, client_source, transport, command_or_url, tools_json, tool_count, est_total_tokens, status, last_scanned_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW()::text)
             ON CONFLICT(server_name) DO UPDATE SET
               client_source=EXCLUDED.client_source, transport=EXCLUDED.transport,
               command_or_url=EXCLUDED.command_or_url, tools_json=EXCLUDED.tools_json,
               tool_count=EXCLUDED.tool_count, est_total_tokens=EXCLUDED.est_total_tokens,
               status=EXCLUDED.status, last_scanned_at=NOW()::text",
            &[
                &rec.id, &rec.server_name, &rec.client_source, &rec.transport,
                &rec.command_or_url, &rec.tools_json, &(rec.tool_count as i32),
                &(rec.est_total_tokens as i32), &rec.status,
            ],
        )?;
        Ok(())
    }

    fn list_mcp_catalog(&self) -> Result<Vec<McpCatalogRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, server_name, client_source, transport, command_or_url, tools_json, tool_count, est_total_tokens, status, last_scanned_at
             FROM mcp_catalog ORDER BY server_name ASC",
            &[],
        )?;
        Ok(rows.into_iter().map(|r| Self::row_to_mcp(&r)).collect())
    }

    fn search_mcp_catalog(&self, query: &str, limit: usize) -> Result<Vec<McpCatalogRecord>> {
        let mut client = self.lock_client()?;
        let kw = format!("%{}%", query.to_lowercase());
        let rows = client.query(
            "SELECT id, server_name, client_source, transport, command_or_url, tools_json, tool_count, est_total_tokens, status, last_scanned_at
             FROM mcp_catalog
             WHERE lower(server_name) LIKE $1 OR lower(tools_json::text) LIKE $1
             ORDER BY tool_count DESC LIMIT $2",
            &[&kw, &(limit as i64)],
        )?;
        Ok(rows.into_iter().map(|r| Self::row_to_mcp(&r)).collect())
    }

    fn upsert_skill_catalog(&self, rec: &SkillCatalogRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        let triggers_json = serde_json::to_value(&rec.triggers)?;
        client.execute(
            "INSERT INTO skills_catalog (id, skill_name, root_source, skill_path, description, triggers_json, est_tokens, has_scripts, has_references, status, last_scanned_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, NOW()::text)
             ON CONFLICT(skill_name) DO UPDATE SET
               root_source=EXCLUDED.root_source, skill_path=EXCLUDED.skill_path,
               description=EXCLUDED.description, triggers_json=EXCLUDED.triggers_json,
               est_tokens=EXCLUDED.est_tokens, has_scripts=EXCLUDED.has_scripts,
               has_references=EXCLUDED.has_references, status=EXCLUDED.status, last_scanned_at=NOW()::text",
            &[
                &rec.id, &rec.skill_name, &rec.root_source, &rec.skill_path,
                &rec.description, &triggers_json, &(rec.est_tokens as i32),
                &rec.has_scripts, &rec.has_references, &rec.status,
            ],
        )?;
        Ok(())
    }

    fn list_skills_catalog(&self) -> Result<Vec<SkillCatalogRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT id, skill_name, root_source, skill_path, description, triggers_json, est_tokens, has_scripts, has_references, status, last_scanned_at
             FROM skills_catalog ORDER BY skill_name ASC",
            &[],
        )?;
        Ok(rows.into_iter().map(|r| Self::row_to_skill(&r)).collect())
    }

    fn search_skills_catalog(&self, query: &str, limit: usize) -> Result<Vec<SkillCatalogRecord>> {
        let mut client = self.lock_client()?;
        let kw = format!("%{}%", query.to_lowercase());
        let rows = client.query(
            "SELECT id, skill_name, root_source, skill_path, description, triggers_json, est_tokens, has_scripts, has_references, status, last_scanned_at
             FROM skills_catalog
             WHERE lower(skill_name) LIKE $1 OR lower(description) LIKE $1 OR lower(triggers_json::text) LIKE $1
             ORDER BY skill_name ASC LIMIT $2",
            &[&kw, &(limit as i64)],
        )?;
        Ok(rows.into_iter().map(|r| Self::row_to_skill(&r)).collect())
    }

    fn upsert_external_service(&self, rec: &ExternalServiceRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO external_services (id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, COALESCE($12, NOW()::text))
             ON CONFLICT(service_name) DO UPDATE SET
               category=EXCLUDED.category, endpoint=EXCLUDED.endpoint, protocol=EXCLUDED.protocol,
               capabilities=EXCLUDED.capabilities, auth_type=EXCLUDED.auth_type, auth_env_var=EXCLUDED.auth_env_var,
               health_endpoint=EXCLUDED.health_endpoint, usage_example=EXCLUDED.usage_example,
               status=EXCLUDED.status, last_checked_at=COALESCE(EXCLUDED.last_checked_at, NOW()::text)",
            &[
                &rec.id, &rec.service_name, &rec.category, &rec.endpoint, &rec.protocol,
                &rec.capabilities, &rec.auth_type, &rec.auth_env_var, &rec.health_endpoint,
                &rec.usage_example, &rec.status, &rec.last_checked_at,
            ],
        )?;
        Ok(())
    }

    fn list_external_services(&self, category: Option<&str>) -> Result<Vec<ExternalServiceRecord>> {
        let mut client = self.lock_client()?;
        let rows = match category {
            Some(cat) => client.query(
                "SELECT id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at
                 FROM external_services WHERE category = $1 ORDER BY service_name ASC",
                &[&cat],
            )?,
            None => client.query(
                "SELECT id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at
                 FROM external_services ORDER BY service_name ASC",
                &[],
            )?,
        };
        Ok(rows.into_iter().map(|r| Self::row_to_ext_svc(&r)).collect())
    }

    fn search_external_services(&self, query: &str, limit: usize) -> Result<Vec<ExternalServiceRecord>> {
        let mut client = self.lock_client()?;
        let kw = format!("%{}%", query.to_lowercase());
        let rows = client.query(
            "SELECT id, service_name, category, endpoint, protocol, capabilities, auth_type, auth_env_var, health_endpoint, usage_example, status, last_checked_at
             FROM external_services
             WHERE lower(service_name) LIKE $1 OR lower(category) LIKE $1 OR lower(capabilities) LIKE $1
             ORDER BY service_name ASC LIMIT $2",
            &[&kw, &(limit as i64)],
        )?;
        Ok(rows.into_iter().map(|r| Self::row_to_ext_svc(&r)).collect())
    }

    fn enqueue_lock_wait(&self, entry: &LockWaitEntry) -> Result<()> {
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

    fn list_lock_wait_queue(&self, resource_type: Option<&str>, resource_id: Option<&str>) -> Result<Vec<LockWaitEntry>> {
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

    fn pop_next_lock_waiter(&self, resource_type: &str, resource_id: &str) -> Result<Option<LockWaitEntry>> {
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

    fn update_lock_wait_status(&self, wait_id: &str, status: &str) -> Result<bool> {
        let mut client = self.lock_client()?;
        let affected = client.execute(
            "UPDATE lock_wait_queue SET status = $1 WHERE wait_id = $2",
            &[&status, &wait_id],
        )?;
        Ok(affected > 0)
    }

    fn gc_expired_lock_waits(&self) -> Result<usize> {
        let mut client = self.lock_client()?;
        let now = chrono::Utc::now();
        let affected = client.execute(
            "UPDATE lock_wait_queue SET status = 'expired' WHERE status IN ('waiting','notified') AND timeout_at::timestamptz <= $1",
            &[&now],
        )?;
        Ok(affected as usize)
    }

    fn insert_benchmark_run(&self, run: &BenchmarkRunRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        let host_fp = serde_json::to_string(&run.host_fingerprint)?;
        let res_json = serde_json::to_string(&run.results_json)?;
        let wcount = run.workload_count as i32;
        client.execute(
            "INSERT INTO benchmark_runs (
                run_id, suite_name, git_commit, app_version, host_fingerprint,
                workload_count, total_baseline_tokens, total_steward_tokens,
                overall_token_savings_pct, overall_speedup_factor,
                total_hazards_intercepted, total_noise_blocked, sla_passed,
                results_json, report_sha256, duration_ms, created_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
             ON CONFLICT (run_id) DO UPDATE SET
                suite_name = EXCLUDED.suite_name,
                git_commit = EXCLUDED.git_commit,
                app_version = EXCLUDED.app_version,
                host_fingerprint = EXCLUDED.host_fingerprint,
                workload_count = EXCLUDED.workload_count,
                total_baseline_tokens = EXCLUDED.total_baseline_tokens,
                total_steward_tokens = EXCLUDED.total_steward_tokens,
                overall_token_savings_pct = EXCLUDED.overall_token_savings_pct,
                overall_speedup_factor = EXCLUDED.overall_speedup_factor,
                total_hazards_intercepted = EXCLUDED.total_hazards_intercepted,
                total_noise_blocked = EXCLUDED.total_noise_blocked,
                sla_passed = EXCLUDED.sla_passed,
                results_json = EXCLUDED.results_json,
                report_sha256 = EXCLUDED.report_sha256,
                duration_ms = EXCLUDED.duration_ms,
                created_at = EXCLUDED.created_at",
            &[
                &run.run_id,
                &run.suite_name,
                &run.git_commit,
                &run.app_version,
                &host_fp,
                &wcount,
                &run.total_baseline_tokens,
                &run.total_steward_tokens,
                &run.overall_token_savings_pct,
                &run.overall_speedup_factor,
                &run.total_hazards_intercepted,
                &run.total_noise_blocked,
                &run.sla_passed,
                &res_json,
                &run.report_sha256,
                &run.duration_ms,
                &run.created_at,
            ],
        )?;
        Ok(())
    }

    fn get_benchmark_run(&self, run_id: &str) -> Result<Option<BenchmarkRunRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT run_id, suite_name, git_commit, app_version, host_fingerprint,
                    workload_count, total_baseline_tokens, total_steward_tokens,
                    overall_token_savings_pct, overall_speedup_factor,
                    total_hazards_intercepted, total_noise_blocked, sla_passed,
                    results_json, report_sha256, duration_ms, created_at
             FROM benchmark_runs WHERE run_id = $1",
            &[&run_id],
        )?;
        Ok(rows.first().map(Self::row_to_benchmark_run))
    }

    fn list_benchmark_runs(&self, limit: usize) -> Result<Vec<BenchmarkRunRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT run_id, suite_name, git_commit, app_version, host_fingerprint,
                    workload_count, total_baseline_tokens, total_steward_tokens,
                    overall_token_savings_pct, overall_speedup_factor,
                    total_hazards_intercepted, total_noise_blocked, sla_passed,
                    results_json, report_sha256, duration_ms, created_at
             FROM benchmark_runs WHERE run_id NOT LIKE 'awr:%' ORDER BY created_at DESC LIMIT $1",
            &[&(limit as i64)],
        )?;
        Ok(rows.iter().map(Self::row_to_benchmark_run).collect())
    }

    fn list_all_benchmark_runs(&self, limit: usize) -> Result<Vec<BenchmarkRunRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT run_id, suite_name, git_commit, app_version, host_fingerprint,
                    workload_count, total_baseline_tokens, total_steward_tokens,
                    overall_token_savings_pct, overall_speedup_factor,
                    total_hazards_intercepted, total_noise_blocked, sla_passed,
                    results_json, report_sha256, duration_ms, created_at
             FROM benchmark_runs ORDER BY created_at DESC LIMIT $1",
            &[&(limit as i64)],
        )?;
        Ok(rows.iter().map(Self::row_to_benchmark_run).collect())
    }

    fn delete_benchmark_run(&self, run_id: &str) -> Result<bool> {
        let mut client = self.lock_client()?;
        let affected = client.execute(
            "DELETE FROM benchmark_runs WHERE run_id = $1",
            &[&run_id],
        )?;
        Ok(affected > 0)
    }

    fn insert_effectiveness_event(&self, event: &EffectivenessLedgerRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "INSERT INTO effectiveness_ledger (
                event_id, session_id, source_channel, tool_or_action, provenance_rule,
                baseline_tokens, steward_tokens, tokens_saved,
                baseline_rounds, steward_rounds, rounds_saved,
                hazard_intercepted, noise_blocked, elapsed_us, evidence_summary, created_at
             ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
             ON CONFLICT (event_id) DO NOTHING",
            &[
                &event.event_id,
                &event.session_id,
                &event.source_channel,
                &event.tool_or_action,
                &event.provenance_rule,
                &event.baseline_tokens,
                &event.steward_tokens,
                &event.tokens_saved,
                &event.baseline_rounds,
                &event.steward_rounds,
                &event.rounds_saved,
                &event.hazard_intercepted,
                &event.noise_blocked,
                &event.elapsed_us,
                &event.evidence_summary,
                &event.created_at,
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
        let mut client = self.lock_client()?;
        let cutoff = crate::storage::window_to_cutoff_iso(window).unwrap_or_default();
        let tool = tool_filter.unwrap_or("").to_string();

        let agg = client.query_one(
            "SELECT COUNT(*),
                    COALESCE(SUM(baseline_tokens), 0)::BIGINT,
                    COALESCE(SUM(steward_tokens), 0)::BIGINT,
                    COALESCE(SUM(tokens_saved), 0)::BIGINT,
                    COALESCE(SUM(rounds_saved), 0)::BIGINT,
                    COALESCE(SUM(hazard_intercepted), 0)::BIGINT,
                    COALESCE(SUM(noise_blocked), 0)::BIGINT,
                    COALESCE(AVG(elapsed_us)::DOUBLE PRECISION, 0.0)
             FROM effectiveness_ledger
             WHERE ($1 = '' OR created_at >= $1)
               AND ($2 = '' OR tool_or_action = $2)",
            &[&cutoff, &tool],
        )?;

        let total_events: i64 = agg.get(0);
        let total_base: i64 = agg.get(1);
        let total_stew: i64 = agg.get(2);
        let total_saved: i64 = agg.get(3);
        let total_rounds: i64 = agg.get(4);
        let total_haz: i64 = agg.get(5);
        let total_noise: i64 = agg.get(6);
        let avg_us: f64 = agg.get(7);

        let rule_rows = client.query(
            "SELECT provenance_rule,
                    COUNT(*),
                    COALESCE(SUM(tokens_saved), 0)::BIGINT,
                    COALESCE(SUM(rounds_saved), 0)::BIGINT,
                    COALESCE(SUM(hazard_intercepted), 0)::BIGINT,
                    COALESCE(SUM(noise_blocked), 0)::BIGINT
             FROM effectiveness_ledger
             WHERE ($1 = '' OR created_at >= $1)
               AND ($2 = '' OR tool_or_action = $2)
             GROUP BY provenance_rule
             ORDER BY SUM(tokens_saved) DESC, COUNT(*) DESC",
            &[&cutoff, &tool],
        )?;
        let by_rule = rule_rows
            .iter()
            .map(|r| RuleAggregate {
                provenance_rule: r.get(0),
                event_count: r.get::<_, i64>(1) as usize,
                tokens_saved: r.get(2),
                rounds_saved: r.get(3),
                hazards_intercepted: r.get(4),
                noise_blocked: r.get(5),
            })
            .collect();

        let ev_rows = client.query(
            "SELECT event_id, session_id, source_channel, tool_or_action, provenance_rule,
                    baseline_tokens, steward_tokens, tokens_saved,
                    baseline_rounds, steward_rounds, rounds_saved,
                    hazard_intercepted, noise_blocked, elapsed_us, evidence_summary, created_at
             FROM effectiveness_ledger
             WHERE ($1 = '' OR created_at >= $1)
               AND ($2 = '' OR tool_or_action = $2)
             ORDER BY created_at DESC LIMIT $3",
            &[&cutoff, &tool, &(limit as i64)],
        )?;
        let recent_events = ev_rows
            .iter()
            .map(|r| EffectivenessLedgerRecord {
                event_id: r.get(0),
                session_id: r.get(1),
                source_channel: r.get(2),
                tool_or_action: r.get(3),
                provenance_rule: r.get(4),
                baseline_tokens: r.get(5),
                steward_tokens: r.get(6),
                tokens_saved: r.get(7),
                baseline_rounds: r.get(8),
                steward_rounds: r.get(9),
                rounds_saved: r.get(10),
                hazard_intercepted: r.get(11),
                noise_blocked: r.get(12),
                elapsed_us: r.get(13),
                evidence_summary: r.get(14),
                created_at: r.get(15),
            })
            .collect();

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
        let mut client = self.lock_client()?;
        let mut removed = 0usize;
        removed += client.execute(
            "DELETE FROM coordination_locks WHERE holder_agent LIKE 'bench-%' OR resource_id LIKE '%bench:%'",
            &[],
        )? as usize;
        removed += client.execute(
            "DELETE FROM lock_wait_queue WHERE waiting_agent LIKE 'bench-%' OR resource_id LIKE '%bench:%'",
            &[],
        )? as usize;
        removed += client.execute(
            "DELETE FROM effectiveness_ledger WHERE session_id LIKE 'bench:%' OR tool_or_action LIKE 'bench:%'",
            &[],
        )? as usize;
        removed += client.execute(
            "DELETE FROM benchmark_runs WHERE run_id LIKE 'bench:%' OR suite_name LIKE 'bench:%'",
            &[],
        )? as usize;
        removed += client.execute(
            "DELETE FROM audit_events WHERE session_id LIKE 'bench:%' OR target_id LIKE '%bench:%'",
            &[],
        )? as usize;
        Ok(removed)
    }

    fn insert_mesh_dialogue(&self, rec: &MeshDialogueRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        let rh = rec.redaction_hits as i32;
        let inj = if rec.injection_flagged { 1i32 } else { 0i32 };
        let ver = if rec.verified_signature { 1i32 } else { 0i32 };
        client.execute(
            "INSERT INTO mesh_dialogues
             (dialogue_id, thread_id, from_node, from_agent, to_node, to_agent, intent, content, redaction_hits, injection_flagged, verified_signature, status, created_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
             ON CONFLICT (dialogue_id) DO UPDATE SET status = EXCLUDED.status",
            &[
                &rec.dialogue_id, &rec.thread_id, &rec.from_node, &rec.from_agent, &rec.to_node,
                &rec.to_agent, &rec.intent, &rec.content, &rh, &inj, &ver, &rec.status, &rec.created_at,
            ],
        )?;
        Ok(())
    }

    fn list_mesh_dialogues(&self, limit: usize) -> Result<Vec<MeshDialogueRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT dialogue_id, thread_id, from_node, from_agent, to_node, to_agent, intent, content, redaction_hits, injection_flagged, verified_signature, status, created_at
             FROM mesh_dialogues ORDER BY created_at DESC LIMIT $1",
            &[&(limit as i64)],
        )?;
        Ok(rows.into_iter().map(|r| MeshDialogueRecord {
            dialogue_id: r.get(0),
            thread_id: r.get(1),
            from_node: r.get(2),
            from_agent: r.get(3),
            to_node: r.get(4),
            to_agent: r.get(5),
            intent: r.get(6),
            content: r.get(7),
            redaction_hits: r.get::<_, i32>(8) as usize,
            injection_flagged: r.get::<_, i32>(9) != 0,
            verified_signature: r.get::<_, i32>(10) != 0,
            status: r.get(11),
            created_at: r.get(12),
        }).collect())
    }

    fn update_mesh_dialogue_status(&self, dialogue_id: &str, status: &str) -> Result<bool> {
        let mut client = self.lock_client()?;
        let n = client.execute(
            "UPDATE mesh_dialogues SET status = $1 WHERE dialogue_id = $2",
            &[&status, &dialogue_id],
        )?;
        Ok(n > 0)
    }

    fn insert_mesh_transfer(&self, rec: &MeshTransferRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        let sz = rec.size_bytes as i64;
        let sw = rec.secret_warnings as i32;
        let ver = if rec.verified_signature { 1i32 } else { 0i32 };
        client.execute(
            "INSERT INTO mesh_transfers
             (transfer_id, from_node, from_agent, to_node, file_name, size_bytes, sha256, inbox_path, secret_warnings, verified_signature, status, accepted_path, created_at, updated_at)
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
             ON CONFLICT (transfer_id) DO UPDATE SET status = EXCLUDED.status, accepted_path = EXCLUDED.accepted_path, updated_at = EXCLUDED.updated_at",
            &[
                &rec.transfer_id, &rec.from_node, &rec.from_agent, &rec.to_node, &rec.file_name,
                &sz, &rec.sha256, &rec.inbox_path, &sw, &ver, &rec.status, &rec.accepted_path,
                &rec.created_at, &rec.updated_at,
            ],
        )?;
        Ok(())
    }

    fn list_mesh_transfers(&self, limit: usize) -> Result<Vec<MeshTransferRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT transfer_id, from_node, from_agent, to_node, file_name, size_bytes, sha256, inbox_path, secret_warnings, verified_signature, status, accepted_path, created_at, updated_at
             FROM mesh_transfers ORDER BY created_at DESC LIMIT $1",
            &[&(limit as i64)],
        )?;
        Ok(rows.into_iter().map(|r| MeshTransferRecord {
            transfer_id: r.get(0),
            from_node: r.get(1),
            from_agent: r.get(2),
            to_node: r.get(3),
            file_name: r.get(4),
            size_bytes: r.get::<_, i64>(5) as u64,
            sha256: r.get(6),
            inbox_path: r.get(7),
            secret_warnings: r.get::<_, i32>(8) as usize,
            verified_signature: r.get::<_, i32>(9) != 0,
            status: r.get(10),
            accepted_path: r.get(11),
            created_at: r.get(12),
            updated_at: r.get(13),
        }).collect())
    }

    fn update_mesh_transfer_status(
        &self,
        transfer_id: &str,
        status: &str,
        accepted_path: Option<&str>,
    ) -> Result<bool> {
        let mut client = self.lock_client()?;
        let now = chrono::Utc::now().to_rfc3339();
        let n = client.execute(
            "UPDATE mesh_transfers SET status = $1, accepted_path = COALESCE($2, accepted_path), updated_at = $3 WHERE transfer_id = $4",
            &[&status, &accepted_path, &now, &transfer_id],
        )?;
        Ok(n > 0)
    }

    fn insert_audit_event(&self, rec: &AuditEventRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        let delta_str = serde_json::to_string(&rec.before_after_delta)?;
        client.execute(
            "INSERT INTO audit_events (
                event_id, trace_id, session_id, domain, actor_kind, actor_id,
                source_channel, category, action, target_type, target_id,
                concurrency_mode, outcome, before_after_delta, elapsed_us,
                evidence_sha256, created_at
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17)
             ON CONFLICT (event_id) DO NOTHING",
            &[
                &rec.event_id,
                &rec.trace_id,
                &rec.session_id,
                &rec.domain,
                &rec.actor_kind,
                &rec.actor_id,
                &rec.source_channel,
                &rec.category,
                &rec.action,
                &rec.target_type,
                &rec.target_id,
                &rec.concurrency_mode,
                &rec.outcome,
                &delta_str,
                &rec.elapsed_us,
                &rec.evidence_sha256,
                &rec.created_at,
            ],
        )?;
        Ok(())
    }

    fn query_audit_events(&self, filter: &AuditQueryFilter) -> Result<Vec<AuditEventRecord>> {
        let mut client = self.lock_client()?;
        let domain = filter.domain.as_deref().unwrap_or("");
        let actor_kind = filter.actor_kind.as_deref().unwrap_or("");
        let category = filter.category.as_deref().unwrap_or("");
        let action = filter.action.as_deref().unwrap_or("");
        let outcome = filter.outcome.as_deref().unwrap_or("");
        let trace_id = filter.trace_id.as_deref().unwrap_or("");
        let session_id = filter.session_id.as_deref().unwrap_or("");
        let target_kw = filter.target_contains.as_deref().unwrap_or("");
        let cutoff = filter
            .window
            .as_deref()
            .and_then(crate::storage::window_to_cutoff_iso)
            .unwrap_or_default();
        let lim = if filter.limit == 0 { 50 } else { filter.limit.min(100_000) } as i64;

        let rows = client.query(
            "SELECT event_id, trace_id, session_id, domain, actor_kind, actor_id,
                    source_channel, category, action, target_type, target_id,
                    concurrency_mode, outcome, before_after_delta, elapsed_us,
                    evidence_sha256, created_at
             FROM audit_events
             WHERE ($1 = '' OR domain = $1)
               AND ($2 = '' OR actor_kind = $2)
               AND ($3 = '' OR category = $3)
               AND ($4 = '' OR action = $4)
               AND ($5 = '' OR outcome = $5)
               AND ($6 = '' OR trace_id = $6)
               AND ($7 = '' OR session_id = $7)
               AND ($8 = '' OR target_id LIKE '%' || $8 || '%')
               AND ($9 = '' OR created_at >= $9)
             ORDER BY created_at DESC LIMIT $10",
            &[&domain, &actor_kind, &category, &action, &outcome, &trace_id, &session_id, &target_kw, &cutoff, &lim],
        )?;

        Ok(rows
            .into_iter()
            .map(|r| {
                let delta_str: String = r.get(13);
                AuditEventRecord {
                    event_id: r.get(0),
                    trace_id: r.get(1),
                    session_id: r.get(2),
                    domain: r.get(3),
                    actor_kind: r.get(4),
                    actor_id: r.get(5),
                    source_channel: r.get(6),
                    category: r.get(7),
                    action: r.get(8),
                    target_type: r.get(9),
                    target_id: r.get(10),
                    concurrency_mode: r.get(11),
                    outcome: r.get(12),
                    before_after_delta: serde_json::from_str(&delta_str)
                        .unwrap_or_else(|_| serde_json::json!({})),
                    elapsed_us: r.get(14),
                    evidence_sha256: r.get(15),
                    created_at: r.get(16),
                }
            })
            .collect())
    }

    fn audit_domain_summary(&self, window: &str) -> Result<AuditDomainSummary> {
        let mut client = self.lock_client()?;
        let cutoff = crate::storage::window_to_cutoff_iso(window).unwrap_or_default();
        let rows = client.query(
            "SELECT domain, actor_kind, outcome,
                    COUNT(*)::BIGINT,
                    COALESCE(AVG(elapsed_us)::DOUBLE PRECISION, 0.0)
             FROM audit_events
             WHERE ($1 = '' OR created_at >= $1)
             GROUP BY domain, actor_kind, outcome
             ORDER BY COUNT(*) DESC",
            &[&cutoff],
        )?;
        let breakdown: Vec<AuditDomainBreakdown> = rows
            .into_iter()
            .map(|r| AuditDomainBreakdown {
                domain: r.get(0),
                actor_kind: r.get(1),
                outcome: r.get(2),
                event_count: r.get::<_, i64>(3) as usize,
                avg_elapsed_us: r.get(4),
            })
            .collect();

        let mut summary = AuditDomainSummary {
            window: window.to_string(),
            breakdown,
            ..Default::default()
        };
        for b in &summary.breakdown {
            summary.total_events += b.event_count;
            if b.domain == "IN_AGENT" {
                summary.in_agent_count += b.event_count;
            } else {
                summary.out_of_agent_count += b.event_count;
            }
            if b.outcome == "failed" || b.outcome == "blocked" {
                summary.failed_or_blocked_count += b.event_count;
            }
            if b.outcome == "recovered" {
                summary.recovered_count += b.event_count;
            }
        }
        Ok(summary)
    }

    fn prune_old_audit_events(&self, before_iso: &str) -> Result<usize> {
        let mut client = self.lock_client()?;
        let affected = client.execute("DELETE FROM audit_events WHERE created_at < $1", &[&before_iso])?;
        Ok(affected as usize)
    }

    fn insert_body_checkpoint(&self, rec: &BodyCheckpointRecord) -> Result<()> {
        let mut client = self.lock_client()?;
        let active_files = serde_json::to_string(&rec.active_files)?;
        let active_locks = serde_json::to_string(&rec.active_locks)?;
        let verified_facts = serde_json::to_string(&rec.verified_facts)?;
        let open_blockers = serde_json::to_string(&rec.open_blockers)?;
        let tb = rec.token_budget as i32;
        client.execute(
            "INSERT INTO body_checkpoints (
                checkpoint_id, body_id, from_brain, session_id, goal_summary,
                active_files_json, active_locks_json, verified_facts_json, open_blockers_json,
                next_step_hint, token_budget, status, evidence_sha256, created_at,
                resumed_by_brain, resumed_at
             ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)
             ON CONFLICT (checkpoint_id) DO UPDATE SET
                status = EXCLUDED.status,
                resumed_by_brain = EXCLUDED.resumed_by_brain,
                resumed_at = EXCLUDED.resumed_at",
            &[
                &rec.checkpoint_id,
                &rec.body_id,
                &rec.from_brain,
                &rec.session_id,
                &rec.goal_summary,
                &active_files,
                &active_locks,
                &verified_facts,
                &open_blockers,
                &rec.next_step_hint,
                &tb,
                &rec.status,
                &rec.evidence_sha256,
                &rec.created_at,
                &rec.resumed_by_brain,
                &rec.resumed_at,
            ],
        )?;
        Ok(())
    }

    fn get_latest_active_body_checkpoint(&self, body_id: &str) -> Result<Option<BodyCheckpointRecord>> {
        let mut client = self.lock_client()?;
        let rows = client.query(
            "SELECT checkpoint_id, body_id, from_brain, session_id, goal_summary,
                    active_files_json, active_locks_json, verified_facts_json, open_blockers_json,
                    next_step_hint, token_budget, status, evidence_sha256, created_at,
                    resumed_by_brain, resumed_at
             FROM body_checkpoints
             WHERE status = 'active' AND ($1 = '' OR body_id = $1)
             ORDER BY created_at DESC LIMIT 1",
            &[&body_id],
        )?;
        Ok(rows.into_iter().next().map(|r| Self::row_to_body_checkpoint(&r)))
    }

    fn list_body_checkpoints(&self, status_filter: Option<&str>, limit: usize) -> Result<Vec<BodyCheckpointRecord>> {
        let mut client = self.lock_client()?;
        let st = status_filter.unwrap_or("");
        let lim = if limit == 0 { 20 } else { limit.min(1000) } as i64;
        let rows = client.query(
            "SELECT checkpoint_id, body_id, from_brain, session_id, goal_summary,
                    active_files_json, active_locks_json, verified_facts_json, open_blockers_json,
                    next_step_hint, token_budget, status, evidence_sha256, created_at,
                    resumed_by_brain, resumed_at
             FROM body_checkpoints
             WHERE ($1 = '' OR status = $1)
             ORDER BY created_at DESC LIMIT $2",
            &[&st, &lim],
        )?;
        Ok(rows.into_iter().map(|r| Self::row_to_body_checkpoint(&r)).collect())
    }

    fn mark_body_checkpoint_resumed(&self, checkpoint_id: &str, by_brain: &str, resumed_at: &str) -> Result<bool> {
        let mut client = self.lock_client()?;
        let affected = client.execute(
            "UPDATE body_checkpoints
             SET status = 'resumed', resumed_by_brain = $2, resumed_at = $3
             WHERE checkpoint_id = $1",
            &[&checkpoint_id, &by_brain, &resumed_at],
        )?;
        Ok(affected > 0)
    }
}

impl PgStorageProvider {
    fn row_to_body_checkpoint(r: &postgres::Row) -> BodyCheckpointRecord {
        let files_s: String = r.get(5);
        let locks_s: String = r.get(6);
        let facts_s: String = r.get(7);
        let blockers_s: String = r.get(8);
        let tb: i32 = r.get(10);
        BodyCheckpointRecord {
            checkpoint_id: r.get(0),
            body_id: r.get(1),
            from_brain: r.get(2),
            session_id: r.get(3),
            goal_summary: r.get(4),
            active_files: serde_json::from_str(&files_s).unwrap_or_default(),
            active_locks: serde_json::from_str(&locks_s).unwrap_or_default(),
            verified_facts: serde_json::from_str(&facts_s).unwrap_or_default(),
            open_blockers: serde_json::from_str(&blockers_s).unwrap_or_default(),
            next_step_hint: r.get(9),
            token_budget: tb.max(0) as usize,
            status: r.get(11),
            evidence_sha256: r.get(12),
            created_at: r.get(13),
            resumed_by_brain: r.get(14),
            resumed_at: r.get(15),
        }
    }

    fn row_to_benchmark_run(r: &postgres::Row) -> BenchmarkRunRecord {
        let host_str: String = r.get(4);
        let res_str: String = r.get(13);
        BenchmarkRunRecord {
            run_id: r.get(0),
            suite_name: r.get(1),
            git_commit: r.get(2),
            app_version: r.get(3),
            host_fingerprint: serde_json::from_str(&host_str).unwrap_or_else(|_| serde_json::json!({})),
            workload_count: r.get::<_, i32>(5) as usize,
            total_baseline_tokens: r.get(6),
            total_steward_tokens: r.get(7),
            overall_token_savings_pct: r.get(8),
            overall_speedup_factor: r.get(9),
            total_hazards_intercepted: r.get(10),
            total_noise_blocked: r.get(11),
            sla_passed: r.get(12),
            results_json: serde_json::from_str(&res_str).unwrap_or_else(|_| serde_json::json!([])),
            report_sha256: r.get(14),
            duration_ms: r.get(15),
            created_at: r.get(16),
        }
    }
    fn row_to_wait(r: &postgres::Row) -> LockWaitEntry {
        LockWaitEntry {
            wait_id: r.get(0),
            resource_type: r.get(1),
            resource_id: r.get(2),
            scope: r.get(3),
            waiting_agent: r.get(4),
            priority: r.get(5),
            enqueued_at: r.get(6),
            timeout_at: r.get(7),
            status: r.get(8),
        }
    }
    fn row_to_mcp(r: &postgres::Row) -> McpCatalogRecord {
        McpCatalogRecord {
            id: r.get(0),
            server_name: r.get(1),
            client_source: r.get(2),
            transport: r.get(3),
            command_or_url: r.get(4),
            tools_json: r.get(5),
            tool_count: r.get::<_, i32>(6) as usize,
            est_total_tokens: r.get::<_, i32>(7) as usize,
            status: r.get(8),
            last_scanned_at: r.get(9),
        }
    }

    fn row_to_skill(r: &postgres::Row) -> SkillCatalogRecord {
        let triggers_val: serde_json::Value = r.get(5);
        let triggers: Vec<String> = serde_json::from_value(triggers_val).unwrap_or_default();
        SkillCatalogRecord {
            id: r.get(0),
            skill_name: r.get(1),
            root_source: r.get(2),
            skill_path: r.get(3),
            description: r.get(4),
            triggers,
            est_tokens: r.get::<_, i32>(6) as usize,
            has_scripts: r.get(7),
            has_references: r.get(8),
            status: r.get(9),
            last_scanned_at: r.get(10),
        }
    }

    fn row_to_ext_svc(r: &postgres::Row) -> ExternalServiceRecord {
        ExternalServiceRecord {
            id: r.get(0),
            service_name: r.get(1),
            category: r.get(2),
            endpoint: r.get(3),
            protocol: r.get(4),
            capabilities: r.get(5),
            auth_type: r.get(6),
            auth_env_var: r.get(7),
            health_endpoint: r.get(8),
            usage_example: r.get(9),
            status: r.get(10),
            last_checked_at: r.get(11),
        }
    }
    /// 原生 PostgreSQL Advisory Lock 尝试加锁（会话级锁，基于 64 位整数哈希）
    pub fn try_advisory_lock(&self, key: i64) -> Result<bool> {
        let mut client = self.lock_client()?;
        let row = client.query_one("SELECT pg_try_advisory_lock($1)", &[&key])?;
        let granted: bool = row.get(0);
        Ok(granted)
    }

    /// 原生 PostgreSQL Advisory Lock 解锁
    pub fn advisory_unlock(&self, key: i64) -> Result<bool> {
        let mut client = self.lock_client()?;
        let row = client.query_one("SELECT pg_advisory_unlock($1)", &[&key])?;
        let unlocked: bool = row.get(0);
        Ok(unlocked)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PG_TEST_URL: &str = "postgresql://postgres@127.0.0.1:5432/agent_body_test";

    fn get_test_provider() -> Option<PgStorageProvider> {
        let url = std::env::var("AGENT_BODY_TEST_PG_URL").unwrap_or_else(|_| PG_TEST_URL.to_string());
        PgStorageProvider::connect(&url).ok()
    }

    #[test]
    fn test_pg_provider_schema_init_and_presets() {
        let provider = match get_test_provider() {
            Some(p) => p,
            None => {
                eprintln!("跳过本地 PG 测试 (本地 PG 未就绪)");
                return;
            }
        };

        assert_eq!(provider.provider_name(), "postgres");

        let rules = provider.get_placement_rules().expect("get placement rules");
        assert_eq!(rules.len(), 8, "PG 应加载 8 条默认放置规则");

        let policies = provider.get_scan_policies().expect("get scan policies");
        assert_eq!(policies.len(), 10, "PG 应加载 10 条默认扫描策略");
    }

    #[test]
    fn test_pg_provider_assets_crud_and_tsvector() {
        let provider = match get_test_provider() {
            Some(p) => p,
            None => return,
        };

        let uid = uuid::Uuid::new_v4();
        let test_path = format!("/root/new/shared_proj/pg_analyzer_{}", uid);
        let asset = AssetRecord {
            id: format!("pg-asset-{}", uid),
            path: test_path.clone(),
            asset_type: "script".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("PostgreSQL 性能诊断与内核分析脚本".to_string()),
            desc_kv: serde_json::json!({"engine": "postgres", "author": "digoal"}),
            usage: Some("python run_pg.py".to_string()),
            size_bytes: Some(4096),
            file_count: Some(2),
            tags: vec!["postgres".to_string(), "analyzer".to_string()],
            status: "active".to_string(),
        };

        provider.upsert_asset(&asset).expect("upsert asset");

        let fetched = provider.get_asset_by_path(&test_path)
            .expect("get asset")
            .expect("must exist");
        assert_eq!(fetched.owner, "user:digoal");

        // 测试 tsvector 全文检索
        let search_res = provider.search_assets_fts("内核分析", 10).expect("fts search");
        assert!(!search_res.is_empty(), "PG tsvector 检索应该成功命中目标资产");
        assert!(search_res.iter().any(|r| r.id == asset.id));
    }

    #[test]
    fn test_pg_provider_property_graph() {
        let provider = match get_test_provider() {
            Some(p) => p,
            None => return,
        };

        let node = FactNodeRecord {
            id: "service:pg_test_daemon".to_string(),
            node_type: "service".to_string(),
            name: "pg_test_daemon".to_string(),
            label: "PG测试守护进程".to_string(),
            properties: serde_json::json!({"port": 5432}),
            updated_at: None,
        };
        provider.upsert_fact_node(&node).expect("upsert fact node");

        let fetched_node = provider.get_fact_node("service:pg_test_daemon")
            .expect("get node")
            .expect("must exist");
        assert_eq!(fetched_node.name, "pg_test_daemon");

        let port_node = FactNodeRecord {
            id: "port:5432".to_string(),
            node_type: "port".to_string(),
            name: "5432".to_string(),
            label: "PostgreSQL端口".to_string(),
            properties: serde_json::json!({}),
            updated_at: None,
        };
        provider.upsert_fact_node(&port_node).expect("upsert port node");

        let edge = FactEdgeRecord::new_explicit(
            "service:pg_test_daemon",
            "port:5432",
            "listens_on",
            serde_json::json!({}),
            1.0,
        );
        provider.upsert_fact_edge(&edge).expect("upsert edge");

        let sub = provider.traverse_fact_graph("service:pg_test_daemon", 1).expect("traverse graph");
        assert_eq!(sub.nodes.len(), 2);
        assert_eq!(sub.edges.len(), 1);
    }

    #[test]
    fn test_pg_provider_phase53_parity() {
        let pg_url = std::env::var("AGENT_BODY_TEST_PG_URL").unwrap_or_else(|_| PG_TEST_URL.to_string());
        let provider = match PgStorageProvider::connect(&pg_url) {
            Ok(p) => p,
            Err(_) => {
                eprintln!("跳过 Phase 53 PG 一致性测试 (PG 暂不可用)");
                return;
            }
        };

        // 1. 验证无需 vector 扩展也可建库，且 memories.sensitive 列存在
        {
            let mut client = provider.lock_client().expect("lock client");
            let row = client
                .query_one(
                    "SELECT COUNT(*) FROM information_schema.columns WHERE table_name = 'memories' AND column_name = 'sensitive'",
                    &[],
                )
                .expect("check sensitive column");
            assert_eq!(row.get::<_, i64>(0), 1);
        }

        let uid = uuid::Uuid::new_v4();

        // 2. upsert_asset 标记 stale 清空向量
        let asset_path = format!("/root/new/work/pg_stale_{}", uid);
        let asset = AssetRecord {
            id: format!("pg-stale-{}", uid),
            path: asset_path.clone(),
            asset_type: "project".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("PG stale embedding test".to_string()),
            desc_kv: serde_json::json!({}),
            usage: None,
            size_bytes: Some(128),
            file_count: Some(1),
            tags: vec!["pg".to_string()],
            status: "active".to_string(),
        };
        provider.upsert_asset(&asset).expect("upsert asset");
        provider
            .update_asset_embedding(&asset_path, &[0.5, 0.5])
            .expect("set emb");
        assert_eq!(
            provider.get_asset_embedding(&asset_path).expect("get emb"),
            vec![0.5, 0.5]
        );
        let mut stale_asset = asset.clone();
        stale_asset.desc_kv = serde_json::json!({"embedding_status": "stale"});
        provider.upsert_asset(&stale_asset).expect("upsert stale");
        assert!(
            provider
                .get_asset_embedding(&asset_path)
                .expect("cleared emb")
                .is_empty()
        );

        // 3. insert_memory ON CONFLICT 全字段更新与空向量保留
        let mem_id = format!("pg-mem-{}", uid);
        let mem = MemoryRecord {
            id: mem_id.clone(),
            memory_type: "experience".to_string(),
            content: format!("PG upsert memory token {}", uid),
            summary: Some("pg s1".to_string()),
            source_agent: Some("agent-pg".to_string()),
            source_session: Some("sess-pg".to_string()),
            tags: vec!["v1".to_string()],
            weight: 0.6,
            pinned: false,
            status: "active".to_string(),
            embedding: vec![0.1, 0.2, 0.3],
        };
        provider.insert_memory(&mem).expect("insert mem");
        let mut mem2 = mem.clone();
        mem2.tags = vec!["v1".to_string(), "v2".to_string()];
        mem2.pinned = true;
        mem2.embedding = Vec::new();
        provider.insert_memory(&mem2).expect("upsert mem empty emb");
        let found = provider
            .search_memories_fts(&uid.to_string(), 5)
            .expect("fts mem");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(found[0].tags, vec!["v1".to_string(), "v2".to_string()]);
        assert!(found[0].pinned);

        // 4. 同日仅过期 5 秒的协作锁必须立即失效并被 GC 清理（回归 NOW()::text ' ' vs RFC3339 'T' Bug）
        let expired_at = (chrono::Utc::now() - chrono::Duration::seconds(5)).to_rfc3339();
        let acquired_at = (chrono::Utc::now() - chrono::Duration::seconds(65)).to_rfc3339();
        let res_id = format!("/root/new/tmp/pg_lock_{}", uid);
        let exp_lock = CoordinationLock {
            lock_id: format!("lock-exp-{}", uid),
            resource_type: "file".to_string(),
            resource_id: res_id.clone(),
            scope: "exclusive".to_string(),
            holder_agent: "agent-old".to_string(),
            holder_pid: Some(1234),
            acquired_at,
            expires_at: expired_at,
            metadata: None,
        };
        assert!(provider.insert_coordination_lock(&exp_lock).expect("insert exp lock"));
        assert!(
            provider
                .get_coordination_lock("file", &res_id)
                .expect("get exp lock")
                .is_none(),
            "同日已过期 5 秒的锁必须不可见"
        );
        assert!(
            provider
                .gc_expired_coordination_locks()
                .expect("gc locks")
                >= 1
        );

        // 5. benchmark_runs awr:% 过滤与删除
        let awr_id = format!("awr:snap:pg-{}", uid);
        let awr_run = BenchmarkRunRecord {
            run_id: awr_id.clone(),
            suite_name: "awr_internal".to_string(),
            git_commit: "pg".to_string(),
            app_version: crate::VERSION.to_string(),
            host_fingerprint: serde_json::json!({}),
            workload_count: 0,
            total_baseline_tokens: 0,
            total_steward_tokens: 0,
            overall_token_savings_pct: 0.0,
            overall_speedup_factor: 1.0,
            total_hazards_intercepted: 0,
            total_noise_blocked: 0,
            sla_passed: true,
            results_json: serde_json::json!({}),
            report_sha256: "awr".to_string(),
            duration_ms: 0,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        provider.insert_benchmark_run(&awr_run).expect("insert awr");
        assert!(
            provider
                .list_benchmark_runs(100)
                .expect("filtered")
                .iter()
                .all(|r| !r.run_id.starts_with("awr:"))
        );
        assert!(
            provider
                .list_all_benchmark_runs(100)
                .expect("all")
                .iter()
                .any(|r| r.run_id == awr_id)
        );
        assert!(provider.delete_benchmark_run(&awr_id).expect("del awr"));

        // 6. Preflight PostgreSQL 连通性探针通过
        let mut cfg = crate::config::Config::default();
        cfg.storage.provider = "postgres".to_string();
        cfg.storage.pg_url = Some(pg_url);
        let mut rep = crate::preflight::PreflightReport::new();
        crate::preflight::PreflightChecker::check_storage_backend(&mut rep, &cfg);
        assert!(rep.passed_required);
    }
}
