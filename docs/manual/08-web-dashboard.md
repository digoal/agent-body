# Agent Body — 官方使用手册：第 8 章 Web UI 2.0 全景交互指挥舱

> 版本: v1.1.0 (Web UI 2.0 · Phase 64 · Agent 本体中枢版)  
> 技术架构: Tokio 异步内嵌 HTTP 引擎 (`parity_api/*` + `assets/{index.html,style.css,js/*.js}`) · 原生 Canvas 力导向交互拓扑 · 9 大工作台 · 白天工程蓝图 / 夜间深空双主题 · 单二进制 100% 离线自包含 · 零外部 CDN

---

## 1. 概述与九大核心工作台

**Web UI 2.0** 是 **Agent Body（Agent 本体）** 提供的轻量级浏览器全景指挥舱，所有 HTML/CSS/JS/Canvas 渲染引擎均编译内嵌于单二进制中，零外部 CDN/Node.js 依赖：

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
   - 集中展示 `body_brief` 自适应简报组装（含 AOPCC 乐观直通租约）、8 大放置规则推导与自演化、跨会话记忆（写入/RRF 混合检索/衰减与矛盾审计）、DuckDB 变更热力图/趋势/导出、宿主 GC/会话结清 `dry-run` 预览、`MeshEnvelope v1.0` 联邦图谱/对话/沙盒收件箱与 **`43/43`** 黑白屏同步矩阵。
6. **🎬 路演演示剧场 (Roadshow Live Demo Theater · Phase 22 / Phase 25 / Phase 45 升级)**：
   - 顶部提供「脑体分离 · Agent 本体 —— 比较优势的终极工程形态」导览与宿主机商业价值 ROI 实时账单（Context Token 节省率、工具/资产/图谱规模、`43/43` 对齐认证）；
   - 采用 **三层渐进式故事舞台**：第 1 层展示每幕「💡 一句话生活比喻 (`analogy`)」「🎯 核心用意 (`intent_summary`)」、3 张核心收益数字卡 (`hero_metrics`) 与演讲者 30 秒提词卡；第 2 层左右分屏对比「盲人 Agent 三步翻车故事线」vs「7 套真引擎驱动的专属可视化组件」（含第 7 幕「换脑不失忆 · 跨大脑 1ms 本体接力」）；第 3 层提供默认收起的 `<details>` 技术专家原始 JSON 核验抽屉；
   - 支持 8 秒/幕自动巡航（`Space` 暂停 / `←→` 切幕）、演讲者大屏模式与一键无痕重置 `demo:*` 沙盒；
   - 左侧内置 `/deck`（[presentation/agent-body-deck.html](../../presentation/agent-body-deck.html)，25 页深度技术交流版）与 `/presentation`（[presentation/presentation.html](../../presentation/presentation.html)，17 页对外路演版）离线 PPT 直达入口（Decks-as-Code 生成，全套 6 讲 30 分钟专题见 [presentation/index.html](../../presentation/index.html)），页脚内置全局「⌨️ 黑屏命令镜像器」实时回显等价 CLI 与 MCP JSON-RPC。
7. **📊 实证基准、竞品对标与自进化探针 (Empirical Benchmark v3.9 & Feedback · Phase 27~64)**：
   - **轨道 A — A/B 对照基准跑批**：支持一键运行 `full` / `competitors` / `macro` / `micro` / `dq` / 单项负载（`W1`~`W19`，含 `W19` 多模态与 Know-How 回填），同机实时比对 Without Agent Body 裸奔基线 vs With Agent Body 真引擎的 Token 节省率、延迟加速比、四维上下文质量净胜（$\Delta Q$）、有效信息密度倍数、6 大微基准 SLA（`M1`~`M6`）与 10 项质量及 Shadow A/B 稳态门禁（`DQ1`~`DQ10`），每批生成 **SHA-256 防篡改指纹**；
   - **轨道 B — 生产效能存证账本**：按 `24h` / `7d` / `30d` / `all` 时间窗展示日常每一次 MCP/CLI/Web/Hook 调用自动记账的 `PR-01`~`PR-14` 归因收益与溯源流水；
   - **轨道 C — 热门竞品对标 & 使用反馈/自进化面板 (`body_feedback`)**：同屏展示 `COMP-C1~C4` 竞品对比与 12 维源码能力矩阵，并直接驱动 `user_feedback.md` 列表、五点对比探针（`P1~P5`）、信息新鲜度对账（`reconcile`）与已验证反馈沉淀入记忆（`sync`），与 43 个原生 MCP 工具及 CLI 保持 **43/43 (100%)** 双向同步。
8. **📜 双域行为审计与自进化工作台 (Dual-Domain Audit & Self-Evolution · Phase 43)**：
   - 支持按 `IN_AGENT` / `OUT_OF_AGENT` 双域过滤审计流水、`trace_id` 因果链路钻取、DuckDB OLAP 聚合视图、一键触发审计自进化（AOPCC 租约滑窗升降级 + Placement/Feedback 反哺）与 Parquet/CSV 导出，同步回显等价 `agent-body audit` CLI 与 `body_audit_query` MCP 请求体。
9. **🧬 本体中枢与稳态工作台 (Agent Embodiment Center · Phase 44~64)**：
   - 展示确定性 `body_id`、六大生理器官实时自省、五维生命体征雷达与一键 `L1_AUTONOMIC` 稳态自愈、`L0~L3` 四级自治权限包络表、跨大脑 `ContinuityCapsule (<=300 tok)` 接力保存/恢复表单、内生 Wait Events (18/18) + `/proc/self` + AWR 快照黄金基线差分/报告库（含 `delete_snapshot` / `delete_awr_report`）、`ADDM` `L1` 自调优、多本体 `/api/v1/mesh/body_manifest` 联邦名片视图，以及 **Phase 58/63 三态运行模式（`bypass`/`shadow_ab`/`body_only`）、MCP Schema 减税档位（`core`/`dev`/`ops`/`full`）与 Shadow A/B 证伪自愈（`H1~H5`）交互控制台**；
   - 拓扑/资产雷达同步支持按 `modality`、`asset_type`、`kv_filter`（如 `schema_version=2.0`）、`fuzzy`、`include_links` 过滤 Omni-RRF 召回，并支持一键触发 `--extract-knowhow` / `--backfill-multimodal` 扫描。

### 1.1 ☀️ 白天工程蓝图（默认） / 🌙 夜间深空双色调切换 (Phase 25+)
- 顶栏右侧提供 `#btn-theme-toggle` 一键昼夜主题切换（非输入焦点时可按快捷键 **`T`** 瞬间切换，状态自动持久化至 `localStorage`）；
- **白天模式（默认）**采用暖纸工程蓝图色系（`#F6F4EF` 暖纸底 + 纯白 `#FFFFFF` 卡片 + `#1F5D6E` 深蓝图青 + `#C25E32` 暖陶橙 + `#21735C` 松林绿），久看护眼；**夜间模式**采用深空指挥舱高对比暗色系；切换瞬间 Canvas 拓扑图节点描边、文字底衬、连边与选中外环自动同步重绘；
- 端口防撞、Pre-Hook、放置推导、DuckDB 热力图与趋势、宿主 GC 等交互结果默认展示可视化摘要卡片与进度条，原始 JSON 收纳在可折叠 `<details>` 抽屉中。

---

## 2. 快速启动与配置

```bash
# 1. 前台启动 (默认 127.0.0.1:8765)
agent-body web

# 2. 自定义监听地址、端口与管理鉴权 Token
agent-body web --host 0.0.0.0 --port 8765 --auth-token "my-admin-token"
```

在 `~/.agent-body/config.toml` 中配置 `[web]` 节点后，`agent-body daemon` 将自动拉起 Web 控制台，并通过 `EventBus` 将 `FileWatcher` 与后台巡检事件实时推流至右侧 SSE 事件栏：

```toml
[web]
enabled = true
host = "0.0.0.0"
port = 8765
# auth_token = "secret123"
```

---

## 3. RESTful API 参考规范

| 请求方法 | 路径 | 功能说明 | 鉴权要求 |
|:---:|:---|:---|:---:|
| `GET` | `/api/v1/health` | 探活与基础运行健康度检查 | 开放 |
| `GET` | `/api/v1/overview` | 宿主机硬件、事实图谱、协作锁与生态全局 KPI 概览 | 开放 |
| `GET` | `/api/v1/topology` | 宿主机全景真实事实拓扑图谱 (Nodes & Edges，含 `L1/L2/L3`) | 开放 |
| `GET` | `/api/v1/topology/impact?target=<id>` | 指定实体或路径的 3 跳加权衰减爆炸半径上下游子图 | 开放 |
| `GET` | `/api/v1/facts` | 监听端口、去噪进程、Systemd/Cron、工程仓库与 CLI 工具能力 | 开放 |
| `GET` | `/api/v1/locks` | 当前所有生效中的活跃协作锁矩阵 | 开放 |
| `GET` | `/api/v1/locks/wait-queue` | 协作锁等待队列 (`lock_wait_queue`) | 开放 |
| `GET` | `/api/v1/locks/deadlocks` | WFG 死锁环路检测报告 | 开放 |
| `GET` | `/api/v1/locks/history` | 最近锁生命周期与协调审计历史 | 开放 |
| `POST` | `/api/v1/locks/release` | 强制释放指定资源锁 | Bearer Token |
| `GET` | `/api/v1/ecosystem?q=<kw>` | MCP Server、SKILL 目录与外部服务通讯录清单 | 开放 |
| `GET` | `/api/v1/ecosystem/route?intent=<q>&budget=600` | Context 防爆路由器演练与 Token 节省估算 | 开放 |
| `GET` | `/api/v1/governance?q=<kw>` | 放置规则、扫描策略、长期记忆、变更审计流与 Preflight 预检 | 开放 |
| `GET` | `/api/v1/mesh/status` / `/api/v1/mesh/query` | 本机跨平台探针、Peer Mesh 状态与联邦动作查询 | 开放 |
| `GET` | `/api/v1/mesh/ping` / `facts` / `locks` / `graph` / `body_manifest` | 节点间图谱、事实与本体名片联邦端点（带 `MeshEnvelope` 校验） | Token / HMAC |
| `POST` | `/api/v1/mesh/dialogue` / `transfer` | `MeshEnvelope v1.0` HMAC 签名防重放对话与 `<=10MB` 沙盒投递 | HMAC-SHA256 |
| `GET` | `/api/v1/parity/matrix` | 黑白屏 43 工具 100% 映射对齐自检矩阵 | 开放 |
| `GET` | `/api/v1/audit/events` / `summary` / `trace/:id` / `olap` | 双域审计流水查询、时间窗统计、因果链钻取与 DuckDB OLAP 聚合 | 开放 |
| `POST` | `/api/v1/audit/evolve` / `export` | 手动触发审计驱动自进化飞轮与 Parquet/CSV 安全导出 | Bearer Token |
| `GET` | `/api/v1/body/manifest` / `vitals` / `envelope` / `checkpoints` / `peers` / `perf/awr` | 本体六器官自省、五维稳态分、L0~L3 权限矩阵、胶囊历史、对等节点名片与 AWR 报告 | 开放 |
| `GET` | `/api/v1/body/runtime-mode` / `ab-report?window=7d` | 读取当前三态运行模式（`mode`/`mcp_profile`/`shadow_primary`）与 Shadow A/B 证伪/自愈统计 | 开放 |
| `POST` | `/api/v1/body/runtime-mode` / `ab-compare` | 切换三态运行模式与 MCP 减税档位；实时跑一次 Shadow A/B 对比并返回 `H1~H5` 自愈计数 | Bearer Token |
| `GET` | `/api/v1/assets/search?q=<kw>&kv_filter=...` | 全信号 Omni-RRF 检索纳管资产（支持 `modality`/`asset_type`/`kv_filter`/`graph_hops`/`fuzzy`/`include_links`） | 开放 |
| `POST` | `/api/v1/assets/scan` | 触发指定目录白黑名单扫描（支持 `extract_knowhow`/`backfill_multimodal`/`enrich_with_ollama`） | Bearer Token |
| `POST` | `/api/v1/body/handoff` / `resume` / `heal` / `perf/snapshot` / `perf/awr` / `perf/tune` / `perf/delete` | 保存/恢复接力胶囊、`L1` 自愈、抓取/对比 AWR 快照、ADDM 调优与删除报告/快照 | Bearer Token |
| `GET` | `/api/v1/demo/scenarios` / `/api/v1/demo/roi` | 7 大路演剧目列表与实时 ROI 商业价值核算 | 开放 |
| `POST` | `/api/v1/demo/run` / `/api/v1/demo/reset` | 真引擎实跑指定路演剧目与一键重置 `demo:*` 沙盒 | Bearer Token |
| `GET` | `/api/v1/benchmark/workloads` | 获取 19 大标准 A/B 负载 (`W1~W19`) 与 `PR-01~14` 规则字典 | 开放 |
| `POST` | `/api/v1/benchmark/run` | 真机运行 `full` / `competitors` / `macro` / `micro` / `dq` / `W1~W19` 基准 | Bearer Token |
| `GET` | `/api/v1/benchmark/report` | 查询指定或最近批次报告及历史列表（含 SHA-256 指纹） | 开放 |
| `GET` | `/api/v1/benchmark/ledger` | 审计指定时间窗内的生产效能存证账本流水 | 开放 |
| `GET` | `/api/v1/benchmark/compare` | 对比两个批次（`run_a` vs `run_b`）的回归差异 | 开放 |
| `GET` | `/api/v1/benchmark/competitors` | 运行或查看 `COMP-C1~C4` 热门开源竞品横向对标与 12 维矩阵 | 开放 |
| `POST` | `/api/v1/benchmark/reset` | 清理 `bench:*` 隔离沙盒记录 | Bearer Token |
| `GET` | `/api/v1/feedback` | 列出 `user_feedback.md` 实际使用反馈总账条目 | 开放 |
| `POST` | `/api/v1/feedback` | 触发反馈追加 (`add`)、同步入记忆 (`sync`)、对账 (`reconcile`) 与 `P1~P5` 探针 (`probes`) | Bearer Token |
| `GET` | `/deck` / `/presentation` | 内嵌离线版路演与架构演示 PPT | 开放 |
| `GET` | `/api/v1/stream` | Server-Sent Events (SSE) 实时事件推流 | 开放 |

> 💡 **安全说明**：当非回环暴露（`host != 127.0.0.1`）或配置了 `[web].auth_token` 时，所有 `POST` 写端点及 `/api/v1/env/gc` 等敏感接口均统一受 `Authorization: Bearer <token>` 保护。
