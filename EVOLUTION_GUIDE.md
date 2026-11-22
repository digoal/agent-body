# Agent Body — 零重扫项目演进指南 (`EVOLUTION_GUIDE.md`)

<!-- MANIFEST: modules=27 mcp_tools=43 spi_methods=82 tables=27 workloads=17 micro=5 dq=8 pr_rules=13 -->

> **核心定位（解决 ISSUE #17）**：让任何新接手的 AI Agent 或开发者在 **不重复扫描 3.8 万行源码与 48 份历史 `docs/src_doc/` 文档** 的前提下，**2 轮以内**完成需求定位、触点锁定与规范演进。

---

## 一、零重扫 5 分钟开工协议（必读 SOP）

1. **只读 Tier-0 文档（仅 2 份）**：
   - 本文件 `EVOLUTION_GUIDE.md`（架构地图 + 变更触点矩阵）；
   - 当前活跃路线图 [docs/src_doc/roadmap-phase46.md](docs/src_doc/roadmap-phase46.md)（断点续传状态表；已完成的前序阶段见 [docs/src_doc/roadmap-phase44-45.md](docs/src_doc/roadmap-phase44-45.md)、[docs/src_doc/roadmap-phase43.md](docs/src_doc/roadmap-phase43.md)、[docs/src_doc/roadmap-phase42.md](docs/src_doc/roadmap-phase42.md)、[docs/src_doc/roadmap-phase40-41.md](docs/src_doc/roadmap-phase40-41.md)、[docs/src_doc/roadmap-phase38-39.md](docs/src_doc/roadmap-phase38-39.md) 与 [docs/src_doc/roadmap-phase37.md](docs/src_doc/roadmap-phase37.md)）。
2. **运行 O(1) 实时架构快照（`<100ms`，禁止手动数模块/工具数）**：
   ```bash
   make evolution-check
   # 需要完整工具/表/负载名列表时：
   python3 scripts/evolution_manifest.py --json
   ```
3. **查符号用 CodeGraph，禁止盲目全仓 `grep`**：
   ```bash
   codegraph explore "<SymbolOrFile>"
   # 源码改动后刷新符号地图：make codegraph
   ```
4. **文档三级分层与禁扫令 (Document Tiering)**：
   - **`Tier-0`（开工必读）**：`EVOLUTION_GUIDE.md` + 当前最新活跃 `docs/src_doc/roadmap-phase*.md`；
   - **`Tier-1`（按改点跳读）**：`docs/design_doc/05~17`、`docs/manual/01~10`、`user_feedback.md`；
   - **`Tier-2`（历史冻结归档，严禁常规迭代全量扫描）**：`docs/src_doc/roadmap-phase2..33.md`、`docs/src_doc/progress-phase*.md`、`docs/design_doc/review*.md`。

---

## 二、8 大高频演进场景 O(1) 变更触点矩阵 (Change-Impact Matrix)

> 遇到以下任何演进需求，直接按表锁定必改文件，无需全仓搜索“还有哪里要改”：

| 演进场景 | 必改代码触点（按串行修改顺序） | 必改测试 / 门禁 / 文档触点 |
|---|---|---|
| **1. 新增 / 修改原生 MCP 工具** | 1. `src/api/mcp_tools.rs` (`list_tools` + `call_tool_inner`)<br/>2. `src/main.rs` (1:1 CLI 子命令)<br/>3. `src/web/server.rs` & `src/web/parity_api.rs` (REST API + `parity/matrix`)<br/>4. `src/web/assets/mod.rs` (前端工作台交互) | `src/api/mcp.rs` 单测 + `tests/test_web_dashboard.rs` + `docs/manual/03,04` + `make evolution-check` |
| **2. 新增 / 修改数据库表或列** | 1. `src/storage/{schema,pg_schema,duckdb_schema}.sql`<br/>2. `src/storage/models.rs`<br/>3. `src/storage/spi.rs` (`StorageProvider` Trait)<br/>4. `src/storage/{sqlite,pg,duckdb}_provider.rs`<br/>5. `src/storage/migration.rs` (按需迁移) | `src/storage/mod.rs` 单测 + `tests/duckdb_integration.rs` + `make evolution-check` |
| **3. 新增 / 修改 Benchmark 负载或 DQ** | 1. `src/benchmark/{workloads,micro,dq,competitors}.rs`<br/>2. `src/benchmark/mod.rs` (`run_suite` / 汇总)<br/>3. `src/benchmark/report.rs` (`BENCHMARK_SPEC_VERSION` / 渲染) | `tests/benchmarks.rs` + `docs/design_doc/10-benchmark-spec.md` + `docs/manual/10-benchmark-and-roi.md` |
| **4. 新增 / 修改安全脱敏与 Pre-Hook 门禁** | 1. `src/scanner/metadata.rs` (`SecretRedactor` / `PromptInjectionScanner`)<br/>2. `src/api/hooks.rs` (`pre_tool_call` 高危拦截)<br/>3. `src/hooks/dispatcher.rs` / `src/web/server.rs` | `src/benchmark/workloads.rs` (`W11`) + `src/benchmark/dq.rs` (`DQ4`) + `docs/design_doc/11-security-spec.md` |
| **5. 新增 / 修改资产放置规约** | 1. `src/storage/schema.sql` (`placement_rules` 预置)<br/>2. `src/placement/{engine_v2,conventions,learner}.rs` | `src/placement/` 单测 + `README.md` 放置规范 |
| **6. 新增 / 修改宿主机物理事实探针** | 1. `src/fact/` (`capability`/`process_net`/`automation`/`assets_catalog`)<br/>2. `src/platform/` (Linux/macOS/Windows 抽象)<br/>3. `src/brief/assembler.rs` (必须走索引优先 `Index-First`) | `src/fact/` 单测 + `W1`/`W2`/`W8` Benchmark 回归 |
| **7. 新增 / 修改效能存证规则 `PR-XX`** | 1. `src/benchmark/provenance.rs` (`ProvenanceRule` 枚举与推导公式)<br/>2. `src/benchmark/ledger.rs` (`record_channel_call`)<br/>3. 对应 CLI/MCP/Web 入口埋点 | `src/benchmark/ledger.rs` 单测 + `make evolution-check` |
| **8. 新增 / 修改配置项或外部依赖** | 1. `src/config/mod.rs` (`Config` + `validate()`)<br/>2. `config.example.toml`<br/>3. `src/preflight/mod.rs` (启动预检探针) | `INSTALL.md` + `agent-body preflight` 冒烟 |

---

## 三、27 个顶层模块坐标速查表 (`src/lib.rs`)

| 模块路径 | 核心公开 Struct / Trait | 职责边界（严守规约 #2） |
|---|---|---|
| `src/api/` | `McpToolRegistry`, `AgentHookEngine`, `McpServer` | 43 个原生 MCP stdio 工具注册与 Hook 门禁（不写业务逻辑） |
| `src/audit/` | `AuditLogger`, `AuditEvolutionAnalyzer`, `AuditOlapReport` | `IN_AGENT`/`OUT_OF_AGENT` 双域行为审计、脱敏防洪、AOPCC 滑窗自进化与 DuckDB OLAP |
| `src/benchmark/` | `BenchmarkEngine`, `EffectivenessLedger`, `ProvenanceRule` | 双轨/三轨实证评测、Micro/DQ/竞品对标与 `PR-01~13` 存证 |
| `src/body/` | `BodyManifestEngine`, `ContinuityManager`, `HomeostasisVitalsEngine`, `AutonomyEnvelope` | Agent 本体身份证、`<300 tok` 跨大脑接力胶囊、五维稳态自愈与 `L0~L3` 自治权限包络 |
| `src/brief/` | `BriefAssembler`, `BriefRequest`, `OptimisticLeaseEngine` | 索引优先自适应情境组装（`<10ms`，40% 记忆预算，AOPCC 乐观租约） |
| `src/changes/` | `ChangesEngine` | 宿主变更审计流与开工回溯摘要（含脱敏） |
| `src/config/` | `Config`, `WebConfig`, `StorageConfig` | TOML 配置加载、校验与 `~/.agent-body/` 权限收敛 |
| `src/coordination/` | `CoordinationEngine`, `DeadlockDetector`, `LockWaitQueue` | 多 Agent 协作锁、层级互斥传播与 WFG 死锁环路检测 |
| `src/daemon/` | `StewardDaemon`, `BackgroundScheduler` | 常驻守护主循环、后台巡检与生命周期托管 |
| `src/demo/` | `DemoEngine`, `RoiReport` | 6 大路演沙盒剧目（`demo:*` 隔离，真引擎实跑） |
| `src/ecosystem/` | `McpConfigScanner`, `SkillCatalogScanner`, `ContextBudgetRouter` | MCP/SKILL 目录、外部服务通讯录与 Token 防爆路由 |
| `src/env/` | `EnvCollector`, `PortChecker`, `HostGarbageCollector` | `/proc` 任务采集（脱敏+去噪）、端口防撞与白名单 `dry-run` GC |
| `src/fact/` | `FactFacade`, `CapabilityEngine`, `ProcessNetTopology`, `AutomationScanner`, `PropertyGraphOrganizer` | 宿主机客观事实图谱（工具/端口/Systemd/Cron/资料编目/L1~L3 属性图） |
| `src/feedback/` | `FeedbackEngine`, `FreshnessReconciler`, `SelfEvolutionProbeEngine` | `user_feedback.md` 双向读写、新鲜度对账与 `P1~P5` 自进化探针 |
| `src/hooks/` | `EventBus`, `HookDispatcher`, `ClientHookGenerator`, `HookPipelineEngine`, `OptimisticFailureDiagnoser` | 事件总线、POSIX 转义防注入、客户端生成与 `<2ms` 碰壁自愈 |
| `src/links/` | `LinksEngine`, `ImpactAnalyzer`, `LinkAuditor`, `LinkRepairer`, `CodeGraphBridge` | 宏观依赖拓扑、加权 3 跳爆炸半径、假阳性过滤与 CodeGraph 桥接 |
| `src/memory/` | `MemoryManager`, `MemoryProvider`, `MemoryFactory`, `MemoryIntakeFilter` | FTS5+Ollama RRF 混合检索、SPI 与 Mem0/Letta/PowerContext 适配 |
| `src/mesh/` | `MeshClient`, `MeshNodeFacts`, `MeshEnvelope` | 轻量多机联邦 HTTP Peer Mesh、HMAC 防重放信封、跨机对话与沙盒投递 |
| `src/placement/` | `PlacementEngineV2`, `ConventionChecker`, `PlacementLearner` | 8 大目录放置建议、工程安全规约与搬移纠偏自演化 |
| `src/platform/` | `PlatformProbe`, `LinuxProbe`, `MacosProbe`, `WindowsProbe` | 跨平台系统探针抽象与纯函数解析 |
| `src/preflight/` | `PreflightChecker`, `PreflightReport` | 依赖预检、磁盘水位预警与 `0600/0700` 安全基线检查 |
| `src/scanner/` | `Scanner`, `SecretRedactor`, `PromptInjectionScanner`, `OllamaAnnotator` | 白黑灰分级扫描、全链路脱敏、抗投毒清洗与三层元数据标注 |
| `src/session/` | `SessionManager`, `SessionLifecycleManager` | 会话绑定、完工结清与临时垃圾回收 |
| `src/storage/` | `Storage`, `StorageProvider`, `Sqlite/Pg/DuckDbStorageProvider`, `DuckDbAnalyticsEngine` | 82 方法 SPI 门面、三后端实现、跨库热迁移与 Parquet/CSV 导出 |
| `src/tui/` | `TuiDashboard` | 终端单屏实时看板 |
| `src/watcher/` | `FileWatcher`, `ChangeHandler` | `inotify` 300ms 防抖监听与增量变更同步 |
| `src/web/` | `WebServer`, `TopologyGraph`, `parity_api`, `assets/mod.rs` | 单二进制 HTTP/SSE、昼夜双主题 8 大工作台、`check_auth` 与 Parity 校验 |

---

## 四、演进提交前 5 秒自检门禁

```bash
export PATH="/root/.cargo/bin:$PATH"
make evolution-check && make lint-panic && make check && make test && make sync-mirror
```
