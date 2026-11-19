# Agent Host Steward — 官方使用手册：第 8 章 Web UI 2.0 全景交互指挥舱

> 版本: v0.3.2+ (Web UI 2.0)  
> 技术架构: Tokio 异步内嵌 HTTP 引擎 · 原生 Canvas 力导向交互拓扑 · 5 大工作台 · 单二进制 100% 离线自包含 · 零外部 CDN

---

## 1. 概述与五大核心工作台

**Web UI 2.0** 是 Agent Host Steward 提供的轻量级浏览器全景指挥舱，所有 HTML/CSS/JS/Canvas 渲染引擎均编译内嵌于单二进制中，零外部 CDN/Node.js 依赖：

1. **🕸️ 宿主机事实拓扑图谱 (Host Reality Topology)**：
   - 100% 源自 `/proc` 进程、监听端口、Systemd 服务、Cron 任务、`fact_nodes`/`fact_edges` 属性图与 `links` 依赖边；
   - 支持滚轮缩放、画布平移、节点拖拽、类型过滤开关、关键词搜索高亮、点击节点查看物理元数据，并支持一键触发 **3 跳爆炸半径子图推演**（`/api/v1/topology/impact`）。
2. **📡 运行时事实雷达 (Runtime Facts Radar)**：
   - 分 5 个页签实时透视：监听端口表、去噪后的活跃任务进程（自动过滤线程与系统底噪并关联端口）、Systemd & Cron 自动化拓扑、Git 工程仓库（含 `CodeGraph Ready` 标记）与本地 CLI 工具能力。
3. **🔒 多 Agent 协作锁与死锁雷达 (Coordination & WFG)**：
   - 实时监控活跃锁与剩余 TTL 进度条（支持可选 Bearer Token 管理员强制释放）、`lock_wait_queue` 等待队列、Wait-For Graph (WFG) 死锁环路自检结果及锁审计历史。
4. **🧭 生态防爆路由器 (Ecosystem & Context Budget Router)**：
   - 浏览已纳管的 MCP Server、120+ SKILL 目录与脱敏后的外部服务通讯录；
   - 内置交互式 **Context Budget Router 演练沙盒**，输入任务意图与 Token 预算即可实时预览 Top-K 召回清单与节省 Token 百分比。
5. **📐 规约 · 记忆 · 变更审计与 Mesh (Governance & Mesh)**：
   - 集中展示 8 大资产放置规则、跨会话长期记忆（支持 FTS 搜索）、最近文件系统变更审计流、Preflight 运行依赖预检报告与 Peer Mesh 节点状态。

---

## 2. 快速启动与配置

```bash
# 1. 前台启动 (默认 127.0.0.1:8765)
agent-steward web

# 2. 自定义监听地址、端口与管理鉴权 Token
agent-steward web --host 0.0.0.0 --port 8756 --auth-token "my-admin-token"
```

在 `~/.steward/config.toml` 中配置 `[web]` 节点后，`agent-steward daemon` 将自动拉起 Web 控制台，并通过 `EventBus` 将 `FileWatcher` 与后台巡检事件实时推流至右侧 SSE 事件栏：

```toml
[web]
enabled = true
host = "0.0.0.0"
port = 8756
# auth_token = "secret123"
```

---

## 3. RESTful API 参考规范

| 请求方法 | 路径 | 功能说明 | 鉴权要求 |
|:---:|:---|:---|:---:|
| `GET` | `/api/v1/health` | 探活与基础运行健康度检查 | 开放 |
| `GET` | `/api/v1/overview` | 宿主机硬件、事实图谱、协作锁与生态全局 KPI 概览 | 开放 |
| `GET` | `/api/v1/topology` | 宿主机全景真实事实拓扑图谱 (Nodes & Edges) | 开放 |
| `GET` | `/api/v1/topology/impact?target=<id>` | 指定实体或路径的 3 跳爆炸半径上下游子图 | 开放 |
| `GET` | `/api/v1/facts` | 监听端口、去噪进程、Systemd/Cron、工程仓库与 CLI 工具能力 | 开放 |
| `GET` | `/api/v1/locks` | 当前所有生效中的活跃协作锁矩阵 | 开放 |
| `GET` | `/api/v1/locks/wait-queue` | 协作锁等待队列 (`lock_wait_queue`) | 开放 |
| `GET` | `/api/v1/locks/deadlocks` | WFG 死锁环路检测报告 | 开放 |
| `GET` | `/api/v1/locks/history` | 最近锁生命周期与协调审计历史 | 开放 |
| `POST` | `/api/v1/locks/release` | 强制释放指定资源锁 | 可选 Bearer Token |
| `GET` | `/api/v1/ecosystem?q=<kw>` | MCP Server、SKILL 目录与外部服务通讯录清单 | 开放 |
| `GET` | `/api/v1/ecosystem/route?intent=<q>&budget=600` | Context 防爆路由器演练与 Token 节省估算 | 开放 |
| `GET` | `/api/v1/governance?q=<kw>` | 放置规则、扫描策略、长期记忆、变更审计流与 Preflight 预检 | 开放 |
| `GET` | `/api/v1/mesh/status` | 本机跨平台探针与 Peer Mesh 节点配置状态 | 开放 |
| `GET` | `/api/v1/stream` | Server-Sent Events (SSE) 实时事件推流 | 开放 |
