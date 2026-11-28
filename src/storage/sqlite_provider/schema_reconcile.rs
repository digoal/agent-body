use std::collections::HashSet;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};

/// 列出指定 SQLite 表当前所有列名（保持原始顺序）
fn list_table_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let sql = format!("PRAGMA table_info(\"{}\")", table.replace('"', "\"\""));
    let mut stmt = conn
        .prepare(&sql)
        .with_context(|| format!("读取 SQLite 表 {} 列信息失败", table))?;
    let cols = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .with_context(|| format!("解析 SQLite 表 {} 列信息失败", table))?;
    Ok(cols)
}

/// 校验 SQLite 标识符仅含安全 ASCII 字符，防止 DDL 注入
fn is_safe_ident(ident: &str) -> bool {
    !ident.is_empty()
        && ident
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// 当目标规范列缺失且存在满足匹配条件的旧版命名列时，无损重命名列（100% 保留历史行）
fn rename_legacy_column_if_needed<F>(
    conn: &Connection,
    table: &str,
    canonical_col: &str,
    legacy_matcher: F,
) -> Result<bool>
where
    F: Fn(&str) -> bool,
{
    let cols = list_table_columns(conn, table)?;
    if cols.is_empty() || cols.iter().any(|c| c == canonical_col) {
        return Ok(false);
    }

    let Some(legacy_col) = cols.iter().find(|c| legacy_matcher(c.as_str())) else {
        return Ok(false);
    };

    if !is_safe_ident(table) || !is_safe_ident(legacy_col) || !is_safe_ident(canonical_col) {
        anyhow::bail!(
            "拒绝执行不安全列重命名: {}.{} -> {}",
            table,
            legacy_col,
            canonical_col
        );
    }

    let ddl = format!(
        "ALTER TABLE {} RENAME COLUMN {} TO {}",
        table, legacy_col, canonical_col
    );
    conn.execute(&ddl, []).with_context(|| {
        format!(
            "SQLite 无损重命名历史列失败 ({}.{} -> {})",
            table, legacy_col, canonical_col
        )
    })?;
    eprintln!(
        "[SQLite][MIGRATE] 已无损将历史列 {}.{} 重命名为 {}",
        table, legacy_col, canonical_col
    );
    Ok(true)
}

/// 检查并在必要时补充指定表缺失的非主键列（幂等）
fn ensure_columns_present(
    conn: &Connection,
    table: &str,
    required_cols: &[(&str, &str)],
) -> Result<usize> {
    let existing: HashSet<String> = list_table_columns(conn, table)?.into_iter().collect();
    if existing.is_empty() {
        return Ok(0);
    }
    let mut added = 0usize;
    for &(col_name, col_def) in required_cols {
        if existing.contains(col_name) {
            continue;
        }
        let ddl = format!("ALTER TABLE {} ADD COLUMN {} {}", table, col_name, col_def);
        conn.execute(&ddl, []).with_context(|| {
            format!("SQLite 增量补列失败 ({}.{})", table, col_name)
        })?;
        eprintln!(
            "[SQLite][MIGRATE] 已自动补齐历史库缺失列 {}.{}",
            table, col_name
        );
        added += 1;
    }
    Ok(added)
}

/// 执行 SQLite 历史库全量 Schema 幂等自愈与无损迁移：
/// 1. 在 `BEGIN IMMEDIATE` 写锁事务内执行检查与迁移，避免 Daemon/MCP/Web 多进程并发首次打开老库竞态；
/// 2. 自动识别并无损重命名旧版重命名前的历史列（如 benchmark_runs / effectiveness_ledger）；
/// 3. 自动补齐老库缺失的增量列并自动修复失步的外部内容型 FTS5 虚表；
/// 4. 写入迁移版本记录与当前版本号，保证可重入与可观测。
pub(super) fn reconcile_sqlite_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch("BEGIN IMMEDIATE;")
        .context("获取 SQLite Schema 迁移写事务锁失败")?;
    let res = reconcile_sqlite_schema_in_tx(conn);
    match res {
        Ok(()) => {
            conn.execute_batch("COMMIT;")
                .context("提交 SQLite Schema 迁移事务失败")?;
            Ok(())
        }
        Err(e) => {
            if let Err(rb_err) = conn.execute_batch("ROLLBACK;") {
                eprintln!("[SQLite][WARN] Schema 迁移事务回滚失败: {}", rb_err);
            }
            Err(e)
        }
    }
}

fn reconcile_sqlite_schema_in_tx(conn: &Connection) -> Result<()> {
    let mut migrated_any = false;

    // 0. 历史 assets 表先补 content_text 列：否则 schema.sql 已建的触发器引用 NEW.content_text，
    //    会令后续 RENAME COLUMN 校验触发器失败。
    let asset_cols = list_table_columns(conn, "assets")?;
    if !asset_cols.is_empty() && !asset_cols.iter().any(|c| c == "content_text") {
        conn.execute(
            "ALTER TABLE assets ADD COLUMN content_text TEXT NOT NULL DEFAULT ''",
            [],
        )
        .context("历史 assets 表补齐 content_text 列失败")?;
        migrated_any = true;
    }

    // 1. benchmark_runs: 无损将旧版 total_*_tokens（排除 baseline）重命名为 total_body_tokens
    if rename_legacy_column_if_needed(conn, "benchmark_runs", "total_body_tokens", |col| {
        col.starts_with("total_")
            && col.ends_with("_tokens")
            && col != "total_baseline_tokens"
            && col != "total_body_tokens"
    })? {
        migrated_any = true;
    }

    // 2. effectiveness_ledger: 无损将旧版 *_tokens / *_rounds 重命名为 body_tokens / body_rounds
    if rename_legacy_column_if_needed(conn, "effectiveness_ledger", "body_tokens", |col| {
        col.ends_with("_tokens") && col != "baseline_tokens" && col != "body_tokens"
    })? {
        migrated_any = true;
    }
    if rename_legacy_column_if_needed(conn, "effectiveness_ledger", "body_rounds", |col| {
        col.ends_with("_rounds") && col != "baseline_rounds" && col != "body_rounds"
    })? {
        migrated_any = true;
    }

    // 3. 对历史库执行关键表的全列完整性自检与缺列自动补齐
    let column_specs: &[(&str, &[(&str, &str)])] = &[
        (
            "memories",
            &[
                ("sensitive", "INTEGER DEFAULT 0"),
                ("embedding", "TEXT DEFAULT '[]'"),
                ("last_accessed_at", "TEXT"),
                ("access_count", "INTEGER DEFAULT 0"),
            ],
        ),
        (
            "assets",
            &[
                ("embedding", "TEXT DEFAULT '[]'"),
                ("content_text", "TEXT NOT NULL DEFAULT ''"),
                ("last_scanned_at", "TEXT"),
                ("last_modified_at", "TEXT"),
            ],
        ),
        ("fact_nodes", &[("embedding", "TEXT DEFAULT '[]'")]),
        (
            "links",
            &[
                ("line_number", "INTEGER DEFAULT NULL"),
                ("snippet", "TEXT DEFAULT NULL"),
                ("provenance", "TEXT DEFAULT 'static_scan'"),
            ],
        ),
        (
            "benchmark_runs",
            &[
                ("total_body_tokens", "INTEGER NOT NULL DEFAULT 0"),
                ("total_hazards_intercepted", "INTEGER NOT NULL DEFAULT 0"),
                ("total_noise_blocked", "INTEGER NOT NULL DEFAULT 0"),
                ("sla_passed", "INTEGER NOT NULL DEFAULT 0"),
                ("results_json", "TEXT NOT NULL DEFAULT '[]'"),
                ("report_sha256", "TEXT NOT NULL DEFAULT ''"),
                ("duration_ms", "INTEGER NOT NULL DEFAULT 0"),
            ],
        ),
        (
            "effectiveness_ledger",
            &[
                ("session_id", "TEXT NOT NULL DEFAULT 'default'"),
                ("body_tokens", "INTEGER NOT NULL DEFAULT 0"),
                ("body_rounds", "INTEGER NOT NULL DEFAULT 1"),
                ("hazard_intercepted", "INTEGER NOT NULL DEFAULT 0"),
                ("noise_blocked", "INTEGER NOT NULL DEFAULT 0"),
                ("elapsed_us", "INTEGER NOT NULL DEFAULT 0"),
            ],
        ),
        (
            "audit_events",
            &[
                ("concurrency_mode", "TEXT DEFAULT NULL"),
                ("before_after_delta", "TEXT NOT NULL DEFAULT '{}'"),
                ("elapsed_us", "INTEGER NOT NULL DEFAULT 0"),
            ],
        ),
        (
            "body_checkpoints",
            &[
                ("resumed_by_brain", "TEXT DEFAULT NULL"),
                ("resumed_at", "TEXT DEFAULT NULL"),
                ("tx_epoch", "INTEGER NOT NULL DEFAULT 0"),
                ("fast_weight_snapshot_id", "TEXT DEFAULT NULL"),
                ("fast_weight_base_model", "TEXT DEFAULT NULL"),
            ],
        ),
    ];

    for &(table, cols) in column_specs {
        if ensure_columns_present(conn, table, cols)? > 0 {
            migrated_any = true;
        }
    }

    // 3.5. 升级旧版 5 列 assets_fts 至 6 列（含 content_text），并确保 assets_trigram_fts 存在
    let assets_fts_cols = list_table_columns(conn, "assets_fts")?;
    if !assets_fts_cols.is_empty() && !assets_fts_cols.iter().any(|c| c == "content_text") {
        conn.execute_batch(
            "DROP TRIGGER IF EXISTS assets_fts_insert;
             DROP TRIGGER IF EXISTS assets_fts_delete;
             DROP TRIGGER IF EXISTS assets_fts_update;
             DROP TABLE IF EXISTS assets_fts;
             CREATE VIRTUAL TABLE assets_fts USING fts5(
                 path,
                 description,
                 desc_kv,
                 usage,
                 tags,
                 content_text,
                 content=assets,
                 content_rowid=rowid,
                 tokenize='unicode61 remove_diacritics 2'
             );
             CREATE TRIGGER assets_fts_insert AFTER INSERT ON assets BEGIN
                 INSERT INTO assets_fts(rowid, path, description, desc_kv, usage, tags, content_text)
                 VALUES (NEW.rowid, NEW.path, NEW.description, NEW.desc_kv, NEW.usage, NEW.tags, NEW.content_text);
             END;
             CREATE TRIGGER assets_fts_delete AFTER DELETE ON assets BEGIN
                 INSERT INTO assets_fts(assets_fts, rowid, path, description, desc_kv, usage, tags, content_text)
                 VALUES ('delete', OLD.rowid, OLD.path, OLD.description, OLD.desc_kv, OLD.usage, OLD.tags, OLD.content_text);
             END;
             CREATE TRIGGER assets_fts_update AFTER UPDATE ON assets BEGIN
                 INSERT INTO assets_fts(assets_fts, rowid, path, description, desc_kv, usage, tags, content_text)
                 VALUES ('delete', OLD.rowid, OLD.path, OLD.description, OLD.desc_kv, OLD.usage, OLD.tags, OLD.content_text);
                 INSERT INTO assets_fts(rowid, path, description, desc_kv, usage, tags, content_text)
                 VALUES (NEW.rowid, NEW.path, NEW.description, NEW.desc_kv, NEW.usage, NEW.tags, NEW.content_text);
             END;
             INSERT INTO assets_fts(assets_fts) VALUES ('rebuild');",
        )
        .context("升级历史 assets_fts 虚表至 6 列（含 content_text）失败")?;
        eprintln!("[SQLite][MIGRATE] 已无损升级 assets_fts 至 6 列全文索引 (含 content_text)");
        migrated_any = true;
    }

    let trigram_exists: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='assets_trigram_fts'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    if !trigram_exists {
        conn.execute_batch(
            "CREATE VIRTUAL TABLE IF NOT EXISTS assets_trigram_fts USING fts5(
                 path,
                 description,
                 desc_kv,
                 tags,
                 content_text,
                 content=assets,
                 content_rowid=rowid,
                 tokenize='trigram'
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
        )
        .context("创建并重建历史库 assets_trigram_fts 虚表失败")?;
        migrated_any = true;
    }

    // 4. 校验并自动修复外部内容型 FTS5 虚表与基表行计数失步（防历史库新建 FTS5 后 UPDATE 触发 SQLITE_CORRUPT_VTAB 267）
    let mut fts_pairs: Vec<(&str, &str)> = vec![
        ("memories", "memories_fts"),
        ("assets", "assets_fts"),
        ("skills_catalog", "skills_catalog_fts"),
        ("external_services", "external_services_fts"),
    ];
    let has_trigram_now: bool = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='assets_trigram_fts'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap_or(0)
        > 0;
    if has_trigram_now {
        fts_pairs.push(("assets", "assets_trigram_fts"));
    }
    for (base_table, fts_table) in fts_pairs {
        let base_cnt: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {}", base_table), [], |r| r.get(0))
            .unwrap_or(0);
        let docsize_table = format!("{}_docsize", fts_table);
        let fts_cnt: i64 = conn
            .query_row(&format!("SELECT COUNT(*) FROM {}", docsize_table), [], |r| r.get(0))
            .unwrap_or(-1);
        if base_cnt != fts_cnt && fts_cnt >= 0 {
            let rebuild_sql = format!("INSERT INTO {}({}) VALUES ('rebuild')", fts_table, fts_table);
            conn.execute(&rebuild_sql, []).with_context(|| {
                format!(
                    "重建失步的 FTS5 虚表 {} 失败 (base={}, fts={})",
                    fts_table, base_cnt, fts_cnt
                )
            })?;
            eprintln!(
                "[SQLite][MIGRATE] 已自动重建失步 FTS5 索引 {} (base_rows={}, old_fts_rows={})",
                fts_table, base_cnt, fts_cnt
            );
            migrated_any = true;
        }
    }

    // 5. 登记迁移版本并刷新当前运行版本元数据
    conn.execute(
        "INSERT OR IGNORE INTO migrations(version, description) VALUES (?1, ?2)",
        params![
            2i64,
            "schema reconciler v2: canonical body column rename and missing-column self-heal"
        ],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO migrations(version, description) VALUES (?1, ?2)",
        params![
            3i64,
            "schema reconciler v3: assets.content_text + 6-col assets_fts + optional assets_trigram_fts"
        ],
    )?;
    conn.execute(
        "INSERT INTO config(key, value, updated_at)
         VALUES ('body_version', ?1, strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
         ON CONFLICT(key) DO UPDATE SET
           value = excluded.value,
           updated_at = excluded.updated_at",
        params![env!("CARGO_PKG_VERSION")],
    )?;

    if migrated_any {
        eprintln!("[SQLite][MIGRATE] 历史数据库 Schema 自动自愈完成，所有历史记录已无损保留");
    }
    Ok(())
}
