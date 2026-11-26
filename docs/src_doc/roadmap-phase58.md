# Phase 58 演进路线图 — 三态运行模式与真实 A/B 证伪 (`ISSUE #43`) + MCP Profile 减税 + 原生 Hook 桥接

> **启动时间**：2026-11-26 | **前置基线**：Phase 57 (`v1.0.0 GA`) | **状态**：`✅ 已完成`

---

## 一、核心目标
在保持 `27 模块 / 43 MCP 工具 / 88 SPI 方法 / 27 表` 零破坏兼容与五重门禁全绿前提下：
1. 落地 **ISSUE #43** 三态运行模式（`body_only` / `bypass` / `shadow_ab`）及真实世界 A/B 对照分析引擎，消除“实验室高分掩盖真实帮倒忙场景”的盲区；
2. 引入 MCP Schema 分层档位（`core` 10 工具 `<12KB` / `dev` 18 工具 / `ops` 20 工具 / `full` 43 工具），削减 ~78% 每轮上下文工具税；
3. 落地原生 Rust Hook 桥接器 `agent-body hooks bridge`，并在 Python 桥接脚本加入 `AGENT_BODY_MODE=bypass` 零子进程短路；
4. 修复 Web CORS `*` 跨域泄露面、HTTP Body 二进制解析与 `HybridSearchEngine` 重复全表查询/500 条硬截断。

---

## 二、断点续传任务台账

| 任务编号 | 任务内容 | 目标产物 | 状态 |
| :--- | :--- | :--- | :--- |
| `P58-01` | 新增 `[runtime]` 配置（`RuntimeMode` / `McpToolProfile` / `RuntimeConfig` / `AGENT_BODY_MODE` / `AGENT_BODY_MCP_PROFILE` / `update_runtime_mode`）与 `config.example.toml` 模板 | `src/config/mod.rs`, `config.example.toml` | `✅ Done` |
| `P58-02` | 新建 `ShadowAbEngine`（原生实时搜索 vs FTS 双轨对照、磁盘陈旧检测、漏召回/帮倒忙率统计、`audit_events` `category="ab_shadow"` 落库与汇总报表） | `src/body/runtime_ab.rs`, `src/body/mod.rs` | `✅ Done` |
| `P58-03` | MCP Profile 减税（`core`/`dev`/`ops`/`full`）、`body_brief` / `body_asset_search` / `body_hook_pre` 接入三态模式、`body_state` 新增 `runtime_mode` / `ab_report` / `ab_compare` 与原生 `hooks bridge` | `src/api/{mcp_schemas,mcp_tools,mcp,hooks}.rs`, `src/hooks/client_gen.rs` | `✅ Done` |
| `P58-04` | CLI (`mcp --profile`, `hooks bridge`, `body runtime-mode|ab-report|ab-compare`) 与 Web REST API (`/api/v1/body/runtime-mode`, `ab-report`, `ab-compare`) 1:1:1 同步 | `src/cli_args.rs`, `src/cli_handlers/*`, `src/web/parity_api/gov_body_endpoints.rs` | `✅ Done` |
| `P58-05` | 安全与检索快赢：Web CORS 回环白名单收敛、Header-only UTF-8 解析、修正 `AgentSteward-Web` 残留、`HybridSearchEngine` 单次查库（上限 2000）+ 客户端复用 | `src/web/{http,server}.rs`, `src/memory/hybrid.rs`, `tests/test_web_dashboard.rs` | `✅ Done` |
| `P58-06` | 文档、路线图与 `ISSUE.md` / `user_feedback.md` 尾债同步，过 `make quality-gate && cargo test` 与端到端 CLI 冒烟实证 | `docs/src_doc/{roadmap-phase58,progress}.md`, `EVOLUTION_GUIDE.md`, `ISSUE.md`, `user_feedback.md` | `✅ Done` |
