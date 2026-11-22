# Agent Host Steward — 代码实现与设计文档全面 Review 及 v2.0 演进规划报告

> 评估时间: 2026-11-15T14:52:00+08:00  
> 评估人: digoal 德哥 / Antigravity  
> 目标: 对齐当前 Rust 核心实现与现有设计文档（01~07），盘点当前实现完备度，梳理体验痛点与能力断层，制定下一阶段（v2.0）设计文档与功能体验升级规划。

---

## 一、当前代码实现与设计文档对照评估矩阵

目前 Rust 生产代码已完成 **v0.2.0**（22 项全量自动化测试，`release` 构建通过）：

| 设计域 (Design Domain) | 对应设计文档 | Rust 实现状态与模块 | 实现完备度 | 关键差异 / 待增强点 |
|---|---|---|---|---|
| **Placement 规范放置域** | `01-scenarios.md` (S-27)<br>`03-feature-spec.md` (F-01~04)<br>`07-data-model.md` | `src/placement/mod.rs`<br>`src/storage/schema.sql` (`placement_rules`) | **95%** (高度完备) | 规则匹配、8大内置目录、命名规范、安全建议完备。待增强：**动态学习能力**（从用户多次放置行为中提取新规则）。 |
| **Scanner 分级扫描域** | `01-scenarios.md` (S-28)<br>`03-feature-spec.md`<br>`07-data-model.md` (`scan_policies`) | `src/scanner/mod.rs`<br>`src/scanner/matcher.rs` | **90%** (高度完备) | 白名单深度到底、黑名单浅层阻断、globset 通配符、批量事务提交已实现。待增强：**Git ignore 级联识别**与 **智能分类启发式（ML/模型辅助）**。 |
| **Env 宿主环境与动态进程** | `01-scenarios.md` (S-29)<br>`03-feature-spec.md`<br>`07-data-model.md` (`active_processes`) | `src/env/mod.rs`<br>`src/storage/` (`active_processes`) | **85%** (完备可用) | 硬件指标、sysinfo 进程过滤底噪、PID/端口/资源采集入库已实现。待增强：**端口冲突预警（Port Conflict Warning）** 与 **僵尸进程/孤儿进程诊断**。 |
| **Memory 长期记忆域** | `01-scenarios.md`<br>`07-data-model.md` (`memories`) | `src/memory/mod.rs`<br>`src/storage/` (FTS5) | **80%** (基础完备) | 记忆分类、权重、FTS5 全文搜索、结构化标签已实现。待增强：**本地 Ollama 向量检索（sqlite-vec）** 与 **记忆有效性淘汰/过期衰减机制**。 |
| **Daemon & Watcher 守护域** | `05-architecture.md`<br>`docs/src_doc/hooks-and-integration.md` | `src/daemon/mod.rs`<br>`src/watcher/` | **90%** (完备可用) | inotify 监听、300ms 防抖、changes 审计流、60s `/proc` 轮询巡检、优雅关闭已实现。待增强：**systemd service 单元一键托管生成**。 |
| **Links & Graph 关联域** | `07-data-model.md` (`links`) | `src/links/mod.rs` | **85%** (完备可用) | 脚本路径引用与服务端口依赖自动提取、Mermaid 拓扑图导出已实现。待增强：**多跳图查询（Graph Traversal）** 与 **逆向影响分析（Blast Radius）**。 |
| **Changes & Context 追踪域** | `07-data-model.md` (`changes`) | `src/changes/mod.rs` | **90%** (完备可用) | `changes since` 会话开工简报生成已实现。待增强：**会话隔离标签（Session Tagging）** 与 **重大变更事件主动推送 Webhook**。 |
| **MCP 原生协议与交互** | `06-api-spec.md`<br>`docs/src_doc/hooks-and-integration.md` | `src/api/mcp.rs`<br>`src/api/mcp_tools.rs` | **95%** (高度完备) | 13 个原生 MCP 工具注册，支持 Claude/Cursor/AGY 客户端接入。待增强：**交互式建议确认（Prompt/Approval Hooks）**。 |

---

## 二、当前版本体验痛点与能力断层诊断

站在 Agent 实际使用与运维视角，深入排查发现以下 **4 大关键体验痛点**：

1. **痛点 1: 向量与语义混合检索缺失（目前仅有 SQLite FTS5 全文搜索）**
   - *现象*: Agent 在回忆“上次关于数据库备份的讨论”时，如果关键词没有精确命中（例如记成了“数据容灾存档”），FTS5 会返回 0 条记录。
   - *需求*: 需要无缝结合本地已运行的 Ollama（如 `nomic-embed-text` 或 `bge-m3`），实现 **FTS5 关键词检索 + 向量语义检索 + Reciprocal Rank Fusion (RRF) 混合重排**。
2. **痛点 2: 端口与进程防撞预警（Port Conflict & Resource Guard）**
   - *现象*: Agent 启动一个新服务（如临时起一个 FastAPI 或微服务）之前，虽然能调用 `steward_env_processes`，但需要 Agent 自身肉眼排查。
   - *需求*: 提供针对指定端口（如 8080）或进程名称的 **前置防撞预警 API (`steward_env_port_check`)**，给出占用 PID、命令、已运行时间及释放建议。
3. **痛点 3: 影响面与爆炸半径分析（Blast Radius Analysis）**
   - *现象*: Agent 计划修改或删除 `/root/new/shared_proj/common_utils.py` 时，不知道宿主机上有哪些脚本或服务依赖它。
   - *需求*: 利用 `links` 引擎计算**逆向影响树**，Agent 动刀前调用 `steward_links_impact --target ...`，立刻得出“3 个生产脚本与 1 个 cron 依赖此文件，需慎重修改”。
4. **痛点 4: 会话生命周期绑定与自动归档（Session Lifecycle Hook）**
   - *现象*: Agent 会话结束时，往往在 `/root/new/tmp/` 遗留临时脚本或后台挂着测试进程。
   - *需求*: 增加 `steward session end` 钩子，自动清理与该会话关联的临时资产，提示孤儿进程，并生成该会话的《工单环境变更总结》。

---

## 三、v2.0 设计文档迭代规划建议

基于上述诊断，建议对 `docs/design_doc/` 进行版本升级（v1.0 -> v2.0），并更新以下具体设计文档：

1. **升级 `docs/design_doc/01-scenarios.md`**:
   - 新增 **S-30 (语义+全文混合检索场景)**：跨词表同义概念召回。
   - 新增 **S-31 (端口与资源防撞预警场景)**：启动前探测占用并给出规避端口。
   - 新增 **S-32 (资产变更爆炸半径分析场景)**：修改核心库前推演上下游依赖。
   - 新增 **S-33 (会话生命周期闭环清理场景)**：开工检出、完工归档与垃圾回收。
2. **升级 `docs/design_doc/02-requirements.md`**:
   - 增加功能性需求：
     - `FA-14`: 向量嵌入与混合检索（Hybrid Search: BM25/FTS5 + Vector + RRF）。
     - `FE-13`: 端口占用实时诊断与可用端口推荐。
     - `FL-04`: 实体依赖拓扑的逆向依赖与影响面推演。
     - `FS-05`: 会话标签（Session ID）与临时资产自动 TTL 归档。
3. **升级 `docs/design_doc/03-feature-spec.md`**:
   - 细化混合检索（RRF 算法权重与阈值配置）。
   - 细化 MCP 工具扩展规格（`steward_port_check`, `steward_links_impact`, `steward_session_close`）。
4. **升级 `docs/design_doc/06-api-spec.md` & `07-data-model.md`**:
   - 更新 `assets` 与 `memories` 向量表结构设计及与 Ollama 通信协议。
   - 扩展 `links` 递归 CTE 图遍历查询规范。

---

## 四、实施建议与下一步推荐动作

建议按以下顺序推进：
1. **第一步（文档迭代先行）**: 串行更新 `docs/design_doc/01-scenarios.md`、`02-requirements.md`、`03-feature-spec.md`、`06-api-spec.md`、`07-data-model.md`，固化 v2.0 规格标准；
2. **第二步（断点续传排期）**: 在 `docs/src_doc/roadmap-phase3.md` 中立项第三阶段开发任务（混合检索、端口防撞、影响面推演、会话清理）；
3. **第三步（串行编码实施）**: 遵循 `skills/agent-steward-developer/SKILL.md` 指引，TDD 推进代码实现。
