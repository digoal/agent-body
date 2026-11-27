//! 扩展私有物理表沙盒 SQL 解析与前缀隔离执行器 (`ScopedSqlExecutor`)

use anyhow::bail;
use rusqlite::Connection;

/// 扩展私有 SQL 沙盒校验与执行器
///
/// 强制拦截任何未以 `table_prefix` (`ext_<ext_id>__`) 开头的
/// `CREATE TABLE` / `CREATE INDEX` / `INSERT` / `UPDATE` / `DELETE` / `DROP` / `ALTER` 语句，
/// 防止扩展跨命名空间读写其他扩展或内核系统表。
pub struct ScopedSqlExecutor;

impl ScopedSqlExecutor {
    /// 校验 `ext_id` 与 `table_prefix` 一致性，并严格解析 SQL 脚本中每条语句
    pub fn validate_scoped_sql(ext_id: &str, table_prefix: &str, sql: &str) -> anyhow::Result<Vec<String>> {
        validate_prefix_contract(ext_id, table_prefix)?;
        let statements = split_sql_statements(sql)?;
        if statements.is_empty() {
            bail!("扩展 '{ext_id}' 迁移 SQL 不能为空");
        }
        for stmt in &statements {
            validate_statement_target(ext_id, table_prefix, stmt)?;
        }
        Ok(statements)
    }

    /// 在 SQLite 连接事务内执行已通过前缀隔离校验的迁移语句
    pub fn execute_on_sqlite(
        conn: &Connection,
        ext_id: &str,
        table_prefix: &str,
        sql: &str,
    ) -> anyhow::Result<usize> {
        let statements = Self::validate_scoped_sql(ext_id, table_prefix, sql)?;
        let tx = conn.unchecked_transaction()?;
        for stmt in &statements {
            tx.execute_batch(stmt)?;
        }
        tx.commit()?;
        Ok(statements.len())
    }
}

fn validate_prefix_contract(ext_id: &str, table_prefix: &str) -> anyhow::Result<()> {
    let trimmed_id = ext_id.trim();
    if trimmed_id.is_empty()
        || !trimmed_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
    {
        bail!("非法扩展 ID '{ext_id}'");
    }
    let expected = format!("ext_{trimmed_id}__");
    if table_prefix != expected {
        bail!("扩展 '{ext_id}' 的 table_prefix '{table_prefix}' 必须严格等于 '{expected}'");
    }
    Ok(())
}

/// 剥离 SQL 注释并按语句切分（保留单引号字符串字面量内部的分号与字符）
fn split_sql_statements(sql: &str) -> anyhow::Result<Vec<String>> {
    let mut cleaned = String::with_capacity(sql.len());
    let bytes = sql.as_bytes();
    let mut i = 0usize;
    let mut in_single_quote = false;
    let mut in_double_quote = false;

    while i < bytes.len() {
        let ch = bytes[i] as char;
        if in_single_quote {
            cleaned.push(ch);
            if ch == '\'' {
                if i + 1 < bytes.len() && bytes[i + 1] == b'\'' {
                    cleaned.push('\'');
                    i += 2;
                    continue;
                }
                in_single_quote = false;
            }
            i += 1;
            continue;
        }
        if in_double_quote {
            cleaned.push(ch);
            if ch == '"' {
                in_double_quote = false;
            }
            i += 1;
            continue;
        }
        if ch == '\'' {
            in_single_quote = true;
            cleaned.push(ch);
            i += 1;
            continue;
        }
        if ch == '"' {
            in_double_quote = true;
            cleaned.push(ch);
            i += 1;
            continue;
        }
        if ch == '-' && i + 1 < bytes.len() && bytes[i + 1] == b'-' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if ch == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            let mut closed = false;
            while i + 1 < bytes.len() {
                if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                    i += 2;
                    closed = true;
                    break;
                }
                i += 1;
            }
            if !closed {
                bail!("SQL 存在未闭合块注释");
            }
            cleaned.push(' ');
            continue;
        }
        cleaned.push(ch);
        i += 1;
    }

    if in_single_quote || in_double_quote {
        bail!("SQL 存在未闭合引号");
    }

    let mut out = Vec::new();
    let mut current = String::new();
    let mut in_sq = false;
    let mut in_dq = false;
    let c_bytes = cleaned.as_bytes();
    let mut idx = 0usize;
    while idx < c_bytes.len() {
        let ch = c_bytes[idx] as char;
        if ch == '\'' && !in_dq {
            in_sq = !in_sq;
            current.push(ch);
        } else if ch == '"' && !in_sq {
            in_dq = !in_dq;
            current.push(ch);
        } else if ch == ';' && !in_sq && !in_dq {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                out.push(trimmed.to_string());
            }
            current.clear();
        } else {
            current.push(ch);
        }
        idx += 1;
    }
    let tail = current.trim();
    if !tail.is_empty() {
        out.push(tail.to_string());
    }
    Ok(out)
}

fn tokenize_head(stmt: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut buf = String::new();
    let mut chars = stmt.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch.is_ascii_whitespace() {
            if !buf.is_empty() {
                tokens.push(std::mem::take(&mut buf));
                if tokens.len() >= 12 {
                    break;
                }
            }
        } else if matches!(ch, '(' | ')' | ',' | ';') {
            if !buf.is_empty() {
                tokens.push(std::mem::take(&mut buf));
                if tokens.len() >= 12 {
                    break;
                }
            }
            tokens.push(ch.to_string());
            if tokens.len() >= 12 {
                break;
            }
        } else if ch == '"' {
            // 保留双引号标识符并在末尾截断
            for inner in chars.by_ref() {
                if inner == '"' {
                    break;
                }
                buf.push(inner);
            }
            if !buf.is_empty() {
                tokens.push(std::mem::take(&mut buf));
                if tokens.len() >= 12 {
                    break;
                }
            }
        } else {
            buf.push(ch);
        }
    }
    if !buf.is_empty() && tokens.len() < 12 {
        tokens.push(buf);
    }
    tokens
}

fn check_prefixed_ident(ext_id: &str, prefix: &str, ident: &str, stmt: &str) -> anyhow::Result<()> {
    if ident.contains('.') {
        bail!("扩展 '{ext_id}' SQL 禁止跨 schema/数据库限定表名 '{ident}': {stmt}");
    }
    if !ident.starts_with(prefix) || ident.len() <= prefix.len() {
        bail!(
            "扩展 '{ext_id}' SQL 目标对象 '{ident}' 必须严格以 '{prefix}' 开头: {stmt}"
        );
    }
    let suffix = &ident[prefix.len()..];
    if !suffix
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        bail!("扩展 '{ext_id}' SQL 目标对象 '{ident}' 包含非法字符");
    }
    Ok(())
}

fn validate_statement_target(ext_id: &str, prefix: &str, stmt: &str) -> anyhow::Result<()> {
    let tokens = tokenize_head(stmt);
    if tokens.is_empty() {
        return Ok(());
    }
    let upper: Vec<String> = tokens.iter().map(|t| t.to_ascii_uppercase()).collect();
    let kw = |idx: usize| upper.get(idx).map(String::as_str).unwrap_or("");
    let raw = |idx: usize| tokens.get(idx).map(String::as_str).unwrap_or("");

    match kw(0) {
        "CREATE" => validate_create(ext_id, prefix, stmt, &upper, &tokens),
        "INSERT" => {
            let mut pos = 1usize;
            if kw(pos) == "OR" {
                pos += 2;
            }
            if kw(pos) != "INTO" {
                bail!("扩展 '{ext_id}' INSERT 语句语法无效: {stmt}");
            }
            check_prefixed_ident(ext_id, prefix, raw(pos + 1), stmt)
        }
        "UPDATE" => {
            let mut pos = 1usize;
            if kw(pos) == "OR" {
                pos += 2;
            }
            check_prefixed_ident(ext_id, prefix, raw(pos), stmt)
        }
        "DELETE" => {
            if kw(1) != "FROM" {
                bail!("扩展 '{ext_id}' DELETE 语句必须显式包含 FROM: {stmt}");
            }
            check_prefixed_ident(ext_id, prefix, raw(2), stmt)
        }
        "DROP" => {
            let obj = kw(1);
            if obj != "TABLE" && obj != "INDEX" {
                bail!("扩展 '{ext_id}' 仅允许 DROP TABLE 或 DROP INDEX: {stmt}");
            }
            let pos = if kw(2) == "IF" && kw(3) == "EXISTS" {
                4
            } else {
                2
            };
            check_prefixed_ident(ext_id, prefix, raw(pos), stmt)
        }
        "ALTER" => {
            if kw(1) != "TABLE" {
                bail!("扩展 '{ext_id}' 仅允许 ALTER TABLE: {stmt}");
            }
            check_prefixed_ident(ext_id, prefix, raw(2), stmt)
        }
        other => bail!("扩展 '{ext_id}' 迁移沙盒禁止执行 '{other}' 语句: {stmt}"),
    }
}

fn validate_create(
    ext_id: &str,
    prefix: &str,
    stmt: &str,
    upper: &[String],
    tokens: &[String],
) -> anyhow::Result<()> {
    let kw = |idx: usize| upper.get(idx).map(String::as_str).unwrap_or("");
    let raw = |idx: usize| tokens.get(idx).map(String::as_str).unwrap_or("");

    let mut pos = 1usize;
    let is_unique = if kw(pos) == "UNIQUE" {
        pos += 1;
        true
    } else {
        false
    };

    match kw(pos) {
        "TABLE" if !is_unique => {
            pos += 1;
            if kw(pos) == "IF" && kw(pos + 1) == "NOT" && kw(pos + 2) == "EXISTS" {
                pos += 3;
            }
            check_prefixed_ident(ext_id, prefix, raw(pos), stmt)
        }
        "INDEX" => {
            pos += 1;
            if kw(pos) == "IF" && kw(pos + 1) == "NOT" && kw(pos + 2) == "EXISTS" {
                pos += 3;
            }
            let idx_name = raw(pos);
            check_prefixed_ident(ext_id, prefix, idx_name, stmt)?;
            if kw(pos + 1) != "ON" {
                bail!("扩展 '{ext_id}' CREATE INDEX 缺少 ON 子句: {stmt}");
            }
            check_prefixed_ident(ext_id, prefix, raw(pos + 2), stmt)
        }
        other => bail!("扩展 '{ext_id}' 迁移沙盒禁止 CREATE {other}: {stmt}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_sql_allows_valid_prefixed_ddl_and_dml() {
        let sql = r#"
            -- 私有表初始化
            CREATE TABLE IF NOT EXISTS ext_demo_a__items (
                id TEXT PRIMARY KEY,
                note TEXT NOT NULL DEFAULT 'a;b'
            );
            CREATE INDEX IF NOT EXISTS ext_demo_a__idx_note ON ext_demo_a__items(note);
            INSERT INTO ext_demo_a__items (id, note) VALUES ('1', 'ok');
            UPDATE ext_demo_a__items SET note = 'updated' WHERE id = '1';
            DELETE FROM ext_demo_a__items WHERE id = '2';
            ALTER TABLE ext_demo_a__items ADD COLUMN tag TEXT;
            DROP INDEX IF EXISTS ext_demo_a__idx_note;
        "#;
        let stmts = ScopedSqlExecutor::validate_scoped_sql("demo_a", "ext_demo_a__", sql).unwrap();
        assert_eq!(stmts.len(), 7);
    }

    #[test]
    fn scoped_sql_blocks_cross_extension_and_system_tables() {
        let bad_cases = [
            "CREATE TABLE body_cat_extension (id TEXT);",
            "CREATE TABLE ext_other__secret (id TEXT);",
            "CREATE INDEX ext_demo_a__idx ON body_cat_proc(proc_name);",
            "CREATE INDEX bad_idx ON ext_demo_a__items(id);",
            "INSERT INTO ext_other__items (id) VALUES ('x');",
            "UPDATE kernel_entities SET entity_id = 'x';",
            "DELETE FROM kernel_leases;",
            "DROP TABLE body_cat_hook;",
            "ALTER TABLE main.ext_demo_a__items ADD COLUMN x TEXT;",
            "PRAGMA writable_schema = ON;",
            "ATTACH DATABASE '/tmp/evil.db' AS evil;",
        ];
        for sql in bad_cases {
            let res = ScopedSqlExecutor::validate_scoped_sql("demo_a", "ext_demo_a__", sql);
            assert!(res.is_err(), "expected rejection for SQL: {sql}");
        }
    }
}
