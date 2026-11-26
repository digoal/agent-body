use std::collections::HashSet;
use std::path::Path;
use std::time::{Duration, Instant};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::{AnnotationConfig, KnowledgeGraphConfig};
use crate::fact::knowledge_graph::{
    file_anchor_id, sha256_prefix_hex, term_node_id, AliasCanonicalizer, HeadingChunker,
};
use crate::scanner::metadata::{is_sensitive_file, PromptInjectionScanner, SecretRedactor};
use crate::storage::models::{FactEdgeRecord, FactNodeRecord};
use crate::storage::{Storage, StorageProvider};

/// 单条原子 Know-How 因果/对比/适用边界三元组
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowHowTriple {
    pub subject: String,
    pub relation: String,
    pub object: String,
    #[serde(default)]
    pub boundary: String,
    #[serde(default)]
    pub evidence_quote: String,
}

/// L2 小模型 Know-How 抽取流水线执行报告
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KnowHowExtractionReport {
    pub sections_scanned: usize,
    pub triples_extracted: usize,
    pub triples_grounded: usize,
    pub hallucinations_rejected: usize,
    pub knowhow_nodes_created: usize,
    pub edges_created: usize,
    pub ollama_available: bool,
    pub model_used: String,
    pub elapsed_ms: u128,
}

fn normalize_for_grounding(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_ascii_lowercase()
}

/// 确定性防幻觉原文锚点校验门禁：
/// `evidence_quote` 必须在原切片（空白折叠小写化）中命中连续子串（>=8 字符），或关键技术词均存在于原切片中。
pub fn verify_grounding_quote(chunk_text: &str, evidence_quote: &str) -> bool {
    let norm_chunk = normalize_for_grounding(chunk_text);
    let norm_quote = normalize_for_grounding(evidence_quote);
    if norm_chunk.is_empty() || norm_quote.chars().count() < 6 {
        return false;
    }
    if norm_quote.chars().count() >= 8 && norm_chunk.contains(&norm_quote) {
        return true;
    }

    let tokens: Vec<String> = norm_quote
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && (c as u32) <= 0x7F)
        .filter(|t| t.chars().count() >= 2)
        .filter(|t| {
            !matches!(
                *t,
                "the" | "and" | "for" | "with" | "from" | "that" | "this" | "into" | "when" | "的" | "和" | "与" | "在"
            )
        })
        .map(str::to_string)
        .collect();

    if tokens.len() >= 2 {
        return tokens.iter().all(|t| norm_chunk.contains(t.as_str()));
    }
    if let Some(only) = tokens.first() {
        return only.len() >= 6
            && (only.contains('_') || only.contains('-'))
            && norm_chunk.contains(only.as_str());
    }
    false
}

fn normalize_knowhow_relation(raw: &str) -> Option<(&'static str, f32, &'static str)> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "causes" | "leads_to" | "triggers" => Some(("causes", 0.85, "causal_mechanism")),
        "mitigates" | "solves" | "reduces" => Some(("mitigates", 0.88, "remediation_action")),
        "depends_on" | "requires" => Some(("depends_on", 0.80, "prerequisite_rule")),
        "contrasts_with" | "tradeoff" | "vs" => {
            Some(("contrasts_with", 0.72, "tradeoff_comparison"))
        }
        "applies_when" | "valid_when" => {
            Some(("applies_when", 0.82, "applicability_boundary"))
        }
        "supersedes" | "replaces" => Some(("supersedes", 0.95, "knowledge_evolution")),
        _ => None,
    }
}

/// 面向本地小模型（`qwen3.5:0.8b`, `think: false`）的章节级 L2 Know-How 抽取器与防幻觉入库门禁
pub struct KnowHowExtractor;

impl KnowHowExtractor {
    /// 纯离线可测入口：对单个章节产生的候选三元组执行原文锚点校验与 `fact_nodes`/`fact_edges` 写入
    #[allow(clippy::too_many_arguments)]
    pub fn ingest_triples_for_section(
        storage: &dyn StorageProvider,
        section_id: &str,
        source_path: &str,
        section_heading: &str,
        chunk_text: &str,
        content_hash: Option<&str>,
        extractor_model: &str,
        triples: &[KnowHowTriple],
        aliases: &AliasCanonicalizer,
    ) -> Result<(usize, usize, usize, usize)> {
        let mut grounded = 0usize;
        let mut rejected = 0usize;
        let mut kh_nodes = 0usize;
        let mut edges_written = 0usize;
        let mut seen_claims: HashSet<String> = HashSet::new();

        for triple in triples.iter().take(4) {
            let subj_raw = triple.subject.trim();
            let obj_raw = triple.object.trim();
            if subj_raw.is_empty() || obj_raw.is_empty() {
                rejected += 1;
                continue;
            }
            let Some((rel, weight, dir_kind)) = normalize_knowhow_relation(&triple.relation)
            else {
                rejected += 1;
                continue;
            };
            if rel == "supersedes" && !has_supersedes_signal(chunk_text) {
                rejected += 1;
                continue;
            }
            if !verify_grounding_quote(chunk_text, &triple.evidence_quote) {
                rejected += 1;
                continue;
            }

            let subj_canon = aliases.canonicalize(subj_raw);
            let obj_canon = aliases.canonicalize(obj_raw);
            if subj_canon == obj_canon || subj_canon.len() < 2 || obj_canon.len() < 2 {
                rejected += 1;
                continue;
            }

            let claim = format!("{} -> [{}] -> {}", subj_canon, rel, obj_canon);
            if !seen_claims.insert(claim.clone()) {
                continue;
            }
            grounded += 1;

            let sid = term_node_id(&subj_canon);
            let oid = term_node_id(&obj_canon);
            for (nid, term) in [(&sid, &subj_canon), (&oid, &obj_canon)] {
                if storage.get_fact_node(nid)?.is_none() {
                    storage.upsert_fact_node(&FactNodeRecord {
                        id: nid.clone(),
                        node_type: "domain_term".to_string(),
                        name: term.clone(),
                        label: term.clone(),
                        properties: json!({
                            "canonical_name": term,
                            "source_sample_paths": [source_path],
                        }),
                        updated_at: None,
                    })?;
                }
            }

            let kh_id = format!(
                "kh:{}",
                sha256_prefix_hex(&format!("{}|{}|{}", source_path, section_id, claim), 8)
            );
            let clean_quote: String = triple.evidence_quote.trim().chars().take(180).collect();
            let clean_boundary: String = triple.boundary.trim().chars().take(140).collect();

            storage.upsert_fact_node(&FactNodeRecord {
                id: kh_id.clone(),
                node_type: "knowhow".to_string(),
                name: claim.clone(),
                label: claim.clone(),
                properties: json!({
                    "claim": claim,
                    "boundary_condition": clean_boundary,
                    "evidence_quote": clean_quote,
                    "confidence": weight,
                    "source_path": source_path,
                    "section_heading": section_heading,
                    "content_hash": content_hash,
                    "extractor": extractor_model,
                }),
                updated_at: None,
            })?;
            kh_nodes += 1;

            // 术语间有向 Know-How 边
            storage.upsert_fact_edge(&FactEdgeRecord {
                id: None,
                source_id: sid.clone(),
                target_id: oid.clone(),
                relation: rel.to_string(),
                properties: json!({
                    "boundary": clean_boundary,
                    "evidence_quote": clean_quote,
                    "source_path": source_path,
                    "extractor": extractor_model,
                }),
                confidence: weight.max(0.75),
                evidence_tier: "L2_KNOWHOW".to_string(),
                weight,
                direction_kind: dir_kind.to_string(),
                updated_at: None,
            })?;
            edges_written += 1;

            // 仅当 section 节点已在库内时建立 section -> knowhow 归属边，避免外键约束失败
            if storage.get_fact_node(section_id)?.is_some() {
                storage.upsert_fact_edge(&FactEdgeRecord {
                    id: None,
                    source_id: section_id.to_string(),
                    target_id: kh_id.clone(),
                    relation: "contains".to_string(),
                    properties: json!({"source_path": source_path}),
                    confidence: 0.95,
                    evidence_tier: "L2_KNOWHOW".to_string(),
                    weight: 0.90,
                    direction_kind: "structural_containment".to_string(),
                    updated_at: None,
                })?;
                edges_written += 1;
            }

            // knowhow -> 概念词挂载
            for tid in [sid, oid] {
                storage.upsert_fact_edge(&FactEdgeRecord {
                    id: None,
                    source_id: kh_id.clone(),
                    target_id: tid,
                    relation: "mentions".to_string(),
                    properties: json!({"source_path": source_path}),
                    confidence: 0.92,
                    evidence_tier: "L2_KNOWHOW".to_string(),
                    weight: 0.85,
                    direction_kind: "concept_reference".to_string(),
                    updated_at: None,
                })?;
                edges_written += 1;
            }
        }

        Ok((grounded, rejected, kh_nodes, edges_written))
    }

    /// 连接本地 Ollama (`think: false` + JSON Schema) 从活跃高价值文档章节抽取 L2 Know-How 边并执行防幻觉门禁
    pub fn extract_from_storage(
        storage: &Storage,
        ann_cfg: &AnnotationConfig,
        kg_cfg: &KnowledgeGraphConfig,
        limit_docs: usize,
        path_prefix: Option<&str>,
    ) -> Result<KnowHowExtractionReport> {
        let started = Instant::now();
        let mut rep = KnowHowExtractionReport {
            model_used: ann_cfg.model.clone(),
            ..Default::default()
        };

        if !ann_cfg.enabled || limit_docs == 0 {
            rep.elapsed_ms = started.elapsed().as_millis();
            return Ok(rep);
        }
        if !is_ollama_online(&ann_cfg.ollama_url) {
            rep.ollama_available = false;
            rep.elapsed_ms = started.elapsed().as_millis();
            return Ok(rep);
        }
        rep.ollama_available = true;

        let aliases = AliasCanonicalizer::new(&kg_cfg.aliases);
        let mut assets: Vec<_> = storage
            .list_assets(5000)?
            .into_iter()
            .filter(|a| {
                a.status == "active"
                    && a.asset_type == "document"
                    && !is_sensitive_file(Path::new(&a.path))
                    && a.security_level != "secret"
                    && path_prefix.map(|p| a.path.starts_with(p)).unwrap_or(true)
            })
            .collect();
        assets.sort_by(|a, b| {
            let sa = a.desc_kv.get("importance_score").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let sb = b.desc_kv.get("importance_score").and_then(|v| v.as_f64()).unwrap_or(0.5);
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });

        for asset in assets.into_iter().take(limit_docs) {
            let p = Path::new(&asset.path);
            let ext = p
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if !matches!(ext.as_str(), "md" | "markdown" | "txt" | "rst" | "adoc") || !p.is_file() {
                continue;
            }
            let Ok(raw) = std::fs::read_to_string(p) else {
                continue;
            };
            let file_id = file_anchor_id(&asset.path);
            let content_hash = asset
                .desc_kv
                .get("content_hash")
                .and_then(|v| v.as_str())
                .map(str::to_string);

            // 确保文件锚点存在
            if storage.get_fact_node(&file_id)?.is_none() {
                let file_lbl = p.file_name().and_then(|s| s.to_str()).unwrap_or(&asset.path);
                storage.upsert_fact_node(&FactNodeRecord {
                    id: file_id.clone(),
                    node_type: "file".to_string(),
                    name: asset.path.clone(),
                    label: file_lbl.to_string(),
                    properties: json!({"source_path": asset.path}),
                    updated_at: None,
                })?;
            }

            let chunks = HeadingChunker::chunk_markdown(&asset.path, &raw, kg_cfg.max_section_chars);
            for chunk in chunks
                .into_iter()
                .filter(|c| c.text.chars().count() >= 60)
                .take(3)
            {
                // 确保 section 节点存在
                if storage.get_fact_node(&chunk.section_id)?.is_none() {
                    storage.upsert_fact_node(&FactNodeRecord {
                        id: chunk.section_id.clone(),
                        node_type: "section".to_string(),
                        name: chunk.heading_path.clone(),
                        label: chunk.heading_path.clone(),
                        properties: json!({
                            "source_path": asset.path,
                            "heading_path": chunk.heading_path,
                            "snippet_preview": chunk.preview,
                            "content_hash": content_hash,
                        }),
                        updated_at: None,
                    })?;
                    let parent = chunk.parent_section_id.clone().unwrap_or_else(|| file_id.clone());
                    if storage.get_fact_node(&parent)?.is_some() {
                        storage.upsert_fact_edge(&FactEdgeRecord {
                            id: None,
                            source_id: parent,
                            target_id: chunk.section_id.clone(),
                            relation: "contains".to_string(),
                            properties: json!({"source_path": asset.path}),
                            confidence: 1.0,
                            evidence_tier: "L1_KNOWLEDGE".to_string(),
                            weight: 0.90,
                            direction_kind: "structural_containment".to_string(),
                            updated_at: None,
                        })?;
                    }
                }

                let sanitized_text = PromptInjectionScanner::scan_and_sanitize(
                    &SecretRedactor::redact(&chunk.text),
                )
                .sanitized;
                let prompt = build_knowhow_prompt(&chunk.heading_path, &sanitized_text);
                rep.sections_scanned += 1;

                let Ok(triples) = call_ollama_knowhow(ann_cfg, &prompt) else {
                    continue;
                };
                rep.triples_extracted += triples.len();
                let (gr, rej, kh_cnt, e_cnt) = Self::ingest_triples_for_section(
                    storage.provider().as_ref(),
                    &chunk.section_id,
                    &asset.path,
                    &chunk.heading_path,
                    &sanitized_text,
                    content_hash.as_deref(),
                    &ann_cfg.model,
                    &triples,
                    &aliases,
                )?;
                rep.triples_grounded += gr;
                rep.hallucinations_rejected += rej;
                rep.knowhow_nodes_created += kh_cnt;
                rep.edges_created += e_cnt;
            }
        }

        rep.elapsed_ms = started.elapsed().as_millis();
        Ok(rep)
    }
}

fn is_ollama_online(ollama_url: &str) -> bool {
    let url = format!("{}/api/tags", ollama_url.trim_end_matches('/'));
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(1200))
        .build()
    else {
        return false;
    };
    client.get(&url).send().map(|r| r.status().is_success()).unwrap_or(false)
}

fn has_supersedes_signal(chunk_text: &str) -> bool {
    let lower = chunk_text.to_ascii_lowercase();
    [
        "取代",
        "替代",
        "废弃",
        "弃用",
        "不再使用",
        "supersedes",
        "deprecated",
        "replaces",
        "instead of",
        "replaced by",
    ]
    .iter()
    .any(|kw| lower.contains(kw))
}

fn build_knowhow_prompt(heading_path: &str, sanitized_text: &str) -> String {
    let snippet: String = sanitized_text.chars().take(1000).collect();
    format!(
        "Extract up to 3 concise domain knowledge triples (short concept names, not sentences) from Text.\n\
         Section: {heading_path}\n\
         Allowed relations:\n\
         - causes (X triggers/causes Y)\n\
         - mitigates (X solves/reduces Y)\n\
         - depends_on (X requires/depends on Y)\n\
         - contrasts_with (X tradeoff vs Y)\n\
         - applies_when (X valid under condition Y)\n\
         - supersedes (ONLY if text explicitly says X replaces deprecated Y)\n\
         Every triple MUST copy a verbatim substring (>=10 chars) from Text into evidence_quote.\n\
         Text:\n{snippet}\n\
         Return strict JSON matching {{\"triples\":[{{\"subject\":\"...\",\"relation\":\"...\",\"object\":\"...\",\"boundary\":\"...\",\"evidence_quote\":\"...\"}}]}}."
    )
}

fn call_ollama_knowhow(ann_cfg: &AnnotationConfig, prompt: &str) -> Result<Vec<KnowHowTriple>> {
    let url = format!("{}/api/generate", ann_cfg.ollama_url.trim_end_matches('/'));
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_millis(ann_cfg.timeout_ms.max(15_000)))
        .build()?;

    let body = json!({
        "model": ann_cfg.model,
        "prompt": prompt,
        "stream": false,
        "think": false,
        "options": {
            "temperature": 0.0,
            "num_predict": 384
        },
        "format": {
            "type": "object",
            "properties": {
                "triples": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "subject": { "type": "string" },
                            "relation": { "type": "string" },
                            "object": { "type": "string" },
                            "boundary": { "type": "string" },
                            "evidence_quote": { "type": "string" }
                        },
                        "required": ["subject", "relation", "object", "evidence_quote"]
                    }
                }
            },
            "required": ["triples"]
        }
    });

    let resp = client.post(&url).json(&body).send()?;
    if !resp.status().is_success() {
        return Err(anyhow!("Ollama Know-How HTTP 异常: {}", resp.status()));
    }
    let resp_json: Value = resp.json()?;
    let raw_text = resp_json
        .get("response")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("Ollama 缺少 response 字段"))?;
    let parsed: Value = serde_json::from_str(raw_text)?;
    let arr = parsed
        .get("triples")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut out = Vec::new();
    for item in arr {
        if let Ok(t) = serde_json::from_value::<KnowHowTriple>(item) {
            out.push(t);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KnowledgeGraphConfig;
    use crate::fact::knowledge_graph::{expand_graph_for_query, KnowledgeGraphBuilder};
    use crate::storage::models::AssetRecord;

    #[test]
    fn test_grounding_gate_blocks_hallucinations_and_accepts_anchored_quotes() {
        let chunk = "高频 full_page_writes 会引起 WAL 写放大，调大 max_wal_size 可在长事务负载下有效缓解尖刺。";
        assert!(verify_grounding_quote(chunk, "高频 full_page_writes 会引起 WAL 写放大"));
        assert!(verify_grounding_quote(chunk, "max_wal_size 缓解 尖刺"));
        assert!(!verify_grounding_quote(chunk, "完全虚构的 Redis 集群脑裂问题"));
        assert!(!verify_grounding_quote(chunk, "短词"));
    }

    #[test]
    fn test_knowhow_ingest_supersedes_and_write_through_invalidation() {
        let storage = Storage::open_in_memory().expect("in_memory");
        let dir = tempfile::tempdir().expect("tempdir");
        let doc = dir.path().join("wal_knowhow.md");
        let body = "# PG WAL 调优\n高频 full_page_writes 会造成 wal_spike，调大 max_wal_size 可缓解写放大，仅适用于 SSD 磁盘。\n";
        std::fs::write(&doc, body).expect("write");
        let doc_path = doc.to_string_lossy().to_string();

        storage
            .upsert_asset(&AssetRecord {
                id: uuid::Uuid::new_v4().to_string(),
                path: doc_path.clone(),
                asset_type: "document".to_string(),
                owner: "user:tester".to_string(),
                security_level: "internal".to_string(),
                scan_tier: "deep".to_string(),
                description: Some("PG WAL Know-How".to_string()),
                desc_kv: json!({"domain": "storage_engine"}),
                usage: None,
                size_bytes: Some(128),
                file_count: None,
                tags: vec!["document".to_string()],
                status: "active".to_string(),
            })
            .expect("upsert");

        let kg_cfg = KnowledgeGraphConfig {
            pmi_threshold: 0.0,
            ..Default::default()
        };
        KnowledgeGraphBuilder::mine_from_storage(&storage, &kg_cfg).expect("mine l1");

        let sec = storage
            .list_all_fact_nodes(200)
            .expect("list")
            .into_iter()
            .find(|n| n.node_type == "section")
            .expect("section exists");

        let aliases = AliasCanonicalizer::default();
        let triples = vec![
            KnowHowTriple {
                subject: "full_page_writes".to_string(),
                relation: "causes".to_string(),
                object: "wal_spike".to_string(),
                boundary: "仅适用于 SSD 磁盘".to_string(),
                evidence_quote: "高频 full_page_writes 会造成 wal_spike".to_string(),
            },
            KnowHowTriple {
                subject: "imaginary_knob".to_string(),
                relation: "causes".to_string(),
                object: "oom_crash".to_string(),
                boundary: "".to_string(),
                evidence_quote: "这是模型编造的幻觉引文".to_string(),
            },
        ];

        let (gr, rej, kh_cnt, _) = KnowHowExtractor::ingest_triples_for_section(
            storage.provider().as_ref(),
            &sec.id,
            &doc_path,
            "PG WAL 调优",
            body,
            Some("hash123"),
            "qwen3.5:0.8b",
            &triples,
            &aliases,
        )
        .expect("ingest");
        assert_eq!(gr, 1);
        assert_eq!(rej, 1);
        assert_eq!(kh_cnt, 1);

        let exp = expand_graph_for_query(storage.provider().as_ref(), "full_page_writes", 2, &aliases);
        assert!(exp.doc_scores.contains_key(&doc_path));
        let secs = exp.matched_sections.get(&doc_path).cloned().unwrap_or_default();
        assert!(
            secs.iter().any(|s| s.contains("Know-How") && s.contains("SSD")),
            "图扩散应带出 Know-How 边界说明: {:?}",
            secs
        );

        // 验证写穿级联删除：删除源路径后所有 section 与 knowhow 节点清零
        let deleted = KnowledgeGraphBuilder::invalidate_source_path(
            storage.provider().as_ref(),
            &doc_path,
        )
        .expect("invalidate");
        assert!(deleted >= 2);
        let remaining_derived: Vec<_> = storage
            .list_all_fact_nodes(200)
            .expect("list")
            .into_iter()
            .filter(|n| {
                n.properties.get("source_path").and_then(|v| v.as_str()) == Some(doc_path.as_str())
            })
            .collect();
        assert!(remaining_derived.is_empty(), "源文档失效后衍生 section/knowhow 必须全部级联清理");
    }
}
