# Agent Host Steward — 产品设计工程
  
> 宿主环境大管家：驻留在 Agent 宿主机中，管记忆、管素材、管软件环境、管一切。  
  
这不是为让 Agent 做好某一次任务、一次协作而生的产品，而是让 Agent 更清晰的了解自己所处的环境，更好的完成任务和协作。  
  
可以把它理解成 Agent 所处的宿主机上的一切动态与静态对象（文件、进程、文件夹等）元数据 + 对象位置的大管家，甚至还应该包括建议（例如，如果会产生一个新的项目、文件，这些对象应该放哪里）。  
  
管理对象也应该有颗粒度、有白名单黑名单的设计，不应该一股脑全生成对象元数据（语义(可采用 desc kv/json 存储，至少应该知道它们是干什么的、什么时候创建的、修改的、谁的、保密级别等等)、全文检索、bm25、标量、图），例如白名单包含的文件夹应该细到最深层，黑名单则只到当前目录层级。  
  
## idea 
- [为什么记忆管理没人做对](idea_doc/01-agent-环境大管家-为什么记忆管理没人做对.md)
- [agent steward 产品设计](idea_doc/02-agent-steward-设计全过程.md)

## 交付件清单与进度

| # | 交付件 | 文件 | 状态 | 更新时间 |
|---|--------|------|------|----------|
| 1 | 使用场景集 | [01-scenarios.md](design_doc/01-scenarios.md) | ✅ 完成 | 2026-11-15 |
| 2 | 需求分析 | [02-requirements.md](design_doc/02-requirements.md) | ✅ 完成 | 2026-11-15 |
| 3 | 功能清单 | [03-feature-spec.md](design_doc/03-feature-spec.md) | ✅ 完成 | 2026-11-15 |
| 4 | 产品手册 | [04-product-manual.md](design_doc/04-product-manual.md) | ✅ 完成 | 2026-11-15 |
| 5 | 架构设计 | [05-architecture.md](design_doc/05-architecture.md) | ✅ 完成 | 2026-11-15 |
| 6 | 接口规范 | [06-api-spec.md](design_doc/06-api-spec.md) | ✅ 完成 | 2026-11-15 |
| 7 | 数据模型 | [07-data-model.md](design_doc/07-data-model.md) | ✅ 完成 | 2026-11-15 |
| — | Review 报告 | [REVIEW.md](design_doc/REVIEW.md) | ✅ 完成 | 2026-11-15 |
| — | Review 修复进度 | [REVIEW-FIX-PROGRESS.md](design_doc/REVIEW-FIX-PROGRESS.md) | ✅ 14/14 全部修复 | 2026-11-15 |
| — | 初始化 SQL | [init.sql](design_doc/init.sql) | ✅ 已验证 | 2026-11-15 |

## 设计原则
1. **本地优先、隐私优先** — 所有数据存本地，不外传
2. **渐进式接入** — 每层独立可用，组合更强
3. **被动 + 主动双模** — API 注册 + 自动感知
4. **关联是核心价值** — 记忆×资产×环境×变更 四维关联
5. **MCP 原生** — 天然对接所有 Agent 框架
