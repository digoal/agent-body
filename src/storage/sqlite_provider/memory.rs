use super::SqliteStorageProvider;
use anyhow::{Result};
use rusqlite::params;
use crate::storage::models::*;

impl SqliteStorageProvider {
    pub(super) fn impl_insert_memory(&self, mem: &MemoryRecord) -> Result<()> {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::IoSqliteWrite);
        let conn = self.lock_conn()?;
        let tags_str = serde_json::to_string(&mem.tags)?;
        let emb_str = serde_json::to_string(&mem.embedding)?;

        conn.execute(
            "INSERT INTO memories (id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
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
                updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now')",
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


    pub(super) fn impl_search_memories_fts(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        let candidates = super::build_fts_query_candidates(query);
        if candidates.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT m.id, m.type, m.content, m.summary, m.source_agent, m.source_session, m.tags, m.weight, m.pinned, m.status, m.embedding
             FROM memories_fts f
             JOIN memories m ON f.rowid = m.rowid
             WHERE memories_fts MATCH ?1 AND m.status = 'active'
             ORDER BY bm25(memories_fts, 1.5, 3.0, 2.5)
             LIMIT ?2"
        )?;

        let mut merged: Vec<MemoryRecord> = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();
        for fts_q in &candidates {
            let Ok(mapped) = stmt.query_map(params![fts_q, limit as i64], |row| {
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
            }) else {
                continue;
            };
            for row in mapped.flatten() {
                if seen_ids.insert(row.id.clone()) {
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

        if merged.is_empty() {
            let kw = format!("%{}%", query.trim().to_lowercase());
            let mut like_stmt = conn.prepare(
                "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
                 FROM memories
                 WHERE status = 'active'
                   AND (lower(content) LIKE ?1 OR lower(COALESCE(summary, '')) LIKE ?1 OR lower(tags) LIKE ?1)
                 ORDER BY updated_at DESC
                 LIMIT ?2",
            )?;
            let rows = like_stmt.query_map(params![kw, limit as i64], |row| {
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
            })?;
            for r in rows {
                let rec = r?;
                if seen_ids.insert(rec.id.clone()) {
                    merged.push(rec);
                    if merged.len() >= limit {
                        break;
                    }
                }
            }
        }

        Ok(merged)
    }


    pub(super) fn impl_search_memories_fuzzy_db(&self, query: &str, limit: usize) -> Result<Vec<MemoryRecord>> {
        let q_trim = query.trim();
        if q_trim.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.lock_conn()?;
        let pat = format!("%{}%", super::assets_search::escape_sql_like(&q_trim.to_lowercase()));
        let mut stmt = conn.prepare(
            "SELECT id, type, content, summary, source_agent, source_session, tags, weight, pinned, status, embedding
             FROM memories
             WHERE status = 'active'
               AND (LOWER(content) LIKE ?1 ESCAPE '\\'
                    OR LOWER(COALESCE(summary, '')) LIKE ?1 ESCAPE '\\'
                    OR LOWER(tags) LIKE ?1 ESCAPE '\\')
             ORDER BY pinned DESC, weight DESC, updated_at DESC
             LIMIT ?2",
        )?;
        let rows = stmt
            .query_map(params![pat, limit as i64], |row| {
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
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
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
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }


    pub(super) fn impl_update_memory_status(&self, memory_id: &str, status: &str) -> Result<usize> {
        let conn = self.lock_conn()?;
        let rows = conn.execute(
            "UPDATE memories SET status = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now') WHERE id = ?1",
            params![memory_id, status],
        )?;
        Ok(rows)
    }


    pub(super) fn impl_record_change(&self, change: &ChangeRecord) -> Result<()> {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::IoSqliteWrite);
        let conn = self.lock_conn()?;
        let details_str = serde_json::to_string(&change.details)?;
        conn.execute(
            "INSERT INTO changes (type, target, description, details, source, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, COALESCE(?6, strftime('%Y-%m-%dT%H:%M:%f+00:00','now')))",
            params![
                change.change_type,
                change.target,
                change.description,
                details_str,
                change.source,
                change.created_at,
            ],
        )?;
        Ok(())
    }


    pub(super) fn impl_list_changes(&self, limit: usize) -> Result<Vec<ChangeRecord>> {
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


}
