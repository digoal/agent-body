# Phase 82 演进路线与交付验收台账（ISSUE #58：会话级 `Session Read Ledger` × 多语言结构骨架优先预读）

> **定位**：在不新增顶层模块（**27 模块**）、不新增 MCP 工具（**43 工具**）、不新增 SQLite 表（**28 表**）、不引入外部 Tree-Sitter 依赖且 **100% 保持 LLM Prefix Cache 不可变**的前提下，通过零 DB Hook 快路径直击真实会话中占 Tool Calls **43.5% ~ 62.0%** 的 `read_file` 盲翻页与长会话重复读取问题（超长会话重复读高达 32.4%）。

---

## 一、核心交付与架构设计

| 子项 | 代码触点 | 机制与收益说明 |
|---|---|---|
| **82.1 多语言确定性结构骨架提取器 (`FileOutline`)** | 新增 `src/hooks/outline.rs`（`extract_file_outline` / `format_outline_compact`）、导出至 `src/hooks/mod.rs`、`hooks outline` CLI 零 DB 子命令 | 零外部 AST/Tree-Sitter 依赖，线性只读扫描 Rust / Python / JS·TS / Go / C·C++ / SQL / Markdown（自动跳过 fenced code block）/ TOML / YAML / Shell，过滤注释与二进制文件，输出精确 `L<line>` 符号导航并严格按 `token_guard_outline_max_bytes` 控容 |
| **82.2 会话级读账本 (`Session Read Ledger`)** | 新增 `src/hooks/read_ledger.rs`（`ReadLedgerStore` / `evaluate_pre_read` / `invalidate_path`）与 `src/hooks/read_ledger_helpers.rs`（确定性纯函数助手）、`hooks read-ledger` CLI 零 DB 子命令 | 基于 `~/.agent-body/tmp/read_ledger/<conv_id>.jsonl` Append-Only JSONL（POSIX `O_APPEND` 原子追加 + 100 文件自动淘汰）；记录 `(path, start, end, mtime, size, step, epoch)` 并归并区间，完全覆盖拦截（附未读关键符号导航）、3 步内 Repeat-Confirm 自动放行、部分重叠保留 5 行衔接增量裁剪 |
| **82.3 Auto-Compact 精确感知与写失效闭环** | `sync_compact_from_transcript` + `src/hooks/bridge.rs`（`try_pre_tool_fast` / `try_post_tool_fast` / `append_rx_cache`） | 增量游标扫描 `transcriptPath` 解析真实 `"type":"CHECKPOINT"`（杜绝字符串误判）及 `stepIdx` 回退自动推进 `epoch` 清空旧区间；`PostToolUse(write_file\|edit_file)` 与磁盘 `mtime+size` 变化双保险失效；首屏切片触发时追加骨架导览至 `agy_rx_<conv_id>.txt`（去重且总上限 `2600B`），下一轮 `PreInvocation` 自动消费回注 |
| **82.4 运行配置与旁路开关** | `src/config/{types,mod}.rs`、`config.example.toml`、`src/hooks/token_guard.rs`（`TokenGuardPolicy`） | 新增 `token_guard_read_ledger_enabled`（`true`）、`token_guard_read_ledger_window_steps`（`40`, `5..=500`）、`token_guard_outline_enabled`（`true`）、`token_guard_outline_max_bytes`（`1400`, `256..=8192`），支持 `AGENT_BODY_READ_LEDGER=0` 独立旁路 |

---

## 二、测试、门禁与实机验收记录

- **全工作区测试**：`RUST_TEST_THREADS=1 make test` 通过 **479 passed, 0 failed**（含根包 347 项单测、21 套集成套件、`kernel/*` 3 crates、`contrib/*` 12 扩展与 `distributions/*` 3 发行版）。
- **六重质量门禁**：`make quality-gate` 全部通过（`evolution-check` 0 漂移、`citizen-audit` 7/7 PASS、`arch-check` PASS、`lint-panic` PASS、`lint-hygiene` 0/15 `let _ =` & 0 `>1000L` 生产文件、`lint-strict` PASS）。
- **Release 构建与实机零 DB Bridge 端到端演练**（`/usr/local/bin/agent-body`，`91,380,928` bytes）：
  - `agent-body hooks outline src/hooks/token_guard.rs --after-line 250 --max-bytes 1400`：耗时 **4.93 ms**，输出 **1,339 bytes** 紧凑导览；
  - 大文件首屏自动切片 `L1~250`：`4.93 ms`；
  - `pre_invocation` 消费并回注骨架导览：`4.35 ms`；
  - 重复读 `L1~250` 零 DB 拦截（附未读符号导航）：`4.43 ms`；
  - Repeat-Confirm（3 步内同参数二次请求自动放行）：`4.27 ms`；
  - 重叠翻页 `L150~400` 自动增量裁剪为 `L246~400`：`4.31 ms`；
  - `post_tool(edit_file)` 写失效（`4.90 ms`）后重读放行：`4.68 ms`；
  - `transcript.jsonl` 真实 `CHECKPOINT` 自动推进 `epoch` 清空后放行：`4.53 ms`。

---

## 三、关联待定规划（见 `ISSUE.md` #58）

- **暂不拆分 `~/.gemini/GEMINI.md`**：保持原状待定；
- **Phase 83 候选**：`agent-body token-audit`（基于 `transcriptPath` 增量归因各工具 Token 与省流率）+ Context 高水位自适应紧凑档位。
