# 03 — 全量 CLI 命令行实战字典（CLI Reference）

> **章节目标**: 提供 Agent Host Steward 所有命令行子命令、参数选项、使用场景与真实执行输出的标准参考手册。

---

## 1. CLI 命令总览

```bash
agent-steward [OPTIONS] <COMMAND>
```

| 顶级子命令 | 简要说明 | 适用场景 |
|---|---|---|
| `status` | 查看大管家自身版本、数据库位置、纳管资产与进程统计 | 状态巡检与探活 |
| `brief` | 一键获取宿主系统硬件、活跃任务、最近资产与记忆简报 | Agent 开工首调 |
| `place` | 咨询对象放置规范建议或列出放置规则表 | 新建文件/工程前防乱放 |
| `asset` | 资产分级扫描录入、全文检索与列表查看 | 资产盘点与检索 |
| `env` | 硬件规格、活跃动态任务进程、网络端口防撞检测 | 运维排障、资源防撞 |
| `memory` | 跨会话长期记忆写入、全文与向量混合 RRF 检索 | 知识与决策沉淀 |
| `links` | 查询实体依赖拓扑、导出 Mermaid 架构图、推演爆炸半径 | 改动代码前影响分析 |
| `changes`| 查看自某个时间点以来的宿主环境变更历史 | 开工前了解环境变动 |
| `close` | 会话完工结算、清理临时测试资产、归档沉淀长期记忆 | 会话结束自洁 |
| `mcp` | 启动标准 JSON-RPC 2.0 stdio MCP 服务端 | 供 Agent 客户端挂载 |
| `daemon` | 启动常驻后台守护主循环 (inotify 监听 + 60s 定时巡检) | 系统服务守护 |

---

## 2. 详细子命令与实操指南

### 2.1 `status` — 系统状态与统计
- **说明**: 显示当前大管家的基础运行信息与数据库统计。
- **示例**:
  ```bash
  agent-steward status
  ```
- **真实输出**:
  ```text
  ╔═══════════════════════════════════════════════════════╗
  ║      Agent Host Steward (Rust 版) — 系统状态          ║
  ╚═══════════════════════════════════════════════════════╝
  版本:           v0.2.0
  数据目录:       "/root/.steward"
  数据库文件:     "/root/.steward/steward.db"
  已纳管资产数:   0
  放置规范规则:   8 条
  分级扫描策略:   10 条
  当前动态进程:   198 个
  ```

---

### 2.2 `brief` — 宿主环境开工简报
- **说明**: 聚合硬件、动态进程、资产与长期记忆，生成 Agent 开工即食的全局上下文。
- **示例**:
  ```bash
  agent-steward brief
  ```

---

### 2.3 `place` — 规范放置建议
提供两大子命令：`suggest` 与 `rules`。

#### (1) `place suggest`
- **参数**:
  - `-t, --type <TYPE>`: 对象类型（`project`, `script`, `skill`, `note`, `tmp`, `src`）
  - `-n, --name <NAME>`: 对象名称
  - `-i, --intent <INTENT>`: 对象用途与预期生命周期
  - `-s, --security <LEVEL>`: 保密级别（默认 `internal`，可选 `public`, `confidential`, `secret`）
- **示例**:
  ```bash
  agent-steward place suggest --type script --name sec_scanner --intent "抓取美股财报"
  ```
- **真实输出**:
  ```text
  ╔═══════════════════════════════════════════════════════╗
  ║             Steward 对象放置与路径建议                ║
  ╚═══════════════════════════════════════════════════════╝
  推荐目标路径:   "/root/new/shared_proj/sec_scanner/"
  命中规范规则:   shared_tools
  规则理由:       长期沉淀的可复用脚本与程序库
  命名建议:       推荐命名格式: {name}/
  冲突状态:       ✅ 路径空闲可用
  伴随开发指引:
    * 建立独立子目录，配备 README.md 与入口脚本，严禁硬编码敏感信息
    * 工程规范: 建议在同目录下配备 README.md 说明用途、依赖库及执行示例。
  ```

#### (2) `place rules`
- **说明**: 查看大管家内置的 8 大宿主机目录放置规范字典。
- **示例**:
  ```bash
  agent-steward place rules
  ```

---

### 2.4 `env` — 系统环境与进程管理
提供 `show`, `processes`, `port` 三大子命令。

#### (1) `env show`
- **说明**: 查看 CPU 核心与型号、总内存/已用内存、磁盘分区容量与可用空间。
- **示例**:
  ```bash
  agent-steward env show
  ```
- **真实输出**:
  ```text
  === 系统与硬件规格 ===
  OS:       Linux 13 Debian GNU/Linux
  Kernel:   6.12.107+deb13-amd64
  Host:     digoal-home
  CPU:      8 核心 (Intel(R) Core(TM) i7-8550U CPU @ 1.80GHz)
  Memory:   15642 MB 总量 (已用: 3184 MB, 20.4%)
  Disks:
    - /          总计 232.7 GB / 可用 81.1 GB
    - /boot/efi  总计 1.0 GB / 可用 0.9 GB
  ```

#### (2) `env processes`
- **说明**: 智能过滤内核底噪，列出当前由用户或 Agent 运行的普通动态任务进程。
- **示例**:
  ```bash
  agent-steward env processes
  ```

#### (3) `env port <PORT>`（防撞预警）
- **参数**:
  - `<PORT>`: 目标网络端口号（1-65535）
  - `-s, --suggest-count <N>`: 若冲突推荐的空闲端口数（默认 3）
- **示例 1（冲突场景）**:
  ```bash
  agent-steward env port 22
  ```
  ```json
  {
    "port": 22,
    "status": "OCCUPIED",
    "conflict": {
      "pid": 957,
      "name": "sshd",
      "command": "sshd: /usr/sbin/sshd -D [listener] 0 of 10-100 startups",
      "owner": "root"
    },
    "recommended_free_ports": [23, 24, 25],
    "advice": "⚠️ 端口 22 已被进程 'sshd' (PID: 957, 用户: root) 占用，命令行: sshd: /usr/sbin/sshd -D [listener] 0 of 10-100 startups。为避免服务冲突与踩踏，建议改用推荐空闲端口: [23, 24, 25]"
  }
  ```
- **示例 2（空闲场景）**:
  ```bash
  agent-steward env port 8999
  ```
  ```json
  {
    "port": 8999,
    "status": "FREE",
    "conflict": null,
    "recommended_free_ports": [],
    "advice": "✅ 端口 8999 处于空闲可用状态，可以安全绑定启动服务。"
  }
  ```

---

### 2.5 `memory` — 跨会话长期记忆
提供 `add` 与 `search` 子命令。

#### (1) `memory add`
- **参数**:
  - `-t, --type <TYPE>`: 类型（`decision`, `task`, `experience`, `preference`, `fact`, `note`）
  - `-c, --content <TEXT>`: 记忆正文
  - `-s, --summary <TEXT>`: 一句话摘要
- **示例**:
  ```bash
  agent-steward memory add -c "决定选用 Rust 编写核心守护进程" -s "选型决策"
  ```

#### (2) `memory search`
- **参数**:
  - `<QUERY>`: 搜索关键词或语义描述
  - `-l, --limit <N>`: 限制返回条数（默认 10）
  - `--hybrid`: 是否启用 Ollama 向量 + FTS5 混合 RRF 检索（默认为 true）
- **示例**:
  ```bash
  agent-steward memory search "Rust 选型" --hybrid
  ```
- **真实输出**:
  ```text
  记忆搜索结果 (混合模式 RRF, 共 1 条):
    - [decision] 决定选用 Rust 1.99 与 SQLite 编写 Agent Host Steward 核心进程 [RRF: 0.0164] (ID: mem-82ecbb7b-6e9d-4ab2-bf6a-d596561299a6)
  ```

---

### 2.6 `links` — 关联依赖与爆炸半径
提供 `query`, `graph`, `impact` 三大子命令。

#### (1) `links graph`
- **说明**: 输出 Mermaid 格式的实体拓扑关系图。
- **示例**:
  ```bash
  agent-steward links graph
  ```

#### (2) `links impact <TARGET>`（爆炸半径推演）
- **说明**: 输入资产路径，BFS 递归计算直接依赖与多跳间接下游。
- **示例**:
  ```bash
  agent-steward links impact /root/new/shared_proj/test.py
  ```
- **真实输出**:
  ```json
  {
    "target": "/root/new/shared_proj/test.py",
    "risk_level": "LOW",
    "direct_dependents": [],
    "indirect_dependents": [],
    "total_dependents_count": 0,
    "summary": "✅ 未检测到任何上下游资产或服务依赖 '/root/new/shared_proj/test.py'。改动或删除此对象爆炸半径极小 (Risk: LOW)。"
  }
  ```

---

### 2.7 `changes` — 宿主环境变更回溯
- **参数**:
  - `-s, --since <TIMESTAMP>`: 起始时间戳（ISO 8601，如 `2026-11-15T00:00:00`）
  - `-l, --limit <N>`: 返回条数（默认 20）
- **示例**:
  ```bash
  agent-steward changes
  ```

---

### 2.8 `close` — 会话完工结算与垃圾回收
- **参数**:
  - `<SESSION_ID>`: 当前会话 ID
  - `--clean-temp`: 是否清理 `/root/new/tmp/` 下临时文件（默认 true）
- **示例**:
  ```bash
  agent-steward close session-20261115-demo
  ```
- **真实输出**:
  ```json
  {
    "session_id": "session-20261115-demo",
    "cleaned_files_count": 4,
    "freed_bytes": 9201,
    "cleaned_paths": [
      "/root/new/tmp/test_truncation.py",
      "/root/new/tmp/test_zoom.sh",
      "/root/new/tmp/test_todo_lifecycle.py",
      "/root/new/tmp/test_atomic.py"
    ],
    "changes_summary": "### 宿主环境变更回溯 (Changes Summary)\n\n> 最近记录数: `0`\n\n*(期间环境平稳，未检测到任何资产或服务变动)*\n",
    "saved_memory_id": "mem-session-e2d36c85-d5e8-4ce1-b3f8-1397e79459ea"
  }
  ```
