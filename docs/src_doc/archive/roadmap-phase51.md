# Phase 51 演进路线图：根目录核心文件自文档化、规约深化与旗舰双演说 HTML 推翻重写 (ISSUE #34 & #35)

> **对应 ISSUE**: [ISSUE.md #34 & #35](../../ISSUE.md#L164-L166)  
> **创建时间**: 2026-11-22 19:53 CST  
> **完成时间**: 2026-11-22 20:06 CST  
> **当前状态**: ✅ 全部完成（M1~M4 全部 10 项原子任务已验收交付）  
> **关联规约**: README 规约 #11, #13, #14, #17, #20, #27, #28

---

## 一、阶段目标与设计决策

1. **ISSUE #34（根目录核心文件逐项描述与防漂移门禁）**：
   - 将 `README.md`「项目结构」中的根目录核心文件从单行拼接升级为 **15 项逐一登记并附精准职责描述**（补齐此前遗漏的 `.gitignore` 与 `.dockerignore`）；
   - 在 `scripts/evolution_manifest.py --check` 新增根目录核心文件自文档化校验，从机制上杜绝新增/改名根目录文件漏写描述。
2. **README 规约深化升级**：
   - 升级规约 **#17**：确立演说代码化（Decks-as-Code）、架构大跃迁整体重构红线与 Playwright 无头渲染硬门禁（`make verify-decks`）；
   - 升级规约 **#20**：纳入根目录核心文件自文档化红线；
   - 升级规约 **#14**：明确每 Phase 验收收口时同步刷新 `docs/src_doc/progress.md` 与 `EVOLUTION_GUIDE.md`；
   - 升级规约 **#13**：将 `make verify-decks` 纳入演说/前端大屏变更时的强制门禁。
3. **ISSUE #35（两份旗舰 HTML 演说推翻重写 · Decks-as-Code）**：
   - 彻底推翻 Phase 24 旧骨架的 `presentation/presentation.html` 与 `presentation/agent-body-deck.html`，采用 `scripts/deck_engine.py` + `scripts/build_flagship_{roadshow,techdeck}.py` 确定性生成；
   - **`presentation/presentation.html`（16 页 · 对外路演版 · 15~20 分钟）**：围绕「比较优势 → 脑体分离六大器官 → AOPCC 乐观-悲观并发 → 换脑不失忆 → 安全与双域审计 → 内生 AWR 性能工程 → Benchmark v3.7 帕累托双门禁与竞品实测 → 5 分钟上手」重构；
   - **`presentation/agent-body-deck.html`（24 页 · 深度技术交流版 · 45~60 分钟）**：深入剖析 27 模块 / 43 MCP / 82 SPI / 27 表 / `L1~L3` 证据分层图谱 / AOPCC 数学边界 / 新鲜度写穿失效 / `MeshEnvelope v1.0` / `P1~P5` 自进化 / 等待事件+AWR+ADDM / 五重质量门禁与递弱代偿边界；
   - 严格遵循「地层剖面 / 工程蓝图」视觉规范（1600×900、直角内联 SVG、零外部依赖、`G/N/T/F` 快捷键、右侧联动讲稿抽屉、步进动画），并通过 Playwright 无头视觉与交互全绿验真，重编安装重启服务同步更新 Web UI 内嵌演说。

---

## 二、原子任务断点续传追踪表

| 任务 ID | 里程碑 | 原子任务描述 | 状态 | 完成时间 | 验证摘要 |
|---|---|---|---|---|---|
| **P51-01** | M1 | 创建 `roadmap-phase51.md`、升级 `README.md` 规约 `#13/#14/#17/#20`、补齐 `progress.md` Phase 46~51 与 `EVOLUTION_GUIDE.md` Tier-0 入口 | ✅ 已完成 | 2026-11-22 19:55 | 规约条款清晰落地，Tier-0 与总表闭环 |
| **P51-02** | M1 | `README.md` 根目录 15 个核心文件逐项补充描述、校准 2 处计数微漂移，并在 `evolution_manifest.py` 增加根目录文件防漂移校验 | ✅ 已完成 | 2026-11-22 19:56 | 15/15 根文件全登记，`make evolution-check` 0 drift |
| **P51-03** | M2 | 审视 `scripts/deck_engine.py` 并在 `Makefile` 新增 `verify-decks` 目标，保持与 `01~06` 专题 Deck 100% 兼容 | ✅ 已完成 | 2026-11-22 19:56 | `Makefile` 新增 `verify-decks`，零副作用 |
| **P51-04** | M2 | 编写 `scripts/build_flagship_roadshow.py`，推翻重写 16 页对外路演版 `presentation/presentation.html` | ✅ 已完成 | 2026-11-22 20:05 | 16 页全量基于现网真实数据与 Bench v3.7 / AWR |
| **P51-05** | M3 | 编写 `scripts/build_flagship_techdeck.py`，推翻重写 24 页深度技术交流版 `presentation/agent-body-deck.html` | ✅ 已完成 | 2026-11-22 20:05 | 24 页全栈内核剖析 + 步进动画 + 连绵口语讲稿 |
| **P51-06** | M3 | 同步刷新 `scripts/build_decks_part2.py` 与 `presentation/index.html` 的旗舰 Deck 导航卡片、页数与亮点标签 | ✅ 已完成 | 2026-11-22 20:04 | 总览页与新版 16/24 页旗舰 Deck 100% 对齐 |
| **P51-07** | M4 | 运行 `make verify-decks`（Playwright 无头验真 9/9 HTML：0 报错、0 溢出、0 SVG 重叠/越界、快捷键/缩放讲稿全通） | ✅ 已完成 | 2026-11-22 20:06 | 9/9 HTML 全部 PASS（0 error / 0 overflow / 0 overlap） |
| **P51-08** | M4 | 同步刷新 `ISSUE.md`（标绿 `#34/#35`）、`docs/manual/08-web-dashboard.md` 与 `skills/agent-body-*`（含全局镜像） | ✅ 已完成 | 2026-11-22 20:06 | 四层文档与全局 SKILL 镜像一致 |
| **P51-09** | M4 | 执行 `make quality-gate && make test && make restart`，核验 Web 内嵌 `/presentation` & `/deck` 与现网 `DQ5 >= 98%` | ✅ 已完成 | 2026-11-22 20:08 | 门禁/测试/服务热重启/Web 内嵌验真全绿 |
| **P51-10** | M4 | 执行 `make sync-mirror` 同步开源镜像并输出三段式交付小结 | ✅ 已完成 | 2026-11-22 20:08 | 镜像同步完成 |
