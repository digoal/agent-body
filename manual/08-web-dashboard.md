# Agent Host Steward — 官方使用手册：第 8 章 Web UI 与可视化监控大屏

> 版本: v0.3.2+  
> 技术架构: Tokio 异步内嵌 HTTP 引擎 · 原生 ESM/Canvas 拓扑渲染 · 单二进制 100% 离线自包含 · 零外部 CDN

---

## 1. 概述与核心能力

**Web UI / 可视化监控大屏** 是 Agent Host Steward 提供的基于浏览器的轻量级全局观测控制台。专为多 Agent 高并发作业、多进程微服务编排、系统依赖深层审计量身定制：

- 🌐 **宿主机事实拓扑全景图 (Host Reality Topology)**：
  - 自动将底层 `/proc` 内核进程、监听端口、Systemd 守护服务、Crontab 定时任务、代码资产与 LinksEngine 依赖图组织为标准拓扑网络；
  - 实时力导向图可视化，点击节点高亮多跳关联与爆炸半径（Blast Radius）。
- 🔒 **多 Agent 协作锁动态监控看板 (Coordination Locks Board)**：
  - 活跃排他/共享锁全局网格（Resource Type、Holder Agent、PID、Mode、租期 TTL 倒计时）；
  - 锁竞争冲突风险预警、孤儿超时死锁一键安全释放（带管理员鉴权支持）。
- ⚡ **全链路 SSE 实时事件流 (Server-Sent Events)**：
  - 浏览器建立原生长连接（`/api/v1/stream`），文件系统变更、进程上下线、锁申请与释放秒级推流推屏，无需频繁轮询。
- 📦 **工业级单二进制自包含 (Zero External Dependencies)**：
  - HTML/CSS/JS/Canvas 渲染器编译期内嵌二进制文件，无外部 npm、node_modules、CDN 依赖，隔离网络环境 100% 可用。

---

## 2. 快速启动与使用

### 2.1 方式一：独立命令行启动 (CLI)

```bash
# 1. 默认监听 127.0.0.1:8765 启动前台 Web 监控看板
agent-steward web

# 2. 自定义端口与跨主机访问绑定 (如监听 0.0.0.0 供局域网查看)
agent-steward web --host 0.0.0.0 --port 9000

# 3. 启用管理员操作鉴权 Token (保护锁强制释放接口)
agent-steward web --port 8765 --auth-token "my-admin-token"
```

启动成功后，终端将输出如下访问入口 Banner：
```
╭────────────────────────────────────────────────────────────╮
│  🌐 Agent Host Steward Web Dashboard                       │
│  URL: http://127.0.0.1:8765                                │
╰────────────────────────────────────────────────────────────╯
```
打开任意现代浏览器访问该 URL，即可直接进入深色极简工业风监控大屏。

### 2.2 方式二：常驻后台守护服务随启 (Daemon)

在 `~/.steward/config.toml` 中配置 `[web]` 节点：

```toml
[web]
# 是否随 daemon 守护服务自动拉起 Web 监控大屏
enabled = true

# 监听地址 (默认 127.0.0.1 保证安全)
host = "127.0.0.1"

# 监听端口
port = 8765

# 管理员鉴权 Token (可选)
# auth_token = "secret123"
```

启动或重启守护服务：
```bash
systemctl --user restart agent-steward
# 或前台运行守护进程:
agent-steward daemon
```
守护进程将同时托管 inotify 监听、动态进程采集与 Web 监控大屏服务。

---

## 3. 大屏核心板块功能详解

### 3.1 顶部状态栏 (Header Metrics)
- **主机信息**: 宿主机名与操作系统内核版本；
- **计算资源**: CPU 物理核心数、当前内存使用量与利用率；
- **锁状态灯**: 当前活跃的排他/共享协作锁总数，一旦产生高频排队将自动橙色高亮告警。

### 3.2 宿主机事实拓扑与依赖图谱 (Topology Panel)
- **多类型节点差异化呈现**:
  - 🔵 **进程 (Process)**: 标识 PID、程序名、完整启动命令；
  - 🟠 **端口 (Port)**: 标识当前被绑定的 TCP 端口；
  - 🟣 **服务 (Service)**: 标识管理的 systemd 服务与 crontab 任务；
  - 🟢 **核心资产 (File/Asset)**: 核心工程、公共库脚本与关键配置文件。
- **关联连边**: 展示 `listens_on`（绑定）、`depends_on`（依赖）、`impacts`（影响）等因果网络。

### 3.3 活跃协作锁看板 (Locks Panel)
- 展示由各 Agent（通过 `steward_lock_acquire` 或 Pre-Hook）获取的实时协作锁；
- 动态进度条倒计时显示剩余 TTL（秒）；
- 支持管理员输入 Token 一键强制解除失联 Agent 残留的孤儿锁。

### 3.4 实时事件流日志 (SSE Stream)
- 显示与管家后端的长连接状态灯（在线/重连中）；
- 滚动打印文件系统感知流、进程探测刷新流与多 Agent 加锁/排队日志。

---

## 4. RESTful API 参考规范

Web 引擎向外暴露一套标准 JSON API，供自定义前端集成或监控脚本调用：

| 请求方法 | 路径 | 功能说明 | 鉴权要求 |
|:---:|:---|:---|:---:|
| `GET` | `/api/v1/health` | 探活与基础运行健康度检查 | 开放 |
| `GET` | `/api/v1/overview` | 宿主机硬件利用、资产与规则全局概览 | 开放 |
| `GET` | `/api/v1/topology` | 宿主机全景事实拓扑图谱 (Nodes & Edges) | 开放 |
| `GET` | `/api/v1/topology/impact?target=<path>` | 指定资产的爆炸半径上下游子图 | 开放 |
| `GET` | `/api/v1/locks` | 当前所有生效中的活跃协作锁矩阵 | 开放 |
| `GET` | `/api/v1/locks/history` | 最近锁申请与变更历史 | 开放 |
| `POST` | `/api/v1/locks/release` | 强制释放指定资源锁 | 可选 Bearer Token |
| `GET` | `/api/v1/stream` | Server-Sent Events (SSE) 实时事件推流 | 开放 |
