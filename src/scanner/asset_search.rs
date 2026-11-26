use std::collections::HashMap;
use std::path::Path;
use anyhow::Result;
use serde_json::Value;

use crate::body::{WaitEventGuard, WaitEventId};
use crate::memory::embedding::EmbeddingClient;
use crate::memory::hybrid::cosine_similarity;
use crate::scanner::metadata::{format_asset_line, DetailLevel};
use crate::scanner::multimodal::infer_modality;
use crate::storage::models::AssetRecord;
use crate::storage::spi::StorageProvider;

/// 统一资产检索选项（支持 FTS5 + 向量 RRF 混合检索 + 多模态/领域/KV 结构化过滤）
#[derive(Debug, Clone)]
pub struct AssetSearchOptions {
    pub query: String,
    pub limit: usize,
    pub modality: Option<String>,
    pub domain: Option<String>,
    pub kv_key: Option<String>,
    pub kv_value: Option<String>,
    pub hybrid: bool,
}

impl AssetSearchOptions {
    pub fn new(query: impl Into<String>, limit: usize) -> Self {
        Self {
            query: query.into(),
            limit: limit.clamp(1, 200),
            modality: None,
            domain: None,
            kv_key: None,
            kv_value: None,
            hybrid: true,
        }
    }

    pub fn has_filters(&self) -> bool {
        self.modality.as_deref().is_some_and(|s| !s.trim().is_empty())
            || self.domain.as_deref().is_some_and(|s| !s.trim().is_empty())
            || self.kv_key.as_deref().is_some_and(|s| !s.trim().is_empty())
            || self.kv_value.as_deref().is_some_and(|s| !s.trim().is_empty())
    }
}

/// 资产检索命中项
#[derive(Debug, Clone)]
pub struct AssetSearchHit {
    pub asset: AssetRecord,
    pub score: f64,
    pub matched_channels: Vec<String>,
}

/// 资产混合检索引擎（FTS5 + Vector RRF + 多模态结构化过滤）
pub struct AssetSearchEngine;

impl AssetSearchEngine {
    pub fn effective_modality(asset: &AssetRecord) -> String {
        if let Some(m) = asset.desc_kv.get("modality").and_then(|v| v.as_str()) {
            let trimmed = m.trim().to_ascii_lowercase();
            if !trimmed.is_empty() {
                return trimmed;
            }
        }
        let ext = Path::new(&asset.path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");
        infer_modality(&asset.asset_type, ext).to_string()
    }

    fn value_is_non_empty(val: &Value) -> bool {
        match val {
            Value::Null => false,
            Value::String(s) => !s.trim().is_empty(),
            Value::Array(arr) => !arr.is_empty(),
            Value::Object(map) => !map.is_empty(),
            _ => true,
        }
    }

    fn value_contains_ci(val: &Value, needle_lower: &str) -> bool {
        match val {
            Value::Null => false,
            Value::String(s) => s.to_ascii_lowercase().contains(needle_lower),
            Value::Number(n) => n.to_string().to_ascii_lowercase().contains(needle_lower),
            Value::Bool(b) => b.to_string().contains(needle_lower),
            Value::Array(arr) => arr.iter().any(|item| Self::value_contains_ci(item, needle_lower)),
            Value::Object(map) => map.values().any(|v| Self::value_contains_ci(v, needle_lower)),
        }
    }

    /// 结构化过滤条件校验（`modality` / `domain` / `kv_key` / `kv_value`）
    pub fn matches_filters(asset: &AssetRecord, opts: &AssetSearchOptions) -> bool {
        if let Some(ref m) = opts.modality {
            let want = m.trim().to_ascii_lowercase();
            if !want.is_empty() && Self::effective_modality(asset) != want {
                return false;
            }
        }

        if let Some(ref d) = opts.domain {
            let want = d.trim().to_ascii_lowercase();
            if !want.is_empty() {
                let got = asset
                    .desc_kv
                    .get("domain")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if got != want && !got.contains(&want) {
                    return false;
                }
            }
        }

        let kv_key_clean = opts.kv_key.as_deref().map(str::trim).filter(|s| !s.is_empty());
        let kv_val_clean = opts.kv_value.as_deref().map(str::trim).filter(|s| !s.is_empty());

        match (kv_key_clean, kv_val_clean) {
            (Some(k), Some(v)) => {
                let needle = v.to_ascii_lowercase();
                let Some(val) = asset.desc_kv.get(k) else {
                    return false;
                };
                if !Self::value_contains_ci(val, &needle) {
                    return false;
                }
            }
            (Some(k), None) => {
                let Some(val) = asset.desc_kv.get(k) else {
                    return false;
                };
                if !Self::value_is_non_empty(val) {
                    return false;
                }
            }
            (None, Some(v)) => {
                let needle = v.to_ascii_lowercase();
                if !Self::value_contains_ci(&asset.desc_kv, &needle) {
                    return false;
                }
            }
            (None, None) => {}
        }

        true
    }

    /// 执行资产混合搜索（自动判断是否启用本地 Ollama Query Embedding）
    pub fn search(
        storage: &dyn StorageProvider,
        opts: &AssetSearchOptions,
    ) -> Result<Vec<AssetSearchHit>> {
        let q = opts.query.trim();
        let query_vec = if opts.hybrid && !q.is_empty() {
            let client = EmbeddingClient::default();
            if client.is_available() {
                client.embed_text(q)
            } else {
                None
            }
        } else {
            None
        };
        Self::search_with_query_vec(storage, opts, query_vec.as_deref())
    }

    /// 使用给定 Query Embedding 向量执行 FTS5 + Vector RRF + 结构化过滤检索（可离线确定性单测）
    pub fn search_with_query_vec(
        storage: &dyn StorageProvider,
        opts: &AssetSearchOptions,
        query_vec: Option<&[f32]>,
    ) -> Result<Vec<AssetSearchHit>> {
        let limit = opts.limit.clamp(1, 200);
        let q = opts.query.trim();

        // 空查询：直接走结构化过滤列表
        if q.is_empty() {
            let scan_limit = if opts.has_filters() { 5000 } else { limit };
            let assets = storage.list_assets(scan_limit)?;
            let mut hits = Vec::new();
            for a in assets {
                if Self::matches_filters(&a, opts) {
                    let imp = a
                        .desc_kv
                        .get("importance_score")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.5);
                    hits.push(AssetSearchHit {
                        asset: a,
                        score: imp,
                        matched_channels: vec!["filter".to_string()],
                    });
                    if hits.len() >= limit {
                        break;
                    }
                }
            }
            return Ok(hits);
        }

        let fts_fetch = if opts.has_filters() {
            (limit * 10).clamp(100, 1000)
        } else {
            (limit * 4).clamp(40, 200)
        };
        let fts_raw = storage.search_assets_fts(q, fts_fetch)?;

        let mut merged: HashMap<String, AssetSearchHit> = HashMap::new();
        let mut fts_rank = 0usize;
        for a in fts_raw {
            if !Self::matches_filters(&a, opts) {
                continue;
            }
            let rrf = 1.0 / (60.0 + fts_rank as f64);
            fts_rank += 1;
            merged.insert(
                a.path.clone(),
                AssetSearchHit {
                    asset: a,
                    score: rrf,
                    matched_channels: vec!["fts".to_string()],
                },
            );
        }

        // 若带结构化过滤且 FTS 命中不足，对降序资产做一次轻量内存子串兜底
        let need_scan = query_vec.is_some() || (opts.has_filters() && merged.len() < limit);
        if need_scan {
            let candidates = storage.list_assets(5000)?;

            if opts.has_filters() && merged.len() < limit {
                let q_lower = q.to_ascii_lowercase();
                let mut kw_rank = fts_rank;
                for a in &candidates {
                    if merged.contains_key(&a.path) || !Self::matches_filters(a, opts) {
                        continue;
                    }
                    let text_match = a.path.to_ascii_lowercase().contains(&q_lower)
                        || a.description
                            .as_deref()
                            .unwrap_or("")
                            .to_ascii_lowercase()
                            .contains(&q_lower)
                        || Self::value_contains_ci(&a.desc_kv, &q_lower);
                    if text_match {
                        let rrf = 1.0 / (60.0 + kw_rank as f64) * 0.85;
                        kw_rank += 1;
                        merged.insert(
                            a.path.clone(),
                            AssetSearchHit {
                                asset: a.clone(),
                                score: rrf,
                                matched_channels: vec!["kv_scan".to_string()],
                            },
                        );
                    }
                }
            }

            if let Some(qvec) = query_vec {
                let _wait = WaitEventGuard::enter(WaitEventId::CpuVectorCosine);
                let mut vec_scored: Vec<(AssetRecord, f32)> = Vec::new();
                for a in candidates {
                    if a.desc_kv.get("embedding_status").and_then(|v| v.as_str()) != Some("ready") {
                        continue;
                    }
                    if !Self::matches_filters(&a, opts) {
                        continue;
                    }
                    if let Ok(vec) = storage.get_asset_embedding(&a.path) {
                        if vec.len() == qvec.len() && !vec.is_empty() {
                            let sim = cosine_similarity(qvec, &vec);
                            if sim > 0.05 {
                                vec_scored.push((a, sim));
                            }
                        }
                    }
                }
                vec_scored.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap_or(std::cmp::Ordering::Equal));
                let vec_cap = (limit * 4).max(40);
                for (rank, (a, _sim)) in vec_scored.into_iter().take(vec_cap).enumerate() {
                    let rrf = 1.0 / (60.0 + rank as f64);
                    if let Some(hit) = merged.get_mut(&a.path) {
                        hit.score += rrf;
                        if !hit.matched_channels.iter().any(|c| c == "vec") {
                            hit.matched_channels.push("vec".to_string());
                        }
                    } else {
                        merged.insert(
                            a.path.clone(),
                            AssetSearchHit {
                                asset: a,
                                score: rrf,
                                matched_channels: vec!["vec".to_string()],
                            },
                        );
                    }
                }
            }
        }

        let mut ranked: Vec<AssetSearchHit> = merged
            .into_values()
            .map(|mut h| {
                let imp = h
                    .asset
                    .desc_kv
                    .get("importance_score")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.5);
                h.score += imp * 0.002;
                h
            })
            .collect();

        ranked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.asset.path.cmp(&b.asset.path))
        });
        ranked.truncate(limit);
        Ok(ranked)
    }

    /// 统一渲染搜索结果文本（供 CLI / MCP 复用）
    pub fn format_results(hits: &[AssetSearchHit], detail: DetailLevel) -> String {
        let mut out = format!(
            "资产搜索匹配结果 (共 {} 条, detail={:?}):\n",
            hits.len(),
            detail
        );
        for h in hits {
            out.push_str(&format_asset_line(&h.asset, detail));
            if !matches!(detail, DetailLevel::Micro) && !h.matched_channels.is_empty() {
                out.push_str(&format!("\n    召回通道: [{}]", h.matched_channels.join("+")));
            }
            out.push('\n');
        }
        out
    }
}

/// 顶层便捷格式化函数导出
pub fn format_search_results(hits: &[AssetSearchHit], detail: DetailLevel) -> String {
    AssetSearchEngine::format_results(hits, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Storage;
    use serde_json::json;

    fn sample_asset(path: &str, asset_type: &str, desc: &str, kv: Value) -> AssetRecord {
        AssetRecord {
            id: uuid::Uuid::new_v4().to_string(),
            path: path.to_string(),
            asset_type: asset_type.to_string(),
            owner: "user:tester".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some(desc.to_string()),
            desc_kv: kv,
            usage: None,
            size_bytes: Some(1024),
            file_count: None,
            tags: vec![],
            status: "active".to_string(),
        }
    }

    #[test]
    fn test_structured_filters_and_vector_rrf_recall() {
        let storage = Storage::open_in_memory().expect("open_in_memory");

        let img = sample_asset(
            "/tmp/test/arch_diagram.png",
            "media",
            "[web_dashboard] PNG 图像 arch_diagram.png 1280x720",
            json!({
                "modality": "image",
                "domain": "web_dashboard",
                "dimensions": "1280x720",
                "visual_summary": "系统三态运行模式架构拓扑图",
                "embedding_status": "ready",
                "importance_score": 0.9
            }),
        );
        let db = sample_asset(
            "/tmp/test/analytics.db",
            "dataset",
            "[storage_engine] DB 数据库，含表 [orders, users]",
            json!({
                "modality": "dataset",
                "domain": "storage_engine",
                "table_names": ["orders", "users"],
                "columns": ["orders.amount", "orders.region"],
                "embedding_status": "pending",
                "importance_score": 0.7
            }),
        );
        let code = sample_asset(
            "/tmp/test/main.rs",
            "script",
            "[asset_scanner] Rust scanner entrypoint",
            json!({
                "modality": "code",
                "domain": "asset_scanner",
                "embedding_status": "ready",
                "importance_score": 0.8
            }),
        );

        storage.upsert_asset(&img).expect("upsert img");
        storage.upsert_asset(&db).expect("upsert db");
        storage.upsert_asset(&code).expect("upsert code");
        storage
            .update_asset_embedding(&img.path, &[1.0, 0.0, 0.0])
            .expect("embed img");
        storage
            .update_asset_embedding(&code.path, &[0.0, 1.0, 0.0])
            .expect("embed code");

        // 1. 纯语义向量召回（英文查询与中文 visual_summary 无 FTS 字面交集，但向量完全匹配）
        let opts_vec = AssetSearchOptions::new("system topology diagram", 5);
        let hits = AssetSearchEngine::search_with_query_vec(
            storage.provider().as_ref(),
            &opts_vec,
            Some(&[0.98, 0.02, 0.0]),
        )
        .expect("vec search");
        assert!(!hits.is_empty());
        assert_eq!(hits[0].asset.path, "/tmp/test/arch_diagram.png");
        assert!(hits[0].matched_channels.contains(&"vec".to_string()));

        // 2. modality 过滤
        let mut opts_mod = AssetSearchOptions::new("", 10);
        opts_mod.modality = Some("dataset".to_string());
        let db_hits =
            AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts_mod, None)
                .expect("mod search");
        assert_eq!(db_hits.len(), 1);
        assert_eq!(db_hits[0].asset.path, "/tmp/test/analytics.db");

        // 3. kv_key + kv_value 过滤
        let mut opts_kv = AssetSearchOptions::new("analytics", 10);
        opts_kv.kv_key = Some("table_names".to_string());
        opts_kv.kv_value = Some("orders".to_string());
        let kv_hits =
            AssetSearchEngine::search_with_query_vec(storage.provider().as_ref(), &opts_kv, None)
                .expect("kv search");
        assert_eq!(kv_hits.len(), 1);
        assert_eq!(kv_hits[0].asset.path, "/tmp/test/analytics.db");
    }
}
