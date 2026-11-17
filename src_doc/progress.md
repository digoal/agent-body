# Agent Host Steward (Rust 版) — 研发与测试进度表（断点续传）

> 创建时间: 2026-11-15T12:50:00+08:00  
> 最新更新: 2026-11-15T19:15:00+08:00  
> 技术栈: Rust 1.99.0 + SQLite 3 (rusqlite bundled with FTS5) + Tokio + Clap  
> 当前状态: Phase 1~6 全部完成，Phase 7 规划就绪待启动

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
| | 3.3 | 可插拔记忆：成熟开源生态适配器 (Mem0 / Letta / PowerContext) | `src/memory/adapters/` | ⬜ 待执行 | - |

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
- 下一阶段为 **三位一体 Phase 3：可插拔记忆与成熟开源适配 (Pluggable Memory & Open Ecosystem)**。

---

## 断点恢复指引
如果任务中断，重新启动时只需：
1. 读取本文件 `src_doc/progress.md`；
2. 找到首个状态为 `⏳ 进行中` 或 `⬜ 待执行` 的子任务 ID（当前为三位一体 Phase 3 任务 3.1，详见 [roadmap-modular-phase3.md](roadmap-modular-phase3.md)）；
3. 继续按顺序串行执行，每完成一步立即更新对应路线图与本文件的状态与完成时间戳。
