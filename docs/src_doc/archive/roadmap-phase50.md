# Phase 50 演进路线图：内生等待事件 (Wait Events)、`/proc/self` 内核探针、AWR 快照基线差分与 ADDM 自优化 (ISSUE #32)

> **对应 ISSUE**: [ISSUE.md #32](../../ISSUE.md#L155)  
> **设计规格书**: [18-wait-events-and-awr-perf-diagnostics-spec.md](../design_doc/18-wait-events-and-awr-perf-diagnostics-spec.md)  
> **创建时间**: 2026-11-22 18:45 CST  
> **当前状态**: ✅ 全部完成（M1~M4 全部 10 项原子任务已验收交付）  
> **关联规约**: README 规约 #2, #8, #9, #13, #16, #18, #26, #27, #28

---

## 一、阶段目标与架构边界

1. **借鉴 PostgreSQL / Oracle 内核性能工程**：
   - **等待事件体系 (`WaitEventRegistry`)**：正交划分 `Lock` / `IO` / `Net` / `CPU` / `IPC` / `Idle` 6 大类 18 种子事件，配合 8 档对数延迟直方图与无锁 `AtomicU64` RAII `WaitEventGuard`（单次埋点 `< 100 ns`，零堆分配）。
   - **操作级统计视图 (`OperationStatsRegistry`)**：对标 `pg_stat_statements`，统计高频工具与后台子阶段 `calls`、`total_us`、`max_us`、`P50/P95/P99`。
   - **零特权 `/proc/self` 内核探针 (`KernelSelfTelemetry`)**：对标轻量 eBPF，直读 `/proc/self/{status,io,fd}` 获取 `rss_kb`、`hwm_kb`、`open_fds`、`voluntary_ctxt_switches`（I/O 与锁阻塞）、`nonvoluntary_ctxt_switches`（CPU 争用）、物理读写字节与 SQLite `db_size / wal_size`。
   - **AWR 快照、黄金基准 (`Golden Baseline`) 与 ADDM 自调优 (`BodyPerfEngine`)**：支持抓快照、设黄金基线、生成任意两快照或 `Baseline vs Current` 的差分报告，并由 ADDM 规则引擎给出根因处方与 `L1_AUTONOMIC` 一键自调优（`perf_tune`）。
2. **流式分页内存治理（回归规约 #9 `< 90 MB`）**：
   - 将 `FreshnessReconciler::reconcile` 与 `HomeostasisVitalsEngine` 中的大批量资产扫描改为流式分块处理 + `PRAGMA shrink_memory` + `malloc_trim(0)`，将守护进程真实匿名 RSS 压降至 `< 70 MB`。
3. **严守 43 工具 & 27 表零膨胀（规约 #16）**：
   - 内聚挂载于第 43 原生工具 `body_state` 的 `perf_snapshot` / `perf_awr` / `perf_tune` 子动作，持久化复用 `system_meta`，并在 CLI、Web REST API 与前端工作台实现 1:1:1 同步。

---

## 二、原子任务断点续传追踪表

| 任务 ID | 里程碑 | 原子任务描述 | 状态 | 完成时间 | 验证摘要 |
|---|---|---|---|---|---|
| **P50-01** | M1 | 编写设计规格书 `docs/design_doc/18-wait-events-and-awr-perf-diagnostics-spec.md` | ✅ 已完成 | 2026-11-22 18:48 | 定义 6 类 18 事件、Time Model、`/proc/self` 与 ADDM 矩阵 |
| **P50-02** | M1 | 实现无锁 `WaitEventRegistry`、对数分位直方图、操作统计与 `/proc/self` 内核探针 (`src/body/perf_awr.rs`) | ✅ 已完成 | 2026-11-22 18:48 | `AtomicU64` 零锁 8 档分位桶 + `/proc/self/{status,io,fd}` |
| **P50-03** | M1 | 在 `SqliteStorageProvider`、`LinksEngine`、`FreshnessReconciler` 等植入 RAII `WaitEventGuard` | ✅ 已完成 | 2026-11-22 18:52 | `lock_conn` 自动统计 `Lock::SqliteMutex`，读写与正则切面接入 |
| **P50-04** | M2 | 实现 AWR 快照抓取、黄金基线 (`Golden Baseline`) 设定与差分报告生成器 | ✅ 已完成 | 2026-11-22 18:52 | 滚动保留 32 份快照 + `awr:baseline` 差分对比 |
| **P50-05** | M2 | 实现 ADDM 瓶颈诊断规则引擎与 `L1_AUTONOMIC` 一键自优化 (`perf_tune`) | ✅ 已完成 | 2026-11-22 18:52 | 5 大 ADDM 规则 + `PRAGMA optimize/wal_checkpoint/shrink_memory` + `malloc_trim` |
| **P50-06** | M2 | 优化 `FreshnessReconciler` 工作集及时释放与 SQLite 内存回收 | ✅ 已完成 | 2026-11-22 18:54 | 巡检末尾释放大集合并触发 `shrink_memory` + `malloc_trim(0)` |
| **P50-07** | M3 | 扩展 MCP `body_state` (`perf_snapshot` / `perf_awr` / `perf_tune`) 与 CLI `agent-body body` 子命令 | ✅ 已完成 | 2026-11-22 18:53 | 43 工具零膨胀内聚扩展，CLI 新增 3 子命令 |
| **P50-08** | M3 | 扩展 Web REST API (`/api/v1/body/perf/*`) 与前端「🧬 本体与生命体征」内生 AWR 诊断工作台 | ✅ 已完成 | 2026-11-22 18:55 | 黑白屏 1:1:1 同步 + 黑屏命令镜像器联动 |
| **P50-09** | M4 | 编写 `perf_awr` 单元与集成测试（探针开销 `<100ns`、归因、基线差分、自调优、Web Parity） | ✅ 已完成 | 2026-11-22 18:56 | 单测 + CLI 解析测试 + Web E2E 鉴权与功能测试 |
| **P50-10** | M4 | 更新 `README.md` 规约 #9/#27、手册、`ISSUE.md`，重编重启 `agent-body.service` 并执行真机 AWR 基线验收 | ✅ 已完成 | 2026-11-22 19:05 | `270/270` 测试全绿，真机设定黄金基线并通过 `perf-awr`/`perf-tune` 验收 |
