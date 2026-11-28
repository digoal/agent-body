use super::PgStorageProvider;
use anyhow::{Result};
use crate::storage::models::*;

impl PgStorageProvider {
    pub(super) fn impl_insert_memory(&self, mem: &MemoryRecord) -> Result<()> {
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


    pub(super) fn impl_search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
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


    pub(super) fn impl_list_all_active_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
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


    pub(super) fn impl_list_all_memories(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
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


    pub(super) fn impl_update_memory_status(&self, memory_id: &str, status: &str) -> Result<usize> {
        let mut client = self.lock_client()?;
        let count = client.execute(
            "UPDATE memories SET status = $2, updated_at = NOW() WHERE id = $1",
            &[&memory_id, &status],
        )?;
        Ok(count as usize)
    }


    pub(super) fn impl_record_change(&self, change: &ChangeRecord) -> Result<()> {
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


    pub(super) fn impl_list_changes(&self, limit: usize) -> Result<Vec<ChangeRecord>> {
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


    pub(super) fn impl_search_memories_fuzzy_db(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        let q_trim = query.trim();
        if q_trim.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let mut client = self.lock_client()?;
        let pat = format!("%{}%", crate::storage::sqlite_provider::assets_search::escape_sql_like(q_trim));
        let rows = client.query(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             WHERE status = 'active'
               AND (content ILIKE $1 ESCAPE '\\' OR coalesce(summary, '') ILIKE $1 ESCAPE '\\' OR tags::text ILIKE $1 ESCAPE '\\')
             ORDER BY pinned DESC, weight DESC, updated_at DESC
             LIMIT $2",
            &[&pat, &(limit as i64)],
        )?;
        let mut results = Vec::with_capacity(rows.len());
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


}
