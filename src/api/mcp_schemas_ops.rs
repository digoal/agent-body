use serde_json::Value;

/// 返回工具 #23~43 的 MCP JSON Schema 定义（从 mcp_schemas.rs 拆分，保持单文件高可读性）
pub(crate) fn ops_tool_schemas() -> Vec<Value> {
    vec![
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
                "description": "基于 DuckDB 原生列式引擎将指定的Agent 本体数据表秒级导出为 Parquet 格式，供离线深度分析或数据科学工具消费。",
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
                "description": "实证 Benchmark 对照评测与生产效能存证账本引擎（v3.9）：运行 A/B 对照负载(W1~W19)、内核微基准(M1~M6)、数据质量与遥测/图谱健康(DQ1~DQ10)、热门竞品横向对标(COMP-C1~C4)或审计生产效能账本(PR-01~PR-14)。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": { "type": "string", "enum": ["workloads", "run", "report", "ledger", "compare", "export", "reset", "competitors"], "default": "report" },
                        "suite": { "type": "string", "default": "full", "description": "full | macro | micro | dq | competitors | W1~W19 | COMP-C1~C4" },
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
                "description": "Agent 本体中枢（宿主机具身底座，第 43 原生工具）：提供本体身份证与六器官自省 (manifest)、五维稳态生命体征 (vitals)、四级自治权限包络评估 (envelope)、跨大脑工作态接力胶囊保存与恢复 (handoff_save/handoff_resume)、L1 稳态反射自愈 (heal)、多本体联邦名片发现 (peers)、三态运行模式与真实 A/B 对照 (runtime_mode/ab_report/ab_compare)、OpenMetrics/JSON 实时指标 (metrics) 以及内生等待事件与 AWR 快照/报告库管理与 ADDM 自调优 (perf_snapshot/perf_awr/perf_snapshots/perf_reports/perf_delete/perf_tune)。",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "action": {
                            "type": "string",
                            "enum": ["manifest", "vitals", "envelope", "handoff_save", "handoff_resume", "heal", "peers", "runtime_mode", "ab_report", "ab_compare", "metrics", "perf_snapshot", "perf_awr", "perf_snapshots", "perf_reports", "perf_delete", "perf_tune"],
                            "default": "manifest",
                            "description": "操作类型"
                        },
                        "format": { "type": "string", "enum": ["prometheus", "json"], "default": "prometheus", "description": "action=metrics 时输出格式" },
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
