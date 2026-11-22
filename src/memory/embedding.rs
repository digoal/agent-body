use std::time::Duration;
use chrono::Local;
use serde_json::{json, Map, Value};
use crate::config::{Config, EmbeddingConfig};

fn run_blocking_safe<F, T>(f: F) -> Option<T>
where
    F: FnOnce() -> T + Send,
    T: Send,
{
    if tokio::runtime::Handle::try_current().is_ok() {
        std::thread::scope(|s| s.spawn(f).join().ok())
    } else {
        Some(f())
    }
}

/// 统一本地 Ollama Embedding 客户端与标准化向量摘要生成器 (Phase 29)
#[derive(Debug, Clone)]
pub struct EmbeddingClient {
    ollama_url: String,
    model: String,
    expected_dim: usize,
}

impl Default for EmbeddingClient {
    fn default() -> Self {
        Self::from_global_config()
    }
}

impl EmbeddingClient {
    pub fn from_config(cfg: &EmbeddingConfig) -> Self {
        Self {
            ollama_url: cfg.ollama_url.trim_end_matches('/').to_string(),
            model: cfg.model.clone(),
            expected_dim: cfg.dimensions,
        }
    }

    pub fn from_global_config() -> Self {
        let cfg = Config::load_or_default(None)
            .map(|c| c.embedding)
            .unwrap_or_default();
        Self::from_config(&cfg)
    }

    pub fn with_endpoint(ollama_url: &str, model: &str, expected_dim: usize) -> Self {
        Self {
            ollama_url: ollama_url.trim_end_matches('/').to_string(),
            model: model.to_string(),
            expected_dim,
        }
    }

    pub fn model_name(&self) -> &str {
        &self.model
    }

    pub fn expected_dimensions(&self) -> usize {
        self.expected_dim
    }

    /// 快速探活本地 Ollama 服务与模型列表
    pub fn is_available(&self) -> bool {
        let tags_url = format!("{}/api/tags", self.ollama_url);
        run_blocking_safe(|| {
            let Ok(client) = reqwest::blocking::Client::builder()
                .timeout(Duration::from_millis(800))
                .build()
            else {
                return false;
            };
            match client.get(&tags_url).send() {
                Ok(resp) => resp.status().is_success(),
                Err(_) => false,
            }
        })
        .unwrap_or(false)
    }

    /// 同步调用本地 Ollama `/api/embeddings` 生成文本向量
    pub fn embed_text(&self, text: &str) -> Option<Vec<f32>> {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::NetOllamaEmbed);
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        let truncated: String = trimmed.chars().take(2000).collect();
        let endpoint = format!("{}/api/embeddings", self.ollama_url);
        let model = self.model.clone();

        run_blocking_safe(move || {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_millis(5000))
                .build()
                .ok()?;

            let body = json!({
                "model": model,
                "prompt": truncated,
            });

            let resp = client.post(endpoint).json(&body).send().ok()?;
            if !resp.status().is_success() {
                return None;
            }

            let res_json: Value = resp.json().ok()?;
            let arr = res_json.get("embedding")?.as_array()?;
            let vec: Vec<f32> = arr
                .iter()
                .filter_map(|x| x.as_f64().map(|f| f as f32))
                .collect();
            if vec.is_empty() {
                None
            } else {
                Some(vec)
            }
        })
        .flatten()
    }
}

/// 计算向量的 L2 范数
pub fn vector_l2_norm(vec: &[f32]) -> f32 {
    vec.iter().map(|x| x * x).sum::<f32>().sqrt()
}

/// 将标准 Embedding 摘要属性注入 JSON Object（避免 768 维裸数组撑爆 UI/Context）
pub fn inject_embedding_meta(obj: &mut Map<String, Value>, model: &str, vec: &[f32]) {
    let norm = ((vector_l2_norm(vec) as f64) * 10000.0).round() / 10000.0;
    let preview: Vec<f64> = vec
        .iter()
        .take(8)
        .map(|&v| ((v as f64) * 10000.0).round() / 10000.0)
        .collect();

    obj.insert("embedding_status".to_string(), json!("ready"));
    obj.insert("embedding_model".to_string(), json!(model));
    obj.insert("embedding_dim".to_string(), json!(vec.len()));
    obj.insert("embedding_norm".to_string(), json!(norm));
    obj.insert("embedding_preview".to_string(), json!(preview));
    obj.insert("embedding_updated_at".to_string(), json!(Local::now().to_rfc3339()));
}

/// 标记待向量化状态（当 Ollama 离线或尚未批量计算时）
pub fn inject_pending_embedding_meta(obj: &mut Map<String, Value>, model: &str, expected_dim: usize) {
    if !obj.contains_key("embedding_status") {
        obj.insert("embedding_status".to_string(), json!("pending_ollama"));
    }
    if !obj.contains_key("embedding_model") {
        obj.insert("embedding_model".to_string(), json!(model));
    }
    if !obj.contains_key("embedding_dim") {
        obj.insert("embedding_dim".to_string(), json!(expected_dim));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inject_embedding_meta_format() {
        let mut map = Map::new();
        let vec = vec![0.123456, -0.654321, 0.5, 0.0, 0.1, 0.2, 0.3, 0.4, 0.9];
        inject_embedding_meta(&mut map, "nomic-embed-text", &vec);

        assert_eq!(map["embedding_status"], "ready");
        assert_eq!(map["embedding_model"], "nomic-embed-text");
        assert_eq!(map["embedding_dim"], 9);
        assert_eq!(map["embedding_preview"].as_array().unwrap().len(), 8);
        assert!(map["embedding_norm"].as_f64().unwrap() > 0.8);
    }

    #[test]
    fn test_embedding_client_graceful_fallback_when_unreachable() {
        let client = EmbeddingClient::with_endpoint("http://127.0.0.1:59998", "nomic-embed-text", 768);
        assert!(!client.is_available());
        assert!(client.embed_text("hello world").is_none());
    }

    #[tokio::test]
    async fn test_embedding_client_inside_tokio_runtime_does_not_panic() {
        let client = EmbeddingClient::with_endpoint("http://127.0.0.1:59998", "nomic-embed-text", 768);
        assert!(!client.is_available());
        assert!(client.embed_text("tokio safety check").is_none());
    }
}
