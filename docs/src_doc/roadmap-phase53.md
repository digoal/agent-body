# Phase 53 演进路线图：存储三后端语义对齐、AWR 孤儿清理、全量迁移与 OLAP 管道修复 (ISSUE #37)

> **对应 ISSUE**: [ISSUE.md #37](../../ISSUE.md)  
> **所属计划**: 方案 A · `v1.0.0 GA` 工业级收口路线（Phase 52 ~ Phase 57）  
> **创建时间**: 2026-11-22 21:43 CST  
> **当前状态**: ✅ 全部完成（P53-01 ~ P53-08 全部 8 项原子任务已验收交付，含 DuckDB 8/8 与真实 PG 5/5 实证）  
> **关联规约**: README 规约 #4, #6, #9, #10, #12, #13, #14, #27, #28

---

## 一、阶段目标与治理边界

1. **P53-01 阶段立项**：创建 `roadmap-phase53.md`，同步修正 `ISSUE.md #36/#37` 状态并将 `EVOLUTION_GUIDE.md` Tier-0 入口切换至 `roadmap-phase53.md`（SPI `82 → 87`）。
2. **P53-02 存储 SPI 与门面扩展 (`82 → 87`)**：新增 `upsert_scan_policy`、`list_all_memories`、`delete_benchmark_run`、`list_all_benchmark_runs`、`list_created_at_map` 5 个语义契约并在 `Storage` 门面透传。
3. **P53-03 SQLite Provider 语义对齐与调度日志硬化**：补齐 `insert_memory` 冲突保留向量与元数据、`search_memories_fts` `active` 过滤、`record_change` 保留历史时间、锁/队列时间用 `julianday` 准确比较、`enqueue_lock_wait` 幂等覆盖、`query_audit_events` 上限放宽至 10w、实现 5 个新 SPI 方法，并让 `scheduler.rs` 记录向量补全报错而非静默 `.unwrap_or(0)`。
4. **P53-04 PostgreSQL Schema 与 Provider 对齐**：移除 `pg_schema.sql` 强依赖 `CREATE EXTENSION vector` 阻断、补 `memories.sensitive` 列、`upsert_asset` 支持陈旧向量置空、`insert_memory` 全字段更新、协作锁/等待队列改用 `timestamptz` 比较（根治同日过期锁不失效 Bug）、`list_benchmark_runs` 过滤 `awr:%`、实现 5 个新 SPI 方法。
5. **P53-05 DuckDB Schema/Provider 对齐与 AWR 孤儿清理**：为 DuckDB `memories` 尾列补 `sensitive`、`init()` 预设规则/策略改为 `ON CONFLICT DO NOTHING` 防覆盖用户配置、为 `replace_active_processes`/`add_link`/`insert_coordination_lock` 补齐事务原子性与去重、用 `try_cast AS TIMESTAMPTZ` 统一 UTC 比较、`list_benchmark_runs` 过滤 `awr:%`、`delete_bench_sandbox_records` 纳入 `audit_events`；同时在 `perf_awr.rs` 淘汰快照/报告时调用 `delete_benchmark_run` 清除孤儿行。
6. **P53-06 全量热迁移、`SyncPipeline`、`DuckDbAnalyticsEngine` 与 `Preflight` 实现**：打通 `StorageMigrator` 全部 20 类实体与向量/策略无损幂等迁移；修复 `SyncPipeline` 抹平历史时间、漏 `links`/`active_processes`/`body_checkpoints` 及未更新 `sync_watermarks` 问题；激活 `DuckDbAnalyticsEngine` 时间窗、`memory_decay_curve` 真实账龄分桶与 `QUANTILE_CONT` P50/P95 分位计算；落地 `PreflightChecker::check_storage_backend`。
7. **P53-07 全覆盖验证（含真实 PG 实例）**：新增 SQLite/DuckDB/AWR/Preflight 单测与全量集成测试，并在本地 PostgreSQL 临时库 `agent_body_phase53_verify` 上实测无 `vector` 扩展建库、跨库迁移与语义对齐（测后删库清理）。
8. **P53-08 交付收口**：更新路线图/台账/进度表，全量通过 `make quality-gate && make test`、`make restart` 现网冒烟与 `make sync-mirror`。

---

## 二、原子任务断点续传追踪表

| 任务 ID | 里程碑 | 原子任务描述 | 状态 | 完成时间 | 验证摘要 |
|---|---|---|---|---|---|
| **P53-01** | M1 | 创建 `roadmap-phase53.md`，更新 `ISSUE.md`、`EVOLUTION_GUIDE.md` (`spi_methods=87`) | ✅ 已完成 | 2026-11-22 21:44 | Tier-0 与 `#36`/`#37` 台账同步就绪 |
| **P53-02** | M1 | `src/storage/spi.rs` & `src/storage/mod.rs` 扩展 5 个 SPI 方法 (`82 → 87`) | ✅ 已完成 | 2026-11-22 21:44 | `upsert_scan_policy`、`list_all_memories`、`list_all_benchmark_runs`、`delete_benchmark_run`、`list_created_at_map` |
| **P53-03** | M2 | `sqlite_provider.rs` 冲突/FTS/时间/上限/5 SPI 修复 + `scheduler.rs` 报错可见化 | ✅ 已完成 | 2026-11-22 21:45 | 保留非空向量、`active` FTS 过滤、`julianday` 锁比较、`backfill` 打印告警 |
| **P53-04** | M2 | `pg_schema.sql` & `pg_provider.rs` 去 `pgvector` 强依赖、`sensitive` 列、锁 `timestamptz`、`awr:%` 与 5 SPI | ✅ 已完成 | 2026-11-22 22:02 | 无 `vector` 扩展可建库，`chrono::DateTime<Utc>` 绑定修复同日过期锁 |
| **P53-05** | M2 | `duckdb_schema.sql` & `duckdb_provider.rs` 事务/去重/时间/5 SPI + `perf_awr.rs` 孤儿清理 | ✅ 已完成 | 2026-11-22 22:00 | `ON CONFLICT DO NOTHING`、事务原子化、`links` 去重、双约束 `ON CONFLICT` 与 AWR 淘汰删行 |
| **P53-06** | M3 | `migration.rs` 20 实体全迁移、`sync_pipeline.rs` 10 表+时间保留、`duckdb_analytics.rs` 真 OLAP、`preflight` PG 探针 & `main.rs` 输出 | ✅ 已完成 | 2026-11-22 21:53 | 20 实体幂等迁移、10 表水位线+真实时间、P50/P95 分位、脱敏 PG `Preflight` |
| **P53-07** | M4 | 新增单元测试/集成测试 + 本地真实 PostgreSQL (`agent_body_phase53_verify`) 实证（测完清理） | ✅ 已完成 | 2026-11-22 22:02 | `duckdb_integration` 8/8 + 真实 PG 5/5 全绿，临时库已清理 |
| **P53-08** | M4 | 完结台账文档、`make quality-gate && make test`、`make restart` + `preflight` + `make sync-mirror` | ✅ 已完成 | 2026-11-22 22:06 | 全门禁 0 漂移、全测通过、Release 服务运行与镜像零差同步 |
