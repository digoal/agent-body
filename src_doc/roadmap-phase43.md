# Agent Host Steward — Phase 43 演进路线图：双域全链路行为审计与数据驱动自进化（ISSUE #26）

> **编制**: Antigravity · **日期**: 2026-11-22  
> **基线版本**: `v0.4.0`（`34f7b1c`）+ Phase 1~42 全部完成（`252` 项测试 PASS、Benchmark `v3.5`、`DQ1~DQ6 100%`）  
> **对齐约束**: [README.md 规约 #4, #8, #18, #22, #26](../README.md) · [16-dual-domain-audit-and-self-evolution-spec.md](../design_doc/16-dual-domain-audit-and-self-evolution-spec.md) · [EVOLUTION_GUIDE.md](../EVOLUTION_GUIDE.md) · [user_feedback.md](../user_feedback.md)  
> **当前状态**: ✅ **Phase 43 已全部完成（`43.1 ~ 43.6` 全部 ✅，Benchmark `v3.6` 全绿交付）**

---

## 一、立项背景与问题锚点

| 编号 | 现网/架构问题实证 | Phase 43 演进目标 | 关联 ISSUE |
|---|---|---|---|
| **`CON-19` 双域行为审计盲区与自治动作黑盒** | ① 现网 `changes` 表 `10,159/10,727 (94.7%)` 为未折叠 `watcher:file_modified` 洪泛噪音；② `IN_AGENT` 域仅记录含 `path` 的 `edit/write`，丢失 `run_command`/`pre_tool_call`/19 个无 ROI 规则 MCP 工具审计与 `trace_id` 串联；③ `OUT_OF_AGENT` 域（`BackgroundScheduler`、`FreshnessReconciler`、`HostGarbageCollector`、`LinkRepairer --apply`、`PlacementLearner`、锁 GC、记忆审计）**0 条审计记录**，直接修改/删除宿主对象无据可查 | 建立 **`IN_AGENT` + `OUT_OF_AGENT` 双域统一行为审计引擎 (`audit_events`)**：全链路 `SecretRedactor` 脱敏、`before_after_delta` + `evidence_sha256` 存证、`Watcher` 60s 时间窗折叠防洪，覆盖 41 工具与 7 大管家自治维护动作 | **ISSUE #26** / Phase 43.2~43.3 |
| **`BENCH-12` 审计闭环与自进化基准缺失** | 缺少从历史审计流中抽取“AOPCC 租约经验降级/康复、Placement 目录聚类与摩擦热点自动上报”的数据驱动自进化通道；`DuckDbAnalyticsEngine` 未覆盖双域审计 OLAP 聚合；Benchmark `v3.5` 尚无双域审计完整性与自进化验证负载 | 打通 **Audit → AOPCC Lease / Placement / Feedback 自进化飞轮** 与 DuckDB OLAP 多维分析导出；升级 Benchmark 至 **`v3.6`**，新增 `W16`、`DQ7` 与 `PR-12` | **ISSUE #26** / Phase 43.4~43.6 |

---

## 二、Phase 43 串行原子任务拆解与断点续传检查点表

| 阶段 | 步骤 ID | 原子任务摘要 | 精确代码/文档触点 | 验收门禁 (DoD) | 状态 | 完成时间 |
|:---:|:---:|---|---|---|:---:|:---:|
| **规划立项** | **43.1** | **ISSUE #26 深度思考、规约 #4/#8/#18/#22 改进与新增规约 #26、设计规格书 `16-*.md`、反馈总账与 Phase 43 路线图立项** | `README.md`, `ISSUE.md`, `design_doc/16-*.md`, `src_doc/roadmap-phase43.md`, `src_doc/progress.md`, `src_doc/product-boundaries-and-governance.md`, `EVOLUTION_GUIDE.md`, `user_feedback.md`, `src/placement/conventions.rs`, `skills/agent-steward-developer/SKILL.md` | 规约 #4/#8/#18/#22/#26 同步落地四端；`make evolution-check` (`0 drift`)、`make lint-panic` & `cargo check` 全绿 | ✅ 已完成 | 2026-11-22 |
| **Phase 43<br/>双域审计底座** | **43.2** | **双域审计表 (`audit_events`)、三端 Storage SPI 与统一 `AuditLogger`**：① 在 SQLite/PG/DuckDB Schema 及 `migration.rs` 新增 `audit_events` 表与索引；② 扩展 `StorageProvider` Trait（`insert_audit_event`、`query_audit_events`、`audit_domain_summary`、`prune_old_audit_events`）及三后端实现；③ 新建 `src/audit/`（`AuditLogger` 强制 `SecretRedactor` 脱敏、`before_after_delta` `<=4KB` 截断、`evidence_sha256`、60s `Watcher` 事件折叠防洪与 `<80μs` 容错写入） | `src/storage/{schema,pg_schema,duckdb_schema}.sql`, `src/storage/{models,spi,mod,migration}.rs`, `src/storage/{sqlite,pg,duckdb}_provider.rs`, `src/audit/mod.rs` (新), `src/lib.rs` | 三后端（SQLite/DuckDB/PG SQL）Schema 与 SPI 契约单测全绿；敏感令牌 100% 脱敏且写入无 Panic | ✅ 已完成 | 2026-11-22 |
| | **43.3** | **`IN_AGENT` + `OUT_OF_AGENT` 全链路埋点贯通**：① **`IN_AGENT` 域**：打通 `BriefAssembler`（签发租约追踪）、`AgentHookEngine::pre_tool_call` & `post_tool_call_with_error`（补全命令/服务/锁/失败自愈审计）、`McpToolRegistry` 全部 41 工具、`main.rs` 核心 CLI、Web 写端点及 `Mesh` 协同请求；② **`OUT_OF_AGENT` 域**：打通 `BackgroundScheduler::run_inspection_cycle`、`FreshnessReconciler::reconcile`、`HostGarbageCollector::run_gc`、`LinkRepairer::repair_links`、`PlacementLearner::apply_candidate`、`MemoryLifecycleManager::audit_memories`、`CoordinationEngine::gc_expired_locks`、`SessionManager::close_session_with_dry_run` 与 `ChangeHandler`（60s 防洪折叠） | `src/api/{mcp_tools,hooks}.rs`, `src/brief/assembler.rs`, `src/main.rs`, `src/web/{server,parity_api}.rs`, `src/mesh/mod.rs`, `src/daemon/scheduler.rs`, `src/feedback/probes.rs`, `src/env/gc.rs`, `src/links/repair.rs`, `src/placement/learner.rs`, `src/memory/lifecycle.rs`, `src/coordination/mod.rs`, `src/session/mod.rs`, `src/watcher/handler.rs` | 模拟 In-Agent 工具调用 + Out-of-Agent `GC/Repair/Reconcile/Daemon` 动作后，`audit_events` 100% 留痕并含可审计 `before_after_delta` | ✅ 已完成 | 2026-11-22 |
| | **43.4** | **数据驱动自进化飞轮与 DuckDB OLAP 分析扩展**：① 新建 `src/audit/evolution.rs`（基于 24h `audit_events` 滑动窗统计 AOPCC 租约经验有效率 $\hat{p}_{\text{valid}}$ 并动态校准 `OptimisticLeaseEngine` 升降级；聚类高频错放目录/高频警告动作反哺 `PlacementLearner` 与 `FeedbackEngine::add_item_deduped`）；② 升级 `SelfEvolutionProbeEngine`（`P2`/`P4` 消费双域审计统计）；③ 扩展 `DuckDbAnalyticsEngine` & `SyncPipeline`（支持 `audit_events` 与 `effectiveness_ledger` 同步、多维 OLAP 聚合 `audit_analytics_summary` 及 Parquet/CSV 安全导出） | `src/audit/evolution.rs` (新), `src/brief/optimistic.rs`, `src/feedback/probes.rs`, `src/storage/{sync_pipeline,duckdb_analytics}.rs` | 连续碰壁自动降级乐观租约、后台康复后自动恢复置信度；DuckDB OLAP 聚合与 Parquet/CSV 导出单测全绿 | ✅ 已完成 | 2026-11-22 |
| | **43.5** | **黑白屏 1:1:1 双向同步（MCP / CLI / Web UI）**：① 在 `src/api/mcp_tools.rs` 新增/内聚 `steward_audit_query`（支持 `action: query | summary | trace | evolve | export`）及 `parity/matrix` 同步；② 在 `src/main.rs` 补齐 1:1 `agent-steward audit` 子命令族；③ 在 `src/web/parity_api.rs` & `src/web/assets/mod.rs` 新增双主题适配的「📜 双域行为审计与自进化」面板（支持按 `IN_AGENT / OUT_OF_AGENT` 过滤、`trace_id` 因果链路钻取、OLAP 热榜视图、一键触发审计自进化与 Parquet 导出，含 CLI/MCP 等价镜像器） | `src/api/mcp_tools.rs`, `src/main.rs`, `src/web/{server,parity_api}.rs`, `src/web/assets/mod.rs`, `tests/test_web_dashboard.rs` | Parity 校验 100% 对齐；`tests/test_web_dashboard.rs` 与 MCP stdio 测试全绿；Web 写端点受 `check_auth` 保护 | ✅ 已完成 | 2026-11-22 |
| | **43.6** | **Benchmark `v3.6` (`W16` + `DQ7` + `PR-12`)、四层文档同步与全量验收交付**：① 在 `src/benchmark/` 新增 `W16 (dual_domain_audit_and_lease_self_evolution)`、`DQ7 (audit_completeness_and_redaction_hygiene)` 与 `PR-12`；② 刷新 `EVOLUTION_GUIDE.md`、`manual/03,04,08,09,10`、`arch/01,02,03`、`design_doc/06,07,10`、`skills/`；③ 运行全量 `evolution-check`/`lint-panic`/`check`/`test`/`build --release`/现网 `bench run`/服务热重启与 `sync-mirror` | `src/benchmark/{mod,workloads,dq,provenance,ledger,report}.rs`, `scripts/evolution_manifest.py`, `EVOLUTION_GUIDE.md`, `manual/*`, `arch/*`, `skills/*`, `tests/benchmarks.rs` | `W16` & `DQ7` PASS（脱敏 0 泄露、100% 双域留痕、租约自进化命中）；全量测试 PASS；0 warning；0 漂移 | ✅ 已完成 | 2026-11-22 |

---

## 三、断点续传与依赖指引

- **串行依赖顺序**：`43.1 (规划与规约 ✅) → 43.2 (audit_events 存储与脱敏底座) → 43.3 (IN_AGENT + OUT_OF_AGENT 全链路埋点) → 43.4 (审计自进化飞轮与 DuckDB OLAP) → 43.5 (黑白屏 1:1 同步) → 43.6 (Benchmark v3.6 与全量验收)`。
- **中断恢复方法**：新会话只需按“零重扫开工协议”读取 `EVOLUTION_GUIDE.md` 与本路线图 `src_doc/roadmap-phase43.md`，定位首个 `⏳ 待执行` 任务即可零重扫无缝续接。
