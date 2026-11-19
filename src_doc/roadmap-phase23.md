# Agent Host Steward — Phase 23 演进路线图：分层语义元数据 Key 体系与本地 Ollama 小模型双轨自动标注

> **编制**: Antigravity · **日期**: 2026-11-19  
> **基线 Commit**: `e7f1fab` (`v0.3.3`, Phase 1~22 全部交付，39 个 MCP 工具黑白屏 100% 同步)  
> **对齐基准**: [README.md ISSUE #8](../README.md#L305) & [核心设计原则与约束 §1-§17](../README.md#L206-L279)  
> **断点续传检查点表**: [progress-phase23.md](progress-phase23.md)

---

## 一、目标与核心架构

解决现网 `assets` 表中 `desc_kv` 仅记录 `{"watcher_sync": true}` 或 `{"filename": ..., "tier": ...}`、无法从 KV 看清对象业务语义的痛点，并兼顾宿主机 CPU 算力约束与主 Agent Context 防爆原则：

1. **三层正交元数据 Key 体系 (`src/scanner/metadata.rs`)**：
   - **L1 通用语义核心层（必填）**：`purpose` (≤30字，同步回填 `description`)、`domain`、`capabilities` (2~4 项)、`entry_point`、`usage_hint` (同步回填 `usage`)。
   - **L2 资产类型特化层（按 `asset_type` 动态挂载）**：
     - `script`: `lang`, `symbols`, `io_contract`, `dependencies`, `side_effects`, `loc`
     - `document`: `doc_category`, `headings`, `key_entities`, `actionable_takeaway`
     - `config`: `target_system`, `config_sections`, `bound_ports_or_paths`, `env_vars_referenced`
     - `project` / `dataset`: `tech_stack`, `build_test_cmd`, `codegraph_ready`
   - **L0 管线治理与溯源层**：`content_hash`、`annotation_status` (`pending` | `heuristic_only` | `ollama_enriched` | `needs_re_enrich` | `manual`)、`annotation_model`、`annotated_at`、`importance_score`。
2. **双轨混合标注流水线**：
   - **Track 1（同步 `< 1ms`）**：`HeuristicMetadataExtractor` 基于文件头与语法特征确定性提取 `symbols`/`headings`/`config_sections`/`dependencies`/`side_effects`/`loc`/`content_hash` 与初版 `purpose`。
   - **Track 2（异步 / 按需 `--enrich`）**：`OllamaAnnotator` (`src/scanner/ollama_annotator.rs`) 对 `deep` 级待精炼资产进行密钥脱敏（`SecretRedactor`）后，使用数据导向防复读 Prompt + `"think": false` + JSON Schema 强约束调用本地 Ollama（默认 `qwen3.5:0.8b`）提炼高阶语义；Ollama 不可用时优雅降级为 `heuristic_only`。
3. **防覆写合并策略**：
   - 修复 `ChangeHandler` (`src/watcher/handler.rs`) 与 `AutoSyncEngine` (`src/hooks/auto_sync.rs`)，若 `content_hash` 未变则保留已有丰富 `desc_kv`/`description`/`usage`；若内容变更则刷新 L1 并标 `needs_re_enrich`。
4. **三级渐进式 Token 披露 (`detail_level: "micro" | "standard" | "full"`)**：
   - `micro` (~25 tok)、`standard` (~85 tok，剥离 L0 管线噪音)、`full` (~180 tok，含 L0 溯源)，全面贯通 MCP、CLI 与 Web UI。

---

## 二、Phase 23 原子任务清单

| 步骤 ID | 原子任务 | 涉及文件 | 验收标准 |
|:---:|---|---|---|
| **23.1** | 配置与预检层扩展 (`[annotation]`) | `src/config/mod.rs`, `config.example.toml`, `src/preflight/mod.rs` | `AnnotationConfig` 配置可解析校验；`preflight` 探测 Ollama 生成模型就绪状态并支持优雅降级 |
| **23.2** | L1 确定性提取器与防覆写修复 | `src/scanner/metadata.rs`, `src/scanner/mod.rs`, `src/scanner/state_machine.rs`, `src/watcher/handler.rs`, `src/hooks/auto_sync.rs` | 提取 L1/L2/L0 结构化 KV；`Watcher`/`AutoSync` 不再覆写抹除已有语义；单文件提取 `<1ms` |
| **23.3** | L2 本地 Ollama 异步精炼引擎 | `src/scanner/ollama_annotator.rs`, `src/daemon/scheduler.rs` | `SecretRedactor` 脱敏、`"think": false` + JSON Schema 强约束、`BackgroundScheduler` 增量精炼与优雅降级 |
| **23.4** | 三级渐进披露与黑白屏 1:1 同步 | `src/api/mcp_tools.rs`, `src/main.rs`, `src/web/parity_api.rs`, `src/web/assets/mod.rs`, `src/demo/mod.rs` | `steward_asset_search` 支持 `detail_level`；`steward_asset_scan` 支持 `enrich_with_ollama`；Web UI 展示彩色 KV 胶囊与一键精炼 |
| **23.5** | 全量测试、现网库实测与四层同步 | `tests/`, `README.md`, `manual/`, `skills/`, `src_doc/` | `make lint-panic` 0 违规、`cargo test` 全绿、现网库重扫 + Ollama 实测对比、四层文档与服务热重启完成 |
