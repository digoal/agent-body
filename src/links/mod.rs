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
        let _ = self.storage.delete_auto_links_for_source(&path_str);
        self.discover_links_from_script(&path_str, &content)
    }

    /// 自动分析脚本/代码文件内容中的跨资产引用或服务依赖 (支持精准行号、切片与多语言语法抽取)
    pub fn discover_links_from_script(&self, asset_path: &str, content: &str) -> Result<usize> {
        let mut count = 0;
        let is_python = asset_path.ends_with(".py");
        let is_shell = asset_path.ends_with(".sh") || asset_path.ends_with(".bash");
        let is_js = asset_path.ends_with(".js") || asset_path.ends_with(".ts");

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
                            let _ = self.storage.add_link(&link);
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
                    let _ = self.storage.add_link(&link);
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
                    let _ = self.storage.add_link(&link);
                    count += 1;
                }
                }
            }

            // 4. 绝对路径正则探测 (/root/new/...)
            for cap in path_re.captures_iter(line) {
                let matched_path = &cap[1];
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
                    let _ = self.storage.add_link(&link);
                    count += 1;
                }
            }

            // 5. 常见数据库与网络服务依赖
            if line.contains("5432") || line.contains("postgresql://") {
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
                let _ = self.storage.add_link(&link);
                count += 1;
            }

            if line.contains("11434") || line.contains("ollama") {
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
                let _ = self.storage.add_link(&link);
                count += 1;
            }

            if line.contains("6379") || line.contains("redis://") {
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
                let _ = self.storage.add_link(&link);
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
}
