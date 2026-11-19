use std::fs::File;
use std::io::Read;
use std::path::Path;
use serde_json::{json, Map, Value};
use crate::scanner::importance::ImportanceScore;
use crate::storage::models::AssetRecord;

/// 渐进式元数据披露粒度 (ISSUE #8 Context 防爆设计)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailLevel {
    /// 极简名片 (~25 tokens): path, type, purpose, domain, entry_point
    Micro,
    /// 标准语义卡 (~85 tokens): L1 核心层 + L2 类型特化层，自动剥离 L0 管线噪音键
    Standard,
    /// 全量诊断卡 (~180 tokens): 完整 AssetRecord 含 L0 管线溯源键
    Full,
}

impl DetailLevel {
    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s.unwrap_or("standard").trim().to_lowercase().as_str() {
            "micro" | "min" | "brief" => Self::Micro,
            "full" | "all" | "raw" => Self::Full,
            _ => Self::Standard,
        }
    }
}

/// L0 管线治理与溯源内部键（在 Standard 级别对 Agent 隐藏以节省 Context Token）
const L0_PIPELINE_KEYS: &[&str] = &[
    "content_hash",
    "annotation_status",
    "annotation_model",
    "annotated_at",
    "importance_score",
    "importance_reason",
    "tier",
    "filename",
    "watcher_sync",
];

/// 按披露级别投影资产 JSON 视图
pub fn project_asset_view(asset: &AssetRecord, level: DetailLevel) -> Value {
    let kv_obj = asset.desc_kv.as_object();
    let purpose = kv_obj
        .and_then(|m| m.get("purpose"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| asset.description.clone())
        .unwrap_or_default();
    let domain = kv_obj
        .and_then(|m| m.get("domain"))
        .and_then(|v| v.as_str())
        .unwrap_or("general");
    let entry_point = kv_obj
        .and_then(|m| m.get("entry_point"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match level {
        DetailLevel::Micro => json!({
            "path": asset.path,
            "type": asset.asset_type,
            "domain": domain,
            "purpose": purpose,
            "entry_point": entry_point,
        }),
        DetailLevel::Standard => {
            let mut filtered_kv = Map::new();
            if let Some(map) = kv_obj {
                for (k, v) in map {
                    if !L0_PIPELINE_KEYS.contains(&k.as_str()) {
                        filtered_kv.insert(k.clone(), v.clone());
                    }
                }
            }
            json!({
                "id": asset.id,
                "path": asset.path,
                "type": asset.asset_type,
                "owner": asset.owner,
                "security_level": asset.security_level,
                "scan_tier": asset.scan_tier,
                "description": asset.description,
                "usage": asset.usage,
                "tags": asset.tags,
                "desc_kv": Value::Object(filtered_kv),
            })
        }
        DetailLevel::Full => serde_json::to_value(asset).unwrap_or_else(|_| json!({})),
    }
}

/// 格式化单条资产为高信噪比可读文本行（供 MCP / CLI 输出）
pub fn format_asset_line(asset: &AssetRecord, level: DetailLevel) -> String {
    let kv = asset.desc_kv.as_object();
    let purpose = kv
        .and_then(|m| m.get("purpose"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| asset.description.clone())
        .unwrap_or_else(|| "(无描述)".to_string());
    let domain = kv
        .and_then(|m| m.get("domain"))
        .and_then(|v| v.as_str())
        .unwrap_or("-");
    let entry = kv
        .and_then(|m| m.get("entry_point"))
        .and_then(|v| v.as_str())
        .unwrap_or("");

    match level {
        DetailLevel::Micro => {
            if entry.is_empty() {
                format!("  - [{}|{}] {} | {}", asset.asset_type, domain, asset.path, purpose)
            } else {
                format!("  - [{}|{}] {} | {} (入口: {})", asset.asset_type, domain, asset.path, purpose, entry)
            }
        }
        DetailLevel::Standard => {
            let caps = kv
                .and_then(|m| m.get("capabilities"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str())
                        .collect::<Vec<_>>()
                        .join(" / ")
                })
                .unwrap_or_default();
            let symbols_or_headings = kv
                .and_then(|m| m.get("symbols").or_else(|| m.get("headings")).or_else(|| m.get("config_sections")))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str())
                        .take(5)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
                .unwrap_or_default();
            let mut line = format!(
                "  - [{}|{}] {}\n    用途: {}",
                asset.asset_type, domain, asset.path, purpose
            );
            if !caps.is_empty() {
                line.push_str(&format!(" | 能力: [{}]", caps));
            }
            if !entry.is_empty() {
                line.push_str(&format!("\n    入口: {}", entry));
            }
            if !symbols_or_headings.is_empty() {
                line.push_str(&format!(" | 关键符号/章节: [{}]", symbols_or_headings));
            }
            if let Some(ref u) = asset.usage {
                if !u.is_empty() {
                    line.push_str(&format!("\n    用法: {}", u));
                }
            }
            line
        }
        DetailLevel::Full => {
            let model = kv
                .and_then(|m| m.get("annotation_model"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let status = kv
                .and_then(|m| m.get("annotation_status"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            format!(
                "  - [{}|{}|status:{}|model:{}] {}\n    用途: {}\n    desc_kv: {}",
                asset.asset_type,
                domain,
                status,
                model,
                asset.path,
                purpose,
                serde_json::to_string(&asset.desc_kv).unwrap_or_default()
            )
        }
    }
}

/// 敏感文件判定（严禁读取内容送入提取或 Ollama）
pub fn is_sensitive_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    let path_str = path.to_string_lossy().to_lowercase();

    if path_str.contains("/.ssh/") || path_str.contains("/.gnupg/") {
        return true;
    }
    if name == ".env" || name.starts_with(".env.") || name == "id_rsa" || name == "id_ed25519" {
        return true;
    }
    if name.ends_with(".pem") || name.ends_with(".key") || name.ends_with(".p12") || name.ends_with(".pfx") {
        return true;
    }
    if name.contains("secret") || name.contains("credential") || name.contains("password") {
        // 允许代码/文档自身讨论脱敏规则，仅拦截数据/配置文件
        let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
        if matches!(ext, "json" | "toml" | "yaml" | "yml" | "ini" | "conf" | "env" | "txt" | "") {
            return true;
        }
    }
    false
}

/// 密钥与敏感凭据脱敏器（送入本地 Ollama 前强制调用）
pub struct SecretRedactor;

impl SecretRedactor {
    pub fn redact(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for line in input.lines() {
            let lower = line.to_lowercase();
            let looks_like_secret_kv = (lower.contains("api_key")
                || lower.contains("apikey")
                || lower.contains("secret_key")
                || lower.contains("access_token")
                || lower.contains("auth_token")
                || lower.contains("password")
                || lower.contains("passwd"))
                && (line.contains('=') || line.contains(':'));

            if looks_like_secret_kv && !line.trim_start().starts_with("//") && !line.trim_start().starts_with('#') {
                if let Some(idx) = line.find('=').or_else(|| line.find(':')) {
                    let key_part = &line[..=idx];
                    out.push_str(key_part);
                    out.push_str(" \"<REDACTED>\"\n");
                    continue;
                }
            }

            let masked = Self::mask_inline_tokens(line);
            out.push_str(&masked);
            out.push('\n');
        }
        out
    }

    fn mask_inline_tokens(line: &str) -> String {
        let mut result = line.to_string();
        for prefix in &["sk-", "ghp_", "gho_", "glpat-", "xoxb-", "AKIA"] {
            while let Some(pos) = result.find(prefix) {
                let end = result[pos..]
                    .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == ';')
                    .map(|off| pos + off)
                    .unwrap_or(result.len());
                if end - pos >= 12 {
                    result.replace_range(pos..end, "<REDACTED_TOKEN>");
                } else {
                    break;
                }
            }
        }
        result
    }
}

/// L1 确定性提取结果
#[derive(Debug, Clone)]
pub struct ExtractedMetadata {
    pub description: String,
    pub usage: Option<String>,
    pub desc_kv: Value,
    pub security_level: String,
    pub extra_tags: Vec<String>,
}

/// L1 确定性微秒级元数据提取器（0 LLM 开销，< 1ms）
pub struct HeuristicMetadataExtractor;

impl HeuristicMetadataExtractor {
    pub fn extract(
        path: &Path,
        asset_type: &str,
        tier: &str,
        imp_score: Option<&ImportanceScore>,
        size_bytes: Option<i64>,
    ) -> ExtractedMetadata {
        let file_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let domain = infer_domain_from_path(path);

        // 安全红线：黑名单目录或敏感文件严禁读取文件内容
        if tier == "shallow" || is_sensitive_file(path) {
            let sec_level = if is_sensitive_file(path) { "secret" } else { "confidential" };
            return ExtractedMetadata {
                description: format!("[{}] 敏感/受限资产 ({})，已跳过内容解析", domain, file_name),
                usage: None,
                desc_kv: json!({
                    "purpose": format!("受限保护文件 ({})", file_name),
                    "domain": domain,
                    "capabilities": ["restricted_access"],
                    "entry_point": file_name,
                    "annotation_status": "skipped_sensitive",
                    "annotation_model": "heuristic-v1",
                    "tier": tier,
                }),
                security_level: sec_level.to_string(),
                extra_tags: vec!["sensitive_guarded".to_string()],
            };
        }

        // 读取前 8KB 用于微秒级特征提取与指纹计算
        let snippet = read_head_utf8(path, 8192).unwrap_or_default();
        let content_hash = compute_fnv1a_hash(&snippet, size_bytes.unwrap_or(0));
        let loc = snippet.lines().count();
        let fs_meta = std::fs::metadata(path).ok();
        let actual_size = size_bytes.or_else(|| fs_meta.as_ref().map(|m| m.len() as i64));
        let last_modified_at = fs_meta
            .as_ref()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let dt: chrono::DateTime<chrono::Local> = t.into();
                dt.to_rfc3339()
            });
        #[cfg(unix)]
        let file_mode = fs_meta.as_ref().map(|m| {
            use std::os::unix::fs::PermissionsExt;
            format!("{:04o}", m.permissions().mode() & 0o7777)
        });
        #[cfg(not(unix))]
        let file_mode: Option<String> = None;

        let mut kv = Map::new();
        kv.insert("canonical_path".to_string(), json!(path.to_string_lossy().to_string()));
        kv.insert("category".to_string(), json!(asset_type));
        kv.insert("file_name".to_string(), json!(file_name));
        if !ext.is_empty() {
            kv.insert("extension".to_string(), json!(ext));
        }
        if let Some(sz) = actual_size {
            kv.insert("size_bytes".to_string(), json!(sz));
        }
        if let Some(ref mode) = file_mode {
            kv.insert("file_mode".to_string(), json!(mode));
        }
        if let Some(ref mtime) = last_modified_at {
            kv.insert("last_modified_at".to_string(), json!(mtime));
        }
        kv.insert("domain".to_string(), json!(domain));
        kv.insert("content_hash".to_string(), json!(content_hash));
        kv.insert("annotation_status".to_string(), json!("heuristic_only"));
        kv.insert("annotation_model".to_string(), json!("heuristic-v1"));
        kv.insert("embedding_status".to_string(), json!("pending"));
        kv.insert("tier".to_string(), json!(tier));
        if let Some(imp) = imp_score {
            kv.insert("importance_score".to_string(), json!(imp.score));
        }

        let mut extra_tags = Vec::new();
        let (purpose, entry_point, usage_hint) = match asset_type {
            "script" => Self::extract_script(&snippet, &ext, &file_name, loc, &mut kv, &mut extra_tags),
            "document" => Self::extract_document(path, &snippet, &file_name, &mut kv, &mut extra_tags),
            "config" => Self::extract_config(&snippet, &ext, &file_name, &mut kv, &mut extra_tags),
            "dataset" => Self::extract_dataset(&snippet, &ext, &file_name, actual_size, &mut kv, &mut extra_tags),
            "media" => Self::extract_media(path, &snippet, &ext, &file_name, actual_size, &mut kv, &mut extra_tags),
            _ => Self::extract_other(&snippet, &ext, &file_name, loc, actual_size, &mut kv),
        };

        kv.insert("purpose".to_string(), json!(purpose));
        kv.insert("entry_point".to_string(), json!(entry_point));
        if let Some(ref u) = usage_hint {
            kv.insert("usage_hint".to_string(), json!(u));
        }

        let description = format!("[{}] {}", domain, purpose);
        kv.insert("description".to_string(), json!(description));
        ExtractedMetadata {
            description,
            usage: usage_hint,
            desc_kv: Value::Object(kv),
            security_level: "internal".to_string(),
            extra_tags,
        }
    }

    fn extract_script(
        snippet: &str,
        ext: &str,
        file_name: &str,
        loc: usize,
        kv: &mut Map<String, Value>,
        extra_tags: &mut Vec<String>,
    ) -> (String, String, Option<String>) {
        let lang = match ext {
            "rs" => "rust",
            "py" => "python",
            "sh" | "bash" | "zsh" => "shell",
            "js" | "mjs" | "cjs" | "jsx" => "javascript",
            "ts" | "tsx" => "typescript",
            "go" => "go",
            "c" | "h" => "c",
            "cpp" | "cc" | "hpp" => "cpp",
            "lua" => "lua",
            "rb" => "ruby",
            _ => ext,
        };
        kv.insert("lang".to_string(), json!(lang));
        kv.insert("loc".to_string(), json!(loc));

        let mut symbols: Vec<String> = Vec::new();
        let mut deps: Vec<String> = Vec::new();
        let mut doc_comment: Option<String> = None;

        for line in snippet.lines().take(120) {
            let t = line.trim();
            if doc_comment.is_none() {
                if let Some(c) = t.strip_prefix("//!").or_else(|| t.strip_prefix("///")) {
                    let clean = c.trim();
                    if !clean.is_empty() && clean.chars().count() >= 4 {
                        doc_comment = Some(truncate_chars(clean, 45));
                    }
                } else if lang == "python" && t.starts_with('#') && !t.starts_with("#!") {
                    let clean = t.trim_start_matches('#').trim();
                    if !clean.is_empty() && clean.chars().count() >= 4 {
                        doc_comment = Some(truncate_chars(clean, 45));
                    }
                }
            }

            // 提取符号
            if symbols.len() < 8 {
                if let Some(sym) = extract_symbol_name(t, lang) {
                    if !symbols.contains(&sym) {
                        symbols.push(sym);
                    }
                }
            }

            // 提取依赖
            if deps.len() < 6 {
                if (t.starts_with("use ") || t.starts_with("import ") || t.starts_with("from ")) && t.len() < 80 {
                    let dep = t.trim_end_matches(';').to_string();
                    if !deps.contains(&dep) {
                        deps.push(dep);
                    }
                }
            }
        }

        // 启发式副作用识别
        let mut side_effects = Vec::new();
        if snippet.contains("TcpListener") || snippet.contains("bind(") || snippet.contains("serve(") {
            side_effects.push("port_bind");
        }
        if snippet.contains("execute(") || snippet.contains("INSERT ") || snippet.contains("upsert_") {
            side_effects.push("db_write");
        }
        if snippet.contains("fs::write") || snippet.contains("create_dir") || snippet.contains("OpenOptions") {
            side_effects.push("fs_write");
        }
        if snippet.contains("reqwest") || snippet.contains("http://") || snippet.contains("https://") {
            side_effects.push("net");
        }
        if side_effects.is_empty() {
            side_effects.push("readonly");
        }

        let entry_point = symbols.first().cloned().unwrap_or_else(|| file_name.to_string());
        let purpose = doc_comment.unwrap_or_else(|| {
            if symbols.is_empty() {
                format!("{} 脚本模块 ({})", lang, file_name)
            } else {
                format!("{} 模块，提供 {}", lang, symbols.iter().take(3).cloned().collect::<Vec<_>>().join(", "))
            }
        });

        let mut caps = Vec::new();
        if !symbols.is_empty() {
            caps.push(format!("导出 {} 个核心符号", symbols.len()));
        }
        if side_effects.contains(&"db_write") {
            caps.push("数据库持久化".to_string());
        }
        if side_effects.contains(&"net") {
            caps.push("网络通信/HTTP调用".to_string());
        }
        if caps.is_empty() {
            caps.push(format!("{} 逻辑实现", lang));
        }

        let usage_hint = match lang {
            "python" => Some(format!("python3 {}", file_name)),
            "shell" => Some(format!("bash {}", file_name)),
            _ => symbols.first().map(|s| format!("引入符号: {}", s)),
        };

        kv.insert("symbols".to_string(), json!(symbols));
        kv.insert("dependencies".to_string(), json!(deps));
        kv.insert("side_effects".to_string(), json!(side_effects));
        kv.insert("capabilities".to_string(), json!(caps));
        extra_tags.push(lang.to_string());

        (purpose, entry_point, usage_hint)
    }

    fn extract_document(
        path: &Path,
        snippet: &str,
        file_name: &str,
        kv: &mut Map<String, Value>,
        extra_tags: &mut Vec<String>,
    ) -> (String, String, Option<String>) {
        let path_str = path.to_string_lossy().to_lowercase();
        let doc_category = if file_name.eq_ignore_ascii_case("SKILL.md") {
            "skill"
        } else if path_str.contains("design_doc") {
            "design"
        } else if path_str.contains("manual") {
            "manual"
        } else if file_name.contains("roadmap") || file_name.contains("progress") {
            "roadmap"
        } else if file_name.contains("权威网站") {
            "authority"
        } else {
            "note"
        };

        let mut headings = Vec::new();
        let mut h1_title: Option<String> = None;
        let mut frontmatter_desc: Option<String> = None;
        let mut in_frontmatter = false;

        for (idx, line) in snippet.lines().take(100).enumerate() {
            let t = line.trim();
            if idx == 0 && t == "---" {
                in_frontmatter = true;
                continue;
            }
            if in_frontmatter {
                if t == "---" {
                    in_frontmatter = false;
                    continue;
                }
                if let Some(rest) = t.strip_prefix("description:") {
                    frontmatter_desc = Some(truncate_chars(rest.trim().trim_matches('"'), 45));
                }
                continue;
            }
            if t.starts_with("# ") || t.starts_with("## ") {
                let title = t.trim_start_matches('#').trim().to_string();
                if h1_title.is_none() && t.starts_with("# ") {
                    h1_title = Some(truncate_chars(&title, 45));
                }
                if headings.len() < 6 && !title.is_empty() {
                    headings.push(truncate_chars(&title, 35));
                }
            }
        }

        let purpose = frontmatter_desc
            .or_else(|| h1_title.clone())
            .unwrap_or_else(|| format!("{} 文档 ({})", doc_category, file_name));
        let entry_point = headings.first().cloned().unwrap_or_else(|| file_name.to_string());

        kv.insert("doc_category".to_string(), json!(doc_category));
        kv.insert("headings".to_string(), json!(headings));
        kv.insert("capabilities".to_string(), json!(vec![format!("{} 知识文档", doc_category)]));
        extra_tags.push(doc_category.to_string());

        (purpose, entry_point, Some(format!("按章节检索: {}", file_name)))
    }

    fn extract_config(
        snippet: &str,
        ext: &str,
        file_name: &str,
        kv: &mut Map<String, Value>,
        extra_tags: &mut Vec<String>,
    ) -> (String, String, Option<String>) {
        let mut sections = Vec::new();
        let mut env_vars = Vec::new();
        let target_system = match ext {
            "sql" => "database_schema",
            "toml" => {
                if file_name == "Cargo.toml" { "rust_cargo" } else { "toml_config" }
            }
            "service" => "systemd_unit",
            "json" => "json_config",
            "yaml" | "yml" => "yaml_config",
            _ => "config",
        };

        for line in snippet.lines().take(120) {
            let t = line.trim();
            if sections.len() < 8 {
                if ext == "sql" {
                    let upper = t.to_uppercase();
                    if upper.starts_with("CREATE TABLE") || upper.starts_with("CREATE VIRTUAL TABLE") {
                        let name = t.split_whitespace().nth(if upper.contains("NOT EXISTS") { 5 } else { 2 }).unwrap_or("");
                        let clean = name.trim_matches('(');
                        if !clean.is_empty() && !sections.contains(&clean.to_string()) {
                            sections.push(clean.to_string());
                        }
                    }
                } else if t.starts_with('[') && t.ends_with(']') && !t.starts_with("[[") {
                    let sec = t.trim_matches(|c| c == '[' || c == ']').to_string();
                    if !sections.contains(&sec) {
                        sections.push(sec);
                    }
                }
            }
            // 提取环境变量名（绝不存值）
            if env_vars.len() < 6 {
                for word in t.split(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
                    if word.len() >= 4
                        && word.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                        && (word.contains("KEY") || word.contains("TOKEN") || word.contains("URL") || word.contains("STEWARD") || word.contains("PATH"))
                    {
                        if !env_vars.contains(&word.to_string()) {
                            env_vars.push(word.to_string());
                        }
                    }
                }
            }
        }

        let purpose = if sections.is_empty() {
            format!("{} 配置文件 ({})", target_system, file_name)
        } else {
            format!("{} 配置，包含 [{}]", target_system, sections.iter().take(3).cloned().collect::<Vec<_>>().join(", "))
        };
        let entry_point = sections.first().cloned().unwrap_or_else(|| file_name.to_string());

        kv.insert("target_system".to_string(), json!(target_system));
        kv.insert("config_sections".to_string(), json!(sections));
        kv.insert("env_vars_referenced".to_string(), json!(env_vars));
        kv.insert("capabilities".to_string(), json!(vec![format!("{} 配置定义", target_system)]));
        extra_tags.push(target_system.to_string());

        (purpose, entry_point, None)
    }

    fn extract_dataset(
        snippet: &str,
        ext: &str,
        file_name: &str,
        size_bytes: Option<i64>,
        kv: &mut Map<String, Value>,
        extra_tags: &mut Vec<String>,
    ) -> (String, String, Option<String>) {
        let fmt = if ext.is_empty() { "dataset" } else { ext };
        let mut columns: Vec<String> = Vec::new();
        let mut sample_rows = 0usize;

        if matches!(fmt, "csv" | "tsv") {
            let sep = if fmt == "tsv" { '\t' } else { ',' };
            let mut lines = snippet.lines().filter(|l| !l.trim().is_empty());
            if let Some(header) = lines.next() {
                columns = header
                    .split(sep)
                    .map(|c| c.trim().trim_matches('"').to_string())
                    .filter(|c| !c.is_empty())
                    .take(12)
                    .collect();
            }
            sample_rows = lines.count();
        } else if matches!(fmt, "jsonl" | "ndjson") {
            let mut lines = snippet.lines().filter(|l| !l.trim().is_empty());
            if let Some(first) = lines.next() {
                if let Ok(Value::Object(obj)) = serde_json::from_str::<Value>(first) {
                    columns = obj.keys().take(12).cloned().collect();
                }
            }
            sample_rows = lines.count() + 1;
        }

        let purpose = if columns.is_empty() {
            format!("{} 数据集 ({}, {} bytes)", fmt, file_name, size_bytes.unwrap_or(0))
        } else {
            format!("{} 数据集，字段 [{}]", fmt, columns.iter().take(4).cloned().collect::<Vec<_>>().join(", "))
        };
        kv.insert("dataset_format".to_string(), json!(fmt));
        kv.insert("columns".to_string(), json!(columns));
        kv.insert("sample_rows_in_head".to_string(), json!(sample_rows));
        kv.insert("capabilities".to_string(), json!(vec![format!("{} 结构化数据存取", fmt)]));
        extra_tags.push(fmt.to_string());

        (purpose, file_name.to_string(), Some(format!("数据检索/DuckDB分析: {}", file_name)))
    }

    fn extract_media(
        _path: &Path,
        snippet: &str,
        ext: &str,
        file_name: &str,
        size_bytes: Option<i64>,
        kv: &mut Map<String, Value>,
        extra_tags: &mut Vec<String>,
    ) -> (String, String, Option<String>) {
        let fmt = if ext.is_empty() { "media" } else { ext };
        let mut dimensions: Option<String> = None;

        if fmt == "svg" {
            if let Some(idx) = snippet.find("viewBox=\"") {
                let rest = &snippet[idx + 9..];
                if let Some(end) = rest.find('"') {
                    dimensions = Some(rest[..end].to_string());
                }
            }
        }

        let purpose = match dimensions {
            Some(ref d) => format!("{} 矢量图形 ({}, viewBox: {})", fmt.to_uppercase(), file_name, d),
            None => format!("{} 媒体资源 ({}, {} bytes)", fmt.to_uppercase(), file_name, size_bytes.unwrap_or(0)),
        };

        kv.insert("media_format".to_string(), json!(fmt));
        if let Some(ref d) = dimensions {
            kv.insert("viewbox".to_string(), json!(d));
        }
        kv.insert("capabilities".to_string(), json!(vec![format!("{} 媒体资产", fmt)]));
        extra_tags.push(fmt.to_string());

        (purpose, file_name.to_string(), None)
    }

    fn extract_other(
        snippet: &str,
        ext: &str,
        file_name: &str,
        loc: usize,
        size_bytes: Option<i64>,
        kv: &mut Map<String, Value>,
    ) -> (String, String, Option<String>) {
        let is_binary = snippet.is_empty() || snippet.contains('\u{FFFD}');
        let fmt = if !ext.is_empty() {
            ext
        } else if is_binary {
            "binary"
        } else {
            "plain_text"
        };
        let first_meaningful = if !is_binary {
            snippet
                .lines()
                .map(|l| l.trim())
                .find(|l| !l.is_empty() && l.chars().count() >= 4)
                .map(|l| truncate_chars(l, 45))
        } else {
            None
        };
        let purpose = first_meaningful.unwrap_or_else(|| {
            format!("{} 资源文件 ({}, {} bytes)", fmt, file_name, size_bytes.unwrap_or(0))
        });

        kv.insert("format".to_string(), json!(fmt));
        kv.insert("is_binary".to_string(), json!(is_binary));
        kv.insert("loc".to_string(), json!(loc));
        kv.insert("capabilities".to_string(), json!(vec![format!("{} 资源文件", fmt)]));
        (purpose, file_name.to_string(), None)
    }
}

/// 防覆写智能合并：保护已有的 Ollama/手动丰富元数据不被 Watcher 或重扫抹除
pub fn merge_with_existing_asset(mut new_rec: AssetRecord, existing: Option<&AssetRecord>) -> AssetRecord {
    let Some(old) = existing else {
        return new_rec;
    };

    new_rec.id = old.id.clone();

    let old_hash = old.desc_kv.get("content_hash").and_then(|v| v.as_str());
    let new_hash = new_rec.desc_kv.get("content_hash").and_then(|v| v.as_str());
    let old_status = old
        .desc_kv
        .get("annotation_status")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let is_rich_old = matches!(old_status, "ollama_enriched" | "manual");

    if is_rich_old && old_hash.is_some() && old_hash == new_hash {
        // 文件实质内容未变，完整保留已有的 Ollama 语义标注与用法
        new_rec.description = old.description.clone();
        new_rec.usage = old.usage.clone();
        new_rec.desc_kv = old.desc_kv.clone();
    } else if is_rich_old && old_hash != new_hash {
        // 文件内容发生实质变更，采用新 L1 事实并标记需要重新精炼
        if let Some(obj) = new_rec.desc_kv.as_object_mut() {
            obj.insert("annotation_status".to_string(), json!("needs_re_enrich"));
        }
    }

    new_rec
}

pub fn infer_domain_from_path(path: &Path) -> String {
    let s = path.to_string_lossy().to_lowercase();
    if s.contains("/coordination/") { "lock_coordination" }
    else if s.contains("/scanner/") { "asset_scanner" }
    else if s.contains("/fact/") { "host_reality_graph" }
    else if s.contains("/memory/") { "memory_context" }
    else if s.contains("/placement/") { "placement_governance" }
    else if s.contains("/links/") { "dependency_graph" }
    else if s.contains("/web/") { "web_dashboard" }
    else if s.contains("/api/") { "mcp_api" }
    else if s.contains("/hooks/") { "hooks_pipeline" }
    else if s.contains("/ecosystem/") { "ecosystem_router" }
    else if s.contains("/demo/") { "roadshow_demo" }
    else if s.contains("/storage/") { "storage_engine" }
    else if s.contains("/skills/") { "agent_skills" }
    else if s.contains("/design_doc/") { "architecture_design" }
    else if s.contains("/manual/") { "user_manual" }
    else if s.contains("/shared_proj/") { "shared_tools" }
    else if s.contains("/note/") { "knowledge_notes" }
    else { "workspace_general" }
    .to_string()
}

pub fn read_head_utf8(path: &Path, max_bytes: usize) -> Option<String> {
    let mut file = File::open(path).ok()?;
    let mut buf = vec![0u8; max_bytes];
    let n = file.read(&mut buf).ok()?;
    buf.truncate(n);
    Some(String::from_utf8_lossy(&buf).to_string())
}

fn compute_fnv1a_hash(content: &str, size_bytes: i64) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in content.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    for b in size_bytes.to_le_bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

fn extract_symbol_name(line: &str, lang: &str) -> Option<String> {
    match lang {
        "rust" => {
            for kw in &["pub fn ", "pub async fn ", "pub struct ", "pub enum ", "pub trait ", "fn "] {
                if let Some(rest) = line.strip_prefix(kw) {
                    let name = rest.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("");
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
            None
        }
        "python" => {
            for kw in &["def ", "async def ", "class "] {
                if let Some(rest) = line.strip_prefix(kw) {
                    let name = rest.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("");
                    if !name.is_empty() && !name.starts_with('_') {
                        return Some(name.to_string());
                    }
                }
            }
            None
        }
        "javascript" | "typescript" => {
            for kw in &["export function ", "export async function ", "export class ", "function ", "class "] {
                if let Some(rest) = line.strip_prefix(kw) {
                    let name = rest.split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("");
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
            }
            None
        }
        "go" => {
            if let Some(rest) = line.strip_prefix("func ") {
                let clean = rest.trim_start_matches(|c: char| c != ')' && !c.is_alphabetic());
                let name = clean.trim_start_matches(')').trim().split(|c: char| !c.is_alphanumeric() && c != '_').next().unwrap_or("");
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
            None
        }
        _ => None,
    }
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_chars).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::io::Write;

    #[test]
    fn test_heuristic_extract_rust_and_projection() {
        let dir = tempdir().unwrap();
        let work_dir = dir.path().join("coordination");
        std::fs::create_dir_all(&work_dir).unwrap();
        let rs_path = work_dir.join("deadlock.rs");
        let mut f = File::create(&rs_path).unwrap();
        writeln!(f, "//! Wait-For Graph (WFG) 死锁检测引擎").unwrap();
        writeln!(f, "use std::collections::HashMap;").unwrap();
        writeln!(f, "pub struct DeadlockDetector;").unwrap();
        writeln!(f, "pub fn detect_cycles() {{}}").unwrap();

        let meta = HeuristicMetadataExtractor::extract(&rs_path, "script", "deep", None, Some(128));
        assert_eq!(meta.desc_kv["domain"], "lock_coordination");
        assert_eq!(meta.desc_kv["lang"], "rust");
        assert!(meta.desc_kv["purpose"].as_str().unwrap().contains("死锁检测"));
        let syms = meta.desc_kv["symbols"].as_array().unwrap();
        assert!(syms.iter().any(|v| v == "DeadlockDetector"));
        assert!(syms.iter().any(|v| v == "detect_cycles"));

        let rec = AssetRecord {
            id: "a1".to_string(),
            path: rs_path.to_string_lossy().to_string(),
            asset_type: "script".to_string(),
            owner: "user:digoal".to_string(),
            security_level: meta.security_level,
            scan_tier: "deep".to_string(),
            description: Some(meta.description),
            desc_kv: meta.desc_kv,
            usage: meta.usage,
            size_bytes: Some(128),
            file_count: None,
            tags: meta.extra_tags,
            status: "active".to_string(),
        };

        let micro = project_asset_view(&rec, DetailLevel::Micro);
        assert!(micro.get("content_hash").is_none());
        assert!(micro.get("purpose").is_some());

        let std_view = project_asset_view(&rec, DetailLevel::Standard);
        assert!(std_view["desc_kv"].get("content_hash").is_none(), "Standard 视图应剥离 L0 管线键");
        assert!(std_view["desc_kv"].get("symbols").is_some());

        let full = project_asset_view(&rec, DetailLevel::Full);
        assert!(full["desc_kv"].get("content_hash").is_some(), "Full 视图应保留 L0 溯源键");
    }

    #[test]
    fn test_secret_redactor_and_sensitive_file_guard() {
        let dir = tempdir().unwrap();
        let env_path = dir.path().join(".env.prod");
        File::create(&env_path).unwrap().write_all(b"OPENAI_API_KEY=sk-1234567890abcdef").unwrap();

        let meta = HeuristicMetadataExtractor::extract(&env_path, "config", "deep", None, Some(35));
        assert_eq!(meta.security_level, "secret");
        assert_eq!(meta.desc_kv["annotation_status"], "skipped_sensitive");

        let raw = "api_key = \"sk-1234567890abcdef\"\nlet x = 1;";
        let redacted = SecretRedactor::redact(raw);
        assert!(!redacted.contains("sk-1234567890abcdef"));
        assert!(redacted.contains("<REDACTED>"));
    }

    #[test]
    fn test_merge_prevents_clobbering_ollama_enriched() {
        let existing = AssetRecord {
            id: "old-id".to_string(),
            path: "/tmp/a.rs".to_string(),
            asset_type: "script".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("[scanner] 高精度Ollama标注".to_string()),
            desc_kv: json!({
                "purpose": "高精度Ollama标注",
                "content_hash": "abc123",
                "annotation_status": "ollama_enriched",
                "annotation_model": "qwen3.5:0.8b"
            }),
            usage: Some("run_a()".to_string()),
            size_bytes: Some(100),
            file_count: None,
            tags: vec![],
            status: "active".to_string(),
        };

        let same_hash_new = AssetRecord {
            id: "new-id".to_string(),
            path: "/tmp/a.rs".to_string(),
            asset_type: "script".to_string(),
            owner: "user:digoal".to_string(),
            security_level: "internal".to_string(),
            scan_tier: "deep".to_string(),
            description: Some("watcher_sync".to_string()),
            desc_kv: json!({"content_hash": "abc123", "annotation_status": "heuristic_only"}),
            usage: None,
            size_bytes: Some(100),
            file_count: None,
            tags: vec![],
            status: "active".to_string(),
        };

        let merged = merge_with_existing_asset(same_hash_new, Some(&existing));
        assert_eq!(merged.desc_kv["annotation_status"], "ollama_enriched");
        assert_eq!(merged.desc_kv["purpose"], "高精度Ollama标注");
        assert_eq!(merged.usage.as_deref(), Some("run_a()"));
    }
}
