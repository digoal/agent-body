# Agent Host Steward — 官方使用手册 (User Manual)

> **版本**: v0.4.0 (Rust Edition)  
> **编写标准**: 遵循项目内置技能 [`skills/agent-steward-doc-writer/SKILL.md`](../skills/agent-steward-doc-writer/SKILL.md)  
> **更新时间**: 2026-11-19

---

## 欢迎阅读宿主环境大管家手册

**Agent Host Steward** 是常驻在 AI Agent 宿主机中的轻量级系统认知底座（三位一体架构：宿主机事实图谱 × 用户偏好规约 × 可插拔记忆）。本手册按由浅入深、场景化解耦的原则组织为 **10 大专题章节**。

---

## 手册章节目录

| 章节编号 | 章节标题 | 核心内容提要 | 目标受众 | 编写状态 |
|---|---|---|---|---|
| **[01-getting-started.md](01-getting-started.md)** | **快速上手与编译部署** | 运行环境准备、Cargo 编译、数据与配置文件初始化、首次冒烟验证 | 全体用户 / Agent | ✅ 已完成 |
| **[02-core-concepts.md](02-core-concepts.md)** | **核心架构与设计哲学** | 三位一体架构、白名单深挖到底、黑名单浅层阻断、受限扫描与动态进程感知 | 架构师 / 开发者 | ✅ 已完成 |
| **[03-cli-reference.md](03-cli-reference.md)** | **全量 CLI 命令行实战字典** | 涵盖 status, brief, place, asset, env, memory, links, coordination, ecosystem, mesh, demo 等全量命令 | 终端用户 / 运维 | ✅ 已完成 |
| **[04-mcp-integration.md](04-mcp-integration.md)** | **Agent 接入与 MCP 协议指南** | JSON-RPC 2.0 stdio 协议、40 个原生工具规格、Cursor / Claude / Antigravity 配置 | Agent 开发者 | ✅ 已完成 |
| **[05-daemon-and-watcher.md](05-daemon-and-watcher.md)** | **守护进程与 inotify 实时监听** | 300ms 防抖、噪音门禁、changes 审计流、定时巡检、systemd 自启托管 | 系统运维 | ✅ 已完成 |
| **[06-advanced-features.md](06-advanced-features.md)** | **进阶特性指南** | 断链智能修复、放置规则自演化、宿主资源 GC、记忆生命周期与 TUI 看板 | 进阶开发者 / DBA | ✅ 已完成 |
| **[07-troubleshooting.md](07-troubleshooting.md)** | **故障诊断与常见问题 (FAQ)** | SQLite 锁超时排查、FTS5 虚拟表修复、权限问题与高频报错处置 | 全体用户 | ✅ 已完成 |
| **[08-web-dashboard.md](08-web-dashboard.md)** | **Web UI 2.0 与可视化监控大屏** | 昼夜双主题蓝图、7 大工作台、力导向拓扑图、协作锁看板、路演剧场与实证基准 | 全体用户 / 架构师 | ✅ 已完成 |
| **[09-hooks-integration.md](09-hooks-integration.md)** | **HOOK 驱动集成引擎** | EventBus 事件总线、Post-Hook 全自动回写、客户端 Hook 生成器与 Pipeline 编排 | Agent 开发者 | ✅ 已完成 |
| **[10-benchmark-and-roi.md](10-benchmark-and-roi.md)** | **实证 Benchmark 与效能存证账本** | 8 大 A/B 对照负载、5 大微基准 SLA、PR-01~08 溯源账本与 SHA-256 报告导出 | 全体用户 / 架构师 | ✅ 已完成 |

---

## 推荐阅读路径

- **如果你是初次使用的开发者 / Agent**：
  请直接从 [01-getting-started.md](01-getting-started.md) 开始，花费 3 分钟完成编译并在终端跑通 `agent-steward brief`；
- **如果你想配置 Agent（如 Claude Desktop / Antigravity / Cursor）通过 MCP 自动挂载大管家**：
  请重点阅读 [04-mcp-integration.md](04-mcp-integration.md) 与 [09-hooks-integration.md](09-hooks-integration.md)；
- **如果你想在浏览器中直观查看宿主机事实拓扑、协作锁与路演演示剧场**：
  请查阅 [08-web-dashboard.md](08-web-dashboard.md)。
