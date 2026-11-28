use super::DuckDbStorageProvider;
use anyhow::{Result};
use duckdb::params;
use crate::storage::models::*;

impl DuckDbStorageProvider {
    pub(super) fn impl_insert_memory(&self, mem: &MemoryRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let tags_str = serde_json::to_string(&mem.tags)?;
        let embedding_str = serde_json::to_string(&mem.embedding)?;
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO memories (id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)
             ON CONFLICT(id) DO UPDATE SET
                type = excluded.type,
                content = excluded.content,
                summary = excluded.summary,
                source_agent = COALESCE(excluded.source_agent, memories.source_agent),
                source_session = COALESCE(excluded.source_session, memories.source_session),
                tags = excluded.tags,
                weight = excluded.weight,
                pinned = excluded.pinned,
                status = excluded.status,
                embedding = CASE WHEN excluded.embedding != '[]' THEN excluded.embedding ELSE memories.embedding END,
                updated_at = excluded.updated_at",
            params![
                mem.id,
                mem.memory_type,
                mem.content,
                mem.summary,
                mem.source_agent,
                mem.source_session,
                tags_str,
                mem.weight as f64,
                mem.pinned,
                mem.status,
                embedding_str,
                now,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        let conn = self.lock_conn()?;
        let query_pattern = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             WHERE (content ILIKE ?1 OR summary ILIKE ?1 OR tags ILIKE ?1) AND status = 'active'
             ORDER BY updated_at DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![query_pattern, limit as i64], |row| {
            let tags_str: String = row.get(6)?;
            let emb_str: String = row.get(10)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();
            let weight_f: f64 = row.get(7)?;

            Ok(MemoryRecord {
                id: row.get(0)?,
                memory_type: row.get(1)?,
                content: row.get(2)?,
                summary: row.get(3)?,
                source_agent: row.get(4)?,
                source_session: row.get(5)?,
                tags,
                weight: weight_f as f32,
                pinned: row.get(8)?,
                status: row.get(9)?,
                embedding,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_list_all_active_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
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
            let emb_str: String = row.get(10)?;
            let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
            let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();
            let weight_f: f64 = row.get(7)?;

            Ok(MemoryRecord {
                id: row.get(0)?,
                memory_type: row.get(1)?,
                content: row.get(2)?,
                summary: row.get(3)?,
                source_agent: row.get(4)?,
                source_session: row.get(5)?,
                tags,
                weight: weight_f as f32,
                pinned: row.get(8)?,
                status: row.get(9)?,
                embedding,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_list_all_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             ORDER BY updated_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt
            .query_map(params![limit as i64], |row| {
                let tags_str: String = row.get(6)?;
                let emb_str: String = row.get(10)?;
                let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
                let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();
                let weight_f: f64 = row.get(7)?;

                Ok(MemoryRecord {
                    id: row.get(0)?,
                    memory_type: row.get(1)?,
                    content: row.get(2)?,
                    summary: row.get(3)?,
                    source_agent: row.get(4)?,
                    source_session: row.get(5)?,
                    tags,
                    weight: weight_f as f32,
                    pinned: row.get(8)?,
                    status: row.get(9)?,
                    embedding,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_update_memory_status(&self, memory_id: &str, status: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let now = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE memories SET status = ?1, updated_at = ?3 WHERE id = ?2",
            params![status, memory_id, now],
        )?;
        Ok(rows)
    }


    pub(super) fn impl_record_change(&self, change: &ChangeRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        let details_str = serde_json::to_string(&change.details)?;
        let created_at = change
            .created_at
            .clone()
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        conn.execute(
            "INSERT INTO changes (type, target, description, details, source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                change.change_type,
                change.target,
                change.description,
                details_str,
                change.source,
                created_at,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_list_changes(&self, limit: usize) -> Result<Vec<ChangeRecord>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, type, target, description, details, source, created_at
             FROM changes
             ORDER BY created_at DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let details_str: String = row.get(4)?;
            let details: serde_json::Value = serde_json::from_str(&details_str).unwrap_or(serde_json::json!({}));

            Ok(ChangeRecord {
                id: row.get(0)?,
                change_type: row.get(1)?,
                target: row.get(2)?,
                description: row.get(3)?,
                details,
                source: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_search_memories_fuzzy_db(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        let q_trim = query.trim();
        if q_trim.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.lock_conn()?;
        let pat = format!("%{}%", crate::storage::sqlite_provider::assets_search::escape_sql_like(q_trim));
        let mut stmt = conn.prepare(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             WHERE status = 'active'
               AND (content ILIKE ?1 ESCAPE '\\' OR COALESCE(summary, '') ILIKE ?1 ESCAPE '\\' OR tags ILIKE ?1 ESCAPE '\\')
             ORDER BY pinned DESC, weight DESC, updated_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![pat, limit as i64], |row| {
                let tags_str: String = row.get(6)?;
                let emb_str: String = row.get(10)?;
                let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
                let embedding: Vec<f32> = serde_json::from_str(&emb_str).unwrap_or_default();
                let weight_f: f64 = row.get(7)?;
                Ok(MemoryRecord {
                    id: row.get(0)?,
                    memory_type: row.get(1)?,
                    content: row.get(2)?,
                    summary: row.get(3)?,
                    source_agent: row.get(4)?,
                    source_session: row.get(5)?,
                    tags,
                    weight: weight_f as f32,
                    pinned: row.get(8)?,
                    status: row.get(9)?,
                    embedding,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }


}
