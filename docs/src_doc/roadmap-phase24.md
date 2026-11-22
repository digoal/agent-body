# Agent Host Steward — Phase 24 演进路线图：比较优势思想源起与双路演 HTML「地层剖面 / 工程蓝图」重构

> **编制**: Antigravity · **日期**: 2026-11-19  
> **基线 Commit**: `8a3e300` (`v0.3.3`, Phase 1~23 全部交付，194 tests passed，39 个原生 MCP 工具黑白屏 100% 同步)  
> **对齐基准**: [README.md ISSUE #9 & #10](../../README.md#L309-L342) & [核心设计原则与约束 §1-§17](../../README.md#L206-L279)  
> **断点续传检查点表**: [progress-phase24.md](progress-phase24.md)

---

## 一、目标与核心交付物

1. **ISSUE #9（“比较优势”立意源起与 PostgreSQL 调研痛点补完）**：
   - 新建 [docs/idea_doc/00-comparative-advantage-origin.md](../idea_doc/00-comparative-advantage-origin.md) 与配套工程蓝图 [docs/idea_doc/svg/comparative-advantage-steward.svg](../idea_doc/svg/comparative-advantage-steward.svg)，从“硅谷 101 负一楼创业者与园区物业、高管秘书、家庭分工”三组隐喻提炼大卫·李嘉图「比较优势」原理，结合宿主机 `/root/new/src/`（110 个仓库、17+ 个 PG 源码树）实证“先问小秘书（大管家）”省时间、省空间、省 Token 的核心价值；
   - 有机融入 [docs/idea_doc/01-why-memory-management-failed.md](../idea_doc/01-why-memory-management-failed.md) 与 [docs/idea_doc/03-modular-steward-architecture.md](../idea_doc/03-modular-steward-architecture.md)，保持行文浑然一体，零拼凑感。
2. **ISSUE #10（双 HTML 演说文件重构、讲稿联动与自动化自检）**：
   - 升级 [steward-deck.html](../steward-deck.html) 为 **22 页深度技术演讲版**：新增「比较优势与 PG 调研痛点」「三层语义元数据与 Ollama 双轨标注」两页，修正 `v0.3.3` / `39 个原生 MCP 工具` 全量一致性，22 页讲稿与页码严格对齐；
   - 全新重构 [presentation.html](../presentation.html) 为 **16 页对外路演版**：严格遵循「地层剖面 / 工程蓝图」浅色 `1600×900` 固定舞台规范（`0` emoji、`0` 圆角卡片堆叠、`0` 外部依赖），全套内联直角 SVG + 6 页分步动画 + 16 页德哥口播级连绵讲稿 + 页内右侧开窗矢量缩放联动（`G/N/T/P/F/A/S/R/?`）。
3. **真实渲染自检与单二进制热更新**：
   - 使用本机 Headless Chrome + Playwright 对两份 HTML 执行几何边界检测（SVG 零出界/零重叠/零交叉）与全交互快捷键/动画/缩放/定位测试；
   - 重新编译 `cargo build --release` 嵌入最新 HTML 并 `make restart` 热更新 `/deck` 与 `/presentation`，同步更新 `README.md`、`docs/src_doc/progress.md` 与 `/root/new/shared_git/agent-steward`。

---

## 二、Phase 24 原子任务清单

| 步骤 ID | 原子任务 | 涉及文件 | 验收标准 |
|:---:|---|---|---|
| **24.1** | 登记 Phase 24 路线图与断点检查点表 | `docs/src_doc/roadmap-phase24.md`, `docs/src_doc/progress-phase24.md` | 路线图与检查点表落盘，支持断点续传 |
| **24.2** | ISSUE #9：比较优势起源文档 + SVG + Idea 文档融通 | `docs/idea_doc/00-comparative-advantage-origin.md`, `docs/idea_doc/svg/comparative-advantage-steward.svg`, `docs/idea_doc/01-*.md`, `docs/idea_doc/03-*.md` | 逻辑自然融入、SVG 零越界、相对路径正确 |
| **24.3** | ISSUE #9 & #10：升级 `steward-deck.html`（22 页深度演讲版） | `steward-deck.html` | 22 页完整、`v0.3.3` / `39 工具` 全对齐、讲稿同步 |
| **24.4** | ISSUE #9 & #10：全新重构 `presentation.html`（16 页路演版） | `presentation.html` | 16 页地层剖面规范、内联正交 SVG、6 页动画、16 页讲稿与快捷键全通 |
| **24.5** | Headless Chrome 全功能自检 + 编译回归 + 热部署 + 镜像同步 | `README.md`, `docs/src_doc/progress.md`, `target/release/agent-steward`, `/root/new/shared_git/agent-steward` | Playwright 几何与交互测试全绿、`cargo test` 全绿、`make lint-panic` 0 违规、热部署与镜像完成 |
