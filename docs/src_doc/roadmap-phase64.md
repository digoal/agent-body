# Agent Body Roadmap — Phase 64: `v1.1.0` 全一等公民大一统对齐 (`README` / `Web UI` / `SKILL` / 手册 / 防漂移门禁)

> **文档状态**：✅ **已实施** | **基线版本**：Phase 63 (`c15df5e`, `v1.0.0`) -> `v1.1.0`  
> **定位**：依照规约 `#10/#11/#13/#16/#20`，彻底消除 Phase 58~63 高速演进带来的“代码领先文档/Web前端/SKILL”断层，升级防漂移门禁并将 `Cargo.toml` 升版发布 `v1.1.0`。

---

## 1. 阶段交付核对表

| Stage | 核心交付项 | 对应文件 | 状态 |
|---|---|---|---|
| **Stage 1** | 升级 `evolution_manifest.py` 防漂移门禁 + `Cargo.toml` 升 `1.1.0` | [`scripts/evolution_manifest.py`](../../scripts/evolution_manifest.py), [`Cargo.toml`](../../Cargo.toml) | ✅ |
| **Stage 2** | Web UI Action-Level 1:1:1 补齐（三态/MCP 档位/Shadow A/B `H1~H5` 面板 + 资产 `kv_filter`/`modality`/`asset_type`/`backfill_multimodal`/`extract_knowhow` + 端口 `8765`） | [`src/web/assets/index.html`](../../src/web/assets/index.html), [`src/web/assets/js/05_body.js`](../../src/web/assets/js/05_body.js), [`src/web/assets/js/02_topology_facts.js`](../../src/web/assets/js/02_topology_facts.js), [`tests/test_web_dashboard.rs`](../../tests/test_web_dashboard.rs) | ✅ |
| **Stage 3** | 全量文档与 SKILL 对齐（3 SKILL + `README` + `INSTALL` + `EVOLUTION_GUIDE` + `docs/README` + `ISSUE` + `docs/arch/*` + `docs/manual/{README,03..10}` + `docs/design_doc/{06,10}` + `v1.1.0`） | `skills/*`, `docs/manual/*`, `docs/arch/*`, `docs/design_doc/{06,10}*`, [`docs/releases/v1.1.0.md`](../releases/v1.1.0.md) | ✅ |
| **Stage 4** | 五重质量门禁 + Clippy + 全量测试 + Release 重装并重启 `LaunchAgent` 刷新 `http://127.0.0.1:8765` | [`Makefile`](../../Makefile), `~/Library/LaunchAgents/com.digoal.agent-body.plist` | ✅ |
