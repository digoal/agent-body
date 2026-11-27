# Phase 54 演进路线图：Phase 50 18/18 WaitEvent 闭环、黑白屏动作级真 Parity、TUI 升级与 Benchmark v3.8 (ISSUE #38)

> **对应 ISSUE**: [ISSUE.md #38](../../ISSUE.md)  
> **所属计划**: 方案 A · `v1.0.0 GA` 工业级收口路线（Phase 52 ~ Phase 57）  
> **创建时间**: 2026-11-22 22:20 CST  
> **当前状态**: ✅ 全部完成（P54-01 ~ P54-06 全部 6 项原子任务已验收交付：295 passed / 1 ignored，18/18 WaitEvent 全埋点，Bench v3.8 37/37 ✅ PASS）  
> **关联规约**: README 规约 #2, #8, #9, #13, #14, #16, #18, #27, #28

---

## 一、阶段目标与治理边界

1. **P54-01 阶段立项**：创建 `roadmap-phase54.md`，更新 `ISSUE.md #38` 为进行中，更新 `EVOLUTION_GUIDE.md`（`spi_methods=88`、`workloads=18`、`micro=6`、`dq=9`、`pr_rules=14`）。
2. **P54-02 Phase 50 Telemetry 闭环与过期锁 SPI**：
   - 新增 SPI #88 `count_expired_coordination_locks`（SQLite/PG/DuckDB 全实现）并修复 `vitals.rs:181` 过期锁检测永远为 0 的 Bug；
   - 补齐剩余 14 个未埋点 `WaitEventId`（18/18 全覆盖，使 `ADDM-05` 真实可达）；
   - 修复 `perf_awr.rs` `estimate_percentile_from_buckets` 永不低于 `bucket_midpoints[i]` Bug（保证 `est_p50_us <= est_p95_us <= max_us`）、`run_l1_auto_tune` 增加 `AutonomyEnvelope::evaluate("perf_tune")` 校验及非 SQLite 后端准确文案，并新增 `delete_snapshot` / `delete_awr_report` / `delete_perf_artifact`；
   - 在 `scheduler.rs` 后台巡检中按 ≥60 分钟周期自动抓取 `daemon-auto` AWR 快照。
3. **P54-03 黑白屏动作级真 1:1:1 Parity 与 Client Hook 扩展**：
   - 补齐 AWR 报告库 CLI (`PerfAwr --report-id`, `PerfSnapshots`, `PerfReports`, `PerfDelete`)、MCP (`perf_snapshots`/`perf_reports`/`perf_delete`/`report_id` Schema)、Web (`POST/DELETE /api/v1/body/perf/delete` + 前端删除按钮)；
   - 统一 `body_lock_release` 三端契约（CLI/MCP/Web 同时兼容 `lock_id` 与 `(resource_type, resource_id)` + `holder_agent`/`agent_id` + `force`）；
   - 修复 `parity_api.rs` 中 `body_links_visualize` (`agent-body links graph`) 与 `body_lock_deadlock_check` (`agent-body coordination deadlock`) 错误 CLI 映射；
   - 升级 Web Benchmark 工作台至 `v3.8` (`W1~W18` / `M1~M6` / `DQ1~DQ9` / `PR-01~14`) 并渲染 `data_quality` 卡片与门禁表格；
   - 扩展 `src/hooks/client_gen.rs` 支持 `codex` (`AGENTS.md`)、`hermes` (`.hermes/agent-body-rules.md` + `.hermes/hooks/body_bridge.py`) 及 Claude `.claude/settings.json` + `.claude/hooks/body_claude_bridge.py`。
4. **P54-04 终端全景看板 TUI 升级 (`src/tui/mod.rs`)**：
   - 将终端全景看板升级为 `AGENT BODY (v{VERSION})`，展示五维稳态体征、内核 RSS/FD & Top WaitEvents/AWR、24h 双域审计统计、连续性接力胶囊与活跃/过期协作锁状态。
5. **P54-05 Benchmark `v3.8` (`W18` / `M6` / `DQ9` / `PR-14`) 与哈希完整性**：
   - 修复 `overall_speedup_factor` 落库几何均值与 Payload 不一致；扩展 `compute_canonical_sha256` 纳入 `spec_version`/`data_quality`/`competitors`；
   - 新增 `W18`（内生等待事件与 AWR/ADDM 自诊断闭环）、`M6`（`WaitEventGuard` 零分配微秒级耗时 SLA）、`DQ9`（18/18 Telemetry、分位单调性、过期锁 SPI 与 AWR 仓库完整性）、`PR-14`（AWR 诊断省轮记账）。
6. **P54-06 测试、质量门禁、现网部署与镜像同步**：完成新增单测/集成测试、全绿 `make quality-gate && make test`、`make restart` + `bench run --suite full` 现网验证、`make sync-mirror` 与台账收口。

---

## 二、原子任务断点续传追踪表

| 任务 ID | 里程碑 | 原子任务描述 | 状态 | 完成时间 | 验证摘要 |
|---|---|---|---|---|---|
| **P54-01** | M1 | 创建 `roadmap-phase54.md`，更新 `ISSUE.md` & `EVOLUTION_GUIDE.md` | ✅ 已完成 | 2026-11-22 22:20 | Tier-0、`spi_methods=88`、`v3.8` (`W:18, M:6, DQ:9, PR:14`) 就绪 |
| **P54-02** | M1 | SPI #88 过期锁检测 + `vitals` 修复 + 14/18 WaitEvent 全埋点 + `perf_awr` 缺陷修复/删除 + 周期 AWR 快照 | ✅ 已完成 | 2026-11-22 22:24 | 18/18 WaitEvent 实装、`est_p95_us<=max_us`、PG/DuckDB/SQLite 过期锁 SPI、`daemon-auto` 60m 快照 |
| **P54-03** | M2 | AWR 报告库/锁释放三端 Parity + `parity/matrix` 纠错 + Web `v3.8`/DQ 渲染 + `codex`/`hermes`/Claude `.claude/settings.json` | ✅ 已完成 | 2026-11-22 22:27 | `43/43` Parity (`links graph` / `coordination deadlock`)、Web DQ1~9、3 客户端 Hook 实测 |
| **P54-04** | M3 | 升级 `src/tui/mod.rs` 为 `AGENT BODY` 全维本体终端看板 | ✅ 已完成 | 2026-11-22 22:28 | 升级 `AGENT BODY v0.4.0`，实时展示稳态体征/AWR遥测/双域审计/胶囊/协作锁 |
| **P54-05** | M3 | 上线 Benchmark `v3.8` (`W18`/`M6`/`DQ9`/`PR-14`) 与 SHA-256 / 加速比口径一致性修复 | ✅ 已完成 | 2026-11-22 22:30 | `v3.8` 37/37 全绿 (`D1 -99.23%`、`D6 99.8/100`、`M6 6μs`、`DQ9 100%`) |
| **P54-06** | M4 | 全量单测/集成测试、`make quality-gate && make test`、`make restart` 现网 `bench`、`make sync-mirror` | ✅ 已完成 | 2026-11-22 23:09 | `295 passed; 0 failed; 1 ignored`，`0 drift`，`make restart` 与现网 37/37 ✅ PASS |
