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
}

impl HybridSearchEngine {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self {
            storage,
            ollama_endpoint: "http://127.0.0.1:11434".to_string(),
            model_name: "bge-m3:latest".to_string(),
        }
    }

    /// 尝试调用本地 Ollama 获取文本的 1024 维 embedding 向量
    pub async fn fetch_embedding(&self, text: &str) -> Option<Vec<f32>> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(1500))
            .build()
            .ok()?;

        let body = serde_json::json!({
            "model": self.model_name,
            "prompt": text,
        });

        let resp = client.post(format!("{}/api/embeddings", self.ollama_endpoint))
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

    /// 混合检索核心方法：FTS5 + 向量余弦相似度 + RRF 融合重排
    pub async fn search_hybrid(&self, query: &str, limit: usize) -> Result<Vec<HybridSearchResult>> {
        // 1. 获取 FTS5 关键词排序列表
        let fts_results = self.storage.search_memories_fts(query, limit * 2).unwrap_or_default();
        let mut fts_ranks: HashMap<String, usize> = HashMap::new();
        for (idx, m) in fts_results.iter().enumerate() {
            fts_ranks.insert(m.id.clone(), idx + 1);
        }

        // 2. 尝试获取 Query 的向量进行语义计算
        let query_vec = self.fetch_embedding(query).await;
        let mut vec_ranks: HashMap<String, usize> = HashMap::new();
        let mut cos_sims: HashMap<String, f32> = HashMap::new();

        if let Some(ref q_emb) = query_vec {
            // 获取数据库中所有带 embedding 的活跃记忆
            let all_memories = self.storage.list_all_active_memories(500).unwrap_or_default();
            let mut scored_memories: Vec<(String, f32)> = Vec::new();

            for m in all_memories {
                if !m.embedding.is_empty() && m.embedding.len() == q_emb.len() {
                    let sim = cosine_similarity(q_emb, &m.embedding);
                    scored_memories.push((m.id, sim));
                }
            }

            // 按余弦相似度倒序排序
            scored_memories.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            for (idx, (m_id, sim)) in scored_memories.into_iter().take(limit * 2).enumerate() {
                vec_ranks.insert(m_id.clone(), idx + 1);
                cos_sims.insert(m_id, sim);
            }
        }

        // 3. 计算 RRF 得分: RRF_score = 1/(60 + rank_fts) + 1/(60 + rank_vec)
        let mut candidate_ids: Vec<String> = fts_ranks.keys().chain(vec_ranks.keys()).cloned().collect();
        candidate_ids.sort();
        candidate_ids.dedup();

        let mut rrf_scored: Vec<(String, f32, Option<usize>, Option<usize>, Option<f32>)> = Vec::new();
        let k = 60.0f32;

        for id in candidate_ids {
            let fts_r = fts_ranks.get(&id).copied();
            let vec_r = vec_ranks.get(&id).copied();
            let sim = cos_sims.get(&id).copied();

            let score_fts = fts_r.map(|r| 1.0 / (k + r as f32)).unwrap_or(0.0);
            let score_vec = vec_r.map(|r| 1.0 / (k + r as f32)).unwrap_or(0.0);
            let total_rrf = score_fts + score_vec;

            rrf_scored.push((id, total_rrf, fts_r, vec_r, sim));
        }

        // 4. 按 RRF 评分排序并组织结果返回
        rrf_scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let mut final_results = Vec::new();
        let all_memories_map: HashMap<String, MemoryRecord> = self.storage
            .list_all_active_memories(500)?
            .into_iter()
            .map(|m| (m.id.clone(), m))
            .collect();

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
}
