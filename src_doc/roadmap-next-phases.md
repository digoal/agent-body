# Agent Host Steward — 下一阶段路线图 (Phase 5~9)

> 创建时间: 2026-11-18T10:00:00+08:00  
> 基线: 三位一体 Phase 1~4 全部完成 · 89 单测全绿 · 11155 行 Rust · v0.3.1  
> 对齐: README.md ISSUE #1~#9

---

## 一、ISSUE 与阶段映射总览

| ISSUE | 核心问题 | 映射阶段 | 优先级 |
|:-----:|---------|:--------:|:------:|
| #9 | 宿主机主动扫描 + 断点续扫 + 属性图关联 | **Phase 5** | P0 |
| #2 | 记录取舍策略（什么该记、什么不记） | **Phase 5** | P0 |
| #3 | Agent 流程 hook 时机 + 非 Agent 时段主动扫描 | **Phase 5** | P0 |
| #4 | 存储后端抽象（SQLite → PG/DuckDB 多后端） | **Phase 6** | P1 |
| #8 | 产品用户 = Agent，输出格式 Agent-Native | **Phase 7** | P1 |
| #6 | Benchmark 体系与竞品对标 | **Phase 7** | P1 |
| #5 | 跨平台支持（macOS / Windows） | **Phase 8** | P2 |
| #1 | OS 级功能定位思考 | **Phase 9** | P2 |
| #7 | 产品边界守护、原子功能哲学 | **Phase 9** | P2 |

**排序逻辑**:  
- P0 = 产品核心差异化能力，直接决定"大管家"是否名副其实；  
- P1 = 规模化与工程质量，Agent 大量调用后的瓶颈会最先暴露在存储和输出格式；  
- P2 = 生态拓展与长期定位，不影响当前核心价值但决定天花板。

---

## 二、Phase 5：宿主机主动扫描与智能记录引擎

> **对齐 ISSUE**: #9（主动扫描 + 断点续扫 + 属性图）、#2（记录取舍）、#3（hook 时机）  
> **目标**: 大管家不再被动等待 Agent 提问，而是像真正的管家一样主动巡查、持续积累、按重要性筛选存储。

### 5.1 断点续扫状态机 (Incremental Scanner State Machine)

**涉及文件**: `src/scanner/state_machine.rs`（新增）、`src/scanner/mod.rs`（扩展）

**当前状态**: `Scanner::scan_path()` 是一次性全量扫描，无中断恢复能力。扫描到一半被 kill 则全部丢失。

**目标实现**:
- `ScanJob` 结构体：记录 `job_id`、`root_path`、`last_scanned_path`、`depth`、`status`（pending/running/paused/completed）、`progress_pct`、`created_at`、`updated_at`
- 每扫完一个目录批次（如 100 个目录条目）持久化一次断点到 SQLite `scan_jobs` 表
- 恢复时从 `last_scanned_path` 的字典序下一个条目继续，不重复扫描
- 新增 MCP 工具 `steward_scan_start` / `steward_scan_status` / `steward_scan_pause` / `steward_scan_resume`

**验收标准**:
- 扫描 `/root/new/` 时在 50% 位置 kill 进程，重启后自动续扫，最终结果与全量扫描一致
- 单测覆盖：正常完成、中断恢复、重复启动幂等

### 5.2 重要性评分与噪声过滤器 (Importance Scorer & Noise Filter)

**涉及文件**: `src/scanner/importance.rs`（新增）、`src/memory/intake.rs`（新增）

**当前状态**: 记忆和事实"来者不拒"，没有入库前的重要性判断。大量低价值信息（如 `/tmp` 下的中间文件、`node_modules` 深层依赖）会污染检索质量。

**目标实现**:
- `ImportanceScorer` trait：输入 `(path, metadata, content_summary)` → 输出 `score: f32`（0.0~1.0）+ `reason: String`
- 内置评分规则引擎（无需 LLM）：
  - 路径权重：用户工作目录 > 系统目录 > 临时目录
  - 文件类型权重：`.rs/.py/.sql/.md` > `.log/.tmp/.cache`
  - 变更频率权重：近 7 天有变更 > 30 天未变更 > 90 天未变更
  - 关联权重：被其他文件引用/依赖的 > 孤立文件
- `NoiseFilter`：score < 阈值（默认 0.3，可配置）的直接跳过不入库
- 记忆入库同理：`MemoryIntake` 在写入前检查重复（语义去重 + 精确去重）+ 重要性评分

**验收标准**:
- 扫描 `/root/new/` 时，`/root/new/tmp/` 下的临时文件 score < 0.3 被过滤
- `node_modules/`、`.git/objects/` 等噪声目录不会产生资产记录
- 对比过滤前后的资产表行数，噪声减少 > 60%

### 5.3 属性图关联组织器 (Property Graph Organizer)

**涉及文件**: `src/fact/property_graph.rs`（新增）、`src/storage/schema.sql`（扩展）

**当前状态**: 事实以扁平表存储（`assets`、`memories`、`links`），缺乏层次化的属性图关联。Agent 查询"这个项目依赖了哪些工具、占用了哪些端口、跑了哪些 cron"需要分别调用多个 MCP 工具拼凑。

**目标实现**:
- 新增 `fact_nodes` 表（节点：项目/工具/端口/服务/cron/文件）和 `fact_edges` 表（关系：depends_on / listens_on / scheduled_by / reads / writes / part_of）
- `PropertyGraphOrganizer`：扫描完成后自动从已有事实表（capability/process_net/automation/assets_catalog）提取节点和边，写入图表
- 查询接口：`steward_fact_query` 扩展支持图遍历语义，如 `"项目 agent-steward 的完整依赖图"` → 返回以该项目为根的 N 跳子图
- 图的更新策略：增量更新（新扫描结果与已有图 diff，只更新变化部分）

**验收标准**:
- `steward_fact_query("agent-steward 依赖图")` 返回包含工具、端口、systemd 服务、cron 的关联图
- 图节点和边可以通过 `steward_links_visualize` 渲染为 mermaid 图
- 增量更新：修改一个 cron 后重新扫描，只更新该 cron 相关的节点和边

### 5.4 定时扫描调度器与 Agent Hook 集成 (Scheduled Scan & Agent Hooks)

**涉及文件**: `src/daemon/scheduler.rs`（新增）、`src/api/hooks.rs`（新增）

**当前状态**: 扫描只能手动触发（CLI `scan` 子命令或 MCP `steward_asset_scan`）。没有定时后台扫描。Agent 工具调用前后也没有 hook 点。

**目标实现**:
- **定时扫描**: daemon 模式下内置 cron 表达式调度器（基于 tokio cron），默认每 6 小时全量增量扫描一次，可通过 `steward.toml` 配置
- **Agent Hook 协议**:
  - `pre_tool_call(tool_name, args)` → 大管家在 Agent 调用工具前检查：端口冲突？文件即将覆盖的是关键配置？
  - `post_tool_call(tool_name, result)` → 工具执行后捕获变更：新增了文件？修改了配置？自动更新事实图
  - Hook 通过 MCP 工具暴露：`steward_hook_pre` / `steward_hook_post`
  - Hook 是**建议性**的（返回 warnings），不强制阻断 Agent 操作
- **非 Agent 时段**: daemon 进程在无 MCP 连接时自动进入"巡查模式"——以低优先级执行增量扫描 + 记忆衰减审计 + 事实图刷新

**验收标准**:
- daemon 启动后 6 小时自动触发一次增量扫描（可用测试环境缩短到 10 秒验证）
- `steward_hook_pre("bind_port", {"port": 8080})` 在端口已被占用时返回警告
- `steward_hook_post("write_file", {"path": "/etc/xxx.conf"})` 自动更新 automation 拓扑中该文件的依赖链

---

## 三、Phase 6：存储层抽象与多后端支持

> **对齐 ISSUE**: #4（语义/图/FTS/标量/混合检索 + 大规模后端）  
> **目标**: 存储层从"硬编码 SQLite"进化为可插拔 SPI，支持 PG（生产级）和 DuckDB（分析级）后端。

### 6.1 Storage Provider SPI 设计

**涉及文件**: `src/storage/spi.rs`（新增）

**当前状态**: `Storage` 结构体直接持有 `Arc<Mutex<Connection>>`（rusqlite），所有 20+ 个方法都是 SQLite 方言 SQL。

**目标实现**:
- 定义 `StorageProvider` trait（同 memory SPI 的设计哲学）：
  ```
  trait StorageProvider: Send + Sync {
      fn store_asset(&self, record: &AssetRecord) -> Result<()>;
      fn search_assets(&self, query: &str, limit: usize) -> Result<Vec<AssetRecord>>;
      fn store_memory(&self, record: &MemoryRecord) -> Result<()>;
      fn search_memories_hybrid(&self, query: &str, embedding: &[f32], limit: usize) -> Result<Vec<MemoryRecord>>;
      fn store_fact_node(&self, node: &FactNode) -> Result<()>;
      fn traverse_graph(&self, root_id: &str, max_hops: usize) -> Result<SubGraph>;
      // ... 其余方法
  }
  ```
- 现有 `Storage` 重构为 `SqliteProvider` 实现该 trait
- `StorageRouter`：根据 `steward.toml` 中 `[storage] provider = "sqlite"` 配置路由

**验收标准**:
- 重构后 89 个现有测试 100% 通过（行为不变）
- `SqliteProvider` 完整实现所有 trait 方法

### 6.2 PostgreSQL 后端适配器

**涉及文件**: `src/storage/pg_provider.rs`（新增）

**目标实现**:
- 基于 `tokio-postgres` + `pgvector` 实现 `StorageProvider`
- 支持：向量检索（pgvector）、全文检索（tsvector/tsquery）、标量检索、混合 RRF 排序
- Schema 自动迁移（同 SQLite 的 `init_schema` 模式）
- 连接池管理（deadpool-postgres）

**验收标准**:
- 配置 `provider = "postgres"` + 连接串后，所有 MCP 工具行为与 SQLite 一致
- 10 万条记忆的混合检索延迟 < 200ms（本地 PG 实例）

### 6.3 DuckDB 后端适配器（可选）

**涉及文件**: `src/storage/duckdb_provider.rs`（新增）

**目标实现**:
- 基于 `duckdb-rs` 实现 `StorageProvider`
- 聚焦分析场景：资产统计、变更趋势、记忆热力图等 OLAP 查询
- 可与 SQLite/PG 并行运行（主存储 + 分析副本）

**验收标准**:
- 资产统计类查询（按类型/目录/时间聚合）性能优于 SQLite 10x+

### 6.4 后端迁移工具

**涉及文件**: `src/storage/migration.rs`（新增）

**目标实现**:
- `steward migrate --from sqlite --to postgres` 一键迁移所有数据
- 支持增量同步（基于 `updated_at` 时间戳）
- 迁移前后一致性校验（行数 + 抽样 checksum）

---

## 四、Phase 7：Agent-Native 输出优化与 Benchmark 体系

> **对齐 ISSUE**: #8（用户 = Agent）、#6（Benchmark 对标）  
> **目标**: 确保所有输出对 Agent 是最优消费格式；建立可量化的性能与质量基准。

### 7.1 Agent-Native 输出格式标准化

**涉及文件**: `src/api/mcp_tools.rs`（重构）、`src/api/output_format.rs`（新增）

**当前状态**: MCP 工具输出混合了人类可读文本（含 emoji、中文描述）和结构化 JSON。Agent 解析时需要额外理解自然语言。

**目标实现**:
- 所有 MCP 工具输出统一为 `StewardResponse<T>` 结构体：
  ```json
  {
    "status": "ok|warning|error",
    "data": { /* 类型化结构数据 */ },
    "actions": [ /* 建议的后续动作 */ ],
    "confidence": 0.95,
    "source": "fact_graph|memory|placement"
  }
  ```
- `actions` 字段：每条建议附带可直接执行的命令或 MCP 调用参数，Agent 无需二次理解
- 保留 `format=human` 查询参数支持 CLI 人类可读输出

**验收标准**:
- Agent 端（如 Antigravity）可以直接解析 `data` 字段无需正则提取
- `actions` 中的建议可以被 Agent 直接作为下一步工具调用参数

### 7.2 Benchmark 套件

**涉及文件**: `benches/`（新增目录）、`src_doc/benchmark-spec.md`（新增）

**目标实现**:
- **响应延迟 Benchmark**: 各 MCP 工具的 p50/p95/p99 延迟（1000 次调用）
- **扫描吞吐 Benchmark**: 10K/100K/1M 文件目录的扫描速度（文件/秒）
- **检索质量 Benchmark**: 构造 100 个标准查询 + 预期结果集，计算 Recall@10 / NDCG@10
- **内存占用 Benchmark**: 10K/100K 资产下的 RSS 内存峰值
- **竞品对标**: 与 `mem0`、`letta`、`PowerContext` 在记忆检索场景对比；与 `osquery` 在宿主机事实查询场景对比

**验收标准**:
- `cargo bench` 可一键运行全部基准测试
- 输出标准化报告（Markdown 表格 + 火焰图链接）

---

## 五、Phase 8：跨平台抽象层

> **对齐 ISSUE**: #5（macOS / Windows）  
> **目标**: 核心逻辑平台无关，OS 特定实现 cfg-gated 隔离。

### 8.1 平台抽象 Trait 体系

**涉及文件**: `src/platform/mod.rs`（新增）、`src/platform/linux.rs`、`src/platform/macos.rs`、`src/platform/windows.rs`

**当前 Linux 硬绑定清单**:
| 功能 | 当前实现 | macOS 替代 | Windows 替代 |
|------|---------|-----------|-------------|
| 进程端口扫描 | `/proc/net/tcp` 解析 | `lsof -iTCP` | `netstat` / WMI |
| 自动化拓扑 | systemd unit 文件解析 | launchd plist 解析 | Windows Service / Task Scheduler |
| 文件监听 | inotify (notify crate) | kqueue (notify crate 已支持) | ReadDirectoryChangesW (notify crate 已支持) |
| 进程详情 | `/proc/<pid>/{cmdline,cwd,status}` | `sysctl` / `proc_pidpath` | `NtQueryInformationProcess` |
| 工具能力扫描 | `$PATH` + `--version` | 相同 | `$PATH` + `where.exe` |

**目标实现**:
- `PlatformProvider` trait：统一接口，每个平台实现
- `notify` crate 的 `RecommendedWatcher` 已天然跨平台（inotify/kqueue/ReadDirectoryChangesW），只需解耦 daemon 中的 inotify 硬引用
- 编译时 `cfg(target_os = "...")` 选择实现

**验收标准**:
- `cargo build --target x86_64-apple-darwin` 交叉编译成功
- macOS 上 `steward_env_show` / `steward_port_check` 功能正常

### 8.2 CI 矩阵扩展

**涉及文件**: `.github/workflows/ci.yml`

**目标**: GitHub Actions 矩阵增加 macOS-latest 和 windows-latest

---

## 六、Phase 9：产品边界守护与生态定位

> **对齐 ISSUE**: #1（OS 级功能？）、#7（产品边界）  
> **目标**: 明确大管家的"不做清单"和生态协作协议，防止功能蔓延。

### 9.1 产品边界治理文档

**涉及文件**: `src_doc/product-boundary.md`（新增）

**核心输出**:

```
┌────────────────────────────────────────────────────────────────┐
│                   Agent Host Steward 做什么                      │
│                                                                │
│  ✅ 宿主机物理世界感知（工具/端口/进程/服务/文件拓扑）            │
│  ✅ 用户偏好规约（文件放置/命名/安全纪律）                        │
│  ✅ 轻量级本地记忆（内置 SQLite + SPI 适配外部引擎）             │
│  ✅ Agent 情境组装（brief = 事实 + 偏好 + 记忆融合）              │
│  ✅ 变更审计与依赖影响推演                                       │
│  ✅ 宿主机资源 GC（临时文件/僵尸进程清理）                        │
├────────────────────────────────────────────────────────────────┤
│                   Agent Host Steward 不做什么                    │
│                                                                │
│  ❌ 对话管理/会话历史（交给 Agent 框架）                          │
│  ❌ 重型 RAG/向量数据库（交给 Mem0/Letta/PowerContext）           │
│  ❌ 代码编辑/生成/执行（交给 Agent 本体）                         │
│  ❌ 网络代理/API 网关（交给基础设施）                             │
│  ❌ 容器编排/K8s 管理（交给 DevOps 工具链）                       │
│  ❌ 用户认证/权限管理（交给 OS 或 IAM）                           │
└────────────────────────────────────────────────────────────────┘
```

### 9.2 生态集成协议 (Steward Integration Protocol)

**涉及文件**: `src_doc/integration-protocol.md`（新增）

**目标**:
- 定义第三方 Agent 框架接入 Steward 的标准协议（MCP stdio + 约定的工具命名空间）
- 定义外部记忆引擎接入的 SPI 规范版本号（当前 v1）
- 定义宿主机事实图谱的查询语义规范（让生态伙伴可以构建上层应用）

### 9.3 OS 级能力暴露设计

**涉及文件**: `src_doc/os-integration-design.md`（新增）

**对 ISSUE #1 的回答**:
- **短期立场**: 保持 userspace 单二进制，不依赖 OS 内核修改。理由：(1) OS 内核迭代周期以年计，Agent 生态以月计；(2) 不同发行版/OS 分裂严重；(3) userspace 更灵活可插拔。
- **长期探索**: 通过 D-Bus 接口在 Linux 上暴露宿主机事实为系统级服务，让非 Rust 进程也能查询。macOS 通过 XPC Services。Windows 通过 Named Pipes / COM。
- **终极形态**: 如果 Agent OS 成为独立品类（如 Google Fuchsia 对 IoT），Steward 的核心能力可以作为 OS 子系统被内置。

---

## 七、实施节奏与依赖关系

```
Phase 5 (主动扫描)  ──→  Phase 6 (存储抽象)  ──→  Phase 7 (Agent-Native + Benchmark)
                                                       │
                                                       ├──→  Phase 8 (跨平台)
                                                       │
                                                       └──→  Phase 9 (产品边界)
```

- Phase 5 → Phase 6：属性图需要更强的存储后端支撑，5 先用 SQLite 实现原型，6 再抽象
- Phase 6 → Phase 7：Benchmark 需要在多后端上跑，所以 6 先行
- Phase 7 → Phase 8/9：输出格式和 Benchmark 稳定后再扩展平台和定义边界

**预估工作量**:

| 阶段 | 子任务数 | 预估新增代码行 | 核心难点 |
|:----:|:-------:|:------------:|---------|
| Phase 5 | 4 | ~2500 行 | 断点续扫状态机的正确性、属性图的增量更新 |
| Phase 6 | 4 | ~3000 行 | Storage SPI 重构不能破坏 89 个现有测试 |
| Phase 7 | 2 | ~1500 行 | 检索质量 Benchmark 的标准查询集构造 |
| Phase 8 | 2 | ~2000 行 | macOS/Windows 进程扫描的平台适配 |
| Phase 9 | 3 | ~500 行（主要是文档） | 产品边界的取舍决策 |

---

## 八、Phase 5 细化任务看板（串行断点续传）

> Phase 5 已全量完成交付。

| 子任务 ID | 任务内容 | 涉及路径 | 状态 | 完成时间 |
|:---------:|---------|---------|:----:|---------|
| 5.1 | 断点续扫状态机：`ScanJob` + SQLite `scan_jobs` 表 + 增量恢复逻辑 | `src/scanner/state_machine.rs`, `src/storage/schema.sql` | ✅ 已完成 | 2026-11-18 10:02 |
| 5.2 | 重要性评分器：`ImportanceScorer` trait + 内置规则引擎 + `NoiseFilter` | `src/scanner/importance.rs`, `src/memory/intake.rs` | ✅ 已完成 | 2026-11-18 10:05 |
| 5.3 | 属性图组织器：`fact_nodes/fact_edges` 表 + `PropertyGraphOrganizer` + `steward_fact_query` 图遍历扩展 | `src/fact/property_graph.rs`, `src/storage/schema.sql` | ✅ 已完成 | 2026-11-18 10:11 |
| 5.4 | 定时扫描 + Agent Hook：daemon cron 调度 + `steward_hook_pre/post` MCP 工具 | `src/daemon/scheduler.rs`, `src/api/hooks.rs` | ✅ 已完成 | 2026-11-18 10:17 |
| 5.5 | 端到端集成验收：全量测试 + 断点续扫实测 + 属性图查询实测 + 生产部署 | 全系统 | ✅ 已完成 | 2026-11-18 12:02 |

---

## 九、关键设计决策备忘

### 决策 1：属性图用 SQLite 还是独立图数据库？

**决策**: 先用 SQLite 邻接表实现（`fact_nodes` + `fact_edges`），理由：
- 当前规模（单机环境，千级节点）SQLite 完全胜任
- 不引入新依赖，保持单二进制交付
- Phase 6 存储抽象后，PG 后端可用 `Apache AGE` 或 `pg_graphql` 升级为原生图查询

### 决策 2：ImportanceScorer 用 LLM 还是规则？

**决策**: 纯规则引擎，理由：
- 大管家定位是**基础设施层**，不能依赖 LLM 推理（延迟不可控、成本高、离线场景挂掉）
- 规则可确定性复现、可单测、可调试
- 未来可开放 `ImportanceScorer` trait 让上层 Agent 注入 LLM 增强的评分器（插件模式）

### 决策 3：Agent Hook 是阻断式还是建议式？

**决策**: 建议式（返回 warnings + suggested_actions），理由：
- 大管家是"管家"不是"保安"，越权会让 Agent 框架不愿意接入
- Agent 框架自己决定是否采纳建议
- 保持大管家"朋友多、敌人少"的产品哲学

---

> **下一步**: 在 `progress.md` 中追加 Phase 5 条目，然后开始 5.1 的编码实现。
