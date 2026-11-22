use std::sync::Arc;
use regex::Regex;
use crate::storage::{Storage, models::LinkRecord};
use anyhow::Result;

pub mod audit;
pub mod codegraph_bridge;
pub mod impact;
pub mod repair;

pub struct LinksEngine {
    storage: Arc<Storage>,
}

/// 判断路径是否为动态格式化模板、示例占位符或临时日志路径（避免产生假阳性依赖边）
pub(crate) fn is_non_static_link_target(target: &str, trailing_char: Option<char>) -> bool {
    if target.is_empty() {
        return true;
    }
    if target.ends_with('_') || target.ends_with('-') {
        return true;
    }
    if let Some(ch) = trailing_char {
        if matches!(ch, '%' | '{' | '<' | '$' | '*') {
            return true;
        }
    }
    let lower = target.to_lowercase();
    if target.contains("...")
        || lower.contains("/xxx")
        || lower.contains("_xxx")
        || lower.contains("placeholder")
        || target.contains("YYYY")
        || target.contains("MM-DD")
        || target.contains('{')
        || target.contains('}')
        || target.contains('<')
        || target.contains('>')
        || target.contains("%s")
        || target.contains("%d")
        || target.contains("%f")
    {
        return true;
    }
    is_ephemeral_or_log_path(target)
}

pub(crate) fn is_ephemeral_or_log_path(target: &str) -> bool {
    target.starts_with("/tmp/")
        || target.starts_with("/root/new/tmp/")
        || target.contains("/_scratch/")
        || target.ends_with(".log")
        || target.ends_with(".tmp")
}

pub(crate) fn is_pure_data_file(path: &str) -> bool {
    let lower = path.to_lowercase();
    lower.ends_with(".json")
        || lower.ends_with(".jsonl")
        || lower.ends_with(".csv")
        || lower.ends_with(".log")
}

/// 判断是否为显式 Markdown 文件链接 (`[text](/root/new/...)` 或 `[text](file:///root/new/...)`)
pub(crate) fn is_explicit_markdown_link(snippet: Option<&str>) -> bool {
    match snippet {
        Some(s) => s.contains("](/root/new/") || s.contains("](file:///root/new/"),
        None => false,
    }
}

/// 判断静态抽取出的引用边是否属于测试夹具、注释、UI 占位符或 Markdown 文档示例（避免假断链自污染）
pub(crate) fn is_test_or_example_link(
    source_path: &str,
    target_path: &str,
    snippet: Option<&str>,
) -> bool {
    let in_sys_tmp = source_path.starts_with("/tmp/");
    if !in_sys_tmp
        && (source_path.contains("/tests/")
            || source_path.ends_with("_test.rs")
            || source_path.ends_with("_test.py"))
    {
        return true;
    }

    if let Some(snip) = snippet {
        let trimmed = snip.trim();
        let lower = trimmed.to_lowercase();
        if !in_sys_tmp
            && (trimmed.starts_with("//")
                || trimmed.starts_with("///")
                || trimmed.starts_with("//!")
                || trimmed.starts_with("* "))
        {
            return true;
        }
        if source_path.ends_with(".rs")
            && !in_sys_tmp
            && (lower.contains("placeholder=")
                || lower.contains("assert!")
                || lower.contains("assert_eq!")
                || lower.contains("assert_ne!")
                || lower.contains("expect(")
                || lower.contains("${esc(")
                || lower.contains("broken_ref")
                || lower.contains("repaired_ref")
                || lower.contains("示例")
                || lower.contains("adversarial_")
                || trimmed.starts_with("target_id:"))
        {
            return true;
        }
    }

    if source_path.ends_with(".rs")
        && !in_sys_tmp
        && (source_path.contains("/demo/") || source_path.contains("/benchmark/"))
        && !std::path::Path::new(target_path).exists()
    {
        return true;
    }

    if source_path.ends_with(".md")
        && std::path::Path::new(source_path).exists()
        && !std::path::Path::new(target_path).exists()
        && !is_explicit_markdown_link(snippet)
    {
        return true;
    }

    false
}

impl LinksEngine {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }

    /// 手动建立关联
    pub fn create_link(
        &self,
        source_type: &str,
        source_id: &str,
        target_type: &str,
        target_id: &str,
        relation: &str,
        confidence: f32,
    ) -> Result<()> {
        let link = LinkRecord {
            id: None,
            source_type: source_type.to_string(),
            source_id: source_id.to_string(),
            target_type: target_type.to_string(),
            target_id: target_id.to_string(),
            relation: relation.to_string(),
            confidence,
            auto_discovered: false,
            line_number: None,
            snippet: None,
            provenance: Some("manual".to_string()),
            created_at: None,
        };
        self.storage.add_link(&link)
    }

    /// 查询某实体的关联关系
    pub fn query_links(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        self.storage.query_links_for_node(node_type, node_id)
    }

    /// 幂等刷新单个物理资产的自动发现依赖边：
    /// 仅解析文本类脚本/配置/文档（<= 512KB），先清理该 source 的旧 auto_discovered 边再重建。
    pub fn refresh_links_for_asset(&self, asset_path: &std::path::Path) -> Result<usize> {
        let ext = asset_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        let is_supported = matches!(
            ext.as_str(),
            "rs" | "py" | "sh" | "bash" | "js" | "ts" | "toml" | "yaml" | "yml" | "json" | "service" | "conf" | "md"
        );
        if !is_supported {
            return Ok(0);
        }
        let meta = match std::fs::metadata(asset_path) {
            Ok(m) => m,
            Err(_) => return Ok(0),
        };
        if !meta.is_file() || meta.len() > 512 * 1024 {
            return Ok(0);
        }
        let content = match std::fs::read_to_string(asset_path) {
            Ok(c) => c,
            Err(_) => return Ok(0),
        };
        let path_str = asset_path.to_string_lossy().to_string();
        if let Err(e) = self.storage.delete_auto_links_for_source(&path_str) {
            eprintln!("[Links][WARN] 清理旧自动链接失败 ({}): {}", path_str, e);
        }
        self.discover_links_from_script(&path_str, &content)
    }

    /// 自动分析脚本/代码文件内容中的跨资产引用或服务依赖 (支持精准行号、切片与多语言语法抽取)
    pub fn discover_links_from_script(&self, asset_path: &str, content: &str) -> Result<usize> {
        let _wait_guard = crate::body::WaitEventGuard::enter(crate::body::WaitEventId::CpuLinkRegex);
        // 测试目录内的集成测试夹具（非 /tmp/ 沙盒）不纳入宿主拓扑图
        if !asset_path.starts_with("/tmp/")
            && (asset_path.contains("/tests/")
                || asset_path.ends_with("_test.rs")
                || asset_path.ends_with("_test.py"))
        {
            return Ok(0);
        }

        let mut count = 0;
        let is_rust = asset_path.ends_with(".rs");
        let is_python = asset_path.ends_with(".py");
        let is_shell = asset_path.ends_with(".sh") || asset_path.ends_with(".bash");
        let is_js = asset_path.ends_with(".js") || asset_path.ends_with(".ts");
        let is_md = asset_path.ends_with(".md");
        let is_data = is_pure_data_file(asset_path);

        // 编译常用提取正则
        use anyhow::Context;
        let path_re = Regex::new(r#"(/root/new/[a-zA-Z0-9_\-\./]+)"#).context("编译 path_re 失败")?;
        let py_import_re = Regex::new(r#"^(?:from\s+([a-zA-Z0-9_\.]+)\s+import|import\s+([a-zA-Z0-9_\.]+))"#).context("编译 py_import_re 失败")?;
        let sh_source_re = Regex::new(r#"^(?:source|\.)\s+["']?([^"'\s]+)"#).context("编译 sh_source_re 失败")?;
        let sh_exec_re = Regex::new(r#"^(?:bash|sh|python3|python)\s+["']?([^"'\s]+)"#).context("编译 sh_exec_re 失败")?;
        let js_import_re = Regex::new(r#"(?:require\(|from\s+)["']([^"']+)["']"#).context("编译 js_import_re 失败")?;

        for (line_idx, line) in content.lines().enumerate() {
            let line_num = (line_idx + 1) as i32;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            // Rust 源码进入 #[cfg(test)] 单元测试块后立即停止抽取，防止测试字符串假路径污染生产拓扑
            if is_rust && trimmed.starts_with("#[cfg(test)]") {
                break;
            }

            let snippet_str = if trimmed.chars().count() > 120 {
                let s: String = trimmed.chars().take(120).collect();
                format!("{}...", s)
            } else {
                trimmed.to_string()
            };

            // 1. 语法级 Python 模块导入 (from xxx import ... 或 import xxx)
            if is_python {
                if let Some(cap) = py_import_re.captures(trimmed) {
                    let mod_name = cap.get(1).or_else(|| cap.get(2)).map(|m| m.as_str()).unwrap_or("");
                    let top_pkg = mod_name.split('.').next().unwrap_or("");
                    let is_stdlib_or_3p = matches!(
                        top_pkg,
                        "os" | "sys" | "json" | "time" | "re" | "math" | "io" | "argparse"
                            | "hashlib" | "collections" | "urllib" | "pathlib" | "datetime"
                            | "subprocess" | "logging" | "sqlite3" | "typing" | "functools"
                            | "itertools" | "random" | "string" | "shutil" | "tempfile"
                            | "glob" | "csv" | "traceback" | "threading" | "multiprocessing"
                            | "asyncio" | "socket" | "http" | "email" | "xml" | "html"
                            | "unittest" | "dataclasses" | "enum" | "abc" | "copy" | "pprint"
                            | "textwrap" | "inspect" | "importlib" | "warnings" | "contextlib"
                            | "concurrent" | "queue" | "heapq" | "bisect" | "struct"
                            | "codecs" | "base64" | "hmac" | "secrets" | "uuid" | "zlib"
                            | "gzip" | "tarfile" | "zipfile" | "signal" | "errno" | "stat"
                            | "fcntl" | "select" | "ssl" | "urllib3" | "requests" | "numpy"
                            | "pandas" | "scipy" | "torch" | "pydantic" | "fastapi" | "uvicorn"
                            | "flask" | "pytest" | "yaml" | "toml" | "dotenv" | "rich"
                            | "click" | "tqdm" | "httpx" | "aiohttp" | "bs4" | "lxml"
                            | "playwright" | "selenium" | "psycopg2" | "psycopg" | "duckdb"
                            | "openai" | "anthropic" | "PIL" | "cv2" | "matplotlib" | "__future__"
                    );
                    if !mod_name.is_empty() && !is_stdlib_or_3p {
                        let target_candidate = resolve_python_import_target(asset_path, mod_name);
                        if let Some(target_id) = target_candidate {
                            let link = LinkRecord {
                                id: None,
                                source_type: "asset".to_string(),
                                source_id: asset_path.to_string(),
                                target_type: "asset".to_string(),
                                target_id,
                                relation: "depends_on".to_string(),
                                confidence: 0.95,
                                auto_discovered: true,
                                line_number: Some(line_num),
                                snippet: Some(snippet_str.clone()),
                                provenance: Some("ast_import".to_string()),
                                created_at: None,
                            };
                            if let Err(e) = self.storage.add_link(&link) {
                                eprintln!("[Links][WARN] 写入依赖边失败 ({} -> {}): {}", link.source_id, link.target_id, e);
                            }
                            count += 1;
                        }
                    }
                }
            }

            // 2. 语法级 Shell source 或外部脚本调用
            if is_shell {
                if let Some(cap) = sh_source_re.captures(trimmed).or_else(|| sh_exec_re.captures(trimmed)) {
                let script_target = cap[1].to_string();
                if script_target.starts_with("/root/new/") || script_target.ends_with(".sh") || script_target.ends_with(".py") {
                    let link = LinkRecord {
                        id: None,
                        source_type: "asset".to_string(),
                        source_id: asset_path.to_string(),
                        target_type: "asset".to_string(),
                        target_id: script_target,
                        relation: "depends_on".to_string(),
                        confidence: 0.95,
                        auto_discovered: true,
                        line_number: Some(line_num),
                        snippet: Some(snippet_str.clone()),
                        provenance: Some("ast_import".to_string()),
                        created_at: None,
                    };
                    if let Err(e) = self.storage.add_link(&link) {
                        eprintln!("[Links][WARN] 写入依赖边失败 ({} -> {}): {}", link.source_id, link.target_id, e);
                    }
                    count += 1;
                }
                }
            }

            // 3. 语法级 Node.js / JS import & require
            if is_js {
                if let Some(cap) = js_import_re.captures(trimmed) {
                let js_target = cap[1].to_string();
                if js_target.starts_with("/root/new/") || js_target.starts_with('.') {
                    let link = LinkRecord {
                        id: None,
                        source_type: "asset".to_string(),
                        source_id: asset_path.to_string(),
                        target_type: "asset".to_string(),
                        target_id: js_target,
                        relation: "depends_on".to_string(),
                        confidence: 0.9,
                        auto_discovered: true,
                        line_number: Some(line_num),
                        snippet: Some(snippet_str.clone()),
                        provenance: Some("ast_import".to_string()),
                        created_at: None,
                    };
                    if let Err(e) = self.storage.add_link(&link) {
                        eprintln!("[Links][WARN] 写入依赖边失败 ({} -> {}): {}", link.source_id, link.target_id, e);
                    }
                    count += 1;
                }
                }
            }

            // 4. 绝对路径正则探测 (/root/new/...)，严格过滤模板截断、占位符与临时输出路径
            for m in path_re.find_iter(line) {
                let matched_path = m.as_str();
                let trailing_char = line[m.end()..].chars().next();
                if is_non_static_link_target(matched_path, trailing_char) {
                    continue;
                }
                if is_test_or_example_link(asset_path, matched_path, Some(&snippet_str)) {
                    continue;
                }
                if is_md && matched_path.ends_with('/') && !std::path::Path::new(matched_path).exists() {
                    continue;
                }
                if matched_path != asset_path {
                    let link = LinkRecord {
                        id: None,
                        source_type: "asset".to_string(),
                        source_id: asset_path.to_string(),
                        target_type: "asset".to_string(),
                        target_id: matched_path.to_string(),
                        relation: "references".to_string(),
                        confidence: 0.9,
                        auto_discovered: true,
                        line_number: Some(line_num),
                        snippet: Some(snippet_str.clone()),
                        provenance: Some("path_regex".to_string()),
                        created_at: None,
                    };
                    if let Err(e) = self.storage.add_link(&link) {
                        eprintln!("[Links][WARN] 写入依赖边失败 ({} -> {}): {}", link.source_id, link.target_id, e);
                    }
                    count += 1;
                }
            }

            // 5. 常见数据库与网络服务依赖（纯数据文件如 .json/.csv、.md 文档以及链接检测器自身源码跳过字面量误判）
            let skip_port_detect = is_data || is_md || asset_path.ends_with("/links/mod.rs") || trimmed.starts_with("target_id:");
            if !skip_port_detect && (line.contains("5432") || line.contains("postgresql://")) {
                let link = LinkRecord {
                    id: None,
                    source_type: "asset".to_string(),
                    source_id: asset_path.to_string(),
                    target_type: "service".to_string(),
                    target_id: "postgresql:5432".to_string(),
                    relation: "depends_on".to_string(),
                    confidence: 0.88,
                    auto_discovered: true,
                    line_number: Some(line_num),
                    snippet: Some(snippet_str.clone()),
                    provenance: Some("port_detect".to_string()),
                    created_at: None,
                };
                if let Err(e) = self.storage.add_link(&link) {
                    eprintln!("[Links][WARN] 写入依赖边失败 ({} -> {}): {}", link.source_id, link.target_id, e);
                }
                count += 1;
            }

            if !skip_port_detect && (line.contains("11434") || line.contains("ollama")) {
                let link = LinkRecord {
                    id: None,
                    source_type: "asset".to_string(),
                    source_id: asset_path.to_string(),
                    target_type: "service".to_string(),
                    target_id: "ollama:11434".to_string(),
                    relation: "depends_on".to_string(),
                    confidence: 0.88,
                    auto_discovered: true,
                    line_number: Some(line_num),
                    snippet: Some(snippet_str.clone()),
                    provenance: Some("port_detect".to_string()),
                    created_at: None,
                };
                if let Err(e) = self.storage.add_link(&link) {
                    eprintln!("[Links][WARN] 写入依赖边失败 ({} -> {}): {}", link.source_id, link.target_id, e);
                }
                count += 1;
            }

            if !skip_port_detect && (line.contains("6379") || line.contains("redis://")) {
                let link = LinkRecord {
                    id: None,
                    source_type: "asset".to_string(),
                    source_id: asset_path.to_string(),
                    target_type: "service".to_string(),
                    target_id: "redis:6379".to_string(),
                    relation: "depends_on".to_string(),
                    confidence: 0.88,
                    auto_discovered: true,
                    line_number: Some(line_num),
                    snippet: Some(snippet_str.clone()),
                    provenance: Some("port_detect".to_string()),
                    created_at: None,
                };
                if let Err(e) = self.storage.add_link(&link) {
                    eprintln!("[Links][WARN] 写入依赖边失败 ({} -> {}): {}", link.source_id, link.target_id, e);
                }
                count += 1;
            }
        }

        Ok(count)
    }

    /// 导出实体关系图为 Mermaid 图表格式
    pub fn export_mermaid(&self, limit: usize) -> Result<String> {
        let links = self.storage.list_all_links(limit)?;
        if links.is_empty() {
            return Ok("```mermaid\ngraph LR\n    empty[\"暂无关联数据\"]\n```".to_string());
        }

        let mut lines = Vec::new();
        lines.push("```mermaid".to_string());
        lines.push("graph LR".to_string());

        for (idx, link) in links.iter().enumerate() {
            let src_label = sanitize_label(&link.source_id);
            let tgt_label = sanitize_label(&link.target_id);
            let edge = match link.relation.as_str() {
                "depends_on" => "-->|depends_on|",
                "references" => "-.->|references|",
                "produced_by" => "==>|produced_by|",
                _ => "---",
            };
            lines.push(format!("    node_s{}[\"{}\"] {} node_t{}[\"{}\"]", idx, src_label, edge, idx, tgt_label));
        }

        lines.push("```".to_string());
        Ok(lines.join("\n"))
    }
}

fn resolve_python_import_target(asset_path: &str, mod_name: &str) -> Option<String> {
    if mod_name.starts_with("shared_proj") {
        let rel = mod_name.replace('.', "/");
        let py_file = format!("/root/new/{}.py", rel);
        if std::path::Path::new(&py_file).exists() {
            return Some(py_file);
        }
        return Some(format!("/root/new/{}", rel));
    }
    let mod_rel = mod_name.replace('.', "/");
    // 1. 优先检查脚本所在同级目录是否存在 <mod>.py 或 <mod>/__init__.py
    if let Some(parent) = std::path::Path::new(asset_path).parent() {
        let sibling_py = parent.join(format!("{}.py", mod_rel));
        if sibling_py.exists() {
            return Some(sibling_py.to_string_lossy().to_string());
        }
        let sibling_pkg = parent.join(&mod_rel).join("__init__.py");
        if sibling_pkg.exists() {
            return Some(sibling_pkg.to_string_lossy().to_string());
        }
    }
    // 2. 其次检查 /root/new/shared_proj/ 下是否存在对应模块
    let shared_py = format!("/root/new/shared_proj/{}.py", mod_rel);
    if std::path::Path::new(&shared_py).exists() {
        return Some(shared_py);
    }
    let shared_dir = format!("/root/new/shared_proj/{}", mod_rel);
    if std::path::Path::new(&shared_dir).exists() {
        return Some(shared_dir);
    }
    None
}

fn sanitize_label(text: &str) -> String {
    text.replace('\"', "'").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_query_links() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let engine = LinksEngine::new(storage);

        engine.create_link("asset", "/root/new/work/proj_a", "asset", "/root/new/shared_proj/helper", "depends_on", 1.0).unwrap();
        engine.create_link("asset", "/root/new/work/proj_a", "service", "postgresql:5432", "depends_on", 0.9).unwrap();

        let links = engine.query_links("asset", "/root/new/work/proj_a").unwrap();
        assert_eq!(links.len(), 2);

        let mermaid = engine.export_mermaid(10).unwrap();
        assert!(mermaid.contains("graph LR"));
        assert!(mermaid.contains("depends_on"));
    }

    #[test]
    fn test_discover_links_with_line_numbers_and_provenance() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let engine = LinksEngine::new(storage);

        let script_code = r#"# Line 1: Comment
import os
from shared_proj.sec_fetcher import utils
# Line 4: Note read
with open("/root/new/note/test.md") as f:
    pass
# Line 7: DB Connect
conn = "postgresql://localhost:5432/mydb"
"#;
        let count = engine.discover_links_from_script("/root/new/work/demo.py", script_code).unwrap();
        assert!(count >= 3, "应当识别到 import, 路径引用以及 postgres 依赖");

        let links = engine.query_links("asset", "/root/new/work/demo.py").unwrap();
        let sec_link = links.iter().find(|l| l.target_id.contains("sec_fetcher")).expect("sec_fetcher link");
        assert_eq!(sec_link.line_number, Some(3));
        assert_eq!(sec_link.provenance.as_deref(), Some("ast_import"));
        assert!(sec_link.snippet.as_ref().unwrap().contains("from shared_proj"));

        let note_link = links.iter().find(|l| l.target_id == "/root/new/note/test.md").expect("note link");
        assert_eq!(note_link.line_number, Some(5));
        assert_eq!(note_link.provenance.as_deref(), Some("path_regex"));

        let pg_link = links.iter().find(|l| l.target_id == "postgresql:5432").expect("pg link");
        assert_eq!(pg_link.line_number, Some(8));
        assert_eq!(pg_link.provenance.as_deref(), Some("port_detect"));
    }

    #[test]
    fn test_refresh_links_for_asset_idempotent() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let engine = LinksEngine::new(storage.clone());

        let tmp = tempfile::tempdir().expect("tempdir");
        let script_path = tmp.path().join("job.py");
        std::fs::write(&script_path, "conn = 'postgresql://127.0.0.1:5432/db'\n").unwrap();

        let n1 = engine.refresh_links_for_asset(&script_path).unwrap();
        assert_eq!(n1, 1);

        // 修改文件去掉 5432 改为 11434 (ollama)，再次刷新应自动清理旧 auto_discovered 边
        std::fs::write(&script_path, "url = 'http://127.0.0.1:11434/api/tags'\n").unwrap();
        let n2 = engine.refresh_links_for_asset(&script_path).unwrap();
        assert_eq!(n2, 1);

        let links = engine.query_links("asset", &script_path.to_string_lossy()).unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target_id, "ollama:11434");
    }

    #[test]
    fn test_discover_links_utf8_long_chinese_line_no_panic() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let engine = LinksEngine::new(storage.clone());
        let long_cn = format!(
            "# {} 这是一段非常长的中文注释用于测试超过一百四十字节边界时的安全截断 /root/new/shared_proj/a.py",
            "存储与依赖拓扑".repeat(12)
        );
        let cnt = engine.discover_links_from_script("/root/new/work/cn_test.md", &long_cn).unwrap();
        assert_eq!(cnt, 1);
    }

    #[test]
    fn test_discover_links_filters_templates_placeholders_and_json_ports() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let engine = LinksEngine::new(storage.clone());

        let py_code = r#"
p1 = "/root/new/shared_proj/sat_%s_%s.json" % (a, b)
p2 = f"/root/new/cron/earnings-analysis/_scratch/pool_timing_{run}.json"
p3 = "/root/new/tmp/kvaxis/warm1_%s.log" % tag
p4 = "/root/new/video/xxx"
p5 = "/root/new/skills/..."
p6 = "/root/new/new_tmp_placeholder"
p7 = "/root/new/cron/introspections/YYYY-MM-DD-HH-hot.md"
p8 = "/root/new/work/KFS_articles_" + suffix
"#;
        let cnt_py = engine
            .discover_links_from_script("/root/new/shared_proj/sample.py", py_code)
            .unwrap();
        assert_eq!(cnt_py, 0, "模板/占位符/临时路径不应被抽取为静态依赖边");

        let json_data = r#"[{"job":"x","tot":6379,"port":5432,"ts":11434}]"#;
        let cnt_json = engine
            .discover_links_from_script("/root/new/shared_proj/data.json", json_data)
            .unwrap();
        assert_eq!(cnt_json, 0, "纯数据 JSON 中的数字不应误判为服务端口依赖");
    }

    #[test]
    fn test_discover_links_ignores_rust_tests_comments_and_md_examples() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let engine = LinksEngine::new(storage.clone());

        let rs_code = r##"
/// 示例路径: /root/new/shared_proj/doc_example.py
fn helper() {
    let html = r#"<input placeholder="/root/new/shared_proj/my_script.py" />"#;
    let _ = html;
}

#[cfg(test)]
mod tests {
    #[test]
    fn fake_test() {
        let p = "/root/new/work/fake_unit_test_fixture.rs";
        assert!(!p.is_empty());
    }
}
"##;
        let cnt_rs = engine
            .discover_links_from_script("/root/new/work/agent-body/src/demo_mod.rs", rs_code)
            .unwrap();
        assert_eq!(cnt_rs, 0, "Rust 注释、placeholder 与 #[cfg(test)] 内假路径不应入图");

        let cnt_tests_dir = engine
            .discover_links_from_script(
                "/root/new/work/agent-body/tests/integration_fixture.rs",
                "let x = \"/root/new/work/fake_integ.rs\";",
            )
            .unwrap();
        assert_eq!(cnt_tests_dir, 0, "/tests/ 夹具不应纳入生产拓扑");

        let md_example = "示例 CLI:\n`agent-body links impact /root/new/shared_proj/nonexistent_demo.py`\n如 redis:6379 被误判";
        let tmp_md = std::env::temp_dir().join(format!("agent_body_md_test_{}.md", uuid::Uuid::new_v4()));
        std::fs::write(&tmp_md, md_example).expect("write tmp md");
        let cnt_md = engine
            .discover_links_from_script(&tmp_md.to_string_lossy(), md_example)
            .unwrap();
        let _ = std::fs::remove_file(&tmp_md);
        assert_eq!(cnt_md, 0, "真实 .md 中的 CLI 示例与端口文本不应产生假边");
    }
}
