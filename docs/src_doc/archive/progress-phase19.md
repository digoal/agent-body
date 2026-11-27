# Phase 19 & Phase 20 断点续传检查点追踪表 (Progress Phase 19-20)

> **创建时间**: 2026-11-19T07:32:00+08:00  
> **当前状态**: ✅ Phase 19~20 全部完成  
> **全景路线图**: [roadmap-phase19-20.md](roadmap-phase19-20.md)  
> **规约对齐**: [README.md 核心设计原则与约束 §5/§7/§9/§10-§14](../../README.md#L225-L271) & [待探讨 ISSUE #5](../../README.md#L288)

---

## 一、Phase 19: 真实事实拓扑修正与 Web UI 2.0 全景交互指挥舱

| 阶段 | 子任务 ID | 任务内容与检查点 | 涉及路径 | 状态 | 交付时间 |
|:---:|:---:|---|---|:---:|:---:|
| **19.1** | 19.1.1 | 修复 `EnvCollector::collect_and_sync_processes()`：过滤 Linux 线程（`thread_kind().is_some()`）及系统底噪，按 PID 关联真实监听端口 | `src/env/mod.rs` | ✅ 已完成 | 2026-11-19 07:37 |
| | 19.1.2 | 重构 `build_full_topology()` 与 `build_impact_topology()`：整合 `fact_nodes`、`fact_edges`、活跃进程与 `links` 输出真实连通图谱 | `src/web/topology.rs` | ✅ 已完成 | 2026-11-19 07:37 |
| **19.2** | 19.2.1 | 扩展 `SystemOverview` 并新增只读全景 API：`/api/v1/{facts,ecosystem,ecosystem/route,governance,mesh/status}` | `src/web/api.rs`, `src/web/server.rs` | ✅ 已完成 | 2026-11-19 08:05 |
| | 19.2.2 | 补齐 `handle_locks_history()` 审计查询，并在 `BodyDaemon` 打通 Watcher/Scheduler -> SSE `/api/v1/stream` 实时事件推流 | `src/web/locks_api.rs`, `src/daemon/mod.rs` | ✅ 已完成 | 2026-11-19 08:05 |
| **19.3** | 19.3.1 | 重构 Web UI 2.0 视觉框架（暗色科技驾驶舱、全局 KPI 条、5 大工作台导航、可折叠 SSE 流与节点抽屉，100% 离线自包含） | `src/web/assets/mod.rs` | ✅ 已完成 | 2026-11-19 08:15 |
| | 19.3.2 | 实现工作台 1「宿主机事实拓扑交互图（移除 `drawMockGraph`，支持拖拽/缩放/筛选/爆炸半径）」与工作台 2「运行时事实雷达」 | `src/web/assets/mod.rs` | ✅ 已完成 | 2026-11-19 08:15 |
| | 19.3.3 | 实现工作台 3「协作锁/等待队列/WFG死锁雷达」、工作台 4「生态防爆路由器沙盒」与工作台 5「规约/记忆/变更/Mesh」 | `src/web/assets/mod.rs` | ✅ 已完成 | 2026-11-19 08:15 |
| **19.4** | 19.4.T | 补充 Web 2.0 新增 API/真实拓扑/进程去噪单元测试与 E2E 集成测试，确保 `cargo test` 全绿且零 unwrap | `src/web/mod.rs`, `src/env/mod.rs`, `tests/test_web_dashboard.rs` | ✅ 已完成 | 2026-11-19 08:21 |

---

## 二、Phase 20: 四层贯通文档刷新、全局 SKILL 同步与生产热部署

| 阶段 | 子任务 ID | 任务内容与检查点 | 涉及路径 | 状态 | 交付时间 |
|:---:|:---:|---|---|:---:|:---:|
| **20.1** | 20.1.1 | 补全 `docs/manual/04-mcp-integration.md` 第 3 节全部 38 个 MCP 工具字典，并重写 `docs/manual/08-web-dashboard.md` 匹配 Web 2.0 | `docs/manual/04-mcp-integration.md`, `docs/manual/08-web-dashboard.md` | ✅ 已完成 | 2026-11-19 08:18 |
| | 20.1.2 | 刷新 `README.md`、`docs/arch/02-for-developers.md`、`docs/src_doc/progress.md` 的模块/SPI/测试实测数字与 ISSUE 状态 | `README.md`, `docs/arch/02-for-developers.md`, `docs/src_doc/progress.md` | ✅ 已完成 | 2026-11-19 08:23 |
| **20.2** | 20.2.1 | 对齐并同步三套 `SKILL.md` 到项目 `skills/`、`/root/new/skills/` 与 `/root/.gemini/antigravity-cli/skills/` | `skills/agent-body-*/SKILL.md` | ✅ 已完成 | 2026-11-19 08:18 |
| | 20.2.T | 全量验证（`make check && make lint-panic && make test && make restart`）、现网 Web 2.0 真实接口与视图检查及 `/root/new/shared_git/` 同步 | 全系统 & 现网守护进程 | ✅ 已完成 | 2026-11-19 08:23 |

---

## 三、断点恢复指引（规约 #14）

当任务中断或由新会话接手时，按以下顺序无缝恢复：
1. 读取本检查点表 `docs/src_doc/progress-phase19.md`，定位第一个状态为 `⬜ 待开始` 或 `⏳ 进行中` 的子任务 ID；
2. 对照 [roadmap-phase19-20.md](roadmap-phase19-20.md) 中的具体改造方案与验收门禁串行推进；
3. 开始某个子任务前将其状态改为 `⏳ 进行中`，通过对应验证后更新为 `✅ 已完成` 并填写交付时间；
4. Phase 19 与 Phase 20 收口后，同步刷新总进度表 [progress.md](progress.md)。
