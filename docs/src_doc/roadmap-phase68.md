# Phase 68 演进路线图：一等公民 SSoT 交叉对齐与防漂移门禁闭环 (ISSUE #50)

> **对应 ISSUE**: [ISSUE.md #50](../../ISSUE.md#L237)  
> **创建时间**: 2026-11-27 13:01 CST  
> **当前状态**: ✅ 全部完成（`0 drift` / 全仓 43 工具三端精确契约 / `v1.1.0` & `Bench v3.9` 全物料 100% 同步）  
> **关联规约**: README 规约 #11（一等公民同步）、#13（质量门禁）、#20（零重扫防漂移）

---

## 一、阶段目标

1. **三端精确路由与跨通道契约测试**：收敛 `src/web/parity_api/ops_endpoints.rs` 中 `body_audit_query` / `body_state` 的通配路由为真实精确路由，并在 `src/web/mod.rs` + `src/main.rs` 补齐 43 工具 MCP ↔ CLI ↔ `server.rs` 自动化交叉契约测试。
2. **`evolution_manifest.py` 四重 SSoT 硬门禁**：从 `Cargo.toml` 提取 `app_version`，新增版本跨端对齐、活跃文档/Skill 反引号源码路径存在性校验、`parity_api` 43 路由精确命中 `server.rs`、以及 9 份演说/4 套构建脚本 Bench/版本一致性校验。
3. **全仓一等公民同步与动态化演说生成**：将 4 套 `scripts/build_*.py` 改为直接导入 `evolution_manifest.extract_manifest()` 快照重生成全部 9 份 `presentation/*.html`；修复 `EVOLUTION_GUIDE.md`、`docs/arch/02-for-developers.md`、`skills/*`、`README.md`、`INSTALL.md`、`docs/manual/{03,10}` 与 `docs/releases/v1.1.0.md` 的死代码路径与过时计数。

---

## 二、原子任务断点续传追踪表

| 任务 ID | 描述 | 状态 | 完成摘要 |
|---|---|---|---|
| **P68-01** | `parity_api` 通配路由精确化 & `src/web/mod.rs` / `src/main.rs` 交叉契约测试 | ✅ 已完成 | 43 工具 100% 命中 `server.rs` 精确路由，零 `*` 通配符 |
| **P68-02** | `scripts/evolution_manifest.py` 新增 `app_version` 提取与 4 项 SSoT 硬门禁 | ✅ 已完成 | 版本/源码路径/`parity_api` 路由/演说脚本全纳入 `0 drift` 校验 |
| **P68-03** | 重构 4 套 `scripts/build_*.py` 动态绑定 `manifest` 快照并重生成 9 份演说 HTML | ✅ 已完成 | 全套 `presentation/*.html` 同步至 `v1.1.0` / `Bench v3.9` / `W1~W19` / `DQ1~DQ10` / `347+` |
| **P68-04** | 修复文档/Skill 死代码路径、过时版本/测试计数及 `v1.1.0.md` | ✅ 已完成 | `EVOLUTION_GUIDE.md`、`docs/arch/02`、3 SKILL、`README`、`INSTALL`、`03/10`、`v1.1.0.md` 同步 |
| **P68-05** | `make quality-gate` + `clippy` + `cargo test` + Release 重编重启验证 | ✅ 已完成 | 全门禁 `0 drift`、全测试零失败、`/api/v1/parity/matrix` 返回 `100.0%` |
