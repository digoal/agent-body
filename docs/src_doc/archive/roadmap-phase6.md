# Phase 6: 项目结构与文件命名规范化 (Refactor Roadmap & Checkpoints)

> **任务目标**：全面消除中文非 ASCII 路径与大写下划线非标命名，统一采用行业小写中划线（kebab-case）命名规范；完整更新全项目所有引用了被重命名文件的文档、代码与 Skill 链接；清理空目录；跑通 100% 全量自动化测试与死链检查；确保任务在任何意外中断下可瞬间“断点续传”。

---

## 一、子任务分解与执行状态清单

| 子任务编号 | 任务阶段 | 操作对象与说明 | 状态 | 断点标记 |
|---|---|---|:---:|:---:|
| **P6-1** | 断点追踪初始化 | 创建本规划文档 `docs/src_doc/roadmap-phase6.md` | ✅ 已完成 | `CHECKPOINT-INIT` |
| **P6-2** | `docs/idea_doc/` 规范化 | `git mv` 中文非 ASCII 文件至 kebab-case 并更新所有交叉引用 | ✅ 已完成 | `CHECKPOINT-IDEA-DOC` |
| **P6-3** | `docs/design_doc/` 规范化 | `git mv` 大写非标文件至小写 kebab-case 并更新全局文档链接 | ✅ 已完成 | `CHECKPOINT-DESIGN-DOC` |
| **P6-4** | `docs/src_doc/` 规范化 | `git mv` 大写下划线文件至小写 kebab-case 并更新代码与 Skill 引用 | ✅ 已完成 | `CHECKPOINT-SRC-DOC` |
| **P6-5** | 死链与空目录清理 | 清除 `screenshots/` 空目录，运行全局自动化死链扫描审计脚本 (187 引用 100% 校验通过) | ✅ 已完成 | `CHECKPOINT-AUDIT-LINKS` |
| **P6-6** | 全量回归与构建测试 | 运行 `cargo test` (31 测试全部通过) 及 `cargo build --release` | ✅ 已完成 | `CHECKPOINT-TESTS-PASS` |
| **P6-7** | 镜像同步与状态交付 | Git commit 规范化提交，同步更新 `/root/new/shared_git/agent-body` | ✅ 已完成 | `CHECKPOINT-DELIVERY` |

---

## 二、重命名映射对照表

### 1. `docs/idea_doc/` (消除中文与符号)
- `docs/idea_doc/01-agent-Agent 本体-为什么记忆管理没人做对.md` -> `docs/idea_doc/01-why-memory-management-failed.md`
- `docs/idea_doc/02-agent-body-设计全过程.md` -> `docs/idea_doc/02-agent-body-design-process.md`

### 2. `docs/design_doc/` (消除全大写)
- `docs/design_doc/REVIEW.md` -> `docs/design_doc/review.md`
- `docs/design_doc/REVIEW-FIX-PROGRESS.md` -> `docs/design_doc/review-fix-progress.md`
- `docs/design_doc/ALIGNMENT-FIX-PROGRESS.md` -> `docs/design_doc/alignment-fix-progress.md`
- `docs/design_doc/REVIEW-README-ALIGNMENT.md` -> `docs/design_doc/review-readme-alignment.md`
- `docs/design_doc/REVIEW-V2-STATUS-AND-EVOLUTION.md` -> `docs/design_doc/review-v2-status-and-evolution.md`

### 3. `docs/src_doc/` (消除大写与下划线)
- `docs/src_doc/DEVELOPMENT.md` -> `docs/src_doc/development.md`
- `docs/src_doc/HOOKS_AND_INTEGRATION.md` -> `docs/src_doc/hooks-and-integration.md`
- `docs/src_doc/PRODUCT_MANUAL.md` -> `docs/src_doc/product-manual.md`
- `docs/src_doc/PROGRESS.md` -> `docs/src_doc/progress.md`
- `docs/src_doc/ROADMAP_PHASE2.md` -> `docs/src_doc/roadmap-phase2.md`
- `docs/src_doc/ROADMAP_PHASE3.md` -> `docs/src_doc/roadmap-phase3.md`
- `docs/src_doc/ROADMAP_PHASE4.md` -> `docs/src_doc/roadmap-phase4.md`
- `docs/src_doc/ROADMAP_PHASE5.md` -> `docs/src_doc/roadmap-phase5.md`

---

## 三、引用关系与待更新清单预检

1. **`docs/idea_doc/` 引用方**：
   - `docs/src_doc/DEVELOPMENT.md`
   - `docs/design_doc/REVIEW.md`
   - `README.md` (如有)
2. **`docs/design_doc/` 引用方**：
   - `README.md`
   - `docs/src_doc/DEVELOPMENT.md`
   - 各 design 文档相互引用
3. **`docs/src_doc/` 引用方**：
   - `skills/agent-body-developer/SKILL.md`
   - `README.md`
   - `docs/manual/*.md`
   - `src/main.rs` 或其他源码注释 (如有)
