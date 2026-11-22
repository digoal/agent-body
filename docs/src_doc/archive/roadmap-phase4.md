# Agent Host Steward 研发演进路线图 — 第四阶段 (Phase 4)

> **目标**: 图智能与环境拓扑演进 (Graph Intelligence & CodeGraph Federation)  
> **制定时间**: 2026-11-15  
> **执行模式**: 严格串行分步、测试驱动 (TDD)、断点随时续传、环境实操验证  
> **基线状态**: Phase 3 全量通过 (27 unit tests ok, cargo build --release 成功)

---

## 阶段任务状态看板 (Breakpoint Tracking Board)

| 任务编号 | 任务模块 | 核心工作内容 | 状态 | 验证门禁 | 完成时间 |
|---|---|---|---|---|---|
| **P4-1** | 存储层升级 | `links` 表扩展 `line_number`, `snippet`, `provenance`；增量 migration v3 | ✅ 已完成 | `cargo test storage::tests` 通过 | 2026-11-15 17:39 |
| **P4-2** | 依赖抽取器 | 语法级 import/source/require 解析，提取精确行号与代码切片 | ✅ 已完成 | `cargo test links::tests` 通过 | 2026-11-15 17:40 |
| **P4-3** | 断链体检引擎 | `LinksEngine::audit_links` 侦测缺失文件与离线服务端口，计算健康分 | ✅ 已完成 | `cargo test links::audit` 通过 | 2026-11-15 17:41 |
| **P4-4** | CodeGraph 桥接 | `CodeGraphBridge` 自动侦测 `.codegraph/`，直查微观调用方与符号拓扑 | ✅ 已完成 | `cargo test links::codegraph_bridge` 通过 | 2026-11-15 17:42 |
| **P4-5** | 爆炸半径出处增强 | `ImpactAnalyzer` 携带行号、调用片段与 CodeGraph 下钻能力 | ✅ 已完成 | `cargo test links::impact` 通过 | 2026-11-15 17:43 |
| **P4-6** | MCP & CLI 暴露 | 注册 `steward_links_audit` / `steward_codegraph_bridge` 工具与 CLI 子命令 | ✅ 已完成 | `cargo test api::mcp::tests` 通过 | 2026-11-15 17:44 |
| **P4-7** | 全量回归与构建 | 运行全量单元测试与 `cargo build --release` 冒烟运行 | ✅ 已完成 | `cargo test` 全部 PASS (31 tests) | 2026-11-15 17:45 |
| **P4-8** | 手册与 README | 迭代 `docs/manual/` 章节、`README.md` 与用户手册 | ✅ 已完成 | 文档真实反映 v3.0 新特性 | 2026-11-15 17:48 |

---

## 断点续传执行协议

1. **原子化推进**: 每一个子任务（P4-1 至 P4-8）独立自洽，严禁多任务并发混写。
2. **状态实时落盘**: 每次完成一个子任务，必须在此看板中将状态更新为 `✅ 已完成`，并记录完成时间。
3. **上下文保护**: 编写代码过程利用针对性测试，每次编译前设置 `export PATH="/root/.cargo/bin:$PATH"`。
