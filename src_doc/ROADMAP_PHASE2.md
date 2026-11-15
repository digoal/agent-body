# Agent Host Steward (Rust 版) — 第二阶段 Coding 任务与断点续传表

> 规划时间: 2026-11-15T13:42:00+08:00  
> 目标: 从“CLI 原型”跨越至“标准常驻服务 (Daemon + inotify 监听 + 原生 MCP 协议端点)”  
> 当前状态: 规划完成，待按需串行启动

---

## 阶段规划与子任务清单

| 阶段 | 任务 ID | 任务内容 | 涉及模块与文件 | 预期产出 / 验收标准 | 状态 |
|---|---|---|---|---|---|
| **第一阶段: MCP 协议实现** | M-1 | 设计并实现轻量 JSON-RPC 2.0 stdio 协议处理框架 | `src/api/mcp.rs`, `src/api/mod.rs` | 支持标准 initialize、tools/list、tools/call 格式 | ✅ 已完成 (2026-11-15 13:51) |
| | M-2 | 注册 steward 核心工具至 MCP (brief, place, asset, env, memory) | `src/api/mcp_tools.rs` | 完整暴露参数 schema 与返回值 | ✅ 已完成 (2026-11-15 13:52) |
| | M-3 | 增加 CLI 子命令 `steward mcp` 并编写端到端单元测试 | `src/main.rs`, `tests/test_mcp.rs` | Agent 客户端可通过 stdio 直接通信 | ✅ 已完成 (2026-11-15 13:54) |
| **第二阶段: Watcher 与 Daemon 服务** | D-1 | 引入 `notify` crate 实现异步文件系统实时监听器 | `src/watcher/mod.rs` | 支持防抖 300ms，识别 create/modify/delete | ✅ 已完成 (2026-11-15 13:56) |
| | D-2 | 实现增量事件处理流水线（联动 scanner 增量更新与 changes 记录） | `src/watcher/handler.rs` | 文件变更后自动同步 assets 与 FTS5，记录 changes 表 | ✅ 已完成 (2026-11-15 13:57) |
| | D-3 | 实现常驻后台 `steward daemon` 主循环与 Scheduler 定时器 | `src/daemon/mod.rs`, `src/main.rs` | 后台轮询进程与磁盘，配合 watcher 形成完整守护进程 | ✅ 已完成 (2026-11-15 13:58) |
| **第三阶段: 扫描器与匹配增强** | S-1 | 升级 `src/scanner/` 通配符引擎为标准 `globset` / `glob` | `src/scanner/matcher.rs` | 完美支持 `**/*.py`, `!**/target/**` 等复杂规则 | ✅ 已完成 (2026-11-15 14:08) |
| | S-2 | 实现大目录扫描的批处理与高效存储刷盘优化 | `src/scanner/mod.rs` | 支持批量 200 条批量事务提交，显著降低单次写入 I/O 耗时 | ✅ 已完成 (2026-11-15 14:08) |
| **第四阶段: 关联与变更追踪引擎** | L-1 | 实现 `src/links/` 关联关系发现引擎与图查询接口 | `src/links/mod.rs` | 支持代码与脚本依赖建立、记忆与资产双向关联、Mermaid 图表导出 | ✅ 已完成 (2026-11-15 14:09) |
| | L-2 | 实现 `src/changes/` 变更摘要生成与会话回溯 (steward changes since) | `src/changes/mod.rs` | Agent 新会话可查看“上次开工以来变了什么” | ✅ 已完成 (2026-11-15 14:12) |

---

## 断点恢复机制
启动第二阶段 Coding 任务时：
1. 读取本文件 `src_doc/ROADMAP_PHASE2.md`；
2. 找到首个状态为 `⏳ 进行中` 或 `⬜ 待执行` 的任务；
3. 保持串行步进：**更新状态为进行中 -> 编写代码与单元测试 -> cargo test 验证 -> 更新状态为已完成**。
