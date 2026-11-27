# Agent Body — Phase 44~45 演进路线图：Agent 本体升维、跨大脑连续性与全物料同步（ISSUE #27）

> **编制**: Antigravity · **日期**: 2026-11-22 (Asia/Shanghai)  
> **基线版本**: `v0.4.0` + Phase 1~43 全部完成（26 模块 · 42 MCP 工具 · 78 SPI 方法 · 26 张表 · Benchmark `v3.6` · `DQ1~DQ7 100%`）  
> **对齐约束**: [README.md 规约 #1, #2, #3, #8, #11, #13, #17, #27](../../README.md) · [04-agent-body-philosophy.md](../idea_doc/04-agent-body-philosophy.md) · [17-agent-embodiment-and-continuity-spec.md](../design_doc/17-agent-embodiment-and-continuity-spec.md) · [EVOLUTION_GUIDE.md](../../EVOLUTION_GUIDE.md) · [user_feedback.md](../../user_feedback.md)  
> **当前状态**: ✅ **Phase 44~45 已全量完成（`44.1 ~ 45.3` 全部 ✅，Benchmark `v3.7` 全绿交付）**

---

## 一、立项背景与问题锚点

| 编号 | 现网/定位问题实证 | Phase 44~45 演进目标 | 关联 ISSUE |
|---|---|---|---|
| **`REQ-18` 「Agent 本体」定位失配与本体原生能力缺口** | ① 外部受众易将「Agent 本体」误读为多 Agent 编排调度器、将「Agent 本体」误读为面向人类的个人助理，导致核心定位（为 Agent 服务的宿主机底座）失焦且路走窄；② 既有系统已具备感官、反射、海马体、审计与联邦原型，但缺少统一的 **身体身份证 (`BodyManifest`)**、**跨大脑连续性接力胶囊 (`ContinuityCapsule`)**、**五维稳态体检与 L1 自愈 (`HomeostasisVitals`)**、**显式四级自治包络 (`AutonomyEnvelope`)** 与 **多体名片发现**。 | 按推荐 **方案 B（定位升维 + 三层命名零破坏兼容 + 五大本体原语增量演化）**：<br/>① Phase 44 新建第 27 模块 `src/body/` 与第 27 张表 `body_checkpoints`，贯通 `43/43` 黑白屏同步；<br/>② Phase 45 按三层兼容纪律全面刷新注释、文档、`SKILL`、Web 路演剧场第 7 幕，并为 ISSUE #28/#29 铺平零返工前置。 | **ISSUE #27** / Phase 44.1~45.3 |
| **`BENCH-13` 跨大脑连续性接力与本体稳态基准缺失** | Benchmark `v3.6` 已覆盖 `W1~W16` 与 `DQ1~DQ7`，但尚无针对「跨模型/跨客户端换脑接力省轮次与零泄露」及「五维本体生命体征 + `L0~L3` 自治权限防越权」的实证回归门禁。 | 升级 Benchmark 至 **`v3.7`**，新增 **`W17 (cross_brain_continuity_and_body_proprioception)`**、**`DQ8 (body_homeostasis_and_envelope_hygiene)`** 与 **`PR-13`**，守卫帕累托双门禁与零明文泄露。 | **ISSUE #27** / Phase 44.6 |

---

## 二、Phase 44~45 串行原子任务拆解与断点续传检查点表

### 阶段 A：Phase 44 — Agent 本体内核、五维稳态与 Benchmark `v3.7`

| 阶段 | 步骤 ID | 原子任务摘要 | 精确代码/文档触点 | 验收门禁 (DoD) | 状态 | 完成时间 |
|:---:|:---:|---|---|---|:---:|:---:|
| **规划立项** | **44.1** | **ISSUE #27 深度思考、规约体系审计改进与四端同步、Phase 44~45 路线图立项**：① 撰写 `docs/idea_doc/04-agent-body-philosophy.md`（脑体分离模型、方案对比）；② 撰写 `docs/design_doc/17-agent-embodiment-and-continuity-spec.md`（三层兼容契约、五大本体原语、Bench `v3.7`）；③ 改进 `README.md` 规约 #1/#2/#3/#8/#11/#13/#17 并新增规约 #27，同步更新 `product-boundaries-and-governance.md`、`src/placement/conventions.rs`、`skills/agent-body-developer/SKILL.md`、`EVOLUTION_GUIDE.md`、`progress.md`、`ISSUE.md`、`user_feedback.md` | `docs/idea_doc/04-*.md` (新), `docs/design_doc/17-*.md` (新), `docs/src_doc/roadmap-phase44-45.md` (新), `README.md`, `docs/src_doc/product-boundaries-and-governance.md`, `src/placement/conventions.rs`, `skills/agent-body-developer/SKILL.md`, `EVOLUTION_GUIDE.md`, `docs/src_doc/progress.md`, `ISSUE.md`, `user_feedback.md` | `make evolution-check` (`0 drift`)、`make lint-panic`、`cargo test placement::conventions` 与 `cargo check --tests` 全绿 | ✅ 已完成 | 2026-11-22 |
| **Phase 44<br/>本体内核** | **44.2** | **第 27 模块 `src/body/` 与第 27 张表 `body_checkpoints`（Storage SPI + 三端 Schema）**：① 在 SQLite/PG/DuckDB Schema 与 `migration.rs` 新增 `body_checkpoints` 表与索引；② 扩展 `StorageProvider` Trait 4 个接力胶囊 SPI 方法（`insert_body_checkpoint`、`get_latest_active_body_checkpoint`、`list_body_checkpoints`、`mark_body_checkpoint_resumed`）及三端实现；③ 新建 `src/body/{mod,manifest,continuity,vitals,envelope}.rs` 与核心单测 | `src/storage/{schema,pg_schema,duckdb_schema}.sql`, `src/storage/{models,spi,mod,migration}.rs`, `src/storage/{sqlite,pg,duckdb}_provider.rs`, `src/body/*.rs` (新), `src/lib.rs` | `body_checkpoints` 三后端契约与迁移测试全绿；`BodyManifest`/`AutonomyEnvelope`/`ContinuityCapsule` 单测通过且零 Panic | ✅ 已完成 | 2026-11-22 |
| | **44.3** | **跨大脑连续性接力贯通 `BriefAssembler` & `SessionLifecycleManager`**：① `ContinuityManager` 保存胶囊时强制经 `SecretRedactor` + `PromptInjectionScanner` 清洗、限额 `<=300 tok` 并写入 `IN_AGENT` 审计；② `BriefAssembler` 支持自动注入最近激活胶囊摘要（`<=150 tok`）并将简报头标升维为「Agent 本体开工简报」；③ `SessionManager::close_session` 支持可选生成/归档接力胶囊 | `src/body/continuity.rs`, `src/brief/assembler.rs`, `src/session/mod.rs`, `src/audit/mod.rs` | 模拟 Antigravity 保存胶囊 → Claude Code 调用 `BriefAssembler` 自动命中接力目标与文件；敏感令牌 100% 脱敏；简报组装仍 `< 15 ms` | ✅ 已完成 | 2026-11-22 |
| | **44.4** | **五维稳态自愈与多本体联邦名片发现**：① `HomeostasisVitalsEngine` 聚合 `DQ1~DQ7`、`P1~P5`、24h AOPCC 有效率与磁盘水位输出五维雷达分，并绑定 `L1_AUTONOMIC` 反射自愈（接入 `BackgroundScheduler`）；② 在 `src/mesh/` 新增 `/api/v1/mesh/body_manifest` 只读端点与客户端探测，接入 `MeshEnvelope v1.0` HMAC + `read:facts` 鉴权 | `src/body/vitals.rs`, `src/daemon/scheduler.rs`, `src/feedback/probes.rs`, `src/mesh/mod.rs`, `src/web/server.rs` | `vitals` 低分触发 `L1_AUTONOMIC` 自愈且绝不越权执行 `L2/L3`；Mesh 多体名片交换与防重放测试全绿 | ✅ 已完成 | 2026-11-22 |
| | **44.5** | **黑白屏 `43/43` 1:1:1 双向同步**：① 在 `src/api/mcp_tools.rs` 新增第 43 工具 `body_state`（6 个子动作：`manifest/vitals/envelope/handoff_save/handoff_resume/peers`）；② 在 `src/main.rs` 新增 1:1 `agent-body body` 子命令族；③ 在 `src/web/{server,parity_api,assets/mod}.rs` 新增 `/api/v1/body/*` 路由（写端点强接 `check_auth`）与「🧬 本体中枢与生命体征」双主题工作台、更新顶栏品牌与 `43/43` Parity 矩阵 | `src/api/mcp_tools.rs`, `src/main.rs`, `src/web/{server,parity_api,assets/mod}.rs`, `tests/test_web_dashboard.rs` | Parity 校验 `43/43 (100%)`；MCP stdio、CLI 与 Web 集成测试全绿；未鉴权写请求返回 `401` | ✅ 已完成 | 2026-11-22 |
| | **44.6** | **Benchmark `v3.7` (`W17` + `DQ8` + `PR-13`) 与阶段验收**：① 在 `src/benchmark/` 新增 `W17`（跨大脑接力帕累托评测）、`DQ8`（稳态健康与自治包络合规门禁）与 `PR-13`；② 更新 `scripts/evolution_manifest.py`、`EVOLUTION_GUIDE.md` MANIFEST 计数与 `docs/design_doc/10-benchmark-spec.md`；③ 跑通全量 `cargo test`、`build --release`、现网 `bench run` 与守护服务热重启 | `src/benchmark/{mod,workloads,dq,provenance,ledger,report}.rs`, `tests/benchmarks.rs`, `scripts/evolution_manifest.py`, `EVOLUTION_GUIDE.md`, `docs/design_doc/10-benchmark-spec.md` | `W17` & `DQ8` PASS（必含事实 100% 命中、`+ΔQ > 0`、零泄露）；全库单测与 Release 冒烟全绿 | ✅ 已完成 | 2026-11-22 |

### 阶段 B：Phase 45 — 全物料本体化刷新与路演升级（ISSUE #27 收口 & 衔接 #28/#29）

| 阶段 | 步骤 ID | 原子任务摘要 | 精确代码/文档触点 | 验收门禁 (DoD) | 状态 | 完成时间 |
|:---:|:---:|---|---|---|:---:|:---:|
| **Phase 45<br/>物料升维** | **45.1** | **代码注释、CLI Help、MCP 描述与术语门禁同步**：① 遵循 Tier-3 零破坏红线，刷新 `Cargo.toml` description、`src/main.rs` Help、`src/api/mcp_tools.rs` 工具描述与核心模块注释为「Agent 本体」主叙事（保留 `body_*`/`~/.agent-body/` 兼容说明）；② 在 `scripts/evolution_manifest.py` 增加 `Tier-1/2` 误导表述轻量检查 | `Cargo.toml`, `src/main.rs`, `src/api/mcp_tools.rs`, `src/*` 顶部模块注释, `scripts/evolution_manifest.py` | `make evolution-check`、`make lint-panic`、`make check` 全绿；43 个 MCP 工具名与 CLI 子命令保持 100% 兼容 | ✅ 已完成 | 2026-11-22 |
| | **45.2** | **全量核心文档与 3 套 `SKILL` 刷新 + 镜像同步**：① 全面刷新 `README.md`、`EVOLUTION_GUIDE.md`、`INSTALL.md`、`docs/arch/01~03`、`docs/manual/01~10`、`docs/design_doc/05,06,10`；② 为 `docs/idea_doc/00,01,03` 增补衔接导语说明“比较优势/Agent 本体→Agent 本体”历史演进脉络；③ 刷新项目内 3 套 `skills/` 并同步至全局 `/root/.gemini/antigravity-cli/skills/` 与 `/root/new/shared_git/agent-body/` | `README.md`, `EVOLUTION_GUIDE.md`, `INSTALL.md`, `docs/arch/*`, `docs/manual/*`, `docs/idea_doc/00,01,03`, `docs/design_doc/05,06,10`, `skills/*`, `make sync-mirror` | 活跃文档与 3 处 SKILL 镜像 100% 一致；`make evolution-check` `0 drift`；镜像目录 `rsync` 零差异 | ✅ 已完成 | 2026-11-22 |
| | **45.3** | **Web 路演剧场升维（第 7 幕「换脑不失忆」）与 ISSUE #28/#29 交接**：① 在 `src/demo/mod.rs` 与 Web 路演剧场加入「脑体分离 · Agent 本体」顶层导览，并实装 **第 7 幕黄金剧目**：`cross_brain_handoff`（真引擎演示 Antigravity 保存接力胶囊 → Claude Code/Codex 1ms 接盘不重扫）；② 输出面向 Phase 46（ISSUE #28）的 HTML 体系化 30 分钟专题拆解清单；③ 全量回归与守护重启 | `src/demo/mod.rs`, `src/web/assets/mod.rs`, `docs/src_doc/progress.md`, `ISSUE.md`, `user_feedback.md` | 7 大路演剧目 `demo:*` 真引擎实跑全绿并一键无痕复位；全量测试、`build --release` 与现网守护服务全部健康运行 | ✅ 已完成 | 2026-11-22 |

---

## 三、前瞻协同路线（ISSUE #28 → #29 零返工衔接）与断点恢复指引

1. **为什么按 `Phase 44 (本体内核) → Phase 45 (物料刷新) → Phase 46 (ISSUE #28 体系化 HTML 演说) → Phase 47 (ISSUE #29 目录规整 + 全仓路径扫描 + CodeGraph 刷新)` 串行？**
   - 若先执行 `#28` 重做全套 PPT，紧接着实施 `#27` 的本体升维与新能力，PPT 会立刻过时并被迫二次重写；
   - 若先执行 `#29` 移动根目录文档并刷新全仓路径，随后在 `#27/#28` 中新生成的文档/PPT 又会引入新路径引用；
   - 因此按 **`#27 → #28 → #29`** 顺序推进可实现 **100% 零返工**，并在 Phase 47 末统一执行全仓路径刷新与 `.codegraph/` 重建收官。
2. **面向 Phase 46 (ISSUE #28) 的「6 讲 × 30 分钟体系化专题 HTML 演说」拆解蓝图（每讲单点说透，统一地层剖面视觉规范）**：
   - **前置迁入与终刷新**：新建 `presentation/`，将 `agent-body-deck.html` 与 `presentation.html` `git mv` 移入并刷新至 Agent 本体 / Bench `v3.7` / 43 工具现状；
   - **第 1 讲**：*脑体分离与 Agent 本体哲学*（为什么 LLM 需要宿主机身体 + 比较优势 + 递弱代偿防膨胀）；
   - **第 2 讲**：*宿主机物理感官图谱与零污染扫盘*（L1/L2/L3 属性图、`/proc`/Socket/Systemd/Cron、白黑灰分级与 CodeGraph 宏微联邦）；
   - **第 3 讲**：*小脑反射与自适应并发控制 AOPCC*（开工乐观直通租约、Pre-Hook 痛觉红线、`<2ms` 碰壁自愈与 `L0~L3` 权限包络）；
   - **第 4 讲**：*海马体、信息新鲜度与跨大脑接力*（Memory SPI、写穿失效、`FreshnessReconciler` 与 `<=300 tok` 换脑接力胶囊）；
   - **第 5 讲**：*纵深安全、双域审计与探针驱动自进化*（`SecretRedactor`、`PromptInjectionScanner`、`IN/OUT_OF_AGENT`、`P1~P5`、五维稳态自愈与 `MeshEnvelope v1.0`）；
   - **第 6 讲**：*三轨实证 Benchmark v3.7 与可复现工程体系*（帕累托双门禁 $Q$、`COMP-C1~C4` 竞品同跑、`PR-01~13` 账本与 DuckDB OLAP）。
3. **中断恢复指引**：
   - 新会话只需按零重扫开工协议读取 `EVOLUTION_GUIDE.md`、本文件 `docs/src_doc/roadmap-phase44-45.md` 与 `ISSUE.md #28/#29`，可直接进入 **Phase 46 (`ISSUE #28`)**。
