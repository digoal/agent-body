# Agent Body — Phase 13: Web UI 与可视化监控大屏 (Web Dashboard & Visual Monitor)

> 创建时间: 2026-11-18T17:42:00+08:00  
> 对齐: README.md ISSUE #13（Web UI / 可视化监控大屏：提供基于浏览器的轻量级Agent 本体拓扑图与协作锁监控看板）  
> 基线: Phase 1~12 全部完成 · 140 项测试 100% PASS · 31 MCP 工具 · v0.3.2  
> 核心原则: **单二进制自包含交付 · 亚毫秒本地响应 · 工业级可观测性 · 全链路断点续传**

---

## 一、为什么做这件事（背景与价值）

### 1. 痛点分析

在 Phase 1~12 的演化过程中，Agent Body 已经构建了完备的内核级环境感知（/proc、端口、Systemd、Cron）、可插拔记忆、多 Agent 协作锁（CoordinationEngine）以及 31 个原生 MCP 工具。然而：
1. **黑盒化与心智负担**：多 Agent 并发干活时，人类主理人或调度管理员只能通过 CLI 子命令（`agent-body env list`、`agent-body memory search`）或 `tui` 终端界面查看状态，缺乏全局全景视图。
2. **依赖拓扑不直观**：宿主机上的进程-端口-服务-文件-CodeGraph 之间存在深层网状依赖，CLI 文本难以一眼看清逆向依赖爆炸半径（Blast Radius）。
3. **多 Agent 锁冲突不可见**：当多个 Agent 产生资源竞争、锁等待或排队时，目前仅能通过日志或 `lock_list` 查看静态列表，缺乏活跃租期倒计时、死锁风险预警与持锁历史流转的直观展示。

### 2. 核心架构约束与工业级要求

根据 Agent Body 的产品哲学：
- **严格单二进制交付（Single Binary Deployment）**：严禁额外安装 Node.js、Nginx、Python 或依赖外部 CDN 网络；HTML/CSS/JS 静态资产必须编译期内嵌（`include_str!` 或 `rust-embed`），离线环境 100% 独立可用。
- **轻量低开销（Zero Overhead When Idle）**：内嵌异步 HTTP 引擎必须复用已有的 Tokio 运行时，无请求时 CPU 占用趋近于 0，内存增量控制在 10MB 以内。
- **只读安全隔离（Read-Heavy & Safe Operation）**：监控看板以全景观测为主；仅在管理员明确配置授权的情况下，才允许触发受控运维动作（如超时死锁强制释放、临时文件定向 GC）。
- **断点续传与高容错**：从规划、实现到测试拆解为 6 个独立子阶段，每个子阶段都有可验证的检查点与单元测试。

---

## 二、功能规格与大屏全景架构

```
┌────────────────────────────────────────────────────────────────────────┐
│                      Browser / Web Dashboard (Client)                  │
│       [现代化深色极简控制台 · 原生 ESM/Canvas/SVG 渲染 · 零外部 CDN]       │
├───────────────────┬────────────────────┬───────────────────────────────┤
│ 1. 宿主机全景大盘 │ 2. 拓扑图与爆炸半径│ 3. 协作锁实时监控看板         │
│   • CPU/内存/磁盘 │   • 进程/端口/网络 │   • 活跃锁矩阵 (Holder/TTL)   │
│   • 记忆容量/引擎 │   • 服务/Cron 关联 │   • 竞争排队态势              │
│   • 规约与放置统计│   • 文件/仓库依赖拓扑│  • 租期倒计时与释放操作        │
└───────────────────┴─────────▲──────────┴───────────────▲───────────────┘
                              │ REST API / SSE Stream    │
┌─────────────────────────────┼──────────────────────────┼───────────────┐
│                    agent-body daemon / web server                   │
│                    (Tokio 异步内嵌轻量 HTTP 引擎)                      │
├─────────────────────────────┴──────────────────────────┴───────────────┤
│ REST Endpoints:                                                        │
│ • GET /api/v1/overview       -> 宿主规格、健康度、记忆数、放置规则统计 │
│ • GET /api/v1/topology       -> 节点(Nodes)与连边(Edges)网络拓扑图谱   │
│ • GET /api/v1/topology/impact?target=path -> 指定资产的爆炸半径拓扑    │
│ • GET /api/v1/locks          -> 当前活跃锁、排队队列、死锁检测诊断     │
│ • POST /api/v1/locks/release -> 管理员强制释放指定租期异常锁           │
│ • GET /api/v1/stream (SSE)   -> 实时事件总线推流 (锁变更/进程刷新)     │
└─────────────────────────────┬──────────────────────────────────────────┘
                              │
┌─────────────────────────────▼──────────────────────────────────────────┐
│                   Agent Body 内核核心引擎                      │
│ ┌──────────────────┐ ┌───────────────────┐ ┌─────────────────────────┐ │
│ │  Host Reality    │ │ CoordinationEngine│ │ Memory & Placement      │ │
│ │  (Proc/Port/Sys) │ │ (Advisory Lock)   │ │ (SQLite/PG/Learner)     │ │
│ └──────────────────┘ └───────────────────┘ └─────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 三、任务分解与串行执行路线图 (13.1 ~ 13.6)

### 13.1 阶段一：轻量异步 HTTP 引擎与 REST API 核心框架 (`src/web/`)

- **子任务 ID**: `13.1`
- **目标**: 在现有 Tokio 异步运行时之上构建轻量、极速的 HTTP 服务，不引入冗余依赖，提供规范的 JSON REST API 与健康探活。
- **涉及文件**:
  - `Cargo.toml`（评估轻量 HTTP 依赖，如 tiny_http / hyper / axum 等）
  - `src/web/mod.rs`（Web 模块入口、路由器与服务配置）
  - `src/web/server.rs`（HTTP 监听器、静态资源路由分发、错误处理中间件）
  - `src/web/api.rs`（RESTful JSON 序列化处理器与请求封套）
  - `src/lib.rs`（导出 `pub mod web`）
- **具体工作**:
  1. 架构选型与依赖控制：选用与 Tokio 1.37 原生契合的高性能异步 Web 基础组件，实现路由匹配与 JSON 编解码。
  2. 实现通用响应封套 `WebResponse<T>`（包含 `code`, `message`, `data`, `timestamp`）。
  3. 实现基础探活端点 `GET /api/v1/health` 与系统概览端点 `GET /api/v1/overview`。
  4. 编写 `src/web/` 模块的单元测试与模拟 HTTP 请求验证。
- **验收标准**:
  - `cargo check` / `cargo test` 编译通过；
  - 启动 HTTP 监听 `127.0.0.1:8765` 后，curl 获取 `/api/v1/health` 返回 `{"status":"ok"}`。
- **断点续传检查点**:
  - [x] 评估并配置 Web HTTP 依赖项
  - [x] 实现 `src/web/mod.rs` 与基础路由结构
  - [x] 实现 `src/web/api.rs` 概览与健康接口
  - [x] 编写测试用例 `test_web_health_and_overview`

---

### 13.2 阶段二：宿主机事实拓扑与依赖爆炸半径图谱 API

- **子任务 ID**: `13.2`
- **目标**: 将Agent 本体底层采集的进程、端口、Systemd 服务、Crontab 定时任务、Git 仓库以及 `LinksEngine` 依赖关系统一转换为适合前端力导向图/树图渲染的标准图数据模型。
- **涉及文件**:
  - `src/web/topology.rs`（拓扑数据聚合器与图谱协议转换）
  - `src/fact/` 与 `src/links/`（对齐底层数据获取接口）
- **图数据协议规格 (Graph Protocol)**:
  ```json
  {
    "nodes": [
      { "id": "proc:18493", "label": "python3 (main.py)", "type": "process", "status": "running", "metadata": { "pid": 18493, "port": 8080 } },
      { "id": "port:8080", "label": ":8080", "type": "port", "status": "bound" },
      { "id": "service:agent-body", "label": "agent-body.service", "type": "systemd", "status": "active" },
      { "id": "file:/root/new/shared_proj/test.py", "label": "test.py", "type": "file", "status": "normal" }
    ],
    "edges": [
      { "source": "proc:18493", "target": "port:8080", "relation": "listens_on" },
      { "source": "service:agent-body", "target": "proc:18493", "relation": "manages" },
      { "source": "proc:18493", "target": "file:/root/new/shared_proj/test.py", "relation": "depends_on" }
    ]
  }
  ```
- **具体工作**:
  1. 聚合 `/proc` 进程与监听端口，生成进程-网络子拓扑；
  2. 聚合 Systemd 与 Crontab，建立定时/守护依赖边；
  3. 挂接 `LinksEngine` 的文件引用关系，提供 `GET /api/v1/topology/impact?target=xxx` 爆炸半径子图提取；
  4. 编写图谱转换单元测试。
- **验收标准**:
  - `GET /api/v1/topology` 返回格式严格符合图规范；
  - 节点类型丰富（process/port/systemd/cron/git/file），关联边准确无环死循环。
- **断点续传检查点**:
  - [x] 建立图节点与边的数据结构定义
  - [x] 实现全景拓扑聚合逻辑
  - [x] 实现定向影响半径拓扑提取接口
  - [x] 编写测试用例 `test_topology_graph_generation`

---

### 13.3 阶段三：协作锁实时监控看板与事件流 API (`src/coordination/`)

- **子任务 ID**: `13.3`
- **目标**: 将 Phase 11 实现的 `CoordinationEngine` 协作锁状态全面暴露给 Web 监控端点，支持锁状态矩阵、租约倒计时、排队态势以及 SSE (Server-Sent Events) 实时推流。
- **涉及文件**:
  - `src/web/locks_api.rs`（协作锁监控 API 与管理动作）
  - `src/web/sse.rs`（SSE 实时事件流通道）
  - `src/coordination/mod.rs`（暴露锁监控查询与事件钩子）
- **具体工作**:
  1. 实现 `GET /api/v1/locks`：返回当前所有活跃锁（Key、AgentID、LockType、TTL、剩余生效时间、创建时间）；
  2. 实现 `GET /api/v1/locks/history`：返回最近 100 条锁获取、释放、超时与冲突拒绝日志；
  3. 实现安全受控的 `POST /api/v1/locks/release`：支持输入 `key` 与管理员授权标识，强制释放超时或异常孤儿锁；
  4. 实现 `GET /api/v1/stream`：基于 SSE 长连接，在锁状态变更、进程上下线时向浏览器实时推送增量变更。
- **验收标准**:
  - 并发加锁场景下，API 能够精准返回多 Agent 持锁状态与剩余 TTL；
  - SSE 客户端建立长连接后能持续收到心跳与更新事件；
  - 单元测试覆盖锁列表查询与安全释放逻辑。
- **断点续传检查点**:
  - [x] 扩展 CoordinationEngine 监控与历史查询接口
  - [x] 实现 `GET /api/v1/locks` 与 `POST /api/v1/locks/release`
  - [x] 实现 SSE 实时事件广播引擎
  - [x] 编写测试用例 `test_locks_api_and_sse_stream`

---

### 13.4 阶段四：自包含单页面大屏与前端交互引擎 (`src/web/assets/`)

- **子任务 ID**: `13.4`
- **目标**: 打造专业、科技感、响应式且完全自包含的 Web 监控看板单页面（HTML + CSS + 原生 JS/SVG/Canvas），通过 `include_str!` 嵌入二进制，零网络外部依赖。
- **涉及文件**:
  - `src/web/assets/index.html`（大屏骨架与语义化 DOM 布局）
  - `src/web/assets/app.css`（深色现代极简工业风主题）
  - `src/web/assets/app.js`（数据获取、SSE 连接、拓扑渲染器、锁看板联动）
  - `src/web/assets/mod.rs`（静态资源打包与 MIME 类型响应封装）
- **核心功能分区**:
  1. **顶部导航状态栏**：宿主机名、IP、系统负载、Agent 本体运行时间、SSE 连接健康状态灯（绿色常亮/重连呼吸灯）；
  2. **面板 A：事实图谱拓扑可视化区**：
     - 支持缩放、平移、节点点击聚焦；
     - 差异化颜色与图标区分进程、端口、Systemd 服务与代码资产；
     - 选中节点高亮关联上下游依赖与多跳爆炸半径；
  3. **面板 B：协作锁态势监控看板**：
     - 活跃锁卡片网格，动态进度条展示 TTL 剩余时间；
     - 锁竞争告警徽标与等待队列展示；
     - 提供一键快速复制 Key 与安全释放弹窗；
  4. **面板 C：Agent 本体核心指标速览**：
     - 记忆库条目计数（FTS5 / Vector）；
     - 资产放置规则命中与纠偏学习沉淀统计；
     - 最近变更审计流水实时滚动。
- **验收标准**:
  - 静态资源零外部 CDN 引用（无外部 font、无外部 script 标签）；
  - 页面加载时间 < 50ms；
  - 在无网络环境下离线打开无任何 404 或脚本崩溃。
- **断点续传检查点**:
  - [x] 设计并编写 `index.html`、`app.css`、`app.js`
  - [x] 实现轻量内置 Canvas/SVG 力导向拓扑渲染引擎
  - [x] 静态资源编译期嵌入 `include_str!` 并完成 MIME 响应封装
  - [x] 编写测试用例 `test_embedded_assets_served`

---

### 13.5 阶段五：CLI 命令、Daemon 常驻集成与配置联动

- **子任务 ID**: `13.5`
- **目标**: 将 Web 监控服务融入Agent 本体的统一命令行体系和后台守护进程，支持配置文件开箱即用与独立子命令快速唤起。
- **涉及文件**:
  - `src/main.rs`（新增 `agent-body web` CLI 子命令与参数解析）
  - `src/config/mod.rs` 与 `config.example.toml`（新增 `[web]` 配置节）
  - `src/daemon/mod.rs`（守护进程启动时按需拉起内嵌 Web 服务）
- **配置项规范 (`config.toml`)**:
  ```toml
  [web]
  # 是否随 daemon 自动启动 Web 监控大屏
  enabled = true
  # 监听地址 (默认 127.0.0.1 保证安全，0.0.0.0 允许局域网访问)
  host = "127.0.0.1"
  # 监听端口
  port = 8765
  # 管理员运维操作鉴权 Token (留空则仅允许只读监控)
  auth_token = ""
  ```
- **CLI 实操命令**:
  ```bash
  # 1. 独立运行 Web 监控服务（前台运行，并在终端输出访问 URL）
  agent-body web --host 127.0.0.1 --port 8765

  # 2. 检查当前 Web 服务的运行状态与端口
  agent-body web status
  ```
- **具体工作**:
  1. 完善 Clap 命令行结构，新增 `web` 子命令，支持 `--port`、`--host`、`--open`（自动调用浏览器打开）；
  2. 端口防撞检测：启动前复用Agent 本体自身的 `body_port_check` 校验端口占用；
  3. 在 `daemon` 主循环中集成 Web Server 的后台 Tokio 协程任务。
- **验收标准**:
  - 执行 `agent-body web --port 8765` 正常启动并在终端打印漂亮 Banner；
  - 端口被占用时给出友好的错误提示与建议端口。
- **断点续传检查点**:
  - [x] 扩展 `src/config/` 支持 `[web]` 配置
  - [x] 实现 `agent-body web` CLI 子命令
  - [x] 接入 daemon 常驻后台集成
  - [x] 编写 CLI 参数解析单元测试

---

### 13.6 阶段六：工业级测试验证、官方手册与全量文档同步

- **子任务 ID**: `13.6`
- **目标**: 贯彻工业级交付标准，补齐全链路端到端集成测试，更新使用手册、架构文档与断点续传追踪表。
- **涉及文件**:
  - `tests/test_web_dashboard.rs`（端到端集成测试）
  - `docs/manual/08-web-dashboard.md`（新增官方手册第 8 章：Web 可视化大屏使用指南）
  - `docs/manual/03-cli-reference.md`（补充 `agent-body web` 命令行参考）
  - `docs/src_doc/progress.md`（更新进度表并归档 Phase 13）
  - `README.md`（更新项目架构图、功能列表与 ISSUE #13 状态）
- **具体工作**:
  1. 编写端到端测试用例：启动 Web 服务 -> 发送 HTTP 请求查询拓扑 -> 发送模拟 Agent 加锁 -> 验证 Web 看板锁数据更新 -> 安全释放锁；
  2. 撰写《第 8 章：Web 可视化大屏使用指南》，涵盖单机监控、多 Agent 协作排障、拓扑穿透与 Docker 端口映射说明；
  3. 更新 `progress.md` 与 `README.md`。
- **验收标准**:
  - `cargo test --all` 全部 140+ 项测试 100% 通过；
  - 文档齐全无死链，手册包含清晰实操截图示意或 ASCII 交互流。
- **断点续传检查点**:
  - [x] 编写端到端集成测试 `tests/test_web_dashboard.rs`
  - [x] 撰写 `docs/manual/08-web-dashboard.md`
  - [x] 更新 `docs/manual/03-cli-reference.md`
  - [x] 更新 `docs/src_doc/progress.md` 与 `README.md`

---

## 四、执行原则与断点恢复指南

1. **严格顺序推进**：按 `13.1 -> 13.2 -> 13.3 -> 13.4 -> 13.5 -> 13.6` 顺序串行执行，杜绝跨阶段大跃进。
2. **每步有产物、步步有验证**：每个子阶段完成时，必须确保 `cargo check` 和对应测试 100% 绿色通过，并在 `progress.md` 中勾选对应检查点。
3. **断点恢复流程**：任何时候若遇到外部中断或异常退出：
   - 第一步：检查 `docs/src_doc/progress.md` 中的最新勾选状态；
   - 第二步：运行 `cargo test` 确认已有测试状态；
   - 第三步：直接从第一个未完成的子任务检查点继续无缝往下执行。
