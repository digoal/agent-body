# 03 — 功能规格（Feature Spec）

> Agent Host Steward 产品设计 · 交付件 3/7
> 版本: v1.0 · 更新: 2026-11-15

---

## 一、产品名称与定位

**产品名**: `steward` (Agent Host Steward)

**一句话定位**: 驻留在 Agent 宿主机上的环境大管家 —— 为 AI Agent 提供记忆、资产、环境、变更的统一持久化与关联检索基础设施。

**产品形态**: 本地 daemon 进程 + CLI 工具 + MCP/REST API

---

## 二、模块划分

```
steward
├── core/          # 核心引擎：存储、索引、搜索
├── memory/        # 记忆管理模块
├── assets/        # 资产管理模块
├── env/           # 环境感知模块
├── changes/       # 变更管理模块
├── links/         # 智能关联模块
├── api/           # 对外接口层（MCP + REST + CLI）
├── scheduler/     # 定时任务调度（巡检、维护、快照）
└── config/        # 配置管理
```

---

## 三、模块功能详述

### 模块 1：Memory（记忆管理）

#### 1.1 数据模型

每条记忆是一个 `MemoryEntry`：

```
MemoryEntry:
  id:            UUID
  type:          enum [decision, task, experience, preference, fact, note]
  content:       text            # 记忆正文
  summary:       text            # 一句话摘要（用于列表展示和快速匹配）
  source:        object          # 来源信息
    session_id:  string          #   会话 ID
    agent:       string          #   Agent 名称（Antigravity/Cursor/自定义）
    timestamp:   datetime        #   产生时间
  tags:          string[]        # 标签
  weight:        float [0, 1]    # 权重（影响召回优先级）
  pinned:        bool            # 是否永不衰减
  status:        enum [active, archived, outdated]
  context_id:    string          # 关联上下文 ID（用于跨维度关联）
  embedding:     vector(768)     # 语义向量
  created_at:    datetime
  updated_at:    datetime
  last_accessed: datetime        # 最后被引用的时间
  access_count:  int             # 被引用次数
```

#### 1.2 核心操作

| 操作 | 说明 | 需求来源 |
|------|------|----------|
| `memory.write` | 写入一条记忆 | FM-01 |
| `memory.search` | 语义搜索 + 关键词搜索 + 过滤 | FM-02, FM-03 |
| `memory.get` | 按 ID 获取 | FM-01 |
| `memory.update` | 更新记忆内容/标签/状态 | FM-01 |
| `memory.delete` | 软删除 | FM-01 |
| `memory.import` | 从外部会话历史批量导入 | FM-04 |
| `memory.task.create` | 创建任务记忆 | FM-10 |
| `memory.task.update` | 更新任务进度 | FM-10 |
| `memory.task.list` | 列出进行中的任务 | FM-10 |
| `memory.maintain` | 触发记忆维护（衰减/强化/合并/过时检测） | FM-06~09 |

#### 1.3 搜索策略

搜索分三路并行，结果合并排序：

1. **语义搜索**: query → embedding → 向量近邻搜索 → 按余弦距离排序
2. **关键词搜索**: query → 分词 → 全文索引匹配 → 按 BM25 排序
3. **标签/过滤**: 按 type、tags、时间范围、来源过滤

合并排序使用 **RRF (Reciprocal Rank Fusion)**，详见 05-architecture.md 第六节：

```
RRF_score(d) = α × 1/(k + rank_fts(d)) + (1-α) × 1/(k + rank_vec(d))

k = 60      (标准 RRF 常数)
α = 0.5     (FTS vs Vec 权重, 可配置)

最终分值再乘以 weight 字段，实现衰减/强化影响召回顺序。
```

#### 1.4 记忆维护策略

**衰减**: 每日执行一次
```
if not pinned and last_accessed < now - decay_period:
    weight = weight × decay_factor  # 默认 decay_factor=0.95, decay_period=7天
    if weight < archive_threshold:  # 默认 0.1
        status = archived
```

**强化**: 每次被引用时
```
on access:
    access_count += 1
    last_accessed = now
    weight = min(1.0, weight + reinforce_step)  # 默认 reinforce_step=0.05
```

**合并**: 每周执行一次
```
for each pair with cosine_similarity > 0.92:
    suggest_merge(entry_a, entry_b)  # 不自动合并，只建议
```

---

### 模块 2：Assets（资产管理）

#### 2.1 数据模型

```
AssetEntry:
  id:            UUID
  path:          string          # 文件/目录绝对路径
  type:          enum [project, script, document, dataset, config, media, other]
  name:          string          # 显示名
  size:          int             # 字节大小
  mime_type:     string          # 文件 MIME 类型
  tags:          string[]        # 标签（语言、框架、用途）
  summary:       text            # 一句话摘要
  description:   text            # 详细描述
  status:        enum [active, stale, orphan, archived]
  project_id:    UUID?           # 所属项目
  metadata:      jsonb           # 扩展元信息
    git_remote:  string?         #   Git 远程地址
    git_branch:  string?         #   当前分支
    git_status:  string?         #   clean / dirty / ahead / behind
    codegraph:   bool            #   是否有 codegraph 索引
    dependencies: string[]       #   依赖列表
  embedding:     vector(768)     # 语义向量（基于 name + summary + tags）
  created_at:    datetime        # 文件创建时间
  modified_at:   datetime        # 文件最后修改时间
  indexed_at:    datetime        # 索引时间
  accessed_at:   datetime        # 最后访问时间
  context_id:    string          # 关联上下文 ID
```

#### 2.2 核心操作

| 操作 | 说明 | 需求来源 |
|------|------|----------|
| `assets.scan` | 扫描指定目录，索引所有资产 | FA-01 |
| `assets.register` | 手动注册一个资产 | FA-05 |
| `assets.search` | 语义/关键词/过滤搜索 | FA-03, FA-04 |
| `assets.get` | 按 ID 或路径获取 | FA-01 |
| `assets.update` | 更新标签/描述/状态 | FA-02 |
| `assets.sync` | 增量同步（检测文件系统变化，更新索引） | FA-07 |
| `assets.health` | 健康检查（孤儿文件、陈旧项目、Git 状态） | FA-08~10 |
| `assets.tag` | 批量打标签 | FA-02 |

#### 2.3 自动标签策略

根据文件扩展名、目录名、文件内容自动推断标签：

```yaml
rules:
  - pattern: "*.py"  → tag: python
  - pattern: "*.rs"  → tag: rust
  - pattern: "*.md"  → tag: markdown, article
  - pattern: "*.sql" → tag: sql, database
  - pattern: "*/src/*" && has(".codegraph/") → tag: source-code, codegraph
  - pattern: has("Cargo.toml") → tag: rust-project
  - pattern: has("pyproject.toml") → tag: python-project
  - pattern: has("package.json") → tag: node-project
  - pattern: has(".git/") → tag: git-repo
  - pattern: "*/markdown/*" → tag: article
  - pattern: "*/shared_proj/*" → tag: tool, script
  - content_match: "import psycopg2|import asyncpg|pg_connect" → tag: postgresql
  - content_match: "import duckdb" → tag: duckdb
```

更高质量的标签可以调用本地 LLM（通过 Ollama）对文件摘要进行分类。

---

### 模块 3：Environment（环境感知）

#### 3.1 数据模型

```
EnvSnapshot:
  id:            UUID
  timestamp:     datetime
  system:
    os:          string          # e.g. "Ubuntu 22.04 LTS"
    kernel:      string          # e.g. "5.15.0-91-generic"
    arch:        string          # e.g. "x86_64"
    hostname:    string
    cpu:         object { cores, model, load_avg }
    memory:      object { total, used, available }
    disk:        object[] { mount, total, used, available, filesystem }
    gpu:         object[]? { model, memory_total, memory_used, driver }
  network:
    interfaces:  object[]
    proxy:       object? { http, https, socks5 }
    connectivity: object { external_reachable, latency_ms }
    dns:         string[]
  services:      ServiceInfo[]
  packages:      PackageInfo[]
  cron_jobs:     CronJobInfo[]
  env_vars:      object          # 脱敏后的环境变量

ServiceInfo:
  name:          string          # e.g. "postgresql"
  version:       string          # e.g. "17.2"
  status:        enum [running, stopped, crashed, unknown]
  port:          int?
  pid:           int?
  config_file:   string?
  data_dir:      string?
  key_params:    object?         # 服务特定的关键参数
  uptime:        duration?
  resource_usage: object? { cpu_pct, mem_mb }

PackageInfo:
  manager:       enum [apt, pip, npm, cargo, brew, snap, flatpak]
  name:          string
  version:       string
  installed_at:  datetime?

CronJobInfo:
  source:        enum [system_cron, hermes, custom]
  id:            string
  schedule:      string          # cron 表达式
  command:       string
  status:        enum [active, paused, failed]
  last_run:      datetime?
  next_run:      datetime?
```

#### 3.2 核心操作

| 操作 | 说明 | 需求来源 |
|------|------|----------|
| `env.snapshot` | 生成当前完整环境快照 | FE-06 |
| `env.system` | 获取系统信息（OS/CPU/内存/磁盘） | FE-01 |
| `env.services` | 列出所有服务及状态 | FE-02 |
| `env.service(name)` | 获取特定服务详情 | FE-03 |
| `env.packages` | 列出已安装包 | FE-02 |
| `env.network` | 获取网络环境 | FE-04 |
| `env.vars` | 获取环境变量（脱敏） | FE-05 |
| `env.crons` | 列出定时任务 | FE-11 |
| `env.diff(snapshot_a, snapshot_b)` | 两个快照之间的差异 | FE-07 |
| `env.health` | 健康检查（资源告警、服务存活） | FE-08, FE-09 |

#### 3.3 服务详情采集策略

针对常见服务定义专用采集器：

```yaml
service_collectors:
  postgresql:
    detect: "pg_isready || systemctl status postgresql"
    version: "pg_config --version"
    config: "SHOW config_file"
    data_dir: "SHOW data_directory"
    key_params: ["shared_buffers", "work_mem", "effective_cache_size",
                 "max_connections", "wal_level", "max_wal_size"]
    extensions: "SELECT * FROM pg_available_extensions WHERE installed_version IS NOT NULL"

  redis:
    detect: "redis-cli ping"
    version: "redis-cli INFO server | grep redis_version"
    config: "redis-cli CONFIG GET *"
    key_params: ["maxmemory", "maxmemory-policy", "save"]

  ollama:
    detect: "curl -s http://localhost:11434/api/version"
    version: "from API response"
    models: "curl -s http://localhost:11434/api/tags"

  docker:
    detect: "docker info"
    containers: "docker ps --format json"
    images: "docker images --format json"
```

---

### 模块 4：Changes（变更管理）

#### 4.1 数据模型

```
ChangeEntry:
  id:            UUID
  timestamp:     datetime
  type:          enum [file_create, file_modify, file_delete,
                       package_install, package_upgrade, package_remove,
                       service_start, service_stop, service_crash, service_config_change,
                       env_var_change, cron_change, network_change, resource_alert]
  detail:        object          # 类型相关的详细信息
    path:        string?         #   文件路径
    from_value:  string?         #   旧值
    to_value:    string?         #   新值
    reason:      string?         #   变更原因（如果可推断）
  severity:      enum [info, warning, critical]
  affected:      string[]        # 受影响的资产 ID 列表
  source:        string          # 变更来源（user / system / agent / cron）
  context_id:    string?         # 关联上下文
```

#### 4.2 核心操作

| 操作 | 说明 | 需求来源 |
|------|------|----------|
| `changes.watch` | 启动/停止文件系统监听 | FC-01 |
| `changes.scan` | 主动扫描软件包/服务变更 | FC-02, FC-03 |
| `changes.log` | 手动记录一条变更 | FC-04 |
| `changes.query` | 按时间范围/类型/严重度查询变更日志 | FC-04 |
| `changes.since(session_id)` | 获取上次会话以来的变更 | FC-05 |
| `changes.summary(since, until)` | 生成变更摘要 | FC-05 |
| `changes.impact(change_id)` | 分析变更影响 | FC-06 |
| `changes.archive` | 归档历史变更日志 | FC-07 |
| `changes.subscribe(pattern)` | 订阅特定类型的变更通知 | FC-08 |

---

### 模块 5：Links（智能关联）

#### 5.1 数据模型

```
LinkEntry:
  id:            UUID
  source:        NodeRef         # 起点
  target:        NodeRef         # 终点
  relation:      enum [depends_on, created_by, related_to, part_of,
                       writes_to, reads_from, configured_by, deployed_on,
                       supersedes, contradicts]
  confidence:    float [0, 1]    # 关联置信度
  auto:          bool            # 是否自动发现的
  evidence:      text?           # 关联的证据（e.g. "import psycopg2 at line 12"）
  created_at:    datetime
  verified:      bool            # 是否被人工验证

NodeRef:
  domain:        enum [memory, asset, service, package, cron]
  id:            UUID            # 对应域中的实体 ID
  label:         string          # 显示标签
```

#### 5.2 核心操作

| 操作 | 说明 | 需求来源 |
|------|------|----------|
| `links.add` | 添加关联 | FL-03 |
| `links.remove` | 删除关联 | FL-03 |
| `links.query(node, depth)` | 查询节点的关联图 | FL-04 |
| `links.dependents(node)` | 查询依赖于此节点的所有实体 | FL-04 |
| `links.dependencies(node)` | 查询此节点依赖的所有实体 | FL-04 |
| `links.path(from, to)` | 查找两个节点之间的关联路径 | FL-04 |
| `links.visualize(node, depth)` | 生成关联图（Mermaid/SVG） | FL-05 |
| `links.auto_discover` | 自动发现关联关系 | FL-01, FL-02 |
| `links.validate` | 校验关联一致性（断裂链接检测） | FL-06 |

---

### 模块 6：API 层

#### 6.1 三种接口形态

| 接口 | 协议 | 适用场景 |
|------|------|----------|
| MCP | Model Context Protocol over stdio/SSE | Agent 框架直接调用（首选） |
| REST | HTTP JSON over Unix socket (默认) 或 TCP | 脚本、自动化、第三方集成 |
| CLI | `steward` 命令行工具 | 人工交互、调试、cron 脚本 |

三种接口是同一套后端逻辑的不同入口，保证行为一致。

#### 6.2 MCP 工具注册

```yaml
mcp_tools:
  # Memory
  - steward_memory_add
  - steward_memory_search
  - steward_memory_task_create
  - steward_memory_task_update
  - steward_memory_task_list

  # Assets
  - steward_asset_search
  - steward_asset_register
  - steward_asset_get

  # Environment
  - steward_env_system
  - steward_env_services
  - steward_env_service
  - steward_env_packages
  - steward_env_network
  - steward_env_crons
  - steward_env_snapshot

  # Changes
  - steward_changes_since
  - steward_changes_summary
  - steward_changes_query

  # Links
  - steward_links_query
  - steward_links_dependents

  # Meta
  - steward_status           # 大管家自身状态
  - steward_brief    # 一键获取当前上下文简报（综合记忆+资产+环境+变更）
```

其中 `steward_context_brief` 是最重要的 MCP 工具 —— Agent 新建会话时调用一次，就能拿到完整的上下文简报，包含：最近任务进展、环境状态、上次会话以来的变更、需要注意的事项。

#### 6.3 CLI 命令结构

```bash
steward                          # 启动 daemon
steward status                   # 查看 daemon 状态
steward stop                     # 停止 daemon

# Memory
steward memory search "DuckDB 文章"
steward memory write --type decision "选用 FastAPI 做 API 层"
steward memory tasks              # 列出进行中的任务
steward memory maintain           # 手动触发记忆维护

# Assets
steward assets scan /root/new/    # 扫描目录
steward assets search "财报扫描脚本"
steward assets health             # 健康检查
steward assets register /path --tags "tool,python" --desc "xxx"

# Environment
steward env snapshot              # 生成快照
steward env services              # 列出服务
steward env service postgresql    # PG 详情
steward env diff --last           # 与上次快照对比

# Changes
steward changes since --last-session
steward changes summary --days 7
steward changes watch start       # 启动文件监听
steward changes watch stop

# Links
steward links query /root/new/shared_proj/earnings_scan/ --depth 2
steward links visualize /root/new/src/duckdb/ --format mermaid

# Meta
steward brief                     # 一键上下文简报
steward config                    # 查看/编辑配置
steward export                    # 导出所有数据
```

---

### 模块 7：Scheduler（内部调度器）

大管家自身的定时任务：

| 任务 | 默认频率 | 说明 |
|------|----------|------|
| 环境快照 | 每天 03:00 | 生成并存储环境快照 |
| 记忆维护 | 每天 03:30 | 衰减、强化、合并建议 |
| 资产健康检查 | 每周日 04:00 | 孤儿文件、陈旧项目、Git 状态 |
| 变更日志归档 | 每周日 05:00 | 压缩归档历史变更 |
| 包变更扫描 | 每 5 分钟 | 检测 pip/apt/npm 变化 |
| 服务状态检查 | 每 1 分钟 | 检测服务存活 |
| 关联图更新 | 每天 04:00 | 自动发现新关联 |
| 磁盘巡检 | 每天 04:30 | 磁盘使用趋势分析 |

---

## 四、MVP 功能清单（P0 范围）

MVP 只做下面这些，保证核心价值可验证：

| 模块 | 包含功能 | 不包含 |
|------|----------|--------|
| Memory | 写入、搜索（语义+关键词）、任务记忆 | 导入、衰减强化、合并、过时检测 |
| Assets | 目录扫描、标签、搜索、项目注册、脚本注册 | 增量同步、孤儿检测、Git 状态、并发协调 |
| Environment | 系统信息、服务清单+详情、网络、环境变量、cron列表、快照 | 快照对比、健康检查、告警、GPU |
| Changes | — | 全部推到 P1 |
| Links | — | 全部推到 P2 |
| API | MCP + CLI | REST (P1) |
| Scheduler | 环境快照 (每天) | 其他定时任务 |

**MVP 的核心价值主张**: Agent 终于能在新会话中一次调用 `steward brief`，就知道"我记得什么 + 我有什么 + 环境是什么"。

---

*下一步: 04-product-manual.md — 面向用户的完整使用手册*
