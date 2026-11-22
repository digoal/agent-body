# Agent Host Steward — 文档全量刷新检查点（断点续传）

> 创建时间: 2026-11-19T16:15:00+08:00  
> 目标版本: `v0.4.0`（24 源码模块 · 40 原生 MCP 工具 · 27 CLI 顶层命令 · 7 大 Web 工作台 · 10 卷手册）  
> 遵循规约: `README.md` 规约 #10~#18、`skills/agent-steward-doc-writer/SKILL.md` 六大铁律

---

## 阶段执行进度总表

| 阶段 / 批次 | 目标文件 | 目标口径 / 核心动作 | 状态 |
|---|---|---|:---:|
| **Step 0** | `docs/src_doc/progress-doc-refresh.md` | 建立断点续传追踪台账 | ✅ |
| **Step 1** | 源码 & 二进制实跑 | `cargo test`、`release` 二进制只读命令实跑捕获、核对 `mcp_tools.rs` / `server.rs` | ✅ |
| **Batch 1 (P0)** | `README.md` | 更新 24 模块、实测测试数、去掉 v3.1 混用、补充 40/40 说明 | ✅ |
| | `config.example.toml` | 补齐 `[mesh]` 与 `[[hooks]]` 示例并验证反序列化 | ✅ |
| | `INSTALL.md` | 升级至 `v0.4.0+`、补充 `qwen3.5:0.8b`、刷新 `preflight` 实跑输出 | ✅ |
| | `docs/manual/README.md` | 修正“9 大专题章节”为“10 大专题章节” | ✅ |
| | `docs/manual/03-cli-reference.md` | 刷新 `v0.2.0` 输出并补齐 `storage`、`bench` 四段式文档 | ✅ |
| | `docs/manual/04-mcp-integration.md` | 更新 40 个工具并补齐第 40 工具 `steward_benchmark` | ✅ |
| | `docs/manual/08-web-dashboard.md` | 对齐 `v0.4.0` / 7 大工作台 / 40/40 / `/api/v1/benchmark/*` | ✅ |
| **Batch 2 (P1)** | `skills/agent-steward-doc-writer/SKILL.md` | 修正 40 工具 / 10 卷手册 / `01~10` 并同步全局镜像 | ✅ |
| | `skills/agent-steward-guide/SKILL.md` | 补齐 `steward_benchmark`、修正 40 工具与精确测试数并同步镜像 | ✅ |
| | `skills/agent-steward-developer/SKILL.md` | 核对 24 模块 / 40 工具 / RULE #18 / 精确测试数并同步镜像 | ✅ |
| | `docs/arch/01-for-architects.md` | 40 Tools + 规约 #18 双轨实证评测体系 | ✅ |
| | `docs/arch/02-for-developers.md` | 24 模块（补 `src/benchmark/`）、40 工具、实测测试数 | ✅ |
| | `docs/arch/03-for-users.md` | 40 工具 + Web UI 2.0 昼夜主题、路演剧场、`bench` | ✅ |
| | `docs/src_doc/development.md` | `v0.1.0~v0.4.0`、`src/benchmark/`、40 工具、测试说明 | ✅ |
| | `docs/releases/v0.4.0.md` & `docs/releases/README.md` | 合入 Phase 27~28（40 工具、24 模块、实测测试数） | ✅ |
| **Batch 3 (P2)** | `docs/manual/01-getting-started.md` | 刷新 `status` 实跑输出至 `v0.4.0` | ✅ |
| | `docs/manual/05-daemon-and-watcher.md` | 刷新 `v0.4.0` 日志、补充 Ollama 批处理与 `LinksEngine` 刷新 | ✅ |
| | `docs/manual/06-advanced-features.md` | 清理 `v2.0`/`v3.1`，补充三层元数据/WFG/Mesh/Benchmark | ✅ |
| | `docs/manual/07-troubleshooting.md` | 补充 `preflight --json`、Ollama 降级、沙盒 `reset` 排障 | ✅ |
| | `docs/markdown/why-agent-needs-host-steward.md` | 40 工具、去 `v3.0`、自然融入实证 Benchmark 证据链 | ✅ |
| | `docs/design_doc/05-architecture.md` & `06-api-spec.md` | 文首添加历史基线声明与现行权威规格指向 | ✅ |
| **Batch 4 (P2/P3)** | `presentation.html` | 升级 40 工具 / 27 子命令 / 202 测试 / 补充 Benchmark | ✅ |
| | `steward-deck.html` | 统一 40 工具（修正 22/39 冲突）/ 真实 79MB·20.8MB 口径 / 补充 Benchmark | ✅ |
| | 渲染与门禁验收 | Playwright 实测 HTML、`cargo check --tests`、`cargo test`、`cargo build --release`、全量死链/规范审计 | ✅ |

---

## 最终验收实证记录（2026-11-19）
- **Playwright 渲染与交互检查**：`presentation.html`（16 页）与 `steward-deck.html`（22 页）均通过无报错、0 SVG 文字越界、0 SVG 文字重叠、`N`/`T`/`G`/动画按钮响应正常。
- **Rust 静态与编译门禁**：`cargo check --tests` 0 warning；`make lint-panic` 0 生产 `unwrap`/`expect`；`cargo test` **202 passed, 0 failed, 1 doc-test ignored**；`cargo build --release` 成功（含重新内嵌更新后 HTML）。
- **文档与镜像审计**：90 份 `.md` 文件 206 处相对链接 **0 死链**；0 个 Mermaid 未引号括号节点；`skills/agent-steward-{developer,doc-writer,guide}/SKILL.md` 与 `/root/.gemini/antigravity-cli/skills/` 三套镜像 100% 同步。
