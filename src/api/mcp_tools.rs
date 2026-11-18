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
use crate::brief::{BriefAssembler, BriefRequest};
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
                "description": "一键获取宿主机自适应情境简报（事实图谱、工程偏好规约与历史决策经验三位一体组装）。Agent 在新会话开始或切换任务时必调。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "本次任务意图或关键词（用于语义召回匹配的宿主机工具能力、相关仓库与历史决策经验，留空则返回全局概览）"
                        },
                        "session_id": {
                            "type": "string",
                            "description": "当前 Agent 会话 ID（用于拉取该会话的关键决策与任务上下文）"
                        },
                        "budget_tokens": {
                            "type": "integer",
                            "description": "情境预算上限 tokens（默认 1500，严格防爆上下文）",
                            "default": 1500
                        }
                    }
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
                "description": "语义与全文混合检索历史记忆与决策记录（支持 RRF 混合融合排序与近义词召回）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "搜索关键词或语义描述" },
                        "limit": { "type": "integer", "default": 10 },
                        "hybrid": { "type": "boolean", "default": true, "description": "是否启用 Ollama 向量+FTS5 混合重排" }
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
            }),
            serde_json::json!({
                "name": "steward_links_impact",
                "description": "变更爆炸半径推演：评估重构、改动或删除特定文件/服务对上下游的影响范围，返回直接与多跳间接依赖列表及风险评级。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "target": { "type": "string", "description": "被评估对象的绝对路径或服务标识" },
                        "max_depth": { "type": "integer", "default": 3, "description": "依赖推演最大跳数" }
                    },
                    "required": ["target"]
                }
            }),
            serde_json::json!({
                "name": "steward_links_audit",
                "description": "全局依赖拓扑审计：主动扫描全局依赖图中的断链与悬空引用（目标文件不存在、依赖服务离线），输出健康评分与修复建议。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "fix_dangling": { "type": "boolean", "default": false, "description": "是否自动清理已确认失效的文件依赖死链" }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_codegraph_bridge",
                "description": "CodeGraph 宏微联邦桥接：跨层联动局部工程内的 CodeGraph 代码图，查看代码级函数/类调用方或项目代码图统计。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "项目工程目录或文件路径" },
                        "symbol": { "type": "string", "description": "待查询的函数、类或符号名称（可选）" }
                    },
                    "required": ["path"]
                }
            }),
            serde_json::json!({
                "name": "steward_session_close",
                "description": "会话完工结算与环境治理：在任务结束时清理临时测试文件、汇总环境变更、并将工单成果归档沉淀到长期记忆。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "当前会话的唯一标识" },
                        "clean_temp": { "type": "boolean", "default": true, "description": "是否清理 /root/new/tmp/ 下属于本会话的临时文件" }
                    },
                    "required": ["session_id"]
                }
            }),
            serde_json::json!({
                "name": "steward_links_repair",
                "description": "断链智能修复与重定向推演：基于文件名相似度、代码指纹与 Git 历史寻找已迁移/重命名文件，生成 diff 补丁并可自动修正依赖关系。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "apply": { "type": "boolean", "default": false, "description": "是否直接应用修复修改源文件与关联表" }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_placement_learn",
                "description": "放置规范动态自演化：聚类分析历史资产分布与创建事件，自动推演发现潜在的新放置规则候选，并支持一键采纳持久化。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "min_samples": { "type": "integer", "default": 3, "description": "聚类最小样本数" },
                        "accept": { "type": "boolean", "default": false, "description": "是否自动采纳并入库所有发现的规则候选" }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_env_gc",
                "description": "宿主资源垃圾回收与孤儿进程诊断：安全扫描 /root/new/tmp/ 超期孤儿临时文件与后台僵尸测试进程，支持预览与清理。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tmp_dir": { "type": "string", "default": "/root/new/tmp", "description": "待清理的临时目录路径 (强制安全白名单)" },
                        "max_age_hours": { "type": "integer", "default": 72, "description": "文件超期阈值 (小时)" },
                        "apply": { "type": "boolean", "default": false, "description": "是否直接执行清理删除 (默认仅 dry-run 预览)" }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_memory_audit",
                "description": "长期记忆健康体检与矛盾仲裁：检测对立冲突的认知记忆对，计算温和时间衰减淘汰候选，支持指定归档旧条目。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "archive_id": { "type": "string", "description": "若存在冲突，指定归档淘汰的旧记忆 ID" }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_fact_query",
                "description": "宿主机客观事实图谱只读查询（零幻觉真相底座）。查询：1. 机器已装CLI工具与能力推荐(如'有没有处理PDF的东西'、'如何转视频雪碧图')；2. 端口被谁占用与排障(如'8080')；3. 修改某配置会影响哪些systemd服务或cron调度；4. 本地已克隆哪些开源源码(CodeGraph状态)与权威资料清单等。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "自然语言意图或关键词（例如 '处理 PDF 提取文字', '8080', '/root/.steward/config.toml', '权威网站'）"
                        },
                        "fact_type": {
                            "type": "string",
                            "enum": ["auto", "capability", "port", "automation", "impact", "repo", "doc", "graph", "all"],
                            "default": "auto",
                            "description": "事实类别过滤，留空或 auto 为智能识别"
                        }
                    },
                    "required": ["query"]
                }
            }),
            serde_json::json!({
                "name": "steward_hook_pre",
                "description": "Agent 工作流前置检查钩子。在 Agent 执行工具调用前调用，排查即将执行的动作是否存在端口冲突、对核心系统服务的破坏性依赖变更、或文件乱放违规等风险，提供避坑指引。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tool_name": {
                            "type": "string",
                            "description": "即将调用的工具名称（如 run_command, write_file, bind_port, edit_file 等）"
                        },
                        "args": {
                            "type": "object",
                            "description": "工具入参对象（如包含 port, path, command 等参数键值）"
                        }
                    },
                    "required": ["tool_name", "args"]
                }
            }),
            serde_json::json!({
                "name": "steward_hook_post",
                "description": "Agent 工作流后置通知钩子。在 Agent 完成工具执行后调用，通知大管家记录事实变更并触发事实属性图与依赖链增量刷新。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tool_name": {
                            "type": "string",
                            "description": "刚刚执行完成的工具名称（如 write_file, run_command 等）"
                        },
                        "args": {
                            "type": "object",
                            "description": "工具入参对象"
                        },
                        "success": {
                            "type": "boolean",
                            "default": true,
                            "description": "该工具执行是否成功"
                        }
                    },
                    "required": ["tool_name", "args"]
                }
            })
        ]
    }

    /// 执行具体工具调用
    pub fn call_tool(&self, name: &str, args: Value) -> Result<String> {
        match name {
            "steward_brief" => {
                let provider = Arc::new(crate::memory::builtin_provider::BuiltinMemoryProvider::new(self.storage.clone()));
                let assembler = BriefAssembler::new(self.storage.clone(), Some(provider));
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let session_id = args.get("session_id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let budget_tokens = args.get("budget_tokens").and_then(|v| v.as_u64()).unwrap_or(1500) as usize;

                let req = BriefRequest {
                    query,
                    session_id,
                    budget_tokens,
                };
                let output = assembler.assemble(&req)?;
                Ok(output.markdown)
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
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                let use_hybrid = args.get("hybrid").and_then(|v| v.as_bool()).unwrap_or(true);

                if use_hybrid {
                    let arc_storage = Arc::new(self.storage.clone());
                    let engine = crate::memory::hybrid::HybridSearchEngine::new(arc_storage);
                    let rt = tokio::runtime::Handle::current();
                    let results = tokio::task::block_in_place(|| {
                        rt.block_on(engine.search_hybrid(query, limit))
                    })?;

                    let mut out = format!("记忆检索结果 (混合模式 RRF, 共 {} 条):\n", results.len());
                    for r in results {
                        let sim_str = r.cosine_similarity.map(|s| format!(" (语义相似度: {:.2})", s)).unwrap_or_default();
                        out.push_str(&format!("  - [{}] {} [RRF: {:.4}]{} (ID: {})\n",
                            r.memory.memory_type, r.memory.content, r.rrf_score, sim_str, r.memory.id));
                    }
                    Ok(out)
                } else {
                    let mm = MemoryManager::new(self.storage.clone());
                    let results = mm.search(query, limit)?;
                    let mut out = format!("记忆检索结果 (FTS 纯文本, 共 {} 条):\n", results.len());
                    for m in results {
                        out.push_str(&format!("  - [{}] {} (ID: {})\n", m.memory_type, m.content, m.id));
                    }
                    Ok(out)
                }
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

            "steward_links_impact" => {
                let arc_storage = Arc::new(self.storage.clone());
                let analyzer = crate::links::impact::ImpactAnalyzer::new(arc_storage);
                let target = args.get("target").and_then(|v| v.as_str()).unwrap_or("");
                let max_depth = args.get("max_depth").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
                let res = analyzer.analyze_impact(target, max_depth)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "steward_links_audit" => {
                let arc_storage = Arc::new(self.storage.clone());
                let auditor = crate::links::audit::LinkAuditor::new(arc_storage);
                let fix_dangling = args.get("fix_dangling").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = auditor.audit_links(fix_dangling)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "steward_codegraph_bridge" => {
                let path_str = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
                let symbol_opt = args.get("symbol").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty());
                let p = std::path::Path::new(path_str);

                if let Some(symbol) = symbol_opt {
                    let callers = crate::links::codegraph_bridge::CodeGraphBridge::query_symbol_callers(p, symbol)?;
                    Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "path": path_str,
                        "symbol": symbol,
                        "callers_count": callers.len(),
                        "callers": callers,
                    }))?)
                } else {
                    let info = crate::links::codegraph_bridge::CodeGraphBridge::inspect_project(p)?;
                    Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "path": path_str,
                        "has_codegraph": info.is_some(),
                        "project_info": info,
                    }))?)
                }
            }

            "steward_session_close" => {
                let arc_storage = Arc::new(self.storage.clone());
                let sm = crate::session::SessionManager::new(arc_storage);
                let session_id = args.get("session_id").and_then(|v| v.as_str()).unwrap_or("default");
                let clean_temp = args.get("clean_temp").and_then(|v| v.as_bool()).unwrap_or(true);
                let res = sm.close_session(session_id, clean_temp)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "steward_links_repair" => {
                let arc_storage = Arc::new(self.storage.clone());
                let repairer = crate::links::repair::LinkRepairer::new(arc_storage);
                let apply = args.get("apply").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = repairer.repair_links(apply)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "steward_placement_learn" => {
                let learner = crate::placement::learner::PlacementLearner::new(&self.storage);
                let min_samples = args.get("min_samples").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
                let accept = args.get("accept").and_then(|v| v.as_bool()).unwrap_or(false);
                let rep = learner.learn_rules(min_samples)?;
                let mut accepted_rules = Vec::new();
                if accept {
                    for c in &rep.candidates {
                        let rule = learner.apply_candidate(c)?;
                        accepted_rules.push(rule);
                    }
                }
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "learn_report": rep,
                    "accepted_count": accepted_rules.len(),
                    "accepted_rules": accepted_rules,
                }))?)
            }

            "steward_env_gc" => {
                let tmp_dir_str = args.get("tmp_dir").and_then(|v| v.as_str()).unwrap_or("/root/new/tmp");
                let max_age_hours = args.get("max_age_hours").and_then(|v| v.as_u64()).unwrap_or(72);
                let apply = args.get("apply").and_then(|v| v.as_bool()).unwrap_or(false);
                let p = std::path::Path::new(tmp_dir_str);
                let res = crate::env::gc::HostGarbageCollector::run_gc(p, max_age_hours, !apply)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "steward_memory_audit" => {
                let mgr = crate::memory::lifecycle::MemoryLifecycleManager::new(&self.storage);
                if let Some(aid) = args.get("archive_id").and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()) {
                    mgr.archive_memory(aid)?;
                }
                let rep = mgr.audit_memories()?;
                Ok(serde_json::to_string_pretty(&rep)?)
            }

            "steward_fact_query" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("all");
                let fact_type = args.get("fact_type").and_then(|v| v.as_str()).unwrap_or("auto");
                crate::fact::FactFacade::query_facts(fact_type, query)
            }

            "steward_hook_pre" => {
                let tool_name = args.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                let tool_args = args.get("args").cloned().unwrap_or(serde_json::json!({}));
                let engine = crate::api::hooks::AgentHookEngine::new(self.storage.clone());
                let res = engine.pre_tool_call(tool_name, &tool_args);
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "steward_hook_post" => {
                let tool_name = args.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                let tool_args = args.get("args").cloned().unwrap_or(serde_json::json!({}));
                let success = args.get("success").and_then(|v| v.as_bool()).unwrap_or(true);
                let engine = crate::api::hooks::AgentHookEngine::new(self.storage.clone());
                let res = engine.post_tool_call(tool_name, &tool_args, success);
                Ok(serde_json::to_string_pretty(&res)?)
            }

            _ => bail!("Unknown tool: {}", name),
        }
    }
}
