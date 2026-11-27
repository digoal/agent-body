# Phase 61 路线图：L1 Know-How 知识网络、Heading 切片与 Omni-RRF 全信号融合召回 (`ISSUE #44 Stage 1`)

> **版本标签**：`v1.4` | **状态**：`✅ 已完成并收口` | **对应需求**：[ISSUE #44](../../ISSUE.md) & [设计规格 19](../design_doc/19-knowledge-network-and-graph-recall-spec.md)

---

## 一、演进背景

针对端侧小参数 LLM Embedding 表征能力不足、短上下文截断易造成长文前提与因果失真，以及本机 `shadow_ab` 实测暴露的代码驼峰标识（如 `ShadowAbEngine`）FTS 漏召回问题，Phase 61 正式落地 **`ISSUE #44 Stage 1`**：以确定性 **Heading 切片 + L1 概念/PMI 知识网络 + Omni-RRF 8 维文本融合检索** 彻底突破单一弱向量天花板。

---

## 二、核心交付清单

| 编号 | 模块与文件 | 交付内容 |
|:---:|---|---|
| **61.1** | [src/storage/schema.sql](../../src/storage/schema.sql)<br/>[sqlite_provider/assets.rs](../../src/storage/sqlite_provider/assets.rs)<br/>[sqlite_provider/memory.rs](../../src/storage/sqlite_provider/memory.rs)<br/>[sqlite_provider/mod.rs](../../src/storage/sqlite_provider/mod.rs) | 扩展 `fact_nodes` (`section`/`domain_term`/`knowhow`) 与 `fact_edges` (`mentions`/`co_occurs_with`/`causes`/`mitigates`/`contrasts_with`/`applies_when`/`supersedes`) 枚举约束与老库幂等表重建热迁移（保持 27 表不变）；升级 FTS5 查询至加权 `bm25(...)`（`assets_fts`/`memories_fts`）并内置驼峰/蛇形/连字符技术标识多候选查询构建器 (`build_fts_query_candidates`)。 |
| **61.2** | [src/fact/knowledge_graph.rs](../../src/fact/knowledge_graph.rs)<br/>[src/fact/mod.rs](../../src/fact/mod.rs)<br/>[src/config/mod.rs](../../src/config/mod.rs)<br/>[config.example.toml](../../config.example.toml) | 新增 `[knowledge_graph]` 配置块与边界校验；实现 `HeadingChunker`（Markdown `#/##/###` 父子层级切分、面包屑与长节重叠窗口）、`AliasCanonicalizer`（PG/FTS5/SQLite/DuckDB/WAL 等行业同义词归一）、`KnowledgeGraphBuilder`（`section` + `domain_term` + `mentions` + 跨章节 PMI `co_occurs_with` `Top-5` 剪枝防毛线球）与 `expand_graph_for_query`（1~2 跳衰减扩散 + `supersedes` 降权 + 可解释 `reasoning_paths`）；`FactFacade` 接入 `knowledge`/`mine-knowledge`。 |
| **61.3** | [src/scanner/asset_search.rs](../../src/scanner/asset_search.rs)<br/>[src/memory/hybrid.rs](../../src/memory/hybrid.rs)<br/>[src/scanner/metadata.rs](../../src/scanner/metadata.rs)<br/>[src/body/runtime_ab.rs](../../src/body/runtime_ab.rs) | `AssetSearchEngine` 与 `HybridSearchEngine` 全面升级为 **Omni-RRF**：融合 **`bm25_fts` + 纯 Rust `fuzzy` (bigram Jaccard + bounded Levenshtein) + 可选 `semantic_vec` + `graph_knowhow` + `hierarchy` + `doc_links` + $\Phi_{\text{scalar}}$ 标量调制**，离线无向量时自动权重重归一化；扩展 `extract_script` 扫描完整 8KB 头部（最多 24 符号）；`ShadowAbEngine::compare_asset_search` 接入真 Omni-RRF 引擎消除虚假 FTS 回退。 |
| **61.4** | [mcp_schemas.rs](../../src/api/mcp_schemas.rs) / [mcp_tools/mod.rs](../../src/api/mcp_tools/mod.rs)<br/>[cli_args.rs](../../src/cli_args.rs) / [cli_handlers/core.rs](../../src/cli_handlers/core.rs)<br/>[core_endpoints.rs](../../src/web/parity_api/core_endpoints.rs)<br/>[brief/assembler.rs](../../src/brief/assembler.rs) | 严守 `43` MCP 工具零膨胀，1:1:1 透出 `graph_hops`/`fuzzy`/`include_links` 与 CLI `asset scan --mine-knowledge`，Web JSON 返回 `matched_sections` 与 `reasoning_paths`，`BriefAssembler` 保留 `<10ms` 快速子集 (`include_links = false`)。 |

---

## 三、验收状态与门禁指标

- **`make quality-gate` + `cargo clippy --all-targets -- -D warnings`**：`0 drift`、`0 warnings`、`0` 生产 `unwrap/expect`、`let _ = <= 43/50`、`>1500L: 0`、`18/18 WaitEventId`。
- **全平台测试套件**：`cargo test --lib` 与全部 8 套集成测试 `100%` 通过。
- **本机真库 A/B 实证（`shadow_ab` + `/Users/digoal/.agent-body/body.db`）**：验证知识网络构建完成、离线跨词图扩散与 `reasoning_paths` 生效，且 `ShadowAbEngine` 此前 `0 hits / body_regression` 彻底消除。
