# Agent Host Steward 研发演进路线图 — 模块化新体系第 3 阶段 (Phase 3)

> **阶段主题**: 记忆模块可插拔体系与成熟开源适配（Pluggable Memory & Open Ecosystem）—— 生态共赢  
> **制定时间**: 2026-11-17T18:00:00+08:00  
> **制定人**: digoal 德哥 / Antigravity (项目联合创始人)  
> **执行信条**: 严格串行执行 · 状态实时记录 · 任意节点均可断点续传 · 绝不重复造轮子  
> **基线状态**: Phase 1（事实图谱）与 Phase 2（偏好规约）全量 100% 达成，单测 64 项全部 PASS

---

## 一、阶段目标与范围

回答 **"What HAPPENED"**（过往任务经验、决策历史与上下文沉淀）。
市面上已有 Mem0、Letta、Zep、PowerContext 等成熟开源记忆服务。大管家的使命是做“宿主环境底座与统一接入中枢”，绝不陷入重新造轮子的同质化竞争泥潭。

本阶段聚焦三大核心成果落地：
1. **统一 SPI 抽象 (Service Provider Interface)**：定义无歧义的通用特征 `MemoryProvider`（Search, Store, GetContext, Evict）；
2. **内置轻量 Provider**：基于 SQLite FTS5 + 本地向量混合检索，零依赖离线开箱即用；
3. **成熟开源适配器生态**：构建 Mem0、Letta、PowerContext 适配器驱动，支持在 `steward.toml` 中一行透明切换，Agent 面向大管家统一端点即可享受外部高级记忆能力。

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                 Phase 3: 记忆模块可插拔体系与成熟开源适配                   │
├─────────────────────────────────────────────────────────────────────────────┤
│  [3.1] 通用 Memory Provider SPI 抽象设计 ◄─── (当前第一步)                  │
│        • MemoryProvider Trait (search, store, get_context, evict)           │
│        • 统一 MemoryItem, MemoryFilter, SessionContext 跨生态数据契约       │
│  [3.2] 内置轻量引擎封装 (Builtin Local Provider)                           │
│        • 基于 Local SQLite FTS5 + Local Embedding 封装标准 SPI 实现         │
│        • 零外部网络依赖，单机单二进制极速运行                               │
│  [3.3] 成熟开源生态适配器 (Mem0 / Letta / PowerContext Adapters)            │
│        • Mem0 适配器 (REST 桥接)                                            │
│        • Letta / MemGPT 适配器                                              │
│        • PowerContext 适配器 (当前宿主机 8000/8001 端口直连)                 │
│        • steward.toml 中 provider = "..." 一行透明切换与热加载              │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 二、串行任务看板 (Breakpoint Tracking Board)

| 任务 ID | 子任务名称 | 核心职责与产出物 | 状态 | 开始时间 | 完成时间 |
|:---:|---|---|:---:|:---:|:---:|
| **3.1** | **Memory Provider SPI 抽象** | 构建 `src/memory/spi.rs`：定义标准 SPI Trait 与跨生态通用数据模型；覆盖单元测试 | ✅ **已完成** | 2026-11-17 18:00 | 2026-11-17 18:02 |
| **3.2** | **内置轻量引擎 SPI 封装** | 构建 `src/memory/builtin_provider.rs`：将已有 SQLite FTS5 记忆层封装接入标准 SPI；覆盖单元测试 | ✅ **已完成** | 2026-11-17 18:04 | 2026-11-17 18:07 |
| **3.3** | **成熟开源生态适配器套件** | 构建 `src/memory/adapters/` (Mem0, PowerContext, Letta)，支持配置化透明切换；覆盖单元测试 | ⬜ 待执行 | - | - |

---

## 三、断点续传规范

如果任务由于任何原因中断：
1. 查阅本文件 `src_doc/roadmap-modular-phase3.md` 与 `src_doc/progress.md`；
2. 找到首个状态为 `⏳ 进行中` 或 `⬜ 待执行` 的任务；
3. 读取对应模块的代码与测试，恢复上下文并继续串行执行；
4. 完成后立即更新任务状态为 `✅ 已完成` 并打上精确时间戳。
