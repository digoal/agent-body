# Agent Body Phase 76 ~ 78 演进路线图 (`ISSUE #55` — PostgreSQL SPI 事务级海马体：方案 A → 方案 B)

> **定位**：基于 `~/new/agent_body_postgresql_spi_a_b.html` 思路与 [22-pg-tx-memory-spi-a-to-b-spec.md](../design_doc/22-pg-tx-memory-spi-a-to-b-spec.md) 架构设计，严守「脑体分离 × 递弱代偿 × 微内核/周边分离」红线，按 **M1（Phase 76 方案 A 事务底座）→ M2（Phase 77 空闲期睡眠整理）→ M3（Phase 78 方案 B 快慢权重旁路闭环）** 三阶段串行推进。前序阶段硬验收门禁 100% 全绿前，严禁抢跑进入下一阶段。  
> **实施路线决策**：采用 **路线 3（契约下沉 + `contrib/mem_pg_tx/` 独立五件套 + 旗舰无侵入集成 + SQLite 透明降级）**，保持 43 个原生 MCP 工具零膨胀。  
> **前置实证条件**：本机已就绪 PostgreSQL 18（`/usr/lib/postgresql/18/bin/postgres`）及 `btree_gist` / `vector` / `pg_vectorsearch` / `pg_trgm` 扩展，M1 起即可全程真机实跑验证。

---

## Phase 76（M1 · 方案 A 事务底座：`PgTxMemoryProvider`、双时态 MVCC 与解耦写穿）`✅ 已完成`

> **阶段目标**：解决多 Agent 并发写入丢写（Lost Update）、堆表向量写放大与删除后僵尸召回，打通 `tx_epoch` 逻辑纪元与 SQLite 透明降级通道。  
> **真机 PG 18.6 硬验收实测**：16 线程 × 5 并发写同一热点 `fact_key`（80 tx，耗时 `1124.5 ms`）丢写率 **`0%`** · OCC 惊群重试 **`0` 次** · `valid_range` 重叠 **`0`** · 200 事实解耦段 `search_tx` **`P50 = 3.28 ms` / `P99 = 4.34 ms`**（远优于 `< 20 ms`） · 删改后僵尸召回率 **`0%`** · SQLite 透明降级与老库 `body_checkpoints.tx_epoch` 无损迁移全绿 · 43 原生 MCP 工具零膨胀 · `W20` + 全栈门禁全绿。

| 任务编号 | 原子任务清单 | 核心代码 / 文档触点 | 验收标准 | 状态 |
|:---:|---|---|---|:---:|
| **76.1** | **非破坏性事务记忆 SPI 与配置扩展**：在 `src/memory/spi.rs` 增量定义 `TxMemoryEvent` / `TxMemorySnapshot` / `TxWriteOutcome` 与可选 `TxMemoryProvider` Trait；在 `src/config/` 与 `config.example.toml` 扩展 `[memory]`（`provider = "pg_tx"`、`pg_url`、`pg_pool_size`、`vector_backend`、`fallback_to_sqlite`） | `src/memory/spi.rs` · `src/config/` · `config.example.toml` | 既有 4 方法与全部现有配置 100% 向后兼容；参数校验单测通过 | ✅ 已完成 |
| **76.2** | **构建标准五件套 `contrib/mem_pg_tx/` 与四表兼容 DDL**：创建独立扩展包（`Cargo.toml` + `mem_pg_tx.control.toml` + `sql/mem_pg_tx--0.1.0.sql` + `body_module_magic!()` + `tests/regress.rs`），一次性落盘前后向 100% 兼容的 4 表（`ext_mem_pg_tx__events` / `segments` / `fast_weight_snapshots` / `compaction_jobs` 及兼容视图）与活跃唯一索引 | `contrib/mem_pg_tx/` · `Cargo.toml` | `make arch-check` 0 违规（`kernel/` 0 PG 依赖与 0 业务词污染）；PG 18 建表幂等通过 | ✅ 已完成 |
| **76.3** | **连接池化与双时态 MVCC 写穿失效引擎**：在 `contrib/mem_pg_tx/` 实现零新依赖有界连接池、同键 `pg_advisory_xact_lock` 亚毫秒提交期微序列化 + `READ COMMITTED FOR UPDATE` 原子写穿（截断旧 `valid_range` → 标记旧段 `is_deleted=true` → 分配单调 `tx_epoch`）+ `REPEATABLE READ READ ONLY` 快照读 + OCC `40001`/`23505` 兜底重试 | `contrib/mem_pg_tx/src/` | PG 18 实测 16 并发线程压测热点 `fact_key` 丢写率 `0%`、`valid_range` 0 重叠 | ✅ 已完成 |
| **76.4** | **LSM 式解耦检索段与 SQLite 透明降级通道**：实现 `ext_mem_pg_tx__segments`（`tsvector` GIN + 可插拔 `embedding_ref` 适配 `none`/`pgvector`/`pg_vectorsearch`/`external_lsm` + 墓碑过滤）；在 `MemoryFactory` 接入 `"pg_tx"`，PG 不可达自动降级 `builtin` SQLite 并记 `OUT_OF_AGENT` 审计，补齐 SQLite ⇄ PG 双向热同步 | `contrib/mem_pg_tx/src/` · `src/memory/adapters/pg_tx.rs` · `src/memory/adapters/mod.rs` | 删/改后旧向量召回 `0` 条；断网/停 PG 时零崩溃无缝回退 SQLite；严控模式显式报错 | ✅ 已完成 |
| **76.5** | **`tx_epoch` 贯通 AOPCC、`FreshnessReconciler` 与跨脑胶囊**：将单调 `tx_epoch` 注入 `OptimisticLeaseEngine`、`OptimisticFailureDiagnoser` 模式 5 (`MVCC_EPOCH_CONFLICT` `<2ms`)、`FreshnessReconciler` 写穿墓碑及 `ContinuityCapsule`（`body_checkpoints` 经 `schema_reconcile.rs` 无损补 `tx_epoch` 列） | `src/brief/optimistic.rs` · `src/hooks/diagnoser.rs` · `src/feedback/probes.rs` · `src/body/continuity.rs` · `src/storage/sqlite_provider/schema_reconcile.rs` | 老库无损补列回归通过；`ContinuityCapsule` `<= 300 tok` 含 `tx_epoch` 与抗注入校验 | ✅ 已完成 |
| **76.6** | **Action-level 1:1:1 透出（43 工具零膨胀）**：在既有 `body_memory_search` / `body_memory_add` / `body_memory_audit` 与 CLI / Web API 增加双时态参数（`fact_key`/`evidence_tier`/`expected_epoch`/`as_of_epoch`/`include_history`/`tx_status`/`tombstone_fact_key`） | `src/api/mcp_schemas.rs` · `src/api/mcp_tools/` · `src/cli_subcommands.rs` · `src/cli_handlers/core.rs` · `src/web/parity_api/ops_endpoints.rs` | 43/43 Parity 契约与 `tests/pg_tx_memory_integration.rs` / `mcp_all_tools_e2e` / `web_full_e2e` 全绿 | ✅ 已完成 |
| **76.7** | **M1 真机压测、Benchmark `W20` 与全门禁收口**：新增 `tests/pg_tx_memory_integration.rs` PG 18 实机压测与 Benchmark `W20` 负载，验证全量验收指标并更新 `EVOLUTION_GUIDE.md` / `README.md` / `progress.md` / 手册 | `tests/pg_tx_memory_integration.rs` · `src/benchmark/workloads/knowledge.rs` · `docs/` · `EVOLUTION_GUIDE.md` | 丢写率 `0%` · P99 `4.34 ms` (<20ms) · 写穿 `100%` · 全工作区测试与 `verify-webui` 全绿 | ✅ 已完成 |

---

## Phase 77（M2 · 方案 A 满血形态：`L1_AUTONOMIC` 空闲期睡眠整理 `SleepTimeCompactor`） `✅ 已完成`

> **阶段目标**：在不影响热路径 `< 15 ms` SLA 的前提下，利用宿主机空闲窗口异步整理长情景记忆与计算 STC 显著性评分，实现在线推理上下文 Token 降本 `3×–5×` 且事实召回率 `100%`。  
> **前置条件**：Phase 76（M1）全部任务 `✅ 已完成` 且门禁全绿。  
> **硬验收门禁**：在线推理 Token 降本 **`3×–5×`** · 帕累托双门禁 **`Fact Recall = 100%`** 且 **`Q_body >= Q_baseline`** · 不可预测查询自动旁路 0 退化 · 全量写动作 100% 入 `audit_events`。  
> **真机 PG 18.6 硬验收实测**：`SleepTimeCompactor` 压缩比 **`3.73×`**（`272→73 tok`） · 保留原子事实 **`6/6 (100%)`** · 整理期间热路径 `search_tx` **`P50 = 5.19 ms` / `P99 = 10.65 ms`**（远优于 `< 20 ms` SLA） · 不可预测探索查询自动旁路 `QueryPredictabilityGate` 召回 0 退化 · SQLite `memories.status='archived'` 兼容降级、中断作业恢复与暂停开关、`OUT_OF_AGENT` 脱敏审计、Benchmark `v4.0` (`W21` `260→87 tok` / `DQ11` / `PR-15`) 及 `428` 项全工作区测试 + Playwright 9 工作台 E2E 全绿。

| 任务编号 | 原子任务清单 | 核心代码 / 文档触点 | 验收标准 | 状态 |
|:---:|---|---|---|:---:|
| **77.1** | **STC 突触显著性评分引擎 (`SalienceScorer`)**：综合新颖度/惊喜度、跨会话复用频次、人工/探针冲突修正权重与时间衰减，为 `ext_mem_pg_tx__events` 计算并持久化 `salience_score` | `contrib/mem_pg_tx/src/salience.rs` · `src/memory/lifecycle.rs` | 确定性单测覆盖高价值决策 vs 高频低值噪声显著性分层 | ✅ 已完成 |
| **77.2** | **空闲窗口探测与 `SleepTimeCompactor`**：在 `BackgroundScheduler`（`L1_AUTONOMIC` 包络）挂载空闲期整理器，按 CPU/活跃会话空闲阈值消费 `compaction_jobs (sleep_context)`，压缩冗余 `auto_sync` 与长情景片段为高密度摘要并标记 `consolidation_st='sleep_compacted'` | `src/daemon/scheduler.rs` · `contrib/mem_pg_tx/src/sleep_compactor.rs` · `src/memory/sleep_compaction.rs` | 整理期间热路径 `P99 = 10.65 ms`；支持 `dry_run`、`pause/resume` 与中断恢复 | ✅ 已完成 |
| **77.3** | **查询可预测性评估与自动旁路门禁**：引入查询分布熵/命中率检测（`QueryPredictabilityGate`），遇到高度不可预测的探索型查询自动旁路预整理结果、直查原始双时态事件流，规避 Sleep-time Compute 负面干扰边界 | `contrib/mem_pg_tx/src/predictability.rs` · `src/memory/adapters/pg_tx.rs` · `src/memory/hybrid.rs` · `src/brief/assembler.rs` | 不可预测查询测试集准确率无下降（`ΔRecall >= 0`） | ✅ 已完成 |
| **77.4** | **双域审计留痕与三端观测控制**：每次睡眠整理强制经 `SecretRedactor` 脱敏并以 `OUT_OF_AGENT` 写入 `audit_events`（携带 `audit_trace_id` 与 `before_after_delta`）；在 `body_memory_audit` / CLI / Web (`GET`+`POST /api/v1/memories/audit`) 暴露 `sleep_action`/`force` | `src/audit/` · `src/api/mcp_tools/` · `src/cli_handlers/core.rs` · `src/web/` | `audit_events` 100% 可回溯；明文凭据残留 `0`；43 工具零膨胀 | ✅ 已完成 |
| **77.5** | **Benchmark `v4.0` 升级 (`W21` + `DQ11` + `PR-15`)**：新增睡眠整理帕累托负载 `W21`、双时态一致性与墓碑纯净度 `DQ11`、睡眠省 Token 存证规则 `PR-15`，同步 `evolution_manifest.py` | `src/benchmark/` · `scripts/evolution_manifest.py` · `docs/design_doc/10-benchmark-spec.md` | `W21` 省 Token (`260→87 tok`) 且 `Fact Recall = 100%`、`Q_body >= Q_baseline` | ✅ 已完成 |
| **77.6** | **M2 全栈验收与文档/手册同步**：跑通 `make quality-gate && RUST_TEST_THREADS=1 make test && make verify-webui`，更新手册 `03`/`04`/`06`/`08`/`10` 与 `progress.md` | `docs/manual/` · `docs/src_doc/progress.md` · `tests/sleep_compaction_integration.rs` | 0 drift / 0 warning / 428 测试与全部门禁通过 | ✅ 已完成 |

---

## Phase 78（M3 · 方案 B 旁路快权重血缘控制面与神经 Compaction 门禁）`✅ 已完成`

> **阶段目标**：复用 M1/M2 同一套 `tx_epoch` 与 `compaction_jobs` 契约，在**零推理/训练代码进 Rust 本体、零 GB 权重存堆表**的前提下，通过旁路控制面外挂租户级快权重与后台神经整理，实现“常数延迟热态推理 + 事务级回滚 + 合规精准遗忘”。  
> **前置条件**：Phase 77（M2）全部任务 `✅ 已完成` 且帕累托门禁全绿。  
> **硬验收门禁**：百万级模拟事件流检索延迟恒定 · 连续多轮合并通用基准衰减 **`< 3%`** (`eval_glue_delta >= -0.03`) · 墓碑数据诱导召回率 **`< 0.1%`** (`unlearn_leak_rate < 0.001`) · 按 `tx_epoch` 秒级回滚 100% 通过。  
> **真机 PG 18.6 硬验收实测**：**1,000,003 事件流**实测通过 `WITH active_e AS MATERIALIZED` 局部索引物化使 **`search_tx P50 = 2.66 ms` / `P99 = 4.42 ms`**（远优于 `< 20 ms` SLA）、`active_snap P99 = 0.75 ms`、`lineage P99 = 1.94 ms`（`lineage_cte = 7,780 μs`）、按 `tx_epoch` 原子回滚耗时 **`24 ms`** · 3 类故障注入（超标范数 / 回归衰减 / 墓碑泄漏）**`100%`** 阻断置 `aborted` 且无损回退方案 A · 墓碑定向遗忘与 `ContinuityCapsule v2` 同基座热挂载/异构回退全绿 · Benchmark **`v4.1`** (`W22` `320→48 tok` / `DQ12` / `PR-16`) 与 43 工具零膨胀、`tests/fast_weight_m3_integration.rs` + 全栈四门禁全绿。

| 任务编号 | 原子任务清单 | 核心代码 / 文档触点 | 验收标准 | 状态 |
|:---:|---|---|---|:---:|
| **78.1** | **`FastWeightCheckpointSpi` 与旁路 Model Runtime 线协议**：定义标准化 JSON-RPC 2.0/HTTP 旁路接口（支持对接外部 `In-Place TTT` / `Atlas` / `Larimar` / `SCoL` 服务），附带 `MockFastWeightRuntime` 故障注入器与 `HttpFastWeightRuntime` 供离线回归与外部挂接 | `kernel/body-sdk/src/fast_weight.rs` · `contrib/mem_pg_tx/src/fast_weight_runtime.rs` · `src/config/types.rs` · `config.example.toml` | `agent-body` 本体零模型权重文件与零训练依赖（`data:` 内联权重直拒）；`arch-check` & SDK 单测全绿 | ✅ 已完成 |
| **78.2** | **快权重快照血缘账本与范数防雪崩监控**：实现 `fast_weight_snapshots` 状态机（`warming → active → compacted / rolled_back`）、递归 CTE 父子版本链、`artifact_uri` 登记与 `norm_ratio` 异常熔断 | `contrib/mem_pg_tx/src/fast_weight.rs` · `contrib/mem_pg_tx/src/schema.rs` | 百万行下血缘查询 `P99 = 1.94 ms`（`< 5 ms`）；超标 `norm_ratio` 自动拒绝切换并保留原活跃版本 | ✅ 已完成 |
| **78.3** | **定向遗忘墓碑管道 (`selective_unlearn`)**：打通 `tombstone_fact` 删除至 `compaction_jobs` (`tombstone_ids`)，向旁路 Runtime 下发定向擦除作业并记录 `OUT_OF_AGENT` 脱敏审计 | `contrib/mem_pg_tx/src/engine.rs` · `contrib/mem_pg_tx/src/fast_weight_ops.rs` · `src/memory/fast_weight.rs` | 删除事件 `100%` 进入墓碑队列；擦除前检索层严格 `0%` 僵尸召回 | ✅ 已完成 |
| **78.4** | **三道硬门禁原子切换与按 `tx_epoch` 秒级回滚**：仅当 `norm_ratio` 合规、`eval_glue_delta >= -0.03` 且 `unlearn_leak_rate < 0.001` 时在 PG 单事务原子切换 `status='active'`；任一不通过自动 `aborted` 并无损回退方案 A；三端 `body_memory_audit` / `memory audit` / `/api/v1/memories/audit` 暴露 `fast_weight_action` (`status`/`lineage`/`compact`/`unlearn`/`rollback`) | `contrib/mem_pg_tx/src/fast_weight_ops.rs` · `src/memory/fast_weight.rs` · `src/api/mcp_tools/infra_handlers.rs` · `src/cli_handlers/core.rs` · `src/web/parity_api/ops_endpoints.rs` | 3 类故障注入 `100%` 拦截回退；回滚耗时 `24 ms`（`< 1 s`）；43 工具零膨胀 | ✅ 已完成 |
| **78.5** | **热态快权重跨大脑接力 (`ContinuityCapsule v2`)**：扩展 `body_checkpoints` 增加可空 `fast_weight_snapshot_id` 与 `fast_weight_base_model`（SQLite/PG/DuckDB 三后端 + `schema_reconcile.rs` 无损补列 + `PromptInjectionScanner` 清洗 + `<= 300 tok`），同基座新大脑自动热挂、异构基座自动回退纯文本胶囊 | `src/body/continuity.rs` · `src/storage/{models,schema,pg_schema,duckdb_schema,sync_pipeline}.rs` · `src/storage/sqlite_provider/schema_reconcile.rs` · `src/api/mcp_tools/body_handlers.rs` | 老库无损补列回归通过；换脑接力自动核验 `base_model_id` 并返回 `Attached` / `Fallback` 决定论结果 | ✅ 已完成 |
| **78.6** | **M3 百万级 PG 压测、Benchmark `v4.1` 与全一等公民大一统**：新增 `tests/fast_weight_m3_integration.rs` 1,000,003 事件实测与 `Benchmark v4.1`（`W22` + `DQ12` + `PR-16`），同步更新 `README.md`、手册、3 套 SKILL、Web UI 面板与 `presentation/` 演说矩阵 | `tests/fast_weight_m3_integration.rs` · `src/benchmark/` · `docs/` · `skills/` · `scripts/build_*.py` · `presentation/` | `make quality-gate && RUST_TEST_THREADS=1 make test && make verify-webui && make verify-decks` 全绿，0 残留 PG 临时库 | ✅ 已完成 |

---

## 断点恢复与验证入口

- **设计规格依据**：[docs/design_doc/22-pg-tx-memory-spi-a-to-b-spec.md](../design_doc/22-pg-tx-memory-spi-a-to-b-spec.md)
- **前置架构锚点**：[EVOLUTION_GUIDE.md](../../EVOLUTION_GUIDE.md) · [docs/src_doc/progress.md](progress.md)
- **标准收口验证**：
  ```bash
  export PATH="$HOME/.cargo/bin:$PATH"
  make quality-gate && RUST_TEST_THREADS=1 make test && make verify-webui
  ```
