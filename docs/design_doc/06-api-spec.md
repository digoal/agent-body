# 06 — 接口规范（API Spec）

> [!NOTE]
> **历史设计基线说明**：本文档沉淀自项目早期 v2.0 接口设计期（覆盖早期 15 个工具）。当前 `v0.4.0` 全部 **43 个原生 MCP 工具**（含 `body_state`）、CLI 子命令与 Web REST API 权威规格，请以 [docs/manual/04-mcp-integration.md](../manual/04-mcp-integration.md)、[docs/manual/08-web-dashboard.md](../manual/08-web-dashboard.md) 及源码 [src/api/mcp_tools/mod.rs](../../src/api/mcp_tools/mod.rs) 为准。

> Agent Body 产品设计 · 交付件 6/7
> 版本: v2.0 · 更新: 2026-11-15 (新增端口防撞、混合检索、爆炸半径推演、会话完工结清工具)

---

## 一、接口总览

agent-body 对外提供三套等价接口，功能完全一致：

| 通道 | 协议 | 传输 | 使用者 |
|------|------|------|--------|
| MCP | JSON-RPC 2.0 | stdio / SSE | AI Agent |
| REST | HTTP/JSON | TCP 127.0.0.1:9527 | 脚本、Web UI、外部集成 |
| CLI | 子命令 | 终端 | 人工操作、shell 脚本 |

三套接口共享同一个 Service 层，行为完全一致。

---

## 二、MCP 工具定义

### 2.1 body_brief — 上下文摘要

获取当前机器的完整上下文摘要，供 Agent 在新会话开始时快速了解环境。

**参数:**

```json
{
  "section": {
    "type": "string",
    "enum": ["all", "memory", "assets", "env", "changes"],
    "default": "all",
    "description": "返回哪个维度的摘要"
  },
  "focus": {
    "type": "string",
    "description": "关键词过滤，只返回与该关键词相关的信息"
  },
  "verbose": {
    "type": "boolean",
    "default": false,
    "description": "详细模式，返回更多细节"
  },
  "max_tokens": {
    "type": "integer",
    "default": 2000,
    "minimum": 200,
    "maximum": 8000,
    "description": "brief 输出的最大 token 预算。agent-body 会在此预算内自动裁剪：优先保留最近记忆和环境变更，压缩资产列表和服务详情"
  }
}
```

**返回:**

```json
{
  "timestamp": "2026-11-15T10:30:00+08:00",
  "memory": {
    "total": 8234,
    "recent": [
      {
        "id": "uuid",
        "type": "decision",
        "summary": "项目 agent-body 主存储选型: SQLite",
        "created_at": "2026-11-15T09:00:00+08:00",
        "tags": ["agent-body", "存储"]
      }
    ]
  },
  "env": {
    "os": "Ubuntu 22.04 LTS (x86_64)",
    "cpu": "AMD EPYC 7763 (128 cores)",
    "memory": "251.5 GB (used: 45.2 GB)",
    "disk": "/: 1.8TB (49%)",
    "services": [
      {"name": "PostgreSQL", "version": "16.4", "port": 5432, "status": "running"}
    ]
  },
  "assets": {
    "total": 12456,
    "active_projects": [
      {"path": "/root/new/work/agent-body/", "desc": "Agent Body 产品设计", "last_modified": "10min ago"}
    ]
  },
  "changes": {
    "last_24h": 15,
    "highlights": [
      {"type": "file_modified", "target": "03-feature-spec.md", "time": "2026-11-15T10:24:00+08:00"}
    ]
  }
}
```

---

### 2.2 body_placement_suggest — 对象放置与路径建议

当 Agent 准备在宿主机新建项目、生成脚本、写入重要笔记或产生临时数据时调用此接口，获取符合宿主机规范的标准落盘路径与命名安全指引。

**参数:**

```json
{
  "object_type": {
    "type": "string",
    "enum": ["project", "script", "document", "dataset", "note", "tmp", "skill", "other"],
    "required": true,
    "description": "准备生成的对象类别"
  },
  "name": {
    "type": "string",
    "required": true,
    "description": "对象名称或建议文件名 (例如 'sec_scanner', 'pg_tuning_notes')"
  },
  "intent": {
    "type": "string",
    "required": true,
    "description": "对象的用途与生命周期预期 (例如 '长期复用的美股数据抓取工具', '临时调试运行日志')"
  },
  "tags": {
    "type": "array",
    "items": {"type": "string"},
    "description": "技术栈或业务标签 (如 ['python', 'sec', 'finance'])"
  },
  "security_level": {
    "type": "string",
    "enum": ["public", "internal", "confidential", "secret"],
    "default": "internal",
    "description": "保密级别"
  }
}
```

**返回:**

```json
{
  "suggested_path": "/root/new/shared_proj/sec_scanner/",
  "rule_matched": "shared_tools",
  "reason": "长期复用的程序与脚本库统一存放在 /root/new/shared_proj/",
  "naming_convention": "snake_case 目录命名，子目录下配备入口脚本与 README.md",
  "recommended_permissions": "0755",
  "conflict_detected": false,
  "guidelines": [
    "需包含使用示例与依赖说明",
    "严禁将敏感凭据硬编码，应通过环境变量或脱敏配置传入"
  ]
}
```

---

### 2.3 body_placement_rules — 获取目录规范规则库

**参数:**

```json
{
  "pattern_type": {
    "type": "string",
    "description": "过滤指定类型的规则 (project/script/tmp/note 等)"
  }
}
```

**返回:**

```json
{
  "rules": [
    {"rule_name": "shared_proj", "type": "script", "target_base_dir": "/root/new/shared_proj/", "desc": "长期沉淀的程序与脚本"},
    {"rule_name": "work_projects", "type": "project", "target_base_dir": "/root/new/work/", "desc": "常规立项的工作工程目录"},
    {"rule_name": "tmp_files", "type": "tmp", "target_base_dir": "/root/new/tmp/", "desc": "可随时清理的临时文件与日志"},
    {"rule_name": "skills", "type": "skill", "target_base_dir": "/root/new/skills/", "desc": "沉淀的 AI 技能目录"},
    {"rule_name": "notes", "type": "note", "target_base_dir": "/root/new/note/", "desc": "重要笔记与清单"}
  ]
}
```

---

### 2.4 body_memory_add — 写入记忆

```json
{
  "type": {
    "type": "string",
    "enum": ["decision", "task", "experience", "preference", "fact", "note"],
    "required": true,
    "description": "记忆类型"
  },
  "content": {
    "type": "string",
    "required": true,
    "description": "记忆正文"
  },
  "summary": {
    "type": "string",
    "description": "一句话摘要，不提供则自动从 content 截取前 100 字"
  },
  "tags": {
    "type": "array",
    "items": {"type": "string"},
    "description": "标签列表"
  },
  "source_session": {
    "type": "string",
    "description": "来源会话 ID（Agent 自动填充）"
  },
  "source_agent": {
    "type": "string",
    "description": "来源 Agent 名称"
  },
  "pinned": {
    "type": "boolean",
    "default": false,
    "description": "是否永不衰减"
  },
  "sensitive": {
    "type": "boolean",
    "default": false,
    "description": "是否敏感信息（掩码输出）"
  },
  "context_ids": {
    "type": "array",
    "items": {"type": "string"},
    "description": "关联的资产/服务/其他记忆的 ID"
  }
}
```

**返回:**

```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "created_at": "2026-11-15T10:30:00+08:00",
  "embedding_status": "done"
}
```

---

### 2.3 body_memory_search — 搜索记忆

```json
{
  "query": {
    "type": "string",
    "required": true,
    "description": "搜索关键词或自然语言描述"
  },
  "type": {
    "type": "string",
    "enum": ["decision", "task", "experience", "preference", "fact", "note"],
    "description": "按类型过滤"
  },
  "tags": {
    "type": "array",
    "items": {"type": "string"},
    "description": "按标签过滤（AND 关系）"
  },
  "time_range": {
    "type": "object",
    "properties": {
      "start": {"type": "string", "format": "date-time"},
      "end": {"type": "string", "format": "date-time"}
    },
    "description": "时间范围"
  },
  "status": {
    "type": "string",
    "enum": ["active", "archived", "outdated"],
    "default": "active"
  },
  "limit": {
    "type": "integer",
    "default": 20,
    "maximum": 100
  },
  "search_mode": {
    "type": "string",
    "enum": ["hybrid", "fts", "semantic"],
    "default": "hybrid",
    "description": "搜索模式"
  },
  "fts_weight": {
    "type": "number",
    "minimum": 0,
    "maximum": 1,
    "default": 0.5,
    "description": "FTS 在混合搜索中的权重"
  }
}
```

**返回:**

```json
{
  "results": [
    {
      "id": "uuid",
      "type": "decision",
      "content": "项目 agent-body 主存储选型: SQLite + FTS5 + sqlite-vec",
      "summary": "agent-body 存储选型: SQLite",
      "tags": ["agent-body", "存储", "决策"],
      "weight": 0.95,
      "score": 0.87,
      "created_at": "2026-11-15T09:00:00+08:00",
      "source": {
        "agent": "Antigravity",
        "session_id": "abc-123"
      }
    }
  ],
  "total": 3,
  "query_time_ms": 45
}
```

---

### 2.4 body_memory_list — 列出记忆

```json
{
  "last": {
    "type": "integer",
    "description": "最近 N 条",
    "default": 20
  },
  "type": {"type": "string"},
  "status": {"type": "string", "default": "active"},
  "offset": {"type": "integer", "default": 0}
}
```

---

### 2.5 body_memory_update — 更新记忆

```json
{
  "id": {
    "type": "string",
    "required": true,
    "description": "记忆 ID"
  },
  "content": {"type": "string", "description": "更新正文"},
  "summary": {"type": "string"},
  "status": {"type": "string", "enum": ["active", "archived", "outdated"]},
  "tags": {"type": "array", "items": {"type": "string"}},
  "pinned": {"type": "boolean"},
  "weight": {"type": "number", "minimum": 0, "maximum": 1}
}
```

---

### 2.6 body_memory_delete — 删除记忆

```json
{
  "id": {
    "type": "string",
    "required": true
  },
  "hard": {
    "type": "boolean",
    "default": false,
    "description": "true=物理删除, false=标记为 archived"
  }
}
```

---

### 2.9 body_asset_register — 注册资产

```json
{
  "path": {
    "type": "string",
    "required": true,
    "description": "文件或目录的绝对路径"
  },
  "type": {
    "type": "string",
    "enum": ["directory", "project", "script", "document", "dataset", "config", "media", "other"],
    "required": true,
    "description": "资产类型，包含 directory 目录一等公民"
  },
  "owner": {
    "type": "string",
    "default": "user:digoal",
    "description": "归属主体 (如 'user:digoal', 'agent:session-123', 'system')"
  },
  "security_level": {
    "type": "string",
    "enum": ["public", "internal", "confidential", "secret"],
    "default": "internal",
    "description": "保密级别"
  },
  "description": {
    "type": "string",
    "description": "资产人类可读描述"
  },
  "desc_kv": {
    "type": "object",
    "description": "结构化语义元数据 KV (如 {'purpose': '美股抓取', 'ttl': '30d', 'upstream': 'SEC'})"
  },
  "tags": {
    "type": "array",
    "items": {"type": "string"}
  },
  "usage": {
    "type": "string",
    "description": "使用方法说明（特别是脚本类）"
  },
  "dependencies": {
    "type": "array",
    "items": {"type": "string"},
    "description": "依赖的其他资产路径或 ID"
  }
}
```

**返回:**

```json
{
  "id": "uuid",
  "path": "/root/new/work/agent-body/",
  "type": "project",
  "owner": "user:digoal",
  "security_level": "internal",
  "size": "82KB (6 files)",
  "created_at": "2026-11-15T10:30:00+08:00"
}
```

---

### 2.10 body_asset_search — 搜索资产 (Phase 59~63 Omni-RRF)

```json
{
  "query": {"type": "string", "required": true, "description": "自然语言或关键词查询"},
  "limit": {"type": "integer", "default": 20},
  "detail_level": {"type": "string", "enum": ["micro", "standard", "full"], "default": "standard"},
  "asset_type": {"type": "string", "description": "按资产类型过滤 (如 directory/script/project)"},
  "modality": {"type": "string", "description": "按模态过滤 (code/doc/image/pdf/office/db/audio/video)"},
  "kv_filter": {"type": "string", "description": "desc_kv 元数据过滤 (如 schema_version=2.0 或 format=svg)"},
  "graph_hops": {"type": "integer", "default": 1, "description": "属性图拓展跳数 (0~2)"},
  "fuzzy": {"type": "boolean", "default": true, "description": "是否启用模糊 n-gram/编辑距离通道"},
  "include_links": {"type": "boolean", "default": true, "description": "是否融合 links 文档/代码互链"}
}
```

---

### 2.11 body_asset_scan — 目录扫描与分级颗粒度建档 (Phase 59~63)

依据宿主机 `scan_policies` 策略自动执行分级扫描，**杜绝一股脑全量生成**：
- 白名单目录：深入递归到最深层，提取纯 Rust 多模态 L1A (`schema_version=2.0`) + `asset_chunk` 切片 + 全文 FTS5 + 向量 Embedding + 关联 Links；
- 黑名单目录：**止于当前目录层级**，仅建档该目录实体本身标量，绝不深入递归子文件，严禁文本切块与向量；
- 受限目录：仅浅层索引主要入口与 README，跳过大文件向量化。

```json
{
  "path": {
    "type": "string",
    "required": true,
    "description": "要扫描的目录路径"
  },
  "enrich_with_ollama": {"type": "boolean", "default": false, "description": "是否调用本地 Ollama 进行 L2 语义/视觉增强标注"},
  "enrich_limit": {"type": "integer", "default": 50, "description": "单批增强最大条目数"},
  "mine_knowledge": {"type": "boolean", "default": false, "description": "是否同步从章节切片挖掘实体属性图与 PMI 术语共现"},
  "extract_knowhow": {"type": "boolean", "default": false, "description": "是否使用 L2 模型抽取带原文锚定校验的原子 Know-How"},
  "backfill_multimodal": {"type": "boolean", "default": false, "description": "是否仅回填存量缺失 schema_version=2.0 的多模态资产"}
}
```

**返回:**

```json
{
  "scanned_dirs": 156,
  "deep_scanned_files": 342,
  "shallow_blocked_dirs": [
    {
      "path": "/root/new/work/project-x/node_modules",
      "size": "340MB",
      "items_count": 14208,
      "tier": "shallow",
      "reason": "黑名单策略命中: 止于当前目录层级，未递归下钻，未生成向量与全文"
    }
  ],
  "discovered": [
    {"path": "/root/new/work/project-x/", "type": "project", "tier": "deep"},
    {"path": "/root/new/shared_proj/tool.py", "type": "script", "tier": "deep"}
  ],
  "embeddings_queued": 48,
  "elapsed_ms": 860
}
```

---

### 2.12 body_env_processes — 动态任务与进程感知

查询宿主机上除 systemd 守护服务外的活跃动态对象（任务脚本、Worker、临时分析进程、GPU占用与端口）。

**参数:**

```json
{
  "owner": {
    "type": "string",
    "description": "按发起主体过滤 (如 'agent:hermes_cron')"
  },
  "min_cpu_pct": {
    "type": "number",
    "default": 1.0,
    "description": "过滤 CPU 占用底噪 (百分比)"
  }
}
```

**返回:**

```json
{
  "processes": [
    {
      "pid": 14208,
      "name": "python3",
      "command": "python /root/new/shared_proj/earnings_scan/scan_sec.py",
      "owner": "agent:hermes_cron",
      "associated_path": "/root/new/shared_proj/earnings_scan/scan_sec.py",
      "cpu_pct": 98.2,
      "mem_bytes": 440401920,
      "gpu_allocated_mb": 0,
      "started_at": "2026-11-15T08:30:00+08:00",
      "status": "running"
    }
  ]
}
```

---

### 2.10 body_env_show — 环境信息

```json
{
  "section": {
    "type": "string",
    "enum": ["all", "system", "services", "software", "network", "cron", "env_vars"],
    "default": "all"
  },
  "service_name": {
    "type": "string",
    "description": "查看特定服务详情"
  }
}
```

**返回 (section=services):**

```json
{
  "services": [
    {
      "name": "PostgreSQL",
      "version": "16.4",
      "port": 5432,
      "pid": 1234,
      "status": "running",
      "uptime": "15d 3h",
      "details": {
        "data_dir": "/data/pg16",
        "config_file": "/etc/postgresql/16/main/postgresql.conf",
        "connections": {"active": 5, "max": 200},
        "databases": ["postgres", "mydb", "analytics"],
        "total_size": "45.2 GB"
      }
    }
  ]
}
```

---

### 2.11 body_env_snapshot — 拍环境快照

```json
{
  "label": {
    "type": "string",
    "description": "快照标签（可选）"
  }
}
```

**返回:**

```json
{
  "snapshot_id": "uuid",
  "timestamp": "2026-11-15T10:30:00+08:00",
  "label": "before-upgrade",
  "summary": {
    "services": 5,
    "software_packages": 1234,
    "env_vars": 89,
    "cron_jobs": 17
  }
}
```

---

### 2.12 body_search — 跨维度搜索

```json
{
  "query": {
    "type": "string",
    "required": true,
    "description": "搜索查询"
  },
  "scope": {
    "type": "array",
    "items": {"type": "string", "enum": ["memory", "assets", "env", "changes"]},
    "default": ["memory", "assets", "env"],
    "description": "搜索范围"
  },
  "limit": {
    "type": "integer",
    "default": 10,
    "description": "每个维度的最大结果数"
  }
}
```

**返回:**

```json
{
  "memory": [
    {"id": "uuid", "type": "decision", "summary": "...", "score": 0.92}
  ],
  "assets": [
    {"id": "uuid", "path": "/root/...", "type": "project", "score": 0.85}
  ],
  "env": [
    {"type": "service", "name": "PostgreSQL", "relevance": "version matches query"}
  ],
  "query_time_ms": 120
}
```

---

### 2.16 body_port_check — 端口防撞诊断与可用推荐

在启动任何服务或监听端口前主动校验冲突。

**参数:**
```json
{
  "port": { "type": "integer", "description": "拟占用的网络端口号", "minimum": 1, "maximum": 65535 },
  "suggest_count": { "type": "integer", "default": 3, "description": "若冲突，返回推荐可用空闲端口个数" }
}
```

**返回:**
```json
{
  "port": 8080,
  "status": "OCCUPIED",
  "conflict": {
    "pid": 29841,
    "name": "python3",
    "command": "python /root/new/shared_proj/dashboard/app.py",
    "owner": "user:digoal"
  },
  "recommended_free_ports": [8081, 8082, 8083],
  "advice": "端口已被用户进程占用，建议使用 8081"
}
```

---

### 2.17 body_links_impact — 变更爆炸半径推演

评估重构、改动或删除特定文件/服务对上下游的影响范围。

**参数:**
```json
{
  "target": { "type": "string", "description": "被评估对象的绝对路径或服务名" },
  "max_depth": { "type": "integer", "default": 3, "description": "依赖传递推演最大跳数" }
}
```

**返回:**
```json
{
  "target": "/root/new/shared_proj/sec_fetcher/utils.py",
  "risk_level": "HIGH",
  "direct_dependents": [
    { "type": "asset", "id": "/root/new/work/earnings_cron/run.py", "relation": "references" }
  ],
  "indirect_dependents": [
    { "type": "cron", "id": "sec-daily-scan", "depth": 2 }
  ],
  "summary": "发现 1 个直接脚本与 1 个定时任务依赖该资产，改动需确保接口向后兼容。"
}
```

---

### 2.18 body_session_close — 会话完工结算与垃圾回收

在 Agent 任务完成时调用，清理临时资产并归档会话纪要。

**参数:**
```json
{
  "session_id": { "type": "string", "description": "当前会话的唯一标识" },
  "clean_temp": { "type": "boolean", "default": true, "description": "是否自动回收 /root/new/tmp 下本会话创建的临时文件" }
}
```

**返回:**
```json
{
  "session_id": "session-1024",
  "cleaned_files_count": 3,
  "freed_bytes": 1048576,
  "changes_summary": "本会话新增文件 2 个，修改 1 个",
  "saved_memory_id": "mem-auto-close-883"
}
```

---

## 三、REST API 路由表

所有 REST 接口前缀: `http://127.0.0.1:9527/api/v1`

认证: `Authorization: Bearer <token>`（token 在 `~/.agent-body/token`）

| 方法 | 路径 | 对应 MCP 工具 |
|------|------|---------------|
| GET | `/brief` | body_brief |
| POST | `/memory` | body_memory_add |
| GET | `/memory/search?q=...` | body_memory_search |
| GET | `/memory?last=20` | body_memory_list |
| PATCH | `/memory/:id` | body_memory_update |
| DELETE | `/memory/:id` | body_memory_delete |
| POST | `/asset` | body_asset_register |
| GET | `/asset/search?q=...` | body_asset_search |
| POST | `/asset/scan` | body_asset_scan |
| GET | `/env` | body_env_show |
| GET | `/env/service/:name` | body_env_show (service_name) |
| POST | `/env/snapshot` | body_env_snapshot |
| GET | `/search?q=...` | body_search |
| GET | `/health` | 健康检查（无需认证） |
| GET | `/status` | 服务状态 |

### REST 错误格式

```json
{
  "error": {
    "code": "NOT_FOUND",
    "message": "Memory with id 'uuid' not found",
    "details": {}
  }
}
```

标准错误码:

| HTTP | code | 说明 |
|------|------|------|
| 400 | INVALID_REQUEST | 参数校验失败 |
| 401 | UNAUTHORIZED | Token 缺失或无效 |
| 404 | NOT_FOUND | 资源不存在 |
| 409 | CONFLICT | 资源冲突（如重复注册） |
| 429 | RATE_LIMITED | 请求过频 |
| 500 | INTERNAL_ERROR | 内部错误 |

---

## 四、CLI 命令清单

```
agent-body                          # 显示帮助
agent-body init                     # 初始化
agent-body start                    # 启动 daemon
agent-body stop                     # 停止 daemon
agent-body restart                  # 重启
agent-body status                   # 查看状态
agent-body health                   # 健康检查
agent-body logs [--follow]          # 查看日志

# Brief
agent-body brief [--section X] [--focus KEYWORD] [--verbose]

# Memory
agent-body memory add --type TYPE --content TEXT [--tags a,b] [--pinned] [--sensitive]
agent-body memory search QUERY [--type TYPE] [--limit N] [--mode hybrid|fts|semantic]
agent-body memory list [--last N] [--type TYPE] [--status STATUS]
agent-body memory show ID
agent-body memory update ID [--content TEXT] [--status STATUS] [--tags a,b]
agent-body memory delete ID [--hard]

# Assets
agent-body asset register PATH --type TYPE [--desc TEXT] [--tags a,b] [--usage TEXT]
agent-body asset search QUERY [--type TYPE] [--tags a,b]
agent-body asset scan PATH [--depth N] [--auto-register]
agent-body asset list [--type TYPE] [--last N]
agent-body asset show ID
agent-body asset remove ID [--hard]

# Environment
agent-body env show [--section SECTION]
agent-body env service NAME
agent-body env snapshot [--label LABEL]
agent-body env snapshots list
agent-body env diff SNAPSHOT_A SNAPSHOT_B    # P1

# Watch (P1)
agent-body watch add PATH
agent-body watch rm PATH
agent-body watch list

# Changes (P1)
agent-body changes list [--since DURATION] [--type TYPE]

# Search
agent-body search QUERY [--scope memory,assets,env] [--limit N]

# Database
agent-body db stats
agent-body db vacuum
agent-body db backup [--output PATH]
agent-body db repair
agent-body db reembed                       # 重新生成所有 embedding

# MCP
agent-body mcp                              # 启动 MCP stdio 通道（供 Agent 连接）

# Export / Import
agent-body export --format json|sqlite --output PATH [--type memory|assets|all]
agent-body import --from PATH [--merge]

# Config
agent-body config show
agent-body config set KEY VALUE
agent-body config edit                      # 用 $EDITOR 打开 config.toml
```

---

## 五、事件钩子接口（P2）

```json
{
  "hook": "on_memory_write",
  "event": {
    "memory_id": "uuid",
    "type": "decision",
    "content": "...",
    "tags": ["agent-body"]
  },
  "timestamp": "2026-11-15T10:30:00+08:00"
}
```

支持的钩子:

| 钩子 | 触发时机 |
|------|----------|
| `on_memory_write` | 新记忆写入 |
| `on_memory_update` | 记忆更新 |
| `on_asset_register` | 新资产注册 |
| `on_asset_change` | 资产文件变更（需 watch） |
| `on_env_diff` | 环境快照发现差异 |
| `on_service_status` | 服务状态变化（启停） |

钩子通过配置注册：

```toml
[[hooks]]
event = "on_memory_write"
action = "exec"
command = "/path/to/handler.sh"

[[hooks]]
event = "on_service_status"
action = "webhook"
url = "http://localhost:8080/notify"
```

---

## 7. 图智能与联邦协同工具 (v3.0 扩充)

### 7.1 `body_links_audit`
全局依赖断链与幽灵引用审计工具。检测目标文件是否存在、目标服务端口是否在线。

- **输入**:
  ```json
  {
    "fix_dangling": false
  }
  ```
- **输出**:
  ```json
  {
    "health_score": 92,
    "total_links": 48,
    "dangling_count": 2,
    "issues": [
      {
        "source": "/root/new/cron/old_backup.sh",
        "target": "/root/new/shared_proj/obsolete_util.py",
        "issue_type": "MISSING_FILE",
        "detail": "目标文件在磁盘上不存在",
        "line_number": 15
      }
    ]
  }
  ```

### 7.2 `body_codegraph_bridge`
跨层联动 CodeGraph 微观代码图。在目标工程内钻取函数/符号级调用拓扑。

- **输入**:
  ```json
  {
    "path": "/root/new/src/hk2",
    "symbol": "fetch_data"
  }
  ```
- **输出**:
  ```json
  {
    "has_codegraph": true,
    "codegraph_db": "/root/new/src/hk2/.codegraph/codegraph.db",
    "symbol": "fetch_data",
    "callers": [
      {
        "caller_name": "process_pipeline",
        "qualified_name": "hk2::pipeline::process_pipeline",
        "file_path": "src/pipeline.rs",
        "line": 88,
        "kind": "calls"
      }
    ]
  }
  ```

---

### 7.3 Web UI 2.0 (`8765`) & Phase 58~63 增量契约

1. **`body_state` 新增三态运行模式与 Shadow A/B Action**：
   - `runtime_status`：查看当前 `mode`（`bypass` / `shadow_ab` / `body_only`）、`AGENT_BODY_MODE` 覆盖状态、`mcp_profile`（`core`/`dev`/`ops`/`full`）与 `shadow_primary`；
   - `runtime_mode`：运行时切换 `mode` / `mcp_profile` / `shadow_primary`；
   - `ab_compare`：同机执行 Live 扫描 vs Body FTS 索引对照，记录漏召回/陈旧条目与 `H1~H5` 自动自愈计数（持久化 `audit_events` `category="ab_shadow"`）；
   - `ab_report`：按时间窗聚合 `H1~H5` 自愈率与帮倒忙率。
2. **Web REST 新增/对齐端点（默认监听 `127.0.0.1:8765`）**：
   - `GET /api/v1/body/runtime-mode` / `POST /api/v1/body/runtime-mode`
   - `POST /api/v1/body/ab-compare` / `GET /api/v1/body/ab-report?window=7d`
   - `GET /api/v1/assets/search?q=...&asset_type=...&modality=...&kv_filter=...&graph_hops=...&fuzzy=...&include_links=...`
   - `POST /api/v1/assets/scan`（支持 `enrich_with_ollama`/`mine_knowledge`/`extract_knowhow`/`backfill_multimodal`）

*下一步: 07-data-model.md — 核心数据模型*
