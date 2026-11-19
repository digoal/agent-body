use std::path::Path;
use std::time::{Duration, Instant};
use anyhow::{anyhow, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use crate::config::AnnotationConfig;
use crate::scanner::metadata::{is_sensitive_file, read_head_utf8, SecretRedactor};
use crate::storage::models::AssetRecord;
use crate::storage::Storage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedSample {
    pub path: String,
    pub domain: String,
    pub purpose: String,
    pub capabilities: Vec<String>,
    pub entry_point: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnrichReport {
    pub total_candidates: usize,
    pub enriched_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    pub ollama_available: bool,
    pub model_used: String,
    pub elapsed_ms: u128,
    pub samples: Vec<EnrichedSample>,
}

/// L2 本地 Ollama 小模型异步语义标注引擎 (ISSUE #8)
pub struct OllamaAnnotator {
    storage: Storage,
    config: AnnotationConfig,
}

impl OllamaAnnotator {
    pub fn new(storage: Storage, config: AnnotationConfig) -> Self {
        Self { storage, config }
    }

    pub fn with_default_config(storage: Storage) -> Self {
        let cfg = crate::config::Config::load_or_default(None)
            .map(|c| c.annotation)
            .unwrap_or_default();
        Self::new(storage, cfg)
    }

    /// 探活本地 Ollama 服务
    pub fn is_ollama_available(&self) -> bool {
        if !self.config.enabled {
            return false;
        }
        let tags_url = format!("{}/api/tags", self.config.ollama_url.trim_end_matches('/'));
        let Ok(client) = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(1200))
            .build()
        else {
            return false;
        };
        match client.get(&tags_url).send() {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    /// 按重要性优先级批量精炼待标注资产（不阻塞主扫描路径）
    pub fn enrich_pending_assets(
        &self,
        limit: usize,
        path_prefix: Option<&str>,
    ) -> Result<EnrichReport> {
        let started = Instant::now();
        let mut report = EnrichReport {
            model_used: self.config.model.clone(),
            ..Default::default()
        };

        if !self.config.enabled || limit == 0 {
            report.elapsed_ms = started.elapsed().as_millis();
            return Ok(report);
        }

        let all_assets = self.storage.list_assets(3000)?;
        let mut candidates: Vec<AssetRecord> = all_assets
            .into_iter()
            .filter(|a| {
                if a.scan_tier != "deep" || a.status != "active" {
                    return false;
                }
                if let Some(prefix) = path_prefix {
                    if !a.path.starts_with(prefix) {
                        return false;
                    }
                }
                if is_sensitive_file(Path::new(&a.path)) || a.security_level == "secret" {
                    return false;
                }
                let status = a
                    .desc_kv
                    .get("annotation_status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("pending");
                !matches!(status, "ollama_enriched" | "manual" | "skipped_sensitive")
            })
            .collect();

        // 按 importance_score 降序排列，高价值核心资产优先精炼
        candidates.sort_by(|a, b| {
            let sa = a.desc_kv.get("importance_score").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let sb = b.desc_kv.get("importance_score").and_then(|v| v.as_f64()).unwrap_or(0.5);
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });

        report.total_candidates = candidates.len();
        if candidates.is_empty() {
            report.ollama_available = self.is_ollama_available();
            report.elapsed_ms = started.elapsed().as_millis();
            return Ok(report);
        }

        if !self.is_ollama_available() {
            report.ollama_available = false;
            report.skipped_count = candidates.len().min(limit);
            report.elapsed_ms = started.elapsed().as_millis();
            return Ok(report);
        }
        report.ollama_available = true;

        for asset in candidates.into_iter().take(limit) {
            match self.enrich_single_asset(&asset) {
                Ok(Some(sample)) => {
                    report.enriched_count += 1;
                    if report.samples.len() < 10 {
                        report.samples.push(sample);
                    }
                }
                Ok(None) => {
                    report.skipped_count += 1;
                }
                Err(_) => {
                    report.failed_count += 1;
                }
            }
        }

        report.elapsed_ms = started.elapsed().as_millis();
        Ok(report)
    }

    /// 对单个资产调用本地 Ollama 小模型生成结构化语义 KV 并回写
    pub fn enrich_single_asset(&self, asset: &AssetRecord) -> Result<Option<EnrichedSample>> {
        let path = Path::new(&asset.path);
        if !path.exists() || !path.is_file() || is_sensitive_file(path) || asset.scan_tier == "shallow" {
            return Ok(None);
        }

        let raw_head = match read_head_utf8(path, self.config.max_snippet_chars * 2) {
            Some(s) if !s.trim().is_empty() => s,
            _ => return Ok(None),
        };
        let snippet: String = raw_head.chars().take(self.config.max_snippet_chars).collect();
        let redacted = SecretRedactor::redact(&snippet);

        let domain_hint = asset
            .desc_kv
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or("workspace_general");

        let static_anchors = asset
            .desc_kv
            .get("symbols")
            .or_else(|| asset.desc_kv.get("headings"))
            .or_else(|| asset.desc_kv.get("config_sections"))
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str())
                    .take(6)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();

        let prompt = build_grounding_prompt(&asset.path, &asset.asset_type, domain_hint, &static_anchors, &redacted);
        let generated = self.call_ollama_json(&prompt)?;

        let purpose = generated
            .get("purpose")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| anyhow!("Ollama 返回的 purpose 为空"))?;

        let domain = generated
            .get("domain")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| domain_hint.to_string());

        let capabilities: Vec<String> = generated
            .get("capabilities")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                    .filter(|s| !s.is_empty())
                    .take(4)
                    .collect()
            })
            .unwrap_or_default();

        let entry_point = generated
            .get("entry_point")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let io_contract = generated
            .get("io_contract")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();

        let usage_hint = generated
            .get("usage_hint")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let mut kv_map: Map<String, Value> = asset.desc_kv.as_object().cloned().unwrap_or_default();
        kv_map.insert("purpose".to_string(), json!(purpose));
        kv_map.insert("domain".to_string(), json!(domain));
        if !capabilities.is_empty() {
            kv_map.insert("capabilities".to_string(), json!(capabilities));
        }
        if !entry_point.is_empty() {
            kv_map.insert("entry_point".to_string(), json!(entry_point));
        }
        if !io_contract.is_empty() {
            kv_map.insert("io_contract".to_string(), json!(io_contract));
        }
        if let Some(ref u) = usage_hint {
            kv_map.insert("usage_hint".to_string(), json!(u));
        }
        kv_map.insert("annotation_status".to_string(), json!("ollama_enriched"));
        kv_map.insert("annotation_model".to_string(), json!(self.config.model));
        kv_map.insert("annotated_at".to_string(), json!(Local::now().to_rfc3339()));

        let mut updated = asset.clone();
        updated.description = Some(format!("[{}] {}", domain, purpose));
        if usage_hint.is_some() {
            updated.usage = usage_hint;
        }
        updated.desc_kv = Value::Object(kv_map);
        self.storage.upsert_asset(&updated)?;

        Ok(Some(EnrichedSample {
            path: updated.path,
            domain,
            purpose,
            capabilities,
            entry_point,
        }))
    }

    fn call_ollama_json(&self, prompt: &str) -> Result<Value> {
        let url = format!("{}/api/generate", self.config.ollama_url.trim_end_matches('/'));
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(self.config.timeout_ms))
            .build()?;

        let body = json!({
            "model": self.config.model,
            "prompt": prompt,
            "stream": false,
            "think": false,
            "options": {
                "temperature": 0.0,
                "num_predict": 256
            },
            "format": {
                "type": "object",
                "properties": {
                    "purpose": { "type": "string" },
                    "domain": { "type": "string" },
                    "capabilities": { "type": "array", "items": { "type": "string" } },
                    "entry_point": { "type": "string" },
                    "io_contract": { "type": "string" },
                    "usage_hint": { "type": "string" }
                },
                "required": ["purpose", "domain", "capabilities", "entry_point", "io_contract", "usage_hint"]
            }
        });

        let resp = client.post(&url).json(&body).send()?;
        if !resp.status().is_success() {
            return Err(anyhow!("Ollama HTTP 状态异常: {}", resp.status()));
        }

        let resp_json: Value = resp.json()?;
        let raw_text = resp_json
            .get("response")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Ollama 响应缺少 response 字段"))?;

        let parsed: Value = serde_json::from_str(raw_text)?;
        Ok(parsed)
    }
}

/// 构造数据导向、带 L1 静态符号锚定、防止小模型复读元指令的 Prompt
pub fn build_grounding_prompt(
    path: &str,
    asset_type: &str,
    domain_hint: &str,
    static_anchors: &str,
    redacted_snippet: &str,
) -> String {
    format!(
        "[文件信息]\npath: {path}\ntype: {asset_type}\ndomain_hint: {domain_hint}\nstatic_anchors: {static_anchors}\ncontent_head:\n{redacted_snippet}\n\n[问题]\n请用简短中文总结上面文件自身的功能与用途（每项不超过25字，capabilities最多3项，entry_point填核心入口符号或章节名）：",
        path = path,
        asset_type = asset_type,
        domain_hint = domain_hint,
        static_anchors = static_anchors,
        redacted_snippet = redacted_snippet,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_grounding_prompt_no_meta_leak() {
        let prompt = build_grounding_prompt(
            "/root/new/work/demo.rs",
            "script",
            "lock_coordination",
            "DeadlockDetector, detect_cycles",
            "pub struct DeadlockDetector;",
        );
        assert!(prompt.contains("[文件信息]"));
        assert!(prompt.contains("DeadlockDetector"));
        assert!(!prompt.contains("元数据标注器"), "禁止使用易导致 0.8B 复读的元角色指令");
    }

    #[test]
    fn test_enrich_graceful_fallback_when_ollama_unreachable() {
        let storage = Storage::open_in_memory().expect("in_memory");
        let tmp = tempfile::tempdir().expect("tempdir");
        let file_path = tmp.path().join("sample.py");
        std::fs::write(&file_path, "# sample python tool\ndef run():\n    pass\n").expect("write");

        storage.upsert_asset(&AssetRecord {
            id: "a-test-1".to_string(),
            path: file_path.to_string_lossy().to_string(),
            asset_type: "script".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("sample".to_string()),
            desc_kv: json!({"annotation_status": "heuristic_only", "importance_score": 0.9}),
            usage: None,
            size_bytes: Some(40),
            file_count: None,
            tags: vec!["python".to_string()],
            status: "active".to_string(),
        }).expect("upsert");

        let cfg = AnnotationConfig {
            enabled: true,
            ollama_url: "http://127.0.0.1:19999".to_string(), // 不可达端口
            model: "qwen3.5:0.8b".to_string(),
            timeout_ms: 800,
            max_snippet_chars: 1200,
            auto_enrich_batch_size: 5,
        };
        let annotator = OllamaAnnotator::new(storage, cfg);
        let report = annotator.enrich_pending_assets(5, None).expect("should gracefully degrade");
        assert!(!report.ollama_available);
        assert_eq!(report.enriched_count, 0);
        assert_eq!(report.total_candidates, 1);
    }
}
