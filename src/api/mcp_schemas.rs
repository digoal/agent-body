use serde_json::Value;
use crate::config::McpToolProfile;

/// Core 档位（10 个高频核心工具）：将 tools/list Schema 体积压减 ~78%，大幅降低每轮上下文工具税
pub const CORE_TOOL_NAMES: &[&str] = &[
    "body_brief",
    "body_fact_query",
    "body_asset_search",
    "body_placement_suggest",
    "body_port_check",
    "body_links_impact",
    "body_memory_search",
    "body_memory_add",
    "body_hook_pre",
    "body_state",
];

/// Dev 档位追加工具（+8 研发工具，共 18 个）
pub const DEV_EXTRA_TOOL_NAMES: &[&str] = &[
    "body_asset_register",
    "body_asset_scan",
    "body_links_query",
    "body_links_audit",
    "body_links_repair",
    "body_codegraph_bridge",
    "body_hook_post",
    "body_session_close",
];

/// Ops 档位追加工具（+10 运维治理工具，共 20 个）
pub const OPS_EXTRA_TOOL_NAMES: &[&str] = &[
    "body_env_processes",
    "body_env_show",
    "body_env_gc",
    "body_lock_acquire",
    "body_lock_release",
    "body_lock_list",
    "body_changes_since",
    "body_audit_query",
    "body_feedback",
    "body_benchmark",
];

/// 按 MCP Profile 过滤暴露给客户端的工具 Schema（默认 Full 保持全部 43 工具）
pub fn tool_schemas_for_profile(profile: McpToolProfile) -> Vec<Value> {
    let all = all_tool_schemas();
    match profile {
        McpToolProfile::Full => all,
        McpToolProfile::Core => all
            .into_iter()
            .filter(|v| {
                v.get("name")
                    .and_then(|n| n.as_str())
                    .map(|name| CORE_TOOL_NAMES.contains(&name))
                    .unwrap_or(false)
            })
            .collect(),
        McpToolProfile::Dev => all
            .into_iter()
            .filter(|v| {
                v.get("name")
                    .and_then(|n| n.as_str())
                    .map(|name| CORE_TOOL_NAMES.contains(&name) || DEV_EXTRA_TOOL_NAMES.contains(&name))
                    .unwrap_or(false)
            })
            .collect(),
        McpToolProfile::Ops => all
            .into_iter()
            .filter(|v| {
                v.get("name")
                    .and_then(|n| n.as_str())
                    .map(|name| CORE_TOOL_NAMES.contains(&name) || OPS_EXTRA_TOOL_NAMES.contains(&name))
                    .unwrap_or(false)
            })
            .collect(),
    }
}

/// 返回 43 个原生 MCP 工具的完整 JSON Schema 定义（从 mcp_tools.rs 拆分以遵守规约 #28 复杂度红线）
pub fn all_tool_schemas() -> Vec<Value> {
        vec![
            serde_json::json!({
                "name": "body_brief",
                "description": "一键获取 Agent 本体自适应开工简报（宿主机事实感官、规约脊柱、历史记忆海马体与跨大脑接力胶囊一体组装）。瞬态大脑（LLM）在新会话开始或切换任务时必调。",
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
                "name": "body_placement_suggest",
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
                "name": "body_placement_rules",
                "description": "查看当前生效的宿主机目录放置规范规则库（shared_proj, work, skills, src, note, tmp 等）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "body_asset_search",
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
                "name": "body_asset_register",
                "description": "手动向 Agent 本体登记一个文件或目录资产，附带结构化意图描述（desc_kv）、所有者与保密级别。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "文件或目录的绝对路径" },
                        "type": {
                            "type": "string",
                            "enum": ["directory", "project", "script", "document", "dataset", "config", "media", "other"],
                            "description": "资产类型"
                        },
                        "owner": { "type": "string", "description": "归属主体（默认使用 [placement].default_owner 配置）" },
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
                "name": "body_asset_scan",
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
                "name": "body_env_processes",
                "description": "查询宿主机上除系统守护服务外活跃的动态任务与进程（过滤底噪，识别 Python 脚本、构建 Worker、CPU/内存/GPU 占用与命令）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "body_env_show",
                "description": "查看宿主机硬件规格（OS 版本、CPU 核心数、内存总量/使用率、磁盘挂载）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "body_memory_add",
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
                "name": "body_memory_search",
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
                "name": "body_links_query",
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
                "name": "body_links_visualize",
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
                "name": "body_changes_since",
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
                "name": "body_port_check",
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
                "name": "body_links_impact",
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
                "name": "body_links_audit",
                "description": "全局依赖拓扑审计：主动扫描全局依赖图中的断链与悬空引用（目标文件不存在、依赖服务离线），输出健康评分与修复建议。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "fix_dangling": { "type": "boolean", "default": false, "description": "是否自动清理已确认失效的文件依赖死链" }
                    }
                }
            }),
            serde_json::json!({
                "name": "body_codegraph_bridge",
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
                "name": "body_session_close",
                "description": "会话完工结算与环境治理：在任务结束时清理临时测试文件、汇总环境变更、并将工单成果归档沉淀到长期记忆。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "session_id": { "type": "string", "description": "当前会话的唯一标识" },
                        "clean_temp": { "type": "boolean", "default": true, "description": "是否清理配置临时目录 ([workspace].temp_dir) 下属于本会话的临时文件" }
                    },
                    "required": ["session_id"]
                }
            }),
            serde_json::json!({
                "name": "body_links_repair",
                "description": "断链智能修复与重定向推演：基于文件名相似度、代码指纹与 Git 历史寻找已迁移/重命名文件，生成 diff 补丁并可自动修正依赖关系。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "apply": { "type": "boolean", "default": false, "description": "是否直接应用修复修改源文件与关联表" }
                    }
                }
            }),
            serde_json::json!({
                "name": "body_placement_learn",
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
                "name": "body_env_gc",
                "description": "宿主资源垃圾回收与孤儿进程诊断：安全扫描受管临时目录超期孤儿临时文件与后台僵尸测试进程，支持预览与清理。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tmp_dir": { "type": "string", "description": "待清理的临时目录路径 (默认使用 [workspace].temp_dir，强制安全白名单)" },
                        "max_age_hours": { "type": "integer", "default": 72, "description": "文件超期阈值 (小时)" },
                        "apply": { "type": "boolean", "default": false, "description": "是否直接执行清理删除 (默认仅 dry-run 预览)" }
                    }
                }
            }),
            serde_json::json!({
                "name": "body_memory_audit",
                "description": "长期记忆健康体检与矛盾仲裁：检测对立冲突的认知记忆对，计算温和时间衰减淘汰候选，支持指定归档旧条目。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "archive_id": { "type": "string", "description": "若存在冲突，指定归档淘汰的旧记忆 ID" }
                    }
                }
            }),
            serde_json::json!({
                "name": "body_fact_query",
                "description": "宿主机客观事实图谱只读查询（零幻觉真相底座）。查询：1. 机器已装CLI工具与能力推荐(如'有没有处理PDF的东西'、'如何转视频雪碧图')；2. 端口被谁占用与排障(如'8080')；3. 修改某配置会影响哪些systemd服务或cron调度；4. 本地已克隆哪些开源源码(CodeGraph状态)与权威资料清单等。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "自然语言意图或关键词（例如 '处理 PDF 提取文字', '8080', '/root/.agent-body/config.toml', '权威网站'）"
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
                "name": "body_hook_pre",
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
                "name": "body_hook_post",
                "description": "Agent 工作流后置通知与 AOPCC 碰壁秒级自愈钩子。成功时记录事实变更并增量更新拓扑；当 success=false 且传入 error_output 时，在 <2ms 内反查宿主机真实端口/工具/服务状态并注入自愈处方。",
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
                        },
                        "error_output": {
                            "type": "string",
                            "description": "可选失败报错输出（如 Connection refused / command not found / Address already in use）"
                        }
                    },
                    "required": ["tool_name", "args"]
                }
            }),
            serde_json::json!({
                "name": "body_hook_generate",
                "description": "Agent 客户端 Hook 自动生成器。为 Antigravity (agy)、Cursor、Claude Code (含 .claude/settings.json)、Codex (AGENTS.md)、Hermes 或 Git 生成自动化 Pre/Post Hook 集成配置与执行规则脚本（规约 #13 MCP 与 CLI 1:1 映射）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "client": {
                            "type": "string",
                            "description": "目标客户端类型 (可选: agy, cursor, claude, codex, hermes, git)"
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
                "name": "body_analytics_heatmap",
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
                "name": "body_analytics_trend",
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
                "name": "body_analytics_export",
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
                            "description": "输出的目标 Parquet 文件绝对路径（需位于受信任临时目录下，如 '/tmp/assets.parquet'）"
                        }
                    },
                    "required": ["table", "output_path"]
                }
            }),
            serde_json::json!({
                "name": "body_lock_acquire",
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
                "name": "body_lock_release",
                "description": "多 Agent 协作锁：主动释放已持有的资源锁（支持按 lock_id 或 resource_type + resource_id 二选一匹配，可选 force 强制释放）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "lock_id": {
                            "type": "string",
                            "description": "申请锁时返回的 lock_id UUID（与 resource_type + resource_id 二选一）"
                        },
                        "resource_type": {
                            "type": "string",
                            "enum": ["port", "directory", "service", "file", "custom"],
                            "description": "按资源类型匹配释放（未传 lock_id 时必填）"
                        },
                        "resource_id": {
                            "type": "string",
                            "description": "按资源标识匹配释放（未传 lock_id 时必填）"
                        },
                        "holder_agent": {
                            "type": "string",
                            "description": "持有者 Agent 标识（用于防越权释放，默认为当前 Agent）"
                        },
                        "agent_id": {
                            "type": "string",
                            "description": "持有者 Agent 标识别名（兼容 Web UI agent_id 字段）"
                        },
                        "force": {
                            "type": "boolean",
                            "default": false,
                            "description": "是否强制释放（管理员运维免持有者校验）"
                        }
                    }
                }
            }),
            serde_json::json!({
                "name": "body_lock_list",
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
                "name": "body_mcp_catalog",
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
                "name": "body_skill_route",
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
                "name": "body_service_contact",
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
                "name": "body_lock_wait",
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
                "name": "body_lock_deadlock_check",
                "description": "多 Agent 协作锁 Wait-For Graph (WFG) 死锁环路诊断：实时检测活跃锁与等待队列间是否存在循环依赖（含目录祖先后代传递），给出退让建议。",
                "inputSchema": {
                    "type": "object",
                    "properties": {}
                }
            }),
            serde_json::json!({
                "name": "body_mesh_query",
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
                "name": "body_demo_scenario",
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
                "name": "body_benchmark",
                "description": "实证 Benchmark 对照评测与生产效能存证账本引擎（v3.8）：运行 A/B 对照负载(W1~W18)、内核微基准(M1~M6)、数据质量与遥测/图谱健康(DQ1~DQ9)、热门竞品横向对标(COMP-C1~C4)或审计生产效能账本(PR-01~PR-14)。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["workloads", "run", "report", "ledger", "compare", "export", "reset", "competitors"], "default": "report" },
                        "suite": { "type": "string", "default": "full", "description": "full | macro | micro | dq | competitors | W1~W18 | COMP-C1~C4" },
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
                "name": "body_feedback",
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
            }),
            serde_json::json!({
                "name": "body_audit_query",
                "description": "双域全链路行为审计、因果链路回溯、DuckDB OLAP 聚合导出与审计驱动自进化引擎（Phase 43）：覆盖 IN_AGENT（Brief/Hook/MCP/CLI/Web/Mesh）与 OUT_OF_AGENT（Daemon巡检/Watcher 60s折叠/HostGC/LinkRepair/Reconcile/RuleLearn）全行为有据可查。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["query", "summary", "trace", "evolve", "export"], "default": "query" },
                        "domain": { "type": "string", "enum": ["IN_AGENT", "OUT_OF_AGENT"], "description": "审计域过滤" },
                        "actor_kind": { "type": "string" },
                        "category": { "type": "string" },
                        "outcome": { "type": "string", "enum": ["success", "warn", "blocked", "failed", "recovered", "dry_run"] },
                        "trace_id": { "type": "string", "description": "action=trace 时指定因果链路 trace_id" },
                        "session_id": { "type": "string" },
                        "target_contains": { "type": "string" },
                        "window": { "type": "string", "enum": ["24h", "7d", "30d", "all"], "default": "7d" },
                        "days": { "type": "integer", "default": 7 },
                        "limit": { "type": "integer", "default": 30 },
                        "auto_feedback": { "type": "boolean", "default": false, "description": "action=evolve 时是否将高频摩擦热点自动沉淀至 user_feedback.md" },
                        "output_path": { "type": "string", "description": "action=export 时输出路径（仅允许受信任临时目录，如 [workspace].temp_dir 或 /tmp/）" }
                    }
                }
            }),
            serde_json::json!({
                "name": "body_state",
                "description": "Agent 本体中枢（宿主机具身底座，第 43 原生工具）：提供本体身份证与六器官自省 (manifest)、五维稳态生命体征 (vitals)、四级自治权限包络评估 (envelope)、跨大脑工作态接力胶囊保存与恢复 (handoff_save/handoff_resume)、L1 稳态反射自愈 (heal)、多本体联邦名片发现 (peers)、三态运行模式与真实 A/B 对照 (runtime_mode/ab_report/ab_compare) 以及内生等待事件与 AWR 快照/报告库管理与 ADDM 自调优 (perf_snapshot/perf_awr/perf_snapshots/perf_reports/perf_delete/perf_tune)。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["manifest", "vitals", "envelope", "handoff_save", "handoff_resume", "heal", "peers", "runtime_mode", "ab_report", "ab_compare", "perf_snapshot", "perf_awr", "perf_snapshots", "perf_reports", "perf_delete", "perf_tune"],
                            "default": "manifest",
                            "description": "操作类型"
                        },
                        "mode": { "type": "string", "enum": ["body_only", "bypass", "shadow_ab"], "description": "action=runtime_mode 时设置运行模式" },
                        "shadow_primary": { "type": "string", "enum": ["live", "body"], "description": "action=runtime_mode 时设置 shadow_ab 主响应侧" },
                        "mcp_profile": { "type": "string", "enum": ["core", "dev", "ops", "full"], "description": "action=runtime_mode 时设置 MCP Schema 暴露档位" },
                        "query": { "type": "string", "description": "action=ab_compare 时待对照的检索查询词" },
                        "window": { "type": "string", "enum": ["24h", "7d", "30d", "all"], "default": "7d", "description": "action=ab_report 时回溯统计窗口" },
                        "from_brain": { "type": "string", "description": "发起接力的大脑标识 (如 antigravity / claude-code / codex)" },
                        "session_id": { "type": "string", "description": "当前会话 ID" },
                        "goal_summary": { "type": "string", "description": "handoff_save 必填：当前核心目标摘要" },
                        "active_files": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "正在处理的核心文件路径列表"
                        },
                        "verified_facts": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "已核实的关键物理事实"
                        },
                        "open_blockers": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "尚未解决的阻塞项"
                        },
                        "next_step_hint": { "type": "string", "description": "下一步行动建议" },
                        "checkpoint_id": { "type": "string", "description": "handoff_resume 时指定接力胶囊 ID（留空恢复最新）" },
                        "check_action": { "type": "string", "description": "action=envelope 时待评估的动作名" },
                        "target": { "type": "string", "description": "action=envelope 时待评估的目标路径或资源" },
                        "dry_run": { "type": "boolean", "default": true, "description": "action=envelope 时是否处于 dry_run 预览" },
                        "label": { "type": "string", "description": "action=perf_snapshot 时快照标签" },
                        "set_baseline": { "type": "boolean", "default": false, "description": "action=perf_snapshot 时是否将该快照设为 Golden Baseline 黄金性能基准" },
                        "snap_a": { "type": "string", "description": "action=perf_awr 时基准快照 ID（留空默认用 baseline）" },
                        "snap_b": { "type": "string", "description": "action=perf_awr 时目标快照 ID（留空默认抓取当前最新态）" },
                        "report_id": { "type": "string", "description": "action=perf_awr 时直接查看已保存的历史 AWR 报告 ID" },
                        "vs_baseline": { "type": "boolean", "default": true, "description": "action=perf_awr 时是否对比黄金基线" },
                        "target_kind": { "type": "string", "enum": ["auto", "snapshot", "report"], "default": "auto", "description": "action=perf_delete 时待删除对象类型" },
                        "target_id": { "type": "string", "description": "action=perf_delete 时待删除的快照或 AWR 报告 ID" },
                        "limit": { "type": "integer", "default": 20 }
                    }
                }
            })
        ]
}
