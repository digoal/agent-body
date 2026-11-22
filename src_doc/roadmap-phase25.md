# Agent Host Steward — Phase 25 演进路线图：Web UI 白天/夜间双色调美化与路演演示剧场故事化重构

> **编制**: Antigravity · **日期**: 2026-11-19  
> **基线 Commit**: `40affe3` (`v0.3.3`, Phase 1~24 全部交付，194 tests passed，39 个原生 MCP 工具黑白屏 100% 同步)  
> **对齐基准**: [ISSUE.md #11 & #12](../ISSUE.md#L65-L71) & [核心设计原则与约束 §1-§17](../README.md#L232-L302)  
> **断点续传检查点表**: [progress-phase25.md](progress-phase25.md)

---

## 一、目标与核心设计规格

### 1. ISSUE #11：Web UI 美化与「☀️ 白天蓝图 / 🌙 夜间深空」双色调切换
- **彻底清零硬编码暗色值**：将 `src/web/assets/mod.rs` 中的散落色值统一收敛到 `:root[data-theme="dark"]`（深空指挥舱）与 `:root[data-theme="light"]`（暖纸工程蓝图，与 `/presentation`、`/deck` 视觉同源）两套语义 CSS Token 体系：
  - **白天蓝图 (`light`)**：`--bg: #F7F6F2`、`--panel: #FFFFFF`、`--panel-2: #F3F1EB`、`--text: #1B2B33`、`--blue: #245A6B`、`--orange: #BE653B`、`--green: #2F7A66`、`--red: #B9382C`、`--canvas-bg: #F4F2EC`、`--canvas-dot: #D5D0C5`；
  - **夜间深空 (`dark`)**：`--bg: #080c14`、`--panel: rgba(17, 24, 39, 0.90)`、`--panel-2: #131d31`、`--text: #f1f5f9`、`--blue: #38bdf8`、`--orange: #fb923c`、`--green: #34d399`、`--red: #f87171`、`--canvas-bg: #070b14`、`--canvas-dot: #1e293b`。
- **一键切换与 Canvas 联动**：顶栏新增 `#btn-theme-toggle` 按钮与快捷键 `T`（非输入焦点时），使用 `localStorage` 持久化主题，并在切换瞬间自动重绘 `#topology-canvas` 节点文字、连边与选中外环。
- **常规工作台去裸 JSON 化**：修正静态版本标示为 `v0.3.4 · 2.0` / `39/39 对齐`，并将端口防撞、Pre-Hook 预检、放置推导、DuckDB 热力图与趋势、宿主 GC 预览等高频交互结果升级为「可视化摘要卡/进度条 + 可折叠原始 JSON」。

### 2. ISSUE #12：Web UI「🎬 路演演示剧场」三层故事化重构
- **破除 JSON 调试台痛点**：废弃主舞台大段 `<pre>JSON.stringify</pre>`，升级为「比较优势总立意导览 + 三层渐进式故事舞台」：
  1. **第一层（人话破题横幅）**：生活化比喻（`analogy`）、业务用意说明（`intent_summary`）、演讲者 30 秒提词卡与 3 大核心收益数字卡（`hero_metrics`）；
  2. **第二层（左右分屏故事对比 + 6 大专属可视化组件）**：左屏呈现盲人 Agent 三步翻车故事线，右屏由真引擎实测数据渲染 6 套场景专属交互图形组件（过滤漏斗+武器库墙、Pre-Hook 1ms 红绿灯、3 跳蝴蝶效应因果链、双 Agent 交叉死锁环路图、Token 99.9% 对比尺、1 秒交接简报评分+自洁护盾）；
  3. **第三层（可折叠技术核验抽屉）**：默认收起 `<details class="raw-json-details">`，供工程师按需核验原始真引擎 JSON 与 CLI/MCP 命令。
- **后端 `DemoEngine` 真实数据修复**：修复 Scene 2 误抓 SSH `:22` 端口并推荐 `:23/:24/:25` 的问题（优先选择 `>= 1024` 活跃应用端口并推荐应用空闲端口），并增强 Scene 3 宿主机真实服务—二进制—端口—Cron 级联依赖链与断链自动修复前后对比。

---

## 二、Phase 25 原子任务清单

| 步骤 ID | 原子任务 | 涉及文件 | 验收标准 |
|:---:|---|---|---|
| **25.1** | 登记 Phase 25 路线图与断点检查点表 | `src_doc/roadmap-phase25.md`, `src_doc/progress-phase25.md` | 路线图、双色调 Token 规范、路演三层结构与检查点表落盘 |
| **25.2** | 升级 `DemoEngine`：补充故事元数据并修复 Scene 2/3 实证数据 | `src/demo/mod.rs` | 新增 `analogy` / `intent_summary` / `hero_metrics`（向后兼容）；Scene 2 探测 `>=1024` 应用端口；Scene 3 含级联链与修复对比；`cargo test demo::tests` 全绿 |
| **25.3** | ISSUE #11：双色调 CSS 变量体系、主题切换、Canvas 联动与面板美化 | `src/web/assets/mod.rs` | 昼夜双主题一键切换与 `localStorage` 记忆、Canvas 自动重绘、版本与对齐标示刷新、高频面板去裸 JSON 化 |
| **25.4** | ISSUE #12：路演演示剧场三层故事舞台与 6 大专属图形组件 | `src/web/assets/mod.rs` | 比较优势导览条、6 幕全部渲染专属图形组件且主舞台无裸 JSON 霸屏、原始 JSON 折叠于 `<details>`、完美适配昼夜双主题 |
| **25.5** | Playwright 真实渲染验收 + 测试回归 + 热部署 + 四层文档同步 | `tests/test_web_dashboard.rs`, `manual/08-web-dashboard.md`, `README.md`, `src_doc/progress.md`, `/root/new/shared_git/agent-steward` | Playwright 昼夜切换与 6 幕图形自检全绿、`make lint-panic` 0 违规、`cargo test` 全绿、Release 热重启与镜像同步完成 |
