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
  
PS: 可能叫 Agent 本体 更恰当？

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
- 🛡️ **[11-security-spec.md](design_doc/11-security-spec.md)**：宿主大管家安全架构、威胁建模与纵深防御规格说明书（全链路脱敏、高危拦截、写鉴权与抗投毒防线）
- 🔄 **[13-freshness-and-self-evolution-spec.md](design_doc/13-freshness-and-self-evolution-spec.md)**：全生命周期信息新鲜度对账与五点对比探针闭环自进化规格说明书（写穿失效、僵尸清理、`FreshnessReconciler` 与 `P1~P5` 探针反馈闭环）
- 🕸️ **[14-graph-intelligence-and-mesh-protocol-spec.md](design_doc/14-graph-intelligence-and-mesh-protocol-spec.md)**：多维证据分层有向加权图谱与联邦网格信封协同协议规格说明书（`L1/L2/L3` 三层关系本体、结构上卷与向量语义发现、`MeshEnvelope v1.0` HMAC 防重放认证、跨机对话与沙盒文件投递）
- ⚡ **[15-adaptive-optimistic-pessimistic-concurrency-spec.md](design_doc/15-adaptive-optimistic-pessimistic-concurrency-spec.md)**：乐观-悲观自适应并发控制（AOPCC）与失败早回退规格说明书（期望代价数学边界、开工乐观直通租约、三态动作分级器与 `PostToolUse` 碰壁秒级自愈）

---

## 三维角色全景解读 (Architecture & Guides)

针对不同技术角色的深层设计、二次开发与生产实操，发布于 **[`arch/`](arch/)** 目录：

- 🏛️ **[01-for-architects.md](arch/01-for-architects.md)**（面向架构师与技术决策者）：
  - 三位一体系统认知模型深度拆解；
  - 存储中枢 Facade + 统一 SPI 抽象（SQLite / PG / DuckDB 双向热迁移与增量同步）；
  - 多 Agent 协作锁与层级互斥传播算法（Advisory Lock + 祖先后代路径检测）；
  - 工业级 NFR 指标（单二进制交付、冷启动 `<50ms`、空闲内存 `<25MB`）与递弱代偿边界。
- 🛠️ **[02-for-developers.md](arch/02-for-developers.md)**（面向研发工程师与贡献者）：
  - 源码 25 大模块全景职责与依赖拓扑速查；
  - 核心扩展三大 How-To（添加原生 MCP 工具、实现新存储后端 SPI、新增环境预检项）；
  - 235+ 项自动化测试套件与本地开发流；
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
- 🔌 **[04-mcp-integration.md](manual/04-mcp-integration.md)**：Agent 接入指南：41 个原生 MCP 工具与 Claude/Cursor 配置
- 🔄 **[05-daemon-and-watcher.md](manual/05-daemon-and-watcher.md)**：后台常驻守护服务、inotify 300ms 防抖与 systemd 自启
- ⚡ **[06-advanced-features.md](manual/06-advanced-features.md)**：进阶特性：端口防撞、断链智能修复、放置规则自演化、三层元数据、WFG 死锁检测、宿主资源 GC、记忆生命周期与 TUI 看板
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
- `arch/` : 面向架构师、开发者、用户的三维角色全景解读文档
- `design_doc/` : 产品功能设计文档与技术规格说明书
- `idea_doc/` : 产品起初的想法与架构演进思考
- `manual/` : 官方使用手册（快速上手、CLI/MCP 参考、Web 大屏、Hooks、Benchmark 等）
- `markdown/` : 对外发布的文章、博客与配套 SVG 图表
- `releases/` : 发版说明文档
- `scripts/` : 辅助运维与演进脚本（架构清单防漂移检查、Ollama 向量补齐等）
- `skills/` : 与本产品相关的对接 AI Agent 的 SKILL 包
- `src/` : Rust 核心源代码
- `src_doc/` : 本项目的阶段开发路线图与演进文档
- `tests/` : 自动化集成测试与 Benchmark 评测套件
- `.dockerignore` : Docker 构建忽略配置
- `.gitignore` : Git 版本控制忽略配置
- `Cargo.lock` : Rust 依赖版本锁定文件
- `Cargo.toml` : Rust 项目清单与依赖配置
- `Dockerfile` : 容器化构建镜像配置
- `EVOLUTION_GUIDE.md` : 零重扫项目演进指南（架构地图、变更触点矩阵与开发 SOP）
- `INSTALL.md` : 详细安装部署指南（依赖矩阵、Docker、Systemd 与故障排查）
- `LICENSE` : Apache-2.0 开源许可证
- `Makefile` : 构建、测试、门禁检查与部署任务编排
- `README.md` : 本文件
- `build.rs` : Cargo 构建脚本
- `config.example.toml` : 运行时配置文件示例
- `docker-compose.yml` : Docker Compose 编排配置
- `presentation.html` : 产品路演 PPT
- `steward-deck.html` : 产品交流 PPT
- `user_feedback.md` : 实际使用反馈总账（优点、缺点痛点、新需求与 Benchmark 迭代记录）
  
## 核心设计原则与约束 (Design Principles & Constraints)

### 产品定位与边界

1. **服务对象准则**：
   - 产品的核心用户是 **AI Agent**，而不是给人日常使用。数据输入输出必须以"极低认知负载、零幻觉只读提取、语义自解释"为第一标准。

2. **边界与递弱代偿哲学**：
   - 产品的边界必须搞清楚，不是啥都做，专注做宿主环境认知的原子功能或分子功能，做专做精做深。如同王东岳所说的"递弱代偿"，越基础的功能越被深度依赖；功能过度泛化会丧失生态定位。
   - 明确不做：不替代 Agent 编排框架（LangGraph/AutoGen/CrewAI）、不承担业务 LLM 推理、不替代容器沙箱隔离（Docker/gVisor）、不充当通用 OLTP 数据库、不替代密钥管理（Vault/1Password）；在跨主机 Mesh 协同中不造 Raft/Paxos 重型共识引擎、不充当大文件全盘镜像系统（Rsync/S3）、不造人类聊天 IM，仅专注 **Agent-to-Agent 轻量认知交换、租约锁协调与受控小产物（`<= 10MB`）沙盒投递**。

3. **系统底座定位探讨**：
   - 本项目所承载的"宿主机事实感知、环境规约与长期记忆"三位一体能力，未来是否应演进为面向智能体时代的下一代 OS 原生内置标配功能。

### 数据治理

4. **事实与记忆的摄入取舍门禁**：
   - 记录事实与 memory 时必须有克制与取舍，绝不能不加筛选全盘记录。通过重要性评分与噪音过滤规则（构建缓存、临时文件、高频变动垃圾阻断），保障事实图谱与记忆库的高纯净度。

5. **实事求是、零捏造 (Ground Reality, Zero Simulation)**：
   - 所有暴露给 Agent 的事实必须源自宿主机真实物理状态（`/proc`、`/sys`、`/etc/systemd`、`socket`、`inotify`）。严禁在缺少证据的情况下推测或编造虚拟状态。
   - **图谱证据分层底线**：事实图谱关系必须严格区分物理显式硬依赖（`L1_EXPLICIT`）、结构聚合派生（`L2_STRUCTURAL`）与隐式语义近邻（`L3_SEMANTIC`），每条边须携带证据层级与推导理由，严禁将高维语义相似伪装成物理硬依赖。

6. **颗粒度分级与白黑名单深度法则**：
   - **白名单目录**（如 `/root/new/shared_proj/`、`/root/new/work/`）：必须深度递归解析到底，构建完整元数据与全文索引。
   - **黑名单目录**（如 `node_modules/`、`.cache/`、`.git/`）：仅记录当前目录实体本身，严禁递归下钻或入库子文件 FTS/向量，防止数据库扫爆与敏感信息泄露。
   - **受限目录**（`restricted`）：受限深度扫描（如开源代码库仅索引 README 与入口文件）。

7. **动态进程过滤底噪**：
   - 进程纳管只抓取 Agent 会话任务、Python 脚本、构建编译 Worker 等应用进程；系统底噪（内核工作线程 `kworker`、systemd 基础进程等）通过白名单与启发式规则过滤。

### 安全、隐私与纵深防御

8. **风险分级干预与失败后置自愈（悲观红线 × 乐观直通）**：
   - 垃圾清理（GC）默认强制 `dry-run` 预览，绝不静默硬删除；强制限定白名单目录。
   - 对高危、不可逆或共享资源写操作（端口 bind、高爆炸半径文件写、系统敏感路径、跨 Agent 锁），通过 `steward_hook_pre` 执行**悲观前置拦截与冲突预测**。
   - 对可逆/低失败代价操作允许依据开工乐观租约直行，并在 `steward_hook_post` 捕获 `success=false`（乐观碰壁）时自动触发宿主机事实反查与替代处方注入。
   - 会话结清（`session_close`）提供可审计的结项清单。

9. **极速与轻量化 (Sub-Millisecond Response)**：
   - 核心只读/决策 API 端到端响应必须在毫秒/微秒级以内（实测微基准：放置决策 `~30 μs`，3 跳图遍历 `~120 μs`，FTS5 检索 `~250 μs`，三位一体冷启动简报 `< 15 ms`），绝不能成为 Agent 执行链路的性能瓶颈。
   - CLI/MCP 单次进程常驻内存 `< 15 MB`，内嵌 Web + DuckDB + inotify 的全功能常驻守护内存 `< 90 MB`、空闲 CPU `< 0.5%`。

### 工程开发规约

10. **运行依赖四层贯通约束**：
    - 必须充分考虑宿主机环境的物理差异。任何运行依赖与假定必须贯彻 **代码层面 × 配置层面 × 文档层面 × SKILL 层面** 四层贯通；启动时强制运行时预检，防止在未知宿主环境中静默崩溃或部署失败。

11. **开发原则**：
    - 本项目使用 AI Agent 开发，收到任务后，先进行任务分解。任务拆解要细致、要按工业级产品标准来要求功能设计/开发/测试/文档、要在任何时刻任何情况任务失败后可以"断点续传"。开发完成后，如有必要记得要及时更新手册、SKILL、MCP 等相关的文档或代码。要将 SKILL、手册、MCP 和代码一样当成一等公民来对待。

12. **零 Panic 原则**：
    - 在非测试代码中，严禁在生产路径直接使用 `unwrap()` 或 `expect()`。一律使用 `anyhow::Result` 或 `thiserror` 向上冒泡错误，保证服务 7×24 稳定运行。

13. **测试驱动与零警告**：
    - 新增功能必须在 `tests/` 或对应模块内提供自动化测试（全库持续守卫 237+ 项自动化测试）。
    - 任何提交前必须 `cargo check --tests` 通过零警告门禁。
    - MCP 工具与 CLI 子命令必须保持一一映射（当前 41/41 同步）。

14. **断点续传检查点机制**：
    - 开发任何跨步骤大型特性时，必须在当前活跃的 `src_doc/roadmap-phase*.md` 内建立原子检查点追踪表，并在总表 [src_doc/progress.md](src_doc/progress.md) 登记阶段状态（与规约 #20 零重扫配合，不再为每个 Phase 额外生成碎片化的 `progress-phase*.md`）。
    - 确保中断后任何新会话均能通过读取 `EVOLUTION_GUIDE.md` + 当前活跃 `roadmap-phase*.md` 恢复上下文无缝继续。

15. **宏微联邦、不重复造轮子**：
    - 大管家负责宏观宿主拓扑（文件、服务、端口、定时任务间的依赖网络），并支持将微观文件级 `links` 自动上卷聚合（Roll-up）至宏观项目/服务级 `fact_edges`，实现文件引用、项目拓扑与 CodeGraph 符号级调用的三层贯通。
    - 微观符号调用（函数、类、AST）下钻交给 CodeGraph，大管家通过 `CodeGraphBridge` 无缝桥接。
    - 记忆模块通过 SPI 抽象，拥抱成熟开源生态（Mem0/Letta/PowerContext），绝不重复发明轮子。

16. **黑白屏全能力双向同步规约 (Black/White-Screen Parity)**：
    - 黑屏功能（MCP 工具、SKILL 指导、CLI 子命令）与白屏功能（Web UI 工作台、REST API）必须保持 **1:1 双向同步**，严禁出现仅存在于单侧的孤岛功能。
    - 为防止 MCP 工具数量膨胀打爆上下文，新能力优先在既有原生 MCP 工具内通过 `action` 子模式内聚聚合，并在 CLI 子命令、Web REST API 与前端工作台按**子动作粒度（Action-level）**保持 1:1:1 同步。
    - 白屏端每次交互应同步镜像回显等价的 CLI 命令与 MCP JSON-RPC 请求体；并通过 `/api/v1/parity/matrix` 与自动化契约测试强制守卫 **100% 对齐率**。

17. **产品路演与场景演示内建规约 (Built-in Roadshow & Live Demo)**：
    - 从产品经理与市场营销推广视角，将高频痛点与核心护城河沉淀为 Web 端可交互的 **路演演示剧场（Roadshow Live Demo）**，与黑屏 `steward_demo_scenario` / `agent-steward demo` 同源演进。
    - 所有路演剧目必须由真实引擎驱动（严禁硬编码假数据），状态写入严格限定在 `demo:*` 隔离沙盒并支持一键无痕复位；同时将离线路演 PPT（`/deck`、`/presentation`）与 ROI 实时账单内建于单二进制 Web 服务中，随版本同步刷新。

18. **实证基准与效能有据可查规约 (Empirical Benchmark & Verifiable ROI Ledger)**：
    - 任何宣称的效能提升（Token 节省率、上下文质量净提升、探索轮次缩减、微秒级响应延迟、故障/冲突拦截率、垃圾噪音阻断率）必须通过可复现的 **A/B 对照基准套件（Without Steward 裸奔基线 vs With Steward 管家赋能）** 与 **生产运行态效能存证账本（Effectiveness Telemetry Ledger）** 双轨实证，严禁无出处估算或主观拍脑袋指标。
    - **帕累托双门禁（既省 Token，上下文质量不降反升）**：严禁通过丢弃关键事实或返回空洞摘要来骗取高 Token 压缩率；每个基准负载与竞品对标轨道必须同时考核 **关键事实完整度（Fact Recall）**、**信噪比/纯净度（Signal Precision）**、**结构化可行动性（Actionability）** 与 **有效信息密度提升倍数（Info Density Gain: Facts/1k-tok）**，强制满足 $Q_{\text{steward}} \ge Q_{\text{baseline}}$ 且必含事实 100% 命中。
    - 基准测试必须基于宿主机真实物理探针与 `bench:*` 隔离沙盒实跑（覆盖内核微基准延迟 SLA 与宏观业务负载），并同步核算通过开工乐观租约省去的**整轮对话税 (`rounds_saved` / 回路工具调用轮次)**；每次评测与日常工具调用的真实耗时（`elapsed_us`）、Token 缩减量（`tokens_saved`）、探索轮次缩减（`rounds_saved`）及风险拦截事件（`hazard_intercepted`）均须携带确定性推导规则编号（`provenance_rule`）落库留痕，支持通过 CLI（`agent-steward bench`）、MCP（`steward_benchmark`）、Web UI 与 DuckDB 导出报告随时溯源核验与跨版本回归比对，确保大管家使用效果 100% 有据可查。

19. **纵深安全防御、全链路脱敏与抗投毒规约 (Defense-in-Depth Security, End-to-End Redaction & Anti-Poisoning)**：
    - **全链路凭据零落盘**：在文件扫描、`/proc` `cmdline` 采集、MCP 配置解析、外部服务登记、长期记忆写入、变更审计流水、跨机 Mesh 对话与本地模型推理前，统一强制经过 `SecretRedactor` 脱敏与敏感文件阻断（`.env*`、`.ssh/`、`*.pem`/`*.key`、URI 口令与高危令牌前缀），严禁明文凭据入库或经由 Brief / Mesh 泄露。
    - **执行门禁与防注入沙盒**：`HookDispatcher` 禁止裸拼接未转义路径交由 `sh -c` 执行；DuckDB 导出在引擎底层强制校验目录白名单与 SQL 转义；`steward_hook_pre` 对高危系统路径（`/etc/shadow`、`/etc/sudoers`、`~/.ssh/` 等）与破坏性操作给出 `CRITICAL` 预警。
    - **接口最小暴露、写鉴权与 Mesh 信封签名**：Web/Mesh 默认绑定 `127.0.0.1` 回环地址；全量 Web 写端点及 GC 端点统一受 `auth_token` Bearer 保护，非回环暴露未配置 `auth_token` 时拒绝写操作；跨节点 Mesh 协同写操作、对话与文件投递强制采用 `MeshEnvelope v1.0`（HMAC-SHA256 签名 + 时间窗/Nonce 防重放 + `~/.steward/mesh_inbox/` 隔离收件箱），严禁跨机直写覆盖系统与业务文件。
    - **存储最小权限与抗上下文投毒**：`~/.steward/` (`0700`) 与 `steward.db`/`config.toml` (`0600`) 权限收敛并通过 `preflight` 巡检；对外部仓库/文档语料及跨机 Mesh 对话内容识别并清洗间接提示词注入（Indirect Prompt Injection），同时纳入 `W11`、`W14` 与 `DQ4` 安全回归基准（详见 [11-security-spec.md](design_doc/11-security-spec.md)）。

20. **零重扫演进导航与横向竞品实证对标规约 (Zero-Rescan Evolution & Competitor Benchmarking)**：
    - **零重扫演进活地图**：新会话演进优先查阅根目录演进指南（[EVOLUTION_GUIDE.md](EVOLUTION_GUIDE.md)）、运行架构清单门禁（`make evolution-check`）并使用本项目 `.codegraph/` 符号地图定位触点，严禁把历史冻结 `roadmap-phase*.md` 全盘扫描一遍。
    - **横向竞品同场实证**：Benchmark 在与裸奔自对比和生产账本对齐的同时，必须纳入与主流 Agent 基础设施（Mem0 / Letta / PowerContext / Shodh-Memory / OpenViking / AgentFS / rg+psutil）的同机实跑与源码级能力对标，严格区分 `LIVE_BENCHMARKED`（实跑）与 `ARCH_VERIFIED`（源码核实），绝不捏造竞品跑分。

21. **全生命周期信息新鲜度、写穿失效与现实对账规约 (Information Freshness, Write-Through Invalidation & Reality Reconciliation)**：
    - **内容变更写穿失效**：当文件 `content_hash` 或 `size_bytes` 变化时，必须同步将 `desc_kv["embedding_status"]` 标为 `"stale"` 并清空旧向量 `'[]'`、将旧 L2 标注标为 `"needs_re_enrich"` 并刷新拓扑依赖边，严禁旧语义与旧向量僵尸驻留。
    - **物理消亡级联净化**：文件删除后必须在检索层（`search_assets_fts` 强制 `status = 'active'`）零召回，并级联清理 `fact_nodes`、`links` 与关联 `auto_sync` 记忆；同路径 `auto_sync` 记忆实行“新写归档旧版”幂等去重，杜绝单文件反复修改霸屏简报。
    - **周期性现实对账 (`FreshnessReconciler`)**：守护进程与按需 CLI/MCP 定期比对库内快照、磁盘文件状态、`/proc` 实时监听端口与 `.codegraph/codegraph.db` `mtime`，自动剪除幽灵端口节点与过旧资产。

22. **内生对比探针驱动的闭环自进化规约 (Probe-Driven Closed-Loop Self-Evolution & Auto-Feedback Flywheel)**：
    - **五点回路对比探针**：在 `SessionStart(Brief)`、`PreToolUse(Guard)`、`InFlight(Search)`、`PostToolUse(Drift)` 与 `Periodic(Regression)` 五个执行切面部署确定性对比探针（`P1~P5`），以实测物理态 vs 历史快照/SLA/上一版本 Benchmark 为参照系，量化记录每一项偏差与改善幅度；并在 `P1/P2/P4` 中闭环跟踪乐观租约有效率 ($P_{\text{valid}}$) 与碰壁自愈率，当某项服务频发失效时自动降级其乐观直通权限。
    - **数字佐证与去重自上报**：严禁拍脑袋优化。任何自愈修复与能力演进必须提供探针实测数字差值（Before/After/Delta）佐证；当越过退化阈值时自动调用幂等去重的 `FeedbackEngine::add_item_deduped` 写入 [user_feedback.md](user_feedback.md)，形成“探针发现 → 数字佐证 → 自动自愈/反馈 → Benchmark 固化”的自进化飞轮（详见 [13-freshness-and-self-evolution-spec.md](design_doc/13-freshness-and-self-evolution-spec.md)）。

23. **多维证据分层、有向加权与隐式语义图谱规约 (Multi-Tier Evidence, Directed Weighted & Semantic Graph Intelligence)**：
    - **三层证据契约**：图谱连边强制划分 `L1_EXPLICIT`（物理硬依赖，权重 `0.85~1.00`）、`L2_STRUCTURAL`（文件上卷/同体服务桥接/工具链提取，权重 `0.55~0.84`）与 `L3_SEMANTIC`（向量余弦 `>= 0.78` + 标签混合发现的隐式关联，权重 `0.35~0.65`），每条边须显式携带 `evidence_tier`、`weight` 与方向语义（前向依赖 / 逆向调度 / 衍生流 / 对称亲和）。
    - **防稠密剪枝与加权衰减推演**：隐式语义边每节点严格 `Top-K <= 3` 稀疏化剪枝；爆炸半径推演按 `S = prod(w_e * 0.8^(hop-1))` 计算有效风险传播，默认以 `L1+L2` 为强影响面基准、以 `L3` 为跨项目关联探索参考，兼顾物理可靠性与隐式关系洞察。

24. **联邦网格信封协议、零信任签名认证与受控沙盒协同规约 (Federated Mesh Wire Protocol, Zero-Trust Auth & Sandboxed Collaboration)**：
    - **信封协议与双向防重放**：跨主机 Mesh 协同统一采用 `MeshEnvelope v1.0` 线协议，写与协同请求强制携带 `timestamp_ms`、`nonce`、`scopes`、`payload_sha256` 及 `HMAC-SHA256` 规范签名；服务端校验 `+-300s` 时间窗、`NonceCache` 防重放与 Peer Scope 最小授权（支持 `read:facts` / `write:locks` / `exchange:dialogue` / `transfer:artifact`）。
    - **受控协同原语**：跨机对话仅限结构化意图（`ASK_CONTEXT` / `HANDOFF_TASK` / `SHARE_MEMORY` / `REPLY`），入出双向经由 `SecretRedactor` 与 `PromptInjectionScanner` 过滤；跨机文件传输严格限额 `<= 10MB`、校验 SHA-256 与阻断敏感文件，隔离落盘至 `~/.steward/mesh_inbox/`（`0600`）待本机显式 `accept`/`reject` 后方可使用（详见 [14-graph-intelligence-and-mesh-protocol-spec.md](design_doc/14-graph-intelligence-and-mesh-protocol-spec.md)）。

25. **乐观-悲观自适应并发控制与失败早回退规约 (Adaptive Optimistic-Pessimistic Concurrency Control & Fail-Fast Fallback, AOPCC)**：
    - **三态动作并发分级**：依据可逆性、失败代价 ($C_{\text{fail}}$) 与冲突概率，将 Agent 动作划分为 `OPTIMISTIC`（只读、服务客户端直连如本机 `:5432`、已授权工具、`/root/new/tmp/` 沙盒验证——0 前置查询直接执行）、`CAUTIOUS_CAS`（常规新建文件的静默本地规则校验）与 `PESSIMISTIC_LOCK`（端口 bind、高爆炸半径文件修改、系统高危路径、跨 Agent 排他锁——强制前置拦截）。
    - **开工乐观租约与碰壁秒级自愈**：`steward_brief` 在 `SessionStart` 签发高置信度（$P_{\text{valid}} \ge 0.90$）且带纪元指纹（`epoch_hash`）的乐观直通契约；若乐观直行报错（如 `Connection refused` / `command not found` / `Address already in use`），必须由 `steward_hook_post(success=false)` 在 `< 2ms` 内反查宿主机真实状态并注入替代处方，消除整轮对话税（详见 [15-adaptive-optimistic-pessimistic-concurrency-spec.md](design_doc/15-adaptive-optimistic-pessimistic-concurrency-spec.md)）。

---

## 开源协议

本项目采用 [Apache-2.0](LICENSE) 开源许可证。
                                                                      