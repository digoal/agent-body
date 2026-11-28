use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use anyhow::Result;
use crate::storage::{Storage, models::MemoryRecord};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HybridSearchResult {
    pub memory: MemoryRecord,
    pub rrf_score: f32,
    pub fts_rank: Option<usize>,
    pub vec_rank: Option<usize>,
    pub cosine_similarity: Option<f32>,
}

pub struct HybridSearchEngine {
    storage: Arc<Storage>,
    ollama_endpoint: String,
    model_name: String,
    fallback_fts_only: bool,
    http_client: reqwest::Client,
}

fn build_embedding_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1500))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

impl HybridSearchEngine {
    /// 从全局配置文件 `[embedding]` 段加载 Ollama 端点、模型与降级策略（配置缺失时回退为 `EmbeddingConfig::default()`）
    pub fn new(storage: Arc<Storage>) -> Self {
        let cfg = crate::config::Config::load_effective_or_default().embedding;
        Self::from_config(storage, &cfg)
    }

    /// 使用指定的 `EmbeddingConfig` 构造混合检索引擎
    pub fn from_config(storage: Arc<Storage>, cfg: &crate::config::EmbeddingConfig) -> Self {
        Self {
            storage,
            ollama_endpoint: cfg.ollama_url.trim_end_matches('/').to_string(),
            model_name: cfg.model.clone(),
            fallback_fts_only: cfg.fallback_fts_only,
            http_client: build_embedding_http_client(),
        }
    }

    /// 使用显式指定的 Ollama 端点与模型名称构造混合检索引擎（默认允许 FTS 降级）
    pub fn with_endpoint(storage: Arc<Storage>, ollama_endpoint: &str, model_name: &str) -> Self {
        Self {
            storage,
            ollama_endpoint: ollama_endpoint.trim_end_matches('/').to_string(),
            model_name: model_name.to_string(),
            fallback_fts_only: true,
            http_client: build_embedding_http_client(),
        }
    }

    pub fn ollama_endpoint(&self) -> &str {
        &self.ollama_endpoint
    }

    pub fn model_name(&self) -> &str {
        &self.model_name
    }

    /// 尝试调用本地 Ollama 获取文本的 embedding 向量（端点与模型由 `[embedding]` 配置决定）
    pub async fn fetch_embedding(&self, text: &str) -> Option<Vec<f32>> {
        let body = serde_json::json!({
            "model": self.model_name,
            "prompt": text,
        });

        let resp = self
            .http_client
            .post(format!("{}/api/embeddings", self.ollama_endpoint))
            .json(&body)
            .send()
            .await
            .ok()?;

        if resp.status().is_success() {
            let res_json: serde_json::Value = resp.json().await.ok()?;
            if let Some(arr) = res_json.get("embedding").and_then(|v| v.as_array()) {
                let vec: Vec<f32> = arr.iter().filter_map(|x| x.as_f64().map(|f| f as f32)).collect();
                if !vec.is_empty() {
                    return Some(vec);
                }
            }
        }

        None
    }

    /// 混合检索核心方法：Omni-RRF (BM25/FTS5 + Fuzzy + 向量余弦 + Know-How 图谱扩散 + 标量调制)
    pub async fn search_hybrid(&self, query: &str, limit: usize) -> Result<Vec<HybridSearchResult>> {
        // 1. 获取 BM25 / FTS5 关键词排序列表
        let fts_results = self.storage.search_memories_fts(query, limit * 2).unwrap_or_default();
        let mut fts_ranks: HashMap<String, usize> = HashMap::new();
        for (idx, m) in fts_results.iter().enumerate() {
            fts_ranks.insert(m.id.clone(), idx + 1);
        }

        // 2. 尝试获取 Query 的向量进行语义计算
        let query_vec = self.fetch_embedding(query).await;
        if query_vec.is_none() && !self.fallback_fts_only {
            anyhow::bail!(
                "Ollama embedding 服务不可用 ({}, model={}) 且配置 [embedding].fallback_fts_only=false，拒绝静默降级为纯 FTS 检索",
                self.ollama_endpoint,
                self.model_name
            );
        }
        let mut vec_ranks: HashMap<String, usize> = HashMap::new();
        let mut cos_sims: HashMap<String, f32> = HashMap::new();

        // 单次查库加载活跃记忆（提升上限至 2000，消除重复全表查询）
        let all_memories = self.storage.list_all_active_memories(2000)?;
        let mut all_memories_map: HashMap<String, MemoryRecord> =
            HashMap::with_capacity(all_memories.len().saturating_add(fts_results.len()));
        for fts_m in fts_results {
            all_memories_map.insert(fts_m.id.clone(), fts_m);
        }
        let db_fuzzy_mems = self
            .storage
            .search_memories_fuzzy_db(query, limit * 3)
            .unwrap_or_default();
        for fm in db_fuzzy_mems {
            let next_rank = fts_ranks.len() + 1;
            fts_ranks.entry(fm.id.clone()).or_insert(next_rank);
            all_memories_map.entry(fm.id.clone()).or_insert(fm);
        }

        // 2.5 Pure-Rust Fuzzy 模糊通道 + Know-How 知识图扩散通道（无向量时重点补偿语义断层）
        let mut fuzzy_scored: Vec<(String, f64)> = Vec::new();
        for m in &all_memories {
            let hay = format!(
                "{} {} {}",
                m.summary.as_deref().unwrap_or(""),
                m.content,
                m.tags.join(" ")
            );
            let fz = crate::scanner::asset_search::fuzzy_text_score(query, &hay);
            if fz >= 0.30 {
                fuzzy_scored.push((m.id.clone(), fz));
            }
        }
        fuzzy_scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut fuzzy_ranks: HashMap<String, usize> = HashMap::new();
        for (idx, (m_id, _)) in fuzzy_scored.into_iter().take(limit * 3).enumerate() {
            fuzzy_ranks.insert(m_id, idx + 1);
        }

        let aliases = crate::fact::AliasCanonicalizer::default();
        let g_exp = crate::fact::expand_graph_for_query(
            self.storage.provider().as_ref(),
            query,
            1,
            &aliases,
        );
        let mut g_mem_vec: Vec<(String, f64)> = g_exp.memory_scores.into_iter().collect();
        g_mem_vec.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let mut graph_ranks: HashMap<String, usize> = HashMap::new();
        for (idx, (m_id, _)) in g_mem_vec.into_iter().take(limit * 3).enumerate() {
            graph_ranks.insert(m_id, idx + 1);
        }

        if let Some(ref q_emb) = query_vec {
            let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::CpuVectorCosine);
            let mut scored_memories: Vec<(String, f32)> = Vec::new();

            const MIN_MEMORY_VEC_SIM: f32 = 0.52;
            for m in all_memories {
                if !m.embedding.is_empty() && m.embedding.len() == q_emb.len() {
                    let sim = cosine_similarity(q_emb, &m.embedding);
                    if sim.is_finite() && sim >= MIN_MEMORY_VEC_SIM {
                        scored_memories.push((m.id.clone(), sim));
                    }
                }
                all_memories_map.insert(m.id.clone(), m);
            }

            // 按余弦相似度倒序排序
            scored_memories.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            for (idx, (m_id, sim)) in scored_memories.into_iter().take(limit * 2).enumerate() {
                vec_ranks.insert(m_id.clone(), idx + 1);
                cos_sims.insert(m_id, sim);
            }
        } else {
            for m in all_memories {
                all_memories_map.insert(m.id.clone(), m);
            }
        }

        // 3. 计算 Omni-RRF 得分并叠加记忆 weight/pinned 事实标量调制
        let mut candidate_ids: Vec<String> = fts_ranks
            .keys()
            .chain(vec_ranks.keys())
            .chain(fuzzy_ranks.keys())
            .chain(graph_ranks.keys())
            .cloned()
            .collect();
        candidate_ids.sort();
        candidate_ids.dedup();

        type RrfScoreRow = (String, f32, Option<usize>, Option<usize>, Option<f32>);
        let mut rrf_scored: Vec<RrfScoreRow> = Vec::new();
        let k = 60.0f32;

        for id in candidate_ids {
            let fts_r = fts_ranks.get(&id).copied();
            let vec_r = vec_ranks.get(&id).copied();
            let fuzzy_r = fuzzy_ranks.get(&id).copied();
            let graph_r = graph_ranks.get(&id).copied();
            let sim = cos_sims.get(&id).copied();

            let score_fts = fts_r.map(|r| 1.2 / (k + r as f32)).unwrap_or(0.0);
            let score_vec = vec_r.map(|r| 1.0 / (k + r as f32)).unwrap_or(0.0);
            let score_fuzzy = fuzzy_r.map(|r| 0.7 / (k + r as f32)).unwrap_or(0.0);
            let score_graph = graph_r.map(|r| 1.0 / (k + r as f32)).unwrap_or(0.0);
            let scalar_mod = all_memories_map
                .get(&id)
                .map(|m| {
                    let pin = if m.pinned { 0.15 } else { 0.0 };
                    (0.85 + 0.15 * m.weight.clamp(0.0, 2.0) + pin).clamp(0.6, 1.4)
                })
                .unwrap_or(1.0);
            let total_rrf = (score_fts + score_vec + score_fuzzy + score_graph) * scalar_mod;

            rrf_scored.push((id, total_rrf, fts_r, vec_r, sim));
        }

        // 4. 按 RRF 评分排序并组织结果返回
        rrf_scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut final_results = Vec::new();
        for (id, rrf_score, fts_rank, vec_rank, cosine_similarity) in rrf_scored.into_iter().take(limit) {
            if let Some(mem) = all_memories_map.get(&id) {
                final_results.push(HybridSearchResult {
                    memory: mem.clone(),
                    rrf_score,
                    fts_rank,
                    vec_rank,
                    cosine_similarity,
                });
            }
        }

        Ok(final_results)
    }

    /// 评估查询可预测性并判定是否需要旁路睡眠整理摘要直查原始事件
    pub fn evaluate_predictability(
        &self,
        query: &str,
        as_of_epoch: Option<i64>,
        include_history: bool,
    ) -> ext_mem_pg_tx::PredictabilityDecision {
        let summaries: Vec<String> = self
            .storage
            .list_all_active_memories(200)
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.tags.iter().any(|t| t == "sleep_compacted" || t == "sleep_summary"))
            .map(|m| m.content)
            .collect();
        let refs: Vec<&str> = summaries.iter().map(|s| s.as_str()).collect();
        ext_mem_pg_tx::QueryPredictabilityGate::evaluate(query, &refs, as_of_epoch, include_history)
    }

    /// 带可预测性旁路门禁的混合检索：
    /// - 高可预测常规查询：优先返回高密度 `sleep_compacted` 摘要以压缩上下文 Token；
    /// - 不可预测探索型查询：自动旁路摘要偏置并回补被整理归档的原始详情（`status='archived'` 且非 `outdated`），确保 `ΔRecall >= 0`。
    pub async fn search_hybrid_with_gate(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<(Vec<HybridSearchResult>, ext_mem_pg_tx::PredictabilityDecision)> {
        let dec = self.evaluate_predictability(query, None, false);
        let mut results = self.search_hybrid(query, limit.saturating_mul(2).max(6)).await?;

        if !dec.bypass_compacted {
            for r in &mut results {
                let is_sum = r
                    .memory
                    .tags
                    .iter()
                    .any(|t| t == "sleep_compacted" || t == "sleep_summary");
                if is_sum {
                    r.rrf_score *= 1.35;
                }
            }
            results.sort_by(|a, b| b.rrf_score.partial_cmp(&a.rrf_score).unwrap_or(std::cmp::Ordering::Equal));
            results.truncate(limit);
            return Ok((results, dec));
        }

        // 探索型旁路：若活跃摘要未覆盖细节关键词，从 archived（睡眠整理折叠保留的原件，非 outdated/tombstoned）回补召回
        let q_lower = query.trim().to_ascii_lowercase();
        let q_terms: Vec<String> = q_lower
            .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c <= '\u{7f}')
            .map(|s| s.trim().to_string())
            .filter(|s| s.len() >= 3)
            .collect();
        let mut seen: std::collections::HashSet<String> =
            results.iter().map(|r| r.memory.id.clone()).collect();
        let all_archived = self.storage.list_all_memories(1000).unwrap_or_default();
        for m in all_archived {
            if m.status != "archived" || seen.contains(&m.id) {
                continue;
            }
            let hay = format!("{} {} {}", m.id, m.content, m.summary.as_deref().unwrap_or("")).to_ascii_lowercase();
            let hit_cnt = q_terms.iter().filter(|t| hay.contains(t.as_str())).count();
            if hit_cnt > 0 {
                let boost = 0.02 + (hit_cnt as f32 * 0.015);
                seen.insert(m.id.clone());
                results.push(HybridSearchResult {
                    memory: m,
                    rrf_score: boost,
                    fts_rank: None,
                    vec_rank: None,
                    cosine_similarity: None,
                });
            }
        }
        results.sort_by(|a, b| b.rrf_score.partial_cmp(&a.rrf_score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);
        Ok((results, dec))
    }
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a * norm_b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cosine_similarity() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![1.0, 0.0, 0.0];
        let v3 = vec![0.0, 1.0, 0.0];

        assert!((cosine_similarity(&v1, &v2) - 1.0).abs() < 1e-5);
        assert!((cosine_similarity(&v1, &v3) - 0.0).abs() < 1e-5);
    }

    #[tokio::test]
    async fn test_hybrid_search_graceful_fallback() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let engine = HybridSearchEngine::new(storage.clone());

        storage.insert_memory(&MemoryRecord {
            id: "mem-test-1".to_string(),
            memory_type: "decision".to_string(),
            content: "PostgreSQL pool failover plan".to_string(),
            summary: Some("PG plan".to_string()),
            source_agent: Some("test".to_string()),
            source_session: None,
            tags: vec!["pg".to_string(), "emergency".to_string()],
            weight: 1.0,
            pinned: false,
            status: "active".to_string(),
            embedding: vec![],
        }).unwrap();

        // 即使没有本地 embedding 向量，纯 FTS5 也能正常优雅返回结果
        let results = engine.search_hybrid("pool", 5).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].memory.id, "mem-test-1");
        assert!(results[0].fts_rank.is_some());
    }

    #[test]
    fn test_hybrid_search_engine_respects_embedding_config() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let cfg = crate::config::EmbeddingConfig {
            ollama_url: "http://192.168.1.100:11434/".to_string(),
            model: "qwen3-embedding:0.6b".to_string(),
            dimensions: 1024,
            fallback_fts_only: true,
        };
        let engine = HybridSearchEngine::from_config(storage, &cfg);
        assert_eq!(engine.ollama_endpoint(), "http://192.168.1.100:11434");
        assert_eq!(engine.model_name(), "qwen3-embedding:0.6b");
    }

    #[tokio::test]
    async fn test_fallback_fts_only_false_returns_error_when_embed_unavailable() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let cfg = crate::config::EmbeddingConfig {
            ollama_url: "http://127.0.0.1:1".to_string(),
            model: "nomic-embed-text".to_string(),
            dimensions: 768,
            fallback_fts_only: false,
        };
        let engine = HybridSearchEngine::from_config(storage, &cfg);
        let err = engine
            .search_hybrid("pool", 5)
            .await
            .expect_err("fallback_fts_only=false 且 embedding 不可用时必须显式报错");
        assert!(err.to_string().contains("fallback_fts_only=false"));
    }

    #[tokio::test]
    async fn test_hybrid_search_omni_fuzzy_and_graph_offline() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        storage
            .insert_memory(&MemoryRecord {
                id: "mem-wal-1".to_string(),
                memory_type: "decision".to_string(),
                content: "Checkpointer spike causes WAL bloat on OLTP writes".to_string(),
                summary: Some("Checkpoint WAL bloat diagnosis".to_string()),
                source_agent: Some("test".to_string()),
                source_session: None,
                tags: vec!["postgresql".to_string(), "wal".to_string()],
                weight: 0.9,
                pinned: true,
                status: "active".to_string(),
                embedding: vec![],
            })
            .expect("insert");

        let cfg = crate::config::KnowledgeGraphConfig {
            pmi_threshold: 0.0,
            ..Default::default()
        };
        crate::fact::KnowledgeGraphBuilder::mine_from_storage(&storage, &cfg).expect("mine kg");

        let engine = HybridSearchEngine::new(storage);
        // 即使查询词有轻度拼写误差 Checkpont，Fuzzy + Graph 仍应离线召回
        let results = engine.search_hybrid("Checkpont", 5).await.expect("omni mem search");
        assert!(!results.is_empty(), "Fuzzy + Know-How 图应支持无向量拼写容错召回");
        assert_eq!(results[0].memory.id, "mem-wal-1");
    }

    #[tokio::test]
    async fn test_hybrid_search_with_gate_bypasses_on_unpredictable_query() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        storage
            .insert_memory(&MemoryRecord {
                id: "sleep-sum-pg".to_string(),
                memory_type: "experience".to_string(),
                content: "[sleep_summary:pg] primary port=5432 max_wal_size=2GB".to_string(),
                summary: Some("Sleep compacted PG summary".to_string()),
                source_agent: Some("sleep_time_compactor".to_string()),
                source_session: None,
                tags: vec!["sleep_compacted".to_string(), "sleep_summary".to_string()],
                weight: 0.85,
                pinned: false,
                status: "active".to_string(),
                embedding: vec![],
            })
            .expect("insert summary");
        storage
            .insert_memory(&MemoryRecord {
                id: "raw-orig-1".to_string(),
                memory_type: "experience".to_string(),
                content: "Root trace in PgTxEngine::search_raw_bypass hit SQLSTATE 40001 during burst".to_string(),
                summary: None,
                source_agent: Some("hook".to_string()),
                source_session: None,
                tags: vec!["pg".to_string()],
                weight: 0.5,
                pinned: false,
                status: "archived".to_string(),
                embedding: vec![],
            })
            .expect("insert archived original");

        let engine = HybridSearchEngine::new(storage);
        let (norm_hits, norm_dec) = engine.search_hybrid_with_gate("pg port 5432", 3).await.expect("norm");
        assert!(!norm_dec.bypass_compacted);
        assert_eq!(norm_hits[0].memory.id, "sleep-sum-pg");

        let (rare_hits, rare_dec) = engine
            .search_hybrid_with_gate("PgTxEngine::search_raw_bypass SQLSTATE 40001", 3)
            .await
            .expect("bypass");
        assert!(rare_dec.bypass_compacted);
        assert!(rare_hits.iter().any(|h| h.memory.id == "raw-orig-1"));
    }
}
