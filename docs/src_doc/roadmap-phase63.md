# Agent Body Roadmap — Phase 63: 多模态解析引擎深化、三通道统一与 Shadow A/B `H1~H5` 自愈 (`v1.6`)

> **文档状态**：✅ **已实施** | **基线版本**：Phase 62 (`6f4a1b7`, `v1.5`) -> `v1.6`  
> **定位**：在不扩张 `27 modules / 43 MCP / 88 SPI / 27 tables` 约束矩阵的前提下，选择性合入多模态 L1A 纯 Rust 解析器、三大摄入通道魔数统一分类、有界子进程探针、多谓词 `kv_filter` 检索、`FreshnessReconciler` 增量无损回填、热切换运行模式与 `ShadowAbEngine` `H1~H5` 帮倒忙检测及写穿自愈。

---

## 1. 核心交付明细

| 交付域 | 关键文件与实现要点 | 验收标准 |
|---|---|---|
| **1. 纯 Rust 多模态 L1A 解析与有界探针** | [`src/scanner/multimodal_ext/mod.rs`](../../src/scanner/multimodal_ext/mod.rs) & [`src/scanner/multimodal.rs`](../../src/scanner/multimodal.rs)<br/>- Pure-Rust HTML (`doc_title`, `headings`, `section_count`, `inline_svg_count`, `html_excerpt`)、Parquet Thrift Footer (`columns`, `column_types`, `num_rows`)、CSV/TSV 列类型推断与行数估算、JSONL 键集合、MP3/FLAC/OGG 音频头；<br/>- `run_command_bounded` (`kill + wait`, 默认 `<=500ms`) 约束 `ffprobe`/`pdfinfo`/`pdftotext`，受 `[annotation].{multimodal_tools_enabled, multimodal_tool_timeout_ms}` 控制；<br/>- `PreflightChecker::check_multimodal_tools` 自动探测宿主机可选工具 | 损坏/截断文件 0 Panic；所有不可信文本均经 `SecretRedactor` + `PromptInjectionScanner`；无外置工具时 100% 回退纯 Rust |
| **2. 三通道魔数+扩展名联合分类与 Schema `2.0`** | [`src/scanner/mod.rs`](../../src/scanner/mod.rs)、[`src/watcher/handler.rs`](../../src/watcher/handler.rs)、[`src/hooks/auto_sync.rs`](../../src/hooks/auto_sync.rs)、[`src/scanner/metadata.rs`](../../src/scanner/metadata.rs)<br/>- 三大入口统一调用 `classify_asset_path`；<br/>- `desc_kv` 统一写入 `schema_version = "2.0"` 与 `mime_type` | `Scanner` / `FileWatcher` / `AutoSyncEngine` 对伪装扩展名或二进制魔数资产判定结果 100% 一致 |
| **3. 多谓词 `kv_filter` 与 1:1:1 增量回填** | [`src/scanner/asset_search.rs`](../../src/scanner/asset_search.rs)、[`src/feedback/probes.rs`](../../src/feedback/probes.rs)、[`src/cli_args.rs`](../../src/cli_args.rs)、[`src/cli_handlers/core.rs`](../../src/cli_handlers/core.rs)、[`src/api/mcp_schemas.rs`](../../src/api/mcp_schemas.rs)、[`src/api/mcp_tools/mod.rs`](../../src/api/mcp_tools/mod.rs)、[`src/web/parity_api/core_endpoints.rs`](../../src/web/parity_api/core_endpoints.rs)<br/>- 支持 `=`, `!=`, `>=`, `>`, `<=`, `<`, `~`, `has:key`, 裸 `key` 多条件与 `asset_type` 过滤；<br/>- `backfill_multimodal_assets` 无损保留已有 `ollama_enriched` 语义并接入 `FreshnessReconciler::reconcile` 与 CLI/MCP/Web | 全端 `asset search` / `asset scan` 参数 1:1:1 对齐；存量 `other` 资产幂等升级 |
| **4. 运行模式热切换与 Shadow A/B `H1~H5` 自愈** | [`src/config/mod.rs`](../../src/config/mod.rs)、[`src/hooks/client_gen.rs`](../../src/hooks/client_gen.rs)、[`src/body/runtime_ab.rs`](../../src/body/runtime_ab.rs)<br/>- 支持 `{data_dir}/{runtime_mode,execution_mode}.override` 热覆盖，CLI/MCP/AGY/Claude/Hermes 桥接一致生效；<br/>- `ShadowAbComparison` 增加 `hindering_flags` (`H1~H5`) 与 `auto_healed_count`，受 `[runtime].auto_heal_on_hindering` 控制，对 `H1`/`H2` 执行写穿自愈并输出零回退合流结果，`H5_LEASE_MISLEAD` 复用 `audit_events` | 复用既有 27 张表与 88 SPI；`tests/multimodal_integration.rs` 与 `runtime_ab` 单测全绿 |

---

## 2. 质量与规范自检门禁

1. 架构指纹严格维持：`27 modules | 43 MCP tools | 88 SPI methods | 27 SQLite tables | W:19, M:6, DQ:10, PR:14`；
2. 零新增 `Cargo.toml` 依赖、所有生产 `.rs` 文件 `<= 1500` 行、`0` 新增生产 `unwrap()`、`let _ =` 总量严控、Clippy 零告警；
3. `cargo test --lib` 与 9 组集成测试 (`benchmarks`, `duckdb_integration`, `test_web_dashboard`, `cli_e2e`, `audit_integration`, `body_awr_integration`, `hooks_integration`, `mesh_integration`, `multimodal_integration`) 全绿。
