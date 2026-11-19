use std::fs;
use std::io::Read;
use std::path::Path;
use anyhow::Result;
use chrono::{DateTime, Local};
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
                let repo_path = Path::new(&r.abs_path);
                let placement_zone = infer_placement_zone(&r.abs_path);
                let git_remote = if r.is_git_repo {
                    probe_git_remote(repo_path)
                } else {
                    None
                };
                let readme_summary = probe_project_readme_summary(repo_path);
                let top_entries = probe_top_level_entries(repo_path, 8);
                let has_readme = repo_path.join("README.md").exists() || repo_path.join("README_zh.md").exists();
                let lang_desc = r.primary_lang.as_deref().unwrap_or("多语言/通用");
                let description = readme_summary.unwrap_or_else(|| {
                    format!(
                        "{} 工程仓库 ({}, 位于 {} 区{})",
                        lang_desc,
                        r.name,
                        placement_zone,
                        if r.codegraph_ready { ", 已构建 CodeGraph" } else { "" }
                    )
                });
                let node = FactNodeRecord {
                    id: node_id.clone(),
                    node_type: "project".to_string(),
                    name: r.name.clone(),
                    label: format!("工程仓库: {}", r.name),
                    properties: serde_json::json!({
                        "description": description,
                        "canonical_path": r.abs_path,
                        "abs_path": r.abs_path,
                        "domain": format!("workspace_{}", placement_zone),
                        "category": if r.is_git_repo { "git_repo" } else { "project_dir" },
                        "placement_zone": placement_zone,
                        "is_git_repo": r.is_git_repo,
                        "codegraph_ready": r.codegraph_ready,
                        "current_branch": r.current_branch,
                        "primary_lang": r.primary_lang,
                        "git_remote_url": git_remote,
                        "has_readme": has_readme,
                        "top_level_entries": top_entries,
                        "recommendation": r.recommendation,
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
                let exposure_scope = if p.bind_ip == "127.0.0.1" || p.bind_ip == "::1" {
                    "loopback_safe"
                } else {
                    "public_exposed"
                };
                let assoc_project = p.cwd.as_deref().and_then(extract_project_from_path);
                let well_known_role = describe_well_known_port(p.port, p.process_name.as_deref());
                let port_desc = match (&p.process_name, well_known_role) {
                    (Some(pname), Some(role)) => format!(
                        "{} 端口 {} ({}, {})，由进程 {} (PID {}) 监听 — {}",
                        p.protocol.to_uppercase(),
                        p.port,
                        p.bind_ip,
                        exposure_scope,
                        pname,
                        p.pid.unwrap_or(0),
                        role
                    ),
                    (Some(pname), None) => format!(
                        "{} 端口 {} ({}, {})，由进程 {} (PID {}) 监听",
                        p.protocol.to_uppercase(),
                        p.port,
                        p.bind_ip,
                        exposure_scope,
                        pname,
                        p.pid.unwrap_or(0)
                    ),
                    (None, Some(role)) => format!(
                        "{} 端口 {} ({}, {}) — {}",
                        p.protocol.to_uppercase(),
                        p.port,
                        p.bind_ip,
                        exposure_scope,
                        role
                    ),
                    (None, None) => format!(
                        "{} 端口 {} 处于监听状态 (绑定 {}, 暴露面: {})",
                        p.protocol.to_uppercase(),
                        p.port,
                        p.bind_ip,
                        exposure_scope
                    ),
                };
                let port_node = FactNodeRecord {
                    id: port_id.clone(),
                    node_type: "port".to_string(),
                    name: p.port.to_string(),
                    label: format!("{} 端口 {}", p.protocol.to_uppercase(), p.port),
                    properties: serde_json::json!({
                        "description": port_desc,
                        "canonical_path": format!("port://{}/{}", p.protocol, p.port),
                        "domain": "host_network",
                        "category": "listening_socket",
                        "port": p.port,
                        "protocol": p.protocol,
                        "bind_ip": p.bind_ip,
                        "exposure_scope": exposure_scope,
                        "pid": p.pid,
                        "process_name": p.process_name,
                        "user": p.user,
                        "cwd": p.cwd,
                        "associated_project": assoc_project,
                        "is_system_daemon": p.is_system_daemon,
                    }),
                    updated_at: None,
                };
                self.storage.upsert_fact_node(&port_node)?;
                node_count += 1;

                if let Some(ref proc_name) = p.process_name {
                    let service_id = format!("service:{}", proc_name);
                    let svc_desc = format!(
                        "运行时监听服务 {} (PID {}, 端口 {}, 用户 {})",
                        proc_name,
                        p.pid.unwrap_or(0),
                        p.port,
                        p.user.as_deref().unwrap_or("unknown")
                    );
                    let service_node = FactNodeRecord {
                        id: service_id.clone(),
                        node_type: "service".to_string(),
                        name: proc_name.clone(),
                        label: format!("服务进程: {}", proc_name),
                        properties: serde_json::json!({
                            "description": svc_desc,
                            "canonical_path": p.cwd.clone().unwrap_or_else(|| format!("service://{}", proc_name)),
                            "domain": "host_service",
                            "category": if p.is_system_daemon { "system_daemon" } else { "runtime_service" },
                            "pid": p.pid,
                            "command": p.command,
                            "cwd": p.cwd,
                            "user": p.user,
                            "bound_port": p.port,
                            "protocol": p.protocol,
                            "associated_project": assoc_project,
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
                let assoc_project = s.working_directory.as_deref().and_then(extract_project_from_path);
                let svc_desc = if s.description.trim().is_empty() {
                    format!("Systemd 守护服务 {} ({})", s.service_name, s.exec_start)
                } else {
                    s.description.clone()
                };
                let snode = FactNodeRecord {
                    id: sid.clone(),
                    node_type: "service".to_string(),
                    name: s.service_name.clone(),
                    label: format!("Systemd服务: {}", s.service_name),
                    properties: serde_json::json!({
                        "description": svc_desc,
                        "canonical_path": s.unit_file_path,
                        "domain": "host_automation",
                        "category": "systemd_unit",
                        "unit_file_path": s.unit_file_path,
                        "exec_start": s.exec_start,
                        "working_directory": s.working_directory,
                        "user": s.user.as_deref().unwrap_or("root"),
                        "referenced_files_count": s.referenced_files.len(),
                        "environment_files": s.environment_files,
                        "associated_project": assoc_project,
                    }),
                    updated_at: None,
                };
                self.storage.upsert_fact_node(&snode)?;
                node_count += 1;

                // 关联依赖文件与引用文件（不再只写 full_path）
                for dep in s.referenced_files {
                    let fnode = self.build_file_fact_node(&dep, "systemd_dependency");
                    let fid = fnode.id.clone();
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
                let schedule_human = describe_cron_schedule(&c.schedule);
                let script_purpose = c
                    .target_script
                    .as_deref()
                    .and_then(|p| self.storage.get_asset_by_path(p).ok().flatten())
                    .and_then(|a| a.description);
                let cron_desc = match (&c.target_script, &script_purpose) {
                    (_, Some(sp)) => format!("Cron 定时任务 ({}, 用户 {}): {}", schedule_human, c.user, sp),
                    (Some(ts), None) => format!("Cron 定时任务 ({}, 用户 {}): 执行 {}", schedule_human, c.user, ts),
                    (None, None) => format!("Cron 定时任务 ({}, 用户 {}): {}", schedule_human, c.user, c.command),
                };
                let cnode = FactNodeRecord {
                    id: cid.clone(),
                    node_type: "cron".to_string(),
                    name: format!("{}-job-{}", c.user, i),
                    label: format!("Cron 定时调度: {}", c.schedule),
                    properties: serde_json::json!({
                        "description": cron_desc,
                        "canonical_path": c.target_script.clone().unwrap_or_else(|| c.source_file.clone()),
                        "domain": "host_automation",
                        "category": "cron_job",
                        "user": c.user,
                        "schedule": c.schedule,
                        "schedule_human": schedule_human,
                        "command": c.command,
                        "target_script": c.target_script,
                        "script_purpose": script_purpose,
                        "source_file": c.source_file,
                    }),
                    updated_at: None,
                };
                self.storage.upsert_fact_node(&cnode)?;
                node_count += 1;

                if let Some(target_script) = c.target_script {
                    let fnode = self.build_file_fact_node(&target_script, "cron_target_script");
                    let fid = fnode.id.clone();
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
            let bin_path = if tool.installed {
                which_binary(&tool.tool_name)
            } else {
                None
            };
            let tnode = FactNodeRecord {
                id: tid,
                node_type: "tool".to_string(),
                name: tool.tool_name.clone(),
                label: format!("CLI工具: {}", tool.tool_name),
                properties: serde_json::json!({
                    "description": tool.description,
                    "canonical_path": bin_path.clone().unwrap_or_else(|| format!("tool://{}", tool.tool_name)),
                    "binary_path": bin_path,
                    "domain": "host_capability",
                    "category": tool.category,
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

    /// 为 fact_nodes 批量生成本地 Ollama Embedding 并注入摘要属性
    pub fn embed_fact_nodes(&self, limit: usize) -> Result<usize> {
        let client = crate::memory::embedding::EmbeddingClient::default();
        if !client.is_available() {
            return Ok(0);
        }
        let nodes = self.storage.list_all_fact_nodes(limit.max(500))?;
        let mut updated = 0usize;
        for mut n in nodes {
            if updated >= limit {
                break;
            }
            let already_ready = n
                .properties
                .get("embedding_status")
                .and_then(|v| v.as_str())
                == Some("ready");
            if already_ready {
                if let Ok(existing_vec) = self.storage.get_fact_node_embedding(&n.id) {
                    if !existing_vec.is_empty() {
                        continue;
                    }
                }
            }
            let desc = n
                .properties
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let domain = n
                .properties
                .get("domain")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let cpath = n
                .properties
                .get("canonical_path")
                .or_else(|| n.properties.get("abs_path"))
                .or_else(|| n.properties.get("full_path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let text = format!(
                "[{}] {} | {} | {} | {} | {}",
                n.node_type, n.name, n.label, domain, cpath, desc
            );
            if let Some(vec) = client.embed_text(&text) {
                self.storage.update_fact_node_embedding(&n.id, &vec)?;
                if let Some(obj) = n.properties.as_object_mut() {
                    crate::memory::embedding::inject_embedding_meta(
                        obj,
                        client.model_name(),
                        &vec,
                    );
                }
                self.storage.upsert_fact_node(&n)?;
                updated += 1;
            }
        }
        Ok(updated)
    }

    /// 构造高信息密度的 `file:*` 事实节点（优先融合 `assets` 表，缺失时实时物理探针补全）
    fn build_file_fact_node(&self, raw_path: &str, role_hint: &str) -> FactNodeRecord {
        let norm = raw_path.trim().trim_end_matches('/');
        let path_str = if norm.is_empty() { raw_path } else { norm };
        let path = Path::new(path_str);
        let file_name = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or(path_str)
            .to_string();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let mut props = serde_json::Map::new();
        props.insert("canonical_path".to_string(), serde_json::json!(path_str));
        props.insert("full_path".to_string(), serde_json::json!(path_str));
        props.insert("file_name".to_string(), serde_json::json!(file_name));
        if !ext.is_empty() {
            props.insert("extension".to_string(), serde_json::json!(ext));
        }
        props.insert("role_hint".to_string(), serde_json::json!(role_hint));

        // 1. 尝试从 assets 表回填完整语义元数据
        if let Ok(Some(asset)) = self.storage.get_asset_by_path(path_str) {
            if let Some(obj) = asset.desc_kv.as_object() {
                for (k, v) in obj {
                    props.insert(k.clone(), v.clone());
                }
            }
            props.insert("asset_type".to_string(), serde_json::json!(asset.asset_type));
            props.insert("owner".to_string(), serde_json::json!(asset.owner));
            props.insert("security_level".to_string(), serde_json::json!(asset.security_level));
            props.insert("scan_tier".to_string(), serde_json::json!(asset.scan_tier));
            if let Some(sz) = asset.size_bytes {
                props.insert("size_bytes".to_string(), serde_json::json!(sz));
            }
            if !asset.tags.is_empty() {
                props.insert("tags".to_string(), serde_json::json!(asset.tags));
            }
            if let Some(ref u) = asset.usage {
                props.insert("usage".to_string(), serde_json::json!(u));
            }
            let desc = asset
                .description
                .clone()
                .filter(|d| !d.trim().is_empty())
                .unwrap_or_else(|| format!("宿主机纳管资产 {} ({})", file_name, path_str));
            props.insert("description".to_string(), serde_json::json!(desc));

            return FactNodeRecord {
                id: format!("file:{}", path_str),
                node_type: "file".to_string(),
                name: file_name,
                label: format!("依赖资产: {}", path_str),
                properties: serde_json::Value::Object(props),
                updated_at: None,
            };
        }

        // 2. 系统级/外部路径：实时物理探针补全
        let meta = fs::symlink_metadata(path).ok();
        let exists = meta.is_some();
        let is_dir = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let is_symlink = meta.as_ref().map(|m| m.file_type().is_symlink()).unwrap_or(false);
        let size_bytes = fs::metadata(path).ok().map(|m| m.len());
        let last_modified_at = fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let dt: DateTime<Local> = t.into();
                dt.to_rfc3339()
            });
        #[cfg(unix)]
        let (file_mode, is_executable) = fs::metadata(path)
            .ok()
            .map(|m| {
                use std::os::unix::fs::PermissionsExt;
                let mode = m.permissions().mode() & 0o7777;
                (Some(format!("{:04o}", mode)), (mode & 0o111) != 0)
            })
            .unwrap_or((None, false));
        #[cfg(not(unix))]
        let (file_mode, is_executable): (Option<String>, bool) = (None, false);

        let file_kind = detect_system_file_kind(path, is_dir, is_executable, &ext);
        let domain = if path_str.starts_with("/etc/") {
            "system_config"
        } else if path_str.starts_with("/usr/") || path_str.starts_with("/bin/") || path_str.starts_with("/sbin/") {
            "system_runtime"
        } else if path_str.starts_with("/run/") || path_str.starts_with("/var/") {
            "system_state"
        } else {
            "host_file"
        };

        let description = if !exists {
            format!("引用路径 {} (当前物理路径不存在或待挂载)", path_str)
        } else if is_dir {
            format!("系统目录 {} (由 {} 引用)", path_str, role_hint)
        } else {
            format!(
                "{} {} ({}, {} bytes{})",
                file_kind_zh(&file_kind),
                file_name,
                path_str,
                size_bytes.unwrap_or(0),
                if is_executable { ", 可执行" } else { "" }
            )
        };

        props.insert("description".to_string(), serde_json::json!(description));
        props.insert("domain".to_string(), serde_json::json!(domain));
        props.insert("category".to_string(), serde_json::json!(file_kind));
        props.insert("exists".to_string(), serde_json::json!(exists));
        props.insert("is_dir".to_string(), serde_json::json!(is_dir));
        props.insert("is_symlink".to_string(), serde_json::json!(is_symlink));
        props.insert("is_executable".to_string(), serde_json::json!(is_executable));
        if let Some(sz) = size_bytes {
            props.insert("size_bytes".to_string(), serde_json::json!(sz));
        }
        if let Some(mode) = file_mode {
            props.insert("file_mode".to_string(), serde_json::json!(mode));
        }
        if let Some(mtime) = last_modified_at {
            props.insert("last_modified_at".to_string(), serde_json::json!(mtime));
        }

        FactNodeRecord {
            id: format!("file:{}", path_str),
            node_type: "file".to_string(),
            name: file_name,
            label: format!("配置/执行依赖: {}", path_str),
            properties: serde_json::Value::Object(props),
            updated_at: None,
        }
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

fn infer_placement_zone(abs_path: &str) -> &'static str {
    if abs_path.starts_with("/root/new/shared_proj/") {
        "shared_proj"
    } else if abs_path.starts_with("/root/new/shared_git/") {
        "shared_git"
    } else if abs_path.starts_with("/root/new/src/") {
        "src"
    } else if abs_path.starts_with("/root/new/work/") {
        "work"
    } else {
        "other"
    }
}

fn extract_project_from_path(path: &str) -> Option<String> {
    for prefix in &["/root/new/work/", "/root/new/shared_proj/", "/root/new/src/", "/root/new/shared_git/"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            let name = rest.split('/').next().unwrap_or("");
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
    }
    None
}

fn probe_git_remote(repo_dir: &Path) -> Option<String> {
    let cfg = fs::read_to_string(repo_dir.join(".git").join("config")).ok()?;
    let mut in_origin = false;
    for line in cfg.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_origin = t.contains("remote \"origin\"");
            continue;
        }
        if in_origin {
            if let Some(url) = t.strip_prefix("url =") {
                return Some(url.trim().to_string());
            }
        }
    }
    None
}

fn probe_project_readme_summary(repo_dir: &Path) -> Option<String> {
    for name in &["README.md", "README_zh.md", "README.rst", "README.txt"] {
        let p = repo_dir.join(name);
        if let Ok(content) = fs::read_to_string(p) {
            for line in content.lines().take(25) {
                let t = line.trim();
                if t.is_empty() || t.starts_with("![") || t.starts_with("[![") || t.starts_with("---") || t.starts_with('<') {
                    continue;
                }
                let clean = t.trim_start_matches('#').trim_start_matches('>').trim();
                if clean.chars().count() >= 6 {
                    return Some(clean.chars().take(90).collect());
                }
            }
        }
    }
    None
}

fn probe_top_level_entries(repo_dir: &Path, limit: usize) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(repo_dir) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if !name.starts_with('.') {
                out.push(name);
            }
        }
    }
    out.sort();
    out.truncate(limit);
    out
}

fn describe_well_known_port(port: u16, proc_name: Option<&str>) -> Option<&'static str> {
    if let Some(pn) = proc_name {
        if pn.contains("agent-steward") {
            return Some("Agent Host Steward Web 监控大屏与 REST API");
        }
        if pn.contains("ollama") {
            return Some("Ollama 本地大模型推理与 Embedding 向量服务");
        }
        if pn.contains("postgres") {
            return Some("PostgreSQL 关系型与向量数据库服务");
        }
    }
    match port {
        22 => Some("OpenSSH 远程安全登录服务"),
        80 | 443 => Some("HTTP/HTTPS Web 入口服务"),
        5432 => Some("PostgreSQL 数据库监听端口"),
        6379 => Some("Redis 内存缓存服务"),
        8756 => Some("Agent Host Steward Web 监控大屏"),
        11434 => Some("Ollama 本地 LLM 与 Embedding 服务"),
        _ => None,
    }
}

fn describe_cron_schedule(expr: &str) -> String {
    let trimmed = expr.trim();
    if trimmed.starts_with('@') {
        return match trimmed {
            "@hourly" => "每小时执行一次".to_string(),
            "@daily" | "@midnight" => "每天 00:00 执行".to_string(),
            "@weekly" => "每周执行一次".to_string(),
            "@monthly" => "每月执行一次".to_string(),
            "@reboot" => "系统启动时执行".to_string(),
            _ => format!("周期调度 ({})", trimmed),
        };
    }
    let parts: Vec<&str> = trimmed.split_whitespace().collect();
    if parts.len() == 5 {
        let (min, hour, dom, mon, dow) = (parts[0], parts[1], parts[2], parts[3], parts[4]);
        if let Some(step) = min.strip_prefix("*/") {
            if hour == "*" && dom == "*" && mon == "*" && dow == "*" {
                return format!("每 {} 分钟执行", step);
            }
        }
        if min.chars().all(|c| c.is_ascii_digit()) && hour == "*" && dom == "*" && mon == "*" && dow == "*" {
            return format!("每小时第 {} 分执行", min);
        }
        if min.chars().all(|c| c.is_ascii_digit()) && hour.chars().all(|c| c.is_ascii_digit()) && dom == "*" && mon == "*" && dow == "*" {
            return format!("每天 {:0>2}:{:0>2} 执行", hour, min);
        }
    }
    format!("Cron 表达式 ({})", trimmed)
}

fn which_binary(cmd: &str) -> Option<String> {
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in path_var.split(':') {
            let cand = Path::new(dir).join(cmd);
            if cand.is_file() {
                return Some(cand.to_string_lossy().to_string());
            }
        }
    }
    None
}

fn detect_system_file_kind(path: &Path, is_dir: bool, is_executable: bool, ext: &str) -> String {
    if is_dir {
        return "directory".to_string();
    }
    if let Ok(mut f) = fs::File::open(path) {
        let mut buf = [0u8; 4];
        if f.read(&mut buf).unwrap_or(0) == 4 && buf == [0x7f, b'E', b'L', b'F'] {
            return "elf_binary".to_string();
        }
    }
    match ext {
        "py" => "python_script".to_string(),
        "sh" | "bash" => "shell_script".to_string(),
        "md" => "markdown_doc".to_string(),
        "toml" | "json" | "yaml" | "yml" | "conf" | "ini" | "service" => "config_file".to_string(),
        _ if is_executable => "executable_script".to_string(),
        _ => "system_file".to_string(),
    }
}

fn file_kind_zh(kind: &str) -> &'static str {
    match kind {
        "elf_binary" => "ELF 可执行二进制",
        "python_script" => "Python 脚本",
        "shell_script" => "Shell 脚本",
        "markdown_doc" => "Markdown 文档",
        "config_file" => "系统配置文件",
        "executable_script" => "可执行脚本",
        _ => "系统文件",
    }
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
