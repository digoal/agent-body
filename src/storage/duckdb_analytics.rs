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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditActionStat {
    pub domain: String,
    pub category: String,
    pub action: String,
    pub outcome: String,
    pub hit_count: usize,
    pub avg_elapsed_us: f64,
    #[serde(default)]
    pub p50_elapsed_us: f64,
    #[serde(default)]
    pub p95_elapsed_us: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditAnalyticsReport {
    pub days: usize,
    pub total_events: usize,
    pub in_agent_count: usize,
    pub out_of_agent_count: usize,
    pub failed_or_blocked_count: usize,
    pub recovered_count: usize,
    pub top_actions: Vec<AuditActionStat>,
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

    /// 双域行为审计 OLAP 多维聚合 (Phase 43)
    fn audit_analytics_summary(&self, days: usize) -> Result<AuditAnalyticsReport>;

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

    /// 创建或复用短 TTL (15s) 内存 DuckDB 分析快照（单飞串行防并发雪崩）
    pub fn from_synced_storage(source: Arc<dyn crate::storage::StorageProvider>) -> Result<Self> {
        use std::sync::OnceLock;
        use std::time::{Duration, Instant};

        struct CachedDuckDb {
            source_ptr: usize,
            latest_asset: String,
            latest_change: Option<i64>,
            latest_memory: (String, String),
            created_at: Instant,
            conn: Arc<Mutex<Connection>>,
        }

        static CACHE: OnceLock<Mutex<Option<CachedDuckDb>>> = OnceLock::new();
        let cache_mutex = CACHE.get_or_init(|| Mutex::new(None));
        let mut guard = cache_mutex
            .lock()
            .map_err(|e| anyhow::anyhow!("DuckDB 缓存锁异常: {}", e))?;

        let source_ptr = Arc::as_ptr(&source) as *const () as usize;
        let latest_asset = source
            .list_assets(1)
            .ok()
            .and_then(|v| v.into_iter().next().map(|a| a.path))
            .unwrap_or_default();
        let latest_change = source
            .list_changes(1)
            .ok()
            .and_then(|v| v.into_iter().next().map(|c| c.id))
            .unwrap_or_default();
        let latest_memory = source
            .list_all_memories(1)
            .ok()
            .and_then(|v| v.into_iter().next().map(|m| (m.id, m.status)))
            .unwrap_or_default();
        if let Some(ref cached) = *guard {
            if cached.source_ptr == source_ptr
                && cached.latest_asset == latest_asset
                && cached.latest_change == latest_change
                && cached.latest_memory == latest_memory
                && cached.created_at.elapsed() < Duration::from_secs(15)
            {
                return Ok(Self::new(cached.conn.clone()));
            }
        }

        let duckdb_prov = crate::storage::DuckDbStorageProvider::open_in_memory()?;
        let pipeline = crate::storage::SyncPipeline::new(source, Arc::new(duckdb_prov.clone()));
        pipeline.sync_all()?;
        let conn = duckdb_prov.conn();
        *guard = Some(CachedDuckDb {
            source_ptr,
            latest_asset,
            latest_change,
            latest_memory,
            created_at: Instant::now(),
            conn: conn.clone(),
        });
        Ok(Self::new(conn))
    }

    /// 轻量仅同步 audit_events 表（毫秒级，避免 audit 接口全表拷贝 2 万+ 资产）
    pub fn from_synced_audit_events(source: Arc<dyn crate::storage::StorageProvider>) -> Result<Self> {
        let duckdb_prov = crate::storage::DuckDbStorageProvider::open_in_memory()?;
        let pipeline = crate::storage::SyncPipeline::new(source, Arc::new(duckdb_prov.clone()));
        pipeline.sync_audit_events_only()?;
        Ok(Self::new(duckdb_prov.conn()))
    }

    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|e| anyhow::anyhow!("DuckDB 分析连接锁异常 (mutex poisoned): {}", e))
    }
}

impl AnalyticsProvider for DuckDbAnalyticsEngine {
    fn changes_heatmap(&self, days: usize) -> Result<Vec<HeatmapCell>> {
        let conn = self.lock_conn()?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days.max(1) as i64)).to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT
                CASE
                    WHEN position('/' in target) > 0 THEN substr(target, 1, instr(substr(target, 2), '/') + 1)
                    ELSE target
                END as dir,
                coalesce(substr(created_at, 1, 13) || ':00:00Z', 'unknown') as time_bucket,
                count(*) as cnt
             FROM changes
             WHERE COALESCE(try_cast(created_at AS TIMESTAMPTZ) >= try_cast(?1 AS TIMESTAMPTZ), created_at >= ?1)
             GROUP BY 1, 2
             ORDER BY cnt DESC"
        )?;

        let rows = stmt.query_map(params![cutoff], |row| {
            let cnt: i64 = row.get(2)?;
            Ok(HeatmapCell {
                directory: row.get(0)?,
                time_bucket: row.get(1)?,
                change_count: cnt as usize,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn asset_growth_trend(&self, days: usize) -> Result<Vec<TrendPoint>> {
        let conn = self.lock_conn()?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days.max(1) as i64)).to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT
                coalesce(substr(created_at, 1, 10), 'unknown') as day,
                type,
                count(*) as daily_cnt,
                sum(count(*)) OVER (PARTITION BY type ORDER BY coalesce(substr(created_at, 1, 10), 'unknown')) as cum_cnt
             FROM assets
             WHERE COALESCE(try_cast(created_at AS TIMESTAMPTZ) >= try_cast(?1 AS TIMESTAMPTZ), created_at >= ?1)
             GROUP BY 1, 2
             ORDER BY 1 ASC"
        )?;

        let rows = stmt.query_map(params![cutoff], |row| {
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

    fn most_changed_assets(&self, days: usize, limit: usize) -> Result<Vec<ChangeFrequency>> {
        let conn = self.lock_conn()?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days.max(1) as i64)).to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT target, count(*) as cnt, max(created_at) as last_changed
             FROM changes
             WHERE COALESCE(try_cast(created_at AS TIMESTAMPTZ) >= try_cast(?1 AS TIMESTAMPTZ), created_at >= ?1)
             GROUP BY target
             ORDER BY cnt DESC
             LIMIT ?2"
        )?;

        let rows = stmt.query_map(params![cutoff, limit as i64], |row| {
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
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT
                CAST(GREATEST(0, COALESCE(date_diff('day', try_cast(created_at AS TIMESTAMPTZ), current_timestamp), 0)) AS BIGINT) as age_days,
                sum(case when status = 'active' then 1 else 0 end) as active_cnt,
                sum(case when status != 'active' then 1 else 0 end) as archived_cnt
             FROM memories
             GROUP BY 1
             ORDER BY 1 ASC"
        )?;

        let rows = stmt.query_map([], |row| {
            let age_days: i64 = row.get(0)?;
            let active: Option<i64> = row.get(1)?;
            let archived: Option<i64> = row.get(2)?;
            Ok(DecayPoint {
                age_days: age_days.max(0) as usize,
                active_count: active.unwrap_or(0) as usize,
                archived_count: archived.unwrap_or(0) as usize,
            })
        })?.collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    fn fact_graph_stats(&self) -> Result<GraphStats> {
        let conn = self.lock_conn()?;
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

    fn audit_analytics_summary(&self, days: usize) -> Result<AuditAnalyticsReport> {
        let conn = self.lock_conn()?;
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(days.max(1) as i64)).to_rfc3339();
        let mut stmt = conn.prepare(
            "SELECT domain, category, action, outcome,
                    COUNT(*),
                    CAST(COALESCE(AVG(elapsed_us), 0.0) AS DOUBLE),
                    CAST(COALESCE(QUANTILE_CONT(elapsed_us, 0.50), 0.0) AS DOUBLE),
                    CAST(COALESCE(QUANTILE_CONT(elapsed_us, 0.95), 0.0) AS DOUBLE)
             FROM audit_events
             WHERE COALESCE(try_cast(created_at AS TIMESTAMPTZ) >= try_cast(?1 AS TIMESTAMPTZ), created_at >= ?1)
             GROUP BY domain, category, action, outcome
             ORDER BY COUNT(*) DESC
             LIMIT 30",
        )?;
        let top_actions = stmt
            .query_map(params![cutoff], |r| {
                Ok(AuditActionStat {
                    domain: r.get(0)?,
                    category: r.get(1)?,
                    action: r.get(2)?,
                    outcome: r.get(3)?,
                    hit_count: r.get::<_, i64>(4)? as usize,
                    avg_elapsed_us: r.get(5)?,
                    p50_elapsed_us: r.get(6)?,
                    p95_elapsed_us: r.get(7)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut report = AuditAnalyticsReport {
            days,
            ..Default::default()
        };
        let mut agg_stmt = conn.prepare(
            "SELECT domain, outcome, COUNT(*)
             FROM audit_events
             WHERE COALESCE(try_cast(created_at AS TIMESTAMPTZ) >= try_cast(?1 AS TIMESTAMPTZ), created_at >= ?1)
             GROUP BY domain, outcome",
        )?;
        let agg_rows = agg_stmt.query_map(params![cutoff], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)? as usize))
        })?;
        for row in agg_rows {
            let (domain, outcome, cnt) = row?;
            report.total_events += cnt;
            if domain == "IN_AGENT" {
                report.in_agent_count += cnt;
            } else {
                report.out_of_agent_count += cnt;
            }
            if outcome == "failed" || outcome == "blocked" {
                report.failed_or_blocked_count += cnt;
            }
            if outcome == "recovered" {
                report.recovered_count += cnt;
            }
        }
        report.top_actions = top_actions;
        Ok(report)
    }

    fn export_to_parquet(&self, table: &str, output_path: &str) -> Result<ExportSummary> {
        let conn = self.lock_conn()?;
        let valid_tables = [
            "assets",
            "changes",
            "memories",
            "fact_nodes",
            "fact_edges",
            "links",
            "active_processes",
            "audit_events",
            "effectiveness_ledger",
            "body_checkpoints",
        ];
        if !valid_tables.contains(&table) {
            anyhow::bail!("不支持导出表: {}, 仅允许: {:?}", table, valid_tables);
        }

        // 底层沙盒路径与防 SQL 注入校验
        let safe_path = validate_safe_export_path(output_path)?;
        let safe_path_str = safe_path.to_string_lossy().to_string();

        // 统计行数
        let count_query = format!("SELECT count(*) FROM {}", table);
        let row_count: i64 = conn.query_row(&count_query, [], |r| r.get(0))?;

        // 确保父目录存在
        if let Some(parent) = safe_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 尝试 DuckDB 原生导出：
        // 优先尝试 PARQUET；若当前环境未内置/无法在线拉取 parquet 扩展，则回退到 CSV 格式以保证离线强壮性
        let is_csv_requested = safe_path_str.ends_with(".csv");
        let sql_escaped_path = safe_path_str.replace('\'', "''");
        let mut exported = false;

        if !is_csv_requested {
            // 禁用在线自动下载扩展，避免在离线/内网环境下挂死
            let _ = conn.execute_batch("SET autoinstall_known_extensions=false; SET autoload_known_extensions=false;");
            let parquet_sql = format!("COPY (SELECT * FROM {}) TO '{}' (FORMAT PARQUET)", table, sql_escaped_path);
            if conn.execute(&parquet_sql, []).is_ok() {
                exported = true;
            }
        }

        if !exported {
            // 回退到 DuckDB 核心原生内置的 CSV 格式（无外部依赖，零挂死）
            // 显式指定 FORMAT CSV，避免 DuckDB 根据 .parquet 后缀推断调用缺失的 parquet 扩展
            let csv_sql = format!("COPY (SELECT * FROM {}) TO '{}' (FORMAT CSV, HEADER, DELIMITER ',')", table, sql_escaped_path);
            conn.execute(&csv_sql, [])
                .with_context(|| format!("DuckDB 导出数据失败: {}", csv_sql))?;
        }

        let file_size_bytes = std::fs::metadata(&safe_path)
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(ExportSummary {
            table: table.to_string(),
            row_count: row_count as usize,
            output_path: safe_path_str,
            file_size_bytes,
        })
    }
}

/// 校验 DuckDB 导出路径：禁止控制字符、单引号、`..` 穿越与软链接逃逸，限定于安全临时导出目录
pub fn validate_safe_export_path(output_path: &str) -> Result<std::path::PathBuf> {
    let trimmed = output_path.trim();
    if trimmed.is_empty() {
        anyhow::bail!("安全拦截：导出路径不能为空");
    }
    if trimmed.chars().any(|c| c == '\0' || c == '\n' || c == '\r' || c == '\'') {
        anyhow::bail!("安全拦截：导出路径包含非法字符（控制字符或单引号）: {}", trimmed);
    }

    let path = std::path::Path::new(trimmed);
    if !path.is_absolute() {
        anyhow::bail!("安全拦截：导出路径必须为绝对路径: {}", trimmed);
    }
    if path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        anyhow::bail!("安全拦截：导出路径禁止包含 '..' 目录穿越组件: {}", trimmed);
    }

    let allowed_roots = crate::config::Config::load_effective_or_default().allowed_temp_roots();

    let in_allowed_lexical = allowed_roots.iter().any(|root| path.starts_with(root));
    if !in_allowed_lexical {
        let roots_desc: Vec<String> = allowed_roots
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        anyhow::bail!(
            "安全拦截：导出路径仅允许位于受信任临时目录 ({}) 下: {}",
            roots_desc.join(", "),
            trimmed
        );
    }

    // 若父目录已存在，进一步解析真实规范路径防止软链接逃逸
    if let Some(parent) = path.parent() {
        if parent.exists() {
            let canon_parent = std::fs::canonicalize(parent)?;
            let canon_allowed = allowed_roots.iter().any(|root| {
                std::fs::canonicalize(root)
                    .map(|cr| canon_parent.starts_with(&cr))
                    .unwrap_or_else(|_| canon_parent.starts_with(root))
            });
            if !canon_allowed {
                anyhow::bail!("安全拦截：导出目录软链接逃逸出安全白名单: {}", canon_parent.display());
            }
        }
    }

    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::duckdb_provider::DuckDbStorageProvider;

    #[test]
    fn test_export_blocks_path_traversal_and_sql_injection() {
        let provider = DuckDbStorageProvider::open_in_memory().expect("open");
        let engine = DuckDbAnalyticsEngine::new(provider.conn());

        assert!(engine.export_to_parquet("assets", "/etc/passwd.csv").is_err());
        assert!(engine.export_to_parquet("assets", "/root/new/tmp/../../etc/shadow.csv").is_err());
        assert!(engine.export_to_parquet("assets", "/tmp/bad'inject.csv").is_err());
        assert!(engine.export_to_parquet("assets", "relative/out.csv").is_err());
    }

    #[test]
    fn test_duckdb_audit_olap_and_export() {
        let sqlite = crate::storage::Storage::open_in_memory().expect("sqlite");
        crate::audit::AuditLogger::record(
            &sqlite,
            crate::audit::AuditEventInput::in_agent("mcp", "tool_call", "body_port_check")
                .with_outcome("success")
                .with_elapsed_us(120),
        )
        .unwrap();
        crate::audit::AuditLogger::record(
            &sqlite,
            crate::audit::AuditEventInput::out_of_agent("governance_engine", "gc", "host_gc", "gc_dry_run")
                .with_outcome("dry_run")
                .with_elapsed_us(340),
        )
        .unwrap();

        let engine = DuckDbAnalyticsEngine::from_synced_storage(sqlite.provider().clone()).expect("sync");
        let summary = engine.audit_analytics_summary(7).expect("olap summary");
        assert_eq!(summary.total_events, 2);
        assert_eq!(summary.in_agent_count, 1);
        assert_eq!(summary.out_of_agent_count, 1);
        assert!(!summary.top_actions.is_empty());

        let tmp = tempfile::tempdir().expect("tempdir");
        let out_csv = tmp.path().join("audit_export.csv");
        let exp = engine
            .export_to_parquet("audit_events", &out_csv.to_string_lossy())
            .expect("export audit_events");
        assert_eq!(exp.row_count, 2);
        assert!(out_csv.exists());
    }
}
