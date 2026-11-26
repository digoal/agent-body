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
    pub in_degree: usize,
    pub out_degree: usize,
    pub path_depth: usize,
    pub canonical_path: Option<String>,
    pub metadata: serde_json::Value,
}

fn default_topo_edge_weight() -> f32 {
    1.0
}

fn default_topo_edge_tier() -> String {
    "L1_EXPLICIT".to_string()
}

/// 拓扑连边
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyEdge {
    pub source: String,
    pub target: String,
    pub relation: String,
    pub label: Option<String>,
    #[serde(default = "default_topo_edge_weight")]
    pub weight: f32,
    #[serde(default = "default_topo_edge_tier")]
    pub evidence_tier: String,
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
        "project" | "repo" | "git" => NodeType::Git,
        "file" | "asset" | "directory" | "doc" | "script" => NodeType::File,
        "process" | "proc" => NodeType::Process,
        "memory" => NodeType::Memory,
        other => NodeType::Custom(other.to_string()),
    }
}

fn normalize_path_str(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() > 1 && trimmed.ends_with('/') {
        trimmed.trim_end_matches('/').to_string()
    } else {
        trimmed.to_string()
    }
}

fn extract_path_from_id_or_meta(id: &str, metadata: &serde_json::Value) -> Option<String> {
    for key in ["abs_path", "path", "full_path", "ref"] {
        if let Some(s) = metadata.get(key).and_then(|v| v.as_str()) {
            let norm = normalize_path_str(s);
            if norm.starts_with('/') {
                return Some(norm);
            }
        }
    }
    for prefix in ["file:", "asset:", "project:", "git:", "dir:"] {
        if let Some(rest) = id.strip_prefix(prefix) {
            let norm = normalize_path_str(rest);
            if norm.starts_with('/') {
                return Some(norm);
            }
        }
    }
    if id.starts_with('/') {
        return Some(normalize_path_str(id));
    }
    None
}

fn compute_effective_path_depth_with_base(path: &str, base_dir: &std::path::Path) -> usize {
    let norm = normalize_path_str(path);
    let base_norm = normalize_path_str(&base_dir.to_string_lossy());
    if !base_norm.is_empty() && base_norm != "/" {
        let prefix = format!("{}/", base_norm);
        if let Some(rel) = norm.strip_prefix(&prefix) {
            let count = rel.split('/').filter(|s| !s.is_empty()).count();
            return count.max(1);
        }
        if norm == base_norm {
            return 1;
        }
    }
    let count = norm.trim_matches('/').split('/').filter(|s| !s.is_empty()).count();
    count.max(1)
}

fn compute_effective_path_depth(path: &str) -> usize {
    let base = crate::config::Config::load_effective_or_default().effective_workspace_base();
    compute_effective_path_depth_with_base(path, &base)
}

fn canonicalize_node_id(raw_id: &str, raw_type: &str) -> String {
    if let Some(rest) = raw_id.strip_prefix("asset:").or_else(|| raw_id.strip_prefix("file:")) {
        let norm = normalize_path_str(rest);
        if norm.starts_with('/') {
            return format!("file:{}", norm);
        }
    }
    if let Some(rest) = raw_id.strip_prefix("project:").or_else(|| raw_id.strip_prefix("git:")) {
        let norm = normalize_path_str(rest);
        if norm.starts_with('/') {
            return format!("project:{}", norm);
        }
    }
    if raw_id.starts_with('/') {
        let norm = normalize_path_str(raw_id);
        let prefix = if matches!(raw_type, "project" | "repo" | "git") {
            "project"
        } else {
            "file"
        };
        return format!("{}:{}", prefix, norm);
    }
    if !raw_id.contains(':') {
        return format!("{}:{}", raw_type, normalize_path_str(raw_id));
    }
    raw_id.to_string()
}

fn make_topology_node(
    id: String,
    label: String,
    node_type: NodeType,
    status: String,
    metadata: serde_json::Value,
) -> TopologyNode {
    let canonical_path = extract_path_from_id_or_meta(&id, &metadata);
    let path_depth = canonical_path
        .as_deref()
        .map(compute_effective_path_depth)
        .unwrap_or(1);
    TopologyNode {
        id,
        label,
        node_type,
        status,
        in_degree: 0,
        out_degree: 0,
        path_depth,
        canonical_path,
        metadata,
    }
}

fn fact_record_to_topology_node(gn: FactNodeRecord) -> TopologyNode {
    let node_type = map_fact_node_type(&gn.node_type);
    let status = match gn.node_type.as_str() {
        "port" => "listening".to_string(),
        _ => "active".to_string(),
    };
    let canon_id = canonicalize_node_id(&gn.id, &gn.node_type);
    let label = if gn.name.trim().is_empty() || gn.name.ends_with('/') {
        let norm = normalize_path_str(&gn.name);
        norm.split('/').next_back().filter(|s| !s.is_empty()).unwrap_or(&norm).to_string()
    } else {
        gn.name
    };
    make_topology_node(canon_id, label, node_type, status, gn.properties)
}

fn populate_degrees(nodes: &mut [TopologyNode], edges: &[TopologyEdge]) {
    let mut in_deg: HashMap<&str, usize> = HashMap::new();
    let mut out_deg: HashMap<&str, usize> = HashMap::new();
    for e in edges {
        if e.source == e.target {
            continue;
        }
        *out_deg.entry(e.source.as_str()).or_insert(0) += 1;
        *in_deg.entry(e.target.as_str()).or_insert(0) += 1;
    }
    for n in nodes.iter_mut() {
        n.in_degree = *in_deg.get(n.id.as_str()).unwrap_or(&0);
        n.out_degree = *out_deg.get(n.id.as_str()).unwrap_or(&0);
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
    if edge.source == edge.target {
        return;
    }
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
    let fact_nodes = storage.list_all_fact_nodes(500).unwrap_or_default();
    let mut fact_node_map: HashMap<String, FactNodeRecord> = HashMap::new();
    let mut fact_by_canon_id: HashMap<String, FactNodeRecord> = HashMap::new();
    for gn in fact_nodes {
        let cid = canonicalize_node_id(&gn.id, &gn.node_type);
        fact_by_canon_id.insert(cid, gn.clone());
        fact_node_map.insert(gn.id.clone(), gn);
    }

    // 1a. 先加入所有参与了边的 fact_nodes，并建立边
    for fe in fact_edges {
        let src_id = if let Some(src) = fact_node_map.get(&fe.source_id).cloned() {
            let tnode = fact_record_to_topology_node(src);
            let id = tnode.id.clone();
            push_node_unique(&mut nodes, &mut seen_nodes, tnode);
            id
        } else {
            let inferred_type = fe.source_id.split(':').next().unwrap_or("custom");
            let cid = canonicalize_node_id(&fe.source_id, inferred_type);
            let inferred_label = cid.split(':').nth(1).unwrap_or(&cid);
            let label = normalize_path_str(inferred_label)
                .split('/')
                .next_back()
                .filter(|s| !s.is_empty())
                .unwrap_or(inferred_label)
                .to_string();
            push_node_unique(
                &mut nodes,
                &mut seen_nodes,
                make_topology_node(
                    cid.clone(),
                    label,
                    map_fact_node_type(inferred_type),
                    "active".to_string(),
                    serde_json::json!({}),
                ),
            );
            cid
        };

        let dst_id = if let Some(dst) = fact_node_map.get(&fe.target_id).cloned() {
            let tnode = fact_record_to_topology_node(dst);
            let id = tnode.id.clone();
            push_node_unique(&mut nodes, &mut seen_nodes, tnode);
            id
        } else {
            let inferred_type = fe.target_id.split(':').next().unwrap_or("custom");
            let cid = canonicalize_node_id(&fe.target_id, inferred_type);
            let inferred_label = cid.split(':').nth(1).unwrap_or(&cid);
            let label = normalize_path_str(inferred_label)
                .split('/')
                .next_back()
                .filter(|s| !s.is_empty())
                .unwrap_or(inferred_label)
                .to_string();
            push_node_unique(
                &mut nodes,
                &mut seen_nodes,
                make_topology_node(
                    cid.clone(),
                    label,
                    map_fact_node_type(inferred_type),
                    "active".to_string(),
                    serde_json::json!({}),
                ),
            );
            cid
        };

        push_edge_unique(
            &mut edges,
            &mut seen_edges,
            TopologyEdge {
                source: src_id,
                target: dst_id,
                relation: fe.relation.clone(),
                label: Some(fe.relation),
                weight: fe.weight,
                evidence_tier: fe.evidence_tier,
            },
        );
    }

    // 1b. 补充关键未连边事实节点（service / port / cron 全部保留，project 限前 25 个）
    let mut extra_projects = 0usize;
    for gn in fact_node_map.into_values() {
        let canon_id = canonicalize_node_id(&gn.id, &gn.node_type);
        if seen_nodes.contains(&canon_id) {
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
                make_topology_node(
                    proc_id.clone(),
                    format!("{}:{}", p.name, p.pid),
                    NodeType::Process,
                    p.status.clone(),
                    serde_json::json!({
                        "pid": p.pid,
                        "name": p.name,
                        "owner": p.owner,
                        "command": p.command,
                        "associated_path": p.associated_path,
                        "ports": p.listening_ports,
                    }),
                ),
            );

            // 关联监听端口
            for port in &p.listening_ports {
                let port_id = format!("port:{}", port);
                push_node_unique(
                    &mut nodes,
                    &mut seen_nodes,
                    make_topology_node(
                        port_id.clone(),
                        format!(":{}", port),
                        NodeType::Port,
                        "listening".to_string(),
                        serde_json::json!({ "port": port }),
                    ),
                );
                push_edge_unique(
                    &mut edges,
                    &mut seen_edges,
                    TopologyEdge {
                        source: proc_id.clone(),
                        target: port_id,
                        relation: "listens_on".to_string(),
                        label: Some("bind".to_string()),
                        weight: 1.0,
                        evidence_tier: "L1_EXPLICIT".to_string(),
                    },
                );
            }

            // 关联执行脚本文件
            if let Some(ref script_path) = p.associated_path {
                let norm_path = normalize_path_str(script_path);
                let file_id = format!("file:{}", norm_path);
                let file_label = norm_path.split('/').next_back().unwrap_or(&norm_path).to_string();
                push_node_unique(
                    &mut nodes,
                    &mut seen_nodes,
                    make_topology_node(
                        file_id.clone(),
                        file_label,
                        NodeType::File,
                        "active".to_string(),
                        serde_json::json!({ "path": norm_path }),
                    ),
                );
                push_edge_unique(
                    &mut edges,
                    &mut seen_edges,
                    TopologyEdge {
                        source: proc_id,
                        target: file_id,
                        relation: "runs".to_string(),
                        label: Some("exec".to_string()),
                        weight: 1.0,
                        evidence_tier: "L1_EXPLICIT".to_string(),
                    },
                );
            }
        }
    }

    // 3. 整合 links 表中的文件/服务依赖边
    if let Ok(links) = storage.list_all_links(200) {
        for l in links {
            let src_id = canonicalize_node_id(&l.source_id, &l.source_type);
            let dst_id = canonicalize_node_id(&l.target_id, &l.target_type);
            let src_norm = normalize_path_str(&l.source_id);
            let dst_norm = normalize_path_str(&l.target_id);

            let src_label = src_norm.split('/').next_back().filter(|s| !s.is_empty()).unwrap_or(&src_norm).to_string();
            let dst_label = dst_norm.split('/').next_back().filter(|s| !s.is_empty()).unwrap_or(&dst_norm).to_string();

            push_node_unique(
                &mut nodes,
                &mut seen_nodes,
                make_topology_node(
                    src_id.clone(),
                    src_label,
                    map_fact_node_type(&l.source_type),
                    "active".to_string(),
                    serde_json::json!({ "ref": src_norm }),
                ),
            );
            push_node_unique(
                &mut nodes,
                &mut seen_nodes,
                make_topology_node(
                    dst_id.clone(),
                    dst_label,
                    map_fact_node_type(&l.target_type),
                    "active".to_string(),
                    serde_json::json!({ "ref": dst_norm }),
                ),
            );
            push_edge_unique(
                &mut edges,
                &mut seen_edges,
                TopologyEdge {
                    source: src_id,
                    target: dst_id,
                    relation: l.relation.clone(),
                    label: Some(l.relation),
                    weight: l.confidence,
                    evidence_tier: "L1_EXPLICIT".to_string(),
                },
            );
        }
    }

    // 4. 若资产节点很少，补充前 30 条核心资产
    if let Ok(assets) = storage.list_assets(30) {
        for a in assets {
            let norm_path = normalize_path_str(&a.path);
            let file_id = format!("file:{}", norm_path);
            let label = norm_path.split('/').next_back().filter(|s| !s.is_empty()).unwrap_or(&norm_path).to_string();
            let mut meta = serde_json::Map::new();
            if let Some(obj) = a.desc_kv.as_object() {
                for (k, v) in obj {
                    meta.insert(k.clone(), v.clone());
                }
            }
            meta.insert("path".to_string(), serde_json::json!(norm_path));
            meta.insert("canonical_path".to_string(), serde_json::json!(norm_path));
            meta.insert("asset_type".to_string(), serde_json::json!(a.asset_type));
            meta.insert("owner".to_string(), serde_json::json!(a.owner));
            meta.insert("security_level".to_string(), serde_json::json!(a.security_level));
            meta.insert("scan_tier".to_string(), serde_json::json!(a.scan_tier));
            if let Some(sz) = a.size_bytes {
                meta.insert("size_bytes".to_string(), serde_json::json!(sz));
            }
            if !a.tags.is_empty() {
                meta.insert("tags".to_string(), serde_json::json!(a.tags));
            }
            if let Some(ref d) = a.description {
                meta.insert("description".to_string(), serde_json::json!(d));
            }
            if let Some(ref u) = a.usage {
                meta.insert("usage".to_string(), serde_json::json!(u));
            }
            push_node_unique(
                &mut nodes,
                &mut seen_nodes,
                make_topology_node(
                    file_id,
                    label,
                    NodeType::File,
                    a.status.clone(),
                    serde_json::Value::Object(meta),
                ),
            );
        }
    }

    enrich_topology_nodes(&mut nodes, storage, &fact_by_canon_id);
    populate_degrees(&mut nodes, &edges);
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
            let norm_root = canonicalize_node_id(&root_id, "file");
            for gn in sub.nodes {
                let mut tnode = fact_record_to_topology_node(gn);
                if tnode.id == root_id || tnode.id == norm_root {
                    tnode.status = "target".to_string();
                } else {
                    tnode.status = "impacted".to_string();
                }
                push_node_unique(&mut nodes, &mut seen_nodes, tnode);
            }
            for fe in sub.edges {
                let s_type = fe.source_id.split(':').next().unwrap_or("file");
                let t_type = fe.target_id.split(':').next().unwrap_or("file");
                push_edge_unique(
                    &mut edges,
                    &mut seen_edges,
                    TopologyEdge {
                        source: canonicalize_node_id(&fe.source_id, s_type),
                        target: canonicalize_node_id(&fe.target_id, t_type),
                        relation: fe.relation.clone(),
                        label: Some(fe.relation),
                        weight: fe.weight,
                        evidence_tier: fe.evidence_tier,
                    },
                );
            }
        }
    }

    // 2. 补充 links 表中的直接关联引用
    let root_id = if seen_nodes.is_empty() {
        let id = canonicalize_node_id(target, "file");
        let norm = normalize_path_str(target);
        push_node_unique(
            &mut nodes,
            &mut seen_nodes,
            make_topology_node(
                id.clone(),
                norm.split('/').next_back().filter(|s| !s.is_empty()).unwrap_or(&norm).to_string(),
                NodeType::File,
                "target".to_string(),
                serde_json::json!({ "path": norm }),
            ),
        );
        id
    } else {
        nodes[0].id.clone()
    };

    let clean_target = normalize_path_str(
        target
            .strip_prefix("file:")
            .or_else(|| target.strip_prefix("asset:"))
            .unwrap_or(target),
    );
    if let Ok(links) = storage.query_links_for_node("file", &clean_target) {
        for link in links {
            let other = if link.source_id == clean_target {
                &link.target_id
            } else {
                &link.source_id
            };
            let dep_id = canonicalize_node_id(other, "file");
            let norm_other = normalize_path_str(other);
            push_node_unique(
                &mut nodes,
                &mut seen_nodes,
                make_topology_node(
                    dep_id.clone(),
                    norm_other.split('/').next_back().filter(|s| !s.is_empty()).unwrap_or(&norm_other).to_string(),
                    NodeType::File,
                    "impacted".to_string(),
                    serde_json::json!({ "path": norm_other, "relation": link.relation }),
                ),
            );

            push_edge_unique(
                &mut edges,
                &mut seen_edges,
                TopologyEdge {
                    source: root_id.clone(),
                    target: dep_id,
                    relation: link.relation,
                    label: Some("impacts".to_string()),
                    weight: link.confidence,
                    evidence_tier: "L1_EXPLICIT".to_string(),
                },
            );
        }
    }

    let empty_facts = HashMap::new();
    enrich_topology_nodes(&mut nodes, storage, &empty_facts);
    populate_degrees(&mut nodes, &edges);
    let summary = summarize_topology(&nodes, &edges);
    TopologyGraph {
        nodes,
        edges,
        summary,
    }
}

/// 统一丰富拓扑节点 metadata：回填 fact_nodes、assets 语义库，并对系统路径/端口/进程兜底补全属性
fn enrich_topology_nodes(
    nodes: &mut [TopologyNode],
    storage: &dyn StorageProvider,
    fact_by_canon_id: &HashMap<String, FactNodeRecord>,
) {
    for node in nodes.iter_mut() {
        if !node.metadata.is_object() {
            node.metadata = serde_json::json!({});
        }
        let Some(map) = node.metadata.as_object_mut() else {
            continue;
        };

        // 1. 尝试从 fact_nodes 合并已有属性
        if let Some(fn_rec) = fact_by_canon_id.get(&node.id) {
            if let Some(fobj) = fn_rec.properties.as_object() {
                for (k, v) in fobj {
                    map.entry(k.clone()).or_insert_with(|| v.clone());
                }
            }
        }

        // 2. 若节点具有物理路径，优先从 assets 表回填完整语义元数据
        if let Some(ref cpath) = node.canonical_path {
            map.entry("canonical_path".to_string())
                .or_insert_with(|| serde_json::json!(cpath));
            if let Ok(Some(asset)) = storage.get_asset_by_path(cpath) {
                if let Some(dkv) = asset.desc_kv.as_object() {
                    for (k, v) in dkv {
                        map.entry(k.clone()).or_insert_with(|| v.clone());
                    }
                }
                map.entry("asset_type".to_string())
                    .or_insert_with(|| serde_json::json!(asset.asset_type));
                map.entry("owner".to_string())
                    .or_insert_with(|| serde_json::json!(asset.owner));
                map.entry("security_level".to_string())
                    .or_insert_with(|| serde_json::json!(asset.security_level));
                map.entry("scan_tier".to_string())
                    .or_insert_with(|| serde_json::json!(asset.scan_tier));
                if let Some(sz) = asset.size_bytes {
                    map.entry("size_bytes".to_string())
                        .or_insert_with(|| serde_json::json!(sz));
                }
                if !asset.tags.is_empty() {
                    map.entry("tags".to_string())
                        .or_insert_with(|| serde_json::json!(asset.tags));
                }
                if let Some(ref u) = asset.usage {
                    map.entry("usage".to_string())
                        .or_insert_with(|| serde_json::json!(u));
                }
                if let Some(ref d) = asset.description {
                    if !d.trim().is_empty() {
                        map.entry("description".to_string())
                            .or_insert_with(|| serde_json::json!(d));
                    }
                }
            } else {
                // 未入库路径：轻量物理探针补全，避免单 Key
                let p = std::path::Path::new(cpath);
                let meta = std::fs::symlink_metadata(p).ok();
                let exists = meta.is_some();
                let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
                let size_bytes = std::fs::metadata(p).ok().map(|m| m.len());
                map.entry("exists".to_string())
                    .or_insert_with(|| serde_json::json!(exists));
                map.entry("is_dir".to_string())
                    .or_insert_with(|| serde_json::json!(is_dir));
                if let Some(sz) = size_bytes {
                    map.entry("size_bytes".to_string())
                        .or_insert_with(|| serde_json::json!(sz));
                }
                map.entry("domain".to_string())
                    .or_insert_with(|| serde_json::json!(if is_dir { "workspace_dir" } else { "host_file" }));
                map.entry("category".to_string())
                    .or_insert_with(|| serde_json::json!(if is_dir { "directory" } else { "referenced_file" }));
            }
        }

        // 3. 按节点类型兜底保证 description / domain / category 100% 存在
        match node.node_type {
            NodeType::Process => {
                map.entry("domain".to_string())
                    .or_insert_with(|| serde_json::json!("host_process"));
                map.entry("category".to_string())
                    .or_insert_with(|| serde_json::json!("active_process"));
                if !map.contains_key("description") {
                    let pid = map.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
                    let owner = map.get("owner").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let desc = format!("活跃进程 {} (PID {}, 归属 {})", node.label, pid, owner);
                    map.insert("description".to_string(), serde_json::json!(desc));
                }
            }
            NodeType::Port => {
                map.entry("domain".to_string())
                    .or_insert_with(|| serde_json::json!("host_network"));
                map.entry("category".to_string())
                    .or_insert_with(|| serde_json::json!("listening_socket"));
                if !map.contains_key("description") {
                    let port_num = map.get("port").and_then(|v| v.as_u64()).map(|n| n.to_string()).unwrap_or_else(|| node.label.clone());
                    let proto = map.get("protocol").and_then(|v| v.as_str()).unwrap_or("tcp");
                    map.insert(
                        "description".to_string(),
                        serde_json::json!(format!("{} 监听端口 {}", proto.to_uppercase(), port_num)),
                    );
                }
            }
            NodeType::Service => {
                map.entry("domain".to_string())
                    .or_insert_with(|| serde_json::json!("host_service"));
                map.entry("category".to_string())
                    .or_insert_with(|| serde_json::json!("service"));
                map.entry("description".to_string())
                    .or_insert_with(|| serde_json::json!(format!("宿主机服务节点 {}", node.label)));
            }
            NodeType::Cron => {
                map.entry("domain".to_string())
                    .or_insert_with(|| serde_json::json!("host_automation"));
                map.entry("category".to_string())
                    .or_insert_with(|| serde_json::json!("cron_job"));
                map.entry("description".to_string())
                    .or_insert_with(|| serde_json::json!(format!("Cron 定时任务 {}", node.label)));
            }
            NodeType::Git => {
                map.entry("domain".to_string())
                    .or_insert_with(|| serde_json::json!("workspace_project"));
                map.entry("category".to_string())
                    .or_insert_with(|| serde_json::json!("git_repo"));
                map.entry("description".to_string())
                    .or_insert_with(|| serde_json::json!(format!("工程仓库 {}", node.label)));
            }
            NodeType::File => {
                map.entry("domain".to_string())
                    .or_insert_with(|| serde_json::json!("host_file"));
                map.entry("category".to_string())
                    .or_insert_with(|| serde_json::json!("file_asset"));
                if !map.contains_key("description") {
                    let cpath = node.canonical_path.as_deref().unwrap_or(&node.id);
                    map.insert(
                        "description".to_string(),
                        serde_json::json!(format!("宿主机文件节点 {} ({})", node.label, cpath)),
                    );
                }
            }
            _ => {
                map.entry("domain".to_string())
                    .or_insert_with(|| serde_json::json!("host_entity"));
                map.entry("description".to_string())
                    .or_insert_with(|| serde_json::json!(format!("宿主机实体 {}", node.label)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topology_path_normalization_and_depth() {
        assert_eq!(normalize_path_str("/root/new/tmp/"), "/root/new/tmp");
        assert_eq!(normalize_path_str("/"), "/");
        assert_eq!(
            canonicalize_node_id("asset:/root/new/tmp/", "asset"),
            "file:/root/new/tmp"
        );
        assert_eq!(
            canonicalize_node_id("file:/root/new/tmp", "file"),
            "file:/root/new/tmp"
        );
        let portable_base = std::path::Path::new("/home/alice/workspace");
        assert_eq!(
            compute_effective_path_depth_with_base("/home/alice/workspace/work", portable_base),
            1
        );
        assert_eq!(
            compute_effective_path_depth_with_base("/home/alice/workspace/work/agent-body", portable_base),
            2
        );
        assert_eq!(
            compute_effective_path_depth_with_base("/home/alice/workspace/work/agent-body/src", portable_base),
            3
        );
        assert_eq!(
            compute_effective_path_depth_with_base(
                "/home/alice/workspace/work/agent-body/src/web/mod.rs",
                portable_base
            ),
            5
        );
    }

    #[test]
    fn test_map_fact_node_type_eliminates_asset_custom() {
        assert_eq!(map_fact_node_type("asset"), NodeType::File);
        assert_eq!(map_fact_node_type("directory"), NodeType::File);
        assert_eq!(map_fact_node_type("project"), NodeType::Git);
        assert_eq!(map_fact_node_type("port"), NodeType::Port);
    }

    #[test]
    fn test_topology_enrichment_eliminates_single_key_nodes() {
        use crate::storage::sqlite_provider::SqliteStorageProvider;
        use crate::storage::models::{AssetRecord, LinkRecord};

        let storage = SqliteStorageProvider::open_in_memory().expect("open_in_memory");
        storage.init().expect("init");

        storage
            .upsert_asset(&AssetRecord {
                id: "asset-rich-1".to_string(),
                path: "/root/new/shared_proj/rich_tool.py".to_string(),
                asset_type: "script".to_string(),
                owner: "user:digoal".to_string(),
                security_level: "internal".to_string(),
                scan_tier: "deep".to_string(),
                description: Some("[shared_tools] 自动化校验脚本".to_string()),
                desc_kv: serde_json::json!({
                    "domain": "shared_tools",
                    "purpose": "自动化校验脚本",
                    "embedding_status": "ready",
                    "embedding_model": "nomic-embed-text",
                    "embedding_dim": 768
                }),
                usage: Some("python3 rich_tool.py".to_string()),
                size_bytes: Some(2048),
                file_count: None,
                tags: vec!["python".to_string()],
                status: "active".to_string(),
            })
            .expect("upsert asset");

        storage
            .add_link(&LinkRecord {
                id: None,
                source_type: "asset".to_string(),
                source_id: "/root/new/shared_proj/rich_tool.py".to_string(),
                target_type: "asset".to_string(),
                target_id: "/root/new/shared_proj/missing_ref.py".to_string(),
                relation: "depends_on".to_string(),
                confidence: 0.9,
                auto_discovered: true,
                line_number: Some(12),
                snippet: None,
                provenance: Some("static_scan".to_string()),
                created_at: None,
            })
            .expect("add link");

        let graph = build_full_topology(&storage);
        for n in &graph.nodes {
            let obj = n.metadata.as_object().expect("metadata should be object");
            assert!(
                obj.contains_key("description"),
                "node {} missing description: {:?}",
                n.id,
                n.metadata
            );
            assert!(
                obj.len() >= 4,
                "node {} still has too few keys: {:?}",
                n.id,
                n.metadata
            );
        }

        let rich = graph
            .nodes
            .iter()
            .find(|n| n.id == "file:/root/new/shared_proj/rich_tool.py")
            .expect("rich node present");
        assert_eq!(rich.metadata["domain"], "shared_tools");
        assert_eq!(rich.metadata["embedding_status"], "ready");
        assert_eq!(rich.metadata["usage"], "python3 rich_tool.py");
    }
}
