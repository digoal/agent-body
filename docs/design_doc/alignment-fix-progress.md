# README 对齐与设计修复进度表（支持断点续传）

> 创建时间: 2026-11-15T11:57:00+08:00  
> 当前状态: 执行中  
> 目标: 串行完成 1 (场景与需求补正), 2 (功能规格与 API 补正), 3 (数据模型与 SQL 落地验证)

---

## 阶段规划与子任务状态

| 阶段 | 子任务 ID | 任务内容 | 涉及文件 | 状态 | 完成时间 |
|------|-----------|----------|----------|------|----------|
| **阶段 1** | 1.1 | 建立断点续传进度追踪文档 | `docs/design_doc/alignment-fix-progress.md` | ✅ 已完成 | 2026-11-15 11:57 |
| | 1.2 | 更新场景集：补充 S-27(放置建议), S-28(白黑名单颗粒度), S-29(动态进程与并发任务) | `docs/design_doc/01-scenarios.md` | ✅ 已完成 | 2026-11-15 11:57 |
| | 1.3 | 更新需求分析：新增 FP(放置建议需求)与 FA/FE/FC 颗粒度与元数据需求 | `docs/design_doc/02-requirements.md` | ✅ 已完成 | 2026-11-15 11:58 |
| **阶段 2** | 2.1 | 更新功能规格：恢复 directory 类型，增加 Placement 引擎与三级颗粒度扫描规范 | `docs/design_doc/03-feature-spec.md` | ✅ 已完成 | 2026-11-15 12:00 |
| | 2.2 | 更新 API 规范：新增 `body_placement_suggest`, 升级 `body_asset_scan/search`, 新增 `body_process_list` | `docs/design_doc/06-api-spec.md` | ✅ 已完成 | 2026-11-15 12:01 |
| | 2.3 | 同步更新架构与产品手册：补充 Placement 架构与 CLI/配置文档 | `docs/design_doc/04-product-manual.md`, `docs/design_doc/05-architecture.md` | ✅ 已完成 | 2026-11-15 12:02 |
| **阶段 3** | 3.1 | 更新数据模型文档：升级 assets、新增 placement_rules/scan_policies/active_processes | `docs/design_doc/07-data-model.md` | ✅ 已完成 | 2026-11-15 12:03 |
| | 3.2 | 更新 init.sql：写入完整 DDL、触发器及预置白黑名单与放置规则 | `docs/design_doc/init.sql` | ✅ 已完成 | 2026-11-15 12:04 |
| | 3.3 | 本地运行并验证 init.sql：确保语法、触发器、初始化数据 100% 通过 | 测试数据库验证 | ✅ 已完成 | 2026-11-15 12:04 |
| | 3.4 | 同步 README.md 交付物清单与最终验收复盘 | `README.md` | ✅ 已完成 | 2026-11-15 12:05 |

---

## 验收结论

✅ **1, 2, 3 全部阶段共 10 个子任务已 100% 串行完成**！
- 补全了“新对象放置建议（Placement Engine）”全链路（场景 S-27、需求 FP-01~04、模块规格、MCP API、`placement_rules` 表及预置数据）；
- 落地了“白黑名单颗粒度分级机制（Granularity Tiering）”（场景 S-28、需求 FA-12、三级扫描策略、`scan_policies` 表、黑名单目录止于当前层级绝不下钻）；
- 完善了对象元数据 Schema（`directory` 一等公民、`owner`、`security_level`、`desc_kv`、全文 FTS5 触发器同步）；
- 纳管了动态任务进程（场景 S-29、需求 FE-12、`active_processes` 表与 API）；
- 本地 Python + SQLite 环境全量跑通 `init.sql`，表、触发器、FTS 搜索及预置数据验证无误。

---

## 断点恢复指引
如果任务中断，重新启动时只需：
1. 读取本文件 `docs/design_doc/alignment-fix-progress.md`；
2. 找到首个状态为 `⏳ 进行中` 或 `⬜ 待执行` 的子任务 ID；
3. 继续按顺序串行执行，每完成一步立即更新本文件的状态与完成时间戳。
