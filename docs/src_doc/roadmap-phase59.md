# Phase 59 路线图：多模态对象理解深化与资产混合检索全链路闭环 (`ISSUE #42`)

> **版本标签**：`v1.2` | **状态**：`✅ 已完成并收口` | **对应需求**：[ISSUE #42](../../ISSUE.md)

---

## 一、演进背景与痛点诊断

在 Phase 58 完成三态运行模式（`body_only` / `bypass` / `shadow_ab`）与 MCP 减税后，复查资产感知与检索引擎时发现两处核心结构性短板：

1. **多模态对象浅表化**：此前 `HeuristicMetadataExtractor` 对图片仅支持 `<svg viewBox>` 粗略提取，而对光栅图（PNG/JPEG/WebP/GIF/BMP）、PDF、Office（`.docx`/`.pptx`/`.xlsx`）、音视频（WAV/MP3/MP4/MOV 等）与二进制数据库/列存（`.sqlite`/`.db`/`.parquet`/`.duckdb`）缺少 L1 真值结构与 Schema 解析，且 `OllamaAnnotator` 对非 UTF-8 资产直接 `Ok(None)` 跳过。
2. **检索路径半截断**：`assets.embedding` 虽已持久化且 `OllamaAnnotator` 支持向量化，但 MCP `body_asset_search`、CLI `agent-body asset search` 与 Web `/api/v1/assets/search` 全部仅调用 `search_assets_fts`，导致“生成了资产向量却从未使用”，同时缺少按 `modality` / `domain` / `kv_key` / `kv_value` 精准过滤能力，`list_assets(3000/5000)` 也存在后台长尾截断隐患。

---

## 二、核心交付清单

| 编号 | 模块与文件 | 交付内容 |
|:---:|---|---|
| **59.1** | [src/scanner/multimodal.rs](../../src/scanner/multimodal.rs) | 新建三层级多模态理解引擎：统一 `infer_modality` (`image/audio/video/pdf/office/dataset/code/document/config`)；纯 Rust 光栅头尺寸/色彩解析（PNG/JPEG/GIF/WebP/BMP）与 SVG 文本/关键词抽取；WAV/MP4 纯 Rust 解析 + 可选 `ffprobe` 自适应增强；PDF 魔数 + 可选 `pdfinfo`/`pdftotext` + 纯 Rust 回退估算；Office ZIP LFH 成员扫描 + 可选 `unzip` 核心 XML 属性抽取；SQLite 只读 Schema/行数、Parquet `duckdb` in-memory `DESCRIBE` 与 DuckDB 只读表列解析；内嵌 RFC 4648 Base64 编码器。 |
| **59.2** | [src/scanner/metadata.rs](../../src/scanner/metadata.rs)<br/>[src/scanner/mod.rs](../../src/scanner/mod.rs) | 全面注入统一 `desc_kv["modality"]`；将 `pdf`/`docx`/`pptx`/`xlsx`/`sqlite`/`db`/`parquet`/`duckdb` 与多媒体资产分派至多模态提取器；升级 `format_asset_line` 在 `Standard` 披露模式下输出多模态尺寸/时长/页数/库表摘要；扩展 `infer_file_attributes` 对 Office 与拓展音视频格式的识别与打标。 |
| **59.3** | [src/config/mod.rs](../../src/config/mod.rs)<br/>[config.example.toml](../../config.example.toml)<br/>[src/scanner/ollama_annotator.rs](../../src/scanner/ollama_annotator.rs) | `[annotation]` 新增 `vision_enabled`（默认 `false`）、`vision_model`（默认 `"qwen2.5vl:3b"`）与 `vision_max_image_bytes`（默认 `4MB`）及合法性校验；解除 `enrich_pending_assets` (`3000 -> 50000`) 与 `embed_pending_assets` (`5000 -> 50000`) 截断；新增图片 Base64 Vision L2 路径 (`ollama_vision_enriched`) 与二进制多模态 L1 摘要回退 L2 路径，并将 `visual_summary`/`ocr_keywords`/`columns`/`table_names`/`pdf_title`/`svg_text_snippets` 合并写入资产向量语义文本。 |
| **59.4** | [src/scanner/asset_search.rs](../../src/scanner/asset_search.rs)<br/>[src/api/mcp_schemas.rs](../../src/api/mcp_schemas.rs)<br/>[src/api/mcp_tools.rs](../../src/api/mcp_tools.rs)<br/>[src/cli_args.rs](../../src/cli_args.rs)<br/>[src/cli_handlers/core.rs](../../src/cli_handlers/core.rs)<br/>[src/web/parity_api/core_endpoints.rs](../../src/web/parity_api/core_endpoints.rs)<br/>[src/brief/assembler.rs](../../src/brief/assembler.rs) | 新建统一 `AssetSearchEngine`：打通 FTS5 + `assets.embedding` 余弦 (`WaitEventId::CpuVectorCosine`) RRF 混合检索 + `modality`/`domain`/`kv_key`/`kv_value` 结构化过滤；实现 MCP `body_asset_search`（`BodyOnly`）、CLI `asset search`、Web `/api/v1/assets/search` 1:1:1 全对齐，`BriefAssembler` 接入过滤引擎并关闭向量调用 (`hybrid: false`) 守住 `<10ms` 开工简报 SLA。 |

---

## 三、架构不变量与门禁状态

- **零膨胀与三后端兼容**：保持 `27 modules / 43 MCP tools / 88 SPI methods / 27 SQLite tables` 不变，`Cargo.toml` 零新增外部依赖，继续使用既有 `assets.embedding` / `desc_kv` 存储列。
- **代码卫生守则**：新增 `multimodal.rs` 与 `asset_search.rs` 并把 `extract_media` 迁出 `metadata.rs`，全仓生产代码保持 `0` 个 `>1500L` 文件、`0` 个生产 `unwrap/expect`、无新增 `let _ =`、18/18 WaitEvent 全埋点与 `SecretRedactor`/`PromptInjectionScanner` 全链路脱敏抗投毒。
- **验收条目**：
  1. `make quality-gate`（架构防漂移 + 零 panic + 代码卫生 + `-D warnings` 校验 + `--all-targets` 编译）全绿通过；
  2. `scanner::multimodal` / `scanner::asset_search` / `scanner::ollama_annotator` / `config::tests` / `api::mcp::tests` 单元测试与集成测试矩阵全部通过；
  3. `/tmp/mm_smoke` E2E 实操验证真实 PNG/SVG/WAV/SQLite 文件扫描入库与 `--modality` / `--kv-key` / `--kv-value` 精准检索通过（验证后清理临时目录）。
