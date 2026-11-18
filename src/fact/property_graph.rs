use std::path::Path;
use anyhow::Result;
use crate::storage::Storage;
use crate::storage::models::{FactNodeRecord, FactEdgeRecord, SubGraph};
use crate::fact::capability::CapabilityEngine;
use crate::fact::process_net::ProcessNetTopology;
use crate::fact::automation::AutomationTopology;
use crate::fact::assets_catalog::AssetsCatalogEngine;

/// 宿主机层次化属性图组织器 (Property Graph Organizer)
pub struct PropertyGraphOrganizer {
    storage: Storage,
}

impl PropertyGraphOrganizer {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// 从宿主机事实全量构建或增量刷新属性图 (返回 (新增或更新节点数, 新增或更新边数))
    pub fn build_host_graph(&self) -> Result<(usize, usize)> {
        let mut node_count = 0usize;
        let mut edge_count = 0usize;

        // 1. 挖掘系统源码工程节点 (Projects)
        if let Ok(repos) = AssetsCatalogEngine::scan_repos(None) {
            for r in repos {
                let node_id = format!("project:{}", r.name);
                let node = FactNodeRecord {
                    id: node_id.clone(),
                    node_type: "project".to_string(),
                    name: r.name.clone(),
                    label: format!("工程仓库: {}", r.name),
                    properties: serde_json::json!({
                        "abs_path": r.abs_path,
                        "codegraph_ready": r.codegraph_ready,
                        "current_branch": r.current_branch,
                        "primary_lang": r.primary_lang,
                    }),
                    updated_at: None,
                };
                self.storage.upsert_fact_node(&node)?;
                node_count += 1;
            }
        }

        // 2. 挖掘运行时端口与服务拓扑 (Ports & Services)
        if let Ok(ports) = ProcessNetTopology::scan_listening_ports() {
            for p in ports {
                let port_id = format!("port:{}", p.port);
                let port_node = FactNodeRecord {
                    id: port_id.clone(),
                    node_type: "port".to_string(),
                    name: p.port.to_string(),
                    label: format!("{} 端口 {}", p.protocol.to_uppercase(), p.port),
                    properties: serde_json::json!({
                        "protocol": p.protocol,
                        "bind_ip": p.bind_ip,
                    }),
                    updated_at: None,
                };
                self.storage.upsert_fact_node(&port_node)?;
                node_count += 1;

                if let Some(ref proc_name) = p.process_name {
                    let service_id = format!("service:{}", proc_name);
                    let service_node = FactNodeRecord {
                        id: service_id.clone(),
                        node_type: "service".to_string(),
                        name: proc_name.clone(),
                        label: format!("服务进程: {}", proc_name),
                        properties: serde_json::json!({
                            "pid": p.pid,
                            "command": p.command,
                            "cwd": p.cwd,
                            "user": p.user,
                        }),
                        updated_at: None,
                    };
                    self.storage.upsert_fact_node(&service_node)?;
                    node_count += 1;

                    // 边: service -> port (listens_on)
                    let edge = FactEdgeRecord {
                        id: None,
                        source_id: service_id.clone(),
                        target_id: port_id.clone(),
                        relation: "listens_on".to_string(),
                        properties: serde_json::json!({"pid": p.pid}),
                        confidence: 1.0,
                        updated_at: None,
                    };
                    self.storage.upsert_fact_edge(&edge)?;
                    edge_count += 1;

                    // 若 cwd 包含工程路径，关联 service -> project (depends_on)
                    if let Some(ref cwd) = p.cwd {
                        for prefix in &["/root/new/work/", "/root/new/shared_proj/"] {
                            if cwd.starts_with(prefix) {
                                let sub = &cwd[prefix.len()..];
                                let proj_name = sub.split('/').next().unwrap_or("");
                                if !proj_name.is_empty() {
                                    let proj_id = format!("project:{}", proj_name);
                                    let proj_edge = FactEdgeRecord {
                                        id: None,
                                        source_id: service_id.clone(),
                                        target_id: proj_id,
                                        relation: "depends_on".to_string(),
                                        properties: serde_json::json!({"path": cwd}),
                                        confidence: 0.95,
                                        updated_at: None,
                                    };
                                    let _ = self.storage.upsert_fact_edge(&proj_edge);
                                    edge_count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. 挖掘 Systemd 服务与 Cron 定时调度 (Automation)
        if let Ok(services) = AutomationTopology::scan_systemd_services() {
            for s in services {
                let sid = format!("service:{}", s.service_name);
                let snode = FactNodeRecord {
                    id: sid.clone(),
                    node_type: "service".to_string(),
                    name: s.service_name.clone(),
                    label: format!("Systemd服务: {}", s.service_name),
                    properties: serde_json::json!({
                        "unit_file_path": s.unit_file_path,
                        "description": s.description,
                        "exec_start": s.exec_start,
                        "working_directory": s.working_directory,
                        "user": s.user,
                    }),
                    updated_at: None,
                };
                self.storage.upsert_fact_node(&snode)?;
                node_count += 1;

                // 关联依赖文件与引用文件
                for dep in s.referenced_files {
                    let fid = format!("file:{}", dep);
                    let fnode = FactNodeRecord {
                        id: fid.clone(),
                        node_type: "file".to_string(),
                        name: Path::new(&dep).file_name().and_then(|f| f.to_str()).unwrap_or(&dep).to_string(),
                        label: format!("配置/执行依赖: {}", dep),
                        properties: serde_json::json!({"full_path": dep}),
                        updated_at: None,
                    };
                    self.storage.upsert_fact_node(&fnode)?;
                    node_count += 1;

                    let edge = FactEdgeRecord {
                        id: None,
                        source_id: sid.clone(),
                        target_id: fid,
                        relation: "depends_on".to_string(),
                        properties: serde_json::json!({}),
                        confidence: 1.0,
                        updated_at: None,
                    };
                    self.storage.upsert_fact_edge(&edge)?;
                    edge_count += 1;
                }
            }
        }

        if let Ok(crons) = AutomationTopology::scan_cron_jobs() {
            for (i, c) in crons.into_iter().enumerate() {
                let cid = format!("cron:{}_{}", c.user, i);
                let cnode = FactNodeRecord {
                    id: cid.clone(),
                    node_type: "cron".to_string(),
                    name: format!("{}-job-{}", c.user, i),
                    label: format!("Cron 定时调度: {}", c.schedule),
                    properties: serde_json::json!({
                        "user": c.user,
                        "schedule": c.schedule,
                        "command": c.command,
                    }),
                    updated_at: None,
                };
                self.storage.upsert_fact_node(&cnode)?;
                node_count += 1;

                if let Some(target_script) = c.target_script {
                    let fid = format!("file:{}", target_script);
                    let fnode = FactNodeRecord {
                        id: fid.clone(),
                        node_type: "file".to_string(),
                        name: Path::new(&target_script).file_name().and_then(|f| f.to_str()).unwrap_or(&target_script).to_string(),
                        label: format!("Cron脚本: {}", target_script),
                        properties: serde_json::json!({"full_path": target_script}),
                        updated_at: None,
                    };
                    self.storage.upsert_fact_node(&fnode)?;
                    node_count += 1;

                    let edge = FactEdgeRecord {
                        id: None,
                        source_id: cid,
                        target_id: fid,
                        relation: "scheduled_by".to_string(),
                        properties: serde_json::json!({"schedule": c.schedule}),
                        confidence: 1.0,
                        updated_at: None,
                    };
                    self.storage.upsert_fact_edge(&edge)?;
                    edge_count += 1;
                }
            }
        }

        // 4. 挖掘核心 CLI 工具能力 (Tools)
        let mut cap_engine = CapabilityEngine::new();
        cap_engine.scan_host();
        for tool in cap_engine.all_capabilities() {
            let tid = format!("tool:{}", tool.tool_name);
            let tnode = FactNodeRecord {
                id: tid,
                node_type: "tool".to_string(),
                name: tool.tool_name.clone(),
                label: format!("CLI工具: {}", tool.tool_name),
                properties: serde_json::json!({
                    "category": tool.category,
                    "description": tool.description,
                    "installed": tool.installed,
                    "best_practice_args": tool.best_practice_args,
                }),
                updated_at: None,
            };
            self.storage.upsert_fact_node(&tnode)?;
            node_count += 1;
        }

        Ok((node_count, edge_count))
    }

    /// 查询以指定实体为核心的 N 跳关系子图
    pub fn query_subgraph(&self, query: &str, max_hops: usize) -> Result<SubGraph> {
        let trimmed = query.trim();
        // 尝试精确匹配 ID
        if let Some(node) = self.storage.get_fact_node(trimmed)? {
            return self.storage.traverse_fact_graph(&node.id, max_hops);
        }

        // 模糊搜索候选节点
        let candidates = self.storage.find_fact_nodes_by_name(trimmed)?;
        if let Some(first) = candidates.first() {
            self.storage.traverse_fact_graph(&first.id, max_hops)
        } else {
            Ok(SubGraph::default())
        }
    }

    /// 将子图格式化为适合 Agent 阅读的结构化 Markdown 与 Mermaid 关系图
    pub fn format_graph_report(&self, subgraph: &SubGraph) -> String {
        let mut out = String::new();
        out.push_str("# 🕸️ 宿主机实体属性图关联透视 (Property Graph)\n\n");

        if subgraph.nodes.is_empty() {
            out.push_str("未检索到与查询匹配的实体图节点。可尝试执行全局事实构建或换用项目名/端口号/服务名重新检索。\n");
            return out;
        }

        if let Some(ref root) = subgraph.root_node {
            out.push_str(&format!("**根探测节点**: `{}` ({}) - 类型: `{}`\n\n", root.name, root.label, root.node_type));
        }

        out.push_str(&format!("- **关联节点总数**: {}\n", subgraph.nodes.len()));
        out.push_str(&format!("- **实体关联边数**: {}\n\n", subgraph.edges.len()));

        out.push_str("### 📋 关联实体节点清单\n\n");
        out.push_str("| 节点 ID | 类型 | 实体名称 | 标签与语义 |\n");
        out.push_str("|---------|------|----------|------------|\n");
        for n in &subgraph.nodes {
            out.push_str(&format!("| `{}` | `{}` | `{}` | {} |\n", n.id, n.node_type, n.name, n.label));
        }
        out.push_str("\n");

        if !subgraph.edges.is_empty() {
            out.push_str("### 🔗 关系依赖拓扑\n\n");
            out.push_str("| 源实体 | 关系类型 | 目标实体 | 关系置信度 |\n");
            out.push_str("|--------|:--------:|----------|:----------:|\n");
            for e in &subgraph.edges {
                out.push_str(&format!("| `{}` | **-- {} -->** | `{}` | {:.2} |\n", e.source_id, e.relation, e.target_id, e.confidence));
            }
            out.push_str("\n");
        }

        out.push_str("### 📊 可视化拓扑关系图 (Mermaid)\n\n");
        out.push_str("```mermaid\n");
        out.push_str(&self.render_mermaid(subgraph));
        out.push_str("\n```\n");

        out
    }

    /// 将子图渲染为规整、美观、带有中文注释与安全框线的 Mermaid 流程图
    pub fn render_mermaid(&self, subgraph: &SubGraph) -> String {
        let mut lines = Vec::new();
        lines.push("flowchart LR".to_string());

        // 节点声明与样式
        for n in &subgraph.nodes {
            let clean_id = sanitize_mermaid_id(&n.id);
            let clean_label = n.name.replace('"', "").replace('(', "").replace(')', "");
            let shape = match n.node_type.as_str() {
                "project" => format!("[\"📦 {}\"]", clean_label),
                "service" => format!("([\"⚙️ {}\"])", clean_label),
                "port" => format!("{{\"🔌 端口:{}\"}}", clean_label),
                "cron" => format!("{{\"⏰ {}\"}}", clean_label),
                "tool" => format!("[\"🛠️ {}\"]", clean_label),
                "file" => format!("[\"📄 {}\"]", clean_label),
                _ => format!("[\"{}\"]", clean_label),
            };
            lines.push(format!("    {}{}", clean_id, shape));
        }

        // 边关系
        for e in &subgraph.edges {
            let src = sanitize_mermaid_id(&e.source_id);
            let tgt = sanitize_mermaid_id(&e.target_id);
            let arrow = match e.relation.as_str() {
                "listens_on" => "-->|\"监听\"|",
                "depends_on" => "-->|\"依赖\"|",
                "scheduled_by" => "-->|\"调度\"|",
                "part_of" => "-->|\"归属\"|",
                "reads" => "-->|\"读取\"|",
                "writes" => "-->|\"写入\"|",
                _ => "-->",
            };
            lines.push(format!("    {} {} {}", src, arrow, tgt));
        }

        lines.join("\n")
    }
}

fn sanitize_mermaid_id(raw: &str) -> String {
    raw.replace(':', "_")
        .replace('.', "_")
        .replace('/', "_")
        .replace('-', "_")
        .replace('@', "_")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_graph_organizer_and_mermaid_rendering() {
        let storage = Storage::open_in_memory().expect("open_in_memory");
        let organizer = PropertyGraphOrganizer::new(storage.clone());

        // 手工注入测试节点与边
        let n1 = FactNodeRecord {
            id: "project:agent-steward".to_string(),
            node_type: "project".to_string(),
            name: "agent-steward".to_string(),
            label: "宿主大管家工程".to_string(),
            properties: serde_json::json!({}),
            updated_at: None,
        };
        let n2 = FactNodeRecord {
            id: "service:agent-steward".to_string(),
            node_type: "service".to_string(),
            name: "agent-steward".to_string(),
            label: "大管家守护进程".to_string(),
            properties: serde_json::json!({}),
            updated_at: None,
        };
        let n3 = FactNodeRecord {
            id: "port:8000".to_string(),
            node_type: "port".to_string(),
            name: "8000".to_string(),
            label: "服务端口8000".to_string(),
            properties: serde_json::json!({}),
            updated_at: None,
        };

        storage.upsert_fact_node(&n1).unwrap();
        storage.upsert_fact_node(&n2).unwrap();
        storage.upsert_fact_node(&n3).unwrap();

        storage.upsert_fact_edge(&FactEdgeRecord {
            id: None,
            source_id: "service:agent-steward".to_string(),
            target_id: "project:agent-steward".to_string(),
            relation: "depends_on".to_string(),
            properties: serde_json::json!({}),
            confidence: 1.0,
            updated_at: None,
        }).unwrap();

        storage.upsert_fact_edge(&FactEdgeRecord {
            id: None,
            source_id: "service:agent-steward".to_string(),
            target_id: "port:8000".to_string(),
            relation: "listens_on".to_string(),
            properties: serde_json::json!({}),
            confidence: 1.0,
            updated_at: None,
        }).unwrap();

        // 提取子图
        let sub = organizer.query_subgraph("agent-steward", 2).expect("query subgraph");
        assert_eq!(sub.nodes.len(), 3);
        assert_eq!(sub.edges.len(), 2);

        // 验证报告与 Mermaid 输出
        let report = organizer.format_graph_report(&sub);
        assert!(report.contains("flowchart LR"));
        assert!(report.contains("service_agent_steward"));
        assert!(report.contains("port_8000"));
        assert!(report.contains("监听"));
        assert!(report.contains("依赖"));
    }
}
