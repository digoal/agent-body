# Phase 79 演进路线与交付验收台账（ISSUE #56：超集检索 × DB 侧模糊索引 × 真基线 A/B）

> **定位**：彻底根治 `asset search` 漏召回正文、5,000 行截断、弱向量垃圾噪声与 `ShadowAbEngine` 假基线平局问题，使 Agent Body 检索在「准确、无缺失、无多余」三项指标上成为原生 `rg`/`grep` 的严格超集。

---

## 交付清单与验收实证

| 任务模块 | 核心变更与代码触点 | 验收状态 |
|---|---|---|
| **79.1 配置与三端 Schema 升级** | `KnowledgeGraphConfig` 新增 `db_fuzzy_index` / `max_index_content_bytes` / `min_vec_similarity` / `strict_identifier_vec_gate`；三后端 `assets` 补 `content_text` 列；SQLite 升级 6 列 `assets_fts` + 第 28 张虚表 `assets_trigram_fts`；`schema_reconcile.rs` 无损先补 `content_text` 列再重建 FTS5 触发器；`sync_pipeline.rs` 升级 19 列 DuckDB Appender | ✅ 已完成 |
| **79.2 存储 SPI 与三后端 DB 检索** | `StorageProvider` 扩充至 **95 方法**（+7：`update_asset_content`、`get_asset_content`、`search_assets_fuzzy_db`、`search_assets_scalar_db`、`search_assets_vector_db`、`set_fuzzy_index_enabled`、`search_memories_fuzzy_db`）；SQLite / PG / DuckDB 分域实现 `assets_search.rs` 及 `memory.rs` 模糊通道 | ✅ 已完成 |
| **79.3 正文脱敏采集与存量回填** | 新增 `src/scanner/content_extract.rs`（严格执行 `shallow` / `is_sensitive_file` 零字节拦截、非文本与 `>20MB` 过滤、3/4+1/4 截断与 `SecretRedactor` 脱敏）；全链路接入 `Scanner` / `ScanStateMachine` / `FileWatcher` / `auto_sync` / `ShadowAbEngine`；CLI `asset scan --backfill-content` | ✅ 已完成 |
| **79.4 模块化引擎与防噪门禁** | 将 `src/scanner/asset_search.rs` 拆分为 `src/scanner/asset_search/{mod.rs, fuzzy_and_gate.rs, tests.rs}`（全量 `<1000L`）；落地 `QueryIntent` 识别、`deterministic`/`expansion` 双层分级、`apply_precision_gate`、`matched_line`/`matched_snippet` 定位及记忆混合检索 `MIN_MEMORY_VEC_SIM=0.52` | ✅ 已完成 |
| **79.5 真基线引擎与四端 + WebUI 闭环** | 新增 `src/body/live_search.rs`（`rg` → `grep` → 全文 `WalkDir`）；重构 `compare_asset_search_detailed` 输出 `superset_satisfied` / `missed_by_body` / `extra_by_body`；对齐 CLI / MCP(43 工具不变) / Web API / WebUI 资产台 `⚔️ 对比不用Body (rg/grep)` 与 Body A/B 面板 | ✅ 已完成 |
| **79.6 回归测试与门禁** | 新增 `tests/superset_search_regression.rs`（3 项：超集精确/CJK/0噪声、突破 5,000 行截断及排除 `missing`、DB Trigram 索引切换等价）；扩充 `schema_migration_regression.rs` / `storage_spi_contract.rs` / `web_full_e2e.rs`；`RUST_TEST_THREADS=1 make test` **440 项全绿** | ✅ 已完成 |
