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

    /// 自动分析脚本/代码文件内容中的跨资产引用或服务依赖 (支持精准行号、切片与多语言语法抽取)
    pub fn discover_links_from_script(&self, asset_path: &str, content: &str) -> Result<usize> {
        let mut count = 0;

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

            let snippet_str = if trimmed.len() > 140 {
                format!("{}...", &trimmed[..140])
            } else {
                trimmed.to_string()
            };

            // 1. 语法级 Python 模块导入 (from xxx import ... 或 import xxx)
            if let Some(cap) = py_import_re.captures(trimmed) {
                let mod_name = cap.get(1).or_else(|| cap.get(2)).map(|m| m.as_str()).unwrap_or("");
                if !mod_name.is_empty() && !["os", "sys", "json", "time", "re", "math"].contains(&mod_name) {
                    let target_candidate = if mod_name.starts_with("shared_proj") {
                        format!("/root/new/{}", mod_name.replace('.', "/"))
                    } else {
                        format!("/root/new/shared_proj/{}", mod_name)
                    };

                    let link = LinkRecord {
                        id: None,
                        source_type: "asset".to_string(),
                        source_id: asset_path.to_string(),
                        target_type: "asset".to_string(),
                        target_id: target_candidate,
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

            // 2. 语法级 Shell source 或外部脚本调用
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

            // 3. 语法级 Node.js / JS import & require
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
}
