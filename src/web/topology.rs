use serde::{Deserialize, Serialize};
use crate::storage::StorageProvider;
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

/// 构建宿主机全景事实拓扑
pub fn build_full_topology(storage: &dyn StorageProvider) -> TopologyGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    let mut process_count = 0;
    let mut port_count = 0;
    let mut service_count = 0;
    let mut file_count = 0;

    // 1. 采集活跃进程与监听端口
    if let Ok(processes) = storage.list_active_processes() {
        for p in processes {
            process_count += 1;
            let proc_id = format!("proc:{}", p.pid);
            nodes.push(TopologyNode {
                id: proc_id.clone(),
                label: format!("{}:{}", p.name, p.pid),
                node_type: NodeType::Process,
                status: p.status.clone(),
                metadata: serde_json::json!({
                    "pid": p.pid,
                    "name": p.name,
                    "command": p.command,
                    "ports": p.listening_ports,
                }),
            });

            // 监听端口，建立端口节点与关系边
            for port in &p.listening_ports {
                port_count += 1;
                let port_id = format!("port:{}", port);
                nodes.push(TopologyNode {
                    id: port_id.clone(),
                    label: format!(":{}", port),
                    node_type: NodeType::Port,
                    status: "listening".to_string(),
                    metadata: serde_json::json!({ "port": port }),
                });

                edges.push(TopologyEdge {
                    source: proc_id.clone(),
                    target: port_id,
                    relation: "listens_on".to_string(),
                    label: Some("bind".to_string()),
                });
            }
        }
    }

    // 2. 采集核心资产 (前 50 条)
    if let Ok(assets) = storage.list_assets(50) {
        for a in assets {
            file_count += 1;
            let file_id = format!("asset:{}", a.id);
            nodes.push(TopologyNode {
                id: file_id.clone(),
                label: a.path.split('/').last().unwrap_or(&a.path).to_string(),
                node_type: NodeType::File,
                status: a.status.clone(),
                metadata: serde_json::json!({
                    "path": a.path,
                    "asset_type": a.asset_type,
                    "security_level": a.security_level,
                }),
            });
        }
    }

    // 3. 从事实图谱关联属性边与服务
    if let Ok(graph_nodes) = storage.list_all_fact_nodes(50) {
        for gn in graph_nodes {
            if gn.node_type == "service" {
                service_count += 1;
                nodes.push(TopologyNode {
                    id: format!("service:{}", gn.name),
                    label: gn.name.clone(),
                    node_type: NodeType::Service,
                    status: "active".to_string(),
                    metadata: gn.properties,
                });
            }
        }
    }

    let summary = TopologySummary {
        total_nodes: nodes.len(),
        total_edges: edges.len(),
        process_count,
        port_count,
        service_count,
        file_count,
    };

    TopologyGraph {
        nodes,
        edges,
        summary,
    }
}

/// 构建指定目标的爆炸半径子图
pub fn build_impact_topology(target: &str, storage: &dyn StorageProvider) -> TopologyGraph {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    let root_id = format!("target:{}", target);
    nodes.push(TopologyNode {
        id: root_id.clone(),
        label: target.split('/').last().unwrap_or(target).to_string(),
        node_type: NodeType::File,
        status: "target".to_string(),
        metadata: serde_json::json!({ "path": target }),
    });

    // 查询依赖引用
    if let Ok(links) = storage.query_links_for_node("file", target) {
        for link in links {
            let dep_id = format!("dep:{}", link.target_id);
            nodes.push(TopologyNode {
                id: dep_id.clone(),
                label: link.target_id.split('/').last().unwrap_or(&link.target_id).to_string(),
                node_type: NodeType::File,
                status: "impacted".to_string(),
                metadata: serde_json::json!({ "relation": link.relation }),
            });

            edges.push(TopologyEdge {
                source: root_id.clone(),
                target: dep_id,
                relation: link.relation,
                label: Some("impacts".to_string()),
            });
        }
    }

    let summary = TopologySummary {
        total_nodes: nodes.len(),
        total_edges: edges.len(),
        process_count: 0,
        port_count: 0,
        service_count: 0,
        file_count: nodes.len(),
    };

    TopologyGraph {
        nodes,
        edges,
        summary,
    }
}
