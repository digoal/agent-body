use std::path::Path;
use std::time::{Duration, Instant};
use anyhow::{anyhow, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use crate::config::AnnotationConfig;
use crate::scanner::metadata::{
    is_sensitive_file, read_head_utf8, PromptInjectionScanner, SecretRedactor,
};
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

        let all_assets = self.storage.list_assets(50_000)?;
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
                !matches!(
                    status,
                    "ollama_enriched" | "ollama_vision_enriched" | "manual" | "skipped_sensitive"
                )
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

    /// 按重要性优先级批量为 deep 级资产生成本地 Ollama Embedding 并写入摘要 KV
    pub fn embed_pending_assets(&self, limit: usize, path_prefix: Option<&str>) -> Result<usize> {
        if limit == 0 {
            return Ok(0);
        }
        let client = crate::memory::embedding::EmbeddingClient::default();
        if !client.is_available() {
            return Ok(0);
        }

        let all_assets = self.storage.list_assets(50_000)?;
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
                a.desc_kv
                    .get("embedding_status")
                    .and_then(|v| v.as_str())
                    != Some("ready")
            })
            .collect();

        candidates.sort_by(|a, b| {
            let sa = a.desc_kv.get("importance_score").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let sb = b.desc_kv.get("importance_score").and_then(|v| v.as_f64()).unwrap_or(0.5);
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut updated = 0usize;
        for mut asset in candidates.into_iter().take(limit) {
            let purpose = asset
                .desc_kv
                .get("purpose")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| asset.description.clone())
                .unwrap_or_default();
            let domain = asset
                .desc_kv
                .get("domain")
                .and_then(|v| v.as_str())
                .unwrap_or("workspace_general");
            let caps = asset
                .desc_kv
                .get("capabilities")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let modality = asset
                .desc_kv
                .get("modality")
                .and_then(|v| v.as_str())
                .unwrap_or(&asset.asset_type);
            let mm_context = build_multimodal_kv_summary(&asset.desc_kv);
            let text = if mm_context.is_empty() {
                format!(
                    "[{}|{}|{}] {} | {} | {}",
                    asset.asset_type, modality, domain, asset.path, purpose, caps
                )
            } else {
                format!(
                    "[{}|{}|{}] {} | {} | {} | {}",
                    asset.asset_type, modality, domain, asset.path, purpose, caps, mm_context
                )
            };
            if let Some(vec) = client.embed_text(&text) {
                self.storage.update_asset_embedding(&asset.path, &vec)?;
                if let Some(obj) = asset.desc_kv.as_object_mut() {
                    crate::memory::embedding::inject_embedding_meta(
                        obj,
                        client.model_name(),
                        &vec,
                    );
                }
                self.storage.upsert_asset(&asset)?;
                updated += 1;
            }
        }
        Ok(updated)
    }

    /// 对单个资产调用本地 Ollama 小模型（含可选多模态与 Vision 图片增强）生成结构化语义 KV 并回写
    pub fn enrich_single_asset(&self, asset: &AssetRecord) -> Result<Option<EnrichedSample>> {
        let _wait = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::NetOllamaEnrich);
        let path = Path::new(&asset.path);
        if !path.exists() || !path.is_file() || is_sensitive_file(path) || asset.scan_tier == "shallow" {
            return Ok(None);
        }

        let modality = asset
            .desc_kv
            .get("modality")
            .and_then(|v| v.as_str())
            .unwrap_or(&asset.asset_type);
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let domain_hint = asset
            .desc_kv
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or("workspace_general");

        // 路线 A: 图片资产 + 开启 Ollama Vision -> Base64 视觉理解增强
        let is_raster_image = modality == "image"
            && matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp");
        if is_raster_image && self.config.vision_enabled {
            let meta_len = std::fs::metadata(path).map(|m| m.len() as usize).unwrap_or(0);
            if meta_len > 0 && meta_len <= self.config.vision_max_image_bytes {
                if let Ok(bytes) = std::fs::read(path) {
                    let img_b64 = crate::scanner::multimodal::base64_encode(&bytes);
                    let dims = asset
                        .desc_kv
                        .get("dimensions")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");
                    let vprompt = format!(
                        "[图片资产信息]\npath: {}\ndimensions: {}\ndomain_hint: {}\n\n[任务]\n请用简短中文概括图片核心内容（visual_summary <=40字）、图中可见文字或实体关键词（ocr_keywords 最多5项）、purpose、domain、capabilities：",
                        asset.path, dims, domain_hint
                    );
                    let generated = self.call_ollama_vision_json(&vprompt, &img_b64)?;
                    let raw_visual_summary = generated
                        .get("visual_summary")
                        .or_else(|| generated.get("purpose"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !is_placeholder_value(s))
                        .ok_or_else(|| anyhow!("Ollama Vision 返回的 visual_summary/purpose 为空"))?;
                    let visual_summary =
                        PromptInjectionScanner::scan_and_sanitize(&raw_visual_summary).sanitized;
                    let raw_purpose = generated
                        .get("purpose")
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !is_placeholder_value(s))
                        .unwrap_or_else(|| visual_summary.clone());
                    let purpose = PromptInjectionScanner::scan_and_sanitize(&raw_purpose).sanitized;
                    let domain = generated
                        .get("domain")
                        .and_then(|v| v.as_str())
                        .map(|s| s.trim().to_string())
                        .filter(|s| !is_placeholder_value(s))
                        .unwrap_or_else(|| domain_hint.to_string());
                    let ocr_keywords: Vec<String> = generated
                        .get("ocr_keywords")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(|s| s.trim().to_string()))
                                .filter(|s| !is_placeholder_value(s))
                                .map(|s| PromptInjectionScanner::scan_and_sanitize(&s).sanitized)
                                .take(5)
                                .collect()
                        })
                        .unwrap_or_default();
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

                    let mut kv_map: Map<String, Value> = asset.desc_kv.as_object().cloned().unwrap_or_default();
                    kv_map.insert("visual_summary".to_string(), json!(visual_summary));
                    if !ocr_keywords.is_empty() {
                        kv_map.insert("ocr_keywords".to_string(), json!(ocr_keywords));
                    }
                    kv_map.insert("purpose".to_string(), json!(purpose));
                    kv_map.insert("domain".to_string(), json!(domain));
                    if !capabilities.is_empty() {
                        kv_map.insert("capabilities".to_string(), json!(capabilities));
                    }
                    kv_map.insert("annotation_status".to_string(), json!("ollama_vision_enriched"));
                    kv_map.insert("annotation_model".to_string(), json!(self.config.vision_model));
                    kv_map.insert("annotated_at".to_string(), json!(Local::now().to_rfc3339()));

                    let mut updated = asset.clone();
                    updated.description = Some(format!("[{}] {}", domain, purpose));
                    updated.desc_kv = Value::Object(kv_map);
                    self.storage.upsert_asset(&updated)?;

                    return Ok(Some(EnrichedSample {
                        path: asset.path.clone(),
                        purpose,
                        domain,
                        capabilities,
                        entry_point: "vision_image".to_string(),
                    }));
                }
            }
        }

        // 路线 B: 文本文件或具备 L1 多模态结构化上下文的二进制文件
        let raw_head = read_head_utf8(path, self.config.max_snippet_chars * 2).unwrap_or_default();
        let is_clean_text = !raw_head.trim().is_empty() && !raw_head.contains('\u{FFFD}');
        let mm_summary = build_multimodal_kv_summary(&asset.desc_kv);
        let redacted = if is_clean_text {
            let snippet: String = raw_head.chars().take(self.config.max_snippet_chars).collect();
            SecretRedactor::redact(&snippet)
        } else if !mm_summary.is_empty() {
            format!("[multimodal_l1_summary modality={}] {}", modality, mm_summary)
        } else {
            return Ok(None);
        };

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

    fn call_ollama_vision_json(&self, prompt: &str, image_base64: &str) -> Result<Value> {
        let url = format!("{}/api/generate", self.config.ollama_url.trim_end_matches('/'));
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_millis(self.config.timeout_ms.max(20_000)))
            .build()?;

        let body = json!({
            "model": self.config.vision_model,
            "prompt": prompt,
            "images": [image_base64],
            "stream": false,
            "think": false,
            "options": {
                "temperature": 0.0,
                "num_predict": 256
            },
            "format": {
                "type": "object",
                "properties": {
                    "visual_summary": { "type": "string" },
                    "ocr_keywords": { "type": "array", "items": { "type": "string" } },
                    "purpose": { "type": "string" },
                    "domain": { "type": "string" },
                    "capabilities": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["visual_summary", "purpose", "domain"]
            }
        });

        let resp = client.post(&url).json(&body).send()?;
        if !resp.status().is_success() {
            return Err(anyhow!("Ollama Vision HTTP 状态异常: {}", resp.status()));
        }
        let resp_json: Value = resp.json()?;
        let raw_text = resp_json
            .get("response")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Ollama Vision 响应缺少 response 字段"))?;
        let parsed: Value = serde_json::from_str(raw_text)?;
        Ok(parsed)
    }
}

fn is_placeholder_value(s: &str) -> bool {
    let t = s.trim();
    t.is_empty()
        || matches!(
            t.to_ascii_lowercase().as_str(),
            "none" | "n/a" | "na" | "null" | "unknown" | "-" | "无" | "未知" | "暂无"
        )
}

/// 抽取 L1/L2 多模态关键 KV 为紧凑语义文本（供 L2 摘要与 Embedding 共享）
pub fn build_multimodal_kv_summary(kv: &Value) -> String {
    let mut parts = Vec::new();
    let str_keys = [
        ("visual_summary", "visual"),
        ("dimensions", "dim"),
        ("pdf_title", "pdf_title"),
        ("pdf_excerpt", "pdf_excerpt"),
        ("doc_title", "doc_title"),
        ("codec", "codec"),
    ];
    for (k, label) in str_keys {
        if let Some(s) = kv.get(k).and_then(|v| v.as_str()) {
            if !s.trim().is_empty() {
                parts.push(format!("{}:{}", label, s.trim()));
            }
        }
    }
    let arr_keys = [
        ("ocr_keywords", "ocr"),
        ("svg_text_snippets", "svg_text"),
        ("table_names", "tables"),
        ("columns", "cols"),
    ];
    for (k, label) in arr_keys {
        if let Some(arr) = kv.get(k).and_then(|v| v.as_array()) {
            let items: Vec<_> = arr.iter().filter_map(|x| x.as_str()).take(8).collect();
            if !items.is_empty() {
                parts.push(format!("{}:[{}]", label, items.join(", ")));
            }
        }
    }
    if let Some(pg) = kv.get("page_count").and_then(|v| v.as_u64()) {
        parts.push(format!("pages:{}", pg));
    }
    if let Some(dur) = kv.get("duration_secs").and_then(|v| v.as_f64()) {
        parts.push(format!("duration:{:.1}s", dur));
    }
    parts.join(" | ")
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
            ..AnnotationConfig::default()
        };
        let annotator = OllamaAnnotator::new(storage, cfg);
        let report = annotator.enrich_pending_assets(5, None).expect("should gracefully degrade");
        assert!(!report.ollama_available);
        assert_eq!(report.enriched_count, 0);
        assert_eq!(report.total_candidates, 1);
    }

    #[test]
    fn test_multimodal_kv_summary_and_vision_safe_fallback() {
        let kv = json!({
            "modality": "image",
            "dimensions": "1280x720",
            "visual_summary": "架构拓扑",
            "ocr_keywords": ["SQLite", "DuckDB"],
            "table_names": ["orders"]
        });
        let summary = build_multimodal_kv_summary(&kv);
        assert!(summary.contains("dim:1280x720"));
        assert!(summary.contains("visual:架构拓扑"));
        assert!(summary.contains("ocr:[SQLite, DuckDB]"));
        assert!(summary.contains("tables:[orders]"));

        // 二进制 PNG 无有效 L1 KV 且 vision_enabled=false 时应安全返回 Ok(None) 不报错
        let dir = tempfile::tempdir().expect("tempdir");
        let png_path = dir.path().join("bin.png");
        std::fs::write(&png_path, b"\x89PNG\r\n\x1a\n\x00\x00\x00").expect("write png");
        let storage = Storage::open_in_memory().expect("mem");
        let mut cfg = AnnotationConfig::default();
        cfg.vision_enabled = false;
        let annotator = OllamaAnnotator::new(storage, cfg);
        let asset = AssetRecord {
            id: uuid::Uuid::new_v4().to_string(),
            path: png_path.to_string_lossy().to_string(),
            asset_type: "media".to_string(),
            owner: "user:tester".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: None,
            desc_kv: json!({"modality": "image"}),
            usage: None,
            size_bytes: Some(11),
            file_count: None,
            tags: vec![],
            status: "active".to_string(),
        };
        let res = annotator.enrich_single_asset(&asset).expect("no panic on binary png");
        assert!(res.is_none());

        assert!(is_placeholder_value("无"));
        assert!(is_placeholder_value(" N/A "));
        assert!(!is_placeholder_value("架构拓扑图"));
        assert_eq!(AnnotationConfig::default().vision_model, "qwen3.5:0.8b");
    }
}
