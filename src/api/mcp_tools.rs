use std::path::PathBuf;
use serde_json::Value;
use anyhow::{bail, Result};
use crate::storage::Storage;
use crate::storage::models::AssetRecord;
use crate::placement::{PlacementEngine, PlacementRequest};
use crate::scanner::Scanner;
use crate::env::EnvCollector;
use crate::memory::MemoryManager;
use crate::links::LinksEngine;
use crate::changes::ChangesEngine;
use std::sync::Arc;

pub struct McpToolRegistry {
    storage: Storage,
}

impl McpToolRegistry {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// 列出所有注册的 MCP 工具及其标准 JSON Schema
    pub fn list_tools(&self) -> Vec<Value> {
        vec![
            serde_json::json!({
                "name": "steward_brief",
                "description": "一键获取宿主机全局上下文简报（系统硬件规格、活跃动态进程、最近资产与关键记忆）。Agent 在新会话开始时必调。",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "steward_placement_suggest",
                "description": "新对象放置建议。在准备新建项目、脚本、笔记、临时文件前必须调用，获取规范存放路径、命名约定与安全脱敏指引，严禁随手乱放。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "object_type": {
                            "type": "string",
                            "enum": ["project", "script", "skill", "note", "tmp", "src", "other"],
                            "description": "准备生成的对象类别"
                        },
                        "name": {
                            "type": "string",
                            "description": "对象名称或建议文件名 (例如 'sec_scanner', 'pg_tuning_notes')"
                        },
                        "intent": {
                            "type": "string",
                            "description": "对象的用途与生命周期预期 (例如 '长期复用的美股数据抓取工具', '临时调试运行日志')"
                        },
                        "security_level": {
                            "type": "string",
                            "enum": ["public", "internal", "confidential", "secret"],
                            "default": "internal",
                            "description": "保密级别"
                        }
                    },
                    "required": ["object_type", "name", "intent"]
                }
            }),
            serde_json::json!({
                "name": "steward_placement_rules",
                "description": "查看当前生效的宿主机目录放置规范规则库（shared_proj, work, skills, src, note, tmp 等）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "steward_asset_search",
                "description": "全文检索与语义搜索宿主机已纳管的代码、工具、项目、文档及配置资产。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "搜索关键词或自然语言描述"
                        },
                        "limit": {
                            "type": "integer",
                            "default": 10,
                            "description": "最多返回结果数"
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "steward_asset_register",
                "description": "手动向大管家登记一个文件或目录资产，附带结构化意图描述（desc_kv）、所有者与保密级别。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "文件或目录的绝对路径" },
                        "type": {
                            "type": "string",
                            "enum": ["directory", "project", "script", "document", "dataset", "config", "media", "other"],
                            "description": "资产类型"
                        },
                        "owner": { "type": "string", "default": "user:digoal", "description": "归属主体" },
                        "security_level": {
                            "type": "string",
                            "enum": ["public", "internal", "confidential", "secret"],
                            "default": "internal"
                        },
                        "description": { "type": "string", "description": "文字描述" },
                        "desc_kv": { "type": "object", "description": "结构化键值 (如 {'purpose': '抓取', 'status': 'ready'})" }
                    },
                    "required": ["path", "type"]
                }
            }),
            serde_json::json!({
                "name": "steward_asset_scan",
                "description": "对指定目录触发分级颗粒度扫描（白名单递归深入解析，黑名单严格止于当层，杜绝扫爆数据库）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "要扫描的目标目录绝对路径" }
                    },
                    "required": ["path"]
                }
            }),
            serde_json::json!({
                "name": "steward_env_processes",
                "description": "查询宿主机上除系统守护服务外活跃的动态任务与进程（过滤底噪，识别 Python 脚本、构建 Worker、CPU/内存/GPU 占用与命令）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "steward_env_show",
                "description": "查看宿主机硬件规格（OS 版本、CPU 核心数、内存总量/使用率、磁盘挂载）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "steward_memory_add",
                "description": "向大管家写入一条长期记忆（决策依据、排障经验、技术偏好或事实），跨会话持久化。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["decision", "task", "experience", "preference", "fact", "note"],
                            "default": "decision"
                        },
                        "content": { "type": "string", "description": "记忆正文" },
                        "summary": { "type": "string", "description": "一句话摘要" }
                    },
                    "required": ["content"]
                }
            }),
            serde_json::json!({
                "name": "steward_memory_search",
                "description": "全文检索历史记忆与决策记录。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "搜索关键词" },
                        "limit": { "type": "integer", "default": 10 }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "steward_links_query",
                "description": "查询指定实体（资产路径、服务名、记忆 ID）的双向关联依赖拓扑与图关系。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "type": { "type": "string", "description": "实体类型: asset, service, memory", "default": "asset" },
                        "id": { "type": "string", "description": "实体标识 (如绝对路径 /root/new/work/demo.py 或服务名 postgresql:5432)" }
                    },
                    "required": ["id"]
                }
            }),
            serde_json::json!({
                "name": "steward_links_visualize",
                "description": "导出宿主机实体关系图拓扑结构为 Mermaid 格式，直观呈现依赖图景。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "default": 20 }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_changes_since",
                "description": "查询自某个时间点以来宿主机的变更历史（新增/修改文件、环境扰动等），Agent 开工前感知上下文变动。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "since": { "type": "string", "description": "起始时间戳 (ISO 8601 格式，如 2026-11-15T00:00:00，缺省则返回最近记录)" },
                        "limit": { "type": "integer", "default": 20 }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_port_check",
                "description": "端口防撞预警与空闲推荐：启动服务或绑定端口前检测是否冲突，若冲突返回占用进程详情与推荐可用端口。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "port": { "type": "integer", "description": "拟占用的网络端口号 (1-65535)" },
                        "suggest_count": { "type": "integer", "default": 3, "description": "若冲突返回推荐空闲端口个数" }
                    },
                    "required": ["port"]
                }
            })
        ]
    }

    /// 执行具体工具调用
    pub fn call_tool(&self, name: &str, args: Value) -> Result<String> {
        match name {
            "steward_brief" => {
                let env_collector = EnvCollector::new(self.storage.clone());
                let summary = env_collector.collect_system_summary();
                let procs = env_collector.collect_and_sync_processes()?;
                let assets = self.storage.list_assets(5)?;

                let mut out = String::new();
                out.push_str("=== Steward Brief 宿主上下文简报 ===\n");
                out.push_str(&format!("系统: {} | 内核: {} | 内存使用率: {:.1}%\n\n", summary.os_name, summary.kernel_version, summary.memory_used_pct));
                out.push_str("【活跃动态任务进程】(前 5):\n");
                for p in procs.iter().take(5) {
                    let mem_mb = p.mem_bytes as f64 / (1024.0 * 1024.0);
                    out.push_str(&format!("  - [PID {}] {} (CPU {:.1}%, 内存 {:.1}MB) -> {}\n", p.pid, p.name, p.cpu_pct, mem_mb, p.command));
                }
                out.push_str("\n【最近活跃资产】:\n");
                for a in assets {
                    out.push_str(&format!("  - [{:8}] {} (层级: {})\n", a.asset_type, a.path, a.scan_tier));
                }
                Ok(out)
            }

            "steward_placement_suggest" => {
                let engine = PlacementEngine::new(self.storage.clone());
                let object_type = args.get("object_type").and_then(|v| v.as_str()).unwrap_or("project").to_string();
                let name_arg = args.get("name").and_then(|v| v.as_str()).unwrap_or("unnamed").to_string();
                let intent = args.get("intent").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let security_level = args.get("security_level").and_then(|v| v.as_str()).unwrap_or("internal").to_string();

                let req = PlacementRequest {
                    object_type,
                    name: name_arg,
                    intent,
                    tags: vec![],
                    security_level,
                };
                let suggestion = engine.suggest(&req)?;

                let mut out = String::new();
                out.push_str("=== Steward 对象放置建议 ===\n");
                out.push_str(&format!("推荐目标路径: {:?}\n", suggestion.suggested_path));
                out.push_str(&format!("命中规范规则: {}\n", suggestion.rule_matched));
                out.push_str(&format!("规则理由:     {}\n", suggestion.reason));
                out.push_str(&format!("命名建议:     {}\n", suggestion.naming_convention));
                out.push_str(&format!("冲突状态:     {}\n", if suggestion.conflict_detected { "⚠️ 路径已存在冲突！" } else { "✅ 路径空闲可用" }));
                out.push_str("伴随开发与安全指引:\n");
                for g in suggestion.guidelines {
                    out.push_str(&format!("  * {}\n", g));
                }
                Ok(out)
            }

            "steward_placement_rules" => {
                let rules = self.storage.get_placement_rules()?;
                let mut out = format!("当前生效的宿主机目录放置规范 (共 {} 条):\n", rules.len());
                for r in rules {
                    out.push_str(&format!("  - [{}] {} -> {} (优先级 {})\n", r.rule_name, r.pattern_type, r.target_base_dir, r.priority));
                    if let Some(desc) = r.description {
                        out.push_str(&format!("    说明: {}\n", desc));
                    }
                }
                Ok(out)
            }

            "steward_asset_search" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

                let results = self.storage.search_assets_fts(query, limit)?;
                let mut out = format!("资产搜索匹配结果 (共 {} 条):\n", results.len());
                for a in results {
                    out.push_str(&format!("  - [{}] {} | {}\n", a.asset_type, a.path, a.description.unwrap_or_default()));
                }
                Ok(out)
            }

            "steward_asset_register" => {
                let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let asset_type = args.get("type").and_then(|v| v.as_str()).unwrap_or("other");
                let owner = args.get("owner").and_then(|v| v.as_str()).unwrap_or("user:digoal");
                let security_level = args.get("security_level").and_then(|v| v.as_str()).unwrap_or("internal");
                let description = args.get("description").and_then(|v| v.as_str()).map(|s| s.to_string());
                let desc_kv = args.get("desc_kv").cloned().unwrap_or(serde_json::json!({}));

                let record = AssetRecord {
                    id: uuid::Uuid::new_v4().to_string(),
                    path: path.to_string(),
                    asset_type: asset_type.to_string(),
                    owner: owner.to_string(),
                    security_level: security_level.to_string(),
                    scan_tier: "deep".to_string(),
                    description,
                    desc_kv,
                    usage: None,
                    size_bytes: None,
                    file_count: None,
                    tags: vec![],
                    status: "active".to_string(),
                };
                self.storage.upsert_asset(&record)?;
                Ok(format!("✅ 资产登记成功: {} [{}]", path, asset_type))
            }

            "steward_asset_scan" => {
                let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let scanner = Scanner::new(self.storage.clone())?;
                let report = scanner.scan_path(&PathBuf::from(path_str))?;

                let mut out = format!("分级颗粒度扫描报告 ({:?}):\n", path_str);
                out.push_str(&format!("  - 扫描目录数:       {}\n", report.scanned_dirs));
                out.push_str(&format!("  - 深层索引文件数:   {}\n", report.deep_indexed_files));
                out.push_str(&format!("  - 黑名单阻断目录数: {}\n", report.shallow_blocked_dirs.len()));
                for b in report.shallow_blocked_dirs {
                    out.push_str(&format!("    ★ [Shallow 阻断] {} ({})\n", b.path, b.reason));
                }
                Ok(out)
            }

            "steward_env_processes" => {
                let collector = EnvCollector::new(self.storage.clone());
                let procs = collector.collect_and_sync_processes()?;
                let mut out = format!("当前活跃动态任务进程 (共 {} 个):\n", procs.len());
                for p in procs {
                    let mem_mb = p.mem_bytes as f64 / (1024.0 * 1024.0);
                    out.push_str(&format!("  - [PID {}] {} (CPU {:.1}%, 内存 {:.1}MB) [{}] -> {}\n", p.pid, p.name, p.cpu_pct, mem_mb, p.owner, p.command));
                }
                Ok(out)
            }

            "steward_env_show" => {
                let collector = EnvCollector::new(self.storage.clone());
                let s = collector.collect_system_summary();
                let mut out = String::new();
                out.push_str("=== 系统与硬件规格 ===\n");
                out.push_str(&format!("OS:     {}\n", s.os_name));
                out.push_str(&format!("Kernel: {}\n", s.kernel_version));
                out.push_str(&format!("CPU:    {} 核心 ({})\n", s.cpu_cores, s.cpu_brand));
                out.push_str(&format!("Memory: {} MB 总量 (已用: {} MB, {:.1}%)\n", s.total_memory_mb, s.used_memory_mb, s.memory_used_pct));
                for d in s.disks {
                    out.push_str(&format!("Disk:   {:<10} 总计 {:.1} GB / 可用 {:.1} GB\n", d.mount_point, d.total_space_gb, d.available_space_gb));
                }
                Ok(out)
            }

            "steward_memory_add" => {
                let mm = MemoryManager::new(self.storage.clone());
                let mem_type = args.get("type").and_then(|v| v.as_str()).unwrap_or("decision");
                let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let summary = args.get("summary").and_then(|v| v.as_str());

                let id = mm.add(mem_type, content, summary, Some("mcp"), vec![])?;
                Ok(format!("✅ 长期记忆已录入, ID: {}", id))
            }

            "steward_memory_search" => {
                let mm = MemoryManager::new(self.storage.clone());
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

                let results = mm.search(query, limit)?;
                let mut out = format!("记忆检索结果 (共 {} 条):\n", results.len());
                for m in results {
                    out.push_str(&format!("  - [{}] {} (ID: {})\n", m.memory_type, m.content, m.id));
                }
                Ok(out)
            }

            "steward_links_query" => {
                let arc_storage = Arc::new(self.storage.clone());
                let engine = LinksEngine::new(arc_storage);
                let node_type = args.get("type").and_then(|v| v.as_str()).unwrap_or("asset");
                let node_id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");

                let links = engine.query_links(node_type, node_id)?;
                let mut out = format!("实体关联拓扑 (共 {} 条):\n", links.len());
                for l in links {
                    out.push_str(&format!("  - [{}] {} --({})--> [{}] {} (置信度: {:.2})\n",
                        l.source_type, l.source_id, l.relation, l.target_type, l.target_id, l.confidence));
                }
                Ok(out)
            }

            "steward_links_visualize" => {
                let arc_storage = Arc::new(self.storage.clone());
                let engine = LinksEngine::new(arc_storage);
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                engine.export_mermaid(limit)
            }

            "steward_changes_since" => {
                let arc_storage = Arc::new(self.storage.clone());
                let engine = ChangesEngine::new(arc_storage);
                let since = args.get("since").and_then(|v| v.as_str());
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                engine.generate_summary_markdown(since, limit)
            }

            "steward_port_check" => {
                let port = args.get("port").and_then(|v| v.as_u64()).unwrap_or(8080) as u16;
                let suggest_count = args.get("suggest_count").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
                let res = crate::env::port::PortChecker::check_port(port, suggest_count)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            _ => bail!("Unknown tool: {}", name),
        }
    }
}
