# Agent Body — Phase 46 演进路线图：体系化 30 分钟演讲矩阵与老 Deck 终刷新（ISSUE #28）

> **编制**: Antigravity · **日期**: 2026-11-22 (Asia/Shanghai)  
> **基线版本**: `v0.4.0`（Commit `752af9a` · 27 模块 · 43 MCP 工具 · 82 SPI 方法 · 27 张表 · 268 项测试全绿 · Benchmark `v3.7` 全绿基线 `run-6a0a4995-7302-4bc1-86b6-554a784a6156`）  
> **对齐约束**: [ISSUE.md #28](../../ISSUE.md) · [README.md](../../README.md) · [docs/design_doc/17-agent-embodiment-and-continuity-spec.md](../design_doc/17-agent-embodiment-and-continuity-spec.md) · [docs/src_doc/roadmap-phase44-45.md](roadmap-phase44-45.md)

---

## 一、核心目标与视觉/内容红线

1. **目录归拢**：新建根目录 `presentation/`，使用 `git mv` 将 `agent-body-deck.html` 与 `presentation.html` 迁入，并同步更新 `src/web/assets/mod.rs`、`README.md`、`docs/manual/08-web-dashboard.md` 与 `EVOLUTION_GUIDE.md` 中的引用。
2. **视觉风格 100% 统一**：所有新增演讲 HTML 严格沿用现有 `agent-body-deck.html` / `presentation.html` 的 **「浅色纸基 `#F7F6F2` · 地层剖面 · 工程蓝图」** 设计系统（`1600×900` 固定比例舞台、直角正交内联 SVG、零外部依赖、零 Emoji、底部 `#ctrl` 控制条、右侧 `N/T` 联动讲稿抽屉、`G` 目录、`P/A/S/R` 步进动画、`F` 全屏、`?` 帮助）。
3. **30 分钟单点说透体系化矩阵**：新增 **6 份专题演讲 HTML（每份 14 页，单讲约 30 分钟，每讲只打穿一个核心专题并附全量口播级讲稿）** + **1 份系列总览导航页 `presentation/index.html`**。
4. **旧版双 Deck 终刷新**：将迁入后的 `presentation/presentation.html`（16 页路演版）与 `presentation/agent-body-deck.html`（22 页深度交流版）刷新至当前最新状态（脑体分离 · Agent 本体 · 27 模块 · 43 工具 · 82 SPI · 268 测试 · 7 大剧目 · Benchmark `v3.7` `DQ1~DQ8` 全绿实测）。
5. **自动化视觉门禁**：编写 `scripts/verify_presentations.py`，使用本机 Playwright + Chromium 对 `presentation/` 下全部 9 个 HTML 逐页校验 **0 控制台错误、0 容器溢出、0 SVG 文字越界、0 SVG 文字重叠、交互键全通**。

---

## 二、交付物清单与进度追踪

| 编号 | 交付文件 | 定位与核心专题 | 预计页数 / 时长 | 状态 |
|:---:|---|---|:---:|:---:|
| **46.1** | `presentation/{agent-body-deck,presentation}.html` (`git mv`) + `src/web/assets/mod.rs` | 迁入既有双 Deck 并修复编译内嵌路径 | — | ✅ 已完成 |
| **46.2** | `presentation/01-brain-body-separation.html` | 第 1 讲：脑体分离与 Agent 本体哲学 | 14 页 / 30 min | ✅ 已完成 |
| **46.3** | `presentation/02-host-reality-graph.html` | 第 2 讲：宿主机物理感官图谱与零污染扫盘 | 14 页 / 30 min | ✅ 已完成 |
| **46.4** | `presentation/03-cerebellum-and-aopcc.html` | 第 3 讲：小脑反射、痛觉门禁与自适应并发控制 AOPCC | 13 页 / 30 min | ✅ 已完成 |
| **46.5** | `presentation/04-hippocampus-freshness-continuity.html` | 第 4 讲：海马体、信息新鲜度治理与跨大脑 1ms 接力 | 13 页 / 30 min | ✅ 已完成 |
| **46.6** | `presentation/05-security-audit-evolution-mesh.html` | 第 5 讲：纵深安全、双域审计与探针驱动自进化 | 13 页 / 30 min | ✅ 已完成 |
| **46.7** | `presentation/06-empirical-benchmark-engineering.html` | 第 6 讲：三轨实证 Benchmark v3.7 与可复现工程体系 | 13 页 / 30 min | ✅ 已完成 |
| **46.8** | `presentation/index.html` + 旧版双 Deck 终刷新 | 系列导航总览页 + 刷新 `presentation.html` & `agent-body-deck.html` | 1 页 + 16/22 页 | ✅ 已完成 |
| **46.9** | `scripts/verify_presentations.py` + 全链路验收 | Playwright 9/9 全页视觉门禁 + 文档/构建/服务重启/镜像同步 | — | ✅ 已完成 |
