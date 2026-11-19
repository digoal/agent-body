# Agent Host Steward — Phase 27 ~ Phase 28 断点续传进度表

> **关联路线图**: [roadmap-phase27-28.md](roadmap-phase27-28.md)  
> **设计规格**: [10-benchmark-spec.md](../design_doc/10-benchmark-spec.md)  
> **创建时间**: 2026-11-19T15:18:00+08:00  
> **当前状态**: ✅ Phase 27~28 全部完成 (`2026-11-19T15:54:00+08:00`) · 202 tests passed · 40/40 黑白屏 100% 对齐

| 阶段 | 步骤 ID | 任务名称 | 状态 | 备注 |
|:---:|:---:|---|:---:|---|
| **Phase 27** | **27.1** | 规约 #18 入宪、Benchmark 设计规格与路线图落盘 | ✅ 已完成 | 更新 `README.md`（规约 #18 / ISSUE #13），新增 `design_doc/10-benchmark-spec.md`、`roadmap-phase27-28.md`、`progress-phase27.md` |
| | **27.2** | 存储层三后端（SQLite / PG / DuckDB）Schema 与 6 个 SPI 方法扩展 | ✅ 已完成 | 三后端 Schema + 6 SPI 方法 + `test_benchmark_runs_and_effectiveness_ledger` 通过，`lint-panic` 0 违规 |
| | **27.3** | 实现 `src/benchmark/` 双轨核心引擎（`W1~W8`、`M1~M5`、`PR-01~08`、SHA-256 报告） | ✅ 已完成 | 8 大 A/B 负载 + 5 大微基准全绿，`bench:*` 沙盒自动清除，SHA-256 签名就绪 |
| | **27.4** | 生产调用链路自动记账埋点（`mcp.rs` / `hooks.rs` / `scanner.rs`）与 `DemoEngine` 联动 | ✅ 已完成 | `call_tool` 与 `Scanner::scan_path` 自动按 `PR-01~08` 记账，`DemoEngine::compute_roi` 融合真实账本 |
| | **27.5** | 注册第 40 个原生 MCP 工具 `steward_benchmark`、1:1 CLI `bench` 与扩展 `tests/benchmarks.rs` | ✅ 已完成 | 40 MCP 与 CLI 1:1 对齐，`tests/benchmarks.rs` 6/6 全绿，`lint-panic` 0 违规 |
| **Phase 28** | **28.1** | Web 后端 `/api/v1/benchmark/*` REST API 与 40/40 Parity 契约矩阵升级 | ✅ 已完成 | 6 条 Benchmark REST API + `40/40 (100%)` Parity 矩阵全绿 |
| | **28.2** | Web UI 新增「📊 实证基准与效能账本」第 7 工作台（双主题 + 命令镜像） | ✅ 已完成 | A/B 对照柱状条、微基准 SLA、PR-01~08 流水账本、跨批次对比器上线 |
| | **28.3** | 路演演示剧场与 `presentation.html` / `steward-deck.html` 实证数据锚点联动 | ✅ 已完成 | 路演 ROI 栏实时联动生产账本累计节省 Token 与最新实测加速比 |
| | **28.4** | `[benchmark]` 配置段、手册第 10 章、3 处 SKILL 同步、热部署与镜像同步 | ✅ 已完成 | 202 tests 全绿、`lint-panic` 0 违规、现网真机首跑存证、Daemon 重启与镜像同步完成 |
