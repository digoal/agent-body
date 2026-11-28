# 23 · 超集检索、数据库侧模糊查询与真基线对照规格（Phase 79 / ISSUE #56）

> **定位**：确立 Agent Body 检索能力必须严格成为“不使用 Agent Body（原生 `rg` / `grep` / 全量扫描）”的**严格超集**，在数据库侧完成正文索引、模糊查询（Trigram 索引加速 + LIKE 退化）、BM25、属性图关联、标量过滤、语义向量防噪门禁与 Omni-RRF 融合重排，并重塑真实 Shadow A/B 与 WebUI 对照闭环。

---

## 一、第一性原理与问题根因

不使用 Agent Body 时，编码 Agent 或开发者直接使用 `rg` / `grep` 对磁盘源码与文档进行全量正文扫描，具备两个朴素但极强的性质：**不漏文件、不截断、按字面/正则精确命中并附带行号**。此前本体在以下四个点违背了超集原则：

1. **只索引元数据不索引正文（`content_text` 缺失）**：`assets` 表及 `assets_fts` 仅存路径、描述、标签等摘要，导致源码内部私有函数、标识符或正文片段在 `asset search` 中直接漏召回。
2. **硬编码截断与历史失效资产混入**：检索路径依赖 `list_assets(5000)` 截断且未强制过滤 `status = 'active'`，库内资产超过 5,000 行后后半段完全不可见。
3. **模糊查询在 Rust 内存循环而非数据库执行**：未在数据库内利用 Trigram / GIN / `LIKE` 过滤，吞吐随资产规模线性恶化且受 5,000 行截断影响。
4. **弱语义向量噪声污染与假基线对照**：向量余弦阈值过低（`0.05`）导致精确标识符未命中时塞入大量无关 `[vec]` 结果；`ShadowAbEngine` 基线仅读前 4KB 且上限 4,000 文件，WebUI 也未提供与不用 Body (`rg`/`grep`) 的实时同屏对照。

---

## 二、核心架构与三后端数据模型演进

### 2.1 正文脱敏提取（`src/scanner/content_extract.rs`）

- 统一入口 `extract_indexable_content_text(path, scan_tier, max_bytes)`：
  - **黑白名单硬红线**：`scan_tier == "shallow"` 或命中 `is_sensitive_file(path)` 严禁读取任何字节，直接返回 `None`。
  - **非纯文本保护**：跳过图片、音视频、Office/PDF 二进制、压缩包、`.db`/`.sqlite`/`.duckdb` 与 `> 20MB` 巨型文件，以及含 NUL 字节文件。
  - **可控窗口与脱敏**：超出 `max_index_content_bytes`（默认 `131072` = 128KB）时采用首部 3/4 + 尾部 1/4 截断，并经 `SecretRedactor::redact` 脱敏后再落库。
  - **全链路同步**：在 `Scanner::flush_batch`、`ScanStateMachine`、`FileWatcher`、`auto_sync` 与 `ShadowAbEngine` L1 自愈全量复用，并提供 `backfill_asset_contents`（CLI `asset scan --backfill-content`）对存量活跃资产补齐正文。

### 2.2 三后端数据库侧模糊与 6 列全文检索（95 方法 SPI / 28 张 SQLite 表）

- **Schema 演化**：
  - `assets` 新增 `content_text` 列（三后端 `schema.sql` / `pg_schema.sql` / `duckdb_schema.sql` 与 `sync_pipeline` 19 列 Appender 同步）。
  - SQLite `assets_fts` 升级为 6 列（`path, description, desc_kv, usage, tags, content_text`），在 `schema_reconcile.rs` 中先为历史 `assets` 表补齐 `content_text` 列，再无损重建 6 列 FTS5 及触发器。
  - SQLite 新增可选第 28 张虚表 `assets_trigram_fts`（FTS5 `tokenize='trigram'`）及三联触发器，支持 `set_fuzzy_index_enabled(bool)` 在线启停。
  - PostgreSQL 将 `content_text` 纳入 `fts_doc`（`tsvector` GIN），并在 `DO $$` 块中按可用性创建 `pg_trgm` GIN 索引；DuckDB 通过参数化 `LIKE` / `list_contains` 与内存可观测裁剪提供等价 SPI 契约。
- **7 个新增 SPI 方法**（`src/storage/spi.rs`）：
  `update_asset_content`、`get_asset_content`、`search_assets_fuzzy_db`、`search_assets_scalar_db`、`search_assets_vector_db`、`set_fuzzy_index_enabled`、`search_memories_fuzzy_db`。

---

## 三、Omni-RRF 精确防噪门禁与分层输出

1. **查询意图识别（`QueryIntent`）**：识别 `StrictIdentifier`（驼峰、蛇形、`::` 等代码标识符或短精确词）与 `NaturalLanguage`。
2. **双层召回分级（`match_tier`）**：
   - `deterministic`：精确标识符 / DB 模糊（`fuzzy` + `db_fuzzy_index` / `db_like_scan`）/ 6 列 `bm25_fts`；
   - `expansion`：校准语义向量（`min_vec_similarity = 0.58`，`StrictIdentifier` 且 `strict_identifier_vec_gate = true` 时动态抬升至 `0.65 / 0.78`）、属性图 1~2 跳（`graph_knowhow`）、目录层级（`hierarchy`）与 `doc_links`。
3. **精密防噪门禁（`apply_precision_gate`）**：
   - 若为 `StrictIdentifier` 且存在确定性命中，丢弃纯 `[vec]` 扩展垃圾；
   - 纯 `[vec]` 候选须同时满足相对主峰窗口（`top_vec_sim - 0.12`）与最低相似度门槛；
   - 存在确定性命中时扩展槽位受控裁剪（最多补足 5 条），杜绝“无缺失、却多余”的噪声污染；
   - 结果附带 `matched_line` 与 `matched_snippet`，对齐 `rg` 精准定位体验。

---

## 四、真实基线引擎（`src/body/live_search.rs`）与三端 + WebUI 闭环

- **`run_baseline_workspace_search`**：按 `rg --json` → `grep -rn` → 全文 `WalkDir`（受 `.gitignore` / `ignore_patterns` 约束）优先级执行，真实统计耗时、Token、命中路径与行号。
- **`compare_asset_search_detailed`**：产出 `superset_satisfied`、`common_paths`、`missed_by_body`、`extra_by_body` 与 `baseline_engine`，支撑「不漏文件、不多垃圾、更快更省 Token」量化判据。
- **四端透出**：
  - CLI：`asset search --scope --no-fuzzy-index`、`asset scan --backfill-content --backfill-limit`、`body ab-compare --scope --no-hybrid --no-fuzzy --no-fuzzy-index --graph-hops`；
  - MCP：`body_asset_search` 新增 `use_fuzzy_index` / `scope_path` / `hybrid`，`body_state(action="ab_compare")` 新增 `scope_path` / `hybrid` / `use_fuzzy_index`（维持 43 工具零膨胀）；
  - Web API：`/api/v1/assets/search`（默认 `hybrid=true`，透出 `match_tier` / `matched_line` / `matched_snippet`）与 `POST /api/v1/body/ab-compare`；
  - WebUI：资产台新增 `⚔️ 对比不用Body (rg/grep)` 一键真基线比武与命中分级徽章；Body 控制台支持 `scope_path` / `hybrid` / `DB Trigram` 细粒度对照面板。
