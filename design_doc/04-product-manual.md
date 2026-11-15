# 04 — 产品手册（Product Manual）

> Agent Host Steward 产品设计 · 交付件 4/7
> 版本: v1.0 · 更新: 2026-11-15

---

## steward — Agent 宿主机的环境大管家

**steward** 是一个驻留在你机器上的本地服务，为 AI Agent 提供持久化的记忆、资产索引、环境感知和变更追踪能力。

它解决一个根本问题：**Agent 每开一个新会话就失忆，不知道你的机器上有什么、项目进展到哪了、上次做过什么决定。**

装上 steward 之后，任何 Agent 都能在新会话的第一秒就知道你的全部上下文。

---

## 快速开始

### 安装

```bash
# Linux / macOS 一键安装
curl -sSL https://steward.dev/install | sh

# 或用 Homebrew
brew install steward
```

### 初始化

```bash
steward init
```

这会做三件事：
1. 创建 `~/.steward/` 目录和数据库
2. 快速扫描你的系统环境（运行着什么服务、有哪些项目目录）
3. 启动 steward 后台服务

初始化完成后你会看到：

```
✅ steward initialized
   Database:  ~/.steward/steward.db
   Config:    ~/.steward/config.toml
   Services:  Found 5 running services (PostgreSQL 16, Redis 7.2, Ollama, ...)
   Projects:  Found 12 project directories
   Daemon:    Running (PID 42381)
   MCP:       Ready (steward mcp)
   REST:      http://127.0.0.1:9527
```

### 连接你的 Agent

#### Antigravity

在 `~/.gemini/settings.json` 或你的 MCP 配置中添加：

```json
{
  "mcpServers": {
    "steward": {
      "command": "steward",
      "args": ["mcp"]
    }
  }
}
```

#### Cursor

在 `.cursor/mcp.json` 中：

```json
{
  "mcpServers": {
    "steward": {
      "command": "steward",
      "args": ["mcp"]
    }
  }
}
```

#### 其他 Agent

任何支持 MCP 的 Agent 都可以通过上述方式连接。如果你的 Agent 不支持 MCP，可以通过 REST API 调用 steward（见「REST API」章节）。

---

## 核心概念

### 1. 记忆（Memory）

steward 帮你的 Agent 记住跨会话的重要信息。

**记忆不是聊天记录的复制品。** 记忆是从对话中提炼出的、有价值的信息片段——一个决策、一条经验、一个偏好、一个待办事项。

```bash
# Agent 在对话中记住了一个决策
steward memory add --type decision \
  --content "项目 steward 的主存储选型确定为 SQLite + FTS5 + sqlite-vec" \
  --tags "steward,存储,决策"

# 搜索相关记忆
steward memory search "存储选型"

# 查看最近记忆
steward memory list --last 10
```

记忆有 6 种类型：

| 类型 | 说明 | 举例 |
|------|------|------|
| `decision` | 做过的决策 | "用 Rust 写 steward" |
| `task` | 任务及其状态 | "steward MVP 开发中：已完成存储层" |
| `experience` | 经验教训 | "SQLite 在 WAL 模式下不能跨 NFS" |
| `preference` | 用户偏好 | "用户偏好中文回复、北京时区" |
| `fact` | 事实信息 | "本机 PostgreSQL 数据目录在 /data/pg16" |
| `note` | 备注 | "待确认：steward 是否需要支持 Windows" |

#### 记忆写入策略：谁来决定"这段对话值得记住"？

steward 不会自动监听你和 Agent 的对话。记忆的写入有三种方式，推荐混合使用：

**方式 A：Agent 主动写入（推荐，实时）**

在 Agent 的 system prompt 里加一条规则（见下方「System Prompt 集成」），让 Agent 在遇到以下信号时主动调用 `steward_memory_add`：
- 做了一个技术决策
- 完成了一个任务步骤或整个任务
- 踩了一个坑（经验教训）
- 发现了用户的偏好或习惯
- 发现了一个重要事实

这是最精准的方式，因为 Agent 在对话中最清楚哪些信息有价值。

**方式 B：对话结束后批量提取（补充）**

```bash
# 将一段 Agent 对话的 transcript 喂给 steward，自动提取记忆
steward memory extract --from /path/to/transcript.jsonl

# steward 使用本地 Ollama LLM 分析 transcript，提取决策/任务/经验/偏好
# 提取结果先进入 "pending review" 状态，用户可确认或修改后激活
```

这种方式适合补漏——Agent 忘了记的，事后补。

**方式 C：人工手动写入**

```bash
steward memory add --type experience \
  --content "SQLite FTS5 的 unicode61 tokenizer 对中文分词效果不好，需要预处理" \
  --tags "sqlite,fts5,中文"
```

三种方式不冲突，写入时 steward 自动去重（基于语义相似度 > 0.92 的自动合并建议）。

### 2. 资产（Assets）

**资产 = 你机器上的一切有价值的东西。** 文件、目录、脚本、项目、数据集——它们不在 Agent 的上下文里，但 Agent 需要知道它们的存在。

```bash
# 注册一个项目目录
steward asset register /root/new/work/agent-steward \
  --type project \
  --desc "Agent Host Steward 产品设计工程" \
  --tags "product,design,steward"

# 注册一个常用脚本
steward asset register /root/new/shared_proj/earnings_scan/scan.sh \
  --type script \
  --desc "SEC EDGAR 财报扫描脚本" \
  --tags "finance,sec,scanner"

# 搜索资产
steward asset search "财报扫描"

# 扫描目录，自动发现资产
steward asset scan /root/new/
```

### 3. 环境（Environment）

Agent 经常需要知道"这台机器上跑着什么"，但每次都要手动告诉它。steward 自动感知。

```bash
# 查看完整环境报告
steward env show

# 输出类似:
# ═══ System ═══
# OS:       Ubuntu 22.04 LTS (x86_64)
# Kernel:   6.5.0-44-generic
# CPU:      AMD EPYC 7763 (128 cores)
# Memory:   251.5 GB (used: 45.2 GB, 18%)
# Disk:     /: 1.8TB total, 892GB used (49%)
#
# ═══ Services ═══
# PostgreSQL 16.4    :5432   running   data: /data/pg16
# Redis 7.2.5        :6379   running   
# Ollama 0.4.7       :11434  running   models: nomic-embed-text, qwen2.5
# Docker 27.3.1      socket  running   containers: 3 running
#
# ═══ Cron Jobs ═══
# hermes: 12 active cron jobs (next: daily-finance at 06:00)
# system: 5 crontab entries
#
# ═══ Network ═══
# eth0: 10.0.2.15/24
# Proxy: SOCKS5 127.0.0.1:13659 (active)

# 查看特定服务详情
steward env service postgresql
# → version, port, data dir, config, connections, databases, size

# 拍一个环境快照
steward env snapshot
```

### 4. 变更（Changes）— P1

跟踪你机器上发生了什么变化。当 Agent 跨天工作时，它能看到"昨天到今天之间发生了什么"。

```bash
# 查看最近变更
steward changes list --since "24h"

# 输出:
# 2026-11-15 09:30  file_modified  /root/new/work/agent-steward/03-feature-spec.md
# 2026-11-15 08:00  service_restart PostgreSQL (auto-vacuum triggered)
# 2026-11-14 23:00  cron_output    daily-finance → /root/new/cron/output/...
# 2026-11-14 22:30  memory_added   "steward MVP 功能范围确定"
```

### 5. 关联（Links）— P2

以上四个维度不是孤立的，steward 自动发现它们之间的关联。

```
记忆 "steward 选型用 SQLite"
  ├── 关联资产: /root/new/work/agent-steward/05-architecture.md
  ├── 关联环境: 本机 SQLite 版本 3.45.1
  └── 关联变更: 2026-11-15 写入此决策
```

---

## 核心命令：steward brief

**这是 Agent 最常用的一个命令。** 每次新会话开始时，Agent 调用一次 `steward brief`，就能拿到你的完整上下文摘要。

```bash
steward brief
```

输出：

```markdown
# Steward Brief — 2026-11-15 10:30

## 最近记忆 (10)
- [decision] 项目 steward 主存储选型: SQLite + FTS5 + sqlite-vec (11-15)
- [task] steward 产品设计: 01-03 完成, 04-07 进行中 (11-15)
- [experience] SQLite WAL 模式下不支持跨 NFS 挂载 (11-14)
- ...

## 环境概要
- OS: Ubuntu 22.04 · CPU: 128c · Mem: 252GB (18%)
- Services: PostgreSQL 16.4, Redis 7.2, Ollama 0.4.7, Docker 27.3
- Ollama Models: nomic-embed-text, qwen2.5

## 活跃项目 (3)
- /root/new/work/agent-steward/ — Agent Host Steward 产品设计 (last: 10min ago)
- /root/new/shared_proj/earnings_scan/ — SEC EDGAR 扫描 (last: 2d ago)
- /root/new/src/postgres/ — PostgreSQL 源码 (last: 3d ago)

## 最近变更 (24h)
- 03-feature-spec.md 写入完成
- daily-finance cron 正常产出
- PostgreSQL auto-vacuum 触发一次
```

`steward brief` 是可定制的：

```bash
# 只看记忆
steward brief --section memory

# 带关键词过滤
steward brief --focus "steward"

# 详细模式
steward brief --verbose
```

---

## MCP 工具列表

连接 steward 后，Agent 可用的 MCP 工具：

| 工具名 | 功能 | 参数 |
|--------|------|------|
| `steward_brief` | 获取上下文摘要 | section?, focus?, verbose? |
| `steward_memory_add` | 写入记忆 | type, content, tags?, summary? |
| `steward_memory_search` | 搜索记忆 | query, type?, limit?, time_range? |
| `steward_memory_list` | 列出记忆 | last?, type?, status? |
| `steward_memory_update` | 更新记忆 | id, content?, status?, tags? |
| `steward_asset_register` | 注册资产 | path, type, desc?, tags? |
| `steward_asset_search` | 搜索资产 | query, type?, tags? |
| `steward_asset_scan` | 扫描目录 | path, depth? |
| `steward_env_show` | 完整环境 | section? |
| `steward_env_service` | 服务详情 | name |
| `steward_env_snapshot` | 拍快照 | — |
| `steward_search` | 跨维度搜索 | query, scope?, limit? |

---

## REST API

所有 MCP 工具也可通过 REST 调用。

```bash
# 认证: 使用 ~/.steward/token 中的 bearer token
TOKEN=$(cat ~/.steward/token)

# 获取 brief
curl -H "Authorization: Bearer $TOKEN" \
     http://127.0.0.1:9527/api/v1/brief

# 搜索记忆
curl -H "Authorization: Bearer $TOKEN" \
     "http://127.0.0.1:9527/api/v1/memory/search?q=存储选型"

# 写入记忆
curl -X POST \
     -H "Authorization: Bearer $TOKEN" \
     -H "Content-Type: application/json" \
     -d '{"type":"decision","content":"选用 Rust 作为开发语言","tags":["steward","语言"]}' \
     http://127.0.0.1:9527/api/v1/memory
```

---

## 配置

配置文件位于 `~/.steward/config.toml`：

```toml
[general]
# 数据库路径（默认 ~/.steward/steward.db）
db_path = "~/.steward/steward.db"

# 日志级别
log_level = "info"

# REST API 端口
api_port = 9527

# REST API 绑定地址（默认仅本地）
api_bind = "127.0.0.1"

[embedding]
# Ollama 地址
ollama_url = "http://localhost:11434"

# Embedding 模型
model = "nomic-embed-text"

# 向量维度
dimensions = 768

# Ollama 不可用时降级为纯 FTS 模式
fallback_fts_only = true

[memory]
# 记忆权重衰减周期（天）
decay_period_days = 30

# 衰减系数（每周期乘以此系数）
decay_factor = 0.9

# 记忆上限（超出后建议归档）
max_active = 100000

[assets]
# 扫描忽略的模式
ignore_patterns = [
  ".git", "node_modules", "__pycache__",
  ".steward", ".venv", "target", "build"
]

# 自动扫描的目录
auto_scan_dirs = []

[watch]
# 文件监听的目录
watch_dirs = []

# 防抖时间（毫秒）
debounce_ms = 300

[scheduler]
# 环境快照间隔（小时）
snapshot_interval_hours = 24

# 自动备份保留天数
backup_retention_days = 7

[search]
# 混合搜索中 FTS 的权重（0-1，1=纯FTS，0=纯语义）
fts_weight = 0.5

# 默认搜索结果数
default_limit = 20

# RRF k 参数
rrf_k = 60
```

---

## 日常运维

```bash
# 查看 steward 状态
steward status
# → Daemon: running (PID 42381, uptime 3d 7h)
# → Database: 45.2 MB (8,234 memories, 12,456 assets)
# → Last snapshot: 2026-11-15 04:00

# 停止
steward stop

# 重启
steward restart

# 查看日志
steward logs
steward logs --follow

# 数据库维护
steward db vacuum     # 回收空间
steward db stats      # 查看统计
steward db backup     # 手动备份

# 数据导出
steward export --format json --output ~/steward-backup.json

# 健康检查
steward health
# → Database: OK
# → Ollama: OK (nomic-embed-text loaded)
# → Embedding backlog: 0 pending
# → Disk: OK (892GB free)
```

---

## 故障排查

### steward 无法启动

```bash
# 检查端口是否被占用
ss -tlnp | grep 9527

# 检查日志
cat ~/.steward/logs/steward.log | tail -50

# 数据库损坏修复
steward db repair
```

### Agent 连接不上

```bash
# 测试 MCP 连接
echo '{"jsonrpc":"2.0","method":"initialize","id":1}' | steward mcp

# 测试 REST
curl http://127.0.0.1:9527/health
```

### Embedding 不可用

```bash
# 检查 Ollama
curl http://localhost:11434/api/tags

# 查看 pending embedding 数量
steward db stats | grep pending

# 手动触发 embedding 补充
steward db reembed
```

---

## Agent 集成最佳实践

### 推荐使用模式

连上 steward 后，Agent 的典型工作流：

1. **新会话开始** → 调用 `steward_brief` 获取上下文，立即知道"我记得什么 + 我有什么 + 环境是什么 + 最近发生了什么"
2. **对话中做了决策** → 调用 `steward_memory_add --type decision` 记录
3. **踩坑了** → 调用 `steward_memory_add --type experience` 记录经验教训
4. **需要找文件/脚本** → 调用 `steward_asset_search` 而不是让用户手动翻目录
5. **需要知道环境** → 调用 `steward_env_show` 或 `steward_env_service`
6. **任务做到一半要中断** → 调用 `steward_memory_add --type task` 保存进度
7. **用户要求"继续之前的任务"** → 调用 `steward_memory_search --type task --status in_progress`

### System Prompt 集成

在 Agent 的 system prompt 里加入以下片段，让 Agent 知道如何使用 steward：

```
## 环境大管家 (steward)

你连接了一个名为 steward 的本地环境大管家服务，它为你提供跨会话持久化能力。

### 新会话必做
每次会话开始时，先调用 steward_brief 获取上下文摘要。不要跳过这一步。

### 记忆写入规则
在对话过程中，当出现以下信号时，主动调用 steward_memory_add 记录：
- 做了技术决策 → type=decision
- 完成任务步骤或任务结束 → type=task
- 踩坑/经验教训 → type=experience
- 发现用户偏好 → type=preference
- 重要事实信息 → type=fact
- 其他值得记住的 → type=note
不要记录琐碎的对话内容，只记录有跨会话复用价值的信息。

### 资产利用
需要找文件、脚本、项目时，先用 steward_asset_search 查找，
不要直接问用户"文件在哪"。

### 环境感知
需要知道系统环境、运行的服务、安装的软件时，
用 steward_env_show 或 steward_env_service，不要手动执行 shell 命令来收集。
```

### MCP 工具数量控制

steward 注册了 12 个 MCP 工具，对于上下文窗口有限的模型，可以精简为 3 个核心工具：

| 工具 | 必要性 | 理由 |
|------|--------|------|
| `steward_brief` | **必须** | 新会话第一调用 |
| `steward_memory_add` | **必须** | 写入记忆 |
| `steward_search` | **必须** | 跨维度搜索（记忆+资产+环境一次查完） |

其余工具按需启用。`steward_search` 一个工具能覆盖记忆搜索+资产搜索+环境查询的大部分场景。

---

## 隐私与安全

- **全部数据存在本地**：steward 没有任何云端通信，不发送 telemetry，不自动更新
- **数据库文件加密（P2）**：可选 SQLCipher 加密
- **敏感记忆掩码**：标记 `sensitive: true` 的记忆在 API 输出中自动掩码
- **Token 认证**：REST API 需要 Bearer Token，且默认只绑定 127.0.0.1
- **文件权限**：所有 `~/.steward/` 下的文件权限默认 600

---

## 与现有工具的关系

| 项目 | 定位 | steward 的不同 |
|------|------|----------------|
| mem0 | 记忆管理 API | steward 管记忆，也管资产+环境+变更+关联 |
| Letta (MemGPT) | 自管理记忆的 Agent | steward 是基础设施，不是 Agent |
| Zep | 会话记忆存储 | steward 不存原始会话，存提炼的记忆 |
| Cognee | 知识图谱 | steward 更轻量，不需要 Neo4j |
| PowerContext | 项目上下文 | steward 是全机级别，不限于单项目 |

steward 不替代这些工具，它在更底层的位置——它们可以把 steward 当作存储后端来用。

---

*下一步: 06-api-spec.md — 完整的接口定义*
