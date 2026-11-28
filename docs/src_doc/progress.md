# Agent Body 演进进度总表 (Progress Tracker)

> **当前版本**：`v1.1.0` | **演进策略**：Phase 1~51 历史规划与进度明细已归档至 [docs/src_doc/archive/](archive/)，本表聚焦主线里程碑、Phase 52~75 及品质巩固交付台账与断点恢复入口。

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
| **Phase 44~51** | 升维「脑体分离 · Agent 本体」(`src/body/` 第 43 工具)、全面硬切 `agent-body`、600% CPU 根治、内生 18 Wait Events + AWR/ADDM、体系化演说 HTML 矩阵 | 9 份演说 HTML + 确定性质量门禁 + `W17` / `DQ8` | [archive/roadmap-phase44-45.md](archive/roadmap-phase44-45.md) ~ [51](archive/roadmap-phase51.md) |

---

## 二、近期冲刺与品质巩固交付台账（Phase 52 ~ 75+ · `v1.0.0` → `v1.1.0`）

| Phase / 里程碑 | 主题 | 核心交付摘要 | 对应 ISSUE | 路线图 / 凭证 | 状态 |
|:---:|---|---|---:|---|:---:|
| **Phase 52** | 方案 A Stage 1 — P0 安全止血与运行时排雷 | 修复 `Dockerfile`/`docker-compose`/`build.rs`、MCP `spawn_blocking` 与防 Panic、`body_bridge.py` 破坏命令与 Web `GET` 写库鉴权、`ledger` 死锁误报/拒锁、`cli_args` 布尔参数与文档断链 | `#36` | [roadmap-phase52.md](roadmap-phase52.md) | ✅ **已完成** |
| **Phase 53** | 方案 A Stage 2 — 三后端存储对齐与 OLAP 真修复 | SPI `82→87`，消除 PG `pgvector` 强依赖、`memories.sensitive` 对齐、`awr:%` 隔离与淘汰、20 实体热迁移、`SyncPipeline` 10 表与真实 DuckDB 时间窗分析 | `#37` | [roadmap-phase53.md](roadmap-phase53.md) | ✅ **已完成** |
| **Phase 54** | 方案 A Stage 3 — Telemetry 18/18 与动作级真 Parity | SPI #88 修复过期锁检测、18/18 WaitEvent 全埋点、`perf-delete`/AWR 报告检索全端 1:1:1、Web DQ1~9、`codex`/`hermes` Hook、全维 TUI 与 Benchmark `v3.8` (`W18`/`M6`/`DQ9`/`PR-14`) | `#38` | [roadmap-phase54.md](roadmap-phase54.md) | ✅ **已完成** |
| **Phase 55** | 方案 A Stage 4 — 可移植解耦与跨平台加固 | `[workspace]` 抽象、三后端动态种子去硬编码、打通 `AGENT_BODY_CONFIG`/`MemoryFactory`/`[[hooks]]`/`[[pipelines]]`/`ignore_patterns`、`gnu` 条件编译、Windows `netstat` 与 32KB 有界 HTTP 读取 | `#39` | [roadmap-phase55.md](roadmap-phase55.md) | ✅ **已完成** |
| **Phase 56** | 架构治理 — 拆 God File、压吞错、补集成测试与升级门禁 | `>1500L` 生产文件 `7→0`；`let _ =` 降为 `43/50`（核心自治模块 `0`）+ `RunningFlagGuard`；新增 5 组跨模块集成测试；质量门禁升级 | `#40` | [roadmap-phase56.md](roadmap-phase56.md) | ✅ **已完成** |
| **Phase 57** | **全物料大一统与 `v1.0.0 GA` 正式发版** | 损毁规格修复、3 SKILL + 10 卷手册 + 架构文档刷新、历史文档归档、9/9 演说对齐与公众长文重写、`Cargo.toml` 升 `1.0.0` + `v1.0.0.md` 发布验收 | `#41` | [roadmap-phase57.md](roadmap-phase57.md) | ✅ **已完成** |
| **Phase 58** | **三态运行模式与真实 A/B 证伪 + MCP Profile 减税** | `[runtime]` 三态模式 (`body_only`/`bypass`/`shadow_ab`)、`ShadowAbEngine` 真实对照与帮倒忙率诊断、MCP `core` 10 工具档位省 ~78% Schema 税、原生 `hooks bridge`、Web CORS 收敛与 `HybridSearch` 单次查库 | `#43` | [roadmap-phase58.md](roadmap-phase58.md) | ✅ **已完成** |
| **Phase 59** | **多模态对象理解深化与资产 RRF+KV 混合检索** | `multimodal.rs` 三层级多模态 L1/L2 提取、`vision_enabled` 可选图像视觉理解、解除 `3000/5000` 后台截断、`AssetSearchEngine` FTS5+Vector RRF+`modality/domain/kv` 1:1:1 对齐 CLI/MCP/Web | `#42` | [roadmap-phase59.md](roadmap-phase59.md) | ✅ **已完成** |
| **Phase 60** | **跨平台 100% 全绿测试基线与 Clippy 零告警收口** | 修复跨平台测试失败、清理死依赖并达成 `clippy --all-targets -D warnings` 零告警 | `v1.3` | [roadmap-phase60.md](roadmap-phase60.md) | ✅ **已完成** |
| **Phase 61** | **Know-How 知识网络 (Stage 1) 与 Omni-RRF 多路融合召回** | 扩展 `fact_nodes`/`fact_edges` 知识节点、`HeadingChunker` + 别名归一 + PMI 共现图、加权 `bm25` + 驼峰拆词 + Pure-Rust `fuzzy` + 图/层级/互链/标量 Omni-RRF | `#44` | [roadmap-phase61.md](roadmap-phase61.md) | ✅ **已完成** |
| **Phase 62** | **Know-How 知识网络 (Stage 2)：索引子图下沉与 `v3.9` 基准** | 索引下沉定向 1~2 跳子图与枢纽阻尼、写穿级联清理、`qwen3.5:0.8b` 章节 L2 Know-How 原文锚定抽取与 Benchmark `v3.9` (`W19`/`DQ10`) | `#44` | [roadmap-phase62.md](roadmap-phase62.md) | ✅ **已完成** |
| **Phase 63** | **多模态解析深化、三通道统一与 Shadow A/B `H1~H5` 自愈 (`v1.6`)** | `multimodal_ext.rs` 纯 Rust 解析、三大入口统一魔数分类与 `schema_version=2.0`、`kv_filter`+无损回填全端 1:1:1、运行模式 `.override` 热切与 `ShadowAbEngine` `H1~H5` 自动写穿自愈 | `v1.6` | [roadmap-phase63.md](roadmap-phase63.md) | ✅ **已完成** |
| **Phase 64** | **`v1.1.0` 一等公民大一统对齐 (Web UI / SKILL / 手册 / 门禁)** | 升级 `evolution_manifest.py` 防漂移门禁、`Cargo.toml` 升至 `1.1.0`、Web UI 补齐 Action-level 1:1:1 交互，全量同步 SKILL / 手册 / 文档与 `v1.1.0` 发版说明 | `#46` | [roadmap-phase64.md](roadmap-phase64.md) | ✅ **已完成** |
| **Phase 65~66** | **效果真实性与可信度量攻坚 + 跨平台打磨与 GitHub CI/CD** | 修复 `DQ2`/`DQ6`/`vitals` 十万边截断失真、优化代码标识符 `exact_id` 召回/延迟、消除 macOS 残留并新增双平台 `.github/workflows/{ci,release}.yml` | `#47`, `#48` | [roadmap-phase65-66.md](roadmap-phase65-66.md) | ✅ **已完成** |
| **Phase 67** | **全仓 `< 1000L` 模块化拆分与 `let _ = <= 15` 吞错治理** | 拆分全部 10 个 `>=1000L` 生产文件至 `<1000L`，修复 `prod_slice` 原始字符串漏检并收紧 `code_hygiene.py` 红线至 `>1000L = 0`、`let _ = <= 15` | `#49` | [roadmap-phase67.md](roadmap-phase67.md) | ✅ **已完成** |
| **Phase 68** | **一等公民 SSoT 交叉对齐与防漂移门禁闭环** | `evolution_manifest.py` 新增版本号/反引号源码路径/`parity_api` 43 路由/演说 SSoT 四重硬门禁；重构 4 套演说生成器动态读取快照；补齐 `src/web/mod.rs` 三端交叉契约单测 | `#50` | [roadmap-phase68.md](roadmap-phase68.md) | ✅ **已完成** |
| **Phase 69** | **Top-5 高水位模块预防性拆分与 SQLite/DuckDB 共享 SPI 契约测试** | 预防性拆分 5 个高水位模块至 `<700L`；新增 `tests/storage_spi_contract.rs` 对 SQLite 与 DuckDB 同套 12 域 SPI 契约等价验证 | `#51` | [roadmap-phase69.md](roadmap-phase69.md) | ✅ **已完成** |
| **Phase 70~73** | **微内核 (`kernel/`) 与周边 (`contrib/`) 分离重构与三大精简发行版** | 构建 `body-sdk`/`body-protocol`/`body-kernel` 纯机制微内核与防腐门禁；落地 9 个标准五件套周边扩展；组装验证 `kernel-minimal`/`aiops-probe`/`dba-sentinel` | `#52` | [roadmap-phase70-73.md](roadmap-phase70-73.md) | ✅ **已完成** |
| **Phase 74** | **全栈工业级可观测闭环 (`obs_telemetry` + `/metrics` + W3C Trace)** | 修复 `pipeline.rs` `trace_id` 断链并落地微内核纯机制原语；新增第 10 个 `contrib/obs_telemetry/`；旗舰版新增 `GET /metrics`、`X-Trace-Id`/`traceparent` 与 `AGENT_BODY_LOG_FORMAT=json` | `#53` | [roadmap-phase74.md](roadmap-phase74.md) | ✅ **已完成** |
| **Phase 75** | **全一等公民交叉验证与大一统（Check D~G + `body metrics` 1:1:1）** | 升级 `evolution_manifest.py` Check D~G；加固 CI/Release/Docker (`--target probe`)；补齐旗舰 `body metrics` CLI/MCP；全面刷新手册、架构文档、9 份演说与 `architecture.svg` | `#54` | [roadmap-phase75.md](roadmap-phase75.md) | ✅ **已完成** |
| **v1.1.0 视觉重塑** | **9 份演说 HTML 形象化 SVG 图解引擎重构 (`225fb2c`)** | 新增 `scripts/deck_visuals.py`（28+ 矢量图标与 20 套专属领域架构图）、舞台画布扩至 `1472×640`（72%+ 图示占比），9/9 通过 `make verify-decks` Playwright 无头验真 | — | [presentation/index.html](../../presentation/index.html) | ✅ **已完成** |
| **v1.1.0 品质硬化** | **老库无损 Schema 自愈、11 处缺陷根治与全栈 414 测 + WebUI E2E (`ce7c828`)** | 落地 `schema_reconcile.rs` 事务式老库列改名/补列与 FTS5 脱同步重建、SQLite WAL `64MB` 控容与 CJK 回退检索；修复 Benchmark 报错吞错、记忆类型归一、Mesh 路由、锁 Token 与 MCP 必填校验；新增 18 项测试（全工作区 **414 测**）与 `make verify-webui` Playwright 9 工作台 E2E 门禁 | — | [EVOLUTION_GUIDE.md](../../EVOLUTION_GUIDE.md) | ✅ **已完成** |
| **Phase 76 (M1)** | **PostgreSQL SPI 事务级海马体 · 方案 A 事务底座 (`PgTxMemoryProvider`)** | 落地非破坏性 `TxMemoryProvider` SPI、第 11 扩展 `contrib/mem_pg_tx/` 五件套与前后向兼容 4 表 DDL、有界连接池 + 同键微序列化写穿 + `REPEATABLE READ` 快照读 + LSM 解耦段 + SQLite 降级 + `tx_epoch` 贯通 AOPCC/胶囊/老库自愈 + `W20` 压测（PG 18 实测 16 线程 80tx 丢写 `0%`、P50 `3.28ms` / P99 `4.34ms`、僵尸召回 `0%`） | `#55` | [roadmap-phase76-78.md](roadmap-phase76-78.md) | ✅ **已完成** |
| **Phase 77 (M2)** | **方案 A 满血形态 · `L1_AUTONOMIC` 空闲期睡眠整理 (`SleepTimeCompactor`)** | STC 显著性打分 (`SalienceScorer`)、空闲窗口探测与异步压缩、不可预测查询自动旁路、双域审计全留痕与 Benchmark `v4.0` (`W21`/`DQ11`/`PR-15`)（PG 18.6 实测压缩比 `3.73×` `272→73 tok`、事实保留 `100%`、热路径 `P50=5.19ms` / `P99=10.65ms`） | `#55` | [roadmap-phase76-78.md](roadmap-phase76-78.md) | ✅ **已完成** |
| **Phase 78 (M3)** | **方案 B 旁路快权重快照血缘与神经 Compaction 门禁** | `FastWeightCheckpointSpi` + JSON-RPC 2.0 Runtime 线协议（零推理/权重进本体）、`fast_weight_snapshots` 递归血缘、三道硬门禁 (`norm_ratio`/`eval_glue_delta`/`unlearn_leak_rate`) 原子切换、`selective_unlearn` 定向遗忘、`tx_epoch` 秒级回滚、`ContinuityCapsule v2`（`base_model_id` 同基座热挂）与 Bench `v4.1` (`W22`/`DQ12`/`PR-16`)（PG 18.6 100 万事件实测 `search_tx P50=2.66ms`/`P99=4.42ms`、`lineage P99=1.94ms`、回滚 `24ms`） | `#55` | [roadmap-phase76-78.md](roadmap-phase76-78.md) | ✅ **已完成** |
| **Phase 79** | **超集检索 × DB 侧模糊索引 × 真基线 A/B 与 `contrib/graph_relate`** | `assets.content_text` 脱敏正文索引 + 6 列 `assets_fts` + 第 28 张虚表 `assets_trigram_fts`（95 SPI / 28 表）、`QueryIntent`/`apply_precision_gate` 防噪门禁、`live_search` 真基线 A/B 对照、第 12 扩展 `contrib/graph_relate`（嵌入式 openCypher） | `#56` | [roadmap-phase79.md](roadmap-phase79.md) | ✅ **已完成** |
| **Phase 80** | **原生 PreToolUse Token 守卫 × 默认忽略扩展 × 开箱优化套件** | `TokenGuardEngine`（噪音目录/锁文件/单行压缩拦截、大文件首屏切片 `overwrite`、`rg`/`grep -r`/`find` 自动过滤与 `180` 行上限）、`hooks generate --client agy` 3 伴生文件（含 `token_optimizer.py` + `.ripgreprc`）、`mcp_profile="dev"` 减税 `-58.1%` 工具/`-51.6%` 字节、`graph_relate` 卫生清零与 **452 项全工作区测试全绿** | `#57` | [roadmap-phase80.md](roadmap-phase80.md) | ✅ **已完成** |

---

## 三、断点恢复指引

1. 新会话启动时，优先阅读根目录 [EVOLUTION_GUIDE.md](../../EVOLUTION_GUIDE.md) 与当前活跃路线图 [roadmap-phase80.md](roadmap-phase80.md)；
2. 严禁全仓扫描 `docs/src_doc/archive/` 内已冻结的历史路线图；
3. 每次代码/文档改动收口前执行 `make quality-gate && RUST_TEST_THREADS=1 make test`（改动 Web/演说时追加 `make verify-webui` / `make verify-decks`）与 `make sync-mirror` 校验零漂移。
