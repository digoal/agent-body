# Agent Host Steward — Phase 29 演进路线图：事实对象全景属性字典重构、全链路 Embedding 向量化与 WebUI 元数据贯通

> **编制**: Antigravity · **日期**: 2026-11-19  
> **对齐基准**: 用户反馈（WebUI 元数据 JSON 过干瘪、缺乏 `description` 与 `embedding` 等属性 Key）  
> **断点状态**: ✅ 已完成 (2026-11-19 20:17)

---

## 一、核心目标

彻底消灭拓扑节点与事实对象“只有路径、缺少描述、没有向量、WebUI 没关联 `assets` 语义库”的现状，建立：
1. **7 类事实对象 + 8 类资产统一全景属性字典**：所有节点 100% 具备 `description`、`canonical_path`、`domain`、`category`、`owner`、`security_level`、`importance_score` 及丰富的类型特化属性 Key；
2. **全链路本地 Ollama Embedding 向量化**：`memories`、`fact_nodes`、`assets` 底层持久化向量列，同时在元数据 JSON 暴露 `embedding_status`、`embedding_model`、`embedding_dim`、`embedding_norm`、`embedding_preview`（前 8 维预览）；
3. **WebUI 拓扑与资产语义深度融合**：`src/web/topology.rs` 按路径 Join 回填 `assets` 语义库、`fact_nodes` 与实时物理探针，目录折叠节点汇总子资产域分布与描述，WebUI 抽屉分组渲染 L1/L2/L0 与向量胶囊。

---

## 二、原子任务清单与断点追踪

| 步骤 ID | 原子任务 | 涉及文件 | 状态 | 验收标准 |
|:---:|---|---|:---:|---|
| **29.1** | 全景属性字典重构与扫描分类/噪音门禁硬化 | `src/scanner/mod.rs`, `src/scanner/importance.rs`, `src/scanner/metadata.rs` | ✅ 已完成 | 补齐 `dataset`/`media`/多语言 `script`/`config`/`document` 分类与专属 KV；拦截 PGDATA 原始数据块噪音 |
| **29.2** | `PropertyGraphOrganizer` 事实图谱采集器升级 | `src/fact/property_graph.rs`, `src/fact/assets_catalog.rs` | ✅ 已完成 | `file`/`port`/`project`/`cron`/`service`/`tool` 100% 具备 `description` 与完整 L1/L2 属性 |
| **29.3** | 全链路本地 Ollama Embedding 引擎与三引擎同步 | `src/memory/embedding.rs`, `src/storage/`, `src/memory/`, `src/fact/`, `src/scanner/` | ✅ 已完成 | `memories`、`fact_nodes`、`assets` 向量生成与落库；元数据 JSON 暴露 `embedding_*` 摘要；Ollama 离线优雅降级 |
| **29.4** | WebUI 拓扑组装层与 `assets` / `fact_nodes` 深度融合 | `src/web/topology.rs`, `src/web/assets/mod.rs` | ✅ 已完成 | `/api/v1/topology` 单 Key 节点清零；折叠目录 `dir:*` 聚合子域与描述 |
| **29.5** | WebUI 抽屉升级、现网库全量刷新、回归与四层同步 | `src/web/assets/mod.rs`, `README.md`, `manual/`, `skills/`, `src_doc/` | ✅ 已完成 | `make lint-panic` 0 违规、`cargo test` 全绿、现网重扫 + 向量生成 + 服务热重启实测验证 |
