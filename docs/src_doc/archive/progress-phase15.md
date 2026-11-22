# Phase 15 & Phase 16 断点续传检查点追踪表 (Progress Phase 15-16)

> **创建时间**: 2026-11-18T23:12:00+08:00  
> **当前状态**: 规划就绪，待确认启动实施  
> **全景路线图**: [roadmap-phase15-18.md](roadmap-phase15-18.md)  
> **详细实施设计**: [roadmap-phase15.md](roadmap-phase15.md)  
> **规约对齐**: [README.md 核心设计原则与约束 §10-§14](../../README.md#L247-L266)

---

## 一、Phase 15: 工程规约全面对齐与零 Panic 硬化

| 阶段 | 子任务 ID | 任务内容与检查点 | 涉及路径 | 状态 | 交付时间 |
|:---:|:---:|---|---|:---:|:---:|
| **15.1** | 15.1.1 | 存储层三后端 + 分析引擎 125 处 `.unwrap()` / `.expect()` 清零（规约 #12） | `src/storage/{pg_provider,sqlite_provider,duckdb_provider,duckdb_analytics}.rs` | ✅ 已完成 | 2026-11-18 23:19 |
| | 15.1.2 | 业务层 15 处 `.unwrap()` 清零 + 新增零 Panic 防回归检查命令 `make lint-panic` | `src/links/mod.rs`, `src/session/lifecycle.rs`, `src/hooks/pipeline.rs`, `src/fact/automation.rs`, `Makefile` | ✅ 已完成 | 2026-11-18 23:20 |
| | 15.1.T | 运行全量回归确认零 Panic 改造零退化 | `cargo test` | ✅ 已完成 | 2026-11-18 23:20 |
| **15.2** | 15.2.1 | 补齐 `asset register`、`hooks pre`、`hooks post`、`coordination acquire` 4 个 CLI 子命令 | `src/main.rs` | ✅ 已完成 | 2026-11-18 23:23 |
| | 15.2.2 | 新增 `analytics {heatmap, trend, export}` 3 个 CLI 子命令 & `close --dry-run` 对齐，实现 32/32 1:1 映射（规约 #13） | `src/main.rs`, `src/api/mcp_tools.rs`, `src/session/mod.rs` | ✅ 已完成 | 2026-11-18 23:23 |
| | 15.2.T | 编写并实跑 CLI 1:1 映射冒烟测试与单元测试 | `src/main.rs`, `tests/` | ✅ 已完成 | 2026-11-18 23:23 |
| **15.3** | 15.3.1 | 刷新 `README.md`、`docs/arch/01~03`、`docs/manual/03~04`、`docs/src_doc/development.md` 与实际代码事实对齐（19 模块/40 SPI/32 MCP/实测测试数） | `README.md`, `docs/arch/`, `docs/manual/`, `docs/src_doc/` | ✅ 已完成 | 2026-11-18 23:25 |
| | 15.3.2 | 同步刷新仓库内 `skills/agent-steward-*/SKILL.md` 消除旧版信息，校验全局链接与零警告构建 | `skills/`, `cargo check --tests` | ✅ 已完成 | 2026-11-18 23:25 |

---

## 二、Phase 16: 管理范围扩展与 Context 防爆路由器 (Open Issue #4)

| 阶段 | 子任务 ID | 任务内容与检查点 | 涉及路径 | 状态 | 交付时间 |
|:---:|:---:|---|---|:---:|:---:|
| **16.1** | 16.1.1 | 扩展三套数据库 Schema（`mcp_catalog`、`skills_catalog`、`external_services` 及 FTS 索引）与模型 | `src/storage/{schema.sql,pg_schema.sql,duckdb_schema.sql,models.rs}` | ✅ 已完成 | 2026-11-18 23:27 |
| | 16.1.2 | 扩展 `StorageProvider` SPI 与 SQLite/PG/DuckDB 实现 + `StorageMigrator`/`SyncPipeline` 同步 | `src/storage/{spi.rs,sqlite_provider.rs,pg_provider.rs,duckdb_provider.rs,migration.rs,sync_pipeline.rs}` | ✅ 已完成 | 2026-11-18 23:30 |
| | 16.1.T | 编写三引擎 Catalog CRUD 与 FTS 检索单元测试 | `src/storage/mod.rs` | ✅ 已完成 | 2026-11-18 23:30 |
| **16.2** | 16.2.1 | 实现 `McpConfigScanner`（多客户端 MCP 配置扫描 + 敏感环境变量值强制脱敏） | `src/ecosystem/mod.rs`, `src/ecosystem/scanner.rs`, `src/lib.rs` | ✅ 已完成 | 2026-11-18 23:31 |
| | 16.2.2 | 实现 `SkillCatalogScanner`（解析 `/root/new/skills` 与全局 `SKILL.md` YAML frontmatter、黑名单阻断、Token 估算） | `src/ecosystem/scanner.rs` | ✅ 已完成 | 2026-11-18 23:31 |
| | 16.2.T | 编写脱敏门禁与 SKILL/MCP 扫描单元测试 | `src/ecosystem/scanner.rs` | ✅ 已完成 | 2026-11-18 23:32 |
| **16.3** | 16.3.1 | 实现 `ExternalContactsManager`（外部服务名片管理、明文密钥泄露拦截、`config.toml` 冷启动推导） | `src/ecosystem/contacts.rs` | ✅ 已完成 | 2026-11-18 23:31 |
| | 16.3.2 | 实现轻量服务健康探活器（TCP/HTTP 超时检测、环境变量就绪状态检测，零捏造规约 #5） | `src/ecosystem/contacts.rs` | ✅ 已完成 | 2026-11-18 23:31 |
| | 16.3.T | 编写外部服务防泄密与探活模拟测试 | `src/ecosystem/contacts.rs` | ✅ 已完成 | 2026-11-18 23:32 |
| **16.4** | 16.4.1 | 实现 `ContextBudgetRouter`（多源意图召回、健康度加权、Token 预算截断与节省统计） | `src/ecosystem/router.rs` | ✅ 已完成 | 2026-11-18 23:31 |
| | 16.4.2 | 集成到 `BriefAssembler` 与 `BackgroundScheduler` 后台巡检自动更新 | `src/brief/assembler.rs`, `src/daemon/scheduler.rs` | ✅ 已完成 | 2026-11-18 23:34 |
| | 16.4.T | 编写 Context 路由预算约束与延迟压测测试（`< 10ms`） | `src/ecosystem/router.rs`, `tests/benchmarks.rs` | ✅ 已完成 | 2026-11-18 23:34 |
| **16.5** | 16.5.1 | 注册 `steward_mcp_catalog`、`steward_skill_route`、`steward_service_contact` MCP 与 1:1 CLI 子命令 | `src/api/mcp_tools.rs`, `src/main.rs` | ✅ 已完成 | 2026-11-18 23:36 |
| | 16.5.2 | 四层贯通收口：`config.example.toml` (`[ecosystem]`)、`src/preflight/`、`docs/manual/`、`README.md`、`skills/` 全同步 | `config.example.toml`, `src/preflight/mod.rs`, `docs/manual/`, `README.md`, `skills/` | ✅ 已完成 | 2026-11-18 23:37 |
| | 16.5.T | 编写 `tests/ecosystem_integration.rs`，执行全量 `cargo test`、零 unwrap 门禁、Release 构建与共享仓库同步 | `tests/ecosystem_integration.rs` | ✅ 已完成 | 2026-11-18 23:41 |

---

## 三、断点恢复指引（规约 #14）

当任何会话中断或交接给新 Agent 时，只需执行以下步骤恢复上下文：
1. 读取本检查点文件 `docs/src_doc/progress-phase15.md`，定位第一个状态为 `⬜ 待开始` 或 `⏳ 进行中` 的子任务 ID；
2. 对照 [roadmap-phase15.md](roadmap-phase15.md) 中该子任务的涉及路径、设计契约与验收门禁继续串行实施；
3. 开始前将该子任务状态更新为 `⏳ 进行中`，验证通过后更新为 `✅ 已完成` 并记录交付时间戳；
4. 每完成一个阶段（Phase 15 / Phase 16），同步刷新总进度表 [progress.md](progress.md)。
