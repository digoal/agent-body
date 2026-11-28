use anyhow::Result;
use rusqlite::{params, ToSql};

use super::SqliteStorageProvider;
use crate::memory::hybrid::cosine_similarity;
use crate::scanner::multimodal::infer_modality;
use crate::scanner::multimodal_ext::{matches_kv_filters, parse_kv_filters};
use crate::storage::models::{AssetRecord, AssetScalarFilter, DbFuzzyAssetHit};

pub(super) fn map_asset_row(row: &rusqlite::Row, offset: usize) -> rusqlite::Result<AssetRecord> {
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

pub(crate) fn escape_sql_like(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 4);
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '%' => out.push_str("\\%"),
            '_' => out.push_str("\\_"),
            other => out.push(other),
        }
    }
    out
}

pub(crate) fn extract_line_and_snippet(
    asset: &AssetRecord,
    content_text: &str,
    query: &str,
) -> (Option<usize>, Option<String>) {
    let q_trim = query.trim();
    if q_trim.is_empty() {
        return (None, None);
    }
    let q_lower = q_trim.to_lowercase();
    let tokens: Vec<String> = q_lower
        .split_whitespace()
        .map(str::to_string)
        .filter(|s| !s.is_empty())
        .collect();

    if !content_text.is_empty() {
        let mut fallback_token_match: Option<(usize, String)> = None;
        for (idx, line) in content_text.lines().enumerate() {
            let line_lower = line.to_lowercase();
            if line_lower.contains(&q_lower) {
                return (Some(idx + 1), Some(truncate_snippet(line)));
            }
            if fallback_token_match.is_none()
                && !tokens.is_empty()
                && tokens.iter().all(|t| line_lower.contains(t.as_str()))
            {
                fallback_token_match = Some((idx + 1, truncate_snippet(line)));
            }
        }
        if let Some((ln, snip)) = fallback_token_match {
            return (Some(ln), Some(snip));
        }
        for (idx, line) in content_text.lines().enumerate() {
            let line_lower = line.to_lowercase();
            if tokens.iter().any(|t| line_lower.contains(t.as_str())) {
                return (Some(idx + 1), Some(truncate_snippet(line)));
            }
        }
    }

    if let Some(ref desc) = asset.description {
        let d_lower = desc.to_lowercase();
        if d_lower.contains(&q_lower) || tokens.iter().any(|t| d_lower.contains(t.as_str())) {
            return (None, Some(truncate_snippet(desc)));
        }
    }
    if asset.path.to_lowercase().contains(&q_lower) {
        return (None, Some(asset.path.clone()));
    }
    (None, None)
}

fn truncate_snippet(line: &str) -> String {
    let trimmed = line.trim();
    let mut chars = trimmed.chars();
    let taken: String = chars.by_ref().take(180).collect();
    if chars.next().is_some() {
        format!("{}...", taken)
    } else {
        taken
    }
}

fn bounded_edit_distance(a: &[char], b: &[char], max_dist: usize) -> Option<usize> {
    if a.len().abs_diff(b.len()) > max_dist {
        return None;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        let mut row_min = curr[0];
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            let v = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
            curr[j + 1] = v;
            if v < row_min {
                row_min = v;
            }
        }
        if row_min > max_dist {
            return None;
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    (prev[b.len()] <= max_dist).then_some(prev[b.len()])
}

fn best_typo_match_score(asset: &AssetRecord, content_text: &str, q_lower: &str) -> Option<(f64, String)> {
    let q_chars: Vec<char> = q_lower.chars().collect();
    let max_dist = if q_chars.len() >= 8 { 2 } else { 1 };
    let mut best: Option<(usize, String)> = None;
    let desc_kv_str = asset.desc_kv.to_string();
    let sources = [
        asset.path.as_str(),
        asset.description.as_deref().unwrap_or(""),
        desc_kv_str.as_str(),
        asset.usage.as_deref().unwrap_or(""),
        content_text,
    ];
    for src in sources {
        for tok in src
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|s| s.len() >= 4)
            .take(512)
        {
            let t_lower = tok.to_lowercase();
            let t_chars: Vec<char> = t_lower.chars().collect();
            if let Some(d) = bounded_edit_distance(&q_chars, &t_chars, max_dist) {
                if best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
                    best = Some((d, tok.to_string()));
                }
            }
        }
    }
    best.map(|(d, tok)| (1.0 / (d as f64 + 1.5), format!("fuzzy~={}", tok)))
}

fn trigram_fts_expr(tokens: &[String]) -> Option<String> {
    let parts: Vec<String> = tokens
        .iter()
        .filter(|t| t.chars().count() >= 3)
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" AND "))
    }
}

fn trigram_typo_or_expr(q_trim: &str) -> Option<String> {
    let chars: Vec<char> = q_trim.chars().collect();
    if chars.len() < 5 {
        return None;
    }
    let mut grams = Vec::new();
    let mid = chars.len() / 2;
    for start in [0usize, mid.saturating_sub(1), chars.len().saturating_sub(3)] {
        if start + 3 <= chars.len() {
            let g: String = chars[start..start + 3].iter().collect();
            let quoted = format!("\"{}\"", g.replace('"', "\"\""));
            if !grams.contains(&quoted) {
                grams.push(quoted);
            }
        }
    }
    (!grams.is_empty()).then(|| grams.join(" OR "))
}

impl SqliteStorageProvider {
    pub(super) fn impl_update_asset_content(&self, path: &str, content_text: &str) -> Result<()> {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::IoSqliteWrite);
        let conn = self.lock_conn()?;
        conn.execute(
            "UPDATE assets SET content_text = ?1 WHERE path = ?2",
            params![content_text, path],
        )?;
        Ok(())
    }

    pub(super) fn impl_get_asset_content(&self, path: &str) -> Result<Option<String>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare("SELECT content_text FROM assets WHERE path = ?1")?;
        let mut rows = stmt.query(params![path])?;
        if let Some(row) = rows.next()? {
            let txt: String = row.get(0)?;
            Ok(Some(txt))
        } else {
            Ok(None)
        }
    }

    pub(super) fn impl_set_fuzzy_index_enabled(&self, enabled: bool) -> Result<bool> {
        let conn = self.lock_conn()?;
        if enabled {
            conn.execute_batch(
                "CREATE VIRTUAL TABLE IF NOT EXISTS assets_trigram_fts USING fts5(
                     path, description, desc_kv, tags, content_text,
                     content=assets, content_rowid=rowid, tokenize='trigram'
                 );
                 CREATE TRIGGER IF NOT EXISTS assets_trigram_fts_insert AFTER INSERT ON assets BEGIN
                     INSERT INTO assets_trigram_fts(rowid, path, description, desc_kv, tags, content_text)
                     VALUES (NEW.rowid, NEW.path, NEW.description, NEW.desc_kv, NEW.tags, NEW.content_text);
                 END;
                 CREATE TRIGGER IF NOT EXISTS assets_trigram_fts_delete AFTER DELETE ON assets BEGIN
                     INSERT INTO assets_trigram_fts(assets_trigram_fts, rowid, path, description, desc_kv, tags, content_text)
                     VALUES ('delete', OLD.rowid, OLD.path, OLD.description, OLD.desc_kv, OLD.tags, OLD.content_text);
                 END;
                 CREATE TRIGGER IF NOT EXISTS assets_trigram_fts_update AFTER UPDATE ON assets BEGIN
                     INSERT INTO assets_trigram_fts(assets_trigram_fts, rowid, path, description, desc_kv, tags, content_text)
                     VALUES ('delete', OLD.rowid, OLD.path, OLD.description, OLD.desc_kv, OLD.tags, OLD.content_text);
                     INSERT INTO assets_trigram_fts(rowid, path, description, desc_kv, tags, content_text)
                     VALUES (NEW.rowid, NEW.path, NEW.description, NEW.desc_kv, NEW.tags, NEW.content_text);
                 END;
                 INSERT INTO assets_trigram_fts(assets_trigram_fts) VALUES ('rebuild');",
            )?;
            Ok(true)
        } else {
            conn.execute_batch(
                "DROP TRIGGER IF EXISTS assets_trigram_fts_insert;
                 DROP TRIGGER IF EXISTS assets_trigram_fts_delete;
                 DROP TRIGGER IF EXISTS assets_trigram_fts_update;
                 DROP TABLE IF EXISTS assets_trigram_fts;",
            )?;
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
        let conn = self.lock_conn()?;
        let trigram_ready: bool = use_index
            && conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='assets_trigram_fts'",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .unwrap_or(0)
                > 0;

        let q_lower = q_trim.to_lowercase();
        let tokens: Vec<String> = q_trim
            .split_whitespace()
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        let scope_like = scope_path
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| format!("{}%", escape_sql_like(s)));

        if trigram_ready {
            if let Some(fts_expr) = trigram_fts_expr(&tokens) {
                let mut hits = self.query_trigram_candidates(
                    &conn,
                    &fts_expr,
                    &tokens,
                    scope_like.as_deref(),
                    q_trim,
                    limit,
                    "db_fuzzy_index",
                )?;
                if hits.is_empty() && tokens.len() == 1 {
                    if let Some(or_expr) = trigram_typo_or_expr(q_trim) {
                        let cand_rows = self.query_trigram_raw_rows(
                            &conn,
                            &or_expr,
                            scope_like.as_deref(),
                            200,
                        )?;
                        for (asset, content_text, _) in cand_rows {
                            if let Some((rank_score, note)) =
                                best_typo_match_score(&asset, &content_text, &q_lower)
                            {
                                hits.push(DbFuzzyAssetHit {
                                    asset,
                                    match_mode: "db_fuzzy_index".to_string(),
                                    rank_score,
                                    matched_line: None,
                                    matched_snippet: Some(note),
                                });
                                if hits.len() >= limit {
                                    break;
                                }
                            }
                        }
                    }
                }
                return Ok(hits);
            }
        }

        // DB-side SQL LIKE fallback (short <3 char queries, trigram disabled, or non-indexed fallback)
        self.query_like_fuzzy_db(&conn, &tokens, &q_lower, scope_like.as_deref(), limit)
    }

    #[allow(clippy::too_many_arguments)]
    fn query_trigram_candidates(
        &self,
        conn: &rusqlite::Connection,
        fts_expr: &str,
        all_tokens: &[String],
        scope_like: Option<&str>,
        q_raw: &str,
        limit: usize,
        match_mode: &str,
    ) -> Result<Vec<DbFuzzyAssetHit>> {
        let mut sql = String::from(
            "SELECT a.id, a.path, a.type, a.owner, a.security_level, a.scan_tier,
                    a.description, a.desc_kv, a.usage, a.size_bytes, a.file_count, a.tags, a.status,
                    a.content_text, bm25(assets_trigram_fts) AS rank_bm25
             FROM assets_trigram_fts
             JOIN assets a ON a.rowid = assets_trigram_fts.rowid
             WHERE a.status = 'active' AND assets_trigram_fts MATCH ?1",
        );
        let mut dyn_params: Vec<Box<dyn ToSql>> = vec![Box::new(fts_expr.to_string())];
        let mut idx = 2usize;

        if let Some(sc) = scope_like {
            sql.push_str(&format!(" AND a.path LIKE ?{} ESCAPE '\\'", idx));
            dyn_params.push(Box::new(sc.to_string()));
            idx += 1;
        }
        // Any short (<3 char) tokens from mixed queries must also match in SQL
        for tok in all_tokens.iter().filter(|t| t.chars().count() < 3) {
            let pat = format!("%{}%", escape_sql_like(&tok.to_lowercase()));
            sql.push_str(&format!(
                " AND (LOWER(a.path) LIKE ?{0} ESCAPE '\\' OR LOWER(COALESCE(a.description,'')) LIKE ?{0} ESCAPE '\\' OR LOWER(COALESCE(a.tags,'')) LIKE ?{0} ESCAPE '\\' OR LOWER(a.content_text) LIKE ?{0} ESCAPE '\\')",
                idx
            ));
            dyn_params.push(Box::new(pat));
            idx += 1;
        }
        let path_exact = format!("%{}%", escape_sql_like(&q_raw.trim().to_lowercase()));
        sql.push_str(&format!(
            " ORDER BY CASE WHEN LOWER(a.path) LIKE ?{} ESCAPE '\\' THEN 0 ELSE 1 END, rank_bm25 ASC LIMIT ?{}",
            idx,
            idx + 1
        ));
        dyn_params.push(Box::new(path_exact));
        dyn_params.push(Box::new(limit as i64));

        let param_refs: Vec<&dyn ToSql> = dyn_params.iter().map(|b| b.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(param_refs.as_slice(), |row| {
            let asset = map_asset_row(row, 0)?;
            let content_text: String = row.get(13)?;
            let bm25_val: f64 = row.get(14).unwrap_or(-1.0);
            Ok((asset, content_text, bm25_val))
        })?;

        let mut out = Vec::new();
        for r in rows {
            let (asset, content_text, bm25_val) = r?;
            let (matched_line, matched_snippet) =
                extract_line_and_snippet(&asset, &content_text, q_raw);
            let rank_score = (-bm25_val).max(0.1) + if matched_line.is_some() { 1.0 } else { 0.0 };
            out.push(DbFuzzyAssetHit {
                asset,
                match_mode: match_mode.to_string(),
                rank_score,
                matched_line,
                matched_snippet,
            });
        }
        Ok(out)
    }

    fn query_trigram_raw_rows(
        &self,
        conn: &rusqlite::Connection,
        fts_expr: &str,
        scope_like: Option<&str>,
        limit: usize,
    ) -> Result<Vec<(AssetRecord, String, f64)>> {
        let mut sql = String::from(
            "SELECT a.id, a.path, a.type, a.owner, a.security_level, a.scan_tier,
                    a.description, a.desc_kv, a.usage, a.size_bytes, a.file_count, a.tags, a.status,
                    a.content_text, bm25(assets_trigram_fts) AS rank_bm25
             FROM assets_trigram_fts
             JOIN assets a ON a.rowid = assets_trigram_fts.rowid
             WHERE a.status = 'active' AND assets_trigram_fts MATCH ?1",
        );
        let mut dyn_params: Vec<Box<dyn ToSql>> = vec![Box::new(fts_expr.to_string())];
        if let Some(sc) = scope_like {
            sql.push_str(" AND a.path LIKE ?2 ESCAPE '\\' ORDER BY rank_bm25 ASC LIMIT ?3");
            dyn_params.push(Box::new(sc.to_string()));
        } else {
            sql.push_str(" ORDER BY rank_bm25 ASC LIMIT ?2");
        }
        dyn_params.push(Box::new(limit as i64));
        let refs: Vec<&dyn ToSql> = dyn_params.iter().map(|b| b.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(refs.as_slice(), |row| {
                Ok((map_asset_row(row, 0)?, row.get(13)?, row.get(14).unwrap_or(-1.0)))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn query_like_fuzzy_db(
        &self,
        conn: &rusqlite::Connection,
        tokens: &[String],
        q_lower: &str,
        scope_like: Option<&str>,
        limit: usize,
    ) -> Result<Vec<DbFuzzyAssetHit>> {
        let mut sql = String::from(
            "SELECT id, path, type, owner, security_level, scan_tier,
                    description, desc_kv, usage, size_bytes, file_count, tags, status, content_text
             FROM assets WHERE status = 'active'",
        );
        let mut dyn_params: Vec<Box<dyn ToSql>> = Vec::new();
        let mut idx = 1usize;
        if let Some(sc) = scope_like {
            sql.push_str(&format!(" AND path LIKE ?{} ESCAPE '\\'", idx));
            dyn_params.push(Box::new(sc.to_string()));
            idx += 1;
        }
        for tok in tokens {
            let pat = format!("%{}%", escape_sql_like(&tok.to_lowercase()));
            sql.push_str(&format!(
                " AND (LOWER(path) LIKE ?{0} ESCAPE '\\' OR LOWER(COALESCE(description,'')) LIKE ?{0} ESCAPE '\\' OR LOWER(COALESCE(desc_kv,'')) LIKE ?{0} ESCAPE '\\' OR LOWER(COALESCE(usage,'')) LIKE ?{0} ESCAPE '\\' OR LOWER(COALESCE(tags,'')) LIKE ?{0} ESCAPE '\\' OR LOWER(content_text) LIKE ?{0} ESCAPE '\\')",
                idx
            ));
            dyn_params.push(Box::new(pat));
            idx += 1;
        }
        let exact_pat = format!("%{}%", escape_sql_like(q_lower));
        sql.push_str(&format!(
            " ORDER BY CASE WHEN LOWER(path) LIKE ?{} ESCAPE '\\' THEN 0 ELSE 1 END, updated_at DESC LIMIT ?{}",
            idx,
            idx + 1
        ));
        dyn_params.push(Box::new(exact_pat));
        dyn_params.push(Box::new(limit as i64));

        let refs: Vec<&dyn ToSql> = dyn_params.iter().map(|b| b.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let mut out = Vec::new();
        let rows = stmt.query_map(refs.as_slice(), |row| {
            Ok((map_asset_row(row, 0)?, row.get::<_, String>(13)?))
        })?;
        for (idx_rank, row_res) in rows.enumerate() {
            let (asset, content_text) = row_res?;
            let (matched_line, matched_snippet) =
                extract_line_and_snippet(&asset, &content_text, q_lower);
            out.push(DbFuzzyAssetHit {
                asset,
                match_mode: "db_fuzzy_scan".to_string(),
                rank_score: 1.0 / (idx_rank as f64 + 1.0) + if matched_line.is_some() { 0.8 } else { 0.0 },
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
            idx += 1;
        }
        if let Some(ref k) = filter.kv_key.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            sql.push_str(&format!(" AND desc_kv LIKE ?{} ESCAPE '\\'", idx));
            dyn_params.push(Box::new(format!("%\"{}%", escape_sql_like(k))));
            idx += 1;
        }
        if let Some(ref dom) = filter.domain.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            sql.push_str(&format!(" AND LOWER(desc_kv) LIKE ?{} ESCAPE '\\'", idx));
            dyn_params.push(Box::new(format!("%{}%", escape_sql_like(&dom.to_lowercase()))));
        }
        sql.push_str(" ORDER BY updated_at DESC");

        let refs: Vec<&dyn ToSql> = dyn_params.iter().map(|b| b.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(refs.as_slice(), |row| map_asset_row(row, 0))?;
        let mut out = Vec::new();
        for row_res in rows {
            let asset = row_res?;
            if !matches_scalar_in_mem(&asset, filter) {
                continue;
            }
            out.push(asset);
            if out.len() >= limit {
                break;
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
        let scope_like = scope_path
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| format!("{}%", escape_sql_like(s)));

        let mut sql = String::from(
            "SELECT id, path, type, owner, security_level, scan_tier,
                    description, desc_kv, usage, size_bytes, file_count, tags, status, embedding
             FROM assets
             WHERE status = 'active' AND embedding IS NOT NULL AND embedding != '' AND embedding != '[]'",
        );
        let mut dyn_params: Vec<Box<dyn ToSql>> = Vec::new();
        if let Some(sc) = scope_like {
            sql.push_str(" AND path LIKE ?1 ESCAPE '\\'");
            dyn_params.push(Box::new(sc));
        }
        let refs: Vec<&dyn ToSql> = dyn_params.iter().map(|b| b.as_ref()).collect();
        let mut stmt = conn.prepare(&sql)?;
        let mut scored: Vec<(AssetRecord, f32)> = Vec::new();
        let rows = stmt.query_map(refs.as_slice(), |row| {
            let asset = map_asset_row(row, 0)?;
            let emb_str: String = row.get(13)?;
            Ok((asset, emb_str))
        })?;
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

pub(crate) fn matches_scalar_in_mem(asset: &AssetRecord, filter: &AssetScalarFilter) -> bool {
    if let Some(ref sc) = filter.scope_path.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if !asset.path.starts_with(sc) {
            return false;
        }
    }
    if let Some(ref m) = filter.modality.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let ext = std::path::Path::new(&asset.path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        let inferred = infer_modality(&asset.asset_type, ext);
        let kv_mod = asset
            .desc_kv
            .get("modality")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !inferred.eq_ignore_ascii_case(m) && !kv_mod.eq_ignore_ascii_case(m) {
            return false;
        }
    }
    if let Some(ref at) = filter.asset_type.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        if !asset.asset_type.eq_ignore_ascii_case(at) {
            return false;
        }
    }
    if let Some(ref dom) = filter.domain.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let dom_val = asset
            .desc_kv
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !dom_val.to_lowercase().contains(&dom.to_lowercase()) {
            return false;
        }
    }
    if let Some(ref key) = filter.kv_key.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let Some(v) = asset.desc_kv.get(key) else {
            return false;
        };
        if let Some(ref expected) = filter.kv_value.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            let actual = v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string());
            if !actual.to_lowercase().contains(&expected.to_lowercase()) {
                return false;
            }
        }
    }
    if let Some(ref expr) = filter.kv_filter.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let preds = parse_kv_filters(expr);
        if !matches_kv_filters(asset, &preds) {
            return false;
        }
    }
    true
}
