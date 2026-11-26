# Agent Body 演进进度总表 (Progress Tracker)

> **当前版本**：`v1.0.0 GA` | **演进策略**：Phase 1~51 历史规划与进度明细已归档至 [docs/src_doc/archive/](archive/)，本表聚焦主线里程碑、近期 Phase 52~57 交付台账与断点恢复入口。

---

## 一、历史里程碑全景（Phase 1 ~ 51 · 已归档）

| 阶段区间 | 核心演进主题 | 代表交付 | 归档详情 |
|:---:|---|---|---|
| **Phase 1~4 (模块化 Phase 1~4)** | 宿主事实图谱、放置规约与自学习、可插拔记忆 SPI、自适应情境组装 | Placement / Granularity Scanner / FTS5+BGE-M3 / TUI | [archive/roadmap-modular-phase1.md](archive/roadmap-modular-phase1.md) ~ [4](archive/roadmap-modular-phase4.md) |
| **Phase 5~13** | 断点续扫、PostgreSQL + DuckDB 三后端、`BodyResponse`、跨平台抽象、多 Agent 协作锁、Web UI 1.0 | 3 存储引擎 + 31 MCP 工具 + Preflight + Web 大屏 | [archive/roadmap-next-phases.md](archive/roadmap-next-phases.md) ~ [13](archive/roadmap-phase13.md) |
| **Phase 14~20** | Hook 驱动引擎、零 Panic 硬化、生态目录与防爆路由、WFG 死锁唤醒、Peer Mesh、Web UI 2.0 | EventBus / AutoSync / WFG / Peer Mesh / `make sync-mirror` | [archive/roadmap-phase14.md](archive/roadmap-phase14.md) ~ [20](archive/roadmap-phase19-20.md) |
| **Phase 21~28** | 黑白屏 1:1 Parity、7 幕路演演示剧场、Ollama L1/L2 分层语义、比较优势思想起源、实证 Benchmark 内核 | Benchmark `v1.0` / `COMP` 基础框架 / 昼夜蓝图主题 | [archive/roadmap-phase21-22.md](archive/roadmap-phase21-22.md) ~ [28](archive/roadmap-phase27-28.md) |
| **Phase 29~39** | 全景属性字典、反馈总账与新鲜度写穿 (`FreshnessReconciler`)、`SecretRedactor` & 抗投毒、`EVOLUTION_GUIDE.md`、竞品对标 `v3.0`、帕累托双门禁 `v3.1~v3.3` | 零重扫活地图 + `P1~P5` 五点探针 + `DQ1~DQ5` | [archive/roadmap-phase29.md](archive/roadmap-phase29.md) ~ [39](archive/roadmap-phase38-39.md) |
| **Phase 40~43** | 多维证据分层图谱 (`L1~L3`)、`MeshEnvelope v1.0` HMAC 防重放、AOPCC 乐观-悲观并发控制 (`<2ms` 自愈)、双域全链路审计与 Watcher 60s 折叠 | `W13~W16` / `DQ6~DQ7` / `PR-11~12` | [archive/roadmap-phase40-41.md](archive/roadmap-phase40-41.md) ~ [43](archive/roadmap-phase43.md) |
| **Phase 44~51** | 升维「脑体分离 · Agent 本体」(`src/body/` 第 43 工具)、全面硬切 `agent-body`、600% CPU 根治、内生 18 Wait Events + AWR/ADDM、体系化演说 HTML 矩阵 | 9 份演说 HTML + 5 重门禁 + `W17` / `DQ8` | [archive/roadmap-phase44-45.md](archive/roadmap-phase44-45.md) ~ [51](archive/roadmap-phase51.md) |

---

## 二、近期冲刺交付记录（Phase 52 ~ 57 · `v1.0.0 GA` 主线）

| Phase | 主题 | 核心交付摘要 | 对应 ISSUE | 路线图 | 状态 |
|:---:|---|---|---:|---|:---:|
| **Phase 52** | 方案 A Stage 1 — P0 安全止血与运行时排雷 | 修复 `Dockerfile`/`docker-compose`/`build.rs`、MCP `spawn_blocking` 与防 Panic、`body_bridge.py` 破坏命令与 Web `GET` 写库鉴权、`ledger` 死锁误报/拒锁、`cli_args` 布尔参数与文档断链 | `#36` | [roadmap-phase52.md](roadmap-phase52.md) | ✅ **已完成** |
| **Phase 53** | 方案 A Stage 2 — 三后端存储对齐与 OLAP 真修复 | SPI `82→87`，消除 PG `pgvector` 强依赖、`memories.sensitive` 对齐、`awr:%` 隔离与淘汰、20 实体热迁移、`SyncPipeline` 10 表与真实 DuckDB 时间窗分析 | `#37` | [roadmap-phase53.md](roadmap-phase53.md) | ✅ **已完成** |
| **Phase 54** | 方案 A Stage 3 — Telemetry 18/18 与动作级真 Parity | SPI #88 修复过期锁检测、18/18 WaitEvent 全埋点、`perf-delete`/AWR 报告检索全端 1:1:1、Web DQ1~9、`codex`/`hermes` Hook、全维 TUI 与 Benchmark `v3.8` (`W18`/`M6`/`DQ9`/`PR-14`) | `#38` | [roadmap-phase54.md](roadmap-phase54.md) | ✅ **已完成** |
| **Phase 55** | 方案 A Stage 4 — 可移植解耦与跨平台加固 | `[workspace]` 抽象、三后端动态种子去硬编码、打通 `AGENT_BODY_CONFIG`/`MemoryFactory`/`[[hooks]]`/`[[pipelines]]`/`ignore_patterns`、`gnu` 条件编译、Windows `netstat` 与 32KB 有界 HTTP 读取 | `#39` | [roadmap-phase55.md](roadmap-phase55.md) | ✅ **已完成** |
| **Phase 56** | 架构治理 — 拆 God File、压吞错、补集成测试与升级门禁 | `>1500L` 生产文件 `7→0` (`assets`/`parity_api`/`perf_awr`/`cli_handlers`/`sqlite|pg|duckdb_provider`)；`let _ =` 降为 `43/50`（核心自治模块 `0`）+ `RunningFlagGuard`；新增 5 组跨模块集成测试；五重确定性门禁升级 | `#40` | [roadmap-phase56.md](roadmap-phase56.md) | ✅ **已完成** |
| **Phase 57** | **全物料大一统与 `v1.0.0 GA` 正式发版** | 损毁规格修复、3 SKILL + 10 卷手册 + 架构文档刷新、历史文档归档、9/9 演说对齐与公众长文重写、`Cargo.toml` 升 `1.0.0` + `v1.0.0.md` 发布验收 | `#41` | [roadmap-phase57.md](roadmap-phase57.md) | ✅ **已完成** |
| **Phase 58** | **三态运行模式与真实 A/B 证伪 + MCP Profile 减税** | `[runtime]` 三态模式 (`body_only`/`bypass`/`shadow_ab`)、`ShadowAbEngine` 真实对照与帮倒忙率诊断、MCP `core` 10 工具档位省 ~78% Schema 税、原生 `hooks bridge`、Web CORS 收敛与 `HybridSearch` 单次查库 | `#43` | [roadmap-phase58.md](roadmap-phase58.md) | ✅ **已完成** |
| **Phase 59** | **多模态对象理解深化与资产 RRF+KV 混合检索** | `multimodal.rs` 三层级多模态 L1/L2 提取、`vision_enabled` 可选图像视觉理解、解除 `3000/5000` 后台截断、`AssetSearchEngine` FTS5+Vector RRF+`modality/domain/kv` 1:1:1 对齐 CLI/MCP/Web | `#42` | [roadmap-phase59.md](roadmap-phase59.md) | ✅ **已完成** |
| **Phase 60** | **跨平台 100% 全绿测试基线与 Clippy 零告警收口** | 修复 `links/audit` 临时目录、`upsert_scan_policy` 三后端幂等、`W15`/`diagnoser`/`placement`/`state_machine`/`watcher`/`fact` 等 14+2 项非 Linux 测试失败、清理 `thiserror`/`glob` 死依赖并达成 `clippy --all-targets -D warnings` 零告警 | `v1.3` | [roadmap-phase60.md](roadmap-phase60.md) | ✅ **已完成** |
| **Phase 61** | **Know-How 知识网络 (Stage 1) 与 Omni-RRF 多路融合召回** | 扩展 `fact_nodes`/`fact_edges` 知识节点与 SQLite 热迁移、`HeadingChunker` + 别名归一 + PMI 共现图、加权 `bm25` + 驼峰拆词 + Pure-Rust `fuzzy` + 图 1~2 跳 + 层级 + 互链 + 标量调制 Omni-RRF，修复 `ShadowAbEngine` 0 命中回退 | `#44` | [roadmap-phase61.md](roadmap-phase61.md) | ✅ **已完成** |
| **Phase 62** | **Know-How 知识网络 (Stage 2)：索引子图下沉、小模型 L2 抽取与 `v3.9` 基准** | 索引下沉定向 1~2 跳子图与枢纽阻尼、写穿级联清理 (`FreshnessReconciler`)、`qwen3.5:0.8b` (`think=false`) 章节 L2 Know-How 原文锚定抽取、Web 通道/推理可视化与 Benchmark `v3.9` (`W19`/`DQ10`) | `#44` | [roadmap-phase62.md](roadmap-phase62.md) | ✅ **已完成** |
| **Phase 63** | **多模态解析深化、三通道统一与 Shadow A/B `H1~H5` 自愈 (`v1.6`)** | `multimodal_ext.rs` 纯 Rust HTML/Parquet/CSV/JSONL/音频 L1A、`run_command_bounded` 有界探针、三大入口统一魔数分类与 `schema_version=2.0`、`kv_filter`+无损回填全端 1:1:1、运行模式 `.override` 热切与 `ShadowAbEngine` `H1~H5` 自动写穿自愈 | `v1.6` | [roadmap-phase63.md](roadmap-phase63.md) | ✅ **已完成** |

---

## 三、断点恢复指引

1. 新会话启动时，优先阅读根目录 [EVOLUTION_GUIDE.md](../../EVOLUTION_GUIDE.md) 与最近活跃路线图 [roadmap-phase63.md](roadmap-phase63.md)；
2. 严禁全仓扫描 `docs/src_doc/archive/` 内已冻结的历史路线图；
3. 每次代码/文档改动收口前执行 `make quality-gate && make test && make verify-decks` 与 `make sync-mirror` 校验零漂移。
