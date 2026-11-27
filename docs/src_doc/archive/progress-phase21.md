# Phase 21 & Phase 22 断点续传检查点追踪表 (Progress Phase 21-22)

> **创建时间**: 2026-11-19T09:10:00+08:00  
> **当前状态**: ✅ Phase 21~22 全部完成  
> **全景路线图**: [roadmap-phase21-22.md](roadmap-phase21-22.md)  
> **规约对齐**: [README.md 核心设计原则与约束 §1-§15](../../README.md#L206-L271) & [待探讨 ISSUE #6 / #7](../../README.md#L291-L294)

---

## 一、Phase 21: 黑白屏全能力双向同步与全景工作台升级（对齐 ISSUE #6）

| 阶段 | 子任务 ID | 任务内容与检查点 | 涉及路径 | 状态 | 交付时间 |
|:---:|:---:|---|---|:---:|:---:|
| **21.1** | 21.1.1 | 补齐领域 A/B/C 白屏 REST API：`/api/v1/brief`、`/api/v1/placement/{suggest,learn}`、`/api/v1/assets/{search,register,scan}`、`/api/v1/facts/{port-check,query}`、`/api/v1/links/{audit,repair,mermaid,codegraph}` | `src/web/parity_api.rs`, `src/web/mod.rs`, `src/web/server.rs` | ✅ 已完成 | 2026-11-19 09:28 |
| | 21.1.2 | 补齐领域 D/E/F 白屏 REST API：`/api/v1/hooks/{pre,post,generate}`、`/api/v1/analytics/{heatmap,trend,export}`、`/api/v1/memories/{add,search,audit}`、`/api/v1/env/gc`、`/api/v1/session/close`、`/api/v1/locks/{acquire,wait}`、`/api/v1/ecosystem/service-check`、`/api/v1/mesh/query` 及黑白屏对齐矩阵 `/api/v1/parity/matrix` | `src/web/parity_api.rs`, `src/web/locks_api.rs`, `src/web/server.rs` | ✅ 已完成 | 2026-11-19 09:28 |
| **21.2** | 21.2.1 | 升级白屏工作台 1~3（拓扑 CodeGraph 微观下钻 + 断链审计/修复/Mermaid；事实雷达统一语义问答 + 端口防撞器 + 资产检索/登记/分级扫描；协作锁申请与排队表单）并内置全局「⌨️ 黑屏命令镜像条」 | `src/web/assets/mod.rs` | ✅ 已完成 | 2026-11-19 09:34 |
| | 21.2.2 | 升级白屏工作台 4~5（Pre/Post-Hook 演练场与客户端配置生成器、外部服务探活、自适应开工简报组装器、交互式放置推导与自演化、记忆写入/RRF混合检索/矛盾审计、DuckDB 热力图与趋势、宿主 GC 与会话结清预览、Mesh 探活与 Parity 矩阵面板） | `src/web/assets/mod.rs` | ✅ 已完成 | 2026-11-19 09:34 |

---

## 二、Phase 22: Web 路演演示剧场与产品营销互动引擎（对齐 ISSUE #7 & ISSUE #6 反向同步）

| 阶段 | 子任务 ID | 任务内容与检查点 | 涉及路径 | 状态 | 交付时间 |
|:---:|:---:|---|---|:---:|:---:|
| **22.1** | 22.1.1 | 构建 `src/demo/mod.rs` (`DemoEngine`)：实现 6 大黄金路演剧目真引擎沙盒推演（`demo:*` 隔离命名空间 + `reset_sandbox` 一键无痕复位）与商业价值 `RoiReport` 实时核算器 | `src/demo/mod.rs`, `src/lib.rs` | ✅ 已完成 | 2026-11-19 09:42 |
| | 22.1.2 | 反向同步至黑屏（新增第 39 个 MCP 工具 `body_demo_scenario` + CLI `agent-body demo` 子命令），并在 Web Server 挂载 `/api/v1/demo/{scenarios,run,reset,roi}` 与离线 PPT `/deck`、`/presentation` 路由 | `src/api/mcp_tools.rs`, `src/main.rs`, `src/web/server.rs` | ✅ 已完成 | 2026-11-19 09:42 |
| **22.2** | 22.2.1 | 实现 Web UI 第 6 工作台「🎬 路演演示剧场」：顶部 ROI 商业价值账单、6 大王牌剧目点播与自动巡航 (Auto-Cruise)、左右分屏（裸奔惨案 vs Agent 本体微秒级护航）、演讲者提词卡、黑屏命令同步框及演讲者全屏模式 | `src/web/assets/mod.rs` | ✅ 已完成 | 2026-11-19 09:46 |
| **22.3** | 22.3.T | 补充 `demo::tests`、黑白屏 39 工具 100% Parity 契约单测与 `tests/test_web_dashboard.rs` E2E 测试，确保 `make lint-panic` 与 `cargo test` 全绿 | `src/demo/mod.rs`, `src/web/mod.rs`, `tests/test_web_dashboard.rs` | ✅ 已完成 | 2026-11-19 09:54 |
| | 22.3.D | 四层贯通文档刷新（`README.md`, `docs/manual/03,04,08`, `docs/arch/02`, `agent-body-deck.html`）、3 处全局 `SKILL.md` 同步、`make restart` 现网热部署验收与 `/root/new/shared_git/agent-body` 镜像同步 | 全系统 & 现网守护进程 | ✅ 已完成 | 2026-11-19 09:55 |

---

## 三、断点恢复指引（规约 #14）

当任务中断或由新会话接手时，按以下顺序无缝恢复：
1. 读取本检查点表 `docs/src_doc/progress-phase21.md`，定位第一个状态为 `⬜ 待开始` 或 `⏳ 进行中` 的子任务 ID；
2. 对照 [roadmap-phase21-22.md](roadmap-phase21-22.md) 中的具体改造方案与验收门禁串行推进；
3. 开始某个子任务前将其状态改为 `⏳ 进行中`，通过对应验证后更新为 `✅ 已完成` 并填写交付时间；
4. Phase 21 与 Phase 22 收口后，同步刷新总进度表 [progress.md](progress.md)。
