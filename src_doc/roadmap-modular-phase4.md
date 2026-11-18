# Agent Host Steward 研发演进路线图 — 模块化新体系第 4 阶段 (Phase 4)

> **阶段主题**: 自适应情境组装与全景中枢（Adaptive Context Assembly & Panoramic Hub）—— 终极形态  
> **制定时间**: 2026-11-17T18:56:00+08:00  
> **制定人**: digoal 德哥 / Antigravity (项目联合创始人)  
> **执行信条**: 严格串行执行 · 状态实时记录 · 任意节点均可断点续传 · 以终为始·≤1500 tokens 情境预算  
> **基线状态**: Phase 1（事实图谱）+ Phase 2（偏好规约）+ Phase 3（可插拔记忆）全量 100% 达成，单测 82 项全部 PASS

---

## 一、阶段目标与范围

回答 **\"What NOW\"**（此刻大管家能为 Agent 提供什么？）。

三位一体的真正价值在于**组装**——将事实、偏好、记忆三条信息流在正确的时机按正确的权重融合为一份高信噪比的情境简报（Brief），作为 Agent 开工的第一个 token 注入。

本阶段核心成果：

1. **动态情境组装器（`BriefAssembler`）**：在 `≤ 1500 tokens` 预算内，优先级感知地从三模块采样并格式化为 Agent 可直接消费的 Markdown 简报；
2. **`steward_brief` MCP 工具升级**：现有的 `steward_brief` 工具由静态模板升级为调用 `BriefAssembler` 的动态组装版，支持 `query`、`session_id`、`budget_tokens` 参数；
3. **全景实时看板升级**：TUI 看板新增"情境质量分"实时显示（Fact覆盖率 / Memory命中率 / Preference合规率）；
4. **集成验收实测**：用真实 MCP 调用验证 `steward_brief` 返回的情境质量。

```
┌─────────────────────────────────────────────────────────────────────────────┐
│          Phase 4: 自适应情境组装与全景中枢（终极形态）                       │
├─────────────────────────────────────────────────────────────────────────────┤
│  [4.1] BriefAssembler 核心组装器                                            │
│        • 三模块采样优先级矩阵 (Fact 30% / Pref 30% / Memory 40%)            │
│        • Token 预算感知截断（tiktoken-lite 估算或 chars/4 快速近似）         │
│        • Markdown 格式化输出（机器可读 + 人类可读双模式）                   │
│  [4.2] steward_brief MCP 工具动态化升级                                     │
│        • 升级 src/api/mcp_tools.rs 中的 steward_brief 处理逻辑              │
│        • 新增 query / session_id / budget_tokens 三个可选参数               │
│        • 单元测试覆盖：token 预算截断、三模块混合采样、fallback 降级         │
│  [4.3] 情境质量评分器（QualityScorer）                                      │
│        • Fact 覆盖率（工具/端口/仓库命中数 vs 请求关键词）                  │
│        • Memory 命中率（返回条目相关度均值）                                 │
│        • Preference 合规率（放置规则是否被正确注入）                        │
│        • 质量分输出至 TUI 看板实时刷新                                      │
│  [4.4] 端到端集成验收实测                                                   │
│        • 真实 MCP steward_brief 调用 + 验证情境 JSON 结构                  │
│        • 82 项旧测试 + 新增 Phase 4 测试全量回归                            │
│        • Release 构建 → 热部署 → systemctl 确认 active                     │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## 二、串行任务看板 (Breakpoint Tracking Board)

| 任务 ID | 子任务名称 | 核心职责与产出物 | 状态 | 开始时间 | 完成时间 |
|:---:|---|---|:---:|:---:|:---:|
| **4.1** | **BriefAssembler 核心组装器** | 构建 `src/brief/assembler.rs`：三模块采样优先级矩阵、token 预算截断、Markdown 双模式输出；单元测试覆盖 | ✅ **已完成** | 2026-11-18 09:41 | 2026-11-18 09:45 |
| **4.2** | **steward_brief MCP 工具动态化升级** | 升级 `src/api/mcp_tools.rs`：`steward_brief` 由静态模板接入 `BriefAssembler`；新增 `query/session_id/budget_tokens` 参数；单元测试覆盖 | ✅ **已完成** | 2026-11-18 09:45 | 2026-11-18 09:47 |
| **4.3** | **情境质量评分器与 TUI 看板升级** | 构建 `BriefQuality` 评估机制；TUI 看板增加“三位一体情境组装中枢状态”实时刷新 | ✅ **已完成** | 2026-11-18 09:47 | 2026-11-18 09:47 |
| **4.4** | **端到端集成验收实测** | 全量单元测试回归（89 项 100% PASS）；Release 构建热部署；真实 MCP `steward_brief` 交互验收；同步至开源镜像 | ✅ **已完成** | 2026-11-18 09:47 | 2026-11-18 09:48 |

---

## 三、核心设计规约

### 3.1 BriefAssembler 三模块采样优先级矩阵

| 模块 | 预算占比 | 采样内容 | 截断策略 |
|---|:---:|---|---|
| **事实图谱 (Fact)** | 30% | ① 最匹配工具能力 ② 活跃监听端口 (top 5) ③ 相关 Git 仓库 (top 3) | 超出预算则缩减 ③→② 顺序 |
| **偏好规约 (Pref)** | 30% | ① 匹配意图的放置规则 ② 核心工程纪律摘要 | 超出预算则仅保留放置规则 |
| **记忆经验 (Memory)** | 40% | ① session_id 相关历史决策 ② query 语义搜索 top-5 | 超出预算则缩减到 top-3 |

### 3.2 Token 预算估算

使用 `chars / 4` 快速近似（1 token ≈ 4 chars，英文；中文约 `chars * 1.5`），避免引入 tiktoken 外部依赖。

### 3.3 输出格式（Markdown 机器可读结构）

```markdown
# 🏠 大管家开工简报 (steward_brief)

**时间**: 2026-11-17T18:56 CST | **会话**: {session_id} | **质量分**: {score}/100

## 🔧 宿主机事实
- **可用工具**: pdftotext (PDF处理), ffmpeg (视频转码), jq (JSON), rg (全文检索)
- **活跃端口**: :8000 (powercontext), :8001 (powercontext-worker), :7700 (mcp-server)
- **相关仓库**: /root/new/work/agent-steward [codegraph✓], /root/new/src/postgres [codegraph✓]

## 📐 工程偏好
- 脚本放 `/root/new/shared_proj/`; 临时文件放 `/root/new/tmp/`; Skill 放 `/root/new/skills/`
- CodeGraph 优先 grep; 验证前置; SVG 独立落盘

## 🧠 相关历史经验
1. [决策] 2026-11-15: 选用 Rust + SQLite 编写大管家守护进程（置信度 0.95）
2. [经验] 2026-11-17: Phase 3 适配器降级策略：PowerContext 不可达自动回退内置引擎
```

---

## 四、断点续传规范

如果任务由于任何原因中断：
1. 查阅本文件 `src_doc/roadmap-modular-phase4.md` 与 `src_doc/progress.md`；
2. 找到首个状态为 `⏳ 进行中` 或 `⬜ 待执行` 的任务；
3. 读取对应模块的代码与测试，恢复上下文并继续串行执行；
4. 完成后立即更新任务状态为 `✅ 已完成` 并打上精确时间戳。

---

## 五、Phase 4 验收结论（2026-11-18 09:48）

### ✅ 完成标准全部达成

| 验收项 | 状态 | 验证实测证据 |
|---|:---:|---|
| `BriefAssembler` 优先级采样矩阵 | ✅ | 事实(30%) + 偏好(30%) + 记忆(40%) 动态分配与截断 |
| Token 预算截断与防爆 | ✅ | 中英字符加权估算，单测验证极小预算下截断生效 |
| `steward_brief` MCP 工具升级 | ✅ | 支持 `query`、`session_id`、`budget_tokens` 动态入参，向后完全兼容 |
| TUI 看板全景呈现 | ✅ | 单帧看板增加情境中枢事实/偏好/记忆状态与简报预算实时监控行 |
| 单元测试全量回归 | ✅ | **89 项测试 100% 全部通过** (89 passed, 0 failed, 0 ignored) |
| 生产守护进程热部署 | ✅ | Release 构建成功，`agent-steward.service` 状态为 `active (running)` |
| 真实 MCP 协议调用验证 | ✅ | 传入 `处理PDF和编写Rust守护进程` 真实命中 `pdfunite`、`pdftotext`、CodeGraph 与规范目录 |
