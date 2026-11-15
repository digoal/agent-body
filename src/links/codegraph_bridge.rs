use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use anyhow::{Result, Context};
use rusqlite::{Connection, OpenFlags, params};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGraphCaller {
    pub caller_name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub line: Option<i32>,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeGraphProjectInfo {
    pub root_dir: String,
    pub db_path: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub file_count: usize,
}

pub struct CodeGraphBridge;

impl CodeGraphBridge {
    /// 从指定路径开始逐级向上查找最近的 .codegraph/codegraph.db
    pub fn find_codegraph_db(start_path: &Path) -> Option<PathBuf> {
        let mut curr = if start_path.is_file() {
            start_path.parent()
        } else {
            Some(start_path)
        };

        while let Some(dir) = curr {
            let candidate = dir.join(".codegraph").join("codegraph.db");
            if candidate.is_file() {
                return Some(candidate);
            }
            curr = dir.parent();
        }
        None
    }

    /// 获取项目的 CodeGraph 概览统计信息
    pub fn inspect_project(target_path: &Path) -> Result<Option<CodeGraphProjectInfo>> {
        let db_path = match Self::find_codegraph_db(target_path) {
            Some(p) => p,
            None => return Ok(None),
        };

        let root_dir = db_path
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();

        let conn = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ).with_context(|| format!("打开 CodeGraph SQLite 失败: {:?}", db_path))?;

        let node_count: usize = conn.query_row("SELECT COUNT(*) FROM nodes", [], |r| r.get(0)).unwrap_or(0);
        let edge_count: usize = conn.query_row("SELECT COUNT(*) FROM edges", [], |r| r.get(0)).unwrap_or(0);
        let file_count: usize = conn.query_row("SELECT COUNT(*) FROM files", [], |r| r.get(0)).unwrap_or(0);

        Ok(Some(CodeGraphProjectInfo {
            root_dir,
            db_path: db_path.to_string_lossy().to_string(),
            node_count,
            edge_count,
            file_count,
        }))
    }

    /// 跨层查询目标工程内指定符号（函数、类、方法）的代码级调用方（Callers）
    pub fn query_symbol_callers(target_path: &Path, symbol: &str) -> Result<Vec<CodeGraphCaller>> {
        let db_path = match Self::find_codegraph_db(target_path) {
            Some(p) => p,
            None => return Ok(Vec::new()),
        };

        let conn = Connection::open_with_flags(
            &db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        ).with_context(|| format!("打开 CodeGraph SQLite 失败: {:?}", db_path))?;

        let mut stmt = conn.prepare(
            "SELECT s.name, s.qualified_name, s.file_path, e.line, e.kind
             FROM edges e
             JOIN nodes t ON e.target = t.id
             JOIN nodes s ON e.source = s.id
             WHERE t.name = ?1 OR t.qualified_name = ?1 OR t.name LIKE ?2
             ORDER BY e.line ASC
             LIMIT 50"
        )?;

        let pattern = format!("%{}%", symbol);
        let rows = stmt.query_map(params![symbol, pattern], |row| {
            Ok(CodeGraphCaller {
                caller_name: row.get(0)?,
                qualified_name: row.get(1)?,
                file_path: row.get(2)?,
                line: row.get(3)?,
                kind: row.get(4)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_codegraph_detection_on_known_repo() {
        // /root/new/src/hk2 已存在 .codegraph
        let hk2_path = Path::new("/root/new/src/hk2");
        if hk2_path.exists() {
            let found = CodeGraphBridge::find_codegraph_db(hk2_path);
            assert!(found.is_some(), "应当发现 hk2 的 .codegraph/codegraph.db");

            let info = CodeGraphBridge::inspect_project(hk2_path).unwrap();
            assert!(info.is_some());
            let stats = info.unwrap();
            assert!(stats.node_count > 0, "节点数应当大于 0");
        }
    }
}
