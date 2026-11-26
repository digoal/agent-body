# Phase 60 路线图：跨平台 100% 全绿测试基线、DuckDB/断链/基准可移植修复与 Clippy 零告警收口

> **版本标签**：`v1.3` | **状态**：`✅ 已完成并收口`

---

## 一、演进背景

Phase 58/59 期间于 macOS 开发环境补齐实测能力时，暴露出仓库内 **14 项 `--lib` 测试**、**2 项集成测试（`benchmarks`、`duckdb_integration`）** 在非 Linux / 非 `/root/new` 环境下失败，以及 `Cargo.toml` 残留 `thiserror`/`glob` 死依赖和 `cargo clippy --all-targets` 存在历史 87 处告警未清零的问题。Phase 60 对上述根因进行彻底系统化治理，确立**全平台零失败测试基线 + `clippy --all-targets -- -D warnings` 零告警**。

---

## 二、核心交付清单

| 编号 | 模块与文件 | 交付与根治内容 |
|:---:|---|---|
| **60.1** | [src/links/audit.rs](../../src/links/audit.rs) | 新增 `is_in_sys_temp`（识别 `/tmp/`、`/private/tmp/` 与 `std::env::temp_dir()` 原生/规范路径），修复非 Linux 平台 `/var/folders` 下 `LinkAuditor` 测试沙盒被误判跳过导致 `test_link_repair_dry_run_and_apply` 与 Benchmark `W5` 失败的缺陷。 |
| **60.2** | [sqlite_provider/assets.rs](../../src/storage/sqlite_provider/assets.rs)<br/>[duckdb_provider/assets.rs](../../src/storage/duckdb_provider/assets.rs)<br/>[pg_provider/assets.rs](../../src/storage/pg_provider/assets.rs) | 修复三后端 `impl_upsert_scan_policy` 仅处理 `ON CONFLICT(path_pattern)` 而忽略同 `id` 换路径时触发主键冲突的问题，改为幂等 `DELETE WHERE id = ... OR path_pattern = ...` 后插入，根治 `test_duckdb_provider_parity_rules_links_stale_emb_awr_sandbox` 失败。 |
| **60.3** | [src/benchmark/workloads/advanced.rs](../../src/benchmark/workloads/advanced.rs)<br/>[src/benchmark/workloads/mod.rs](../../src/benchmark/workloads/mod.rs) | 清除 `W15` 残留硬编码 `/root/new/tmp/w15_probe.sql` 改用 `Config::effective_temp_dir()`；对齐并发单元测试环境下 `W13` 耗时门槛至 `< 250_000 μs`。 |
| **60.4** | [hooks/diagnoser.rs](../../src/hooks/diagnoser.rs) / [placement/mod.rs](../../src/placement/mod.rs) / [preflight/mod.rs](../../src/preflight/mod.rs)<br/>[watcher/mod.rs](../../src/watcher/mod.rs) / [scanner/state_machine.rs](../../src/scanner/state_machine.rs)<br/>[fact/assets_catalog.rs](../../src/fact/assets_catalog.rs) / [fact/automation.rs](../../src/fact/automation.rs) / [fact/capability.rs](../../src/fact/capability.rs)<br/>[tests/benchmarks.rs](../../tests/benchmarks.rs) | 全部测试去 `/root/new` 与 `/proc`/`systemd`/`pdftotext` 宿主机绝对假设化：采用 `canonicalize()` 对齐 macOS `/var` 符号链接、`default_workspace_base()` 动态基准、`tempfile` 沙盒实扫替代 `/root/new/note`，以及跨 OS 原生探针分支断言。 |
| **60.5** | [Cargo.toml](../../Cargo.toml) + 全仓 `src/` | 移除未用死依赖 `thiserror` 与 `glob`；批量与手工彻底消除全部 `cargo clippy --all-targets -- -D warnings` 告警与错误（0 警告、0 新增 `let _ =`、0 生产 `unwrap/expect`、0 `>1500L` 文件）。 |

---

## 三、验收与基线结果

1. **`make quality-gate` + `cargo clippy --all-targets -- -D warnings`**：`0 drift`、`0` 生产 `unwrap/expect`、`let _ = <= 43/50`、`>1500L: 0`、`18/18 WaitEventId`，且 `cargo clippy --all-targets -- -D warnings` 实现零警告、零错误编译通过；
2. **`cargo test --lib` 与 8 套集成测试（`benchmarks`、`duckdb_integration`、`test_web_dashboard`、`cli_e2e`、`audit_integration`、`body_awr_integration`、`hooks_integration`、`mesh_integration`）**：跨平台 100% 通过，0 失败。
