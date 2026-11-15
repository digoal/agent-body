use std::sync::Arc;
use regex::Regex;
use crate::storage::{Storage, models::LinkRecord};
use anyhow::Result;

pub mod impact;

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
            created_at: None,
        };
        self.storage.add_link(&link)
    }

    /// 查询某实体的关联关系
    pub fn query_links(&self, node_type: &str, node_id: &str) -> Result<Vec<LinkRecord>> {
        self.storage.query_links_for_node(node_type, node_id)
    }

    /// 自动分析脚本/代码文件内容中的跨资产引用或服务依赖 (例如 import 模块、数据库端口、curl API 等)
    pub fn discover_links_from_script(&self, asset_path: &str, content: &str) -> Result<usize> {
        let mut count = 0;

        // 1. 匹配 Python/Node/Shell 中的绝对路径引用 (如 /root/new/shared_proj/xxx 或 /root/new/note/xxx)
        let path_re = Regex::new(r#"(/root/new/[a-zA-Z0-9_\-\./]+)"#).unwrap();
        for cap in path_re.captures_iter(content) {
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
                    created_at: None,
                };
                let _ = self.storage.add_link(&link);
                count += 1;
            }
        }

        // 2. 匹配常见服务端口/连接串 (如 5432: postgresql, 6379: redis, 11434: ollama)
        if content.contains("5432") || content.contains("postgresql://") {
            let link = LinkRecord {
                id: None,
                source_type: "asset".to_string(),
                source_id: asset_path.to_string(),
                target_type: "service".to_string(),
                target_id: "postgresql:5432".to_string(),
                relation: "depends_on".to_string(),
                confidence: 0.85,
                auto_discovered: true,
                created_at: None,
            };
            let _ = self.storage.add_link(&link);
            count += 1;
        }

        if content.contains("11434") || content.contains("ollama") {
            let link = LinkRecord {
                id: None,
                source_type: "asset".to_string(),
                source_id: asset_path.to_string(),
                target_type: "service".to_string(),
                target_id: "ollama:11434".to_string(),
                relation: "depends_on".to_string(),
                confidence: 0.85,
                auto_discovered: true,
                created_at: None,
            };
            let _ = self.storage.add_link(&link);
            count += 1;
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
    fn test_discover_links_from_script() {
        let storage = Arc::new(Storage::open_in_memory().expect("open_in_memory"));
        let engine = LinksEngine::new(storage);

        let script_code = r#"
import os
import psycopg2
# Read from shared note
with open("/root/new/note/test.md") as f:
    pass
conn = psycopg2.connect(host="localhost", port=5432)
"#;
        let count = engine.discover_links_from_script("/root/new/work/demo.py", script_code).unwrap();
        assert!(count >= 2, "应至少识别到路径引用和 5432 依赖");

        let links = engine.query_links("asset", "/root/new/work/demo.py").unwrap();
        assert!(links.iter().any(|l| l.target_id == "/root/new/note/test.md"));
        assert!(links.iter().any(|l| l.target_id == "postgresql:5432"));
    }
}
