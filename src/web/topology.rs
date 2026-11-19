use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use crate::storage::{FactNodeRecord, StorageProvider};
use crate::web::http::{HttpRequest, HttpResponse, StatusCode};
use crate::web::api::WebResponse;

/// 拓扑节点类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum NodeType {
    Process,
    Port,
    Service,
    Cron,
    Git,
    File,
    Memory,
    Custom(String),
}

/// 拓扑节点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyNode {
    pub id: String,
    pub label: String,
    pub node_type: NodeType,
    pub status: String,
    pub metadata: serde_json::Value,
}

/// 拓扑连边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub label: Option<String>,
}

/// 拓扑图谱响应数据模型
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyGraph {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
    pub summary: TopologySummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologySummary {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub process_count: usize,
    pub port_count: usize,
    pub service_count: usize,
    pub file_count: usize,
}

/// 处理 GET /api/v1/topology
pub fn handle_topology(storage: &dyn StorageProvider) -> HttpResponse {
    let graph = build_full_topology(storage);
    WebResponse::success(graph)
}

/// 处理 GET /api/v1/topology/impact?target=xxx
pub fn handle_topology_impact(req: &HttpRequest, storage: &dyn StorageProvider) -> HttpResponse {
    let target = match req.query_params.get("target") {
        Some(t) if !t.is_empty() => t,
        _ => return WebResponse::error(StatusCode::BAD_REQUEST, "Missing required query parameter: target"),
    };

    let graph = build_impact_topology(target, storage);
    WebResponse::success(graph)
}

fn map_fact_node_type(node_type: &str) -> NodeType {
    match node_type {
        "port" => NodeType::Port,
        "service" => NodeType::Service,
        "cron" => NodeType::Cron,
        "project" => NodeType::Git,
        "file" => NodeType::File,
        "process" => NodeType::Process,
        other => NodeType::Custom(other.to_string()),
    }
}

fn fact_record_to_topology_node(gn: FactNodeRecord) -> TopologyNode {
    let node_type = map_fact_node_type(&gn.node_type);
    let status = match gn.node_type.as_str() {
        "port" => "listening".to_string(),
        _ => "active".to_string(),
    };
    TopologyNode {
        id: gn.id,
        label: gn.name,
        node_type,
        status,
        metadata: gn.properties,
    }
}

fn push_node_unique(
    nodes: &mut Vec<TopologyNode>,
    seen_nodes: &mut HashSet<String>,
    node: TopologyNode,
) -> bool {
    if seen_nodes.insert(node.id.clone()) {
        nodes.push(node);
        true
    } else {
        false
    }
}

fn push_edge_unique(
    edges: &mut Vec<TopologyEdge>,
    seen_edges: &mut HashSet<(String, String, String)>,
    edge: TopologyEdge,
) {
    let key = (edge.source.clone(), edge.target.clone(), edge.relation.clone());
    if seen_edges.insert(key) {
        edges.push(edge);
    }
}

/// 构建宿主机全景事实拓扑（整合 fact_nodes/fact_edges、活跃进程与监听端口、links 依赖边）
pub fn build_full_topology(storage: &dyn StorageProvider) -> TopologyGraph {
    let mut nodes: Vec<TopologyNode> = Vec::new();
    let mut edges: Vec<TopologyEdge> = Vec::new();
    let mut seen_nodes: HashSet<String> = HashSet::new();
    let mut seen_edges: HashSet<(String, String, String)> = HashSet::new();

    // 1. 加载属性图谱中的节点与边（优先保留所有有连边的节点，避免大量孤立节点淹没视图）
    let fact_edges = storage.list_all_fact_edges(500).unwrap_or_default();
    let mut connected_fact_ids: HashSet<String> = HashSet::new();
    for fe in &fact_edges {
        connected_fact_ids.insert(fe.source_id.clone());
        connected_fact_ids.insert(fe.target_id.clone());
    }

    let fact_nodes = storage.list_all_fact_nodes(500).unwrap_or_default();
    let mut fact_node_map: HashMap<String, FactNodeRecord> = HashMap::new();
    for gn in fact_nodes {
        fact_node_map.insert(gn.id.clone(), gn);
    }

    // 1a. 先加入所有参与了边的 fact_nodes，并建立边
    for fe in fact_edges {
        if let Some(src) = fact_node_map.get(&fe.source_id).cloned() {
            push_node_unique(&mut nodes, &mut seen_nodes, fact_record_to_topology_node(src));
        } else if !seen_nodes.contains(&fe.source_id) {
            let inferred_type = fe.source_id.split(':').next().unwrap_or("custom");
            let inferred_label = fe.source_id.split(':').nth(1).unwrap_or(&fe.source_id).to_string();
            push_node_unique(
                &mut nodes,
                &mut seen_nodes,
                TopologyNode {
                    id: fe.source_id.clone(),
                    label: inferred_label,
                    node_type: map_fact_node_type(inferred_type),
                    status: "active".to_string(),
                    metadata: serde_json::json!({}),
                },
            );
        }

        if let Some(dst) = fact_node_map.get(&fe.target_id).cloned() {
            push_node_unique(&mut nodes, &mut seen_nodes, fact_record_to_topology_node(dst));
        } else if !seen_nodes.contains(&fe.target_id) {
            let inferred_type = fe.target_id.split(':').next().unwrap_or("custom");
            let inferred_label = fe.target_id.split(':').nth(1).unwrap_or(&fe.target_id).to_string();
            push_node_unique(
                &mut nodes,
                &mut seen_nodes,
                TopologyNode {
                    id: fe.target_id.clone(),
                    label: inferred_label,
                    node_type: map_fact_node_type(inferred_type),
                    status: "active".to_string(),
                    metadata: serde_json::json!({}),
                },
            );
        }

        push_edge_unique(
            &mut edges,
            &mut seen_edges,
            TopologyEdge {
                source: fe.source_id,
                target: fe.target_id,
                relation: fe.relation.clone(),
                label: Some(fe.relation),
            },
        );
    }

    // 1b. 补充关键未连边事实节点（service / port / cron 全部保留，project 限前 25 个）
    let mut extra_projects = 0usize;
    for gn in fact_node_map.into_values() {
        if seen_nodes.contains(&gn.id) {
            continue;
        }
        match gn.node_type.as_str() {
            "service" | "port" | "cron" => {
                push_node_unique(&mut nodes, &mut seen_nodes, fact_record_to_topology_node(gn));
            }
            "project" if extra_projects < 25 => {
                extra_projects += 1;
                push_node_unique(&mut nodes, &mut seen_nodes, fact_record_to_topology_node(gn));
            }
            _ => {}
        }
    }

    // 2. 采集运行时活跃进程与监听端口（优先纳入带端口或带脚本路径的进程，上限 60 个）
    if let Ok(mut processes) = storage.list_active_processes() {
        processes.sort_by_key(|p| {
            let has_port = if p.listening_ports.is_empty() { 1 } else { 0 };
            let has_path = if p.associated_path.is_none() { 1 } else { 0 };
            (has_port, has_path)
        });

        for p in processes.into_iter().take(60) {
            let proc_id = format!("proc:{}", p.pid);
            push_node_unique(
                &mut nodes,
                &mut seen_nodes,
                TopologyNode {
                    id: proc_id.clone(),
                    label: format!("{}:{}", p.name, p.pid),
                    node_type: NodeType::Process,
                    status: p.status.clone(),
                    metadata: serde_json::json!({
                        "pid": p.pid,
                        "name": p.name,
                        "owner": p.owner,
                        "command": p.command,
                        "associated_path": p.associated_path,
                        "ports": p.listening_ports,
                    }),
                },
            );

            // 关联监听端口
            for port in &p.listening_ports {
                let port_id = format!("port:{}", port);
                push_node_unique(
                    &mut nodes,
                    &mut seen_nodes,
                    TopologyNode {
                        id: port_id.clone(),
                        label: format!(":{}", port),
                        node_type: NodeType::Port,
                        status: "listening".to_string(),
                        metadata: serde_json::json!({ "port": port }),
                    },
                );
                push_edge_unique(
                    &mut edges,
                    &mut seen_edges,
                    TopologyEdge {
                        source: proc_id.clone(),
                        target: port_id,
                        relation: "listens_on".to_string(),
                        label: Some("bind".to_string()),
                    },
                );
            }

            // 关联执行脚本文件
            if let Some(ref script_path) = p.associated_path {
                let file_id = format!("file:{}", script_path);
                let file_label = script_path.split('/').last().unwrap_or(script_path).to_string();
                push_node_unique(
                    &mut nodes,
                    &mut seen_nodes,
                    TopologyNode {
                        id: file_id.clone(),
                        label: file_label,
                        node_type: NodeType::File,
                        status: "active".to_string(),
                        metadata: serde_json::json!({ "path": script_path }),
                    },
                );
                push_edge_unique(
                    &mut edges,
                    &mut seen_edges,
                    TopologyEdge {
                        source: proc_id,
                        target: file_id,
                        relation: "runs".to_string(),
                        label: Some("exec".to_string()),
                    },
                );
            }
        }
    }

    // 3. 整合 links 表中的文件/服务依赖边
    if let Ok(links) = storage.list_all_links(200) {
        for l in links {
            let src_id = if l.source_id.contains(':') {
                l.source_id.clone()
            } else {
                format!("{}:{}", l.source_type, l.source_id)
            };
            let dst_id = if l.target_id.contains(':') {
                l.target_id.clone()
            } else {
                format!("{}:{}", l.target_type, l.target_id)
            };

            push_node_unique(
                &mut nodes,
                &mut seen_nodes,
                TopologyNode {
                    id: src_id.clone(),
                    label: l.source_id.split('/').last().unwrap_or(&l.source_id).to_string(),
                    node_type: map_fact_node_type(&l.source_type),
                    status: "active".to_string(),
                    metadata: serde_json::json!({ "ref": l.source_id }),
                },
            );
            push_node_unique(
                &mut nodes,
                &mut seen_nodes,
                TopologyNode {
                    id: dst_id.clone(),
                    label: l.target_id.split('/').last().unwrap_or(&l.target_id).to_string(),
                    node_type: map_fact_node_type(&l.target_type),
                    status: "active".to_string(),
                    metadata: serde_json::json!({ "ref": l.target_id }),
                },
            );
            push_edge_unique(
                &mut edges,
                &mut seen_edges,
                TopologyEdge {
                    source: src_id,
                    target: dst_id,
                    relation: l.relation.clone(),
                    label: Some(l.relation),
                },
            );
        }
    }

    // 4. 若资产节点很少，补充前 30 条核心资产
    if let Ok(assets) = storage.list_assets(30) {
        for a in assets {
            let file_id = format!("file:{}", a.path);
            push_node_unique(
                &mut nodes,
                &mut seen_nodes,
                TopologyNode {
                    id: file_id,
                    label: a.path.split('/').last().unwrap_or(&a.path).to_string(),
                    node_type: NodeType::File,
                    status: a.status.clone(),
                    metadata: serde_json::json!({
                        "path": a.path,
                        "asset_type": a.asset_type,
                        "security_level": a.security_level,
                    }),
                },
            );
        }
    }

    let summary = summarize_topology(&nodes, &edges);
    TopologyGraph {
        nodes,
        edges,
        summary,
    }
}

fn summarize_topology(nodes: &[TopologyNode], edges: &[TopologyEdge]) -> TopologySummary {
    let mut process_count = 0;
    let mut port_count = 0;
    let mut service_count = 0;
    let mut file_count = 0;

    for n in nodes {
        match n.node_type {
            NodeType::Process => process_count += 1,
            NodeType::Port => port_count += 1,
            NodeType::Service | NodeType::Cron => service_count += 1,
            NodeType::File | NodeType::Git => file_count += 1,
            _ => {}
        }
    }

    TopologySummary {
        total_nodes: nodes.len(),
        total_edges: edges.len(),
        process_count,
        port_count,
        service_count,
        file_count,
    }
}

/// 构建指定目标的爆炸半径子图（结合 links 与 fact_edges 多跳图遍历）
pub fn build_impact_topology(target: &str, storage: &dyn StorageProvider) -> TopologyGraph {
    let mut nodes: Vec<TopologyNode> = Vec::new();
    let mut edges: Vec<TopologyEdge> = Vec::new();
    let mut seen_nodes: HashSet<String> = HashSet::new();
    let mut seen_edges: HashSet<(String, String, String)> = HashSet::new();

    // 1. 尝试在 fact_nodes 中匹配根节点并执行 3 跳遍历
    let mut fact_root_id: Option<String> = None;
    if storage.get_fact_node(target).ok().flatten().is_some() {
        fact_root_id = Some(target.to_string());
    } else {
        for prefix in &["file:", "service:", "port:", "project:", "cron:", "tool:"] {
            let candidate = format!("{}{}", prefix, target);
            if storage.get_fact_node(&candidate).ok().flatten().is_some() {
                fact_root_id = Some(candidate);
                break;
            }
        }
        if fact_root_id.is_none() {
            if let Ok(matches) = storage.find_fact_nodes_by_name(target) {
                if let Some(first) = matches.first() {
                    fact_root_id = Some(first.id.clone());
                }
            }
        }
    }

    if let Some(root_id) = fact_root_id {
        if let Ok(sub) = storage.traverse_fact_graph(&root_id, 3) {
            for gn in sub.nodes {
                let mut tnode = fact_record_to_topology_node(gn);
                if tnode.id == root_id {
                    tnode.status = "target".to_string();
                } else {
                    tnode.status = "impacted".to_string();
                }
                push_node_unique(&mut nodes, &mut seen_nodes, tnode);
            }
            for fe in sub.edges {
                push_edge_unique(
                    &mut edges,
                    &mut seen_edges,
                    TopologyEdge {
                        source: fe.source_id,
                        target: fe.target_id,
                        relation: fe.relation.clone(),
                        label: Some(fe.relation),
                    },
                );
            }
        }
    }

    // 2. 补充 links 表中的直接关联引用
    let root_id = if seen_nodes.is_empty() {
        let id = format!("target:{}", target);
        push_node_unique(
            &mut nodes,
            &mut seen_nodes,
            TopologyNode {
                id: id.clone(),
                label: target.split('/').last().unwrap_or(target).to_string(),
                node_type: NodeType::File,
                status: "target".to_string(),
                metadata: serde_json::json!({ "path": target }),
            },
        );
        id
    } else {
        nodes[0].id.clone()
    };

    let clean_target = target.strip_prefix("file:").unwrap_or(target);
    if let Ok(links) = storage.query_links_for_node("file", clean_target) {
        for link in links {
            let other = if link.source_id == clean_target {
                &link.target_id
            } else {
                &link.source_id
            };
            let dep_id = format!("dep:{}", other);
            push_node_unique(
                &mut nodes,
                &mut seen_nodes,
                TopologyNode {
                    id: dep_id.clone(),
                    label: other.split('/').last().unwrap_or(other).to_string(),
                    node_type: NodeType::File,
                    status: "impacted".to_string(),
                    metadata: serde_json::json!({ "path": other, "relation": link.relation }),
                },
            );

            push_edge_unique(
                &mut edges,
                &mut seen_edges,
                TopologyEdge {
                    source: root_id.clone(),
                    target: dep_id,
                    relation: link.relation,
                    label: Some("impacts".to_string()),
                },
            );
        }
    }

    let summary = summarize_topology(&nodes, &edges);
    TopologyGraph {
        nodes,
        edges,
        summary,
    }
}
