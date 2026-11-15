# Agent Host Steward — 产品设计 Review 报告

> 审查人: digoal 德哥
> 审查范围: 7 个交付件全量（3,820 行 / 127KB）
> 审查时间: 2026-11-15

---

## 一、总体评价

文档体系完整，从场景→需求→功能→手册→架构→接口→数据模型，链路清晰，前后有可追溯性（场景编号→需求ID→功能点→DDL字段）。MVP 边界合理，P0/P1/P2 分级让开发节奏可控。

**整体质量: 8/10**。以下是需要补充、矛盾、遗漏、决策待定的具体问题。

---

## 二、跨文档不一致（必须修）

### 🔴 问题 1：memories 表缺少 `last_accessed` 和 `access_count` 字段

- **03-feature-spec.md** 的 MemoryEntry 数据模型明确定义了 `last_accessed: datetime` 和 `access_count: int`（第 61-62 行），衰减和强化策略都依赖这两个字段（第 99、109 行）。
- **07-data-model.md** 的 DDL 中 `memories` 表却没有这两个列。
- **影响**: 记忆衰减/强化机制无法实现。

**修复**: 07 DDL 补上 `last_accessed_at TEXT` 和 `access_count INTEGER DEFAULT 0`，加索引 `idx_memories_last_accessed`。

### 🔴 问题 2：资产类型枚举不一致

- **03-feature-spec.md** Assets 类型枚举: `[file, directory, project, script, tool, dataset, article, note]` — 8 种
- **06-api-spec.md** asset_register 类型枚举: `[project, script, document, dataset, config, media, other]` — 7 种
- **07-data-model.md** DDL CHECK 约束: `[project, script, document, dataset, config, media, other]` — 7 种

前者多了 `file, directory, tool, article, note`，后者多了 `document, config, media, other`。

**修复**: 统一为 07 的版本 `[project, script, document, dataset, config, media, other]`，回补 03。

### 🟡 问题 3：搜索权重公式不一致

- **03-feature-spec.md** 定义的排序公式: `final_score = 0.5 × semantic_score + 0.3 × keyword_score + 0.2 × recency_bonus × weight`
- **05-architecture.md** 定义的搜索架构用 RRF（Reciprocal Rank Fusion），而非加权线性组合

两种方法本质不同。RRF 是基于排名的融合，线性组合是基于分值的融合。

**修复**: 统一为 05 的 RRF 方案（更成熟，学术和工程实践都更多），删除 03 中的线性公式或标注为"早期草案，已被 05 的 RRF 取代"。

### 🟡 问题 4：配置格式不一致

- **02-requirements.md** NU-04: "YAML 配置文件，合理默认值"
- **04-product-manual.md**: `config.toml`
- **05-architecture.md**: TOML

**修复**: 统一为 TOML，更新 02 的 NU-04 描述。

### 🟡 问题 5：MCP 工具命名不一致

- **03-feature-spec.md**: `steward_memory_write`, `steward_assets_search`, `steward_context_brief`
- **04-product-manual.md**: `steward_memory_add`, `steward_asset_search`, `steward_brief`
- **06-api-spec.md**: `steward_memory_add`, `steward_asset_search`, `steward_brief`

`write` vs `add`、`assets` vs `asset`（单复数）、`context_brief` vs `brief`。

**修复**: 统一为 06 的命名（`steward_memory_add`, `steward_asset_*`），回补 03。

---

## 三、遗漏的场景/需求（应补充）

### 🟡 遗漏 1：记忆写入的"由谁触发"问题

当前设计隐含一个重大假设：**Agent 会主动调用 `steward_memory_add` 来写入记忆**。但实际使用中，大部分 Agent 不会自己决定"这段对话值得记住"。

**缺一个机制**: Agent 对话结束后（或会话中），由谁来提取值得记忆的内容？

三种方案：
1. **Agent 自行判断** — 需要 Agent 的 system prompt 里教它"遇到决策/偏好/经验时调 steward_memory_add"。这依赖 Agent 框架的配合。
2. **steward 主动提取** — Agent 结束对话后，把 transcript 推给 steward，steward 用本地 LLM 自动提取。但 01 的"非场景"第 2 条说"不做 LLM 调用"。
3. **混合模式** — Agent 主动写 + steward 提供"transcript → memories"批处理命令。

**建议**: 在 04 产品手册或 03 功能规格里增加一节"记忆写入策略"，说清楚这个问题。推荐方案 3。

### 🟡 遗漏 2：`steward brief` 的 token 预算控制

`steward brief` 返回的内容会被注入到 Agent 的 prompt 里。如果记忆多、项目多、环境复杂，brief 可能很长（几千 token），挤占 Agent 的上下文窗口。

**缺一个参数**: `max_tokens` 或 `budget`，让 Agent 指定"我最多给 brief 留 2000 token，你自己裁剪"。

**建议**: 在 06-api-spec 的 `steward_brief` 参数里加 `max_tokens: int`。

### 🟡 遗漏 3：多用户场景

当前设计全部基于单用户（`~/.steward/`）。如果一台机器上有多个用户（如跳板机、共享开发机），每个用户需要独立的 steward 实例。

当前设计已经天然支持（每个用户 `~/.steward/` 互不干扰），但没有显式提到。

**建议**: 在 05 架构设计或 04 产品手册里加一段"多用户"，说明天然隔离。

### 🟡 遗漏 4：数据库路径重映射

**03 的边界**说"不做远程机器管理"。但画像 C（DBA/SRE）管的是"多台机器"。如果 Agent 需要知道"远程服务器 db-prod-01 上的 PostgreSQL 版本"，steward 完全不管这个。

不需要现在做，但需要**预留一个扩展点**：未来版本里的"remote host"概念，或者让用户把远程信息作为 `fact` 类型记忆手动注册。

**建议**: 在 01-scenarios 的"非场景"里把这一条描述得更细，并在 03-feature-spec 里预留一个 `scope: local|remote` 字段注释。

---

## 四、架构/技术决策待确认

### 🟠 决策 1：Rust vs Go

05-architecture 把 Rust 作为首选。但考虑到：
- 开发速度：Go >> Rust
- SQLite 生态绑定：Go 的 `mattn/go-sqlite3` 成熟度高，Rust 的 `rusqlite` 也不错
- MCP SDK：目前 Anthropic 官方有 TypeScript 和 Python 的 MCP SDK，Go 有社区版 `mark3labs/mcp-go`，Rust 也有 `modelcontextprotocol/rust-sdk`
- 目标用户不是百万并发服务器，而是单机本地工具
- 单二进制：Go 也能编译成单二进制

**我的判断**: 如果是我独立做 MVP，我会选 **Go**，理由是 MVP 阶段开发效率优先。但 Rust 的长期优势（内存占用、性能天花板）是真实的。

**建议**: 05 里标注"MVP 阶段用 Go 快速验证，1.0 正式版评估是否用 Rust 重写核心"。

### 🟠 决策 2：sqlite-vec 的成熟度

sqlite-vec 是 Alex Garcia 的扩展项目，目前还比较年轻。几个实操注意点：
- 它的 HNSW 索引是否支持增量插入/删除？需要实测。
- 在大规模数据（10 万+向量）下的性能表现？
- 与 FTS5 的 content sync 触发器是否有冲突？

**建议**: MVP 阶段先用 sqlite-vec 的暴力搜索模式（数据量小时够用），10K 记忆以上再切 HNSW。在 05 里加一条"sqlite-vec 风险项"。

### 🟠 决策 3：FTS5 中文分词

05-architecture 提到"中文分词通过 ICU 或 simple tokenizer + jieba 分词预处理"。这有两种实现路径：
1. **入库前分词**: 写入时用 jieba 分词，把分词结果（空格分隔）存入 FTS5。查询时也先分词再查。
2. **自定义 tokenizer**: 编译带 ICU 的 SQLite，或用 fts5 custom tokenizer。

**我的判断**: 路径 1 更简单可控。Rust/Go 都有 jieba 绑定。

**建议**: 05 里明确选路径 1，记录为设计决策。

### 🟠 决策 4：MCP stdio 模式的 daemon 通信

05 的 MCP 模式 A 描述了 `steward mcp` 子进程通过 Unix Socket 连接 daemon。这意味着：
- daemon 需要监听一个 Unix Socket（除了 REST 的 TCP 端口）
- `steward mcp` 是一个瘦桥接进程

**需要明确**: Unix Socket 路径是什么？建议 `~/.steward/steward.sock`。

---

## 五、DDL 技术问题

### 🔴 问题 6：SQLite 不支持 FULL OUTER JOIN

07 的快照对比 SQL 用了 `FULL OUTER JOIN`，但 **SQLite 不支持 FULL OUTER JOIN**（3.39.0 开始支持，但需要确认目标版本）。

**修复**: 改为 `LEFT JOIN` + `UNION` + `LEFT JOIN`（反向），或确保 SQLite 版本 ≥ 3.39.0。

### 🟡 问题 7：memories 表 TEXT 主键 vs INTEGER rowid

`memories` 表用 `TEXT PRIMARY KEY`（UUID hex），但 FTS5 的 `content_rowid=rowid` 依赖整数 rowid。用 TEXT PK 时，SQLite 会自动分配一个整数 rowid，这个 rowid 不暴露在 API 里但需要保证稳定（不会因 VACUUM 变化）。

在 WITHOUT ROWID 表中不会有这个问题（当前不是 WITHOUT ROWID），所以应该安全。但值得在 07 里加一条注释说明。

### 🟡 问题 8：assets_vec 缺失

memories 有 `memories_vec` 向量表，但 assets 没有对应的向量表。03-feature-spec 的 AssetEntry 定义了 `embedding: vector(768)`，06 的 `steward_asset_search` 也支持语义搜索。

**修复**: 07 补上 `assets_vec` 虚拟表，类似 `memories_vec`。

---

## 六、产品手册补充建议

### 🟡 缺少"Agent 集成最佳实践"章节

04 只说了"怎么连"，没说"连上后 Agent 该怎么用"。需要一段最佳实践：

> **推荐使用模式:**
> 1. 新会话开始时，调用 `steward_brief` 获取上下文
> 2. 对话中遇到决策/经验/任务时，调用 `steward_memory_add` 记录
> 3. 需要找文件/脚本时，调用 `steward_asset_search`
> 4. 需要知道环境时，调用 `steward_env_show` 或 `steward_env_service`
> 5. 任务中断时，用 `steward_memory_add --type task` 保存进度

### 🟡 缺少 system prompt 集成示例

Agent 使用 steward 的前提是 system prompt 里告诉 Agent "你有一个叫 steward 的工具"。需要给出一段推荐的 system prompt 片段。

---

## 七、优先级修改建议

| 调整 | 原级别 | 建议级别 | 理由 |
|------|--------|----------|------|
| FC-05 变更摘要 | P1 | **P0** | `steward brief` 的"最近变更"部分依赖它，而 brief 是 MVP 核心价值 |
| FL-03 手动关联 | P1 | P1 不变 | OK |
| FE-07 快照对比 | P1 | P1 不变 | OK |
| REST API | P1 | **P0** | CLI 和 MCP 已覆盖 Agent 和人工场景，REST 可以降 P1，但如果要支持 Web UI 或外部脚本则需 P0。我建议保持 P1，MVP 只做 MCP+CLI |

---

## 八、总结：14 个 Action Item

| # | 级别 | 问题 | 涉及文件 | 动作 |
|---|------|------|----------|------|
| 1 | 🔴 | memories 缺 last_accessed/access_count | 07 | 补 DDL 字段+索引 |
| 2 | 🔴 | 资产类型枚举不一致 | 03, 06, 07 | 统一为 07 版本 |
| 3 | 🔴 | FULL OUTER JOIN 在 SQLite 的兼容性 | 07 | 改写 SQL 或注明版本要求 |
| 4 | 🟡 | 搜索公式不一致 (线性 vs RRF) | 03, 05 | 统一为 RRF |
| 5 | 🟡 | 配置格式不一致 (YAML vs TOML) | 02 | 改为 TOML |
| 6 | 🟡 | MCP 工具命名不一致 | 03 | 统一为 06 版本 |
| 7 | 🟡 | assets_vec 向量表缺失 | 07 | 补 DDL |
| 8 | 🟡 | 记忆写入策略未定义 | 03 或 04 | 增加章节 |
| 9 | 🟡 | brief 缺 max_tokens 参数 | 06 | 补参数 |
| 10 | 🟡 | Agent 集成最佳实践缺失 | 04 | 补章节 |
| 11 | 🟡 | system prompt 示例缺失 | 04 | 补示例 |
| 12 | 🟠 | Rust vs Go 决策 | 05 | 标注 MVP 用 Go |
| 13 | 🟠 | sqlite-vec 成熟度风险 | 05 | 补风险项 |
| 14 | 🟠 | FTS5 中文分词方案 | 05 | 明确选入库前分词 |

**修复顺序**: 先修 🔴（3 个不一致会导致实现时踩坑），再补 🟡（8 个遗漏/增强），🟠 作为设计讨论待定。

---

*Review 完成。等你确认后我逐一修复。*
