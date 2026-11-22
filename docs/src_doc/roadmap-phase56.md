# Phase 56 演进路线图 — God File 拆分、静默吞错压降、跨模块集成测试与五重门禁升级 (`ISSUE #40`)

> **启动时间**：2026-11-23 | **前置基线**：Phase 55 (`ISSUE #39`) 已完成 (`304 passed; 0 failed; 1 ignored` / `bench 37/37 PASS` / `agent-body.service` 在线)
> **状态**：`✅ 已完成 (Phase 56 · ISSUE #40 验收全绿)`

---

## 一、核心目标与验收标准

1. **P0 — `>1500L` 生产 God File 清零 (`0` 个)**：
   - `src/web/assets/mod.rs` (5017L) → 外置 `index.html` / `app.css` / `app_part1.js` / `app_part2.js` via `include_str!` + `OnceLock`，`mod.rs` `< 100L`
   - `src/body/perf_awr.rs` (1555L prod / 1728L) → 拆为 `src/body/perf_awr/{mod.rs, wait_events.rs, awr_engine.rs}`
   - `src/web/parity_api.rs` (1717L) → 拆为 `src/web/parity_api/{mod.rs, core_endpoints.rs, ops_endpoints.rs, gov_body_endpoints.rs}`
   - `src/main.rs` (1869L prod / 1918L) → `main.rs` (`<250L`) + `src/cli_handlers/{mod.rs, core.rs, ops.rs, governance.rs}`
   - `src/storage/sqlite_provider.rs` (2520L) → 拆为 `src/storage/sqlite_provider/{mod.rs, core_ops.rs, graph_locks.rs, eco_bench.rs, mesh_audit_body.rs}`
   - `src/storage/pg_provider.rs` (2353L prod / 2634L) → 拆为 `src/storage/pg_provider/{mod.rs, core_ops.rs, graph_locks.rs, eco_bench.rs, mesh_audit_body.rs}`
   - `src/storage/duckdb_provider.rs` (2674L prod) → 拆为 `src/storage/duckdb_provider/{mod.rs, core_ops.rs, graph_locks.rs, eco_bench.rs, mesh_audit_body.rs}`
   - **架构指标漂移校验**：`27 modules / 43 MCP tools / 30 CLI commands / 88 SPI methods / 27 SQLite tables` 全量保持 0 漂移。
2. **P0 — 静默 `let _ =` 预算收紧 (`115 -> <= 50`)**：
   - 将 `scripts/code_hygiene.py` 中的 `MAX_SILENT_LET_UNDERSCORE` 从 `130` 压降至 `50`。
   - 清理 `spi.rs`、三存储 Provider、`coordination`、`daemon`、`hooks`、`links`、`demo` 等模块中的静默吞错与未命名参数。
3. **P1 — 补全 5 套跨模块集成测试 (`tests/`)**：
   - `tests/mesh_integration.rs`
   - `tests/audit_integration.rs`
   - `tests/body_awr_integration.rs`
   - `tests/hooks_integration.rs`
   - `tests/cli_e2e.rs`
4. **P1 — 升级五重门禁 (`code_hygiene.py` & `evolution_manifest.py`)**：
   - `code_hygiene.py` 新增：(a) `>1500L` 生产 Rust 文件清零拦截；(b) 非基准测试生产代码 `/root/new` 与 `user:digoal` 硬编码拦截；(c) 100% `WaitEventId` 变体埋点检查；(d) `MAX_SILENT_LET_UNDERSCORE = 50`。
   - `evolution_manifest.py` 新增核心活跃文档 Markdown 内部相对链接断链扫描。
5. **P0 — 全量回归验收**：
   - `make quality-gate` 全绿、`make test` 全绿（新增 5 套集成测试全部通过）、`make restart` 守护进程在线、`bench run --suite full` 全通过、`make sync-mirror` 同步完成。

---

## 二、断点续传任务台账

| 任务编号 | 任务内容 | 目标产物 | 状态 |
| :--- | :--- | :--- | :--- |
| `P56-01` | 初始化 `roadmap-phase56.md` 与 `EVOLUTION_GUIDE.md` / `ISSUE.md` 状态 | `roadmap-phase56.md`, `EVOLUTION_GUIDE.md`, `ISSUE.md` | `🟢 Done` |
| `P56-02` | God File 拆分：`web/assets`, `body/perf_awr`, `web/parity_api`, `main`, 三存储 Provider | `>1500L` prod 文件 `7 -> 0` | `🟢 Done` |
| `P56-03` | 静默 `let _ =` 治理：清零 trait 占位、autonomic daemon `RunningFlagGuard` 与关键 IO/DB 静默吞错 (`115 -> 43`) | `src/**/*.rs` | `🟢 Done` |
| `P56-04` | 新增 5 套跨模块集成测试 (`mesh`/`audit`/`body_awr`/`hooks`/`cli_e2e`) | `tests/*_integration.rs`, `tests/cli_e2e.rs` | `🟢 Done` |
| `P56-05` | 门禁升级：`code_hygiene.py` (`<=50`, 核心0吞错, `>1500L=0`, `/root/new`拦截, `18/18 WaitEventId`) + `evolution_manifest.py` + `verify_presentations.py` | `scripts/code_hygiene.py`, `scripts/evolution_manifest.py`, `scripts/verify_presentations.py` | `🟢 Done` |
| `P56-06` | 文档闭环 (`roadmap-phase56.md`, `ISSUE.md`, `user_feedback.md`, `progress.md`) 与全链路验证 | 全门禁 + 全量单元/集成测试 | `🟢 Done` |
