# 03 — 功能规格（Feature Spec）· 三模块解耦版

> Agent Body 产品设计 · 交付件 3/7  
> 版本: v2.0 (模块化体系升级) · 更新: 2026-11-15  
> 战略原则: **事实图谱为基石护城河 · 偏好规约为协作灵魂 · 记忆模块开放插拔成熟生态**

---

## 一、产品名称与定位

**产品名**: `agent-body` (Agent Body / Agent 本体)

**核心使命**: 驻留在 AI Agent 宿主机中的轻量级系统认知底座。它彻底消除了 Agent 对宿主机物理世界的“全盲”状态，建立机器真实世界的客观事实图谱；同时规约用户的代码与文件放置偏好；并通过可插拔标准无缝集成成熟的跨会话记忆生态。

**产品形态**: 单二进制守护进程 (Rust 编写) + CLI 工具 + 标准 MCP stdio/SSE 接口 + Unix Socket

---

## 二、三模块核心划分与边界

```
agent-body
│
├── 1. 事实图谱模块 (Host Reality Graph - 核心护城河)
│   ├── capability/       # 软件与 CLI 工具能力语义注解库 (如 pdftotext, ffmpeg, jq)
│   ├── process_net/      # /proc 内核级进程与网络端口动态映射 (排障/防撞)
│   ├── automation/       # systemd 依赖网络 + crontab 定时偷跑感知
│   ├── assets_code/      # 源码仓库发现 (CodeGraph状态) + 权威资料库编目
│   └── changes_audit/    # 文件实时 inotify 变更流与逆向爆炸半径推演
│
├── 2. 偏好与规约模块 (Preference & Governance - 协作灵魂)
│   ├── placement/        # 资产放置规则引擎 (路径推荐/命名规范/安全边界)
│   ├── conventions/      # 用户操作习惯与工程规约 (首选工具/数据真实性/时区)
│   └── rule_learner/     # 纠偏自演化学习器 (从用户人工纠错中归纳新规则)
│
└── 3. 记忆与上下文模块 (Memory & Context - 开放可插拔生态)
    ├── spi/              # 通用 MemoryProvider 特征抽象 (Search/Store/Context/Evict)
    ├── builtin/          # 内置开箱即用引擎 (SQLite FTS5 + 本地向量混合检索)
    └── adapters/         # 成熟开源项目桥接器 (Mem0 / Letta / PowerContext / Zep)
```

---

## 三、模块功能详述

### 模块 1：宿主机事实图谱 (Host Reality Graph) —— 【核心护城河】

#### 1.1 软件与 CLI 工具能力语义注解 (Capability Graph)
- **业务价值**：当 Agent 面临任务时，无需盲目写脚本或反复 pip 安装失败，直接获知宿主机已就绪的最佳 CLI 工具和参数。
- **核心功能**：
  1. 自动化扫描系统 `$PATH` 下的可执行二进制文件；
  2. 匹配内置的“系统工具能力知识库”，为工具打上功能语义标签（如 `pdf-text-extract`, `image-convert`, `video-transcode`, `json-query`, `fast-search`, `network-sniff`）；
  3. 提供最佳实践命令行模板与避坑提示（例如 `pdftotext -layout input.pdf output.txt` 保持原样排版）。
- **查询接口契约**：
  - `body_fact_query(type="capability", intent="<自然语言意图>")`
  - 返回：推荐工具名、二进制物理绝对路径、已安装版本号、推荐调用参数、同类备选工具列表。

#### 1.2 运行时进程与网络拓扑 (Process & Net Graph)
- **业务价值**：起服务前防撞，端口被占时秒级排障，识别后台孤儿任务。
- **核心功能**：
  1. 直接读取 `/proc` 文件系统与内核网络套接字（零子进程 fork 性能损耗）；
  2. 自动过滤内核底噪（`kworker`, `systemd-journald` 等），专注应用级工作负载；
  3. 端口全景映射：提供端口占用状态、PID、可执行文件完整命令行、工作目录、运行时间、启动者。
- **查询接口契约**：
  - `body_port_check(port=<int>)`
  - `body_env_processes(filter="<keyword>", include_system=false)`

#### 1.3 系统自动化拓扑 (Systemd & Cron Topology)
- **业务价值**：消除“后台神秘脚本干扰”，防止改动通用配置导致后台生产服务挂掉。
- **核心功能**：
  1. 解析 `/etc/systemd/system/` 单元依赖，识别服务依赖的 `EnvironmentFile`、`ConfigurationFile` 与工作目录；
  2. 解析全系统及当前用户的 `crontab` 调度，掌握定时任务执行周期、运行脚本与日志输出路径；
  3. 跨实体关联：输入某配置文件路径，反向推导受其影响的 systemd 服务或 cron 脚本。

#### 1.4 本地源码库与权威资料编目 (Code & Knowledge Graph)
- **业务价值**：Agent 开工前了解本地已有哪些宝藏代码与资料，不重复造轮子、不盲目外网乱搜。
- **核心功能**：
  1. 源码发现：自动发现宿主机所有 Git 仓库（如 `/root/new/src`、`/root/new/work`），识别分支与最近提交；
  2. CodeGraph 就绪状态检测：标明仓库根目录是否存在 `.codegraph`，指导 Agent 优先使用 `codegraph mcp` 探索代码；
  3. 资料库发现：编目本地权威资料（如 `/root/new/note/` 权威网站、离线技术手册、常用脚本库 `/root/new/shared_proj`）。

#### 1.5 物理文件动态审计与逆向爆炸半径 (Changes & Impact)
- **核心功能**：
  1. inotify 监听核心代码与配置目录，带 300ms 智能防抖；
  2. 审计近期变更：过去 1 小时/24 小时哪些文件被改动；
  3. 爆炸半径分析（`body_links_impact`）：输入某公共模块，多跳追溯下游引用依赖链。

---

### 模块 2：用户偏好与规约引擎 (Preference & Governance) —— 【协作灵魂】

#### 2.1 资产放置规则引擎 (Placement Engine)
- **业务价值**：解决“用户偏好把文件放哪”，杜绝 Agent 乱建文件、地皮污染。
- **核心功能**：
  1. **意图匹配与推荐**：根据 Agent 提出的资产类型（`script`, `skill`, `project`, `tmp`, `doc`）和自然语言意图，精准推荐基准物理路径；
  2. **命名范式规约**：强制遵循项目命名规范（蛇形命名、小写下划线、版本号格式等）；
  3. **安全隔离准则**：临时实验必须放 `/root/new/tmp/` 并具备 TTL 回收机制；敏感凭据严格阻断进入公共资产。
- **接口契约**：
  - `body_placement_suggest(type="<type>", name="<name>", intent="<intent>")`

#### 2.2 用户工程与工作流习惯 (Conventions)
- **工具链偏好**：例如在已有 `.codegraph` 项目中优先使用 `codegraph` 工具而非 `grep`；
- **时区与风格**：强制使用北京时间，技术写作严格校验数据真实性；
- **排版标准**：SVG 必须独立落盘为文件再在 Markdown 中引用。

#### 2.3 纠偏自演化学习机制 (Feedback Learner)
- **核心机制**：
  - 当检测到 Agent 产生的文件被人工移动或覆盖时，触发纠偏分析；
  - 提炼新偏好规则并入库，随着协作越深入，Agent 本体越懂用户的组织习惯。

---

### 模块 3：记忆与上下文适配层 (Memory & Context) —— 【开放可插拔生态】

#### 3.1 战略原则：生态共赢、绝不重复造轮子
市面上已有成熟的开源记忆项目（Mem0、Letta、Zep、PowerContext 等），Agent 本体不把精力浪费在重新发明认知心理学长程记忆模型上，而是**定义开放的通用接口，做统一的接入中枢**。

#### 3.2 抽象接口 SPI (Service Provider Interface)
- `search(query, limit, filter)`: 跨会话搜索过往决策、排障经验、用户画像；
- `store(item)`: 沉淀关键决策、工单总结、任务避坑记录；
- `get_context(session_id)`: 一键拉取当前 Agent 所需的会话级背景信息；
- `evict(criteria)`: 根据时间或权重衰减淘汰过期噪音。

#### 3.3 双模支持与生态适配器
1. **内置轻量 Provider (Built-in Local Provider)**：
   - 基于 SQLite FTS5 全文检索 + 本地 Embedding（Ollama / 纯文本降级）；
   - 零网络开销、单机内嵌，开箱即用。
2. **外部成熟开源适配器 (External Adapters)**：
   - **Mem0 Adapter**: 桥接 Mem0 REST/SDK，享受工业级事实抽取与用户画像追踪；
   - **Letta Adapter**: 桥接 Letta 核心记忆系统；
   - **PowerContext Adapter**: 桥接针对代码库与工程上下文深度优化的 PowerContext 记忆体系。

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
  type:          enum [directory, project, script, document, dataset, config, media, other]
  name:          string          # 显示名
  size:          int             # 字节大小
  mime_type:     string          # 文件 MIME 类型
  owner:         string          # 归属主体 (e.g. "user:digoal", "agent:main", "system")
  security_level: enum [public, internal, confidential, secret] # 保密级别
  scan_tier:     enum [deep, shallow, restricted] # 扫描与索引颗粒度层级
  tags:          string[]        # 标签（语言、框架、用途）
  summary:       text            # 一句话摘要
  description:   text            # 详细文本描述
  desc_kv:       jsonb           # 结构化语义描述键值 (例如: {"purpose": "...", "ttl": "...", "upstream": "..."})
  status:        enum [active, stale, orphan, archived]
  project_id:    UUID?           # 所属项目
  metadata:      jsonb           # 扩展元信息
    git_remote:  string?         #   Git 远程地址
    git_branch:  string?         #   当前分支
    git_status:  string?         #   clean / dirty / ahead / behind
    codegraph:   bool            #   是否有 codegraph 索引
    dependencies: string[]       #   依赖列表
  embedding:     vector(768)     # 语义向量（基于 name + summary + tags + desc_kv，仅 deep tier 生成）
  created_at:    datetime        # 文件创建时间
  modified_at:   datetime        # 文件最后修改时间
  indexed_at:    datetime        # 索引时间
  accessed_at:   datetime        # 最后访问时间
  context_id:    string          # 关联上下文 ID
```

#### 2.2 核心操作

| 操作 | 说明 | 需求来源 |
|------|------|----------|
| `assets.scan` | 依据颗粒度策略扫描指定目录，索引资产 | FA-01, FA-12 |
| `assets.register` | 手动注册一个资产（含 owner、密级、desc_kv） | FA-05, FA-13 |
| `assets.search` | 语义/关键词/标量过滤（支持密级与 owner 过滤） | FA-03, FA-04, FA-13 |
| `assets.get` | 按 ID 或路径获取 | FA-01 |
| `assets.update` | 更新标签/描述/desc_kv/密级/状态 | FA-02, FA-13 |
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

#### 2.4 白名单/黑名单与颗粒度分级策略（Granularity Policy）

Agent 本体纳管宿主机一切对象，**坚决杜绝“一股脑全生成对象元数据”**：

```
                    ┌─────────────────────────┐
                    │      扫描路径入口       │
                    └────────────┬────────────┘
                                 │ 匹配 scan_policies
          ┌──────────────────────┼──────────────────────┐
          ▼                      ▼                      ▼
  【Deep Tier (白名单)】   【Shallow Tier (黑名单)】  【Restricted Tier】
   (如 shared_proj, work)   (如 node_modules, cache)   (如 src/ 开源克隆)
          │                      │                      │
   细到最深层递归遍历      ★ 只到当前目录层级 ★        浅层递归 (max_depth=2)
   提取叶子文件与目录      记录目录本身作为一个对象      仅索引项目入口与README
   生成全部元数据:         不深入递归、不下钻解析        生成基础标量与关键词FTS
   - 基础标量              仅记录目录标量:               大文件跳过向量化
   - desc_kv 深度解析      - path, size, file_count      - 避免打爆存储
   - 全文检索 FTS5/BM25    - owner, security_level
   - 语义向量 Embedding    ★ 严禁全文、向量与图谱 ★
   - 依赖关系拓扑 (Links)  ★ 绝不扫爆数据库 ★
```

| 级别 (Tier) | 行为规则 | 深度 (Depth) | 索引生成 | 典型路径示例 |
|---|---|---|---|---|
| **Deep Tier (白名单)** | 核心资产库，细到最深层，全面递归 | 无限制 (-1) | 基础标量 + 全文 FTS5 + 向量 Embedding + 拓扑关联 + 深度 desc_kv | `/root/new/shared_proj/`, `/root/new/work/`, `/root/new/skills/` |
| **Shallow Tier (黑名单)** | 阻断深入递归，**只到当前目录层级**，将该目录实体本身建档 | 0 (仅当前目录) | **仅记录目录标量属性**（大小、项数、修改时间、owner、密级），**严禁全文与向量** | `**/node_modules/**`, `**/.cache/**`, `**/.git/**`, `/root/.ssh/` |
| **Restricted Tier (受限区)** | 大型外部仓库或只读数据集，浅层纳管 | 限制 2 层 | 仅根目录及 README/配置文件生成关键词索引，不做代码行向量化 | `/root/new/src/**`, `/root/new/tmp/**` |

---

### 模块 3：Placement（对象放置建议引擎）

#### 3.1 数据模型

```
PlacementRule:
  id:              UUID
  rule_name:       string          # 规则标识 (e.g. "shared_tools", "work_projects")
  pattern_type:    enum [project, script, note, tmp, dataset, src, skill, other]
  target_base_dir: string          # 建议目标基准路径 (e.g. "/root/new/shared_proj/")
  naming_pattern:  string          # 命名约定模式 (e.g. "{name}/", "YYYYMMDD_{name}.md")
  description:     text            # 规则说明与设计意图
  guideline:       text            # 附带的开发/安全规范指引
  priority:        int             # 匹配优先级 (数字越小优先级越高)
```

#### 3.2 核心操作

| 操作 | 说明 | 需求来源 |
|------|------|----------|
| `placement.suggest` | 根据对象类型、名称、意图与密级，推荐最佳落盘路径与命名规范 | FP-01, FP-03 |
| `placement.list_rules` | 查看当前生效的宿主机目录放置规范规则库 | FP-02 |
| `placement.check_conflict` | 检查推荐路径是否已有同名资产存在，评估冲突风险 | FP-04 |

#### 3.3 内置规范策略与决策逻辑

Agent 本体为 Agent 提供清晰的宿主机空间拓扑导引：

```
                           【Agent 产生新对象请求】
                                      │
            ┌─────────────────────────┼─────────────────────────┐
            ▼                         ▼                         ▼
      [长期复用脚本/程序]         [常规项目/立项工作]           [临时文件/调试日志]
            │                         │                         │
     /root/new/shared_proj/    /root/new/work/           /root/new/tmp/
            │                         │                         │
            ▼                         ▼                         ▼
      [克隆开源项目]              [AI 技能 Skill]           [重要笔记/权威列表]
            │                         │                         │
     /root/new/src/            /root/new/skills/         /root/new/note/
```

- **长期程序/工具**: 放置于 `/root/new/shared_proj/{name}/`，要求建立独立子目录，配备 README、使用示例与依赖说明；
- **常规工作项目**: 放置于 `/root/new/work/{project_name}/`；
- **临时/中间文件**: 放置于 `/root/new/tmp/`，明确生命周期为可随时清理；
- **开源克隆代码**: 放置于 `/root/new/src/{repo_name}/`，进入后优先使用 `codegraph init` 建立索引；
- **AI 技能资产**: 放置于 `/root/new/skills/{skill_name}/`，强制包含 `SKILL.md`；
- **重要笔记**: 放置于 `/root/new/note/`；
- **定时任务脚本与输出**: 放置于 `/root/new/cron/{job_name}/`；
- **对外开源精品**: 放置于 `/root/new/shared_git/{name}/`。

---

### 模块 4：Environment（环境感知）

#### 4.1 数据模型

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
  active_processes: ActiveProcessInfo[] # 活跃的动态任务与普通进程
  env_vars:      object          # 脱敏后的环境变量

ActiveProcessInfo:
  pid:           int             # 进程 PID
  name:          string          # 进程名 (e.g. "python3", "ollama")
  command:       string          # 完整启动命令
  owner:         string          # 发起者 (e.g. "agent:hermes_cron", "user:digoal")
  associated_path: string?       # 关联代码/脚本资产路径 (如 /root/new/shared_proj/xxx.py)
  cpu_pct:       float           # CPU 占用百分比
  mem_bytes:     int             # 内存常驻字节数
  gpu_allocated: int             # 占用 GPU 显存 (MB)
  listening_ports: int[]         # 监听端口
  started_at:    datetime        # 进程启动时间
  status:        enum [running, sleeping, zombie]

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

#### 4.2 核心操作

| 操作 | 说明 | 需求来源 |
|------|------|----------|
| `env.snapshot` | 生成当前完整环境快照 | FE-06 |
| `env.system` | 获取系统信息（OS/CPU/内存/磁盘） | FE-01 |
| `env.services` | 列出所有服务及状态 | FE-02 |
| `env.service(name)` | 获取特定服务详情 | FE-03 |
| `env.processes` | 列出活跃动态任务与普通进程（CPU/内存/GPU/端口/关联资产） | FE-12 |
| `env.packages` | 列出已安装包 | FE-02 |
| `env.network` | 获取网络环境 | FE-04 |
| `env.vars` | 获取环境变量（脱敏） | FE-05 |
| `env.crons` | 列出定时任务 | FE-11 |
| `env.diff(snapshot_a, snapshot_b)` | 两个快照之间的差异 | FE-07 |
| `env.health` | 健康检查（资源告警、服务存活） | FE-08, FE-09 |

#### 4.3 服务详情采集策略

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

### 模块 5：Changes（变更管理）

#### 5.1 数据模型

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

#### 5.2 核心操作

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

### 模块 6：Links（智能关联）

#### 6.1 数据模型

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

#### 6.2 核心操作

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

### 模块 7：API 层

#### 7.1 三种接口形态

| 接口 | 协议 | 适用场景 |
|------|------|----------|
| MCP | Model Context Protocol over stdio/SSE | Agent 框架直接调用（首选） |
| REST | HTTP JSON over Unix socket (默认) 或 TCP | 脚本、自动化、第三方集成 |
| CLI | `agent-body` 命令行工具 | 人工交互、调试、cron 脚本 |

三种接口是同一套后端逻辑的不同入口，保证行为一致。

#### 7.2 MCP 工具注册

```yaml
mcp_tools:
  # Placement (放置建议)
  - body_placement_suggest
  - body_placement_rules

  # Assets
  - body_asset_search
  - body_asset_register
  - body_asset_get
  - body_asset_scan

  # Environment
  - body_env_system
  - body_env_services
  - body_env_service
  - body_env_processes
  - body_env_packages
  - body_env_network
  - body_env_crons
  - body_env_snapshot

  # Memory
  - body_memory_add
  - body_memory_search
  - body_memory_task_create
  - body_memory_task_update
  - body_memory_task_list

  # Changes
  - body_changes_since
  - body_changes_summary
  - body_changes_query

  # Links
  - body_links_query
  - body_links_dependents

  # Meta
  - body_status           # Agent 本体自身状态
  - body_brief            # 一键获取当前上下文简报（综合记忆+资产+环境+变更+进程）
```

其中 `body_brief` 是最重要的 MCP 工具 —— Agent 新建会话时调用一次，就能拿到完整的上下文简报，包含：最近任务进展、环境与进程状态、上次会话以来的变更、需要注意的事项。

#### 7.3 CLI 命令结构

```bash
agent-body                          # 启动 daemon
agent-body status                   # 查看 daemon 状态
agent-body stop                     # 停止 daemon

# Placement
agent-body place suggest --type script --name sec_scraper --intent "美股抓取"
agent-body place rules              # 查看放置规范

# Assets
agent-body assets scan /root/new/   # 扫描目录 (应用白黑名单颗粒度)
agent-body assets search "财报扫描脚本"
agent-body assets health            # 健康检查
agent-body assets register /path --tags "tool,python" --desc "xxx" --owner "user:digoal" --security "internal"

# Environment
agent-body env snapshot             # 生成快照
agent-body env services             # 列出服务
agent-body env processes            # 查看活跃动态进程
agent-body env service postgresql   # PG 详情
agent-body env diff --last          # 与上次快照对比

# Memory
agent-body memory search "DuckDB 文章"
agent-body memory write --type decision "选用 FastAPI 做 API 层"
agent-body memory tasks             # 列出进行中的任务
agent-body memory maintain          # 手动触发记忆维护

# Changes
agent-body changes since --last-session
agent-body changes summary --days 7
agent-body changes watch start      # 启动文件监听
agent-body changes watch stop

# Links
agent-body links query /root/new/shared_proj/earnings_scan/ --depth 2
agent-body links visualize /root/new/src/duckdb/ --format mermaid

# Meta
agent-body brief                    # 一键上下文简报
agent-body config                   # 查看/编辑配置
agent-body export                   # 导出所有数据
```

---

### 模块 8：Scheduler（内部调度器）

Agent 本体自身的定时任务：

| 任务 | 默认频率 | 说明 |
|------|----------|------|
| 环境快照 | 每天 03:00 | 生成并存储环境快照 |
| 记忆维护 | 每天 03:30 | 衰减、强化、合并建议 |
| 资产健康检查 | 每周日 04:00 | 孤儿文件、陈旧项目、Git 状态 |
| 变更日志归档 | 每周日 05:00 | 压缩归档历史变更 |
| 包变更扫描 | 每 5 分钟 | 检测 pip/apt/npm 变化 |
| 服务与动态进程检查 | 每 1 分钟 | 检测服务与高负载任务存活 |
| 关联图更新 | 每天 04:00 | 自动发现新关联 |
| 磁盘巡检 | 每天 04:30 | 磁盘使用趋势分析 |

---

## 四、MVP 功能清单（P0 范围）

MVP 只做下面这些，保证核心价值可验证：

| 模块 | 包含功能 | 不包含 |
|------|----------|--------|
| Placement | 新对象放置建议（匹配内置规范库）、冲突检查 | 用户自定义高级动态规则DSL |
| Assets | 目录扫描（白黑名单颗粒度分级）、标签、搜索（标量+BM25+向量）、项目与脚本注册、结构化元数据（owner, security_level, desc_kv） | 孤儿检测、Git 状态、并发协调 |
| Environment | 系统信息、服务清单+详情、动态进程列表、网络、环境变量、cron列表、快照 | 快照对比、健康检查告警 |
| Memory | 写入、搜索（语义+关键词）、任务记忆 | 导入、衰减强化、合并、过时检测 |
| Changes | — | 全部推到 P1 |
| Links | — | 全部推到 P2 |
| API | MCP + CLI | REST (P1) |
| Scheduler | 环境快照 (每天) | 其他定时任务 |

**MVP 的核心价值主张**: Agent 终于能在新会话中一次调用 `agent-body brief`，就知道"我处于什么环境 + 有哪些对象与活跃任务 + 新建资产应该放哪里"。

---

*下一步: 04-product-manual.md — 面向用户的完整使用手册*
