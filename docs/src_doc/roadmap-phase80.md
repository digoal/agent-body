# Phase 80 演进路线与交付验收台账（ISSUE #57：原生 PreToolUse Token 守卫 × 默认忽略扩展 × 开箱优化套件）

> **定位**：在不引入第三方 MCP Server（如 `@ai-optimizer/core`）及保持 **43 个原生 MCP 工具零膨胀** 的前提下，将经过真机验证的四层 Token 消耗优化方案内化为 `agent-body` 原生工程能力，并同步清理 `contrib/graph_relate` 质量卫生漂移与对齐全仓 16 类一等公民。

---

## 交付清单与验收实证

| 任务模块 | 核心变更与代码触点 | 验收状态 |
|---|---|---|
| **80.1 原生 Token 守卫引擎 (`TokenGuardEngine`)** | 新增 `src/hooks/token_guard.rs`（`TokenGuardPolicy` / `TokenGuardDecision` / `TokenGuardEngine` + 4 项单测）：`read_file`/`view_file`/`read` 拦截 `node_modules`/`__pycache__`/`dist`/`build`/Rust `target`/锁文件/单行压缩超长文件（`<=200` 行显式范围逃生口），大文件自动注入 `StartLine=1, EndLine=250` `overwrite`；`rg`/`grep -r`/`find` 自动注入忽略目录与 `180` 行上限；`grep_search`/`find_by_name` 拦截噪音目录；支持 `AGENT_BODY_TOKEN_GUARD=0` / `AGY_TOKEN_OPTIMIZER=0` 旁路 | ✅ 已完成 |
| **80.2 运行配置与忽略规则扩展** | `src/config/types.rs` `RuntimeConfig` 新增 5 项 `token_guard_*` 配置字段，`src/config/mod.rs` 扩展 `default_ignore_patterns()`（新增 `dist`/`build`/`.next`/`.nuxt`/`.cache`/`coverage`/`.pytest_cache`/`.mypy_cache`/`.ruff_cache`）并补齐 `validate()` 阈值校验；`config.example.toml` 同步全量注释字段 | ✅ 已完成 |
| **80.3 Hook 门禁、Bridge 与客户端生成器闭环** | `src/api/hooks.rs` `PreHookResult` 新增 `overwrite: Option<Value>`，在 `pre_tool_call_with_runtime` 接入 `TokenGuardEngine` 并落库 `audit_events`（`category="hook_pre_token_guard"`）；`src/cli_handlers/ops.rs` `hooks bridge pre_tool` 输出 `overwrite`；新增 `src/hooks/token_guard_templates.rs` 并更新 `client_gen.rs` / `client_templates.rs`，使 `hooks generate --client agy` 一键产出 `hooks.json`（`agent-body` + `token-optimizer`）、`body_bridge.py`、`token_optimizer.py` 与 `.ripgreprc`；同步 `cli_subcommands.rs` 与 `mcp_schemas_ops.rs` | ✅ 已完成 |
| **80.4 `contrib/graph_relate` 卫生硬化** | 从 `contrib/graph_relate/src/ffi.rs`（`1177L -> 778L`）拆分 `ffi_decode.rs`，清零 28 处 `let _ =` 静默吞错；`make lint-hygiene`（`0 / 15` `let _ =`、`0` `>1000L`、`0` 旧命名）、`make lint-panic` 与 `make lint-strict` 全部通过 | ✅ 已完成 |
| **80.5 集成回归与 16 类一等公民同步** | `tests/hooks_integration.rs` 新增 `test_hook_pre_token_guard_deny_and_overwrite`（5 passed）；全工作区 `RUST_TEST_THREADS=1 make test` **452 项全绿**（根包 `401` + `kernel` `27` + `contrib` `20` + `distributions` `4`）；实测 `mcp_profile="dev"`（18 工具）较 `full`（43 工具）减少 **58.1%** 工具声明与 **51.6%** Schema 字节（`35,252 -> 17,065 bytes`）；完成 `C01~C16` 全毛细血管、`architecture.svg` 与 9 份演说 HTML 同步 | ✅ 已完成 |
