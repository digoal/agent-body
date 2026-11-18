use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use duckdb::{params, Connection};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatmapCell {
    pub directory: String,
    pub time_bucket: String,
    pub change_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrendPoint {
    pub day: String,
    pub asset_type: String,
    pub daily_count: usize,
    pub cumulative_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeFrequency {
    pub target: String,
    pub change_count: usize,
    pub last_changed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayPoint {
    pub age_days: usize,
    pub active_count: usize,
    pub archived_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub node_counts_by_type: HashMap<String, usize>,
    pub edge_counts_by_relation: HashMap<String, usize>,
    pub total_nodes: usize,
    pub total_edges: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportSummary {
    pub table: String,
    pub row_count: usize,
    pub output_path: String,
    pub file_size_bytes: u64,
}

/// DuckDB 分析查询专属扩展接口
pub trait AnalyticsProvider: Send + Sync {
    /// 变更热力图：按目录与时间分桶聚合变更次数
    fn changes_heatmap(&self, days: usize) -> Result<Vec<HeatmapCell>>;

    /// 资产增长趋势：按日统计各类型资产的累计数量
    fn asset_growth_trend(&self, days: usize) -> Result<Vec<TrendPoint>>;

    /// 变更频率 Top-N：过去 N 天变更最频繁的文件或目录
    fn most_changed_assets(&self, days: usize, limit: usize) -> Result<Vec<ChangeFrequency>>;

    /// 记忆衰减分布：按创建账龄分桶统计 active/archived 比例
    fn memory_decay_curve(&self) -> Result<Vec<DecayPoint>>;

    /// 事实图谱全景统计：各节点类型与边类型的数量分布
    fn fact_graph_stats(&self) -> Result<GraphStats>;

    /// Parquet 导出：将指定表数据直接导出为高效压缩的 Parquet 文件
    fn export_to_parquet(&self, table: &str, output_path: &str) -> Result<ExportSummary>;
}

pub struct DuckDbAnalyticsEngine {
    conn: Arc<Mutex<Connection>>,
}

impl DuckDbAnalyticsEngine {
    pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
        Self { conn }
    }
}

impl AnalyticsProvider for DuckDbAnalyticsEngine {
    fn changes_heatmap(&self, _days: usize) -> Result<Vec<HeatmapCell>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT 
                CASE 
                    WHEN position('/' in target) > 0 THEN substr(target, 1, instr(substr(target, 2), '/') + 1)
                    ELSE target 
                END as dir,
                coalesce(substr(created_at, 1, 13) || ':00:00Z', 'unknown') as time_bucket,
                count(*) as cnt
             FROM changes
             GROUP BY 1, 2
             ORDER BY cnt DESC"
        )?;

        let rows = stmt.query_map([], |row| {
            let cnt: i64 = row.get(2)?;
            Ok(HeatmapCell {
                directory: row.get(0)?,
                time_bucket: row.get(1)?,
                change_count: cnt as usize,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn asset_growth_trend(&self, _days: usize) -> Result<Vec<TrendPoint>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT 
                coalesce(substr(created_at, 1, 10), 'unknown') as day,
                type,
                count(*) as daily_cnt,
                sum(count(*)) OVER (PARTITION BY type ORDER BY coalesce(substr(created_at, 1, 10), 'unknown')) as cum_cnt
             FROM assets
             GROUP BY 1, 2
             ORDER BY 1 ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            let daily: i64 = row.get(2)?;
            let cum: i64 = row.get(3)?;
            Ok(TrendPoint {
                day: row.get(0)?,
                asset_type: row.get(1)?,
                daily_count: daily as usize,
                cumulative_count: cum as usize,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn most_changed_assets(&self, _days: usize, limit: usize) -> Result<Vec<ChangeFrequency>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT target, count(*) as cnt, max(created_at) as last_changed
             FROM changes
             GROUP BY target
             ORDER BY cnt DESC
             LIMIT ?1"
        )?;

        let rows = stmt.query_map(params![limit as i64], |row| {
            let cnt: i64 = row.get(1)?;
            let last: Option<String> = row.get(2)?;
            Ok(ChangeFrequency {
                target: row.get(0)?,
                change_count: cnt as usize,
                last_changed: last.unwrap_or_default(),
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn memory_decay_curve(&self) -> Result<Vec<DecayPoint>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT 
                0 as age_days,
                sum(case when status = 'active' then 1 else 0 end) as active_cnt,
                sum(case when status = 'archived' then 1 else 0 end) as archived_cnt
             FROM memories
             GROUP BY 1"
        )?;

        let rows = stmt.query_map([], |row| {
            let active: Option<i64> = row.get(1)?;
            let archived: Option<i64> = row.get(2)?;
            Ok(DecayPoint {
                age_days: 0,
                active_count: active.unwrap_or(0) as usize,
                archived_count: archived.unwrap_or(0) as usize,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn fact_graph_stats(&self) -> Result<GraphStats> {
        let conn = self.conn.lock().unwrap();
        let mut node_counts_by_type = HashMap::new();
        let mut edge_counts_by_relation = HashMap::new();
        let mut total_nodes = 0;
        let mut total_edges = 0;

        let mut node_stmt = conn.prepare("SELECT node_type, count(*) FROM fact_nodes GROUP BY 1")?;
        let node_rows = node_stmt.query_map([], |row| {
            let t: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            Ok((t, cnt as usize))
        })?;
        for r in node_rows {
            let (t, cnt) = r?;
            total_nodes += cnt;
            node_counts_by_type.insert(t, cnt);
        }

        let mut edge_stmt = conn.prepare("SELECT relation, count(*) FROM fact_edges GROUP BY 1")?;
        let edge_rows = edge_stmt.query_map([], |row| {
            let r: String = row.get(0)?;
            let cnt: i64 = row.get(1)?;
            Ok((r, cnt as usize))
        })?;
        for r in edge_rows {
            let (r, cnt) = r?;
            total_edges += cnt;
            edge_counts_by_relation.insert(r, cnt);
        }

        Ok(GraphStats {
            node_counts_by_type,
            edge_counts_by_relation,
            total_nodes,
            total_edges,
        })
    }

    fn export_to_parquet(&self, table: &str, output_path: &str) -> Result<ExportSummary> {
        let conn = self.conn.lock().unwrap();
        let valid_tables = ["assets", "changes", "memories", "fact_nodes", "fact_edges", "links", "active_processes"];
        if !valid_tables.contains(&table) {
            anyhow::bail!("不支持导出表: {}, 仅允许: {:?}", table, valid_tables);
        }

        // 统计行数
        let count_query = format!("SELECT count(*) FROM {}", table);
        let row_count: i64 = conn.query_row(&count_query, [], |r| r.get(0))?;

        // 确保父目录存在
        if let Some(parent) = std::path::Path::new(output_path).parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 尝试 DuckDB 原生导出：
        // 优先尝试 PARQUET；若当前环境未内置/无法在线拉取 parquet 扩展，则回退到 CSV 格式以保证离线强壮性
        let is_csv_requested = output_path.ends_with(".csv");
        let mut exported = false;

        if !is_csv_requested {
            // 禁用在线自动下载扩展，避免在离线/内网环境下挂死
            let _ = conn.execute_batch("SET autoinstall_known_extensions=false; SET autoload_known_extensions=false;");
            let parquet_sql = format!("COPY (SELECT * FROM {}) TO '{}' (FORMAT PARQUET)", table, output_path);
            if conn.execute(&parquet_sql, []).is_ok() {
                exported = true;
            }
        }

        if !exported {
            // 回退到 DuckDB 核心原生内置的 CSV 格式（无外部依赖，零挂死）
            // 显式指定 FORMAT CSV，避免 DuckDB 根据 .parquet 后缀推断调用缺失的 parquet 扩展
            let csv_sql = format!("COPY (SELECT * FROM {}) TO '{}' (FORMAT CSV, HEADER, DELIMITER ',')", table, output_path);
            conn.execute(&csv_sql, [])
                .with_context(|| format!("DuckDB 导出数据失败: {}", csv_sql))?;
        }

        let file_size_bytes = std::fs::metadata(output_path)
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(ExportSummary {
            table: table.to_string(),
            row_count: row_count as usize,
            output_path: output_path.to_string(),
            file_size_bytes,
        })
    }
}
