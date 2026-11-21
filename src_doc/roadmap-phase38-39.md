# Agent Host Steward — Phase 38 ~ 39 演进路线图：全生命周期信息新鲜度对账（ISSUE #21）与内生对比探针闭环自进化（ISSUE #22）

> **编制**: Antigravity · **日期**: 2026-11-21  
> **基线版本**: `v0.4.0`（`39fd028`）+ Phase 1~37 全部完成（Benchmark `v3.2`、现网健康分 `100/100`、`DQ1~DQ4` 100%）  
> **对齐约束**: [README.md 规约 #9, #13, #14, #21, #22](../README.md) · [13-freshness-and-self-evolution-spec.md](../design_doc/13-freshness-and-self-evolution-spec.md) · [EVOLUTION_GUIDE.md](../EVOLUTION_GUIDE.md) · [user_feedback.md](../user_feedback.md)  
> **当前状态**: ✅ Phase 38 ~ 39 已全部完成并经真机实测验收 (`Benchmark v3.3` PASS · 五点对比探针 `5/5 ✅`)

---

## 一、立项背景与现网第一性原理实证诊断

对现网 `/root/.steward/steward.db` 开展物理状态与库内快照比对实测，暴露出以下真实新鲜度与自进化断层：

| 诊断项 | 现网实测数字 | 根因定位 | 归属 |
|---|---|---|:---:|
| **`CON-12` 资产改后向量/描述未失效** | **`34` 个** `active` 资产磁盘 `st_size` 已漂移，`embedding` 仍保留旧向量 | `merge_with_existing_asset` 未标记 `embedding_status="stale"`，`upsert_asset` 未重置 `embedding = '[]'` | **Phase 38** |
| **`CON-13` 删后僵尸召回与幽灵端口** | **`19` 条** `status='missing'` 仍被 FTS 召回；`fact_nodes` 残留 **`84` 个 `port:*`**（70+ 幽灵）与 **`1` 个孤儿 `file:`** | `search_assets_fts` 未过滤 `status='active'`；无 SPI `delete_fact_node` 与图谱清理 | **Phase 38** |
| **`CON-14` `auto_sync` 记忆重复霸屏** | `19` 条活跃 `auto_sync` 记忆中 `user_feedback.md` 重复 **6+ 次** | `AutoSyncEngine` 插入摘要记忆前未归档同路径旧记忆，且未刷新 `LinksEngine` | **Phase 38** |
| **`CON-15` 缺少执行回路对比探针与自动反馈** | 执行回路无 `P1~P5` 对比探针，也无 `FeedbackEngine::add_item_deduped` | 无法用定量差值驱动自动对账、Benchmark 回归与防重复入账 | **Phase 39** |
| **`BENCH-09` 缺少新鲜度负载与质量门禁** | `W1~W11` 与 `DQ1~DQ4` 均未检验文件改删、向量失效与幽灵节点清理 | 升级 Benchmark `v3.3`，新增 `W12` 与 `DQ5` | **Phase 39** |

---

## 二、Phase 38 ~ 39 串行原子任务拆解与断点续传检查点表

| 阶段 | 步骤 ID | 任务摘要 | 涉及文件 | 验收门禁 (DoD) | 状态 | 完成时间 |
|:---:|:---:|---|---|---|:---:|:---:|
| **Phase 38** | **38.1** | 规约体系升级、设计规格书 `13-*.md` 与反馈/路线图立项 | `README.md`, `design_doc/13-*.md`, `src_doc/roadmap-phase38-39.md`, `user_feedback.md`, `EVOLUTION_GUIDE.md` | 规约 #9/#13/#14 优化，#21/#22 新增，文档齐全 | ✅ 已完成 | 2026-11-21 |
| | **38.2** | 写穿失效：`mtime_secs` 采集 + `merge_with_existing_asset` `stale` 标记 + 三后端 `upsert_asset`/`search_assets_fts` 过滤 `active` + SPI `delete_fact_node` | `src/scanner/metadata.rs`, `src/storage/{spi,mod,sqlite_provider,pg_provider,duckdb_provider}.rs` | 修改重扫重置 `'[]'` 向量，删除资产 FTS 0 召回 | ✅ 已完成 | 2026-11-21 |
| | **38.3** | `AutoSyncEngine` 同路径记忆幂等归档、`ChangeHandler` 删除级联清理与 `PropertyGraphOrganizer` 幽灵节点剪枝 | `src/hooks/auto_sync.rs`, `src/watcher/handler.rs`, `src/fact/property_graph.rs` | 单文件多次修改只留 1 条活跃摘要；图谱无幽灵端口 | ✅ 已完成 | 2026-11-21 |
| **Phase 39** | **39.1** | 实现 `FeedbackEngine::add_item_deduped`、`FreshnessReconciler` 现实对账器与 `P1~P5` 对比探针引擎 | `src/feedback/mod.rs`, `src/feedback/probes.rs` | `reconcile` 清理漂移/僵尸/重复记忆；5 探针输出数字 $\Delta$ | ✅ 已完成 | 2026-11-21 |
| | **39.2** | 接入 `BackgroundScheduler`、MCP `steward_feedback`、CLI `feedback reconcile/probes` 与 Web REST API (41/41 Parity) | `src/daemon/scheduler.rs`, `src/api/mcp_tools.rs`, `src/main.rs`, `src/web/parity_api.rs` | 黑白屏 1:1 对齐，守护巡检自动对账 | ✅ 已完成 | 2026-11-21 |
| | **39.3** | Benchmark `v3.3`：新增 `W12 (freshness_and_staleness_healing)` 与 `DQ5 (freshness_and_reconciliation_health)` 并同步 `EVOLUTION_GUIDE.md` | `src/benchmark/{mod,workloads,dq,report}.rs`, `EVOLUTION_GUIDE.md`, `design_doc/10-*.md`, `manual/10-*.md` | `W12` 与 `DQ5` PASS，`make evolution-check` 0 漂移 | ✅ 已完成 | 2026-11-21 |
| **收尾验收** | **39.4** | 全量门禁 (`lint-panic`/`check`/`test`/`restart`)、现网 `reconcile`/`probes`/`backfill_embeddings`、`v3.3` 实跑存证与镜像同步 | 全系统 + `scripts/backfill_embeddings.py` + `/root/.steward/steward.db` | 全测试 PASS (`237/237`)、探针 `5/5 ✅`、`DQ1~DQ5 100%`、镜像零差异 | ✅ 已完成 | 2026-11-21 |
