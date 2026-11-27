# Phase 17 断点续传检查点追踪表 (Progress Phase 17)

> **创建时间**: 2026-11-18T23:54:00+08:00  
> **当前状态**: ✅ 100% 全部完成  
> **详细设计**: [roadmap-phase17.md](roadmap-phase17.md) · **全景路线**: [roadmap-phase15-18.md](roadmap-phase15-18.md)

| 阶段 | 子任务 ID | 任务内容与检查点 | 涉及路径 | 状态 | 交付时间 |
|:---:|:---:|---|---|:---:|:---:|
| **17.1** | 17.1.1 | 扩展三后端 Schema（`lock_wait_queue`）与 `LockWaitEntry` 模型 | `src/storage/{schema.sql,pg_schema.sql,duckdb_schema.sql,models.rs}` | ✅ 已完成 | 2026-11-18 23:55 |
| | 17.1.2 | 扩展 `StorageProvider` SPI（5 方法）与 SQLite/PG/DuckDB 实现 | `src/storage/{spi.rs,sqlite_provider.rs,pg_provider.rs,duckdb_provider.rs,mod.rs}` | ✅ 已完成 | 2026-11-18 23:55 |
| **17.2** | 17.2.1 | 实现 WFG 有向图环路检测引擎 `DeadlockDetector` | `src/coordination/deadlock.rs`, `src/coordination/mod.rs` | ✅ 已完成 | 2026-11-18 23:56 |
| **17.3** | 17.3.1 | 新增 `BodyEvent::LockWaiterNotified` 与释放/GC 自动出队唤醒 | `src/hooks/mod.rs`, `src/coordination/mod.rs` | ✅ 已完成 | 2026-11-18 23:56 |
| | 17.3.2 | 实现 `wait_and_acquire` 带死锁拦截的排队获取原语 | `src/coordination/mod.rs` | ✅ 已完成 | 2026-11-18 23:56 |
| **17.4** | 17.4.1 | 新增 `body_lock_wait` / `body_lock_deadlock_check` MCP & 1:1 CLI + Web API | `src/api/mcp_tools.rs`, `src/main.rs`, `src/web/` | ✅ 已完成 | 2026-11-19 00:00 |
| | 17.4.2 | 四层文档/SKILL 同步 + `tests/deadlock_wait_integration.rs` + 全量回归 & Release 构建 | `tests/`, `README.md`, `docs/manual/`, `docs/arch/`, `skills/` | ✅ 已完成 | 2026-11-19 00:00 |
