# 06 — 接口规范（API Spec）

> Agent Host Steward 产品设计 · 交付件 6/7
> 版本: v1.0 · 更新: 2026-11-15

---

## 一、接口总览

steward 对外提供三套等价接口，功能完全一致：

| 通道 | 协议 | 传输 | 使用者 |
|------|------|------|--------|
| MCP | JSON-RPC 2.0 | stdio / SSE | AI Agent |
| REST | HTTP/JSON | TCP 127.0.0.1:9527 | 脚本、Web UI、外部集成 |
| CLI | 子命令 | 终端 | 人工操作、shell 脚本 |

三套接口共享同一个 Service 层，行为完全一致。

---

## 二、MCP 工具定义

### 2.1 steward_brief — 上下文摘要

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
    "description": "brief 输出的最大 token 预算。steward 会在此预算内自动裁剪：优先保留最近记忆和环境变更，压缩资产列表和服务详情"
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
        "summary": "项目 steward 主存储选型: SQLite",
        "created_at": "2026-11-15T09:00:00+08:00",
        "tags": ["steward", "存储"]
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
      {"path": "/root/new/work/agent-steward/", "desc": "Agent Host Steward 产品设计", "last_modified": "10min ago"}
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

### 2.2 steward_memory_add — 写入记忆

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

### 2.3 steward_memory_search — 搜索记忆

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
      "content": "项目 steward 主存储选型: SQLite + FTS5 + sqlite-vec",
      "summary": "steward 存储选型: SQLite",
      "tags": ["steward", "存储", "决策"],
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

### 2.4 steward_memory_list — 列出记忆

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

### 2.5 steward_memory_update — 更新记忆

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

### 2.6 steward_memory_delete — 删除记忆

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

### 2.7 steward_asset_register — 注册资产

```json
{
  "path": {
    "type": "string",
    "required": true,
    "description": "文件或目录的绝对路径"
  },
  "type": {
    "type": "string",
    "enum": ["project", "script", "document", "dataset", "config", "media", "other"],
    "required": true
  },
  "description": {
    "type": "string",
    "description": "资产描述"
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
  "path": "/root/new/work/agent-steward/",
  "type": "project",
  "size": "82KB (6 files)",
  "created_at": "2026-11-15T10:30:00+08:00"
}
```

---

### 2.8 steward_asset_search — 搜索资产

```json
{
  "query": {"type": "string", "required": true},
  "type": {"type": "string"},
  "tags": {"type": "array", "items": {"type": "string"}},
  "limit": {"type": "integer", "default": 20}
}
```

---

### 2.9 steward_asset_scan — 扫描目录

```json
{
  "path": {
    "type": "string",
    "required": true,
    "description": "要扫描的目录路径"
  },
  "depth": {
    "type": "integer",
    "default": 3,
    "description": "最大扫描深度"
  },
  "auto_register": {
    "type": "boolean",
    "default": false,
    "description": "自动注册发现的项目目录"
  }
}
```

**返回:**

```json
{
  "scanned_dirs": 156,
  "discovered": [
    {"path": "/root/new/work/project-x/", "type": "project", "indicators": [".git", "Cargo.toml"]},
    {"path": "/root/new/shared_proj/tool.py", "type": "script", "indicators": ["shebang"]}
  ],
  "auto_registered": 0,
  "elapsed_ms": 1230
}
```

---

### 2.10 steward_env_show — 环境信息

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

### 2.11 steward_env_snapshot — 拍环境快照

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

### 2.12 steward_search — 跨维度搜索

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

## 三、REST API 路由表

所有 REST 接口前缀: `http://127.0.0.1:9527/api/v1`

认证: `Authorization: Bearer <token>`（token 在 `~/.steward/token`）

| 方法 | 路径 | 对应 MCP 工具 |
|------|------|---------------|
| GET | `/brief` | steward_brief |
| POST | `/memory` | steward_memory_add |
| GET | `/memory/search?q=...` | steward_memory_search |
| GET | `/memory?last=20` | steward_memory_list |
| PATCH | `/memory/:id` | steward_memory_update |
| DELETE | `/memory/:id` | steward_memory_delete |
| POST | `/asset` | steward_asset_register |
| GET | `/asset/search?q=...` | steward_asset_search |
| POST | `/asset/scan` | steward_asset_scan |
| GET | `/env` | steward_env_show |
| GET | `/env/service/:name` | steward_env_show (service_name) |
| POST | `/env/snapshot` | steward_env_snapshot |
| GET | `/search?q=...` | steward_search |
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
steward                          # 显示帮助
steward init                     # 初始化
steward start                    # 启动 daemon
steward stop                     # 停止 daemon
steward restart                  # 重启
steward status                   # 查看状态
steward health                   # 健康检查
steward logs [--follow]          # 查看日志

# Brief
steward brief [--section X] [--focus KEYWORD] [--verbose]

# Memory
steward memory add --type TYPE --content TEXT [--tags a,b] [--pinned] [--sensitive]
steward memory search QUERY [--type TYPE] [--limit N] [--mode hybrid|fts|semantic]
steward memory list [--last N] [--type TYPE] [--status STATUS]
steward memory show ID
steward memory update ID [--content TEXT] [--status STATUS] [--tags a,b]
steward memory delete ID [--hard]

# Assets
steward asset register PATH --type TYPE [--desc TEXT] [--tags a,b] [--usage TEXT]
steward asset search QUERY [--type TYPE] [--tags a,b]
steward asset scan PATH [--depth N] [--auto-register]
steward asset list [--type TYPE] [--last N]
steward asset show ID
steward asset remove ID [--hard]

# Environment
steward env show [--section SECTION]
steward env service NAME
steward env snapshot [--label LABEL]
steward env snapshots list
steward env diff SNAPSHOT_A SNAPSHOT_B    # P1

# Watch (P1)
steward watch add PATH
steward watch rm PATH
steward watch list

# Changes (P1)
steward changes list [--since DURATION] [--type TYPE]

# Search
steward search QUERY [--scope memory,assets,env] [--limit N]

# Database
steward db stats
steward db vacuum
steward db backup [--output PATH]
steward db repair
steward db reembed                       # 重新生成所有 embedding

# MCP
steward mcp                              # 启动 MCP stdio 通道（供 Agent 连接）

# Export / Import
steward export --format json|sqlite --output PATH [--type memory|assets|all]
steward import --from PATH [--merge]

# Config
steward config show
steward config set KEY VALUE
steward config edit                      # 用 $EDITOR 打开 config.toml
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
    "tags": ["steward"]
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

*下一步: 07-data-model.md — 核心数据模型*
