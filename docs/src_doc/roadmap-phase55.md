# Phase 55 演进路线图：开源可移植解耦、配置真相化与跨平台/网络加固 (ISSUE #39)

> **对应 ISSUE**: [ISSUE.md #39](../../ISSUE.md)  
> **所属计划**: 方案 A · `v1.0.0 GA` 工业级收口路线（Phase 52 ~ Phase 57）  
> **创建时间**: 2026-11-22 23:21 CST  
> **当前状态**: ✅ 全部完成（P55-01 ~ P55-06 全部 6 项原子任务已交付：开源可移植路径与属主解耦、死配置真相化与运行时接线、`gnu`/macOS/Windows/HTTP 头读取硬化、`make quality-gate && make test && make restart` 与 `make sync-mirror` 闭环）  
> **关联规约**: README 规约 #2, #9, #11, #13, #19, #28

---

## 一、阶段目标与治理边界

1. **P55-01 阶段立项**：创建 `roadmap-phase55.md`，更新 `ISSUE.md #39` 为进行中，更新 `EVOLUTION_GUIDE.md` 活跃路线图指针。
2. **P55-02 配置真相化与可移植工作区模型 (`src/config/mod.rs` & `config.example.toml`)**：
   - 接入 `AGENT_BODY_CONFIG`（优先于 `AGENT_BODY_CONFIG`），清理已失效死字段 `api_port` / `api_bind`（保持对旧版 `config.toml` serde 静默兼容）；
   - 新增 `[workspace]` (`WorkspaceConfig`) 与宿主自适应默认值（`$HOME/new` 存在时自动命中以保持 Zero-Regression，否则使用便携 `$HOME/agent-body-workspace` 与 `std::env::temp_dir().join("agent-body")`）；
   - `default_owner()` 改为从 `$USER`/`$USERNAME` 动态推导（fallback `user:local`）；
   - 新增 `[memory]` (`MemoryConfig`) 与 `Config::build_memory_provider()` 对接 `MemoryFactory::create`；新增 `pipelines: Vec<HookPipeline>`；
   - 补齐 `[mesh]` 完整校验（接受 `mesh.shared_secret` / `web.auth_token` / `mesh.peer_keys` 任一鉴权配置，校验 `timeout_ms`、`replay_window_secs`、`max_transfer_bytes`）与 `[memory].provider` 校验。
3. **P55-03 全仓消除生产代码 `/root/new/` 与 `user:digoal` 硬编码**：
   - 将 SQLite / PostgreSQL / DuckDB 的初始 `placement_rules` & `scan_policies` 种子改为基于 `default_workspace_base()` 与 `$HOME` 幂等注入；
   - 解除 `api` / `web` / `duckdb_analytics` / `session` / `env` / `fact` / `ecosystem` / `feedback` / `preflight` / `links` / `scanner` / `watcher` / `placement` / `hooks` / `demo` / `cli_args` / `main` 等生产模块对 `/root/new/` 与 `user:digoal` 的硬编码耦合。
4. **P55-04 将死配置真实接入运行时主链路**：
   - `[memory]` / `MemoryFactory` 接入 `main.rs` / `mcp_tools.rs` / `parity_api.rs` 中的 `BriefAssembler` 调用点；
   - `[[hooks]]` (`HookDispatcher`) 与 `[[pipelines]]` (`HookPipelineEngine`) 订阅 `EventBus` 并在 `BodyDaemon::run()` 后台执行；
   - `assets.ignore_patterns` 接入 `NoiseFilter` / `Scanner` / `ScanStateMachine` / `FileWatcher` / `ChangeHandler`；
   - `embedding.fallback_fts_only` 接入 `HybridSearchEngine`，当设置为 `false` 且 Ollama 不可用时显式返回错误而非静默降级。
5. **P55-05 跨平台与网络健壮性加固**：
   - `malloc_trim(0)` 三处调用全部改为 `#[cfg(all(target_os = "linux", target_env = "gnu"))]` 以兼容 Linux musl；
   - `KernelSelfTelemetry::collect` 增加非 Linux / 无 `/proc` 场景下的 `sysinfo` 回退路径；
   - Windows `PlatformProbe::listening_ports` 支持中文/多语言本地化 `netstat -ano` 状态词（如 `侦听` / `ABHÖREN` / `ÉCOUTE` 等）同时严格过滤已连接行；
   - `WebServer::handle_connection` 补齐 32KB 有界 HTTP 头循环读取，修复分包/大请求头读取截断问题。
6. **P55-06 测试、质量门禁、真机部署与台账收口**：补齐新增覆盖单测与集成测试，通过 `make quality-gate && make test`、`make restart` + 现网 `preflight` + `bench run --suite full`、`make sync-mirror` 与台账更新。

---

## 二、原子任务断点续传追踪表

| 任务 ID | 里程碑 | 原子任务描述 | 状态 | 完成时间 | 验证摘要 |
|---|---|---|---|---|---|
| **P55-01** | M1 | 创建 `roadmap-phase55.md`，更新 `ISSUE.md` & `EVOLUTION_GUIDE.md` | ✅ 已完成 | 2026-11-22 23:25 | Tier-0 活跃路线图已切换 |
| **P55-02** | M1 | `AGENT_BODY_CONFIG` / `[workspace]` / `[memory]` / `[[pipelines]]` / `[mesh]` 校验 & 清理死字段 | ✅ 已完成 | 2026-11-22 23:41 | 环境变量优先、legacy `api_port`/`api_bind` 静默兼容、三段配置校验实装 |
| **P55-03** | M2 | 全仓消除生产路径 `/root/new/` 与 `user:digoal` 硬编码 & 三后端动态种子 | ✅ 已完成 | 2026-11-22 23:41 | SQLite/PG/DuckDB 动态种子 + 全生产模块可移植路径与属主解耦 |
| **P55-04** | M3 | 接线 `MemoryFactory`、`HookDispatcher`/`HookPipelineEngine`、`ignore_patterns`、`fallback_fts_only` | ✅ 已完成 | 2026-11-22 23:54 | `BriefAssembler` / `BodyDaemon` EventBus / `Scanner`+`Watcher` / `HybridSearchEngine` 全接线 |
| **P55-05** | M3 | `gnu` 条件编译、`KernelSelfTelemetry` `sysinfo` 回退、Windows 本地化 `netstat`、HTTP 头循环读取 | ✅ 已完成 | 2026-11-22 23:41 | musl/non-Linux/多语言 Windows 适配 + 32KB 有界 HTTP 头循环读取 |
| **P55-06** | M4 | 新增测试、`make quality-gate && make test`、`make restart` + 现网验证、`make sync-mirror` | ✅ 已完成 | 2026-11-22 23:55 | 全量门禁、测试、守护重编译部署、Bench v3.8 与镜像同步验收 |
