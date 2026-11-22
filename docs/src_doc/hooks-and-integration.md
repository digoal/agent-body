# Agent Host Steward — Agent 接入与 Hook 机制配置指南

> 版本: v0.4.0 · 适用对象: Antigravity, Cursor, Claude Desktop, Aider 等 Agent 框架  
> 存放路径: `docs/src_doc/hooks-and-integration.md`  
> 更新时间: 2026-11-22 · 完整 Hook 集成与 AOPCC 门禁指南请同时参阅 [docs/manual/09-hooks-integration.md](../manual/09-hooks-integration.md)

---

## 一、配置文件机制 (`config.toml`)

### 1. 配置文件位置
* **默认路径**: `~/.steward/config.toml`
* **自定义路径**:
  * 环境变量指定: `export STEWARD_CONFIG="/path/to/custom_config.toml"`
  * 命令行指定: `steward --config /path/to/custom_config.toml <command>`

### 2. 自动自举生成
首次运行任何 steward 命令时，若配置文件不存在，Steward 会自动创建 `~/.steward/` 目录并生成包含合理默认值的 `config.toml`：

```toml
# 基础运行环境
data_dir = "/root/.steward"
db_path = "/root/.steward/steward.db"
api_port = 9527
api_bind = "127.0.0.1"

[placement]
default_owner = "user:digoal"
default_security_level = "internal"

[assets]
# 自动纳入主动扫描的目录列表
auto_scan_dirs = ["/root/new/work", "/root/new/shared_proj"]
ignore_patterns = [".git", "node_modules", "__pycache__", ".steward", "target", ".venv"]

[watcher]
# inotify 文件系统实时监听目录
watch_dirs = ["/root/new/work", "/root/new/shared_proj"]
debounce_ms = 300

[scheduler]
# 定时主动巡检周期
process_scan_interval_secs = 60
asset_health_cron = "0 4 * * 0"

[embedding]
ollama_url = "http://localhost:11434"
model = "nomic-embed-text"
dimensions = 768
fallback_fts_only = true
```

---

## 二、大管家服务形态：常驻 Daemon 守护进程

Agent Host Steward 本质是一个**本地常驻服务**：
1. **状态单点与并发一致性**：作为 SQLite 本地数据库的唯一写入者（WAL 模式），保证高并发读写安全。
2. **动态对象与环境持续感知**：独立后台轮询 `/proc` 进程状态，秒级发现僵死脚本与高负载进程。
3. **基于 inotify 的被动感知**：实时监听文件系统变更，自动增量建档，无需用户或 Agent 手动刷新。

### 1. 运行与托管方式

#### 方式 A：前台运行（调试模式）
```bash
steward daemon
```

#### 方式 B：Systemd 用户服务托管（生产推荐）
创建 `~/.config/systemd/user/steward.service`：
```ini
[Unit]
Description=Agent Host Steward Daemon
After=network.target

[Service]
Type=simple
ExecStart=/root/new/work/agent-steward/target/release/agent-steward daemon
Restart=always
RestartSec=5
Environment=PATH=/root/.cargo/bin:/usr/local/bin:/usr/bin

[Install]
WantedBy=default.target
```
启动并设为开机自启：
```bash
systemctl --user daemon-reload
systemctl --user enable --now steward
```

---

## 三、主动与被动扫描双模工作原理

```
                           ┌────────────────────────────┐
                           │   Agent Host Steward       │
                           └──────────────┬─────────────┘
                                          │
                  ┌───────────────────────┴───────────────────────┐
                  ▼                                               ▼
         【被动感知模式 (Passive)】                       【主动扫描模式 (Active)】
                  │                                               │
    ┌─────────────┴─────────────┐                   ┌─────────────┴─────────────┐
    │                           │                   │                           │
1. inotify 实时监听         2. Agent API 显式注册   1. CLI/MCP 手动触发        2. Scheduler 定时巡检
- 核心工作区变更监听        - Agent 调 register     - steward asset scan        - 每 60s 扫 /proc 进程
- 300ms 防抖更新索引        - 带业务意图与 desc_kv  - 指定目录分级扫描          - 每日环境健康扫描
- 自动写入 changes 表       - 精准绑定所有者与密级  - 白名单深挖/黑名单止于当层 - 孤儿文件与陈旧项目识别
```

---

## 四、Agent 客户端接入指南 (MCP 原生)

任何支持 MCP 协议的 Agent 客户端均可一键接入 Steward。

### 1. Antigravity / Cursor 配置 (`mcpSettings.json`)
在配置目录中添加：
```json
{
  "mcpServers": {
    "host-steward": {
      "command": "/root/new/work/agent-steward/target/release/agent-steward",
      "args": ["mcp"],
      "env": {
        "STEWARD_CONFIG": "/root/.steward/config.toml"
      }
    }
  }
}
```

### 2. Claude Desktop 配置 (`claude_desktop_config.json`)
```json
{
  "mcpServers": {
    "host-steward": {
      "command": "/root/new/work/agent-steward/target/release/agent-steward",
      "args": ["mcp"]
    }
  }
}
```

---

## 五、三层 Hook 机制：让 Agent 在正确时机主动调用

如何防止 Agent“失忆”或“随手乱建文件”？通过三层递进的 Hook 体系：

### 第一层：System Prompt / Rules Hook（规则级触发，最简单、零侵入）

在 Agent 的全局指令（如 `.cursorrules`、`CLAUDE.md`、Antigravity 全局 rule）中注入以下规范：

```markdown
## 宿主环境大管家 (Host Steward) 交互准则

你是驻留在宿主机中的智能 Agent，宿主机上运行着 Agent Host Steward 大管家。你必须严格遵守以下三大时机 Hook：

1. 【开局必调 — Session Start Hook】
   - 在新会话开始、或接手任何复杂工程任务前，你的第一个动作必须是调用 `steward_brief`。
   - 借此清晰掌握机器硬件规格、当前正在运行的后台脚本（防冲突）、最近活跃的资产与关键架构记忆。

2. 【落盘先行 — Pre-Creation Placement Hook】
   - 在准备创建任何新项目目录、独立工具脚本、测试数据集或记录重要笔记前，**绝对严禁直接在当前目录乱建文件**！
   - 必须先调用 `steward_placement_suggest(type, name, intent, security_level)`。
   - 严格在 Steward 返回的建议路径下创建文件，并遵守附带的命名约定与安全脱敏指引。

3. 【决策固化 — Post-Decision Hook】
   - 当任务中发生关键架构决策（如选型、调优参数、环境变更）时，在输出最终结论后，主动调用 `steward_memory_add` 将决策固化为长期记忆。
```

---

### 第二层：Agent 客户端生命周期 Hook（框架级拦截）

在支持执行脚本 Hook 的 Agent 框架中：

#### 1. 会话启动前置注入 (Session Startup Hook)
在每次 Agent 会话启动前，自动执行并预注入上下文：
```bash
#!/bin/bash
# ~/.steward/hooks/session_start.sh
# 自动生成当前环境上下文并注入 prompt
steward brief
```

#### 2. 工具调用前置与后置自愈 Hook (PreToolUse / PostToolUse · AOPCC)
通过 `agent-steward hooks generate --client agy` 生成并激活的 `steward_bridge.py` 集成：
* **PreToolUse（`agent-steward hooks pre`）**：`ConcurrencyPolicyClassifier` 对可逆只读/客户端直连（如持有开工乐观租约的 `:5432`、`curl`）零阻塞放行；对端口 bind、高爆炸半径文件写、系统敏感路径及多 Agent 排他锁执行悲观前置拦截；
* **PostToolUse（`agent-steward hooks post`）**：写操作成功后自动登记资产与刷新依赖拓扑；当 `run_command` 等工具失败（`--success=false --error-output "..."`）时，`OptimisticFailureDiagnoser` 在 `< 2ms` 内反查宿主机真实端口/工具/服务状态并注入替代处方。

---

### 第三层：系统级与 Git Hook（开发生态自动化）

#### 1. Git Post-Commit Hook
在代码提交后，自动触发大管家增量更新：
在仓库 `.git/hooks/post-commit` 中添加：
```bash
#!/bin/sh
steward asset scan $(pwd) >/dev/null 2>&1 &
```

#### 2. 定时任务 Cron Hook
在编写由系统或 Hermes 调度的定时任务脚本时，在头部与尾部植入状态打点：
```python
# 任务开始时告知 Steward
import urllib.request
# 或通过命令行打点
import os
os.system("steward env sync")
```
