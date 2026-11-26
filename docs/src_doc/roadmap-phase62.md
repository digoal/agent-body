# Phase 62 路线图：Indexed Subgraph 下沉、小模型 L2 Know-How 因果抽取与 Benchmark `v3.9`（ISSUE #44 Stage 2）

> **关联规格**：[19-knowledge-network-and-graph-recall-spec.md](../design_doc/19-knowledge-network-and-graph-recall-spec.md) · **前置阶段**：[roadmap-phase61.md](roadmap-phase61.md)

---

## 一、 本阶段目标

围绕 `ISSUE #44` Stage 2 与 Phase 61 本机实测暴露的 4.9 万节点全图加载截断瓶颈，完成以下五项高优先级落地：

1. **SQLite 索引下沉定向子图扩展**：消除 `expand_graph_for_query` 的 `list_all_fact_nodes(4000)/edges(8000)` 全表拉取与截断，改用 `idx_fact_nodes_name` + PK 精准种子定位及权重排序、度数阻尼、节点上限有界的 1~2 跳子图扩展；并增强 `fuzzy_text_score` 符号前置优先级，确保 `ShadwAbEngine` 等拼写容错稳定命中源码资产。
2. **本地小模型 (`qwen3.5:0.8b`, `think: false`) L2 Know-How 抽取与原文锚定防幻觉**：新增 `KnowHowExtractor`，仅向小模型投喂单章节 `<=1000` 字符脱敏片段并强制 `evidence_quote` 确定性验证（幻觉入库率 `= 0%`），落地 `causes`/`mitigates`/`depends_on`/`contrasts_with`/`applies_when`/`supersedes` 因果与适用边界边。
3. **写穿级联失效闭环**：`KnowledgeGraphBuilder::invalidate_source_path` 接入 `FreshnessReconciler::reconcile`，源文件删除或内容漂移时 `100%` 清理其派生 `section`/`knowhow` 节点及关联边。
4. **Benchmark `v3.9` (`W19` + `DQ10`)**：在 `src/benchmark/workloads/knowledge.rs` 与 `dq.rs` 落地弱向量 Know-How/Omni-RRF 对照基准与防毛线球度数 (`<=32`) / 原文锚定 / 零幽灵节点质量门禁。
5. **CLI / MCP / Web UI 1:1:1**：新增 `asset scan --extract-knowhow`、`body_asset_scan.extract_knowhow`、Web 资产搜索通道徽章 / 章节与推理路径展示、`v3.9` 基准工作台。

---

## 二、 任务检查清单

- [x] `impl_find_fact_nodes_by_name` 等值优先 + `impl_traverse_fact_graph` 有界扩展 + `expand_graph_for_query` 索引下沉定向子图
- [x] `KnowledgeGraphBuilder::invalidate_source_path` + `FreshnessReconciler` 写穿级联清理
- [x] `src/fact/knowhow_extractor.rs` (`verify_grounding_quote`, `ingest_triples_for_section`, `extract_from_storage` `think: false`)
- [x] `Benchmark v3.9`: `W19` (`workloads/knowledge.rs`) + `DQ10` (`dq.rs`)
- [x] CLI / MCP / Web UI 1:1:1 与 `EVOLUTION_GUIDE.md` / `progress.md` / `ISSUE.md` 更新
- [x] 全量门禁 (`make quality-gate`, `clippy`, `cargo test --lib` + 8 组集成测试 + 本机 `/Users/digoal/.agent-body/body.db` 实操校验)
