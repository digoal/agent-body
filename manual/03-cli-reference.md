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
提供 `query`, `graph`, `impact`, `audit`, `codegraph` 五大子命令。

#### (1) `links graph`
- **说明**: 输出 Mermaid 格式的实体拓扑关系图。
- **示例**:
  ```bash
  agent-steward links graph
  ```

#### (2) `links impact <TARGET>`（爆炸半径推演）
- **说明**: 输入资产路径，BFS 递归计算直接依赖与多跳间接下游，并附带代码行号出处与 CodeGraph 微观下钻标记。
- **示例**:
  ```bash
  agent-steward links impact /root/new/shared_proj/test.py
  ```

#### (3) `links audit`（断链与幽灵依赖体检）
- **说明**: 主动扫描全局依赖拓扑中的断链与悬空引用（目标文件不存在、依赖服务离线），输出健康评分与修复建议。
- **参数**:
  - `--fix`: 自动清理已确认失效的文件依赖死链。
- **示例**:
  ```bash
  agent-steward links audit
  agent-steward links audit --fix
  ```

#### (4) `links codegraph <PATH> [SYMBOL]`（CodeGraph 宏微联邦联动）
- **说明**: 跨层联动局部工程内的 CodeGraph 代码图。在不传递 SYMBOL 时输出项目代码图概览；传递 SYMBOL 时输出函数/符号级代码调用方列表（包含文件路径与行号）。
- **示例**:
  ```bash
  agent-steward links codegraph /root/new/src/hk2
  agent-steward links codegraph /root/new/src/hk2 interactive
  ```

#### (5) `links repair`（断链智能修复与重定向推演）
- **说明**: 基于文件名相似度、Git 历史与代码指纹定位已迁移/重命名的断链目标，生成 diff 修复补丁，并支持一键替换修复。
- **参数**:
  - `--apply`: 自动修改源文件并更新关联表。
- **示例**:
  ```bash
  agent-steward links repair
  agent-steward links repair --apply
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

---

### 2.9 `place learn` — 放置规范自演化
- **说明**: 聚类分析历史资产分布与创建事件，自动推演发现潜在的新放置规则候选，并支持一键采纳持久化。
- **参数**:
  - `-m, --min-samples <N>`: 聚类最小样本数（默认 3）
  - `--accept`: 自动采纳并入库所有发现的规则候选
- **示例**:
  ```bash
  agent-steward place learn
  agent-steward place learn --accept
  ```

---

### 2.10 `gc` — 宿主垃圾回收与孤儿进程诊断
- **说明**: 安全扫描 `/root/new/tmp/` 超期孤儿临时文件与后台僵尸测试进程，支持预览与清理。
- **参数**:
  - `--tmp-dir <DIR>`: 待清理的临时目录路径 (强制安全白名单，默认 `/root/new/tmp`)
  - `--max-age-hours <HOURS>`: 文件超期阈值（默认 72）
  - `--apply`: 直接执行清理删除（默认仅 dry-run 预览）
- **示例**:
  ```bash
  agent-steward gc
  agent-steward gc --apply
  ```

---

### 2.11 `memory audit` — 认知记忆冲突体检与仲裁
- **说明**: 检测对立冲突的认知记忆对，计算温和时间衰减淘汰候选，支持指定归档旧条目。
- **参数**:
  - `--archive-id <ID>`: 若存在冲突，指定归档淘汰的旧记忆 ID
- **示例**:
  ```bash
  agent-steward memory audit
  agent-steward memory audit --archive-id mem-01
  ```

---

### 2.12 `tui` — 终端实时全景看板
- **说明**: 启动紧凑 ANSI 交互式终端单屏看板，实时监控硬件指标、过滤后的 Agent 进程树、活动端口拓扑、断链告警与最新审计流。
- **参数**:
  - `-i, --interval <SECS>`: 看板刷新间隔秒数（默认 2）
- **示例**:
  ```bash
  agent-steward tui
  ```

---

### 2.13 `coordination` — 多 Agent 协作互斥锁管理
- **说明**: 管理与查看跨 Agent 资源互斥锁，支持排他/共享模式、目录递归互斥与超时死锁强制解除。
- **子命令**:
  - `list`: 列出当前生效中的活跃协作锁 (`--resource-type <TYPE>`)
  - `release --lock-id <ID>`: 手动释放指定资源锁
  - `gc`: 回收所有已超期的过期死锁
- **示例**:
  ```bash
  agent-steward coordination list
  agent-steward coordination release --lock-id "lock-port-8080"
  agent-steward coordination gc
  ```

---

### 2.14 `preflight` — 运行环境与依赖完整性预检
- **说明**: 逐项验证 procfs、inotify、data_dir、SQLite、DuckDB、Systemd、CodeGraph、Ollama 等运行环境与依赖完整性。
- **参数**:
  - `-j, --json`: 输出原始 JSON 格式以便 CI/CD 或 Agent 机器解析
- **示例**:
  ```bash
  agent-steward preflight
  ```

---

### 2.15 `web` — Web UI 与可视化监控大屏
- **说明**: 启动基于浏览器的轻量级大管家拓扑图与协作锁监控看板。单二进制 100% 自包含交付，零外部 CDN 网络依赖。
- **参数**:
  - `--host <IP>`: 监听地址（默认 `127.0.0.1`，可设为 `0.0.0.0` 允许局域网访问）
  - `-p, --port <PORT>`: 监听端口（默认 `8765`）
  - `--auth-token <TOKEN>`: 管理员受控运维操作（如释放锁）鉴权 Token
- **示例**:
  ```bash
  agent-steward web
  agent-steward web --host 0.0.0.0 --port 9000
  ```

---

### 2.16 `hooks` — HOOK 驱动集成、门禁校验与客户端自动化钩子生成
- **说明**: 自动生成各 Agent 客户端（Antigravity、Cursor、Claude Code、Git）的 Pre/Post Hook 集成配置，或直接从 CLI 触发前置/后置门禁（规约 #13 MCP 与 CLI 1:1 映射）。
- **子命令**:
  - `generate`:
    - `-c, --client <CLIENT>`: 目标客户端类型 (`agy`, `cursor`, `claude`, `git`)
    - `-o, --out-dir <DIR>`: 写入目标根目录（若未指定则输出到标准输出预览）
    - `--dry-run`: 仅预览生成内容，不实际写盘
  - `pre`:
    - `-t, --tool-name <NAME>`: 即将调用的工具名（如 `bind_port`、`write_file`）
    - `-a, --args <JSON>`: 工具入参 JSON 字符串
  - `post`:
    - `-t, --tool-name <NAME>`: 刚执行完成的工具名
    - `-a, --args <JSON>`: 工具入参 JSON 字符串
    - `--success`: 调用是否成功（默认 `true`）
- **示例**:
  ```bash
  agent-steward hooks generate --client agy --dry-run
  agent-steward hooks pre --tool-name bind_port --args '{"port":8080}'
  agent-steward hooks post --tool-name write_file --args '{"path":"/root/new/work/demo.rs"}'
  ```

---

### 2.17 `analytics` — DuckDB 列存分析与 Parquet/CSV 导出
- **说明**: 基于内嵌 DuckDB 引擎统计文件变更热力图、资产/记忆趋势及导出 Parquet/CSV 数据集（与 `steward_analytics_*` 1:1 映射）。
- **子命令**:
  - `heatmap [--days 30]`: 按目录与小时分桶统计变更热力图
  - `trend [--metric assets] [--days 30]`: 统计资产增长、记忆衰减与图谱统计
  - `export --table <TABLE> --output-path <PATH>`: 将指定表导出为 Parquet/CSV
- **示例**:
  ```bash
  agent-steward analytics heatmap --days 7
  agent-steward analytics trend --metric assets --days 30
  agent-steward analytics export --table assets --output-path /root/new/tmp/assets.parquet
  ```

---

### 2.18 补充子命令速查（1:1 映射补齐）
- `agent-steward asset register --path <PATH> --type <TYPE> [--description <DESC>]`：手动向大管家登记单个文件或目录资产（对应 `steward_asset_register`）。
- `agent-steward coordination acquire -t <TYPE> -i <ID> [-s exclusive|shared] [-a <AGENT>] [--ttl-seconds 300]`：申请资源协作锁（对应 `steward_lock_acquire`）。
- `agent-steward coordination wait -t <TYPE> -i <ID> [--wait-timeout-ms 5000]`：排队等待并唤醒获取协作锁，内置 WFG 死锁环路拦截（对应 `steward_lock_wait`）。
- `agent-steward coordination deadlock`：执行 Wait-For Graph 全局死锁与等待队列检测（对应 `steward_lock_deadlock_check`）。
- `agent-steward close <SESSION_ID> [--dry-run]`：会话结算，增加 `--dry-run` 预览开关（对应 `steward_session_close`）。

---

### 2.19 `mcp-catalog` / `skill` / `service` — 生态认知与 Context 防爆路由器 (Phase 16)
- `agent-steward mcp-catalog <scan|list|route>`: 扫描宿主机客户端 MCP 配置（强制脱敏密钥）、列清单与按意图检索（对应 `steward_mcp_catalog`）。
- `agent-steward skill <scan|list|route>`: 扫描宿主机 130+ SKILL 目录，按 `--budget-tokens` 渐进式路由 Top-K SKILL 路径，防止 Context 膨胀（对应 `steward_skill_route`）。
- `agent-steward service <register|list|query|check|bootstrap>`: 管理外部可用服务通讯录与轻量健康探活（对应 `steward_service_contact`）。

---

### 2.20 `platform` / `mesh` — 跨平台原生探针与轻量多机联邦 (Phase 18)
- `agent-steward platform <info|ports|services>`: 使用运行时 `PlatformProbe`（Linux `/proc`+systemd、macOS `lsof`+launchd、Windows `netstat`+schtasks）查看本机监听端口与自动化任务。
- `agent-steward mesh <peers|local|ping|facts|locks|port|acquire>`: 对等节点只读联邦查询、跨机端口占用查询与 HTTP 租约锁申请（对应 `steward_mesh_query`）。

---

### 2.21 `demo` — 路演场景演示与黑白屏同步自检引擎 (Phase 22)
- `agent-steward demo <list|run <scenario_id>|roi|parity|reset>`: 列出或用真实宿主机引擎执行 6 大黄金路演剧目（`scene-1`~`scene-6`）、核算实时商业价值 ROI、检查 39 工具黑白屏同步矩阵，或一键清理 `demo:*` 沙盒锁（对应 `steward_demo_scenario`）。


