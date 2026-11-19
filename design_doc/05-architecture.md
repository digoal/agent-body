# 05 — 架构设计（Architecture）· 三位一体架构体系

> [!NOTE]
> **历史设计基线说明**：本文档沉淀自项目 v2.0 架构重构期（2026-11-15）。当前 `v0.4.0` 完整 24 模块源码架构、61 方法存储 SPI、双轨实证评测与 Web UI 2.0 权威口径，请以 [arch/01-for-architects.md](../arch/01-for-architects.md) 与 [arch/02-for-developers.md](../arch/02-for-developers.md) 为准。

> Agent Host Steward 产品设计 · 交付件 5/7  
> 版本: v2.0 (三模块重构版) · 更新: 2026-11-15  
> 战略定位: **宿主机事实图谱为核心护城河 + 用户偏好规约为协作准则 + 开放插拔记忆拥抱成熟生态**

---

## 一、架构愿景与顶层设计哲学

在真实的宿主机操作环境中，Agent 最致命的缺陷不是跨会话失忆，而是对宿主机物理世界的“全盲”与“缺乏用户默契”。市面上有大量专注于“对话记忆”的开源项目，但几乎没有基础设施为 Agent 提供精准、低延迟、零幻觉的**宿主机物理世界数字化事实**与**用户工程偏好规约**。

大管家（Agent Host Steward）实行严格的关注点分离（Separation of Concerns），确立三大核心子系统：

1. **【事实模块：宿主机事实图谱 (Host Reality Graph)】—— 独家核心护城河**
   - **职责**：回答 **"What IS"**（客观物理世界存在什么）。
   - **特性**：持续更新、客观真理、零幻觉、纯只读语义接口。
   - **覆盖**：进程与端口动态拓扑、系统包与 CLI 工具链“能力语义注解”（如 pdftotext / ffmpeg / jq）、systemd 守护服务与配置依赖、偷跑的 cron 定时任务、本地源码库与权威资料清单、文件近期变更动态。
2. **【偏好模块：用户偏好与规约引擎 (Preference & Governance)】—— 人机协作的灵魂**
   - **职责**：回答 **"How to ACT"**（应该怎样行动与放置）。
   - **特性**：主观约束、规范引导、意图推导、纠偏自演化。
   - **覆盖**：资产放置路径（脚本放哪里、技能放哪里、临时文件放哪里）、工具链选型偏好（codegraph 优先）、命名规范、从用户纠错中自主提炼新规则。
3. **【记忆模块：跨会话记忆与经验 (Memory & Context)】—— 开放生态可插拔**
   - **职责**：回答 **"What HAPPENED"**（过去发生了什么/经验沉淀）。
   - **特性**：标准 SPI 接口、绝不重复造轮子、生态无缝桥接。
   - **覆盖**：内置轻量开箱即用引擎（Local SQLite FTS5 + Local Embedding）；支持一行配置外挂 Mem0、Letta、Zep、PowerContext 等成熟开源记忆服务。

---

## 二、整体系统架构图

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                               AI Agents / Clients                                      │
│             (Antigravity, Cursor, Claude Code, Aider, CLI, Cron Workers)               │
└───────────────────────────────────────────┬────────────────────────────────────────────┘
                                            │ MCP (stdio/SSE) / Unix Domain Socket / CLI
┌───────────────────────────────────────────▼────────────────────────────────────────────┐
│                           Agent Host Steward Core Daemon                               │
├────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                        │
│ ┌────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ 模块 1: 宿主机事实图谱 (Host Reality Graph) 【核心护城河 · 零幻觉只读体系】        │ │
│ │                                                                                    │ │
│ │  ┌───────────────────────┐ ┌───────────────────────┐ ┌──────────────────────────┐  │ │
│ │  │ 工具能力语义图谱引擎   │ │ 运行时进程与网络拓扑  │ │ 系统自动化拓扑网络       │  │ │
│ │  │ (Tool Capability Graph)│ │ (Process & Net Graph) │ │ (Systemd & Cron Topology)│  │ │
│ │  │ • PATH 工具能力注解    │ │ • /proc 精确物理扫描  │ │ • systemd 与配置依赖关系 │  │ │
│ │  │ • 功能标签 (PDF/音视频)│ │ • 端口监听与命令映射  │ │ • crontab 调度检测       │  │ │
│ │  │ • 常用最佳参数配对推荐 │ │ • 僵尸与底噪过滤      │ │ • 自动化脚本影响推演     │  │ │
│ │  └───────────────────────┘ └───────────────────────┘ └──────────────────────────┘  │ │
│ │  ┌─────────────────────────────────────────────────┐ ┌──────────────────────────┐  │ │
│ │  │ 本地源码工程与资料编目 (Code & Knowledge Graph) │ │ 文件近期动态变更审计   │  │ │
│ │  │ • Git 仓库编目 / CodeGraph 就绪状态探测         │ │ (Inotify & File Events)  │  │ │
│ │  │ • 本地文档资料库 / 权威网站与沉淀脚本索引       │ │ • 谁在何时改了什么文件   │  │ │
│ │  └─────────────────────────────────────────────────┘ └──────────────────────────┘  │ │
│ │                                                                                    │ │
│ │  ═══════════════════════════════════════════════════════════════════════════════   │ │
│ │  只读语义查询与探测接口: steward_fact_query / env_port / links_impact              │ │
│ └────────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                        │
│ ┌────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ 模块 2: 用户偏好与规约引擎 (Preference & Governance) 【协作灵魂与规约准则】        │ │
│ │                                                                                    │ │
│ │  ┌─────────────────────────────┐ ┌───────────────────────────────────────────────┐ │ │
│ │  │ 资产放置规则引擎            │ │ 用户操作习惯与工程偏好 (Conventions)         │ │ │
│ │  │ (Placement Rule Engine)     │ │ • 首选工具链 (优先使用 codegraph 而非 grep) │ │ │
│ │  │ • 基准路径推荐 (shared_proj)│ │ • 时区 / 语言 / 编码风格 / 数据真实性底线   │ │ │
│ │  │ • 命名规范与脱敏规则        │ │ • 敏感信息掩码法则                          │ │ │
│ │  └─────────────────────────────┘ └───────────────────────────────────────────────┘ │ │
│ │  ┌─────────────────────────────────────────────────────────────────────────────┐ │ │
│ │  │ 纠偏自演化器 (Rule Learner from Human Corrections): 捕获纠错并自动生成规则    │ │ │
│ │  └─────────────────────────────────────────────────────────────────────────────┘ │ │
│ └────────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                        │
│ ┌────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ 模块 3: 记忆与经验适配层 (Memory & Context) 【开放生态可插拔】                     │ │
│ │                                                                                    │ │
│ │  ┌─────────────────────────────────────────────────────────────────────────────┐ │ │
│ │  │ 通用 Memory Provider SPI (统一 Search / Add / Context 接口标准)              │ │ │
│ │  └──────────────────────────────────────┬──────────────────────────────────────┘ │ │
│ │                                         │                                          │ │
│ │      ┌──────────────────────────────────┴────────────────────────────────┐         │ │
│ │      ▼                                                                   ▼         │ │
│ │  ┌─────────────────────────┐                           ┌────────────────────────┐  │ │
│ │  │ 内置轻量引擎 (Default)  │                           │ 成熟开源桥接器(Adapters)│ │ │
│ │  │ • Local SQLite FTS5     │                           │ • Mem0 Adapter         │  │ │
│ │  │ • Local Embedding (RRF) │                           │ • Letta / Zep Adapter  │  │ │
│ │  │ • 零外部依赖单机运行    │                           │ • PowerContext Adapter │  │ │
│ │  └─────────────────────────┘                           └────────────────────────┘  │ │
│ └────────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                        │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

---

## 三、三大核心模块深度设计

### 3.1 模块一：宿主机事实图谱（Host Reality Graph）—— 核心护城河

事实图谱是物理宿主世界的数字孪生，它对 Agent 屏蔽底层 Linux 琐碎命令（如 `ps`, `netstat`, `dpkg`, `crontab`, `systemctl`），提供**零幻觉、持续更新、带能力语义的只读事实接口**。

#### (1) 工具与软件能力图谱 (Tool Capability Graph)
传统 Linux 环境下，Agent 面临的最大痛点是“不知道本机有什么工具能解决当前任务”。
- **采集与建模机制**：
  1. 扫描 `$PATH` 下的可执行文件与包管理器已装列表；
  2. 匹配**工具能力语义注解库（Capability Knowledge Base）**。大管家内置 Linux 高频工具能力词条：
     - *PDF 处理*：`pdftotext`（提取文字，推荐参数 `-layout`）、`pdftoppm`（PDF转图片）、`pdfunite`（合并）；
     - *多媒体处理*：`ffmpeg`（音视频转码、雪碧图）、`sox`（音频分析）、`imagemagick / convert`（图像变换）；
     - *文本与数据*：`jq`（JSON解析与过滤）、`ripgrep / rg`（极速正文搜索）、`pandoc`（文档格式互转）；
     - *网络与抓包*：`curl`, `httpie`, `tcpdump`, `mitmproxy`；
     - *编译与分析*：`rustc`, `cargo`, `gcc`, `gdb`, `perf`；
  3. 运行时动态探测版本号与路径，生成精准能力记录。
- **Agent 只读问答契约**：
  - 查询：`steward_fact_query(type="capability", intent="处理 PDF 提取排版文字")`
  - 响应：
    ```json
    {
      "found": true,
      "recommended_tool": "pdftotext",
      "path": "/usr/bin/pdftotext",
      "version": "pdftotext version 24.02.0",
      "best_practice_usage": "pdftotext -layout input.pdf output.txt",
      "notes": "基于 poppler-utils，排版保真度高，性能极快，避免额外安装第三方 Python 库",
      "alternatives": [
        {"tool": "python3-pypdf", "type": "library", "notes": "已安装于系统 python 环境"}
      ]
    }
    ```

#### (2) 运行时进程与网络拓扑 (Process & Net Graph)
- **底层机制**：直接读取 Linux `/proc` 文件系统与内核网络套接字信息（避免频繁 fork `ps`/`ss` 命令的性能开销）。
- **进程指纹过滤**：自动过滤系统级底噪（内核线程 `kworker`、基础系统守护进程），精准聚合用户级与 Agent 级工作进程。
- **端口冲突防撞与追溯**：
  - 查询：`steward_port_check(port=8080)`
  - 响应：
    ```json
    {
      "port": 8080,
      "status": "occupied",
      "pid": 18493,
      "process_name": "python3",
      "command": "uvicorn main:app --host 0.0.0.0 --port 8080",
      "cwd": "/root/new/work/api-service",
      "user": "root",
      "uptime_seconds": 259200,
      "recommendation": "端口已被生产服务占用，建议改用 8081 或 8082"
    }
    ```

#### (3) 系统自动化拓扑 (Systemd & Cron Topology)
- **解决痛点**：Agent 调试时经常遇到“后台神秘进程干扰”或“修改配置文件引发服务雪崩”。
- **拓扑关系建模**：
  - 解析 `/etc/systemd/system/` 与 `/etc/crontab` / `crontab -l`；
  - 建立双向边：`ConfigFile -> depends_on -> Service`，`Script -> executed_by -> CronJob`；
  - Agent 在修改任意配置文件前，可查询其爆炸半径：“该文件被 `agent-steward.service` 监听，改动后需要 `systemctl reload`”。

#### (4) 源码工程与权威资料编目 (Code & Knowledge Graph)
- **资产发现**：
  - 自动扫描宿主机源码区（如 `/root/new/src`、`/root/new/work`），识别 Git 仓库状态；
  - 识别仓库是否具备 `.codegraph`，提示 Agent：“该项目已建代码地图，请优先调用 `codegraph mcp` 而非 `grep`”；
  - 编目本地权威资料（如 `/root/new/note/` 权威网站、离线知识库、已沉淀脚本库 `/root/new/shared_proj`）。

---

### 3.2 模块二：用户偏好与规约引擎 (Preference & Governance) —— 协作灵魂

事实回答了物理现实，偏好则决定了人机协作的秩序与默契。

```
                    ┌────────────────────────────┐
                    │ Agent 请求新建/操作某资产   │
                    └─────────────┬──────────────┘
                                  │ 意图: "我想写一个美股财报抓取脚本"
                                  ▼
                    ┌────────────────────────────┐
                    │    偏好规则引擎 (Engine)    │
                    │  • 匹配规则库 (Rules)       │
                    │  • 评估优先级 (Priority)    │
                    │  • 意图打分 (Intent Match) │
                    └─────────────┬──────────────┘
                                  ▼
                    ┌────────────────────────────┐
                    │    输出结构化放置建议       │
                    │  Target: /root/new/shared_proj/sec_crawler/
                    │  Naming: snake_case.py     │
                    │  Conventions: 需带 README  │
                    └─────────────┬──────────────┘
                                  │
                   （若用户后期手动移动/纠正位置）
                                  ▼
                    ┌────────────────────────────┐
                    │   纠偏自演化器 (Learner)    │
                    │  • 记录纠错差异            │
                    │  • 提炼新偏好规则并入库     │
                    └────────────────────────────┘
```

#### (1) 资产放置规则体系 (Placement Rules)
- **基准路径规约**：
  - 脚本与复用工具 -> `/root/new/shared_proj/<name>/`
  - 技能文件 -> `/root/new/skills/<name>/`
  - 临时实验文件 -> `/root/new/tmp/` (带 TTL 自动清理)
  - 源码研究克隆 -> `/root/new/src/<name>/`
  - 正式立项项目 -> `/root/new/work/<name>/`
  - 重要笔记/清单 -> `/root/new/note/<name>.md`
- **安全与命名约束**：
  - 强制小写字母 + 下划线；
  - 包含 API Key / Token 的配置严格禁止进入受监控资产库，自动提示写入环境变量或掩码处理。

#### (2) 工程与工作流偏好 (Conventions)
- **工具链偏好**：例如“在有 `.codegraph` 的目录，严格禁止全盘 `grep`，必须使用 `codegraph` 工具”；
- **时区与格式偏好**：北京时区（Asia/Shanghai）、GitHub-flavored Markdown、SVG 需独立落盘引用；
- **真实性门禁偏好**：严禁无来源编造数据，引述事实必须有落地出处。

#### (3) 纠偏自演化机制 (Rule Learner)
- 当用户手动纠偏 Agent 产生的文件位置（例如将文件从 `/tmp/foo.py` 移动到 `/root/new/shared_proj/foo/`）时，inotify 捕获文件跨目录搬移行为；
- 记录纠偏事件，自演化器生成规则推荐并在下次会话中主动对齐。

---

### 3.3 模块三：记忆与上下文适配层 (Memory & Context) —— 开放可插拔生态

#### (1) 为什么绝不重复造轮子？
当前开源社区在长程记忆、对话状态追踪、认知心理学建模方面已经涌现出一批优秀项目：
- **Mem0**：专注生产级 Agent 记忆层，支持长短期记忆、个性化事实提取；
- **Letta (MemGPT)**：专注多层级上下文管理（Core Memory / Archival Memory / Recall Memory）；
- **Zep**：高性能长程记忆服务，带时序图谱；
- **PowerContext**：工程级项目上下文与记忆沉淀。

**大管家的战略定位是“宿主环境底座”，而非“又一个孤立的记忆应用”**。因此，记忆模块采用 **SPI（Service Provider Interface）适配器架构**，大管家扮演**统一协议门面（Facade）**。

#### (2) Memory Provider SPI 抽象
```rust
#[async_trait]
pub trait MemoryProvider: Send + Sync {
    /// 语义/关键词检索记忆
    async fn search(&self, query: &str, limit: usize, filter: Option<MemoryFilter>) -> Result<Vec<MemoryItem>>;
    
    /// 保存跨会话记忆或任务摘要
    async fn store(&self, item: MemoryItem) -> Result<String>;
    
    /// 获取当前会话上下文画像
    async fn get_context(&self, session_id: &str) -> Result<SessionContext>;
    
    /// 清理或淘汰过期记忆
    async fn evict(&self, criteria: EvictCriteria) -> Result<usize>;
}
```

#### (3) 双层运行模式
1. **内置轻量实现 (Built-in Local Provider)**：
   - 基于嵌入式 SQLite + FTS5 + 本地 Ollama/微型 Embedding；
   - 零网络请求、完全离线、开箱即用，适合个人单机环境。
2. **外部成熟生态桥接器 (External Adapters)**：
   - **Mem0 Adapter**：通过 Mem0 REST/Python API 桥接，支持 Mem0 托管的长短期事实提取；
   - **Letta Adapter**：桥接 Letta 服务端，同步 Core Memory 与 Archival 检索；
   - **PowerContext Adapter**：桥接 PowerContext 记忆检索与项目工作流上下文。
   - 用户只需在 `steward.toml` 中配置：
     ```toml
     [memory]
     provider = "mem0" # 可选: "builtin" | "mem0" | "letta" | "powercontext"
     
     [memory.mem0]
     api_key = "..."
     endpoint = "http://127.0.0.1:8888"
     ```
   - Agent 端只需调用统一步骤 `steward_memory_search`，大管家在底层自动透明路由！

---

## 四、技术选型与运行形态

| 组件 | 选型 | 理由 |
|------|------|------|
| **核心运行时** | **Rust** (当前已全面迁移) | 单二进制零依赖、常驻内存仅 15MB、极速 /proc 内核级解析、无 GC 抖动、绝对安全 |
| **元数据与图存储** | **SQLite (WAL 模式) + FTS5** | 嵌入式、单文件、支持 ACID、全文 BM25 搜索性能卓越 |
| **进程与内核感知** | **procfs / netlink / ss** (Linux 原生 API) | 微秒级直接解析内核状态，零系统子进程 fork 开销，不产生系统底噪 |
| **对外通信协议** | **MCP (stdio + SSE) + CLI + Unix Socket** | 工业级 Agent 标准协议，原生支持 Antigravity / Claude Code / Cursor |
| 相似度 | 余弦距离 | 标准选择 |

### 3.5 技术风险项

#### sqlite-vec 成熟度风险

sqlite-vec（Alex Garcia）是相对年轻的扩展，需注意：
- **HNSW 增量插入/删除**：需实测验证。MVP 阶段数据量 < 10K，可先用暴力扫描（`vec0` 默认 flat index），性能已足够（10K × 768d 暴力搜索约 20ms）。
- **与 FTS5 触发器的共存**：两者都通过虚拟表工作，共用同一个 SQLite 连接，目前无已知冲突，但需要集成测试覆盖。
- **降级策略**：若 sqlite-vec 在特定平台上编译困难，可降级为纯 FTS5 模式（关键词搜索），向量搜索通过外部进程（如 Python faiss-lite）补充。

**决策**: MVP 用 sqlite-vec flat index。数据量超 10K 后评估切 HNSW 或外部向量库。

#### FTS5 中文分词方案

FTS5 内置的 `unicode61` tokenizer 按 Unicode 字符类别分词，对中文效果差（按字拆分而非按词）。

**决策: 入库前 jieba 分词（路径 1）**

```
写入流程:
  原始中文文本
       │
       ▼
  jieba.cut(text)  →  "项目 steward 主 存储 选型 确定 为 SQLite"
       │
       ▼
  空格分隔的分词结果存入 FTS5
       │
       ▼
  FTS5 用 unicode61 tokenizer 按空格切分（已是词级别）

查询流程:
  用户查询 query
       │
       ▼
  jieba.cut(query)  →  分词
       │
       ▼
  FTS5 MATCH 查询
```

Go 生态可用 `yanyiwu/gojieba` 或 `go-ego/gse`（纯 Go，无 cgo 依赖）。

这种方式的优势是 FTS5 不需要自定义 tokenizer，也不依赖 ICU 编译，兼容性最好。代价是写入和查询都多一步分词，但开销极小（< 1ms/条）。

### 3.4 接口协议

| 协议 | 用途 | 实现 |
|------|------|------|
| **MCP** (Model Context Protocol) | Agent 调用主通道 | stdio transport (MVP) + SSE transport (P1) |
| **REST** | 外部集成、Web UI、脚本调用 | HTTP/JSON, 绑定 127.0.0.1 |
| **CLI** | 人工操作、调试、脚本集成 | steward 子命令 |

---

## 四、进程模型

```
                  ┌──────────────────┐
                  │   steward daemon  │
                  │   (主进程)         │
                  │                   │
                  │  ┌──────────────┐ │
      systemd     │  │ MCP Server   │ │◄─── Agent 通过 stdio 连接
    或 launchd ───►│  │ (stdio)      │ │
      启动管理     │  └──────────────┘ │
                  │  ┌──────────────┐ │
                  │  │ REST Server  │ │◄─── curl / Web UI / 脚本
                  │  │ (HTTP 9527)  │ │
                  │  └──────────────┘ │
                  │  ┌──────────────┐ │
                  │  │ Scheduler    │ │     内置定时器
                  │  │ (cron-like)  │ │     环境快照、巡检、维护
                  │  └──────────────┘ │
                  │  ┌──────────────┐ │
                  │  │ File Watcher │ │     inotify / FSEvents
                  │  │ (可选)       │ │     监听关注目录变更
                  │  └──────────────┘ │
                  └──────┬───────────┘
                         │
                   ┌─────▼─────┐
                   │ SQLite DB │  ~/.steward/steward.db
                   │ + FTS5    │  ~/.steward/config.toml
                   │ + Vec     │  ~/.steward/logs/
                   └───────────┘
```

### 4.1 进程生命周期

1. **安装**: `curl -sSL https://steward.dev/install | sh` 下载二进制到 `/usr/local/bin/steward`
2. **初始化**: `steward init` → 创建 `~/.steward/` 目录、初始化 SQLite 数据库、生成默认配置
3. **启动**: `steward start` → 以 daemon 模式运行，同时启动 MCP/REST/Scheduler
4. **连接**: Agent 通过 `steward mcp` 子命令获得 stdio MCP 通道
5. **停止**: `steward stop` → 优雅关闭，刷盘所有数据
6. **自恢复**: 进程崩溃后 systemd/launchd 自动重启，SQLite WAL 保证数据一致性

### 4.2 MCP 连接模式

Agent 连接 steward 有两种模式：

**模式 A: stdio（MVP）**
```
Agent Process
    │ spawn
    ▼
steward mcp   ←──stdin/stdout──► Agent
    │
    ▼ (内部连接)
steward daemon
```

Agent 启动 `steward mcp` 子进程，这个子进程通过 Unix Socket 连接到 daemon，自身作为 Agent 和 daemon 之间的 MCP 桥接。

**模式 B: SSE（P1）**
```
Agent Process ──── HTTP/SSE ────► steward daemon:9527/mcp
```

Agent 直接通过 HTTP SSE 连接 daemon 的 MCP endpoint，无需额外进程。

---

## 五、存储架构

### 5.1 数据库 Schema 概览

```
steward.db
├── memories          # 记忆表
├── memories_fts      # FTS5 全文索引
├── memories_vec      # 向量索引 (sqlite-vec)
├── assets            # 资产表
├── assets_fts        # 资产全文索引
├── asset_tags        # 资产标签关联
├── services          # 服务/环境表
├── env_snapshots     # 环境快照表
├── changes           # 变更事件表 (P1)
├── links             # 关联关系表 (P2)
├── config            # 运行时配置 KV
└── migrations        # 版本迁移记录
```

### 5.2 SQLite 配置优化

```sql
-- WAL 模式，支持并发读
PRAGMA journal_mode = WAL;

-- 合理的 cache size（默认 2000 pages = ~8MB）
PRAGMA cache_size = -8000;  -- 8MB

-- 同步模式：NORMAL，在 WAL 下依然安全
PRAGMA synchronous = NORMAL;

-- 启用外键
PRAGMA foreign_keys = ON;

-- mmap 加速大表读取（64MB）
PRAGMA mmap_size = 67108864;
```

### 5.3 Embedding 流程

```
写入记忆/资产 content
       │
       ▼
  是否有 Ollama 可用？
       │
   ┌───┴───┐
   │ Yes   │ No
   ▼       ▼
Ollama API  纯 FTS5 模式
(nomic-     (降级: 只做关键词搜索,
embed-text)  向量字段留空,
   │         后台排队等 Ollama 恢复)
   ▼
768维向量
   │
   ▼
写入 sqlite-vec 虚拟表
```

**关键决策**: Embedding 不阻塞写入。如果 Ollama 不可用，先写入原始内容和 FTS 索引，向量列为空，标记 `embedding_status = pending`。后台 Scheduler 定期扫描 pending 项，在 Ollama 恢复后补充向量。

---

## 六、搜索架构

### 6.1 混合搜索（Hybrid Search）

```
用户查询 query
       │
       ├────────────────────────┐
       ▼                        ▼
  FTS5 全文搜索              向量语义搜索
  (BM25 打分)                (余弦相似度)
       │                        │
       ▼                        ▼
  FTS 结果集                 Vec 结果集
  [(id, bm25_score)]         [(id, cosine_sim)]
       │                        │
       └────────────┬───────────┘
                    ▼
            RRF (Reciprocal Rank Fusion)
            score = Σ 1/(k + rank_i)
                    │
                    ▼
            合并排序结果
                    │
                    ▼
            应用过滤条件
            (type, time_range, tags, status)
                    │
                    ▼
            应用权重修正
            (weight 字段加权)
                    │
                    ▼
            Top-N 返回
```

### 6.2 RRF 参数

```
k = 60  (标准值，平衡精确匹配与语义匹配)
α = 0.5 (FTS 和 Vec 的相对权重，可配置)
```

用户可通过 API 参数调整 `α`：
- `α = 1.0`：纯关键词搜索
- `α = 0.0`：纯语义搜索
- `α = 0.5`：均衡混合（默认）

---

## 七、文件监听架构

### 7.1 监听策略

不做全盘监听（太重），采用 **注册制 + 智能默认**：

```
注册关注目录列表
(config.toml 配置)
       │
       ▼
  inotify / FSEvents
  监听注册的目录树
       │
       ▼
  事件过滤
  (忽略 .git/, node_modules/,
   __pycache__/, .steward/ 等)
       │
       ▼
  防抖 (debounce 300ms)
  同文件连续修改合并为一次
       │
       ▼
  变更事件入库
  changes 表 + 关联更新
```

### 7.2 智能默认目录

初始化时自动注册这些目录（如果存在）：

| 目录 | 理由 |
|------|------|
| `~/projects/` | 常见项目根 |
| `~/work/` | 工作目录 |
| `~/Documents/` | 文档 |
| 用户 `$HOME` 下的 `.bashrc`, `.zshrc`, `.gitconfig` 等 | 环境配置 |

用户可通过 `steward watch add <path>` 增加、`steward watch rm <path>` 移除。

---

## 八、环境感知架构

### 8.1 数据采集器

```
┌────────────────────────┐
│   Environment Service  │
│                        │
│  ┌─────────────────┐   │
│  │ System Collector │   │  uname, cpu, mem, disk, gpu
│  ├─────────────────┤   │
│  │ Service Detector │   │  ss -tlnp → 端口扫描 → 服务识别
│  ├─────────────────┤   │  systemctl/launchctl/docker
│  │ Software Scanner │   │  dpkg/rpm/brew → 已安装软件
│  ├─────────────────┤   │
│  │ Network Checker  │   │  ip addr, 代理检测, DNS
│  ├─────────────────┤   │
│  │ Cron Scanner     │   │  crontab, systemd timers, hermes cron
│  ├─────────────────┤   │
│  │ Shell Env        │   │  $PATH, 关键环境变量
│  └─────────────────┘   │
│           │             │
│           ▼             │
│    Snapshot Assembler   │  → env_snapshots 表
│           │             │
│           ▼             │
│    Diff Engine (P1)     │  → changes 表
└────────────────────────┘
```

### 8.2 服务识别策略

不依赖固定列表，使用 **端口 → 进程 → 特征** 三层识别：

```
ss -tlnp 获取监听端口列表
          │
          ▼
    每个端口查 pid
          │
          ▼
    /proc/<pid>/exe  → 进程名
    /proc/<pid>/cmdline → 启动参数
          │
          ▼
    特征匹配规则库
    ┌─────────────────────────────────┐
    │ postgres  → PostgreSQL          │
    │ mysqld    → MySQL               │
    │ redis     → Redis               │
    │ ollama    → Ollama              │
    │ node      → Node.js App         │
    │ docker    → Docker Engine       │
    │ ...       → 扩展规则文件         │
    └─────────────────────────────────┘
          │
          ▼
    尝试获取版本
    (pg: SELECT version(),
     redis: INFO server,
     mysql: SELECT @@version)
```

---

## 九、安全架构

### 9.1 访问控制

| 通道 | 认证方式 | 说明 |
|------|----------|------|
| CLI | 本地 Unix 用户身份 | 同用户直接访问，无需额外认证 |
| MCP (stdio) | 继承父进程权限 | Agent spawn 的子进程，天然可信 |
| REST | Bearer Token | `steward init` 时生成随机 token，存 `~/.steward/token` |
| REST (远程) | 默认禁止 | 只绑 127.0.0.1，需显式配置才能绑 0.0.0.0（不推荐） |

### 9.2 数据安全

- **数据库文件权限**: `chmod 600 ~/.steward/steward.db`
- **配置文件权限**: `chmod 600 ~/.steward/config.toml`
- **Token 文件权限**: `chmod 600 ~/.steward/token`
- **无网络外传**: 不含任何 telemetry、auto-update、cloud sync 代码
- **敏感信息标记**: 记忆/资产可标记 `sensitive: true`，标记后：
  - REST 响应中该字段用 `***` 掩码
  - 不出现在 `steward brief` 的汇总中
  - 导出时自动跳过

---

## 十、部署架构

### 10.1 安装方式

| 方式 | 命令 | 适用场景 |
|------|------|----------|
| 一键安装 | `curl -sSL https://steward.dev/install \| sh` | 普通用户 |
| Homebrew | `brew install steward` | macOS |
| apt/dnf | `apt install steward` | Debian/Ubuntu, Fedora |
| Docker | `docker run -v ~/.steward:/data steward` | 容器化部署 |
| 源码编译 | `go build -o steward ./cmd/steward` | 开发者/定制 |

### 10.2 目录结构

```
~/.steward/
├── steward.db          # 主数据库（SQLite）
├── steward.db-wal      # WAL 文件
├── steward.db-shm      # 共享内存
├── config.toml         # 配置文件
├── token               # REST API token
├── rules/              # 服务识别规则扩展
│   └── custom.toml
├── logs/
│   ├── steward.log     # 主日志
│   └── steward.log.1   # 轮转日志
├── backups/            # 自动备份
│   ├── steward-2026-11-14.db
│   └── steward-2026-11-13.db
└── plugins/            # 插件目录 (P2)
```

### 10.3 自动备份

Scheduler 每天 03:00 自动备份：
```
SQLite Online Backup API
→ ~/.steward/backups/steward-<date>.db
→ 保留最近 7 天
→ 超出自动删除
```

---

## 十一、性能设计

### 11.1 热路径优化

| 操作 | 目标延迟 | 优化手段 |
|------|----------|----------|
| 写入 1 条记忆 | < 50ms | WAL + async embedding |
| 关键词搜索 | < 100ms | FTS5 索引 |
| 语义搜索 | < 200ms | sqlite-vec HNSW, embedding 预计算 |
| 混合搜索 | < 200ms | 并行执行 FTS + Vec |
| `steward brief` | < 500ms | 缓存上次环境快照 + 最近记忆 top-10 |
| 环境快照（冷） | < 5s | 并发采集各维度 |
| 环境快照（热） | < 500ms | 增量更新 + 缓存 |

### 11.2 缓存策略

```
┌─────────────────────┐
│   In-Memory Cache    │
│                      │
│  env_snapshot_cache  │  最近一次完整快照, TTL 5min
│  brief_cache         │  最近一次 brief 结果, TTL 1min
│  service_cache       │  服务列表, TTL 1min
│  recent_memories     │  最近 100 条记忆, LRU
└──────────┬──────────┘
           │ miss
           ▼
    SQLite 查询
```

### 11.3 容量规划

| 规模 | 记忆条数 | 资产条数 | 数据库大小（估算） | 内存占用 |
|------|----------|----------|--------------------| ---------|
| 小型（个人） | < 10K | < 50K | < 100MB | ~50MB |
| 中型（重度用户） | 10K-100K | 50K-500K | 100MB - 1GB | ~100MB |
| 大型（极端） | > 100K | > 500K | > 1GB | ~200MB |

超过 100K 记忆建议开启自动归档（archived 状态的记忆移到 archive.db）。

---

## 十二、可扩展性设计

### 12.1 插件机制（P2）

```
steward daemon
       │
       ▼
  Plugin Manager
       │
  ┌────┼────────────────┐
  │    │                 │
  ▼    ▼                 ▼
Hook  Tool              Collector
Plugin Plugin           Plugin
│      │                 │
│  Agent 可调用的      新的环境采集器
│  自定义工具           (如 K8s 集群信息)
│
事件钩子
(on_memory_write,
 on_asset_change,
 on_env_diff)
```

插件通过 WASM 沙箱运行（安全隔离），或通过子进程 + stdin/stdout 通信（简单但需信任）。

### 12.2 多 Agent 并发

```
Agent A ──┐
Agent B ──┼──► steward daemon ──► SQLite (WAL)
Agent C ──┘
```

SQLite WAL 模式支持多个并发读者 + 1 个写者。steward daemon 是唯一写者，通过内部写入队列序列化写操作。多个 Agent 同时调用 MCP 不会冲突。

---

## 十三、冷启动策略

新用户首次 `steward init` 后的冷启动流程：

```
steward init
    │
    ├─ 1. 创建 ~/.steward/ 及数据库
    │
    ├─ 2. 快速环境扫描 (~2s)
    │      uname, cpu, mem, disk
    │      ss -tlnp → 识别运行中的服务
    │      dpkg/rpm list → 已安装软件(后台异步)
    │
    ├─ 3. 智能目录发现 (~1s)
    │      扫描 $HOME 下第一层目录
    │      识别项目目录 (.git, package.json, Cargo.toml 等)
    │      注册为资产
    │
    ├─ 4. Agent 历史导入提示
    │      检测 ~/.gemini/ → 提示导入 Antigravity 历史
    │      检测 ~/.cursor/ → 提示导入 Cursor 历史
    │      检测 ~/.chatgpt/ → 提示导入 ChatGPT 导出
    │
    ├─ 5. Ollama 检测
    │      curl http://localhost:11434/api/tags
    │      有 → 记录可用模型列表, 自动选 embedding 模型
    │      无 → 提示安装, 降级为纯 FTS 模式
    │
    └─ 6. 启动 daemon
           开始监听 MCP + REST
           后台继续深度扫描（不阻塞）
```

**冷启动到可用：< 5 秒。** 后台深度扫描可能持续 30s-2min，但不阻塞用户。

---

## 十四、数据迁移与版本管理

### 14.1 Schema 版本

每次 schema 变更在 `migrations` 表记录版本号。启动时检查：

```
当前 db schema_version = N
代码期望 schema_version = M

if N < M:
    逐版本执行 migration N+1, N+2, ..., M
    更新 schema_version = M
if N > M:
    报错: 数据库版本高于软件版本, 请升级 steward
if N == M:
    正常启动
```

### 14.2 数据导出/导入

```bash
# 导出全部数据为 JSON（可选过滤）
steward export --format json --output backup.json

# 导出为 SQLite 副本
steward export --format sqlite --output backup.db

# 导入
steward import --from backup.json

# 从另一台机器迁移
scp remote:~/.steward/steward.db .
steward import --from steward.db --merge
```

---

*下一步: 06-api-spec.md — MCP/REST/CLI 接口定义*
