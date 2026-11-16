# Agent Host Steward (宿主环境大管家)

> **三位一体架构：宿主机事实图谱为核心护城河 · 用户偏好规约为协作准则 · 记忆模块开放插拔成熟生态**  
> 驻留在 Agent 宿主机中的轻量级系统认知底座（Rust Edition · 单二进制交付）。

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-1.99.0-orange.svg)](https://www.rust-lang.org/)
[![SQLite](https://img.shields.io/badge/SQLite-FTS5_Bundled-green.svg)](https://sqlite.org/)
[![MCP](https://img.shields.io/badge/Protocol-MCP_stdio-purple.svg)](https://modelcontextprotocol.io/)

---

## 什么是 Agent Host Steward？
[产品演示 PPT](https://htmlpreview.github.io/?https://github.com/digoal/agent-steward/blob/main/presentation.html)  
  
市面上有大量专注于“跨会话记忆”或“对话摘要”的开源项目，但它们没有解决一个更底层、更致命的痛点：**Agent 在 Linux 宿主机上干活时，对物理世界是完全“两眼一抹黑”的盲人**：
- 它不知道机器上其实装了 `pdftotext`，却反复折腾 pip 安装第三方库并报错；
- 它不知道 8080 端口已被生产进程占用，直接 bind 导致崩溃；
- 它不知道修改一个通用配置文件，会导致后台依赖它的 systemd 服务挂掉；
- 它不知道哪个 cron 任务在后台定时跑脚本、修改数据库；
- 它不知道用户偏好把脚本放在哪里，随手乱丢文件污染工作区。

**Agent Host Steward（大管家）** 将关注点彻底解耦，确立**三位一体架构**：

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                               AI Agents / Clients                                      │
│             (Antigravity, Cursor, Claude Code, Aider, CLI, Cron Workers)               │
└───────────────────────────────────────────┬────────────────────────────────────────────┘
                                            │ MCP (stdio/SSE) / Unix Socket / CLI
┌───────────────────────────────────────────▼────────────────────────────────────────────┐
│                    Agent Host Steward (宿主环境大管家)                                  │
├────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                        │
│ ┌────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ 模块 1: 宿主机事实图谱 (Host Reality Graph) —— 【核心护城河 · 零幻觉只读体系】       │ │
│ │                                                                                    │ │
│ │ • 工具能力语义注解: 识别 $PATH 工具能力(PDF提取/音视频转码/JSON处理)与推荐调用参数 │ │
│ │ • 运行时进程与端口: 内核 /proc 解析占用(PID/进程名/启动命令/Cwd)，端口防撞预警    │ │
│ │ • 系统自动化依赖拓扑: systemd 配置文件依赖反查、crontab 后台定时偷跑任务排查      │ │
│ │ • 源码与资料库编目: 自动发现 Git 仓库(CodeGraph 状态)、本地权威文档与离线知识库   │ │
│ │ • 变更审计与依赖图: inotify 实时变更流感知、逆向依赖爆炸半径推演                  │ │
│ │                                                                                    │ │
│ │ ── 统一只读查询语义接口: steward_fact_query / steward_port_check / links_impact ──│ │
│ └────────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                        │
│ ┌────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ 模块 2: 用户偏好与规约引擎 (Preference & Governance) —— 【协作灵魂与规约准则】       │ │
│ │                                                                                    │ │
│ │ • 资产放置规则 (Placement): 推荐基准物理路径(/root/new/shared_proj等)、强制命名规范│ │
│ │ • 工程与习惯规约 (Conventions): 优先使用 codegraph 而非 grep、严禁编造数据、北京时区│ │
│ │ • 纠偏自演化学习 (Rule Learner): 从用户人工纠错行为中自动归纳、沉淀新偏好规则      │ │
│ └────────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                        │
│ ┌────────────────────────────────────────────────────────────────────────────────────┐ │
│ │ 模块 3: 跨会话记忆与经验 (Memory & Context) —— 【开放生态可插拔 · 拒绝重复造轮子】 │ │
│ │                                                                                    │ │
│ │ • 通用 Memory Provider SPI 抽象标准                                                │ │
│ │ • 内置极简引擎: Local SQLite FTS5 + 本地向量混合检索 (零网络依赖，开箱即用)         │ │
│ │ • 成熟开源生态桥接器: 一行配置外挂 Mem0 / Letta / PowerContext / Zep 成熟方案       │ │
│ └────────────────────────────────────────────────────────────────────────────────────┘ │
│                                                                                        │
└────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## 典型交互场景 (Zero Hallucination Examples)

### 场景 1：查询机器上的工具能力（问“有没有能处理 PDF 的东西？”）
- **Agent 请求**: `steward_fact_query(type="capability", intent="处理 PDF 提取纯文本")`
- **大管家返回**:
  > “有 `pdftotext`，版本 24.02，绝对路径 `/usr/bin/pdftotext`。**推荐用法**：`pdftotext -layout input.pdf output.txt`（保持原始排版，性能极快，无需 pip 安装 Python 库）。备选工具：`pdftoppm`（转图片）与 Python `pypdf`。”

### 场景 2：端口排障与防撞（问“8080 端口被谁占了？”）
- **Agent 请求**: `steward_port_check(port=8080)`
- **大管家返回**:
  > “8080 端口已被占用！PID: `18493`，进程名: `python3`，启动命令: `uvicorn main:app --port 8080`，工作目录: `/root/new/work/api-service`，已运行 3 天。**建议**：换用 8081 或 8082 端口。”

### 场景 3：资产放置偏好（问“我要写个抓取脚本，该放哪里？”）
- **Agent 请求**: `steward_placement_suggest(type="script", name="sec_crawler", intent="美股财报抓取")`
- **大管家返回**:
  > “依主人规范，复用脚本应放置在 `/root/new/shared_proj/sec_crawler/`；临时文件请放 `/root/new/tmp/`；严禁散落在当前目录。遵循蛇形命名规范。”

---

## 顶层思想与全新演进路线

- 💡 **[03-modular-steward-architecture.md](idea_doc/03-modular-steward-architecture.md)**：大管家三位一体架构设计深度思考（事实、偏好与记忆的分野与统一）
- 🗺️ **[09-modular-roadmap.md](design_doc/09-modular-roadmap.md)**：最新模块化演进路线图（Phase 1 事实图谱护城河 → Phase 2 偏好自演化 → Phase 3 记忆生态插拔 → Phase 4 终极智能中枢）
- 📐 **[05-architecture.md](design_doc/05-architecture.md)**：三模块分层架构与系统规格说明书
- 📖 **[03-feature-spec.md](design_doc/03-feature-spec.md)**：三模块功能规格详细定义

---

## 官方使用手册 (User Manual)

完整的手册发布于 **[`manual/`](manual/)** 目录：

- 🚀 **[01-getting-started.md](manual/01-getting-started.md)**：快速上手、环境依赖准备与首次编译运行
- 🧠 **[02-core-concepts.md](manual/02-core-concepts.md)**：核心理念：大管家定位、黑白名单浅层阻断、放置建议哲学
- 📖 **[03-cli-reference.md](manual/03-cli-reference.md)**：全量 CLI 命令行实操字典（全子命令 + 真实输出）
- 🔌 **[04-mcp-integration.md](manual/04-mcp-integration.md)**：Agent 接入指南：22 个原生 MCP 工具与 Claude/Cursor 配置
- 🔄 **[05-daemon-and-watcher.md](manual/05-daemon-and-watcher.md)**：后台常驻守护服务、inotify 300ms 防抖与 systemd 自启
- ⚡ **[06-advanced-features.md](manual/06-advanced-features.md)**：v3.1 进阶特性：断链智能修复、放置规则自演化、宿主资源 GC、记忆生命周期与 TUI 看板
- 🛠️ **[07-troubleshooting.md](manual/07-troubleshooting.md)**：故障诊断与常见问题 (FAQ)

---

## 快速上手 (Quick Start)

### 1. 编译安装
```bash
# 确保 Rust 工具链生效
export PATH="/root/.cargo/bin:$PATH"

# 编译 release 生产二进制
cargo build --release

# 查看运行状态 (自动完成 ~/.steward 数据初始化)
./target/release/agent-steward status
```

### 2. 常用 CLI 命令一览

```bash
# 1. 开工获取系统硬件、活跃任务、最近资产与长期记忆简报
./target/release/agent-steward brief

# 2. 准备写一个爬虫脚本？先问大管家放哪里
./target/release/agent-steward place suggest --type script --name sec_fetcher --intent "抓取美股财报"

# 3. 准备启动一个测试服务？先探测 8080 端口是否已被占用 (防撞预警)
./target/release/agent-steward env port 8080

# 4. 计划改动公共模块？先推演上下游依赖与多跳爆炸半径
./target/release/agent-steward links impact /root/new/shared_proj/test.py

# 5. 跨会话搜索过去的决策与排障经验 (FTS5 + Ollama 向量混合 RRF 检索)
./target/release/agent-steward memory search "数据库连接池故障应急" --hybrid

# 6. 本次会话完成？清理 /root/new/tmp/ 临时测试文件并沉淀工单纪要进长期记忆
./target/release/agent-steward close session-20261115-01 --clean-temp
```

### 3. Agent MCP 挂载配置

在 Claude Desktop 或 Antigravity MCP 配置文件中添加：
```json
{
  "mcpServers": {
    "agent-steward": {
      "command": "/root/new/work/agent-steward/target/release/agent-steward",
      "args": ["mcp"]
    }
  }
}
```
  
## ISSUE
这里放一些零碎的问题或思考。  
  
1、这个项目的能力是不是应该OS来做，做成OS的必备功能？
  
2、记录事实、memory时也要有取舍，并不是全记下来，什么该记？哪些不该记？策略是什么？
  
3、记录的时机也要考虑清楚，在Agent使用过程中哪个环节加hook? 不在Agent流程中时，又采取什么主动方式进行扫描和事实的编撰？
  
---

## 开源协议

本项目采用 [Apache-2.0](LICENSE) 开源许可证。
