# Agent Body — Phase 30 ~ Phase 31 断点续传进度表

> **关联路线图**: [roadmap-phase30.md](roadmap-phase30.md)  
> **实际使用反馈总账**: [user_feedback.md](../../user_feedback.md)  
> **设计规格基线**: [10-benchmark-spec.md](../design_doc/10-benchmark-spec.md)  
> **创建时间**: 2026-11-19T21:48:00+08:00  
> **当前状态**: ✅ Phase 30 ~ Phase 31 全部完成 (`2026-11-19T22:57:00+08:00`) · 211 tests passed · 41/41 黑白屏 100% 对齐 · `make lint-panic` 0 违规

| 阶段 | 步骤 ID | 任务名称 | 对应反馈 ID | 状态 | 备注 |
|:---:|:---:|---|---|:---:|---|
| **Phase 30** | **30.1** | 建立 `user_feedback.md` 反馈总账与 Phase 30~31 演进路线图/检查点 | `REQ-01` | ✅ 已完成 | 基于现网 `body.db`、`bench report` 与 `links audit` 实测盘点，已记录 `PRO-01~05`、`CON-01~06`、`BENCH-01~04`、`REQ-01~05` |
| | **30.2** | `BriefAssembler` 索引优先加速（`fact_nodes`/`assets` 查询优先 + `/proc` 复用） | `CON-01` | ✅ 已完成 | 优先消费 `fact_nodes`/`assets` 索引并新增 `scan_host_paths_only`，空库调试 `W8` 耗时由 `474.18 ms` 降至 `13.66 ms`（单测全绿） |
| | **30.3** | `LinksEngine` 与 `LinkAuditor` 假阳性脱敏治理（模板/占位符/`tmp`/`.json`） | `CON-02` | ✅ 已完成 | 过滤 `%s`/`{var}`/`...`/`xxx`/`.json`/临时路径假阳性并降级 `.md` 散落引用为 `LOW`，`links::` 10/10 全绿 |
| | **30.4** | 全渠道生产效能存证账本与推导规则 `PR-01~10` 扩展（CLI / Web / 混合检索） | `CON-03`, `REQ-05` | ✅ 已完成 | 新增 `PR-09`/`PR-10` 与 `record_channel_call`，打通 CLI (`brief`/`search`/`fact`/`links audit`) 与 Web REST 记账 |
| | **30.5** | `user_feedback.md` 双向联动与已验证反馈 → `memories` 长期记忆自动沉淀 | `CON-04`, `CON-05`, `REQ-04` | ✅ 已完成 | 新增 `FeedbackEngine`、`body_feedback` (41/41 Parity)、CLI `feedback` 与 `/api/v1/feedback`，已验证反馈幂等沉淀入 `memories` |
| | **30.6** | 宿主机磁盘高水位预警与镜像冗余构建缓存（`target/`）GC 硬化 | `CON-06` | ✅ 已完成 | `PreflightChecker::check_disk_watermark`、`HostGarbageCollector::scan_redundant_build_caches`、`Makefile` `sync-mirror` 与 SKILL 已落地，单测全绿 |
| **Phase 31** | **31.1** | Benchmark `v2.0` 规格协议、`spec_version` 版本化、几何平均加速比与 D6 维度 | `BENCH-01`, `BENCH-03`, `REQ-02` | ✅ 已完成 | 新增 `spec_version="2.0"`、`geometric_mean_speedup`、`query_guard_speedup`、`d6_quality_score` 与 `DQ1~DQ3` 结构，向后兼容 `v1.0` |
| | **31.2** | `W1~W8` 物理等价基线校准（消除 `W2/W3/W4/W7` Apples-to-Oranges 偏差） | `BENCH-02` | ✅ 已完成 | `W2` 增加 `ss -tln`、`W3` 全量写 SQLite+FTS、`W4` 计量搬迁返工、`W7` 真实读 `SKILL.md` 并移除硬编码拉平 |
| | **31.3** | 新增 `W9`（混合语义向量检索 `Recall@K`）与 `W10`（复杂模板依赖抽取 `Precision`） | `BENCH-04`, `REQ-03` | ✅ 已完成 | 实测 `W9 Recall@5 = 100.0%`、`W10 Precision = 100.0%`（假阳性 = 0） |
| | **31.4** | 新增 `DQ1~DQ3` 宿主机数据质量门禁与跨版本 `compare_runs()` 差异报告 | `BENCH-03`, `BENCH-04` | ✅ 已完成 | `src/benchmark/dq.rs` 与 `compare_runs()` `spec_version` 差异标识已就绪 |
| | **31.5** | WebUI v2.0 升级、四层文档/SKILL 同步、真机实跑验收与服务热部署 | 全部 | ✅ 已完成 | `make lint-panic` 0 违规、`211` tests 全绿、Release 构建与现网 `bench v2.0` 实跑存证完成 |
