# Phase 69 演进路线图：Top-5 高水位模块预防性拆分与 SQLite/DuckDB 共享 SPI 契约测试 (ISSUE #51)

> **对应 ISSUE**: [ISSUE.md #51](../../ISSUE.md#L242)  
> **创建时间**: 2026-11-27 13:37 CST  
> **当前状态**: ✅ 全部完成（5 大高水位文件全部拆至 `<700L` / `tests/storage_spi_contract.rs` 双引擎全绿）  
> **关联规约**: README 规约 #7（多存储 SPI 对齐）、#13（测试驱动与质量门禁）、#28（模块化复杂度控制）

---

## 一、阶段目标

1. **Top-5 高水位生产模块预防性拆分（全部压至 `<700L`）**：
   - `src/hooks/client_gen.rs` (`950L`) -> `client_gen.rs` (`278L`) + `src/hooks/client_templates.rs` (`677L`)
   - `src/fact/property_graph.rs` (`944L`) -> `property_graph.rs` (`652L`) + `src/fact/property_graph_helpers.rs` (`305L`)
   - `src/scanner/multimodal.rs` (`941L`) -> `multimodal.rs` (`589L`) + `src/scanner/multimodal_docs.rs` (`366L`)
   - `src/benchmark/competitors.rs` (`921L`) -> `competitors.rs` (`258L`) + `src/benchmark/competitor_tracks.rs` (`674L`)
   - `src/api/mcp_schemas.rs` (`904L`) -> `mcp_schemas.rs` (`455L`) + `src/api/mcp_schemas_ops.rs` (`459L`)
2. **`evolution_manifest.py` 提取适配**：同步扫描 `mcp_schemas_ops.rs` 与 `competitor_tracks.rs`，确保 `43 MCP 工具` 与 `COMP:4` `0 drift`。
3. **SQLite / DuckDB 共享 SPI 契约测试套件 (`tests/storage_spi_contract.rs`)**：覆盖 12 大核心存储域，使用同一份 `run_shared_spi_contract(&dyn StorageProvider)` 断言双后端语义等价性。

---

## 二、原子任务断点续传追踪表

| 任务 ID | 描述 | 状态 | 完成摘要 |
|---|---|---|---|
| **P69-01** | Top-5 高水位生产模块拆分为 10 个 `<700L` 子模块并保持全部公开接口零破坏 | ✅ 已完成 | 5 组模块生产代码行数全部 `<700L`，全仓峰值降至 `885L` |
| **P69-02** | `scripts/evolution_manifest.py` 适配 `mcp_schemas_ops.rs` / `competitor_tracks.rs` 提取 | ✅ 已完成 | `43 MCP工具`、`COMP:4`、`0 drift` |
| **P69-03** | 新增 `tests/storage_spi_contract.rs` 覆盖 SQLite 与 DuckDB 12 域共享 SPI 契约 | ✅ 已完成 | `test_sqlite_storage_spi_contract` 与 `test_duckdb_storage_spi_contract` 双绿 |
| **P69-04** | 文档同步、全量 `quality-gate` / `clippy` / `cargo test` 及 Release 重编重启 | ✅ 已完成 | 全门禁与测试零失败，LaunchAgent 健康校验通过 |
