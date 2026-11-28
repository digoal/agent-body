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
    let mut schemas = vec![
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
                "description": "全文检索与语义搜索宿主机已纳管资产（支持 micro/standard/full 三级渐进披露）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "搜索关键词或自然语言"
                        },
                        "limit": {
                            "type": "integer",
                            "default": 10,
                            "description": "最多返回条数"
                        },
                        "detail_level": {
                            "type": "string",
                            "enum": ["micro", "standard", "full"],
                            "default": "standard",
                            "description": "披露粒度: micro(~25t)|standard(~85t)|full(~180t)"
                        },
                        "modality": {
                            "type": "string",
                            "enum": ["image", "audio", "video", "pdf", "office", "dataset", "code", "document", "config", "media", "other"],
                            "description": "多模态类别过滤"
                        },
                        "asset_type": {
                            "type": "string",
                            "description": "资产类型 (script/document/dataset/config/media/other)"
                        },
                        "domain": {
                            "type": "string",
                            "description": "业务领域过滤"
                        },
                        "kv_key": {
                            "type": "string",
                            "description": "要求 desc_kv 含指定键"
                        },
                        "kv_value": {
                            "type": "string",
                            "description": "要求 desc_kv 含指定子串"
                        },
                        "kv_filter": {
                            "type": "string",
                            "description": "多谓词过滤 (如 importance_score>=0.7,modality=document)"
                        },
                        "graph_hops": {
                            "type": "integer",
                            "default": 1,
                            "description": "知识网络扩散跳数 (0|1|2)"
                        },
                        "fuzzy": {
                            "type": "boolean",
                            "default": true,
                            "description": "DB 模糊容错匹配"
                        },
                        "use_fuzzy_index": {
                            "type": "boolean",
                            "default": true,
                            "description": "DB Trigram 索引加速 (false 退化 LIKE)"
                        },
                        "hybrid": {
                            "type": "boolean",
                            "default": true,
                            "description": "语义向量 RRF 融合"
                        },
                        "scope_path": {
                            "type": "string",
                            "description": "可选：限定检索路径前缀目录 (对标 rg <query> <dir>)"
                        },
                        "include_links": {
                            "type": "boolean",
                            "default": true,
                            "description": "是否包含 links 文档互链扩展通道"
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
                        },
                        "mine_knowledge": {
                            "type": "boolean",
                            "default": false,
                            "description": "扫描后是否同步构建 L1 Know-How 章节/术语/PMI 知识网络"
                        },
                        "extract_knowhow": {
                            "type": "boolean",
                            "default": false,
                            "description": "扫描后是否调用本地小模型 (think=false + 原文锚定) 抽取章节级 L2 Know-How 因果与边界边"
                        },
                        "backfill_multimodal": {
                            "type": "boolean",
                            "default": false,
                            "description": "扫描后是否幂等回填升级存量 other 或旧版 schema 多模态资产"
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
                "description": "向Agent 本体写入一条长期记忆（决策依据、排障经验、技术偏好或事实），跨会话持久化；启用 pg_tx 时支持双时态 fact_key 写穿与 expected_epoch 乐观并发校验。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "type": {
                            "type": "string",
                            "enum": ["decision", "task", "experience", "preference", "fact", "note"],
                            "default": "decision"
                        },
                        "content": { "type": "string", "description": "记忆正文" },
                        "summary": { "type": "string", "description": "一句话摘要" },
                        "fact_key": { "type": "string", "description": "可选逻辑主键（用于双时态版本链与写穿更新，如 file:/path 或 rule:port_policy）" },
                        "evidence_tier": {
                            "type": "string",
                            "enum": ["L1_EXPLICIT", "L2_STRUCTURAL", "L3_SEMANTIC"],
                            "default": "L3_SEMANTIC",
                            "description": "证据分级"
                        },
                        "expected_epoch": { "type": "integer", "description": "可选 CAS 期望逻辑纪元 (tx_epoch)，不匹配时返回 MVCC 冲突" },
                        "tenant_scope": { "type": "string", "description": "可选项目/租户隔离域 (默认读取配置)" }
                    },
                    "required": ["content"]
                }
            }),
            serde_json::json!({
                "name": "body_memory_search",
                "description": "语义与全文混合检索历史记忆与决策记录（支持 RRF 混合融合排序，以及 pg_tx 双时态 as_of_epoch 快照与历史版本回溯）。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "搜索关键词或语义描述" },
                        "limit": { "type": "integer", "default": 10 },
                        "hybrid": { "type": "boolean", "default": true, "description": "是否启用 Ollama 向量+FTS5 混合重排" },
                        "as_of_epoch": { "type": "integer", "description": "可选逻辑纪元快照时间旅行 (tx_epoch <= as_of_epoch)" },
                        "include_history": { "type": "boolean", "default": false, "description": "是否包含已关闭/覆盖的历史双时态版本" },
                        "tenant_scope": { "type": "string", "description": "可选项目/租户隔离域" }
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
                "description": "长期记忆健康体检与矛盾仲裁：检测对立冲突的认知记忆对，计算温和时间衰减淘汰候选，支持指定归档旧条目，查询 PG 事务记忆健康状态 (tx_status)、按 fact_key 执行原子墓碑失效 (tombstone_fact_key)、L1_AUTONOMIC 空闲睡眠整理控制 (sleep_action)，以及方案 B 旁路快权重血缘/遗忘/回滚控制 (fast_weight_action)。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "archive_id": { "type": "string", "description": "若存在冲突，指定归档淘汰的旧记忆 ID" },
                        "tx_status": { "type": "boolean", "default": false, "description": "是否返回 PG 事务记忆引擎的健康态、当前 tx_epoch、活跃事件与逻辑分段计数" },
                        "tombstone_fact_key": { "type": "string", "description": "可选：按逻辑事实键 (fact_key) 对 PG 事务记忆执行写穿墓碑逻辑失效，清除向量/索引召回" },
                        "sleep_action": { "type": "string", "enum": ["status", "run", "dry_run", "pause", "resume"], "description": "可选：睡眠整理观测与控制 (status 查询状态 | run 执行压缩 | dry_run 预览 | pause 暂停周期触发 | resume 恢复)" },
                        "force": { "type": "boolean", "default": false, "description": "当 sleep_action=run 时，是否忽略空闲窗口门禁强制执行单轮整理" },
                        "fast_weight_action": { "type": "string", "enum": ["status", "lineage", "compact", "unlearn", "rollback"], "description": "可选（Phase 78 方案 B）：旁路快权重血缘、神经 Compaction、定向遗忘与按 epoch 回滚控制" },
                        "target_epoch": { "type": "integer", "description": "当 fast_weight_action=rollback 时指定回滚目标 tx_epoch" },
                        "snapshot_id": { "type": "string", "description": "当 fast_weight_action=lineage 时指定追溯起点快照 UUID（留空追当前活跃快照）" },
                        "base_model_id": { "type": "string", "description": "当 compact / unlearn 时指定目标基座模型 ID" },
                        "adapter_kind": { "type": "string", "enum": ["inplace_ttt", "atlas_state", "larimar_mem", "scol_lora"], "description": "可选：快权重适配器形态" }
                    }
                }
            }),
    ];
    schemas.extend(super::mcp_schemas_ops::ops_tool_schemas());
    schemas
}
