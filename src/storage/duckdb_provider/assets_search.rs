use anyhow::Result;
use duckdb::{params, ToSql};

use super::DuckDbStorageProvider;
use crate::memory::hybrid::cosine_similarity;
use crate::storage::models::{AssetRecord, AssetScalarFilter, DbFuzzyAssetHit};
use crate::storage::sqlite_provider::assets_search::{
    escape_sql_like, extract_line_and_snippet, matches_scalar_in_mem,
};

fn map_duck_asset_row(row: &duckdb::Row, offset: usize) -> duckdb::Result<AssetRecord> {
    let desc_kv_str: String = row.get(offset + 7)?;
    let tags_str: String = row.get(offset + 11)?;
    let desc_kv: serde_json::Value =
        serde_json::from_str(&desc_kv_str).unwrap_or_else(|_| serde_json::json!({}));
    let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
    Ok(AssetRecord {
        id: row.get(offset)?,
        path: row.get(offset + 1)?,
        asset_type: row.get(offset + 2)?,
        owner: row.get(offset + 3)?,
        security_level: row.get(offset + 4)?,
        scan_tier: row.get(offset + 5)?,
        description: row.get(offset + 6)?,
        desc_kv,
        usage: row.get(offset + 8)?,
        size_bytes: row.get(offset + 9)?,
        file_count: row.get(offset + 10)?,
        tags,
        status: row.get(offset + 12)?,
    })
}

impl DuckDbStorageProvider {
    pub(super) fn impl_update_asset_content(&self, path: &str, content_text: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "UPDATE assets SET content_text = ?1 WHERE path = ?2",
            params![content_text, path],
        )?;
        Ok(())
    }

    pub(super) fn impl_get_asset_content(&self, path: &str) -> Result<Option<String>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT COALESCE(content_text, '') FROM assets WHERE path = ?1")?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            Ok(Some(row.get::<_, String>(0)?))
        } else {
            Ok(None)
        }
    }

    pub(super) fn impl_search_assets_fuzzy_db(
        &self,
        query: &str,
        limit: usize,
        _use_index: bool,
        scope_path: Option<&str>,
    ) -> Result<Vec<DbFuzzyAssetHit>> {
        let q_trim = query.trim();
        if q_trim.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.lock_conn()?;
        let tokens: Vec<String> = q_trim
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{}%", escape_sql_like(s)))
            .collect();

        let mut sql = String::from(
            "SELECT id, path, type, owner, security_level, scan_tier,
                    description, desc_kv, usage, size_bytes, file_count, tags, status, COALESCE(content_text, '')
             FROM assets WHERE status = 'active'",
        );
        let mut dyn_params: Vec<Box<dyn ToSql>> = Vec::new();
        let mut idx = 1usize;
        if let Some(sc) = scope_path.map(str::trim).filter(|s| !s.is_empty()) {
            sql.push_str(&format!(" AND path LIKE ?{} ESCAPE '\\'", idx));
            dyn_params.push(Box::new(format!("{}%", escape_sql_like(sc))));
            idx += 1;
        }
        for tok in &tokens {
            sql.push_str(&format!(
                " AND (path ILIKE ?{0} ESCAPE '\\' OR COALESCE(description,'') ILIKE ?{0} ESCAPE '\\' OR COALESCE(desc_kv,'') ILIKE ?{0} ESCAPE '\\' OR COALESCE(usage,'') ILIKE ?{0} ESCAPE '\\' OR COALESCE(tags,'') ILIKE ?{0} ESCAPE '\\' OR COALESCE(content_text,'') ILIKE ?{0} ESCAPE '\\')",
                idx
            ));
            dyn_params.push(Box::new(tok.clone()));
            idx += 1;
        }
        let exact_pat = format!("%{}%", escape_sql_like(q_trim));
        sql.push_str(&format!(
            " ORDER BY CASE WHEN path ILIKE ?{} ESCAPE '\\' THEN 0 ELSE 1 END, updated_at DESC LIMIT ?{}",
            idx,
            idx + 1
        ));
        dyn_params.push(Box::new(exact_pat));
        dyn_params.push(Box::new(limit as i64));

        let refs: Vec<&dyn ToSql> = dyn_params.iter().map(|b| b.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(refs.as_slice(), |row| {
            Ok((map_duck_asset_row(row, 0)?, row.get::<_, String>(13)?))
        })?;
        let mut out = Vec::new();
        for (rank_idx, row_res) in rows.enumerate() {
            let (asset, content_text) = row_res?;
            let (matched_line, matched_snippet) =
                extract_line_and_snippet(&asset, &content_text, q_trim);
            out.push(DbFuzzyAssetHit {
                asset,
                match_mode: "db_fuzzy_scan".to_string(),
                rank_score: 1.0 / (rank_idx as f64 + 1.0) + if matched_line.is_some() { 0.8 } else { 0.0 },
                matched_line,
                matched_snippet,
            });
        }
        Ok(out)
    }

    pub(super) fn impl_search_assets_scalar_db(
        &self,
        filter: &AssetScalarFilter,
        limit: usize,
    ) -> Result<Vec<AssetRecord>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.lock_conn()?;
        let mut sql = String::from(
            "SELECT id, path, type, owner, security_level, scan_tier,
                    description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets WHERE status = 'active'",
        );
        let mut dyn_params: Vec<Box<dyn ToSql>> = Vec::new();
        let mut idx = 1usize;
        if let Some(ref sc) = filter.scope_path.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            sql.push_str(&format!(" AND path LIKE ?{} ESCAPE '\\'", idx));
            dyn_params.push(Box::new(format!("{}%", escape_sql_like(sc))));
            idx += 1;
        }
        if let Some(ref at) = filter.asset_type.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            sql.push_str(&format!(" AND LOWER(type) = ?{}", idx));
            dyn_params.push(Box::new(at.to_lowercase()));
        }
        sql.push_str(" ORDER BY updated_at DESC");
        let refs: Vec<&dyn ToSql> = dyn_params.iter().map(|b| b.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(refs.as_slice(), |row| map_duck_asset_row(row, 0))?;
        let mut out = Vec::new();
        for r in rows {
            let asset = r?;
            if matches_scalar_in_mem(&asset, filter) {
                out.push(asset);
                if out.len() >= limit {
                    break;
                }
            }
        }
        Ok(out)
    }

    pub(super) fn impl_search_assets_vector_db(
        &self,
        query_vec: &[f32],
        min_sim: f32,
        limit: usize,
        scope_path: Option<&str>,
    ) -> Result<Vec<(AssetRecord, f32)>> {
        if query_vec.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.lock_conn()?;
        let mut sql = String::from(
            "SELECT id, path, type, owner, security_level, scan_tier,
                    description, desc_kv, usage, size_bytes, file_count, tags, status, embedding
             FROM assets WHERE status = 'active' AND embedding IS NOT NULL AND embedding != '' AND embedding != '[]'",
        );
        let mut dyn_params: Vec<Box<dyn ToSql>> = Vec::new();
        if let Some(sc) = scope_path.map(str::trim).filter(|s| !s.is_empty()) {
            sql.push_str(" AND path LIKE ?1 ESCAPE '\\'");
            dyn_params.push(Box::new(format!("{}%", escape_sql_like(sc))));
        }
        let refs: Vec<&dyn ToSql> = dyn_params.iter().map(|b| b.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(refs.as_slice(), |row| {
            Ok((map_duck_asset_row(row, 0)?, row.get::<_, String>(13)?))
        })?;
        let mut scored = Vec::new();
        for r in rows {
            let (asset, emb_str) = r?;
            let Ok(vec) = serde_json::from_str::<Vec<f32>>(&emb_str) else {
                continue;
            };
            if vec.len() != query_vec.len() {
                continue;
            }
            let sim = cosine_similarity(query_vec, &vec);
            if sim.is_finite() && sim >= min_sim {
                scored.push((asset, sim));
            }
        }
        scored.sort_by(|a, b| b.1.total_cmp(&a.1));
        scored.truncate(limit);
        Ok(scored)
    }
}
