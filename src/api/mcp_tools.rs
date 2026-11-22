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
                "description": "全文检索与语义搜索宿主机已纳管的代码、工具、项目、文档及配置资产（支持 micro/standard/full 三级渐进式 Token 披露）。",
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
                        },
                        "detail_level": {
                            "type": "string",
                            "enum": ["micro", "standard", "full"],
                            "default": "standard",
                            "description": "元数据披露粒度: micro(~25 tok) | standard(~85 tok, 剥离管线噪音) | full(~180 tok, 含完整desc_kv)"
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
                "description": "对指定目录触发分级颗粒度扫描（同步完成 L1 确定性元数据提取；可选开启本地 Ollama 小模型 L2 异步语义精炼）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "要扫描的目标目录绝对路径" },
                        "enrich_with_ollama": {
                            "type": "boolean",
                            "default": false,
                            "description": "是否顺便调用本地 Ollama 小模型对高价值资产做 L2 语义精炼"
                        },
                        "enrich_limit": {
                            "type": "integer",
                            "default": 10,
                            "description": "当 enrich_with_ollama=true 时本次最多精炼的资产数"
                        }
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
                "description": "查询指定实体（资产路径、服务名、项目或记忆 ID）的双向关联依赖与分层属性图谱关系（L1 物理/L2 结构/L3 语义）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "type": { "type": "string", "description": "实体类型: asset, service, project, tool, memory", "default": "asset" },
                        "id": { "type": "string", "description": "实体标识 (如绝对路径、project:<name> 或 service:<name>)" },
                        "max_hops": { "type": "integer", "default": 2, "description": "属性图谱遍历最大跳数" },
                        "min_weight": { "type": "number", "default": 0.0, "description": "最低边权重过滤 (0.0~1.0)" },
                        "include_semantic": { "type": "boolean", "default": true, "description": "是否包含 L3_SEMANTIC 隐式语义边" }
                    },
                    "required": ["id"]
                }
            }),
            serde_json::json!({
                "name": "steward_links_visualize",
                "description": "导出宿主机实体关系图拓扑结构为 Mermaid 格式，支持按需触发 L2/L3 关系挖掘。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "default": 20 },
                        "discover_semantic": { "type": "boolean", "default": false, "description": "导出前是否运行 L2 结构上卷与 L3 隐式语义关系挖掘" }
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
                "description": "变更爆炸半径推演：融合文件 links 与属性图 fact_edges，按跳数衰减与权重计算影响范围，支持按证据层级与阈值过滤。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "target": { "type": "string", "description": "被评估对象的绝对路径、project:<name> 或服务标识" },
                        "max_depth": { "type": "integer", "default": 3, "description": "依赖推演最大跳数" },
                        "min_weight": { "type": "number", "default": 0.5, "description": "最低边权重阈值 (0.0~1.0)" },
                        "include_semantic": { "type": "boolean", "default": false, "description": "是否包含 L3_SEMANTIC 隐式语义边" }
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
            }),
            serde_json::json!({
                "name": "steward_hook_generate",
                "description": "Agent 客户端 Hook 自动生成器。为 Antigravity (agy)、Cursor、Claude Code 或 Git 生成自动化 Pre/Post Hook 集成配置与执行规则脚本（规约 #13 MCP 与 CLI 1:1 映射）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "client": {
                            "type": "string",
                            "description": "目标客户端类型 (可选: agy, cursor, claude, git)"
                        },
                        "out_dir": {
                            "type": "string",
                            "description": "可选写入的目标基准目录。若不填则默认仅返回生成内容"
                        },
                        "dry_run": {
                            "type": "boolean",
                            "default": false,
                            "description": "是否仅预览生成内容而不真正写入磁盘"
                        }
                    },
                    "required": ["client"]
                }
            }),
            serde_json::json!({
                "name": "steward_analytics_heatmap",
                "description": "基于 DuckDB 高性能列存分析引擎，按目录与时间分桶统计近期的变更热力图分布。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "days": {
                            "type": "integer",
                            "default": 30,
                            "description": "回溯天数"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_analytics_trend",
                "description": "基于 DuckDB 分析引擎的资产增长与记忆演变趋势统计。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "metric": {
                            "type": "string",
                            "enum": ["assets", "memories", "changes"],
                            "default": "assets",
                            "description": "分析指标类型"
                        },
                        "days": {
                            "type": "integer",
                            "default": 30,
                            "description": "分析时间跨度（天数）"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_analytics_export",
                "description": "基于 DuckDB 原生列式引擎将指定的管家数据表秒级导出为 Parquet 格式，供离线深度分析或数据科学工具消费。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "table": {
                            "type": "string",
                            "enum": ["assets", "changes", "memories", "fact_nodes", "fact_edges", "links", "active_processes"],
                            "description": "待导出的数据表名称"
                        },
                        "output_path": {
                            "type": "string",
                            "description": "输出的目标 Parquet 文件绝对路径（例如 '/root/new/tmp/assets.parquet'）"
                        }
                    },
                    "required": ["table", "output_path"]
                }
            }),
            serde_json::json!({
                "name": "steward_lock_acquire",
                "description": "多 Agent 协作锁：原子申请资源锁（端口/目录/服务/文件/自定义），彻底杜绝端口冲突与文件覆盖竞态。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "resource_type": {
                            "type": "string",
                            "enum": ["port", "directory", "service", "file", "custom"],
                            "description": "资源类别"
                        },
                        "resource_id": {
                            "type": "string",
                            "description": "资源标识（例如 '8080'、'/etc/nginx/conf.d/'、'nginx.service'）"
                        },
                        "scope": {
                            "type": "string",
                            "enum": ["exclusive", "shared"],
                            "default": "exclusive",
                            "description": "锁范围：exclusive（排他独占，默认）或 shared（共享只读）"
                        },
                        "holder_agent": {
                            "type": "string",
                            "description": "持有者 Agent 标识（默认为当前 Agent）"
                        },
                        "holder_pid": {
                            "type": "integer",
                            "description": "持有者进程 PID（可选）"
                        },
                        "ttl_seconds": {
                            "type": "integer",
                            "default": 300,
                            "description": "锁 TTL 有效期（秒，默认 300s，最大 3600s）"
                        },
                        "metadata": {
                            "type": "object",
                            "description": "锁附加元数据（例如任务目的、关联项目）"
                        }
                    },
                    "required": ["resource_type", "resource_id"]
                }
            }),
            serde_json::json!({
                "name": "steward_lock_release",
                "description": "多 Agent 协作锁：主动释放已持有的资源锁。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "lock_id": {
                            "type": "string",
                            "description": "申请锁时返回的 lock_id UUID"
                        },
                        "holder_agent": {
                            "type": "string",
                            "description": "持有者 Agent 标识（用于防越权释放，默认为当前 Agent）"
                        }
                    },
                    "required": ["lock_id"]
                }
            }),
            serde_json::json!({
                "name": "steward_lock_list",
                "description": "多 Agent 协作锁：查看宿主机当前所有生效中的活跃资源锁（谁持有、何资源、何时到期）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "resource_type": {
                            "type": "string",
                            "enum": ["port", "directory", "service", "file", "custom"],
                            "description": "可选按资源类型过滤"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_mcp_catalog",
                "description": "管理与路由宿主机客户端（Hermes/AGY/Claude/Cursor）已配置的 MCP Server 目录，支持扫描脱敏入库、列清单与按意图检索，防止上下文膨胀。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["scan", "list", "route"], "default": "route" },
                        "query": { "type": "string", "description": "意图或关键词" },
                        "limit": { "type": "integer", "default": 5 }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_skill_route",
                "description": "按任务意图与 Token 预算对宿主机百级 SKILL 目录进行渐进式按需路由，只返回最匹配的 Top-K SKILL 路径与摘要，避免全量注入打爆 Context。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["scan", "list", "route"], "default": "route" },
                        "intent": { "type": "string", "description": "任务意图或关键词" },
                        "budget_tokens": { "type": "integer", "default": 600 },
                        "max_skills": { "type": "integer", "default": 3 }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_service_contact",
                "description": "管理宿主机外部可用服务通讯录（Endpoint、能力标签、鉴权环境变量名引用、调用示例与轻量探活），绝不存储明文密钥。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["register", "list", "query", "check", "bootstrap"], "default": "list" },
                        "service_name": { "type": "string" },
                        "category": { "type": "string" },
                        "endpoint": { "type": "string" },
                        "protocol": { "type": "string", "default": "https" },
                        "capabilities": { "type": "string" },
                        "auth_type": { "type": "string", "default": "none" },
                        "auth_env_var": { "type": "string" },
                        "health_endpoint": { "type": "string" },
                        "usage_example": { "type": "string" },
                        "query": { "type": "string" }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_lock_wait",
                "description": "多 Agent 协作锁排队等待与唤醒获取（Phase 17）：若资源正被占用则入队等待释放通知，并在入队前自动执行 WFG 死锁环路检测，避免循环死等。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "resource_type": { "type": "string", "enum": ["port", "directory", "service", "file", "custom"] },
                        "resource_id": { "type": "string" },
                        "scope": { "type": "string", "enum": ["exclusive", "shared"], "default": "exclusive" },
                        "holder_agent": { "type": "string", "default": "current-agent" },
                        "ttl_seconds": { "type": "integer", "default": 300 },
                        "wait_timeout_ms": { "type": "integer", "default": 5000 },
                        "priority": { "type": "integer", "default": 100 }
                    },
                    "required": ["resource_type", "resource_id"]
                }
            }),
            serde_json::json!({
                "name": "steward_lock_deadlock_check",
                "description": "多 Agent 协作锁 Wait-For Graph (WFG) 死锁环路诊断：实时检测活跃锁与等待队列间是否存在循环依赖（含目录祖先后代传递），给出退让建议。",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "steward_mesh_query",
                "description": "轻量多机联邦 Peer Mesh Wire Protocol v1.0（Phase 18 -> Phase 41）：支持 ping/facts/locks/port/acquire/local、跨机图谱联邦探查(graph)、HMAC签名双向脱敏结构化对话(dialogue_*)与 <=10MB 沙盒文件投递(transfer_*)。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["peers", "ping", "facts", "locks", "port", "acquire", "local", "graph", "dialogue_send", "dialogue_inbox", "dialogue_ack", "transfer_send", "transfer_inbox", "transfer_accept", "transfer_reject"], "default": "ping" },
                        "port": { "type": "integer", "description": "action=port 时要查询的端口" },
                        "peer": { "type": "string", "description": "目标对等节点基地址" },
                        "query": { "type": "string", "description": "action=graph 时的图谱实体查询词" },
                        "hops": { "type": "integer", "default": 2, "description": "action=graph 时的最大跳数" },
                        "thread_id": { "type": "string", "description": "对话线程 ID" },
                        "intent": { "type": "string", "enum": ["ASK_CONTEXT", "HANDOFF_TASK", "SHARE_MEMORY", "REPLY"], "default": "ASK_CONTEXT" },
                        "content": { "type": "string", "description": "对话文本内容" },
                        "to_agent": { "type": "string", "default": "" },
                        "dialogue_id": { "type": "string", "description": "action=dialogue_ack 时的对话 ID" },
                        "file_path": { "type": "string", "description": "action=transfer_send 时的本地文件路径" },
                        "transfer_id": { "type": "string", "description": "action=transfer_accept/reject 时的传输 ID" },
                        "dest_dir": { "type": "string", "description": "action=transfer_accept 时的落盘目标目录" },
                        "resource_type": { "type": "string", "enum": ["port", "directory", "service", "file", "custom"] },
                        "resource_id": { "type": "string" },
                        "scope": { "type": "string", "enum": ["exclusive", "shared"], "default": "exclusive" },
                        "holder_agent": { "type": "string", "default": "current-agent" },
                        "ttl_seconds": { "type": "integer", "default": 300 }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_demo_scenario",
                "description": "Web 路演与黑屏同源场景演示引擎（Phase 22）：列出或实跑 6 大黄金路演剧目（防扫爆/Pre-Hook防撞/爆炸半径/WFG死锁/Context防爆/完工简报），核算商业价值 ROI，检查黑白屏同步矩阵，或一键重置 demo:* 沙盒。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["list", "run", "roi", "parity", "reset"], "default": "list" },
                        "scenario_id": { "type": "string", "description": "剧目 ID (scene-1 ~ scene-6，action=run 时必填)" }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_benchmark",
                "description": "实证 Benchmark 对照评测与生产效能存证账本引擎（v3.3）：运行 A/B 对照负载(W1~W12)、内核微基准(M1~M5)、数据质量与新鲜度(DQ1~DQ5)、热门竞品横向对标(COMP-C1~C4)或审计生产效能账本(PR-01~PR-10)。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["workloads", "run", "report", "ledger", "compare", "export", "reset", "competitors"], "default": "report" },
                        "suite": { "type": "string", "default": "full", "description": "full | macro | micro | dq | competitors | W1~W12 | COMP-C1~C4" },
                        "track": { "type": "string", "default": "all", "description": "竞品对标赛道: all | C1 | C2 | C3 | C4" },
                        "save": { "type": "boolean", "default": true },
                        "run_id": { "type": "string" },
                        "run_a": { "type": "string" },
                        "run_b": { "type": "string" },
                        "window": { "type": "string", "enum": ["24h", "7d", "30d", "all"], "default": "7d" },
                        "tool_filter": { "type": "string" },
                        "format": { "type": "string", "enum": ["json", "markdown", "csv"], "default": "json" },
                        "limit": { "type": "integer", "default": 20 }
                    }
                }
            }),
            serde_json::json!({
                "name": "steward_feedback",
                "description": "实际使用反馈总账 (user_feedback.md) 双向查阅、结构化追加、已验证反馈沉淀记忆，以及 Phase 38~39 信息新鲜度对账 (reconcile) 与五点执行回路对比探针 (probes)",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["list", "add", "sync", "reconcile", "probes"], "default": "list" },
                        "category": { "type": "string", "enum": ["PRO", "CON", "BENCH", "REQ"], "default": "CON" },
                        "priority": { "type": "string", "enum": ["P0", "P1", "P2"], "default": "P1" },
                        "title": { "type": "string" },
                        "evidence": { "type": "string" },
                        "target": { "type": "string" },
                        "auto_heal": { "type": "boolean", "default": true, "description": "action=probes 时是否自动执行 FreshnessReconciler 自愈" },
                        "auto_feedback": { "type": "boolean", "default": false, "description": "action=probes 时是否将越阈值异常自动去重写入 user_feedback.md" }
                    }
                }
            })
        ]
    }

    /// 执行具体工具调用（带生产效能存证自动记账）
    pub fn call_tool(&self, name: &str, args: Value) -> Result<String> {
        let t0 = std::time::Instant::now();
        let res = self.call_tool_inner(name, args);
        if let Ok(ref out) = res {
            crate::benchmark::ledger::EffectivenessLedger::record_mcp_call(
                self.storage.provider().as_ref(),
                name,
                out,
                t0.elapsed().as_micros() as i64,
            );
        }
        res
    }

    fn call_tool_inner(&self, name: &str, args: Value) -> Result<String> {
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
                let detail = crate::scanner::DetailLevel::from_str_opt(
                    args.get("detail_level").and_then(|v| v.as_str()),
                );

                let results = self.storage.search_assets_fts(query, limit)?;
                let mut out = format!("资产搜索匹配结果 (共 {} 条, detail={:?}):\n", results.len(), detail);
                for a in results {
                    out.push_str(&crate::scanner::format_asset_line(&a, detail));
                    out.push('\n');
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
                let enrich = args.get("enrich_with_ollama").and_then(|v| v.as_bool()).unwrap_or(false);
                let enrich_limit = args.get("enrich_limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;

                let scanner = Scanner::new(self.storage.clone())?;
                let report = scanner.scan_path(&PathBuf::from(path_str))?;

                let mut out = format!("分级颗粒度扫描报告 ({:?}):\n", path_str);
                out.push_str(&format!("  - 扫描目录数:       {}\n", report.scanned_dirs));
                out.push_str(&format!("  - 深层索引文件数:   {} (已完成 L1 确定性元数据提取)\n", report.deep_indexed_files));
                out.push_str(&format!("  - 黑名单阻断目录数: {}\n", report.shallow_blocked_dirs.len()));
                for b in report.shallow_blocked_dirs {
                    out.push_str(&format!("    ★ [Shallow 阻断] {} ({})\n", b.path, b.reason));
                }
                if enrich {
                    let annotator = crate::scanner::OllamaAnnotator::with_default_config(self.storage.clone());
                    let er = annotator.enrich_pending_assets(enrich_limit, Some(path_str))?;
                    out.push_str(&format!(
                        "  - Ollama L2 精炼:   模型={} | 在线={} | 成功={}/{} (耗时 {}ms)\n",
                        er.model_used, er.ollama_available, er.enriched_count, er.total_candidates, er.elapsed_ms
                    ));
                    for s in er.samples {
                        out.push_str(&format!("    🤖 [{}] {} -> {} (入口: {})\n", s.domain, s.path, s.purpose, s.entry_point));
                    }
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
                let engine = LinksEngine::new(arc_storage.clone());
                let node_type = args.get("type").and_then(|v| v.as_str()).unwrap_or("asset");
                let node_id = args.get("id").and_then(|v| v.as_str()).unwrap_or("");
                let max_hops = args.get("max_hops").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
                let min_weight = args.get("min_weight").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                let include_semantic = args.get("include_semantic").and_then(|v| v.as_bool()).unwrap_or(true);

                let links = engine.query_links(node_type, node_id)?;
                let mut out = format!("文件级实体关联拓扑 (共 {} 条):\n", links.len());
                for l in links {
                    out.push_str(&format!("  - [{}] {} --({})--> [{}] {} (置信度: {:.2})\n",
                        l.source_type, l.source_id, l.relation, l.target_type, l.target_id, l.confidence));
                }

                let tiers: Vec<&str> = if include_semantic {
                    vec!["L1_EXPLICIT", "L2_STRUCTURAL", "L3_SEMANTIC"]
                } else {
                    vec!["L1_EXPLICIT", "L2_STRUCTURAL"]
                };
                let root_cand = if node_id.contains(':') {
                    node_id.to_string()
                } else {
                    format!("{}:{}", node_type, node_id)
                };
                if let Ok(sub) = arc_storage.traverse_fact_graph_filtered(&root_cand, max_hops, min_weight, Some(&tiers)) {
                    if !sub.edges.is_empty() {
                        out.push_str(&format!("\n属性图谱分层关系 ({} 跳, 共 {} 条边):\n", max_hops, sub.edges.len()));
                        for e in &sub.edges {
                            out.push_str(&format!(
                                "  - [{} w={:.2} {}] {} --({})--> {}\n",
                                e.evidence_tier, e.weight, e.direction_kind, e.source_id, e.relation, e.target_id
                            ));
                        }
                    }
                }
                Ok(out)
            }

            "steward_links_visualize" => {
                let arc_storage = Arc::new(self.storage.clone());
                let discover = args.get("discover_semantic").and_then(|v| v.as_bool()).unwrap_or(false);
                let mut header = String::new();
                if discover {
                    let miner = crate::fact::GraphRelationshipMiner::new(arc_storage.as_ref());
                    let rep = miner.mine_all(true)?;
                    header = format!(
                        "%% 关系挖掘完成: L2上卷={}, 服务桥接={}, 工具={}, 衍生={}, L3语义={}\n",
                        rep.l2_rollup_edges, rep.l2_service_bridge_edges, rep.l2_tool_usage_edges, rep.l2_derivation_edges, rep.l3_semantic_edges
                    );
                }
                let engine = LinksEngine::new(arc_storage);
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                let mermaid = engine.export_mermaid(limit)?;
                Ok(format!("{}{}", header, mermaid))
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
                let min_weight = args.get("min_weight").and_then(|v| v.as_f64()).unwrap_or(0.50) as f32;
                let include_semantic = args.get("include_semantic").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = analyzer.analyze_impact_weighted(target, max_depth, min_weight, include_semantic)?;
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
                let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);
                let res = sm.close_session_with_dry_run(session_id, clean_temp, dry_run)?;
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

            "steward_hook_generate" => {
                let client_str = args.get("client").and_then(|v| v.as_str()).unwrap_or("agy");
                let client_type = client_str.parse::<crate::hooks::ClientType>()?;
                let out_dir = args.get("out_dir").and_then(|v| v.as_str()).map(std::path::Path::new);
                let dry_run = args.get("dry_run").and_then(|v| v.as_bool()).unwrap_or(false);

                let res = crate::hooks::ClientHookGenerator::generate(client_type, out_dir, dry_run)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "steward_analytics_heatmap" => {
                use crate::storage::AnalyticsProvider;
                let days = args.get("days").and_then(|v| v.as_u64()).unwrap_or(30) as usize;
                let engine = crate::storage::DuckDbAnalyticsEngine::from_synced_storage(self.storage.provider().clone())?;
                let heatmap = engine.changes_heatmap(days)?;
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "time_span_days": days,
                    "heatmap": heatmap,
                }))?)
            }

            "steward_analytics_trend" => {
                use crate::storage::AnalyticsProvider;
                let days = args.get("days").and_then(|v| v.as_u64()).unwrap_or(30) as usize;
                let engine = crate::storage::DuckDbAnalyticsEngine::from_synced_storage(self.storage.provider().clone())?;
                let trend = engine.asset_growth_trend(days)?;
                let decay = engine.memory_decay_curve()?;
                let stats = engine.fact_graph_stats()?;
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "time_span_days": days,
                    "asset_growth_trend": trend,
                    "memory_decay": decay,
                    "graph_stats": stats,
                }))?)
            }

            "steward_analytics_export" => {
                use crate::storage::AnalyticsProvider;
                let table = args.get("table").and_then(|v| v.as_str()).unwrap_or("assets");
                let output_path = args.get("output_path").and_then(|v| v.as_str()).unwrap_or("/root/new/tmp/export.parquet");
                let engine = crate::storage::DuckDbAnalyticsEngine::from_synced_storage(self.storage.provider().clone())?;
                let summary = engine.export_to_parquet(table, output_path)?;
                Ok(serde_json::to_string_pretty(&summary)?)
            }

            "steward_lock_acquire" => {
                use crate::coordination::{CoordinationEngine, LockRequest, ResourceType, LockScope};
                let rt_str = args.get("resource_type").and_then(|v| v.as_str()).unwrap_or("custom");
                let res_id = args.get("resource_id").and_then(|v| v.as_str()).unwrap_or("");
                let scope_str = args.get("scope").and_then(|v| v.as_str()).unwrap_or("exclusive");
                let holder = args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("current-agent");
                let pid = args.get("holder_pid").and_then(|v| v.as_u64()).map(|p| p as u32);
                let ttl = args.get("ttl_seconds").and_then(|v| v.as_u64()).unwrap_or(300);
                let meta = args.get("metadata").cloned();

                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let req = LockRequest {
                    resource_type: ResourceType::from_str_loose(rt_str),
                    resource_id: res_id.to_string(),
                    scope: LockScope::from_str_loose(scope_str),
                    holder_agent: holder.to_string(),
                    holder_pid: pid,
                    ttl_seconds: ttl,
                    metadata: meta,
                };
                let res = engine.try_acquire(&req)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "steward_lock_release" => {
                use crate::coordination::CoordinationEngine;
                let lock_id = args.get("lock_id").and_then(|v| v.as_str()).unwrap_or("");
                let holder = args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("current-agent");
                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let released = engine.release(lock_id, holder)?;
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "lock_id": lock_id,
                    "released": released,
                }))?)
            }

            "steward_lock_list" => {
                use crate::coordination::{CoordinationEngine, ResourceType};
                let rt_opt = args.get("resource_type").and_then(|v| v.as_str()).map(ResourceType::from_str_loose);
                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let list = engine.list_active_locks(rt_opt.as_ref())?;
                Ok(serde_json::to_string_pretty(&list)?)
            }

            "steward_mcp_catalog" => {
                use crate::ecosystem::McpConfigScanner;
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("route");
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
                let scanner = McpConfigScanner::new(self.storage.clone());
                match action {
                    "scan" => {
                        let (cnt, sanitized) = scanner.scan_default_clients()?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "mcp_servers_discovered": cnt,
                            "sanitized_env_keys_count": sanitized
                        }))?)
                    }
                    "list" => {
                        let list = self.storage.list_mcp_catalog()?;
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                    _ => {
                        let list = if query.trim().is_empty() {
                            self.storage.list_mcp_catalog()?.into_iter().take(limit).collect()
                        } else {
                            self.storage.search_mcp_catalog(query, limit)?
                        };
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                }
            }

            "steward_skill_route" => {
                use crate::ecosystem::{ContextBudgetRouter, ContextRouteRequest, SkillCatalogScanner};
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("route");
                let intent = args.get("intent").and_then(|v| v.as_str()).unwrap_or("");
                let budget_tokens = args.get("budget_tokens").and_then(|v| v.as_u64()).unwrap_or(600) as usize;
                let max_skills = args.get("max_skills").and_then(|v| v.as_u64()).unwrap_or(3) as usize;
                match action {
                    "scan" => {
                        let scanner = SkillCatalogScanner::new(self.storage.clone());
                        let (cnt, tokens) = scanner.scan_default_roots()?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "skills_discovered": cnt,
                            "total_full_tokens_estimated": tokens
                        }))?)
                    }
                    "list" => {
                        let list = self.storage.list_skills_catalog()?;
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                    _ => {
                        let router = ContextBudgetRouter::new(self.storage.clone());
                        let plan = router.route(&ContextRouteRequest {
                            intent: intent.to_string(),
                            budget_tokens,
                            max_skills,
                            max_mcps: 4,
                            max_services: 3,
                        })?;
                        Ok(serde_json::to_string_pretty(&plan)?)
                    }
                }
            }

            "steward_service_contact" => {
                use crate::ecosystem::ExternalContactsManager;
                use crate::storage::models::ExternalServiceRecord;
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
                let mgr = ExternalContactsManager::new(self.storage.clone());
                match action {
                    "register" => {
                        let service_name = args.get("service_name").and_then(|v| v.as_str()).unwrap_or("unnamed-service");
                        let rec = ExternalServiceRecord {
                            id: format!("svc:{}", service_name),
                            service_name: service_name.to_string(),
                            category: args.get("category").and_then(|v| v.as_str()).unwrap_or("api").to_string(),
                            endpoint: args.get("endpoint").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            protocol: args.get("protocol").and_then(|v| v.as_str()).unwrap_or("https").to_string(),
                            capabilities: args.get("capabilities").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            auth_type: args.get("auth_type").and_then(|v| v.as_str()).unwrap_or("none").to_string(),
                            auth_env_var: args.get("auth_env_var").and_then(|v| v.as_str()).map(String::from),
                            health_endpoint: args.get("health_endpoint").and_then(|v| v.as_str()).map(String::from),
                            usage_example: args.get("usage_example").and_then(|v| v.as_str()).map(String::from),
                            status: "unknown".to_string(),
                            last_checked_at: None,
                        };
                        mgr.register_service(&rec)?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "registered": true,
                            "service_name": rec.service_name,
                            "auth_env_ready": ExternalContactsManager::is_auth_env_ready(rec.auth_env_var.as_deref())
                        }))?)
                    }
                    "check" => {
                        let name_arg = args.get("service_name").and_then(|v| v.as_str()).unwrap_or("");
                        let rep = mgr.check_health(name_arg)?;
                        Ok(serde_json::to_string_pretty(&rep)?)
                    }
                    "query" => {
                        let q = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                        let list = self.storage.search_external_services(q, 10)?;
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                    "bootstrap" => {
                        let cfg = crate::config::Config::load_or_default(None).unwrap_or_default();
                        let count = mgr.bootstrap_from_config(&cfg)?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({ "bootstrapped": count }))?)
                    }
                    _ => {
                        let cat = args.get("category").and_then(|v| v.as_str());
                        let list = self.storage.list_external_services(cat)?;
                        Ok(serde_json::to_string_pretty(&list)?)
                    }
                }
            }

            "steward_lock_wait" => {
                use crate::coordination::{CoordinationEngine, LockRequest, LockScope, ResourceType};
                let rt_str = args.get("resource_type").and_then(|v| v.as_str()).unwrap_or("custom");
                let res_id = args.get("resource_id").and_then(|v| v.as_str()).unwrap_or("");
                let scope_str = args.get("scope").and_then(|v| v.as_str()).unwrap_or("exclusive");
                let holder = args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("current-agent");
                let ttl = args.get("ttl_seconds").and_then(|v| v.as_u64()).unwrap_or(300);
                let wait_ms = args.get("wait_timeout_ms").and_then(|v| v.as_u64()).unwrap_or(5000);
                let priority = args.get("priority").and_then(|v| v.as_i64()).unwrap_or(100) as i32;

                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let req = LockRequest {
                    resource_type: ResourceType::from_str_loose(rt_str),
                    resource_id: res_id.to_string(),
                    scope: LockScope::from_str_loose(scope_str),
                    holder_agent: holder.to_string(),
                    holder_pid: None,
                    ttl_seconds: ttl,
                    metadata: None,
                };
                let res = engine.wait_and_acquire(&req, wait_ms, priority)?;
                Ok(serde_json::to_string_pretty(&res)?)
            }

            "steward_lock_deadlock_check" => {
                use crate::coordination::CoordinationEngine;
                let engine = CoordinationEngine::new(self.storage.provider().clone());
                let rep = engine.detect_deadlocks()?;
                let waits = engine.list_wait_queue(None)?;
                Ok(serde_json::to_string_pretty(&serde_json::json!({
                    "deadlock_report": rep,
                    "wait_queue_count": waits.len(),
                    "wait_queue": waits,
                }))?)
            }

            "steward_mesh_query" => {
                use crate::coordination::{LockRequest, LockScope, ResourceType};
                use crate::mesh::{build_local_facts, MeshClient};
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("ping");
                let cfg = crate::config::Config::load_or_default(None).unwrap_or_default();
                let client = MeshClient::new(&cfg.mesh, cfg.web.auth_token.clone());
                match action {
                    "local" => Ok(serde_json::to_string_pretty(&build_local_facts())?),
                    "peers" => Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "enabled": cfg.mesh.enabled,
                        "peers": client.peers(),
                        "timeout_ms": cfg.mesh.timeout_ms,
                    }))?),
                    "facts" => Ok(serde_json::to_string_pretty(&client.facts_all())?),
                    "locks" => Ok(serde_json::to_string_pretty(&client.locks_all())?),
                    "port" => {
                        let port = args.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16;
                        Ok(serde_json::to_string_pretty(&client.federated_port_check(port))?)
                    }
                    "acquire" => {
                        let peer = args.get("peer").and_then(|v| v.as_str()).unwrap_or("");
                        if peer.is_empty() {
                            bail!("action=acquire 必须提供 peer 参数");
                        }
                        let req = LockRequest {
                            resource_type: ResourceType::from_str_loose(args.get("resource_type").and_then(|v| v.as_str()).unwrap_or("custom")),
                            resource_id: args.get("resource_id").and_then(|v| v.as_str()).unwrap_or("").to_string(),
                            scope: LockScope::from_str_loose(args.get("scope").and_then(|v| v.as_str()).unwrap_or("exclusive")),
                            holder_agent: args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("current-agent").to_string(),
                            holder_pid: None,
                            ttl_seconds: args.get("ttl_seconds").and_then(|v| v.as_u64()).unwrap_or(300),
                            metadata: None,
                        };
                        Ok(serde_json::to_string_pretty(&client.acquire_on_peer(peer, &req)?)?)
                    }
                    "graph" => {
                        let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                        let hops = args.get("hops").and_then(|v| v.as_u64()).unwrap_or(2) as usize;
                        Ok(serde_json::to_string_pretty(&client.federated_graph_query(query, hops))?)
                    }
                    "dialogue_send" => {
                        let peer = args.get("peer").and_then(|v| v.as_str()).unwrap_or("");
                        if peer.is_empty() {
                            bail!("action=dialogue_send 必须提供 peer 参数");
                        }
                        let thread_id = args.get("thread_id").and_then(|v| v.as_str()).unwrap_or("");
                        let intent = args.get("intent").and_then(|v| v.as_str()).unwrap_or("ASK_CONTEXT");
                        let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        let to_agent = args.get("to_agent").and_then(|v| v.as_str()).unwrap_or("");
                        let from_agent = args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("mcp-agent");
                        Ok(serde_json::to_string_pretty(&client.send_dialogue(
                            peer, thread_id, intent, content, from_agent, to_agent,
                        )?)?)
                    }
                    "dialogue_inbox" => {
                        let rows = self.storage.list_mesh_dialogues(30)?;
                        Ok(serde_json::to_string_pretty(&rows)?)
                    }
                    "dialogue_ack" => {
                        let did = args.get("dialogue_id").and_then(|v| v.as_str()).unwrap_or("");
                        let updated = self.storage.update_mesh_dialogue_status(did, "acked")?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({ "dialogue_id": did, "acked": updated }))?)
                    }
                    "transfer_send" => {
                        let peer = args.get("peer").and_then(|v| v.as_str()).unwrap_or("");
                        let file_path = args.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
                        if peer.is_empty() || file_path.is_empty() {
                            bail!("action=transfer_send 必须提供 peer 与 file_path 参数");
                        }
                        let from_agent = args.get("holder_agent").and_then(|v| v.as_str()).unwrap_or("mcp-agent");
                        Ok(serde_json::to_string_pretty(&client.send_transfer(
                            peer, std::path::Path::new(file_path), from_agent,
                        )?)?)
                    }
                    "transfer_inbox" => {
                        let rows = self.storage.list_mesh_transfers(30)?;
                        Ok(serde_json::to_string_pretty(&rows)?)
                    }
                    "transfer_accept" => {
                        let tid = args.get("transfer_id").and_then(|v| v.as_str()).unwrap_or("");
                        let dest_dir = args.get("dest_dir").and_then(|v| v.as_str()).unwrap_or("/root/new/tmp/mesh_accepted");
                        let rec = crate::mesh::transfer::accept_transfer_to_dir(
                            self.storage.provider().as_ref(),
                            tid,
                            std::path::Path::new(dest_dir),
                        )?;
                        Ok(serde_json::to_string_pretty(&rec)?)
                    }
                    "transfer_reject" => {
                        let tid = args.get("transfer_id").and_then(|v| v.as_str()).unwrap_or("");
                        let ok = crate::mesh::transfer::reject_transfer(self.storage.provider().as_ref(), tid)?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({ "transfer_id": tid, "rejected": ok }))?)
                    }
                    _ => Ok(serde_json::to_string_pretty(&client.ping_all())?),
                }
            }

            "steward_demo_scenario" => {
                use crate::demo::DemoEngine;
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
                match action {
                    "run" => {
                        let id = args.get("scenario_id").and_then(|v| v.as_str()).unwrap_or("scene-1");
                        let res = DemoEngine::run_scenario(self.storage.provider().clone(), id)?;
                        Ok(serde_json::to_string_pretty(&res)?)
                    }
                    "roi" => {
                        let roi = DemoEngine::compute_roi(self.storage.provider().as_ref())?;
                        Ok(serde_json::to_string_pretty(&roi)?)
                    }
                    "reset" => {
                        let cleaned = DemoEngine::reset_sandbox(self.storage.provider().as_ref())?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({ "cleaned": cleaned }))?)
                    }
                    "parity" => {
                        let tools = self.list_tools();
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "total_mcp_tools": tools.len(),
                            "tools": tools.iter().filter_map(|t| t.get("name").and_then(|v| v.as_str())).collect::<Vec<_>>(),
                        }))?)
                    }
                    _ => Ok(serde_json::to_string_pretty(&DemoEngine::list_scenarios())?),
                }
            }

            "steward_benchmark" => {
                use crate::benchmark::ledger::EffectivenessLedger;
                use crate::benchmark::provenance::ProvenanceRule;
                use crate::benchmark::report::{
                    render_csv_report, render_markdown_report, BenchmarkReportPayload,
                };
                use crate::benchmark::workloads::list_workload_specs;
                use crate::benchmark::BenchmarkEngine;

                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("report");
                match action {
                    "workloads" => Ok(serde_json::to_string_pretty(&serde_json::json!({
                        "workloads": list_workload_specs(),
                        "provenance_rules": ProvenanceRule::list_all(),
                    }))?),
                    "run" => {
                        let suite = args.get("suite").and_then(|v| v.as_str()).unwrap_or("full");
                        let save = args.get("save").and_then(|v| v.as_bool()).unwrap_or(true);
                        let report = BenchmarkEngine::run_suite(
                            self.storage.provider().clone(),
                            suite,
                            save,
                            None,
                        )?;
                        Ok(serde_json::to_string_pretty(&report)?)
                    }
                    "report" => {
                        let run_id = args.get("run_id").and_then(|v| v.as_str());
                        let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("json");
                        let rec_opt = if let Some(id) = run_id {
                            self.storage.get_benchmark_run(id)?
                        } else {
                            self.storage.list_benchmark_runs(1)?.into_iter().next()
                        };
                        let Some(rec) = rec_opt else {
                            return Ok(serde_json::to_string_pretty(&serde_json::json!({
                                "status": "empty",
                                "message": "尚无基准评测记录，请先执行 steward_benchmark(action='run')"
                            }))?);
                        };
                        let payload = BenchmarkReportPayload::from_record(&rec);
                        match format {
                            "markdown" => render_markdown_report(&payload),
                            "csv" => render_csv_report(&payload),
                            _ => Ok(serde_json::to_string_pretty(&payload)?),
                        }
                    }
                    "ledger" => {
                        let window = args.get("window").and_then(|v| v.as_str()).unwrap_or("7d");
                        let tool_filter = args.get("tool_filter").and_then(|v| v.as_str());
                        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
                        let summary = self.storage.query_effectiveness_ledger(window, tool_filter, limit)?;
                        Ok(serde_json::to_string_pretty(&summary)?)
                    }
                    "compare" => {
                        let a_id = args.get("run_a").and_then(|v| v.as_str()).unwrap_or("");
                        let b_id = args.get("run_b").and_then(|v| v.as_str()).unwrap_or("");
                        let Some(a) = self.storage.get_benchmark_run(a_id)? else {
                            bail!("未找到基准批次 run_a: {}", a_id);
                        };
                        let Some(b) = self.storage.get_benchmark_run(b_id)? else {
                            bail!("未找到基准批次 run_b: {}", b_id);
                        };
                        let diff = EffectivenessLedger::compare_runs(&a, &b);
                        Ok(serde_json::to_string_pretty(&diff)?)
                    }
                    "export" => {
                        let format = args.get("format").and_then(|v| v.as_str()).unwrap_or("markdown");
                        let rec_opt = self.storage.list_benchmark_runs(1)?.into_iter().next();
                        let Some(rec) = rec_opt else {
                            bail!("尚无基准评测记录可导出");
                        };
                        let payload = BenchmarkReportPayload::from_record(&rec);
                        match format {
                            "csv" => render_csv_report(&payload),
                            "json" => Ok(serde_json::to_string_pretty(&payload)?),
                            _ => render_markdown_report(&payload),
                        }
                    }
                    "reset" => {
                        let cleaned = BenchmarkEngine::reset_sandbox(self.storage.provider().as_ref())?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({ "cleaned": cleaned }))?)
                    }
                    "competitors" => {
                        let track = args.get("track").and_then(|v| v.as_str()).unwrap_or("all");
                        let results = crate::benchmark::competitors::run_competitor_track(
                            self.storage.provider().clone(),
                            track,
                        )?;
                        let matrix = crate::benchmark::competitors::build_competitor_capability_matrix();
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "spec_version": crate::benchmark::BENCHMARK_SPEC_VERSION,
                            "results": results,
                            "capability_matrix": matrix,
                        }))?)
                    }
                    other => bail!("未知 benchmark action: {}", other),
                }
            }

            "steward_feedback" => {
                let action = args.get("action").and_then(|v| v.as_str()).unwrap_or("list");
                let fb_path = crate::feedback::FeedbackEngine::default_feedback_path();
                match action {
                    "list" => {
                        let items = crate::feedback::FeedbackEngine::list_items(&fb_path)?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "path": fb_path.to_string_lossy(),
                            "count": items.len(),
                            "items": items
                        }))?)
                    }
                    "add" => {
                        let category = args.get("category").and_then(|v| v.as_str()).unwrap_or("CON");
                        let priority = args.get("priority").and_then(|v| v.as_str()).unwrap_or("P1");
                        let title = args.get("title").and_then(|v| v.as_str()).unwrap_or("");
                        let evidence = args.get("evidence").and_then(|v| v.as_str()).unwrap_or("");
                        let target = args.get("target").and_then(|v| v.as_str()).unwrap_or("");
                        let item = crate::feedback::FeedbackEngine::add_item(
                            &fb_path, category, priority, title, evidence, target,
                        )?;
                        Ok(serde_json::to_string_pretty(&item)?)
                    }
                    "sync" => {
                        let added = crate::feedback::FeedbackEngine::sync_verified_to_memories(
                            &fb_path,
                            &self.storage,
                        )?;
                        Ok(serde_json::to_string_pretty(&serde_json::json!({
                            "synced_to_memories": added,
                            "path": fb_path.to_string_lossy()
                        }))?)
                    }
                    "reconcile" => {
                        let rep = crate::feedback::probes::FreshnessReconciler::reconcile(&self.storage)?;
                        Ok(serde_json::to_string_pretty(&rep)?)
                    }
                    "probes" => {
                        let auto_heal = args.get("auto_heal").and_then(|v| v.as_bool()).unwrap_or(true);
                        let auto_feedback = args.get("auto_feedback").and_then(|v| v.as_bool()).unwrap_or(false);
                        let rep = crate::feedback::probes::SelfEvolutionProbeEngine::run_probes(
                            &self.storage,
                            &fb_path,
                            auto_heal,
                            auto_feedback,
                        )?;
                        Ok(serde_json::to_string_pretty(&rep)?)
                    }
                    other => bail!("未知 feedback action: {}", other),
                }
            }

            _ => bail!("Unknown tool: {}", name),
        }
    }
}
