# Agent Body — 官方使用手册 (User Manual)

> **版本**: v1.1.0 (Rust Edition · Phase 64~78 · 旗舰版、微内核 11 `contrib/` 扩展、`M1~M3` PG 事务海马体方案 A→B 闭环与全栈可观测)  
> **编写标准**: 遵循项目内置技能 [`skills/agent-body-doc-writer/SKILL.md`](../../skills/agent-body-doc-writer/SKILL.md)  
> **更新时间**: 2026-11-28

---

## 欢迎阅读 Agent 本体（宿主具身底座）官方手册

**Agent Body** 是脑体分离架构下常驻宿主机底层的 **Agent 本体（Agent Embodiment Substrate）**——大语言模型是可随时切换的瞬态大脑，本系统是永驻物理宿主机的数字躯干（宿主机事实感官 × 小脑反射与规约脊柱 × 可插拔海马体与跨大脑连续性接力；外置辅助工具隐喻为本体对外两大功能侧面）。本手册按由浅入深、场景化解耦的原则组织为 **10 大专题章节**。

---

## 手册章节目录

| 章节编号 | 章节标题 | 核心内容提要 | 目标受众 | 编写状态 |
|---|---|---|---|---|
| **[01-getting-started.md](01-getting-started.md)** | **快速上手与编译部署** | 运行环境准备、Cargo 编译、数据与配置文件初始化、首次冒烟验证 | 全体用户 / Agent | ✅ 已完成 |
| **[02-core-concepts.md](02-core-concepts.md)** | **核心架构与设计哲学** | 三位一体架构、白名单深挖到底、黑名单浅层阻断、受限扫描与动态进程感知 | 架构师 / 开发者 | ✅ 已完成 |
| **[03-cli-reference.md](03-cli-reference.md)** | **全量 CLI 命令行实战字典** | 涵盖 status, brief, place, asset, env, memory, links, coordination, ecosystem, mesh, demo, bench, feedback, audit, body 等全量 30 顶级命令 | 终端用户 / 运维 | ✅ 已完成 |
| **[04-mcp-integration.md](04-mcp-integration.md)** | **Agent 接入与 MCP 协议指南** | JSON-RPC 2.0 stdio 协议、43 个原生工具规格、Cursor / Claude / Antigravity 配置 | Agent 开发者 | ✅ 已完成 |
| **[05-daemon-and-watcher.md](05-daemon-and-watcher.md)** | **守护进程与 inotify 实时监听** | 300ms 防抖、写穿失效与删除级联、FreshnessReconciler 对账、双频后台巡检（60s 进程轮询 + 1800s 深度巡检）与 systemd/LaunchAgent 自启 | 系统运维 | ✅ 已完成 |
| **[06-advanced-features.md](06-advanced-features.md)** | **进阶特性指南** | 端口防撞、分层加权图谱与断链修复、MeshEnvelope 零信任协同、AOPCC 乐观租约、新鲜度探针与 TUI | 进阶开发者 / DBA | ✅ 已完成 |
| **[07-troubleshooting.md](07-troubleshooting.md)** | **故障诊断与常见问题 (FAQ)** | SQLite 锁超时排查、FTS5 虚拟表修复、权限问题与高频报错处置 | 全体用户 | ✅ 已完成 |
| **[08-web-dashboard.md](08-web-dashboard.md)** | **Web UI 2.0 与可视化监控大屏** | 昼夜双主题蓝图、9 大工作台、本体中枢、双域审计、力导向拓扑图、路演剧场、43/43 同步与实证基准 | 全体用户 / 架构师 | ✅ 已完成 |
| **[09-hooks-integration.md](09-hooks-integration.md)** | **HOOK 驱动集成引擎** | AOPCC 悲观-乐观自适应门禁、碰壁 `<2ms` 自愈、EventBus、AutoSync 与 Pipeline 编排 | Agent 开发者 | ✅ 已完成 |
| **[10-benchmark-and-roi.md](10-benchmark-and-roi.md)** | **实证 Benchmark 与效能存证账本** | Benchmark v4.1：W1~W22 + M1~M6 + DQ1~DQ12 + COMP-C1~C4 + PR-01~16 溯源账本与 SHA-256 导出 | 全体用户 / 架构师 | ✅ 已完成 |

---

## 推荐阅读路径

- **如果你是初次使用的开发者 / Agent**：
  请直接从 [01-getting-started.md](01-getting-started.md) 开始，花费 3 分钟完成编译并在终端跑通 `agent-body brief`；
- **如果你想配置 Agent（如 Claude Desktop / Antigravity / Cursor）通过 MCP 自动挂载Agent 本体**：
  请重点阅读 [04-mcp-integration.md](04-mcp-integration.md) 与 [09-hooks-integration.md](09-hooks-integration.md)；
- **如果你正在生产部署政务云探针、DBA 哨兵或接入 Prometheus/Loki 监控体系**：
  请结合 [05-daemon-and-watcher.md](05-daemon-and-watcher.md)、[07-troubleshooting.md](07-troubleshooting.md) 与 [08-web-dashboard.md](08-web-dashboard.md) 完成 `/metrics`、W3C `traceparent` 与 `agent-body-probe metrics/diagnose` 配置。
