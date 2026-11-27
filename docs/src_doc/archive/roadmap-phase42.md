# Agent Body — Phase 42 演进路线图：悲观 vs 乐观自适应并发控制与失败早回退（ISSUE #25）

> **编制**: Antigravity · **日期**: 2026-11-22  
> **基线版本**: `v0.4.0`（`c9b0a58`）+ Phase 1~41 全部完成（`246` 项测试 PASS、Benchmark `v3.4`、`DQ1~DQ6 100%`）  
> **对齐约束**: [README.md 规约 #8, #18, #22, #25](../../README.md) · [15-adaptive-optimistic-pessimistic-concurrency-spec.md](../design_doc/15-adaptive-optimistic-pessimistic-concurrency-spec.md) · [EVOLUTION_GUIDE.md](../../EVOLUTION_GUIDE.md) · [user_feedback.md](../../user_feedback.md)  
> **当前状态**: ✅ **Phase 42 已全量落地（`42.1 ~ 42.6` 全部 ✅ 已完成，Benchmark `v3.5`）**

---

## 一、立项背景与问题锚点

| 编号 | 现网/架构问题实证 | Phase 42 演进目标 | 关联 ISSUE |
|---|---|---|---|
| **`CON-18` 全悲观查询税与 `PostToolUse` 失败丢弃** | ① 每次回路前调 `agent-body` 虽本地仅 `~1ms`，但强迫 LLM 多跑 1 整轮推理（重发上下文 `5k~30k tok` + `1.5~4s` 延迟）；② `body_brief` 缺显式乐观直通租约；③ [src/api/hooks.rs](../../src/api/hooks.rs#L248) 与 `body_bridge.py` 在 `success=false` 时直接返回忽略，缺少碰壁自动兜底 | 落地 **AOPCC 自适应并发架构**：开工签发高置信度「⚡ 乐观直通契约」、三态动作分类器区分客户端直连 vs 服务绑定、并在 `PostToolUse(success=false)` 时 `< 2 ms` 注入宿主机事实自愈处方 | **ISSUE #25** / Phase 42 |
| **`BENCH-11` 缺少回路轮次税对比基准** | Benchmark `v3.4` 尚无“全悲观前置查询 vs 裸奔盲猜 vs AOPCC 自适应（乐观直通+碰壁秒自愈）”的整轮对话税评测负载 | 升级 Benchmark 至 **`v3.5`**，新增 `W15 (optimistic_vs_pessimistic_execution_loop)` 与效能存证规则 `PR-11` | **ISSUE #25** / Phase 42.6 |

---

## 二、Phase 42 串行原子任务拆解与断点续传检查点表

| 阶段 | 步骤 ID | 原子任务摘要 | 精确代码/文档触点 | 验收门禁 (DoD) | 状态 | 完成时间 |
|:---:|:---:|---|---|---|:---:|:---:|
| **规划立项** | **42.1** | 悲观 vs 乐观并发深度思考、规约 #8/#18/#22 改进与新增规约 #25、设计规格书 `15-*.md`、反馈总账与 Phase 42 路线图立项 | `README.md`, `docs/design_doc/15-*.md`, `docs/src_doc/roadmap-phase42.md`, `docs/src_doc/progress.md`, `docs/src_doc/product-boundaries-and-governance.md`, `EVOLUTION_GUIDE.md`, `user_feedback.md`, `src/placement/conventions.rs`, `skills/agent-body-developer/SKILL.md` | 规约 #8/#18/#22/#25 同步落地四端；`make evolution-check` & `cargo test` 全绿 | ✅ 已完成 | 2026-11-22 |
| **Phase 42<br/>AOPCC 引擎** | **42.2** | **开工乐观租约引擎 (`OptimisticLeaseEngine`) & `BriefAssembler` 升级**：① 从 `fact_nodes`/`active_processes` 动态提取高置信度服务/工具/沙盒/CodeGraph 乐观租约（含 `confidence`、`epoch_hash`、`direct_hint`、`fallback_hint`）；② 在 `body_brief` 输出精简「⚡ 乐观直通契约」区块（`<= 120 tok`），并支持服务宕机/漂移时自动降级 | `src/brief/optimistic.rs` (新), `src/brief/assembler.rs`, `src/brief/mod.rs` | `body_brief` 自动输出可用乐观租约（如 `:5432`/`:11434`/`/root/new/tmp/`），单次耗时保持 `< 15ms` | ✅ 已完成 | 2026-11-22 |
| | **42.3** | **三态并发分类器与 `PostToolUse` 碰壁自愈引擎**：① 实现 `ConcurrencyPolicyClassifier`（区分 `psql -p 5432` 客户端乐观直连 vs `uvicorn --port` 悲观端口绑定 vs 高爆炸半径/敏感路径写）；② 升级 `AgentHookEngine::post_tool_call`，新增 `error_output` 解析与 `OptimisticFailureDiagnoser`（针对 `ECONNREFUSED` / `password auth failed` / `command not found` / `EADDRINUSE` 在 `< 2ms` 内返回宿主机替代处方） | `src/api/hooks.rs`, `src/hooks/diagnoser.rs` (新), `src/hooks/mod.rs` | 乐观碰壁场景 `post_tool_call(success=false)` 准确返回可用替代端口/工具/服务状态；悲观操作 100% 维持拦截 | ✅ 已完成 | 2026-11-22 |
| | **42.4** | **客户端 Hook 桥接器 (`body_bridge.py`) 自适应升级**：① `pre_tool` 对已识别的乐观只读/客户端命令零阻塞放行；② `post_tool` 在 `run_command` 发生 `error` / 非零退出时自动透传 `error_output` 调用 `agent-body hooks post --success=false` 注入自愈提示；③ 重新生成并更新全局 `~/.gemini/config/hooks/body_bridge.py` | `src/hooks/client_gen.rs`, `src/main.rs` (`HooksCommands::Post`), `/root/.gemini/config/hooks/body_bridge.py` | 正常乐观执行零额外进程阻塞；命令失败自动回显自愈处方 | ✅ 已完成 | 2026-11-22 |
| | **42.5** | **黑白屏 1:1 同步与探针闭环联动**：① 扩展 `body_hook_pre`（输出 `concurrency_mode` 与理由）与 `body_hook_post`（支持 `error_output` 与 `recovery_prescription`）；② CLI `hooks pre/post`、Web `/api/v1/hooks/*` 与前端治理面板同步展示乐观租约清单与三态裁决；③ 在 `P1`/`P2`/`P4` 探针追踪乐观租约命中与降级 | `src/api/mcp_tools.rs`, `src/main.rs`, `src/web/parity_api.rs`, `src/web/assets/mod.rs`, `src/feedback/probes.rs` | MCP/CLI/Web 1:1 对齐；前端直观查看乐观直通租约与碰壁恢复建议 | ✅ 已完成 | 2026-11-22 |
| | **42.6** | **Benchmark `v3.5` (`W15` + `PR-11`)、文档刷新与全量验收交付**：① 新增 `W15 (optimistic_vs_pessimistic_execution_loop)` 与 `PR-11`；② 更新 `EVOLUTION_GUIDE.md`、`docs/manual/04,09,10`、`skills/`；③ 全量 `evolution-check`/`lint-panic`/`check`/`test`/`restart`、现网 `bench run` 实证与 `sync-mirror` | `src/benchmark/{mod,workloads,provenance,ledger,report}.rs`, `docs/manual/*`, `EVOLUTION_GUIDE.md`, `skills/*` | `W15` PASS（整轮对话税显著下降且质量 100%）；全测试 PASS；0 漂移 | ✅ 已完成 | 2026-11-22 |

---

## 三、断点续传与依赖指引

- **串行依赖顺序**：`42.1 (规划与规约 ✅) → 42.2 (乐观租约与简报) → 42.3 (三态分类器与碰壁自愈) → 42.4 (Bridge 脚本与全局钩子) → 42.5 (黑白屏与探针闭环) → 42.6 (Benchmark v3.5 与收尾交付)`。
- **中断恢复方法**：新会话只需读取 `EVOLUTION_GUIDE.md` 与本路线图 `docs/src_doc/roadmap-phase42.md`，定位首个 `⏳ 待执行` 任务即可零重扫续接。
