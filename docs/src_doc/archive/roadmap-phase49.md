# Phase 49 演进路线图：代码质量治理、位置无关性硬化与确定性门禁体系 (ISSUE #33)

> **对应 ISSUE**: [ISSUE.md #33](../../ISSUE.md#L157-L158)  
> **创建时间**: 2026-11-22 18:18 CST  
> **当前状态**: ✅ 全部完成（M1~M4 全部 11 项原子任务已验收交付）  
> **关联规约**: README 规约 #9, #11, #12, #13, #14, #16, #18, #21, #28

---

## 一、阶段目标与核心问题

1. **根治 `W10 ❌ FAIL` 与 `SLA ❌ FAIL`**：消除 `src/benchmark/workloads.rs` 中硬编码 `env!("CARGO_MANIFEST_DIR")` 导致的位置耦合，确保单二进制在任意目录/容器均可 100% 通过 `W10` 与全量 `bench run --suite full`。
2. **根治 `DQ5 ❌ FAIL`（45.3% → >= 98.0%）**：消除 `BackgroundScheduler` 对 `FreshnessReconciler::reconcile` 的静默吞错，补齐改名/搬移后的现网对账与清理。
3. **修复改名遗留高危断链与漂移**：修复 `Dockerfile`、`docker-compose.yml`、`config.example.toml`、`src/scanner/matcher.rs` 中残留的 `/.steward`；对齐 `bench --help` 与 `EVOLUTION_GUIDE.md`。
4. **构建五大确定性质量门禁**：引入 `cargo clippy --all-targets -- -D warnings` 与 `make lint-hygiene`，补齐 `evolution_manifest.py` 四大盲区。
5. **精准代码质量治理**：清理用户可见输出中的 `steward` 命名分裂、治理关键路径 `let _ =` 静默吞错、低风险模块化拆分 Top 3 God Files（`src/main.rs`、`src/api/mcp_tools.rs`、`src/benchmark/workloads.rs`）、压降 `agent-body.service` 常驻内存至 `< 90 MB`。
6. **升级工程规约与一等公民同步**：更新 `README.md`（规约 #11/#13/#28）、`EVOLUTION_GUIDE.md`、`skills/agent-body-developer/SKILL.md`、重建 `.codegraph/` 并 `make sync-mirror`。

---

## 二、原子任务断点续传追踪表

| 任务 ID | 里程碑 | 原子任务描述 | 状态 | 完成时间 | 验证摘要 |
|---|---|---|---|---|---|
| **P49-01** | M1 | 根治 `W10` 位置无关性 Bug（移除生产代码 `CARGO_MANIFEST_DIR` + 优化 `.md` 沙盒免疫） | ✅ 已完成 | 2026-11-22 18:36 | `/tmp` 异地验真 `W10` 质量 `56.0→100.0 (+44.0)` `✅ PASS` |
| **P49-02** | M1 | 修复 `Dockerfile` / `docker-compose.yml` / `config.example.toml` / `matcher.rs` 旧路径断链与 `bench --help` 漂移 | ✅ 已完成 | 2026-11-22 18:19 | 全部收敛为 `/root/.agent-body` 与 `W1~W17 / PR-01~13` |
| **P49-03** | M1 | 修复 `BackgroundScheduler` 对账静默吞错并在现网执行 `FreshnessReconciler` 恢复 `DQ5 >= 98%` | ✅ 已完成 | 2026-11-22 18:36 | `DQ5` 由 `45.3%` 恢复至 `99.8% ✅` |
| **P49-04** | M2 | 新增 `make lint-strict`（`--lib/--bin -D warnings` + 自动联动 `clippy`）清零全仓编译警告 | ✅ 已完成 | 2026-11-22 18:35 | `0 warnings` 严格编译通过 |
| **P49-05** | M2 | 升级 `scripts/evolution_manifest.py` 补齐 4 大盲区 + 新增 `scripts/code_hygiene.py` & `make quality-gate` | ✅ 已完成 | 2026-11-22 18:35 | 五重确定性门禁 0 drift / 0 panic / 0 hygiene 违规 |
| **P49-06** | M3 | 清理 Benchmark 报告 / Web UI 等用户可见输出中的 `steward` 命名分裂 | ✅ 已完成 | 2026-11-22 18:24 | 报告与 Web UI 统一 `Without Agent Body vs With Agent Body` |
| **P49-07** | M3 | 治理 `daemon` / `links` / `feedback` / `watcher` / `fact` 核心路径 `let _ =` 静默吞错 | ✅ 已完成 | 2026-11-22 18:28 | 核心自治写入吞错清零，全仓 `let _ =` 降至 `125/130` |
| **P49-08** | M3 | 低风险模块化拆分 Top 3 God Files（`src/main.rs`、`src/api/mcp_tools.rs`、`src/benchmark/workloads.rs`） | ✅ 已完成 | 2026-11-22 18:27 | 拆出 `cli_args.rs` / `mcp_schemas.rs` / `workloads/{mod,advanced}.rs` |
| **P49-09** | M3 | 压降 `agent-body.service` 内存（SQLite `cache_size=-8000` + 巡检后 `malloc_trim(0)`） | ✅ 已完成 | 2026-11-22 18:25 | 已落地缓存收敛与堆页归还机制 |
| **P49-10** | M4 | 升级 `README.md`（规约 #11/#13/#28）、`EVOLUTION_GUIDE.md`、`docs/src_doc/progress.md` 与 `SKILL.md` | ✅ 已完成 | 2026-11-22 18:30 | 规约与全局 SKILL 同步完成 |
| **P49-11** | M4 | 全量验收：`make quality-gate` + `cargo test` + 重编重启 + 真机 `bench full` + `codegraph sync` + `sync-mirror` | ✅ 已完成 | 2026-11-22 18:38 | `268/268` 测试全绿，真机全量基准与门禁全绿 |
