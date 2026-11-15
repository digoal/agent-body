# Agent Host Steward — 官方使用手册 (User Manual)

> **版本**: v0.2.0 (Rust Edition)  
> **编写标准**: 遵循项目内置技能 [`skills/agent-steward-doc-writer/SKILL.md`](../skills/agent-steward-doc-writer/SKILL.md)  
> **更新时间**: 2026-11-15T15:35:00+08:00  

---

## 欢迎阅读宿主环境大管家手册

**Agent Host Steward** 是常驻在 AI Agent 宿主机中的环境认知底座。为了帮助不同背景的开发者、DBA/运维工程师以及 AI Agent 快速掌握大管家的各项能力，本手册按由浅入深、场景化解耦的原则组织为 7 大专题章节。

---

## 手册章节目录与断点编排进度表

| 章节编号 | 章节标题 | 核心内容提要 | 目标受众 | 编写状态 |
|---|---|---|---|---|
| **[01-getting-started.md](01-getting-started.md)** | **快速上手与编译部署** | 运行环境准备、Cargo 编译、数据与配置文件初始化、首次冒烟验证 | 全体用户 / Agent | ✅ 已完成 |
| **[02-core-concepts.md](02-core-concepts.md)** | **核心架构与设计哲学** | 白名单深挖到底、黑名单浅层阻断、放置建议法则、动态进程感知 | 架构师 / 开发者 | ✅ 已完成 |
| **[03-cli-reference.md](03-cli-reference.md)** | **全量 CLI 命令行实战字典** | 涵盖 status, brief, place, asset, env, memory, links, changes, close 实操 | 终端用户 / 运维 | ✅ 已完成 |
| **[04-mcp-integration.md](04-mcp-integration.md)** | **Agent 接入与 MCP 协议指南** | JSON-RPC 2.0 stdio 协议、14 个原生工具规格、Cursor / Claude / AGY 配置 | Agent 开发者 | ✅ 已完成 |
| **[05-daemon-and-watcher.md](05-daemon-and-watcher.md)** | **守护进程与 inotify 实时监听** | 300ms 防抖、changes 审计流、60s 定时巡检、systemd 自启托管 | 系统运维 | ✅ 已完成 |
| **[06-advanced-features.md](06-advanced-features.md)** | **v2.0 进阶特性指南** | 端口防撞预警、爆炸半径推演、Ollama+FTS5 混合 RRF 检索、会话自洁 | 进阶开发者 / DBA | ✅ 已完成 |
| **[07-troubleshooting.md](07-troubleshooting.md)** | **故障诊断与常见问题 (FAQ)** | SQLite 锁超时排查、FTS5 虚拟表修复、权限问题与高频报错处置 | 全体用户 | ✅ 已完成 |

---

## 推荐阅读路径

- **如果你是初次使用的开发者/Agent**：
  请直接从 [01-getting-started.md](01-getting-started.md) 开始，花费 3 分钟完成编译并在终端跑通 `agent-steward brief`；
- **如果你想配置 Agent（如 Claude Desktop / Antigravity）通过 MCP 自动挂载大管家**：
  请重点阅读 [04-mcp-integration.md](04-mcp-integration.md)；
- **如果你在日常运维中需要排查机器端口踩踏或分析代码改动影响**：
  请查阅 [06-advanced-features.md](06-advanced-features.md)。
