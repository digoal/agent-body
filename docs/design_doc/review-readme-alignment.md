# Agent Host Steward — README 核心设计对齐专项 Review 报告

> 审查基准：README.md 核心设计宗旨与原则  
> 审查对象：`docs/design_doc/` 全量交付件（01-scenarios ~ 07-data-model、init.sql、review.md）  
> 审查时间：2026-11-15  
> 审查人：digoal 德哥

---

## 一、审查背景与基准对照

README.md 中关于 **Agent Host Steward** 的最关键定义：

> **宿主环境大管家：驻留在 Agent 宿主机中，管记忆、管素材、管软件环境、管一切。**  
> 这不是为让 Agent 做好某一次任务、一次协作而生的产品，而是让 Agent 更清晰的了解自己所处的环境，更好的完成任务和协作。  
> 可以把它理解成 Agent 所处的宿主机上的一切动态与静态对象（文件、进程、文件夹等）元数据 + 对象位置的大管家，甚至还应该包括建议（例如，如果会产生一个新的项目、文件，这些对象应该放哪里）。  
> 管理对象也应该有颗粒度、有白名单黑名单的设计，不应该一股脑全生成对象元数据（语义(可采用 desc kv/json 存储，至少应该知道它们是干什么的、什么时候创建的、修改的、谁的、保密级别等等)、全文检索、bm25、标量、图），例如白名单包含的文件夹应该细到最深层，黑名单则只到当前目录层级。

经过对 `docs/design_doc/` 现存 7 份文档及 `init.sql` 的拉网式对照，我们发现：**现有设计体系架构扎实、技术选型（Go + SQLite + vec + FTS5 + MCP）务实，但在核心产品理念与 README 的关键要求之间，存在若干重大偏离与关键功能缺失。**

---

## 二、核心偏离与重大缺失清单

### 🔴 偏离与缺失 1：对象位置管理与智能放置建议（Placement Advice）完全缺失

- **README 明确要求**：
  > “可以把它理解成 Agent 所处的宿主机上的一切动态与静态对象（文件、进程、文件夹等）元数据 + 对象位置的大管家，**甚至还应该包括建议（例如，如果会产生一个新的项目、文件，这些对象应该放哪里）**。”
- **现状审查**：
  1. **场景缺失**：`01-scenarios.md` 现有的 26 个场景全部偏向“被动查历史、查环境、查已有资产”，**没有一个场景描述“Agent 要新建一个项目/写一个临时脚本/放一份数据，询问大管家该放哪里”**。
  2. **需求与功能缺失**：`02-requirements.md` 与 `03-feature-spec.md` 没有任何关于“位置建议（Placement Suggestion / Directory Conventions）”的需求编号与功能设计。
  3. **API 缺失**：`06-api-spec.md` 中没有类似 `steward_placement_suggest` 或 `steward_path_recommend` 的接口。
  4. **数据模型缺失**：`07-data-model.md` 和 `init.sql` 没有存放宿主机目录规划、分类规则与路径建议策略的表（如 `placement_rules`）。
- **严重性**：**致命（Critical）**。这直接导致 Steward 只能做“事后翻旧账的索引器”，而无法承担“规范宿主机秩序的大管家”。

---

### 🔴 偏离与缺失 2：纳管颗粒度与白名单/黑名单机制严重偏离，存在“一股脑生成”风险

- **README 明确要求**：
  > “管理对象也应该有颗粒度、有白名单黑名单的设计，**不应该一股脑全生成对象元数据**（语义...、全文检索、bm25、标量、图），**例如白名单包含的文件夹应该细到最深层，黑名单则只到当前目录层级**。”
- **现状审查**：
  1. **“一股脑全量生成”**：`03-feature-spec.md` 2.1 与 `06-api-spec.md` 2.9 中，`steward_asset_scan` 仅有单一的 `depth` 参数。扫描到的所有文件均无差别触发：插入 assets_fts、异步计算向量 embedding、尝试打标签。这在面对大型工程、编译产物、第三方库（如 `node_modules`）时会导致本地资源被彻底打爆。
  2. **黑名单理解严重失真**：现有设计把黑名单等同于 `exclude_patterns`（见 `04-product-manual.md`），即“遇到就直接跳过不理”。
     - **README 的真正智慧**：**黑名单不是视而不见，而是“只到当前目录层级”！**
     - 例如 `/root/.cache`、`/root/secret_vault` 或系统只读目录，作为黑名单对象，Steward 依然需要知道**“这里存在一个目录对象、谁的、多大、何时修改、保密级别”**，但**坚决不递归下钻、不解析子文件、不提取全文与向量**！
  3. **缺少扫描分级策略（Granularity Policy）**：没有配置不同路径层级的元数据生成丰富度（Deep / Shallow / Restricted）。
- **严重性**：**高危（High）**。不仅性能与存储易失控，且违反了大管家对宿主机全景感知的完整度（黑名单目录也是宿主机静态对象，不能在全景中凭空消失）。

---

### 🔴 偏离与缺失 3：元数据模型与属性结构缺失（Desc KV/JSON, Owner, 保密级别, 文件夹实体）

- **README 明确要求**：
  > “语义(可采用 desc kv/json 存储，至少应该知道它们是干什么的、什么时候创建的、修改的、谁的、保密级别等等)、全文检索、bm25、标量、图”
- **现状审查**：
  1. **缺少 `owner`（归属主体）**：现有的 `assets`、`memories` 表完全没有记录对象属于谁（哪个 Agent、用户、或系统进程）。
  2. **缺少分级保密机制**：现有 `assets` 表仅有一个简单的 `sensitive INTEGER DEFAULT 0` 布尔位，缺少多级保密级别（`public / internal / confidential / secret`），无法实现基于密级的访问控制与脱敏生成。
  3. **缺少结构化描述 `desc_kv` / `metadata JSON`**：现有只有平坦的 `description TEXT`。无法结构化存储“是干什么的、业务意图、适用场景、依赖约束”等关键 KV。
  4. **文件夹实体（Directory / Folder）被排除**：
     - 在 `07-data-model.md` 的 `assets` 表中，`type` 枚举为：
       `CHECK (type IN ('project','script','document','dataset','config','media','other'))`
     - **没有 `directory` 或 `folder`！** 之前的一致性 Review 将 `directory` 作为冗余项移除了，导致黑名单中“只记录到当前目录层级”的文件夹对象根本没有合法的类型可以入库！
- **严重性**：**高危（High）**。元数据基础承载能力不足，直接影响后续的标量过滤与混合搜索。

---

### 🟡 偏离与缺失 4：动态与静态对象管理失衡，动态进程对象严重窄化

- **README 明确要求**：
  > “可以把它理解成 Agent 所处的宿主机上的一切动态与静态对象（文件、进程、文件夹等）元数据 + 对象位置的大管家”
- **现状审查**：
  1. 现有的设计把静态对象局限在代码文件/文档（`assets`），而把动态对象完全窄化成了 **`services`（系统守护服务，如 PostgreSQL、Redis）**。
  2. **普通的动态进程对象管理缺失**：
     - 宿主机上由 Agent 启动的后台 Python 任务、临时数据处理脚本、长期跑测的进程、并发工作流 Worker、动态端口监听，根本没有被作为动态对象纳入生命周期跟踪。
     - 导致 Agent 依然无法直接回答：“当前主机上还有哪些 Python 任务在跑？它们占了多少 CPU/显存？是谁发起的？”
- **严重性**：**中高（Medium-High）**。宿主环境动态性未完整覆盖。

---

### 🟡 偏离与缺失 5：定位重心偏离 —— 从“宿主大管家”滑向“传统 Agent 记忆库”

- **README 明确要求**：
  > “这不是为让 Agent 做好某一次任务、一次协作而生的产品，而是让 Agent 更清晰的了解自己所处的环境，更好的完成任务和协作。”
- **现状审查**：
  1. 现有的 7 份文档中，第一大模块依然是传统的 **Memory 管理**（记忆衰减算法、强化算法、相似合并、会话提取、归档），耗费了大量篇幅。
  2. 这导致整体产品形态更像一个“增强版 Mem0 / Letta + 文件扫描器”，而削弱了作为**“宿主机系统资源与对象位置权威总管”**的特色与统治力。
- **纠偏建议**：记忆是环境上下文的一部分，但不应喧宾夺主。必须以“宿主环境全景（动态+静态对象+位置建议）”为第一核心。

---

## 三、系统性整改与对齐方案

### 1. 新增核心能力：Placement Engine（对象放置与位置建议）

#### 1.1 新增场景（写入 `01-scenarios.md`）
- **S-27: 新建项目/脚本/素材时的位置建议**
  - **触发**：Agent 准备新建一个抓取脚本、克隆一个 GitHub 库、或者写一篇分析笔记。
  - **交互**：Agent 调用 `steward_placement_suggest(type="script", name="fetch_sec.py", intent="抓取美股财报")`。
  - **Steward 返回**：推荐路径 `/root/new/shared_proj/fetch_sec/`，命名规范，以及同类项目推荐；同时提醒该目录现有规范与关联依赖。

#### 1.2 新增 API（写入 `06-api-spec.md`）
- `steward_placement_suggest`:
  - **入参**:
    ```json
    {
      "object_type": "project | script | document | dataset | temporary | note",
      "name": "string",
      "intent": "string",
      "tags": ["string"],
      "security_level": "public | internal | confidential | secret"
    }
    ```
  - **返回**:
    ```json
    {
      "suggested_path": "/root/new/shared_proj/sec_fetcher/",
      "reason": "命中规则: 长期复用脚本/工具应收敛至 shared_proj 目录",
      "naming_convention": "snake_case 目录，包含 README.md 与入口脚本",
      "existing_conflicts": false,
      "recommended_permissions": "0755",
      "security_guideline": "敏感配置请外置至环境变量，严禁落盘硬编码"
    }
    ```

#### 1.3 核心数据表扩展（写入 `07-data-model.md` 与 `init.sql`）
```sql
CREATE TABLE placement_rules (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    rule_name       TEXT NOT NULL,
    pattern_type    TEXT NOT NULL,               -- project, script, note, tmp, dataset, src
    target_base_dir TEXT NOT NULL,               -- e.g. /root/new/shared_proj/
    naming_pattern  TEXT,                        -- e.g. "{name}/"
    description     TEXT,
    priority        INTEGER DEFAULT 100,
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);
```

---

### 2. 改造扫描与元数据体系：颗粒度分级与白黑名单机制

#### 2.1 三级纳管颗粒度体系（Granularity Policy）

| 级别 | 适用对象 | 扫描行为 | 元数据生成范围 |
|------|----------|----------|----------------|
| **Deep Tier (白名单)** | 核心工作区、工具库、知识库（如 `/root/new/shared_proj/`, `/root/new/work/`） | 细到最深层，递归下钻遍历所有子文件与子目录 | 基础标量 + `desc_kv` 详细解析 + 全文 FTS5 + BM25 + 向量 Embedding + 关联图谱 (Links) |
| **Shallow Tier (黑名单/阻断区)** | 系统缓存、依赖包、私密目录（如 `node_modules/`, `.cache/`, `/root/secrets/`, `/proc/`） | **只到当前目录层级**（记录该文件夹本身对象，绝不递归深入遍历其子项） | 仅记录该目录的标量元数据（路径、目录大小、最后修改、所有者、保密级别），**禁止全文、禁止向量、禁止图谱** |
| **Restricted Tier (普通/受限区)** | 通用源码克隆、大型静态数据集（如 `/root/new/src/`） | 扫描到项目根目录或指定深度（默认深度 2） | 记录标量 + README/主要入口提取基本 summary，支持关键词搜索，跳过大文件向量化 |

#### 2.2 扫描策略配置表
```sql
CREATE TABLE scan_policies (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    path_pattern    TEXT NOT NULL UNIQUE,        -- 路径匹配通配符或前缀
    tier            TEXT NOT NULL CHECK (tier IN ('deep','shallow','restricted')),
    max_depth       INTEGER DEFAULT -1,          -- deep 为 -1(无限制), shallow 强制为 0
    enable_fts      INTEGER DEFAULT 1,
    enable_vec      INTEGER DEFAULT 1,
    enable_graph    INTEGER DEFAULT 1,
    notes           TEXT
);
```

---

### 3. 数据模型全面升级（`assets` 与 `dynamic_objects`）

#### 3.1 `assets` 表结构扩充
```sql
-- 将 directory 重新纳入合法类型，并增加 owner、security_level、desc_kv
ALTER TABLE assets ADD COLUMN owner TEXT DEFAULT 'user';
ALTER TABLE assets ADD COLUMN security_level TEXT DEFAULT 'internal' 
    CHECK (security_level IN ('public', 'internal', 'confidential', 'secret'));
ALTER TABLE assets ADD COLUMN desc_kv TEXT DEFAULT '{}'; -- JSON 结构化描述键值
ALTER TABLE assets ADD COLUMN scan_tier TEXT DEFAULT 'deep' 
    CHECK (scan_tier IN ('deep', 'shallow', 'restricted'));
```
*(在新建表定义中，将 `type` 约束扩充为：`CHECK (type IN ('directory','project','script','document','dataset','config','media','other'))`)*

#### 3.2 动态对象管理扩展：统一进程与动态运行时
在现有 `services` 基础上增加或扩充为 `dynamic_objects`（或增加 `active_processes` 视图/表）：
```sql
CREATE TABLE active_processes (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    pid             INTEGER NOT NULL,
    name            TEXT NOT NULL,
    command         TEXT,
    owner           TEXT DEFAULT 'user',         -- 哪个 Agent 会话或系统用户启动
    associated_path TEXT,                        -- 运行脚本/工程路径 (关联 assets.path)
    listening_ports TEXT DEFAULT '[]',           -- JSON 端口数组
    status          TEXT DEFAULT 'running',
    cpu_pct         REAL,
    mem_bytes       INTEGER,
    gpu_allocated   INTEGER DEFAULT 0,
    started_at      TEXT,
    last_seen_at    TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);
```

---

## 四、文档修订对照实施表

| 文档 | 需修订章节 | 具体修改行动 | 优先级 |
|------|------------|--------------|--------|
| **01-scenarios.md** | 场景清单 | 补充 S-27（新项目/文件放置建议）、S-28（黑名单目录浅层纳管与防穿透）、S-29（普通进程与并发资源占用感知） | P0 |
| **02-requirements.md** | 功能需求 | 增加 FP（Placement 需求）FM/FA/FE 补全 owner、security_level、desc_kv 验收标准 | P0 |
| **03-feature-spec.md** | Assets & Env 模块 | 1. 恢复 `directory` 类型；<br>2. 增加三级颗粒度扫描策略规范；<br>3. 详细设计 Placement 引擎与规则集 | P0 |
| **05-architecture.md** | 架构设计 | 增加 Placement Advisor 组件架构，细化 FTS5 + sqlite-vec + 标量属性的三路混合检索流 | P1 |
| **06-api-spec.md** | API 规范 | 新增 `steward_placement_suggest`，扩展 `steward_asset_scan` 策略参数，扩展 `steward_asset_search` 支持标量过滤（owner, security_level） | P0 |
| **07-data-model.md** | 数据模型 | 更新 DDL：`assets` 表类型与字段扩展、新增 `placement_rules` 表、新增 `scan_policies` 表、新增 `active_processes` 表 | P0 |
| **init.sql** | 初始化 SQL | 同步应用上述 DDL 变更，更新初始预置规则（白名单、黑名单、放置规则） | P0 |

---

## 五、结论与评审意见

**总结**：当前文档工程是一套完成度很高的“方案”，但此前**稍稍走偏了方向——过于偏向传统 Agent 对话记忆，而淡化了“宿主环境与对象位置大管家”的根本使命**。

通过补充**「对象位置建议（Placement Engine）」**、**「白名单深入到底/黑名单止于当层（Granularity Tiering）」**以及**「结构化 desc kv + owner + 保密级别」**这三根支柱，Agent Host Steward 将真正回归 README.md 所宣示的终局定位：**成为宿主机上一切动态与静态对象无死角、知分寸、懂规则的超级管家**。
