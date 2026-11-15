# Agent Host Steward (Rust 版) — 第三阶段 (v2.0) 体验跃升 Coding 任务与断点续传表

> 规划时间: 2026-11-15T14:52:00+08:00  
> 目标: 在稳定版 v0.2.0 基础上，实施 v2.0 核心体验升级（混合检索、端口防撞、变更爆炸半径推演、会话完工结算）  
> 当前状态: 规划完成，待按需串行启动

---

## 阶段规划与子任务清单

| 阶段 | 任务 ID | 任务内容 | 涉及模块与文件 | 预期产出 / 验收标准 | 状态 |
|---|---|---|---|---|---|
| **第一阶段: 端口防撞预警与进程健康** | P3-1 | 实现本地端口监听探测与占用进程识别 (`steward_env_port_check`) | `src/env/port.rs`, `src/env/mod.rs` | 毫秒级探测端口占用 PID，并推荐 3 个连续可用空闲端口 | ⬜ 待执行 |
| | P3-2 | 注册 `steward_port_check` MCP 工具并在 CLI 中暴露 `steward env port` | `src/api/mcp_tools.rs`, `src/main.rs` | 支持 Agent 在起服务前主动防撞 | ⬜ 待执行 |
| **第二阶段: 关联拓扑逆向爆炸半径** | P3-3 | 基于 `links` 表实现逆向依赖推演与多跳影响面分析 (`steward_links_impact`) | `src/links/impact.rs`, `src/links/mod.rs` | 输入资产路径，递归返回直接依赖与间接下游，标明风险等级 | ⬜ 待执行 |
| | P3-4 | 注册 `steward_links_impact` MCP 工具并在 CLI 中暴露 `steward links impact` | `src/api/mcp_tools.rs`, `src/main.rs` | 支持 Agent 动刀前推演上下游影响 | ⬜ 待执行 |
| **第三阶段: 语义与全文混合检索** | P3-5 | 设计本地 Ollama Embedding 异步客户端与 RRF 排序融合算法 | `src/memory/hybrid.rs`, `src/memory/mod.rs` | 结合 FTS5 关键词与余弦向量相似度计算 RRF 得分 | ⬜ 待执行 |
| | P3-6 | 升级 `steward_memory_search` 支持 hybrid 模式 | `src/api/mcp_tools.rs`, `src/main.rs` | 具备近义概念召回能力，优雅降级（无 Ollama 时自动退化为 FTS5） | ⬜ 待执行 |
| **第四阶段: 会话生命周期与环境治理** | P3-7 | 实现会话完工结算引擎与临时目录 TTL 清理 (`steward_session_close`) | `src/session/mod.rs`, `src/changes/mod.rs` | 自动汇总会话环境变更入库记忆，安全清理临时资产与孤儿进程 | ⬜ 待执行 |
| | P3-8 | 注册 `steward_session_close` MCP 工具并暴露 CLI | `src/api/mcp_tools.rs`, `src/main.rs` | 实现 Agent 会话闭环与环境自洁 | ⬜ 待执行 |

---

## 断点恢复机制
1. 读取本文件 `src_doc/ROADMAP_PHASE3.md`；
2. 找到首个状态为 `⏳ 进行中` 或 `⬜ 待执行` 的任务；
3. 严格遵守 `skills/agent-steward-developer/SKILL.md` 规则串行步进：**更新状态为进行中 -> 编写代码与单元测试 -> cargo test 验证 -> 更新状态为已完成**。
