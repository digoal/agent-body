# Agent Host Steward (宿主环境大管家)

> **三位一体架构：宿主机事实图谱为核心护城河 · 用户偏好规约为协作准则 · 记忆模块开放插拔成熟生态**  
> 驻留在 Agent 宿主机中的轻量级系统认知底座（Rust Edition · 单二进制交付）。

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.99.0-orange.svg)](https://www.rust-lang.org/)
[![SQLite](https://img.shields.io/badge/SQLite-FTS5_Bundled-green.svg)](https://sqlite.org/)
[![MCP](https://img.shields.io/badge/Protocol-MCP_stdio-purple.svg)](https://modelcontextprotocol.io/)

---

## 什么是 Agent Host Steward？
[演讲 PPT](https://htmlpreview.github.io/?https://github.com/digoal/agent-steward/blob/main/steward-deck.html)  
  
[路演 PPT](https://htmlpreview.github.io/?https://github.com/digoal/agent-steward/blob/main/presentation.html)  
  
市面上有大量专注于“跨会话记忆”或“对话摘要”的开源项目，但它们没有解决一个更底层、更致命的痛点：**Agent 在 Linux 宿主机上干活时，对物理世界是完全“两眼一抹黑”的盲人**：
- 它不知道机器上其实装了 `pdftotext`，却反复折腾 pip 安装第三方库并报错；
- 它不知道 8080 端口已被生产进程占用，直接 bind 导致崩溃；
- 它不知道修改一个通用配置文件，会导致后台依赖它的 systemd 服务挂掉；
- 它不知道哪个 cron 任务在后台定时跑脚本、修改数据库；
- 它不知道用户偏好把脚本放在哪里，随手乱丢文件污染工作区。

**Agent Host Steward（大管家）** 将关注点彻底解耦，确立**三位一体架构**：

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                               AI Agents / Clients                                      │
│             (Antigravity, Cursor, Claude Code, Aider, CLI, Cron Workers)               │
└───────────────────────────────────────────┬────────────────────────────────────────────┘
                                            │ MCP (stdio/SSE) / Unix Socket / CLI
┌───────────────────────────────────────────▼────────────────────────────────────────────┐
│                    Agent Host Steward (宿主环境大管家)                                  │
├────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                        │
│ ┌────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ 模块 1: 宿主机事实图谱 (Host Reality Graph) —— 【核心护城河 · 零幻觉只读体系】       │ │
│ │                                                                                    │ │
│ │ • 工具能力语义注解: 识别 $PATH 工具能力(PDF提取/音视频转码/JSON处理)与推荐调用参数 │ │
│ │ • 运行时进程与端口: 内核 /proc 解析占用(PID/进程名/启动命令/Cwd)，端口防撞预警    │ │
│ │ • 系统自动化依赖拓扑: systemd 配置文件依赖反查、crontab 后台定时偷跑任务排查      │ │
│ │ • 源码与资料库编目: 自动发现 Git 仓库(CodeGraph 状态)、本地权威文档与离线知识库   │ │
│ │ • 变更审计与依赖图: inotify 实时变更流感知、逆向依赖爆炸半径推演                  │ │
│ │                                                                                    │ │
│ │ ── 统一只读查询语义接口: steward_fact_query / steward_port_check / links_impact ──│ │
│ └────────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                        │
│ ┌────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ 模块 2: 用户偏好与规约引擎 (Preference & Governance) —— 【协作灵魂与规约准则】       │ │
│ │                                                                                    │ │
│ │ • 资产放置规则 (Placement): 推荐基准物理路径(/root/new/shared_proj等)、强制命名规范│ │
│ │ • 工程与习惯规约 (Conventions): 优先使用 codegraph 而非 grep、严禁编造数据、北京时区│ │
│ │ • 纠偏自演化学习 (Rule Learner): 从用户人工纠错行为中自动归纳、沉淀新偏好规则      │ │
│ └────────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                        │
│ ┌────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ 模块 3: 跨会话记忆与经验 (Memory & Context) —— 【开放生态可插拔 · 拒绝重复造轮子】 │ │
│ │                                                                                    │ │
│ │ • 通用 Memory Provider SPI 抽象标准                                                │ │
│ │ • 内置极简引擎: Local SQLite FTS5 + 本地向量混合检索 (零网络依赖，开箱即用)         │ │
│ │ • 成熟开源生态桥接器: 一行配置外挂 Mem0 / Letta / PowerContext / Zep 成熟方案       │ │
│ └────────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                        │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 典型交互场景 (Zero Hallucination Examples)

### 场景 1：查询机器上的工具能力（问“有没有能处理 PDF 的东西？”）
- **Agent 请求**: `steward_fact_query(type="capability", intent="处理 PDF 提取纯文本")`
- **大管家返回**:
  > “有 `pdftotext`，版本 24.02，绝对路径 `/usr/bin/pdftotext`。**推荐用法**：`pdftotext -layout input.pdf output.txt`（保持原始排版，性能极快，无需 pip 安装 Python 库）。备选工具：`pdftoppm`（转图片）与 Python `pypdf`。”

### 场景 2：端口排障与防撞（问“8080 端口被谁占了？”）
- **Agent 请求**: `steward_port_check(port=8080)`
- **大管家返回**:
  > “8080 端口已被占用！PID: `18493`，进程名: `python3`，启动命令: `uvicorn main:app --port 8080`，工作目录: `/root/new/work/api-service`，已运行 3 天。**建议**：换用 8081 或 8082 端口。”

### 场景 3：资产放置偏好（问“我要写个抓取脚本，该放哪里？”）
- **Agent 请求**: `steward_placement_suggest(type="script", name="sec_crawler", intent="美股财报抓取")`
- **大管家返回**:
  > “依主人规范，复用脚本应放置在 `/root/new/shared_proj/sec_crawler/`；临时文件请放 `/root/new/tmp/`；严禁散落在当前目录。遵循蛇形命名规范。”

---

## 顶层思想与全新演进路线

- 🌱 **[00-comparative-advantage-origin.md](idea_doc/00-comparative-advantage-origin.md)**：项目缘起：从硅谷园区物业、高管秘书到 AI Agent 的「比较优势」协同
- 💡 **[03-modular-steward-architecture.md](idea_doc/03-modular-steward-architecture.md)**：大管家三位一体架构设计深度思考（事实、偏好与记忆的分野与统一）
- 🗺️ **[09-modular-roadmap.md](design_doc/09-modular-roadmap.md)**：最新模块化演进路线图（Phase 1 事实图谱护城河 → Phase 2 偏好自演化 → Phase 3 记忆生态插拔 → Phase 4 终极智能中枢）
- 📐 **[05-architecture.md](design_doc/05-architecture.md)**：三模块分层架构与系统规格说明书
- 📖 **[03-feature-spec.md](design_doc/03-feature-spec.md)**：三模块功能规格详细定义
- 📊 **[10-benchmark-spec.md](design_doc/10-benchmark-spec.md)**：实证 Benchmark 对照评测与生产效能存证账本设计规格说明书（双轨闭环确保使用效果 100% 有据可查）

---

## 三维角色全景解读 (Architecture & Guides)

针对不同技术角色的深层设计、二次开发与生产实操，发布于 **[`arch/`](arch/)** 目录：

- 🏛️ **[01-for-architects.md](arch/01-for-architects.md)**（面向架构师与技术决策者）：
  - 三位一体系统认知模型深度拆解；
  - 存储中枢 Facade + 统一 SPI 抽象（SQLite / PG / DuckDB 双向热迁移与增量同步）；
  - 多 Agent 协作锁与层级互斥传播算法（Advisory Lock + 祖先后代路径检测）；
  - 工业级 NFR 指标（单二进制交付、冷启动 `<50ms`、空闲内存 `<25MB`）与递弱代偿边界。
- 🛠️ **[02-for-developers.md](arch/02-for-developers.md)**（面向研发工程师与贡献者）：
  - 源码 23 大模块全景职责与依赖拓扑速查；
  - 核心扩展三大 How-To（添加原生 MCP 工具、实现新存储后端 SPI、新增环境预检项）；
  - 189+ 项自动化测试套件与本地开发流；
  - 零 Panic 原则、四层贯通与断点续传检查点规范。
- 🧭 **[03-for-users.md](arch/03-for-users.md)**（面向人类主理人与 AI Agent 智能体）：
  - 大管家核心心智模型与物理世界哨兵定位；
  - 典型作业六部曲闭环 SOP（开工简报 → 放置问询 → 端口防撞 → 爆炸半径推演 → 协作锁防踩踏 → 完工结清）；
  - 三大接入方式（CLI 终端交互、MCP 智能体挂载、Web 可视化大屏）；
  - 常见问题排查与速查表 (FAQ)。

---

## 官方使用手册 (User Manual)

完整的手册发布于 **[`manual/`](manual/)** 目录：

- 🚀 **[01-getting-started.md](manual/01-getting-started.md)**：快速上手、环境依赖准备与首次编译运行
- 🧠 **[02-core-concepts.md](manual/02-core-concepts.md)**：核心理念：大管家定位、黑白名单浅层阻断、放置建议哲学
- 📖 **[03-cli-reference.md](manual/03-cli-reference.md)**：全量 CLI 命令行实操字典（全子命令 + 真实输出）
- 🔌 **[04-mcp-integration.md](manual/04-mcp-integration.md)**：Agent 接入指南：40 个原生 MCP 工具与 Claude/Cursor 配置
- 🔄 **[05-daemon-and-watcher.md](manual/05-daemon-and-watcher.md)**：后台常驻守护服务、inotify 300ms 防抖与 systemd 自启
- ⚡ **[06-advanced-features.md](manual/06-advanced-features.md)**：v3.1 进阶特性：断链智能修复、放置规则自演化、宿主资源 GC、记忆生命周期与 TUI 看板
- 🛠️ **[07-troubleshooting.md](manual/07-troubleshooting.md)**：故障诊断与常见问题 (FAQ)
- 🌐 **[08-web-dashboard.md](manual/08-web-dashboard.md)**：Web UI 与可视化监控大屏：基于浏览器的轻量级大管家拓扑图与协作锁监控看板 (单二进制 100% 离线自包含)
- 🪝 **[09-hooks-integration.md](manual/09-hooks-integration.md)**：HOOK 驱动集成引擎：事件总线、Post-Hook 全自动回写、客户端 Hook 生成器、Session 生命周期与 Pipeline 编排
- 📊 **[10-benchmark-and-roi.md](manual/10-benchmark-and-roi.md)**：实证 Benchmark 对照评测与生产效能存证账本（8 大 A/B 负载 + 5 大微基准 + PR-01~08 溯源流水）

---

## 快速上手 (Quick Start)

> 💡 **详细部署与依赖全景**: 参见完整的 [INSTALL.md](INSTALL.md)（含多发行版依赖矩阵、Docker 部署、Systemd 服务托管与故障速查表）。

### 1. 编译安装
```bash
# 确保 Rust 工具链生效
export PATH="/root/.cargo/bin:$PATH"

# 编译 release 生产二进制
cargo build --release

# 查看运行状态 (自动完成 ~/.steward 数据初始化)
./target/release/agent-steward status
```

### 2. 常用 CLI 命令一览

```bash
# 1. 开工获取系统硬件、活跃任务、最近资产与长期记忆简报
./target/release/agent-steward brief

# 2. 准备写一个爬虫脚本？先问大管家放哪里
./target/release/agent-steward place suggest --type script --name sec_fetcher --intent "抓取美股财报"

# 3. 准备启动一个测试服务？先探测 8080 端口是否已被占用 (防撞预警)
./target/release/agent-steward env port 8080

# 4. 计划改动公共模块？先推演上下游依赖与多跳爆炸半径
./target/release/agent-steward links impact /root/new/shared_proj/test.py

# 5. 跨会话搜索过去的决策与排障经验 (FTS5 + Ollama 向量混合 RRF 检索)
./target/release/agent-steward memory search "数据库连接池故障应急" --hybrid

# 6. 本次会话完成？清理 /root/new/tmp/ 临时测试文件并沉淀工单纪要进长期记忆
./target/release/agent-steward close session-20261115-01 --clean-temp

# 7. 启动基于浏览器的可视化监控大屏 (事实拓扑 + 多 Agent 协作锁看板)
./target/release/agent-steward web --port 8765
```

### 3. Agent MCP 挂载配置

在 Claude Desktop 或 Antigravity MCP 配置文件中添加：
```json
{
  "mcpServers": {
    "agent-steward": {
      "command": "/root/new/work/agent-steward/target/release/agent-steward",
      "args": ["mcp"]
    }
  }
}
```

## 项目结构
- arch : 面向架构师、开发者、用户的三维角色全景解读文档
- design_doc : 产品功能设计文档
- idea_doc : 产品起初的想法
- manual : 产品手册
- markdown : 对外发布的文章、博客
- releases : 发版说明文档
- skills : 与本产品相关的对接 AI Agent 的 SKILL 包
- src : 源代码
- src_doc : 本项目的开发文档
- tests : 测试代码
- README.md : 本文件
- presentation.html : 产品路演 PPT
- steward-deck.html : 产品交流 PPT
  
## 核心设计原则与约束 (Design Principles & Constraints)

### 产品定位与边界

1. **服务对象准则**：
   - 产品的核心用户是 **AI Agent**，而不是给人日常使用。数据输入输出必须以"极低认知负载、零幻觉只读提取、语义自解释"为第一标准。

2. **边界与递弱代偿哲学**：
   - 产品的边界必须搞清楚，不是啥都做，专注做宿主环境认知的原子功能或分子功能，做专做精做深。如同王东岳所说的"递弱代偿"，越基础的功能越被深度依赖；功能过度泛化会丧失生态定位。
   - 明确不做：不替代 Agent 编排框架（LangGraph/AutoGen/CrewAI）、不承担业务 LLM 推理、不替代容器沙箱隔离（Docker/gVisor）、不充当通用 OLTP 数据库、不替代密钥管理（Vault/1Password）。

3. **系统底座定位探讨**：
   - 本项目所承载的"宿主机事实感知、环境规约与长期记忆"三位一体能力，未来是否应演进为面向智能体时代的下一代 OS 原生内置标配功能。

### 数据治理

4. **事实与记忆的摄入取舍门禁**：
   - 记录事实与 memory 时必须有克制与取舍，绝不能不加筛选全盘记录。通过重要性评分与噪音过滤规则（构建缓存、临时文件、高频变动垃圾阻断），保障事实图谱与记忆库的高纯净度。

5. **实事求是、零捏造 (Ground Reality, Zero Simulation)**：
   - 所有暴露给 Agent 的事实必须源自宿主机真实物理状态（`/proc`、`/sys`、`/etc/systemd`、`socket`、`inotify`）。严禁在缺少证据的情况下推测或编造虚拟状态。

6. **颗粒度分级与白黑名单深度法则**：
   - **白名单目录**（如 `/root/new/shared_proj/`、`/root/new/work/`）：必须深度递归解析到底，构建完整元数据与全文索引。
   - **黑名单目录**（如 `node_modules/`、`.cache/`、`.git/`）：仅记录当前目录实体本身，严禁递归下钻或入库子文件 FTS/向量，防止数据库扫爆与敏感信息泄露。
   - **受限目录**（`restricted`）：受限深度扫描（如开源代码库仅索引 README 与入口文件）。

7. **动态进程过滤底噪**：
   - 进程纳管只抓取 Agent 会话任务、Python 脚本、构建编译 Worker 等应用进程；系统底噪（内核工作线程 `kworker`、systemd 基础进程等）通过白名单与启发式规则过滤。

### 安全与性能

8. **只读探测为主、破坏性操作强制干预**：
   - 垃圾清理（GC）默认强制 `dry-run` 预览，绝不静默硬删除；强制限定白名单目录。
   - Agent 执行动作前通过 `steward_hook_pre` 进行静态冲突预测；违规时给出确定性告警或提示。
   - 会话结清（`session_close`）提供可审计的结项清单。

9. **极速与轻量化 (Sub-Millisecond Response)**：
   - 核心 API 端到端响应必须在毫秒级以内（实测基准：放置决策 120 μs，图遍历 498 μs，FTS 检索 805 μs），不能成为 Agent 执行链路的性能瓶颈。
   - 常驻守护内存 < 30MB、CPU 空闲 < 0.5%。

### 工程开发规约

10. **运行依赖四层贯通约束**：
    - 必须充分考虑宿主机环境的物理差异。任何运行依赖与假定必须贯彻 **代码层面 × 配置层面 × 文档层面 × SKILL 层面** 四层贯通；启动时强制运行时预检，防止在未知宿主环境中静默崩溃或部署失败。

11. **开发原则**：
    - 本项目使用 AI Agent 开发，收到任务后，先进行任务分解。任务拆解要细致、要按工业级产品标准来要求功能设计/开发/测试/文档、要在任何时刻任何情况任务失败后可以"断点续传"。开发完成后，如有必要记得要及时更新手册、SKILL、MCP 等相关的文档或代码。要将 SKILL、手册、MCP 和代码一样当成一等公民来对待。

12. **零 Panic 原则**：
    - 在非测试代码中，严禁在生产路径直接使用 `unwrap()` 或 `expect()`。一律使用 `anyhow::Result` 或 `thiserror` 向上冒泡错误，保证服务 7×24 稳定运行。

13. **测试驱动与零警告**：
    - 新增功能必须在 `tests/` 或对应模块内提供自动化测试。
    - 任何提交前必须 `cargo check --tests` 通过零警告门禁。
    - MCP 工具与 CLI 子命令必须保持一一映射。

14. **断点续传检查点机制**：
    - 开发任何跨步骤大型特性时，必须在 `src_doc/progress-phase*.md` 建立检查点表格和 `src_doc/roadmap-phase*.md` 路线图。
    - 确保中断后任何新会话均能通过读取 `progress-phase*.md` 恢复上下文无缝继续。

15. **宏微联邦、不重复造轮子**：
    - 大管家负责宏观宿主拓扑（文件、服务、端口、定时任务间的依赖网络）。
    - 微观符号调用（函数、类、AST）下钻交给 CodeGraph，大管家通过 `CodeGraphBridge` 无缝桥接。
    - 记忆模块通过 SPI 抽象，拥抱成熟开源生态（Mem0/Letta/PowerContext），绝不重复发明轮子。

16. **黑白屏全能力双向同步规约 (Black/White-Screen Parity)**：
    - 黑屏功能（MCP 工具、SKILL 指导、CLI 子命令）与白屏功能（Web UI 工作台、REST API）必须保持 **1:1 双向同步**，严禁出现仅存在于单侧的孤岛功能。
    - 白屏端每次交互应同步镜像回显等价的 CLI 命令与 MCP JSON-RPC 请求体；并通过 `/api/v1/parity/matrix` 与自动化契约测试强制守卫 **100% 对齐率**。

17. **产品路演与场景演示内建规约 (Built-in Roadshow & Live Demo)**：
    - 从产品经理与市场营销推广视角，将高频痛点与核心护城河沉淀为 Web 端可交互的 **路演演示剧场（Roadshow Live Demo）**，与黑屏 `steward_demo_scenario` / `agent-steward demo` 同源演进。
    - 所有路演剧目必须由真实引擎驱动（严禁硬编码假数据），状态写入严格限定在 `demo:*` 隔离沙盒并支持一键无痕复位；同时将离线路演 PPT（`/deck`、`/presentation`）与 ROI 实时账单内建于单二进制 Web 服务中，随版本同步刷新。

18. **实证基准与效能有据可查规约 (Empirical Benchmark & Verifiable ROI Ledger)**：
    - 任何宣称的效能提升（Token 节省率、探索轮次缩减、微秒级响应延迟、故障/冲突拦截率、垃圾噪音阻断率）必须通过可复现的 **A/B 对照基准套件（Without Steward 裸奔基线 vs With Steward 管家赋能）** 与 **生产运行态效能存证账本（Effectiveness Telemetry Ledger）** 双轨实证，严禁无出处估算或主观拍脑袋指标。
    - 基准测试必须基于宿主机真实物理探针与 `bench:*` 隔离沙盒实跑（覆盖内核微基准延迟 SLA 与 8 大宏观业务负载）；每次评测与日常工具调用的真实耗时（`elapsed_us`）、Token 缩减量（`tokens_saved`）、探索轮次缩减（`rounds_saved`）及风险拦截事件（`hazard_intercepted`）均须携带确定性推导规则编号（`provenance_rule`）落库留痕，支持通过 CLI（`agent-steward bench`）、MCP（`steward_benchmark`）、Web UI 与 DuckDB 导出报告随时溯源核验与跨版本回归比对，确保大管家使用效果 100% 有据可查。

---

## 未来演进与待探讨 ISSUE (Roadmap & Open Issues)

1. **分布式/多机 Agent Steward 跨主机网状协调 (Gossip / Raft / HTTP Mesh)** `✅ Phase 18 已落地轻量 HTTP Peer Mesh`：
   - 当宿主环境扩展为多台机器构成的集群时，将协作锁与事实图谱提升为跨节点分布式网状协调。

2. **智能死锁检测与等待唤醒机制 (Wait-Queue / Push Notification)** `✅ Phase 17 已落地 WFG + Wait-Queue`：
   - 为排他锁提供可订阅的等待队列，当持锁 Agent 释放资源时，主动通知排队中的 Agent，避免轮询开销。

3. **跨平台原生适配深化 (macOS / Windows)** `✅ Phase 18 已落地 PlatformProbe 抽象`：
   - 当前以 Linux（/proc、inotify、Systemd）为首选核心运行环境，未来进一步拓展对非 Linux 平台的底层环境感知。
  
4. **管理范围的扩大** `✅ Phase 16 已落地 Ecosystem & Context 防爆路由`：
   - 还得帮 Agent 管 MCP, SKILL, 可以使用的外部服务等。因为 MCP、SKILL 多了之后啊，Context 容易被打爆，导致大模型注意力下降，影响模型生成质量。 管外部服务的目的则是 Agent 宿主机本机环境的延展，就像销售人员有一本外部联系人名单一样，随时可以按需来 call。
  
5. **Web UI 显示内容优化与交互重构** `✅ Phase 19 已落地 Web UI 2.0 五大工作台与真实拓扑`：
   - Web UI 显示的内容有点简陋，也不美观。需要重新设计一下。

6. **黑屏功能和白屏功能双向同步** `✅ Phase 21 已落地 39 工具 100% Parity 同步`：
   - 黑屏功能和白屏功能要同步，白屏主要体现在WEB端，黑屏体现在 MCP,SKILL,CLI 侧（已升格为开发规约 #16）。

7. **WEB 侧路演与对外分享演示剧场** `✅ Phase 22 已落地「🎬 路演演示剧场」与第 39 工具`：
   - 要在WEB侧额外设计更多演示功能，路演和对外分享时使用，从产品经理和市场营销推广的角度来挑选哪些场景需要被拿来演示（已升格为开发规约 #17）。

8. **元数据定义哪些 Key 与本地 Ollama 小模型自动标注** `✅ Phase 23 已落地三层正交元数据体系与 Ollama 异步双轨标注`：
   - 定义 **L1 通用语义核心 (`purpose`/`domain`/`capabilities`/`entry_point`/`usage_hint`) + L2 类型特化 Facet + L0 管线溯源 (`content_hash`/`annotation_status`/`annotation_model`)** 三层元数据 Key 体系；
   - 采用 **L1 Rust 微秒级确定性语法提取 + L2 本地 Ollama 小模型（`qwen3.5:0.8b`）脱敏异步精炼** 双轨流水线，配合 **Micro (~25 tok) / Standard (~85 tok) / Full (~180 tok)** 三级渐进式 Token 披露，彻底告别 `{"watcher_sync":true}` 空洞元数据且零云端 Token 消耗。

9. **「比较优势」立意源起与 PostgreSQL 调研痛点补完** `✅ Phase 24 已落地 Idea 起源篇与双 HTML 演说融合`：
   把为什么要做 agent-steward 项目的以下这些意境和萌芽态的想法，整理后补充到 Idea 文档和路演、演讲 HTML 里。
```
我看到硅谷 101采访一位硅谷创业者，这位创业者居然在办公区（负一楼）将近一周都沉浸在 coding 中，原因是这个园区的物业把创业者的琐事都办了，创业者足不出户，沉浸在主业 coding 中。
企业的高管基本上都会配置独立或共享的秘书，把琐事办掉，高管专一的干本职工作，发挥最大能效。
家庭也一样，很多家庭家庭主妇把老公的一切都包了，老公专心工作赚钱。
这些都是很好的互相搭配，每个人都把主要精力放在最本职、最专长的事情上，有个经济学术语叫比较优势。
Agent 呢，我用久了 Agent 后发现，Agent 缺个秘书或者说大管家。为什么呢？比如我经常用 Agent 调研 PostgreSQL 的代码、功能、或应用场景等，每次给 Agent 提交任务，它都要重新拉取代码、文档，然后编译，安装，验证等。其实本机已经有了环境，或以前已经下载好了代码或文档，素材都是现成的，但 Agent 不知道，如果能给 Agent 配个小秘书或大管家，先问小秘书，这事不就解决了吗？也不会浪费时间、浪费空间折腾。Agent 的效率高了，也省 token 了。
```

10. **路演与演讲 HTML「地层剖面 / 工程蓝图」重构与全功能自检** `✅ Phase 24 已落地 16 页路演版与 22 页演讲版双 HTML + Playwright 自检全绿`：
    路演文档不够美观，也没有配演讲稿，可以改进一下：
````
生成单个 html 格式的演说文件(文件名不要出现中文字符)，请都放到一个文件中包括图片你可以用 base64 嵌入。产生的 html 最重要的是内容的正确性、并且是图文并茂的。一图胜千言，图的目的是帮助受众理解内容(原理图、架构图、使用场景图、最佳实践图等等)。图你可以现场绘制或使用原文中的，图要求美观且易于理解，图中表达的任何模块或文字必须在视觉上无任何遮挡、出界、交叉等问题。整个 slide 还可增加一定的动画演示（得有输入、播放等配套按钮），但只在对受众理解内容有帮助时增加。html 中还可以添加一下快捷键，必含按键“目录 G、本页讲稿 N、全文讲稿 T、自动播放、全屏 F”，其它按键按需提供，全屏后按键也要在页面底部显示。讲稿内容要注重逻辑推演和实证，口语化，逻辑要连绵不绝。讲稿不要弹窗打开，而是采用页内开窗，按一下显示，再按一下关闭，循环，在页面右侧显示，显示讲稿时原页面内容矢量缩放到左侧展示，按 T 显示全部讲稿时自动定位到当前 page，右侧显示讲稿时翻页时也进行联动自动滚动右侧演讲稿到当前页面。
html 内容16比9的比例，采用适合阅读的视觉风格。
```
【视觉风格：地层剖面 / 工程蓝图】
浅色主题，深字浅底。
色系：底 #F7F6F2 暖纸白、次级底 #F1EFE9；墨 #1B2B33、次级字 #46585F、
弱化字 #89938F、描边 #D9D5CB。主色 深靛青 #245A6B（浅 #E5EFF0），
强调 陶橙 #BE653B（浅 #F7E9DE），收益 青绿 #2F7A66，提示 金 #AE8A3E。
地层七阶（深→浅）#1E4B5A #2E6373 #47808D #74A5AD #A4C4C6 #CFE0DC #EBF1EE，
越深＝越底层。语义：靛青＝结构/机制，陶橙＝矛盾/代价/承接，青绿＝正确动作，金＝过渡。
字体：标题宋体 Songti SC / Noto Serif CJK SC，正文 PingFang SC / Noto Sans CJK SC，
数字页码 SF Mono；中文标点用「」『』——。
版式：16:9 固定舞台 1600×900，等比缩放 + letterbox，禁止响应式重排；左边距 64px，
眉标 13px/字距3.4px，标题 44px 宋体，导语 19px，1px 分隔线，图区 1400×430，
底部「承接 · NEXT」20px 宋体 + 3px 陶橙左边框，页脚等宽页码。
图形语法（硬规则）：所有示意图现场绘制为内联 SVG，不用外链位图；规整网格／分层横带；
连线只允许水平或竖直直角，禁止斜线穿过文字；文字与连线分离、框内留白 ≥14px、
按框宽自动缩字号，保证零出界／零遮挡／零交叉；一页一图一命题。
禁止：紫粉渐变、emoji、圆角卡片堆叠、滥用投影、装饰性图形、编造统计数字。
交付：单个 HTML 零外部依赖；
```  
注意，交付之前请自检 html 的各项功能是否都正常：内置动画是否正常、按键是否正常、显示讲稿后画面是否完整、缩放后的画面显示是否正常且完整、全文讲稿页面定位是否正确等。
````
  
11. **WEB UI 美化与白天/夜间双色调实时切换** `✅ Phase 25 已落地「☀️ 白天蓝图 / 🌙 夜间深空」双主题系统与面板美化`：
    - 彻底清除硬编码暗色值，建立完整的 `[data-theme="light"]`（暖纸工程蓝图 `#F7F6F2` + 深靛青 `#245A6B` + 陶橙 `#BE653B`，与路演/演讲 HTML 品牌同源）与 `[data-theme="dark"]`（深空指挥舱）两套语义 CSS Token 体系；
    - 顶栏提供一键切换按钮（支持 `T` 快捷键与 `localStorage` 记忆），切换瞬间同步重绘 Canvas 拓扑图配色，并将端口防撞、Pre-Hook、放置推导、DuckDB 热力图与趋势、宿主 GC 等面板升级为「可视化卡片 + 可折叠原始 JSON」。
  
12. **WEB UI「路演演示剧场」三层故事化直观重构** `✅ Phase 25 已落地三层渐进式故事舞台与 6 大专属可视化组件`：
    - 告别主舞台大段裸露 JSON 调试框：顶部增设经济学「比较优势」立意导览与 6 幕场景摘要卡片；
    - 每幕由真引擎返回 **💡 一句话生活比喻 (`analogy`) + 🎯 核心用意 (`intent_summary`) + 3 张高冲击 ROI 指标卡 (`hero_metrics`)**，左屏呈现盲人 Agent 三步翻车故事线，右屏渲染 **6 套专属可视化组件**（黑白名单阻断漏斗+武器库墙、Pre-Hook 1ms 红绿灯、3 跳蝴蝶效应因果链、双 Agent 死锁环路图、Token 99.9% 对比尺、1 秒简报仪表+安全自洁盾），原始 JSON 收纳至底部默认折叠的技术专家核验抽屉。  

13. **实证 Benchmark 对照评测体系与生产效能存证账本（确保使用效果有据可查）** `✅ Phase 27~28 已落地双轨实证引擎、PR-01~08 生产账本与 40/40 黑白屏同步`:
    - 构建「实验室 A/B 可复现对照基准（8 大标准宿主工作负载：Without Steward 裸奔物理探针基线 vs With Steward 真引擎实测 + 5 大内核微基准 SLA）」与「生产运行态每次调用效能存证账本（Token 节省、轮次缩减、端口/锁/断链/乱放冲突拦截、黑名单噪音阻断实时带出处入账）」双轨闭环；
    - 通过第 40 个原生 MCP 工具 `steward_benchmark`、CLI `agent-steward bench` 与 Web UI「📊 实证基准与账本」工作台实现 40/40 黑白屏双向同步与 SHA-256 存证签名，让每一项收益均可量化、可复现、可审计（已升格为开发规约 #18）。
  
---

## 开源协议

本项目采用 [Apache-2.0](LICENSE) 开源许可证。
