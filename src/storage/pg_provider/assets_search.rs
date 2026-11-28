use anyhow::Result;
use postgres::types::ToSql;

use super::PgStorageProvider;
use crate::memory::hybrid::cosine_similarity;
use crate::storage::models::{AssetRecord, AssetScalarFilter, DbFuzzyAssetHit};
use crate::storage::sqlite_provider::assets_search::{
    escape_sql_like, extract_line_and_snippet, matches_scalar_in_mem,
};

fn map_pg_asset_row(row: &postgres::Row, offset: usize) -> AssetRecord {
    let tags_val: serde_json::Value = row.get(offset + 11);
    let tags: Vec<String> = serde_json::from_value(tags_val).unwrap_or_default();
    AssetRecord {
        id: row.get(offset),
        path: row.get(offset + 1),
        asset_type: row.get(offset + 2),
        owner: row.get(offset + 3),
        security_level: row.get(offset + 4),
        scan_tier: row.get(offset + 5),
        description: row.get(offset + 6),
        desc_kv: row.get(offset + 7),
        usage: row.get(offset + 8),
        size_bytes: row.get(offset + 9),
        file_count: row.get(offset + 10),
        tags,
        status: row.get(offset + 12),
    }
}

impl PgStorageProvider {
    pub(super) fn impl_update_asset_content(&self, path: &str, content_text: &str) -> Result<()> {
        let mut client = self.lock_client()?;
        client.execute(
            "UPDATE assets SET content_text = $1 WHERE path = $2",
            &[&content_text, &path],
        )?;
        Ok(())
    }

    pub(super) fn impl_get_asset_content(&self, path: &str) -> Result<Option<String>> {
        let mut client = self.lock_client()?;
        let rows = client.query("SELECT content_text FROM assets WHERE path = $1", &[&path])?;
        Ok(rows.first().map(|r| r.get::<_, String>(0)))
    }

    pub(super) fn impl_set_fuzzy_index_enabled(&self, enabled: bool) -> Result<bool> {
        let mut client = self.lock_client()?;
        if enabled {
            let res = client.batch_execute(
                "CREATE EXTENSION IF NOT EXISTS pg_trgm;
                 CREATE INDEX IF NOT EXISTS idx_pg_assets_fuzzy_trgm ON assets USING GIN (
                     (coalesce(path, '') || ' ' || coalesce(description, '') || ' ' || coalesce(content_text, '')) gin_trgm_ops
                 );",
            );
            Ok(res.is_ok())
        } else {
            client.batch_execute("DROP INDEX IF EXISTS idx_pg_assets_fuzzy_trgm;")?;
            Ok(false)
        }
    }

    pub(super) fn impl_search_assets_fuzzy_db(
        &self,
        query: &str,
        limit: usize,
        use_index: bool,
        scope_path: Option<&str>,
    ) -> Result<Vec<DbFuzzyAssetHit>> {
        let q_trim = query.trim();
        if q_trim.is_empty() || limit == 0 {
            return Ok(Vec::new());
        }
        let mut client = self.lock_client()?;
        let has_trgm_idx = use_index
            && client
                .query(
                    "SELECT 1 FROM pg_indexes WHERE tablename = 'assets' AND indexname = 'idx_pg_assets_fuzzy_trgm'",
                    &[],
                )
                .map(|r| !r.is_empty())
                .unwrap_or(false);

        let tokens: Vec<String> = q_trim
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{}%", escape_sql_like(s)))
            .collect();
        let mut sql = String::from(
            "SELECT id, path, type, owner, security_level, scan_tier,
                    description, desc_kv, usage, size_bytes, file_count, tags, status, content_text
             FROM assets WHERE status = 'active'",
        );
        let mut params_owned: Vec<String> = Vec::new();
        let mut idx = 1usize;

        if let Some(sc) = scope_path.map(str::trim).filter(|s| !s.is_empty()) {
            sql.push_str(&format!(" AND path LIKE ${} ESCAPE '\\'", idx));
            params_owned.push(format!("{}%", escape_sql_like(sc)));
            idx += 1;
        }
        for tok in &tokens {
            sql.push_str(&format!(
                " AND (path || ' ' || coalesce(description, '') || ' ' || coalesce(desc_kv::text, '') || ' ' || coalesce(usage, '') || ' ' || coalesce(tags::text, '') || ' ' || coalesce(content_text, '')) ILIKE ${} ESCAPE '\\'",
                idx
            ));
            params_owned.push(tok.clone());
            idx += 1;
        }
        let exact_pat = format!("%{}%", escape_sql_like(q_trim));
        sql.push_str(&format!(
            " ORDER BY CASE WHEN path ILIKE ${} ESCAPE '\\' THEN 0 ELSE 1 END, updated_at DESC LIMIT {}",
            idx, limit
        ));
        params_owned.push(exact_pat);

        let param_refs: Vec<&(dyn ToSql + Sync)> =
            params_owned.iter().map(|s| s as &(dyn ToSql + Sync)).collect();
        let rows = client.query(&sql, &param_refs)?;
        let mode = if has_trgm_idx {
            "db_fuzzy_index"
        } else {
            "db_fuzzy_scan"
        };
        let mut out = Vec::with_capacity(rows.len());
        for (rank_idx, row) in rows.iter().enumerate() {
            let asset = map_pg_asset_row(row, 0);
            let content_text: String = row.get(13);
            let (matched_line, matched_snippet) =
                extract_line_and_snippet(&asset, &content_text, q_trim);
            out.push(DbFuzzyAssetHit {
                asset,
                match_mode: mode.to_string(),
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
        let mut client = self.lock_client()?;
        let mut sql = String::from(
            "SELECT id, path, type, owner, security_level, scan_tier,
                    description, desc_kv, usage, size_bytes, file_count, tags, status
             FROM assets WHERE status = 'active'",
        );
        let mut params_owned: Vec<String> = Vec::new();
        let mut idx = 1usize;
        if let Some(ref sc) = filter.scope_path.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            sql.push_str(&format!(" AND path LIKE ${} ESCAPE '\\'", idx));
            params_owned.push(format!("{}%", escape_sql_like(sc)));
            idx += 1;
        }
        if let Some(ref at) = filter.asset_type.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            sql.push_str(&format!(" AND LOWER(type) = ${}", idx));
            params_owned.push(at.to_lowercase());
        }
        sql.push_str(" ORDER BY updated_at DESC");
        let refs: Vec<&(dyn ToSql + Sync)> =
            params_owned.iter().map(|s| s as &(dyn ToSql + Sync)).collect();
        let rows = client.query(&sql, &refs)?;
        let mut out = Vec::new();
        for row in rows {
            let asset = map_pg_asset_row(&row, 0);
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
        let mut client = self.lock_client()?;
        let rows = if let Some(sc) = scope_path.map(str::trim).filter(|s| !s.is_empty()) {
            let pat = format!("{}%", escape_sql_like(sc));
            client.query(
                "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status, embedding
                 FROM assets WHERE status = 'active' AND embedding IS NOT NULL AND embedding != '' AND embedding != '[]' AND path LIKE $1 ESCAPE '\\'",
                &[&pat],
            )?
        } else {
            client.query(
                "SELECT id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status, embedding
                 FROM assets WHERE status = 'active' AND embedding IS NOT NULL AND embedding != '' AND embedding != '[]'",
                &[],
            )?
        };
        let mut scored = Vec::new();
        for row in rows {
            let asset = map_pg_asset_row(&row, 0);
            let emb_str: String = row.get(13);
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
