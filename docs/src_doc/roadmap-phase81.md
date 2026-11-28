# Phase 81 演进路线与交付验收台账（ISSUE #58：Rust 单入口 Hook 统一 × `agent-body distill` 命令输出蒸馏器）

> **定位**：在不新增 MCP 工具（保持 **43 工具 / 27 模块 / 95 SPI / 28 SQLite 表** 零膨胀）且**100% 保持 LLM Prefix Cache 不可变前提**下，直击真实会话中 91.4% 复合/高噪 `run_command` 逃逸与双 Python Hook 进程开销，落地「写入前蒸馏（Pre-Ingestion Distillation）」架构。

---

## 核心交付与架构演进

| 模块 | 关键实现与代码触点 | 说明 |
|---|---|---|
| **81.1 确定性命令输出蒸馏引擎 (`OutputDistiller`)** | 新增 `src/hooks/distill.rs`（`DistillKind` / `strip_ansi_and_progress` / `distill_output` / `run_distill_cli`）与顶层 CLI `agent-body distill`（`src/cli_args.rs`、`src/main.rs` 零 DB 快路径） | 零 LLM 调用，自动剥离 ANSI 色码与 `\r` 进度条，按 `BuildTest` / `GitDiff` / `SearchList` / `HttpFetch` / `Generic` 分类折叠；失败时 100% 保留 `FAILED`/`panicked at`/`error` 上下文与原始退出码；超阈值时完整原始日志无损落盘 `~/.agent-body/tmp/distill/`（保留最近 200 份） |
| **81.2 复合与高噪命令 `TokenGuard` 自动包装** | 扩展 `src/config/types.rs`（`token_guard_distill_enabled` / `max_lines` / `max_bytes`）、`src/config/mod.rs`、`config.example.toml` 及 `src/hooks/token_guard.rs` | 保留独立 `rg`/`grep`/`find` 原生限流兼容的同时，为含 `&&`/`;`/`||` 的复合命令与 `cargo test/build/clippy`/`make`/`pytest`/`go test`/`git diff/log/show`/`curl`/`wget`/`codegraph explore` 自动注入 `agent-body distill --shell '...'` `overwrite`；跳过 `IsDaemon`、交互式命令、heredoc 与防递归包装，支持 `AGENT_BODY_DISTILL=0` 独立旁路 |
| **81.3 Rust 原生单入口 AGY Hook 桥接器** | 新增 `src/hooks/bridge.rs`（`handle_cli_bridge` / `handle_bridge_with_storage`），升级 `src/hooks/client_templates.rs` `AGY_HOOKS_JSON_TEMPLATE` 与 `src/cli_handlers/ops.rs` | 将 `hooks.json` 的双 Python 进程（`body_bridge.py` + `token_optimizer.py`）统一为单一原生 Rust 入口 `agent-body hooks bridge <session_start\|pre_tool\|post_tool\|pre_invocation\|stop>`；`pre_invocation`/`stop`/纯放行 `pre_tool`/常规 `post_tool` 全程零 SQLite 打开极速放行 |

---

## 关联待定规划（见 `ISSUE.md` #58）

- **暂不拆分 `~/.gemini/GEMINI.md`**：继续保持原状待定；
- **Phase 82+ 候选**：会话级 `Session Read Ledger`（拦截长会话重复读与盲翻页，支持 auto-compact 感知清空）+ AST 骨架优先预读 + `transcriptPath` Token 遥测归因与水位自适应。
