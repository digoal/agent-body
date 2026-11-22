use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::placement::engine_v2::{PlacementEngineV2, PlacementRequestV2};
use crate::storage::models::{AssetRecord, FactEdgeRecord, FactNodeRecord};
use crate::storage::{AnalyticsProvider, DuckDbAnalyticsEngine, DuckDbStorageProvider, Storage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicroBenchResult {
    pub id: String,
    pub name: String,
    pub iterations: usize,
    pub avg_us: u128,
    pub p50_us: u128,
    pub p95_us: u128,
    pub p99_us: u128,
    pub sla_limit_us: u128,
    pub passed: bool,
    pub detail: String,
}

fn compute_percentiles(samples: &mut [u128]) -> (u128, u128, u128, u128) {
    if samples.is_empty() {
        return (0, 0, 0, 0);
    }
    samples.sort_unstable();
    let n = samples.len();
    let sum: u128 = samples.iter().sum();
    let avg = sum / n as u128;
    let p50 = samples[n / 2];
    let p95 = samples[((n as f64 * 0.95).ceil() as usize).min(n).saturating_sub(1)];
    let p99 = samples[((n as f64 * 0.99).ceil() as usize).min(n).saturating_sub(1)];
    (avg, p50, p95, p99)
}

struct GuardTempDir(PathBuf);

impl Drop for GuardTempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 运行 5 大内核微基准 SLA (M1~M5)
pub fn run_all_micro_benchmarks() -> Result<Vec<MicroBenchResult>> {
    let mut results = Vec::with_capacity(5);
    let is_debug = cfg!(debug_assertions);

    // M1: 属性图 3 跳遍历
    {
        let storage = Storage::open_in_memory()?;
        for i in 0..100 {
            storage.upsert_fact_node(&FactNodeRecord {
                id: format!("node:{}", i),
                node_type: if i % 3 == 0 {
                    "service"
                } else if i % 3 == 1 {
                    "port"
                } else {
                    "project"
                }
                .to_string(),
                name: format!("entity_{}", i),
                label: format!("实体 {}", i),
                properties: serde_json::json!({"index": i}),
                updated_at: None,
            })?;
            if i > 0 {
                storage.upsert_fact_edge(&FactEdgeRecord::new_explicit(
                    format!("node:{}", i - 1),
                    format!("node:{}", i),
                    "depends_on",
                    serde_json::json!({}),
                    0.95,
                ))?;
            }
        }
        let iters = 50usize;
        let mut samples = Vec::with_capacity(iters);
        for _ in 0..iters {
            let t = Instant::now();
            let sub = storage.traverse_fact_graph("node:10", 3)?;
            samples.push(t.elapsed().as_micros());
            if sub.nodes.is_empty() {
                anyhow::bail!("M1 graph traversal returned empty subgraph");
            }
        }
        let (avg_us, p50_us, p95_us, p99_us) = compute_percentiles(&mut samples);
        let sla = if is_debug { 10_000 } else { 1_500 };
        results.push(MicroBenchResult {
            id: "M1".into(),
            name: "micro_graph_3hop".into(),
            iterations: iters,
            avg_us,
            p50_us,
            p95_us,
            p99_us,
            sla_limit_us: sla,
            passed: avg_us < sla,
            detail: "100 节点属性图 3 跳遍历".into(),
        });
    }

    // M2: 资产 FTS5 全文检索
    {
        let storage = Storage::open_in_memory()?;
        for i in 0..300 {
            storage.upsert_asset(&AssetRecord {
                id: format!("asset-{}", i),
                path: format!("/root/new/shared_proj/project_{}/module.rs", i),
                asset_type: "script".to_string(),
                owner: "user:digoal".to_string(),
                security_level: "internal".to_string(),
                scan_tier: "deep".to_string(),
                description: Some(format!("PostgreSQL 数据库内核高可用及监控脚本 模块 {}", i)),
                desc_kv: serde_json::json!({"mod": i}),
                usage: Some("cargo run".to_string()),
                size_bytes: Some(1024),
                file_count: Some(1),
                tags: vec!["postgres".to_string(), "script".to_string()],
                status: "active".to_string(),
            })?;
        }
        let iters = 50usize;
        let mut samples = Vec::with_capacity(iters);
        for _ in 0..iters {
            let t = Instant::now();
            let hits = storage.search_assets_fts("PostgreSQL", 10)?;
            samples.push(t.elapsed().as_micros());
            if hits.is_empty() {
                anyhow::bail!("M2 FTS search returned empty hits");
            }
        }
        let (avg_us, p50_us, p95_us, p99_us) = compute_percentiles(&mut samples);
        let sla = if is_debug { 10_000 } else { 1_500 };
        results.push(MicroBenchResult {
            id: "M2".into(),
            name: "micro_fts5_search".into(),
            iterations: iters,
            avg_us,
            p50_us,
            p95_us,
            p99_us,
            sla_limit_us: sla,
            passed: avg_us < sla,
            detail: "300 资产 SQLite FTS5 全文检索".into(),
        });
    }

    // M3: 放置引擎决策
    {
        let storage = Storage::open_in_memory()?;
        let engine = PlacementEngineV2::new(storage);
        let iters = 200usize;
        let mut samples = Vec::with_capacity(iters);
        for i in 0..iters {
            let req = PlacementRequestV2 {
                object_type: "script".to_string(),
                name: format!("pg_vacuum_analyzer_{}.py", i),
                intent: "PostgreSQL 垃圾回收与死元组诊断工具".to_string(),
                tags: vec!["postgres".to_string()],
                security_level: "internal".to_string(),
                current_workspace: None,
            };
            let t = Instant::now();
            let _res = engine.suggest(&req)?;
            samples.push(t.elapsed().as_micros());
        }
        let (avg_us, p50_us, p95_us, p99_us) = compute_percentiles(&mut samples);
        let sla = if is_debug { 1_000 } else { 400 };
        results.push(MicroBenchResult {
            id: "M3".into(),
            name: "micro_placement_suggest".into(),
            iterations: iters,
            avg_us,
            p50_us,
            p95_us,
            p99_us,
            sla_limit_us: sla,
            passed: avg_us < sla,
            detail: "8 规则放置引擎意图匹配与路径推导".into(),
        });
    }

    // M4: DuckDB 1000 条变更热力图聚合
    {
        let provider = DuckDbStorageProvider::open_in_memory()?;
        {
            let conn_arc = provider.conn();
            let conn = conn_arc
                .lock()
                .map_err(|e| anyhow::anyhow!("DuckDB lock poisoned: {}", e))?;
            conn.execute_batch(
                "INSERT INTO changes (type, target, description, details, source, created_at)
                 SELECT
                     CASE WHEN i % 2 = 0 THEN 'file_modified' ELSE 'file_created' END,
                     '/root/new/work/proj_' || (i % 20) || '/src/file_' || (i % 50) || '.rs',
                     'change ' || i,
                     '{\"iter\": ' || i || '}',
                     'agent',
                     strftime(now(), '%Y-%m-%dT%H:%M:%SZ')
                 FROM generate_series(1, 1000) t(i);",
            )?;
        }
        let engine = DuckDbAnalyticsEngine::new(provider.conn());
        let iters = 20usize;
        let mut samples = Vec::with_capacity(iters);
        for _ in 0..iters {
            let t = Instant::now();
            let _hm = engine.changes_heatmap(30)?;
            let _top = engine.most_changed_assets(30, 10)?;
            samples.push(t.elapsed().as_micros());
        }
        let (avg_us, p50_us, p95_us, p99_us) = compute_percentiles(&mut samples);
        let sla = if is_debug { 150_000 } else { 60_000 };
        results.push(MicroBenchResult {
            id: "M4".into(),
            name: "micro_duckdb_heatmap".into(),
            iterations: iters,
            avg_us,
            p50_us,
            p95_us,
            p99_us,
            sla_limit_us: sla,
            passed: avg_us < sla,
            detail: "DuckDB 1000 条变更热力图 + Top-10 OLAP 聚合".into(),
        });
    }

    // M5: DuckDB 1000 条资产 Parquet 导出
    {
        let provider = DuckDbStorageProvider::open_in_memory()?;
        {
            let conn_arc = provider.conn();
            let conn = conn_arc
                .lock()
                .map_err(|e| anyhow::anyhow!("DuckDB lock poisoned: {}", e))?;
            conn.execute_batch(
                "INSERT INTO assets (id, path, type, owner, security_level, scan_tier, description, desc_kv, usage, size_bytes, file_count, tags, status, updated_at)
                 SELECT
                     'asset-bench-' || i,
                     '/root/new/work/repo_' || (i % 10) || '/item_' || i || '.rs',
                     'project',
                     'user:digoal',
                     'internal',
                     'deep',
                     'DuckDB 性能基准测试资产条目 ' || i,
                     '{\"index\": ' || i || '}',
                     NULL,
                     1024,
                     5,
                     '[\"benchmark\"]',
                     'active',
                     strftime(now(), '%Y-%m-%dT%H:%M:%SZ')
                 FROM generate_series(1, 1000) t(i);",
            )?;
        }
        let engine = DuckDbAnalyticsEngine::new(provider.conn());
        let tmp_dir = std::env::temp_dir().join(format!("steward_micro_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp_dir)?;
        let _guard = GuardTempDir(tmp_dir.clone());
        let export_path = tmp_dir.join("bench_export.parquet");

        let t = Instant::now();
        let summary = engine.export_to_parquet("assets", &export_path.to_string_lossy())?;
        let elapsed_us = t.elapsed().as_micros();
        let sla = if is_debug { 500_000 } else { 150_000 };
        results.push(MicroBenchResult {
            id: "M5".into(),
            name: "micro_duckdb_parquet".into(),
            iterations: 1,
            avg_us: elapsed_us,
            p50_us: elapsed_us,
            p95_us: elapsed_us,
            p99_us: elapsed_us,
            sla_limit_us: sla,
            passed: elapsed_us < sla && summary.row_count == 1000,
            detail: format!("DuckDB 1000 条资产导出 Parquet ({} bytes)", summary.file_size_bytes),
        });
    }

    Ok(results)
}
