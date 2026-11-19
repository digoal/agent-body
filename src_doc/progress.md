# Agent Host Steward (Rust 版) — 研发与测试进度表（断点续传）

> 创建时间: 2026-11-15T12:50:00+08:00  
> 最新更新: 2026-11-19T14:50:00+08:00  
> 技术栈: Rust 1.99.0 + SQLite 3 / PostgreSQL / DuckDB + Tokio + Clap  
> 当前状态: Phase 1~26 全部完成（v0.4.0），39 个原生 MCP 工具（黑白屏 100% 同步），Web UI 2.0 昼夜双主题与三层故事化路演剧场已上线，数据治理硬化与真实依赖图谱闭环已落地，全量测试 198 passed (0 failed, 1 ignored)

---

## 阶段规划与任务清单

| 阶段 | 子任务 ID | 任务内容 | 涉及路径 | 状态 | 完成时间 |
|------|-----------|----------|----------|:---:|----------|
| **阶段 1** | 1.1 | 建立 `src_doc/` 目录与断点续传追踪文档 | `src_doc/progress.md` | ✅ 已完成 | 2026-11-15 12:50 |
| | 1.2 | 编写本地编译与测试环境开发指南 | `src_doc/development.md` | ✅ 已完成 | 2026-11-15 12:50 |
| | 1.3 | 创建 `Cargo.toml` 配置依赖项与编译选项 | `Cargo.toml` | ✅ 已完成 | 2026-11-15 12:53 |
| **阶段 2** | 2.1 | 实现配置管理模块 `src/config/` | `src/config/` | ✅ 已完成 | 2026-11-15 12:54 |
| | 2.2 | 实现存储层 `src/storage/`（内嵌 SQLite schema 自动初始化迁移） | `src/storage/` | ✅ 已完成 | 2026-11-15 12:55 |
| | 2.3 | 编写并跑通存储层单元测试 | `src/storage/mod.rs` | ✅ 已完成 | 2026-11-15 12:55 |
| **阶段 3** | 3.1 | 实现放置建议引擎 `src/placement/`（规则匹配、命名约定、冲突检查） | `src/placement/` | ✅ 已完成 | 2026-11-15 12:56 |
| | 3.2 | 编写并跑通放置建议引擎单元测试 | `src/placement/mod.rs` | ✅ 已完成 | 2026-11-15 12:56 |
| | 3.3 | 实现资产分级扫描引擎 `src/scanner/`（白名单深挖到底，黑名单止于当层） | `src/scanner/` | ✅ 已完成 | 2026-11-15 12:57 |
| | 3.4 | 编写并跑通扫描引擎单元测试 | `src/scanner/mod.rs` | ✅ 已完成 | 2026-11-15 12:57 |
| **阶段 4** | 4.1 | 实现环境感知与动态进程采集 `src/env/`（/proc 扫描、关联资产、系统状态） | `src/env/` | ✅ 已完成 | 2026-11-15 12:58 |
| | 4.2 | 实现记忆读写与全文检索 `src/memory/` | `src/memory/` | ✅ 已完成 | 2026-11-15 12:58 |
| | 4.3 | 编写并跑通环境与记忆模块测试 | `src/env/mod.rs`, `src/memory/mod.rs` | ✅ 已完成 | 2026-11-15 12:58 |
| **阶段 5** | 5.1 | 实现命令行主入口 `src/main.rs` 与子命令解析 | `src/main.rs` | ✅ 已完成 | 2026-11-15 12:59 |
| | 5.2 | 运行全量 `cargo build` 与 `cargo test`，确保 100% 编译与通过测试 | 项目根目录 | ✅ 已完成 | 2026-11-15 12:59 |
| | 5.3 | 编写使用手册 `src_doc/product-manual.md` 与根目录 README 联动 | `src_doc/product-manual.md`, `README.md` | ✅ 已完成 | 2026-11-15 13:00 |
| **阶段 6** | 6.1~6.7 | 项目结构与文件规范化重构 (全小写 kebab-case、死链清理、31 单测回归、共享镜像同步) | 详见 [roadmap-phase6.md](roadmap-phase6.md) | ✅ 已完成 | 2026-11-15 18:31 |
| **阶段 7** | 7.1~7.8 | 拓扑智能自愈、放置规则自进化、宿主资源 GC 与自治生态 (22 原生 MCP、TUI 看板、自愈修复) | 详见 [roadmap-phase7.md](roadmap-phase7.md) | ✅ 已完成 | 2026-11-15 19:35 |
| **三位一体 Phase 1** | 1.1 | 事实图谱核心：工具能力语义注解库与物理扫描器 (Capability Graph & Scanner) | `src/fact/capability.rs` | ✅ **已完成** | 2026-11-15 21:07 |
| | 1.2 | 事实图谱核心：内核级进程与端口拓扑引擎 (/proc & Socket) | `src/fact/process_net.rs` | ✅ **已完成** | 2026-11-15 21:19 |
| | 1.3 | 事实图谱核心：系统自动化依赖网络 (Systemd & Cron Topology) | `src/fact/automation.rs` | ✅ **已完成** | 2026-11-15 21:28 |
| | 1.4 | 事实图谱核心：源码工程与知识资料编目器 (Code & Assets Catalog) | `src/fact/assets_catalog.rs` | ✅ **已完成** | 2026-11-17 17:20 |
| | 1.5 | 事实图谱核心：只读查询语义接口与 MCP/CLI 贯通 (steward_fact_query) | `src/api/` & `src/main.rs` | ✅ **已完成** | 2026-11-17 17:35 |
| **三位一体 Phase 2** | 2.1 | 偏好规约：资产放置规则引擎 2.0 (项目级配置覆盖与意图推导) | `src/placement/engine_v2.rs` | ✅ **已完成** | 2026-11-17 17:39 |
| | 2.2 | 偏好规约：工程操作习惯与安全规约 (CodeGraph优先/真实性门禁) | `src/placement/conventions.rs` | ✅ **已完成** | 2026-11-17 17:43 |
| | 2.3 | 偏好规约：纠偏自演化学习器 2.0 (文件跨目录搬移行为捕获沉淀) | `src/placement/learner.rs` | ✅ **已完成** | 2026-11-17 17:52 |
| **三位一体 Phase 3** | 3.1 | 可插拔记忆：Memory Provider SPI 抽象设计与通用数据契约 | `src/memory/spi.rs` | ✅ **已完成** | 2026-11-17 18:02 |
| | 3.2 | 可插拔记忆：内置轻量引擎实现 (Builtin Local Provider) | `src/memory/builtin_provider.rs` | ✅ **已完成** | 2026-11-17 18:07 |
| | 3.3 | 可插拔记忆：成熟开源生态适配器 (Mem0 / Letta / PowerContext) | `src/memory/adapters/` | ✅ **已完成** | 2026-11-17 18:54 |
| **三位一体 Phase 4** | 4.1 | 自适应情境组装：BriefAssembler 核心组装器 | `src/brief/assembler.rs` | ✅ **已完成** | 2026-11-18 09:45 |
| | 4.2 | 自适应情境组装：steward_brief MCP 动态化升级 | `src/api/mcp_tools.rs` | ✅ **已完成** | 2026-11-18 09:47 |
| | 4.3 | 自适应情境组装：情境质量评分器与 TUI 看板升级 | `src/tui/mod.rs` | ✅ **已完成** | 2026-11-18 09:47 |
| | 4.4 | 自适应情境组装：端到端集成验收实测与生产部署 | 全系统 | ✅ **已完成** | 2026-11-18 09:48 |

---

## 阶段验收结论 (Phase 1~7 & 模块化新体系)

✅ **阶段 1 至阶段 7 全部 23 个子任务 100% 串行完成**！
- 搭建了 Rust 1.99 + SQLite 3 (rusqlite bundled with FTS5) 本地编译与单元测试体系；
- 实现了 `PlacementEngine`（新对象放置建议、冲突检测、安全脱敏规范）；
- 实现了 `PlacementLearner`（基于历史资产与变更聚类自学习沉淀新规范）；
- 实现了 `Scanner`（白名单深度遍历、黑名单仅止于当前目录层级绝不下钻）；
- 实现了 `EnvCollector`（硬件规格采集与 `/proc` 活跃动态任务进程采集）；
- 实现了 `HostGarbageCollector`（安全白名单清理 `/root/new/tmp/` 超期孤儿文件与僵尸进程诊断）；
- 实现了 `MemoryManager`（记忆录入与 FTS5+BGE-M3 混合 RRF 搜索）；
- 实现了 `MemoryLifecycleManager`（时间半衰期衰减与对立语义认知矛盾仲裁）；
- 实现了 `LinksEngine` 与 `CodeGraphBridge` 宏微双层联邦联动，支持断链审计与爆炸半径推演；
- 实现了 `LinkRepairer`（基于 Levenshtein 相似度与代码指纹自动推演断链修复 patch）；
- 实现了 `TuiDashboard`（交互式终端单屏实时看板与状态刷新）；
- 实现了 23 个原生 MCP 工具端点与 CLI 子系统；
- 交付件完整落地于 `manual/`、`src_doc/`、`README.md` 与 `/root/new/shared_git/agent-steward`。

🎉 **三位一体架构 Phase 1（宿主机事实图谱核心）已 100% 全量串行交付**：
- 详见路线图 [roadmap-modular-phase1.md](roadmap-modular-phase1.md)；
- 宿主机事实图谱已全面纳管工具能力、内核进程端口、Systemd/Cron 自动化拓扑、以及源码与权威清单！

🎉 **三位一体架构 Phase 2（偏好规约与自演化）已 100% 全量串行交付**：
- 详见路线图 [roadmap-modular-phase2.md](roadmap-modular-phase2.md)；
- 实现了项目级配置覆盖、意图模糊打分、小写蛇形校验、CodeGraph 与真实性工程纪律、以及跨目录搬移纠偏自演化！
🎉 **三位一体架构 Phase 3（可插拔记忆与成熟开源适配）已 100% 全量串行交付**：
- 详见路线图 [roadmap-modular-phase3.md](roadmap-modular-phase3.md)；
- 实现了 SPI 抽象、内置 SQLite 引擎、以及 PowerContext / Mem0 / Letta 三大适配器，支持 `steward.toml` 一行切换 + 优雅降级；
- 单元测试 82 项 100% PASS，零 warning；服务 `active (running)`。
- 下一阶段为 **三位一体 Phase 4：自适应情境组装与全景中枢 (Adaptive Context Assembly)**。

---

## 下一阶段路线图 (Phase 5~9)

> 详见 [roadmap-next-phases.md](roadmap-next-phases.md)，对齐 README.md 中的 9 个 ISSUE。

| 阶段 | 核心内容 | 对齐 ISSUE | 状态 | 完成时间 |
|:----:|---------|:----------:|:----:|:--------:|
| **Phase 5** | 宿主机主动扫描与智能记录引擎（断点续扫 + 重要性评分 + 属性图 + Agent Hook） | #9 #2 #3 | ✅ **已完成** | 2026-11-18 12:02 |
| **Phase 6** | 存储层抽象与多后端支持（StorageProvider SPI + PostgreSQL pgvector/tsvector + 跨引擎无损热迁移） | #4 | ✅ **已完成** | 2026-11-18 12:23 |
| **Phase 7** | Agent-Native 输出优化与 Benchmark 性能基准体系（StewardResponse 封套 + 亚毫秒基准测试） | #8 #6 | ✅ **已完成** | 2026-11-18 12:26 |
| **Phase 8** | 跨平台抽象层与容错探测（macOS / Windows 兼容抽象） | #5 | ✅ **已完成** | 2026-11-18 12:27 |
| **Phase 9** | 产品边界守护与生态定位（边界指南 + Agent 5阶段生命周期协议） | #1 #7 | ✅ **已完成** | 2026-11-18 12:28 |
| **Phase 10** | DuckDB 分析后端（DuckDB Provider + 分析查询扩展 + 增量同步管道 + 集成测试） | #4 | ✅ **已完成** | 2026-11-18 14:05 |
| **Phase 11** | 多 Agent 协作锁（CoordinationEngine + PG Advisory Lock + MCP 工具 + Pre-Hook 集成） | 新增 #10 | ✅ **已完成** | 2026-11-18 15:20 |
| **Phase 12** | 运行依赖显式化与可移植部署（preflight 预检 + config 模板 + INSTALL.md + Dockerfile + SKILL 同步） | #10(ISSUE) | ✅ **已完成** | 2026-11-18 17:15 |
| **Phase 13** | Web UI / 可视化监控大屏（内嵌 HTTP 引擎 + 宿主拓扑图谱 + 协作锁实时大屏 + 自包含单二进制） | #13 | ✅ **已完成** | 2026-11-18 18:15 |
| **Phase 14** | HOOK 驱动集成引擎（EventBus + AutoSync + 客户端生成器 + Session 自动绑定 + SSE/Webhook/UDS + Pipeline） | #3(HOOK深化) | ✅ **已完成** | 2026-11-18 22:50 |
| **Phase 15** | 工程规约全面对齐与零 Panic 硬化（生产代码 140 处 unwrap 清零 + 补齐 7 个 CLI 实现 32 工具 1:1 + 四层文档/SKILL 同步） | 规约 #10-#13 | ✅ **已完成** | 2026-11-18 23:25 |
| **Phase 16** | 管理范围扩展与 Context 防爆路由器（MCP 注册表 + 130+ SKILL 目录 + 外部服务通讯录 + Token 预算路由） | 待探讨 ISSUE #4 | ✅ **已完成** | 2026-11-18 23:41 |
| **Phase 17** | 智能死锁检测与协作锁等待唤醒中枢（lock_wait_queue + Wait-For Graph 环路检测 + UDS/SSE 唤醒推送） | 待探讨 ISSUE #2 | ✅ **已完成** | 2026-11-19 00:00 |
| **Phase 18** | 跨平台原生探测深化（macOS/Windows）与轻量多机联邦网格（HTTP Peer Mesh + 跨节点锁路由） | 待探讨 ISSUE #3 & #1 | ✅ **已完成** | 2026-11-19 00:19 |
| **Phase 19** | 真实事实拓扑修正、进程去噪与 Web UI 2.0 全景交互指挥舱（5 大工作台 + Daemon SSE 实时推流） | 待探讨 ISSUE #5 / 规约 #5 #7 | ✅ **已完成** | 2026-11-19 08:21 |
| **Phase 20** | 四层贯通文档刷新、全局 3 处 SKILL 镜像同步与现网守护热更新交付 | 规约 #10 #11 #13 #14 | ✅ **已完成** | 2026-11-19 08:23 |
| **Phase 21** | 黑白屏全能力双向同步与全景工作台升级（补齐 21 项缺失+7 项残缺白屏 API/UI + 黑屏命令镜像器 + 100% Parity 矩阵） | 待探讨 ISSUE #6 / 规约 #13 #16 | ✅ **已完成** | 2026-11-19 09:34 |
| **Phase 22** | Web 路演演示剧场与产品营销互动引擎（6 大黄金剧目真引擎沙盒推演 + 左右分屏 + ROI 看板 + 第 39 工具 `steward_demo_scenario`） | 待探讨 ISSUE #7 & #6 / 规约 #5 #17 | ✅ **已完成** | 2026-11-19 09:55 |
| **Phase 23** | 分层语义元数据 Key 体系与本地 Ollama 小模型双轨自动标注（L1 确定性提取 + L2 `qwen3.5:0.8b` 脱敏精炼 + 三级渐进披露 + 防覆写合并） | 待探讨 ISSUE #8 / 规约 #9 #10 #16 | ✅ **已完成** | 2026-11-19 12:00 |
| **Phase 24** | 「比较优势」思想源起补完与双 HTML 演说「地层剖面 / 工程蓝图」重构 + Playwright 渲染自检 | 待探讨 ISSUE #9 & #10 | ✅ **已完成** | 2026-11-19 13:05 |
| **Phase 25** | Web UI 美化（白天工程蓝图 / 夜间深空双主题切换 + 面板去裸 JSON）与「路演演示剧场」三层故事化重构 | 待探讨 ISSUE #11 & #12 | ✅ **已完成** | 2026-11-19 13:50 |
| **Phase 26** | 数据治理硬化（黑名单零逃逸/`restricted` 落地/噪音门禁）+ `LinksEngine` 真实依赖图谱闭环 + v0.4.0 四层全量对齐 | 规约 #2 #4 #5 #6 #10 #11 | ✅ **已完成** | 2026-11-19 14:50 |

> 详见 [roadmap-phase10-11.md](roadmap-phase10-11.md)、[roadmap-phase12.md](roadmap-phase12.md)、[roadmap-phase13.md](roadmap-phase13.md)、[roadmap-phase14.md](roadmap-phase14.md)、[progress-phase14.md](progress-phase14.md)、[roadmap-phase15-18.md](roadmap-phase15-18.md)、[roadmap-phase15.md](roadmap-phase15.md)、[progress-phase15.md](progress-phase15.md)、[roadmap-phase17.md](roadmap-phase17.md)、[progress-phase17.md](progress-phase17.md)、[roadmap-phase18.md](roadmap-phase18.md)、[progress-phase18.md](progress-phase18.md)、[roadmap-phase19-20.md](roadmap-phase19-20.md)、[progress-phase19.md](progress-phase19.md)、[roadmap-phase21-22.md](roadmap-phase21-22.md) 与 [progress-phase21.md](progress-phase21.md)

---

## 阶段 13 子任务追踪清单 (Phase 13 Breakdown)

| 阶段 | 子任务 ID | 任务内容 | 涉及路径 | 状态 | 检查点 |
|------|-----------|----------|----------|:---:|:------|
| **Phase 13** | 13.1 | 轻量异步 HTTP 引擎与 REST API 核心框架 | `src/web/`, `Cargo.toml` | ✅ 已完成 | `[x]` 路由与探活接口 |
| | 13.2 | 宿主机事实拓扑与依赖爆炸半径图谱 API | `src/web/topology.rs` | ✅ 已完成 | `[x]` 图数据模型与聚合 |
| | 13.3 | 协作锁实时监控看板与事件流 API | `src/web/locks_api.rs`, `src/web/sse.rs` | ✅ 已完成 | `[x]` 活跃锁与 SSE 推流 |
| | 13.4 | 自包含单页面大屏与前端交互引擎 | `src/web/assets/` | ✅ 已完成 | `[x]` 深色大屏与无外部CDN |
| | 13.5 | CLI 命令、Daemon 常驻集成与配置联动 | `src/main.rs`, `src/config/` | ✅ 已完成 | `[x]` web 子命令与防撞 |
| | 13.6 | 工业级测试验证、官方手册与全量文档同步 | `tests/`, `manual/08-web-dashboard.md` | ✅ 已完成 | `[x]` 端到端单测与手册 |

---

## 阶段验收结论 (Phase 13: Web UI 与可视化监控大屏)

🎉 **Phase 13（Web UI 与可视化监控大屏）已 100% 全量串行交付**：
- 严格对齐 README.md ISSUE #13（Web UI / 可视化监控大屏：提供基于浏览器的轻量级大管家拓扑图与协作锁监控看板）；
- **核心架构**: 基于 Tokio 原生异步 TCP 构建轻量高性能 HTTP/1.1 与 SSE 引擎，保持单二进制自包含交付，零外部 Node.js/Nginx/CDN 依赖；
- **事实拓扑图谱**: 聚合 `/proc` 进程、监听端口、Systemd 守护服务、Crontab 定时任务与核心资产，输出符合标准图协议的 Nodes & Edges，支持定向查询爆炸半径多跳子图；
- **协作锁监控看板**: 实时暴露 Phase 11 `CoordinationEngine` 活跃锁矩阵、租期 TTL 动态倒计时与排队态势，支持受控管理员强制解除超时孤儿锁；
- **全链路推流**: 实现 `/api/v1/stream` (Server-Sent Events) 长连接推流，实现感知事件秒级推屏；
- **CLI 与守护集成**: 新增 `agent-steward web [--host] [--port] [--auth-token]` 命令，并在 `steward daemon` 常驻后台中按需托管拉起；
- **工业级全量测试**: 新增 4 项 Web 模块单元测试与 1 项端到端全生命周期集成测试（`tests/test_web_dashboard.rs`），全系统 145 项测试 **100% 全部通过**；
- **官方使用手册**: 发布 `manual/08-web-dashboard.md` 并同步更新 `manual/03-cli-reference.md` 与根目录 `README.md`。

---

## 验收总结 (Phase 1~12 全部完成)
- **单元与集成测试**：122 项单元测试 + 8 项协作锁集成测试 + 5 项 DuckDB 集成测试 + 5 项高性能基准测试共 140 项全部 100% PASS，0 warning。
- **运行依赖与环境诊断**：新增 `src/preflight/` 预检引擎（REQUIRED / OPTIONAL / INFO 三级判断），CLI 提供 `agent-steward preflight`，守护进程启动自动校验；
- **配置与可移植部署**：提供带详注的 `config.example.toml`，提供生产部署指南 `INSTALL.md`（含多发行版依赖矩阵与排障 FAQ），提供多阶段 `Dockerfile`、`docker-compose.yml` 与 `Makefile`；
- **存储后端**：支持 SQLite 嵌入式、PostgreSQL (pgvector + tsvector) 和 DuckDB (OLAP 列存分析与 Parquet/CSV 导出) 三后端；实现跨库双向热迁移与毫秒级增量同步管道。
- **多 Agent 协作**：实现 `CoordinationEngine`、资源正规化、排他/共享锁、TTL 自动失效、续租、父子目录递归互斥、PG Advisory Lock (跨连接会话级事务保证)、以及 Pre-Hook 自动冲突拦截。
- **分析与协作工具**：系统原生 MCP 工具总数扩充至 **31 个**。
- **SKILL 体系**：全面刷新项目内置 3 套技能，测试、工具数与代码事实保持 100% 对齐。
- **后台守护进程**：`agent-steward.service` 在后台持续稳定运行。
- **共享仓库同步**：代码已同步至 `/root/new/shared_git/agent-steward`。

---

## 断点恢复指引
如果任务中断，重新启动时只需：
1. 读取本文件 `src_doc/progress.md`；
2. 查看当前全部完成的里程碑记录与下一阶段规划；
3. 当前待启动的 Phase 19~20 详细任务与检查点位于 [roadmap-phase19-20.md](roadmap-phase19-20.md) 与 [progress-phase19.md](progress-phase19.md)，按标准流程串行推进。

🎉 **Phase 12（运行依赖显式化与可移植部署）已 100% 全量串行交付**：
- 严格对齐 README.md ISSUE #10 四层贯通要求；
- 代码层：`PreflightChecker` 逐项验证 procfs、inotify、data_dir、SQLite、DuckDB、Systemd、CodeGraph、Ollama，`build.rs` 注入 Git Commit 与平台信息，`Config::validate` 拦截非法配置；
- 配置层：`config.example.toml` 逐参数注释说明、取值范围与依赖前提；
- 文档层：`INSTALL.md` 覆盖编译依赖、运行依赖、四大发行版安装、Docker、Systemd 与排障速查表；
- 技能层：更新 `agent-steward-guide`、`agent-steward-developer`、`agent-steward-doc-writer` 保持 100% 事实一致；
- 工程化：`Dockerfile`、`docker-compose.yml`、`Makefile` 一应俱全。

🎉 **Phase 11（多 Agent 协作锁与互斥协调中枢）已 100% 全量串行交付**：
- 对齐新增 ISSUE #10；
- 实现了 `CoordinationEngine` 协作锁引擎：支持 Port、Directory、Service、File、Custom 5 大资源类型，支持 Exclusive 与 Shared 两种锁模式；
- 存储层 SPI 扩充 5 个锁生命周期方法（`insert_coordination_lock`, `delete_coordination_lock`, `get_coordination_lock`, `list_coordination_locks`, `gc_expired_coordination_locks`），全面覆盖 SQLite、PostgreSQL (集成 Advisory Lock)、DuckDB；
- 实现了目录层级祖先/后代递归互斥传播（持有父目录排他锁自动阻止子目录操作，持有子目录排他锁自动阻止父目录独占）；
- 实现了 `steward_lock_acquire`、`steward_lock_release`、`steward_lock_list` 3 个原生 MCP 端点（原生工具数达到 31 个）；
- 深度集成 `AgentHookEngine` 的 `hook_pre` 门禁，在绑定端口、修改文件时自动检测排他锁冲突；
- 深度集成守护进程后台调度 `run_inspection_cycle`，定时自动 GC 过期锁；
- 新增 CLI `agent-steward coordination [list|release|gc]` 命令；
- 8 项多 Agent 并发与互斥集成测试（`tests/coordination_integration.rs`）100% PASS！

🎉 **Phase 10（DuckDB 分析后端与增量同步管道）已 100% 全量串行交付**：
- 对齐 README.md ISSUE #4；
- 实现了 `DuckDbStorageProvider` 存储层 SPI 全部 35 个方法，内置 FTS 与 3-hop 递归图遍历；
- 实现了 `DuckDbAnalyticsEngine` 与 `AnalyticsProvider` 扩展：变更热力图、变更走势、高频变更 Top-N、事实图谱全景统计、Parquet/CSV 原生导出；
- 实现了 `SyncPipeline` 增量同步管道，基于 `sync_watermark` 高水位实现秒级增量同步与全量迁移；
- 扩展了 CLI `storage sync` 子命令与 `storage migrate --to duckdb`；
- 注册了 3 个分析 MCP 端点，工具总数升至 28 个；
- 5 项 DuckDB 集成测试与 2 项 OLAP 基准测试 100% PASS，导出 1000 条资产耗时仅 ~50ms！

🎉 **Phase 5（宿主机主动扫描与智能记录引擎）已 100% 全量串行交付**：
- 对齐 README.md ISSUE #9、#2、#3；
- 实现了 `ScanStateMachine` 断点续扫状态机，基于确定性文件排序与持久化检查点，实测支持单步挂起与无损续扫；
- 实现了 `ImportanceScorer` 与 `NoiseFilter` 智能质量门禁，阻断临时缓存与依赖垃圾入库，并在记忆入库端实现 `MemoryIntakeFilter`；
- 实现了 `fact_nodes` / `fact_edges` 宿主机属性图，`PropertyGraphOrganizer` 全景关联工程、守护服务、端口与 Cron，自动生成规整 Mermaid 依赖图；
- 实现了 `AgentHookEngine` 与 `steward_hook_pre` / `steward_hook_post` 原生 MCP 工具（工具总数升至 25 个）；
- 后台守护集成 `BackgroundScheduler` 30 分钟自巡检；
- 全系统单元测试 **101 项 100% 全部通过**；Release 二进制已更新部署，`agent-steward.service` 稳定在线！

🎉 **三位一体架构 Phase 4（自适应情境组装与全景中枢）已 100% 全量串行交付**：
- 详见路线图 [roadmap-modular-phase4.md](roadmap-modular-phase4.md)；
- 实现了 `BriefAssembler` 自适应组装器，在 `≤ budget_tokens` 内根据任务意图动态从事实、偏好、记忆三模块融合采样；
- 升级了原生 MCP 工具 `steward_brief`，支持 `query`、`session_id`、`budget_tokens` 动态调控；
- 升级了 TUI 实时终端看板；
- 全系统单元测试 **89 项 100% 全部通过**；Release 二进制已热部署，`agent-steward.service` 生产守护稳定运行！
