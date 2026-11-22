# 03 — 全量 CLI 命令行实战字典（CLI Reference）

> **章节目标**: 提供 Agent Body 所有命令行子命令、参数选项、使用场景与真实执行输出的标准参考手册。

---

## 1. CLI 命令总览

```bash
agent-body [OPTIONS] <COMMAND>
```

| 顶级子命令 | 简要说明 | 适用场景 |
|---|---|---|
| `status` | 查看大管家自身版本、数据库位置、资产/依赖边/进程统计 | 状态巡检与探活 |
| `tui` | 启动终端 ANSI 实时全景看板 | 终端实时运维观测 |
| `brief` | 一键获取宿主系统硬件、活跃任务、最近资产与记忆简报 | Agent 开工首调 |
| `place` | 咨询对象放置规范建议、列出规则或纠偏自演化学习 | 新建文件/工程防乱放 |
| `asset` | 资产分级扫描录入、手动注册与全文/语义检索 | 资产盘点与检索 |
| `env` | 硬件规格、活跃动态任务进程、网络端口防撞检测 | 运维排障、资源防撞 |
| `memory` | 跨会话长期记忆写入、混合 RRF 检索与矛盾仲裁审计 | 知识与决策沉淀 |
| `mcp` | 启动标准 JSON-RPC 2.0 stdio MCP 服务端（43 工具） | 供 Agent 客户端挂载 |
| `daemon` | 启动常驻后台守护主循环 (inotify 监听 + 60s 定时巡检 + 30min 新鲜度对账) | 系统服务守护 |
| `links` | 查询 `L1/L2/L3` 分层加权拓扑、导出 Mermaid、推演爆炸半径、断链修复与 CodeGraph 联动 | 改动代码前影响分析 |
| `changes` | 查看自某个时间点以来的宿主环境变更历史 | 开工前了解环境变动 |
| `close` | 会话完工结算、清理临时测试资产、归档沉淀长期记忆 | 会话结束自洁 |
| `gc` | 安全清理白名单临时目录过期文件与诊断孤儿进程 | 宿主环境自洁 |
| `fact` | 只读查询宿主机物理事实图谱（工具能力/进程端口/自动化/资料） | 零幻觉客观事实探查 |
| `storage` | 查看当前存储后端并执行 SQLite/PG/DuckDB 热迁移或分析副本同步 | 多引擎存储管理与迁移 |
| `coordination` | 管理跨 Agent 资源协作锁、等待队列唤醒与 WFG 死锁检测 | 多 Agent 防踩踏协同 |
| `preflight` | 运行时环境与系统依赖完整性预检（支持 `--json`） | 部署前与 CI/CD 校验 |
| `web` | 启动内嵌 Web UI 2.0 昼夜蓝图可视化大屏与 REST/SSE 服务 | 浏览器监控与路演 |
| `hooks` | 触发 AOPCC Pre/Post Hook 门禁（含 `<2ms` 碰壁自愈）与生成客户端钩子 | 客户端工作流集成 |
| `analytics` | 基于 DuckDB 列存统计变更热力图、趋势与 Parquet/CSV 导出 | 宿主时序聚合分析 |
| `mcp-catalog` | 扫描与路由宿主机客户端已纳管 MCP Server（密钥脱敏） | 生态目录管理 |
| `skill` | 扫描 120+ SKILL 目录并按 Token 预算防爆路由 Top-K | 防止大模型上下文爆炸 |
| `service` | 管理外部可用服务通讯录与轻量健康探活 | 外部服务目录治理 |
| `mesh` | 轻量多机联邦 `MeshEnvelope v1.0`：跨机图谱、HMAC 签名对话与沙盒文件投递 | 多机集群联动 |
| `platform` | 跨平台原生探针查看本机硬件、端口与自动化任务 | Linux/macOS/Windows 探测 |
| `demo` | 真引擎路演场景演示、ROI 核算与 43 工具黑白屏对齐自检 | 产品路演与一致性验证 |
| `bench` | 运行 `v3.7` 实证 Benchmark（`W1~W17` / `M1~M5` / `DQ1~DQ8` / `COMP-C1~C4`）与 `PR-01~13` 账本 | 实证效能评测与回归对比 |
| `feedback` | `user_feedback.md` 双向管理、`reconcile` 新鲜度对账与 `P1~P5` 五点自进化探针 | 反馈闭环与自进化体检 |
| `audit` | 双域全链路行为审计 (`IN_AGENT` + `OUT_OF_AGENT`)、DuckDB OLAP 与自进化 | 行为溯源与数据驱动进化 |
| `body` | Agent 本体中枢：`manifest` / `vitals` / `envelope` / `handoff` / `resume` / `heal` / `peers` | 跨大脑连续性与稳态治理 |

---

## 2. 详细子命令与实操指南

### 2.1 `status` — 系统状态与统计
- **说明**: 显示当前大管家的基础运行信息与数据库统计。
- **示例**:
  ```bash
  agent-body status
  ```
- **真实输出**:
  ```text
  ╔═══════════════════════════════════════════════════════╗
  ║      Agent Body (Rust 版) — 系统状态          ║
  ╚═══════════════════════════════════════════════════════╝
  版本:           v0.4.0 (9ba7bca, linux/x86_64, built at 2026-11-19T07:53:18Z)
  数据目录:       "/root/.agent-body"
  数据库文件:     "/root/.agent-body/body.db"
  已纳管资产数:   26091
  依赖关系边数:   6407 条
  放置规范规则:   8 条
  分级扫描策略:   12 条
  当前动态进程:   96 个
  ```

---

### 2.2 `brief` — 宿主环境开工简报与乐观直通契约
- **说明**: 聚合硬件、动态进程、资产、长期记忆与 **AOPCC 开工乐观直通租约** (`<= 120 tok`)，默认走 `BriefAssembler` 三位一体组装。
- **参数**:
  - `-q, --query <QUERY>`: 任务查询意图关键词
  - `-s, --session-id <ID>`: 会话 ID（用于召回本会话关键决策）
  - `-b, --budget-tokens <N>`: Token 预算上限（默认 1500）
  - `--legacy`: 兼容旧版极简动态扫描输出
- **示例**:
  ```bash
  agent-body brief
  agent-body brief --query "PostgreSQL 调优" --budget-tokens 1200
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
  agent-body place suggest --type script --name sec_scanner --intent "抓取美股财报"
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
  agent-body place rules
  ```

---

### 2.4 `env` — 系统环境与进程管理
提供 `show`, `processes`, `port` 三大子命令。

#### (1) `env show`
- **说明**: 查看 CPU 核心与型号、总内存/已用内存、磁盘分区容量与可用空间。
- **示例**:
  ```bash
  agent-body env show
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
  agent-body env processes
  ```

#### (3) `env port <PORT>`（防撞预警）
- **参数**:
  - `<PORT>`: 目标网络端口号（1-65535）
  - `-s, --suggest-count <N>`: 若冲突推荐的空闲端口数（默认 3）
- **示例 1（冲突场景）**:
  ```bash
  agent-body env port 22
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
  agent-body env port 8999
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
  agent-body memory add -c "决定选用 Rust 编写核心守护进程" -s "选型决策"
  ```

#### (2) `memory search`
- **参数**:
  - `<QUERY>`: 搜索关键词或语义描述
  - `-l, --limit <N>`: 限制返回条数（默认 10）
  - `--hybrid`: 是否启用 Ollama 向量 + FTS5 混合 RRF 检索（默认为 true）
- **示例**:
  ```bash
  agent-body memory search "Rust 选型" --hybrid
  ```
- **真实输出**:
  ```text
  记忆搜索结果 (混合模式 RRF, 共 1 条):
    - [decision] 决定选用 Rust 1.99 与 SQLite 编写 Agent Body 核心进程 [RRF: 0.0164] (ID: mem-82ecbb7b-6e9d-4ab2-bf6a-d596561299a6)
  ```

---

### 2.6 `links` — 关联依赖、分层图谱与爆炸半径
提供 `query`, `graph`, `impact`, `audit`, `repair`, `codegraph` 六大子命令，支持 **`L1_EXPLICIT` / `L2_STRUCTURAL` / `L3_SEMANTIC`** 多维证据分层有向加权图谱。

#### (1) `links query <ID>`（分层属性图查询）
- **参数**:
  - `-t, --type <TYPE>`: 实体类型（`asset`, `service`, `project`, `tool`, `memory`）
  - `--max-hops <N>`: 属性图谱遍历最大跳数（默认 `2`）
  - `--min-weight <W>`: 最低边权重过滤（`0.0~1.0`，默认 `0.0`）
  - `--include-semantic <true|false>`: 是否包含 `L3_SEMANTIC` 隐式语义边（默认 `true`）
- **示例**:
  ```bash
  agent-body links query -t project project:agent-body --max-hops 2 --min-weight 0.5
  ```

#### (2) `links graph`
- **说明**: 输出 Mermaid 格式的实体拓扑关系图。
- **参数**:
  - `-l, --limit <N>`: 输出边数上限（默认 `20`）
  - `--discover-semantic`: 导出前触发 `L2` 结构上卷与 `L3` 语义关系挖掘
- **示例**:
  ```bash
  agent-body links graph
  agent-body links graph --discover-semantic --limit 30
  ```

#### (3) `links impact <TARGET>`（加权衰减爆炸半径推演）
- **说明**: 融合文件 `links` 与属性图 `fact_edges`，按跳数衰减系数 `0.8^(hop-1)` 与边权重计算影响范围，默认以 `L1+L2` 为强影响基准。
- **参数**:
  - `-m, --max-depth <N>`: 依赖多跳推演最大深度（默认 `3`）
  - `--min-weight <W>`: 最低有效权重阈值（默认 `0.50`）
  - `--include-semantic`: 是否包含 `L3_SEMANTIC` 隐式语义边
- **示例**:
  ```bash
  agent-body links impact /root/new/shared_proj/test.py
  agent-body links impact project:agent-body --include-semantic --min-weight 0.35
  ```

#### (4) `links audit`（断链与幽灵依赖体检）
- **说明**: 主动扫描全局依赖拓扑中的断链与悬空引用（目标文件不存在、依赖服务离线），输出健康评分与修复建议。
- **参数**:
  - `--fix`: 自动清理已确认失效的文件依赖死链。
- **示例**:
  ```bash
  agent-body links audit
  agent-body links audit --fix
  ```

#### (4) `links codegraph <PATH> [SYMBOL]`（CodeGraph 宏微联邦联动）
- **说明**: 跨层联动局部工程内的 CodeGraph 代码图。在不传递 SYMBOL 时输出项目代码图概览；传递 SYMBOL 时输出函数/符号级代码调用方列表（包含文件路径与行号）。
- **示例**:
  ```bash
  agent-body links codegraph /root/new/src/hk2
  agent-body links codegraph /root/new/src/hk2 interactive
  ```

#### (5) `links repair`（断链智能修复与重定向推演）
- **说明**: 基于文件名相似度、Git 历史与代码指纹定位已迁移/重命名的断链目标，生成 diff 修复补丁，并支持一键替换修复。
- **参数**:
  - `--apply`: 自动修改源文件并更新关联表。
- **示例**:
  ```bash
  agent-body links repair
  agent-body links repair --apply
  ```

---

### 2.7 `changes` — 宿主环境变更回溯
- **参数**:
  - `-s, --since <TIMESTAMP>`: 起始时间戳（ISO 8601，如 `2026-11-15T00:00:00`）
  - `-l, --limit <N>`: 返回条数（默认 20）
- **示例**:
  ```bash
  agent-body changes
  ```

---

### 2.8 `close` — 会话完工结算与垃圾回收
- **参数**:
  - `<SESSION_ID>`: 当前会话 ID
  - `--clean-temp`: 是否清理 `/root/new/tmp/` 下临时文件（默认 true）
- **示例**:
  ```bash
  agent-body close session-20261115-demo
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
  agent-body place learn
  agent-body place learn --accept
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
  agent-body gc
  agent-body gc --apply
  ```

---

### 2.11 `memory audit` — 认知记忆冲突体检与仲裁
- **说明**: 检测对立冲突的认知记忆对，计算温和时间衰减淘汰候选，支持指定归档旧条目。
- **参数**:
  - `--archive-id <ID>`: 若存在冲突，指定归档淘汰的旧记忆 ID
- **示例**:
  ```bash
  agent-body memory audit
  agent-body memory audit --archive-id mem-01
  ```

---

### 2.12 `tui` — 终端实时全景看板
- **说明**: 启动紧凑 ANSI 交互式终端单屏看板，实时监控硬件指标、过滤后的 Agent 进程树、活动端口拓扑、断链告警与最新审计流。
- **参数**:
  - `-i, --interval <SECS>`: 看板刷新间隔秒数（默认 2）
- **示例**:
  ```bash
  agent-body tui
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
  agent-body coordination list
  agent-body coordination release --lock-id "lock-port-8080"
  agent-body coordination gc
  ```

---

### 2.14 `preflight` — 运行环境与依赖完整性预检
- **说明**: 逐项验证 procfs、inotify、data_dir、SQLite、DuckDB、Systemd、CodeGraph、Ollama 等运行环境与依赖完整性。
- **参数**:
  - `-j, --json`: 输出原始 JSON 格式以便 CI/CD 或 Agent 机器解析
- **示例**:
  ```bash
  agent-body preflight
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
  agent-body web
  agent-body web --host 0.0.0.0 --port 9000
  ```

---

### 2.16 `hooks` — HOOK 驱动集成、AOPCC 门禁校验与客户端自动化钩子生成
- **说明**: 自动生成各 Agent 客户端（Antigravity、Cursor、Claude Code、Git）的 Pre/Post Hook 集成配置（含 `body_bridge.py` AOPCC 自适应桥接），或直接从 CLI 触发前置/后置门禁与 `<2ms` 碰壁秒级自愈（规约 #13 & #25）。
- **子命令**:
  - `generate`:
    - `-c, --client <CLIENT>`: 目标客户端类型 (`agy`, `cursor`, `claude`, `git`)
    - `-o, --out-dir <DIR>`: 写入目标根目录（若未指定则输出到标准输出预览）
    - `--dry-run`: 仅预览生成内容，不实际写盘
  - `pre`:
    - `-t, --tool-name <NAME>`: 即将调用的工具名（如 `bind_port`、`write_file`、`run_command`）
    - `-a, --args <JSON>`: 工具入参 JSON 字符串
  - `post`:
    - `-t, --tool-name <NAME>`: 刚执行完成的工具名
    - `-a, --args <JSON>`: 工具入参 JSON 字符串
    - `--success <true|false>`: 调用是否成功（默认 `true`，兼容 `--success=false`）
    - `-e, --error-output <TEXT>`: 可选失败错误输出（触发 `OptimisticFailureDiagnoser` `<2ms` 自愈处方注入）
- **示例**:
  ```bash
  agent-body hooks generate --client agy --dry-run
  agent-body hooks pre --tool-name run_command --args '{"command":"psql -p 5432"}'
  agent-body hooks post --tool-name write_file --args '{"path":"/root/new/work/demo.rs"}'
  agent-body hooks post --tool-name run_command --args '{"command":"psql -p 9999"}' --success=false --error-output "Connection refused"
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
  agent-body analytics heatmap --days 7
  agent-body analytics trend --metric assets --days 30
  agent-body analytics export --table assets --output-path /root/new/tmp/assets.parquet
  ```

---

### 2.18 补充子命令速查（1:1 映射补齐）
- `agent-body asset register --path <PATH> --type <TYPE> [--description <DESC>]`：手动向大管家登记单个文件或目录资产（对应 `body_asset_register`）。
- `agent-body coordination acquire -t <TYPE> -i <ID> [-s exclusive|shared] [-a <AGENT>] [--ttl-seconds 300]`：申请资源协作锁（对应 `body_lock_acquire`）。
- `agent-body coordination wait -t <TYPE> -i <ID> [--wait-timeout-ms 5000]`：排队等待并唤醒获取协作锁，内置 WFG 死锁环路拦截（对应 `body_lock_wait`）。
- `agent-body coordination deadlock`：执行 Wait-For Graph 全局死锁与等待队列检测（对应 `body_lock_deadlock_check`）。
- `agent-body close <SESSION_ID> [--dry-run]`：会话结算，增加 `--dry-run` 预览开关（对应 `body_session_close`）。

---

### 2.19 `mcp-catalog` / `skill` / `service` — 生态认知与 Context 防爆路由器 (Phase 16)
- `agent-body mcp-catalog <scan|list|route>`: 扫描宿主机客户端 MCP 配置（强制脱敏密钥）、列清单与按意图检索（对应 `body_mcp_catalog`）。
- `agent-body skill <scan|list|route>`: 扫描宿主机 130+ SKILL 目录，按 `--budget-tokens` 渐进式路由 Top-K SKILL 路径，防止 Context 膨胀（对应 `body_skill_route`）。
- `agent-body service <register|list|query|check|bootstrap>`: 管理外部可用服务通讯录与轻量健康探活（对应 `body_service_contact`）。

---

### 2.20 `platform` / `mesh` — 跨平台原生探针与 `MeshEnvelope v1.0` 联邦协同 (Phase 18 & 41)
- `agent-body platform <info|ports|services>`: 使用运行时 `PlatformProbe`（Linux `/proc`+systemd、macOS `lsof`+launchd、Windows `netstat`+schtasks）查看本机监听端口与自动化任务。
- `agent-body mesh <peers|local|ping|facts|locks|port|acquire|graph|dialogue-send|dialogue-inbox|dialogue-ack|transfer-send|transfer-inbox|transfer-accept|transfer-reject>`: 基于 `MeshEnvelope v1.0`（HMAC-SHA256 签名 + `+-300s` 防重放）支持跨节点只读/租约锁协同、跨机图谱联邦探查（`graph`）、双向脱敏对话（`dialogue-*`）与 `<=10MB` `~/.agent-body/mesh_inbox/` 沙盒投递（`transfer-*`）（对应 `body_mesh_query`）。

---

### 2.21 `demo` — 路演场景演示与黑白屏同步自检引擎 (Phase 22)
- `agent-body demo <list|run <scenario_id>|roi|parity|reset>`: 列出或用真实宿主机引擎执行 7 大黄金路演剧目（`scene-1`~`scene-7`）、核算实时商业价值 ROI、检查 43 工具黑白屏同步矩阵，或一键清理 `demo:*` 沙盒锁（对应 `body_demo_scenario`）。

---

### 2.22 `storage` — 多后端存储管理、热迁移与 DuckDB 同步
- **这是什么**: 在不中断业务的前提下查看当前存储后端，在 SQLite / PostgreSQL / DuckDB 之间执行全量热迁移或增量同步 OLAP 分析副本。
- **为什么需要**: 单机起步用零依赖 SQLite 最省心，但团队多机协同或海量审计时序分析需要迁移至 PostgreSQL + pgvector 或同步至 DuckDB 列存。
- **子命令与参数**:
  - `storage info`: 查看当前生效的存储提供者与数据库路径/连接信息。
  - `storage migrate [--from sqlite|postgres|duckdb] [--from-url <URL>] [--to sqlite|postgres|duckdb] [--to-url <URL>]`: 在不同存储后端间执行全量数据热迁移。
  - `storage sync [--to duckdb] [--to-path <PATH>]`: 将主存储的资产与变更审计同步到 DuckDB 列存副本（默认 `~/.agent-body/steward.duckdb`）。
- **调用示例**:
  ```bash
  agent-body storage info
  agent-body storage sync --to duckdb
  agent-body storage migrate --from sqlite --to postgres --to-url "host=127.0.0.1 dbname=steward user=postgres"
  ```
- **真实输出解读**:
  ```text
  当前活跃存储提供者: sqlite
  配置数据库文件路径: "/root/.agent-body/body.db"
  ```

---

### 2.23 `bench` — 实证 Benchmark v3.7 对照评测与生产效能存证账本 (Phase 27~44)
- **这是什么**: 在真实宿主机上运行「Without Steward 裸奔基线 vs With Steward Agent 本体」A/B 对照测试套件，运行竞品横向对标，并审计日常每次 MCP/CLI/Web/Hook 调用自动记录的生产效能存证流水（对应 `body_benchmark`）。
- **为什么需要**: 用同机实跑数据与 SHA-256 防篡改指纹证明 Token 节省率、上下文质量净胜（$\Delta Q$）、延迟加速比与冲突拦截率，杜绝“拍脑袋宣传数字”（规约 #18）。
- **子命令与参数**:
  - `workloads`: 列出 17 大标准 A/B 负载（`W1`~`W17`）、`M1`~`M5`、`DQ1`~`DQ8` 与 `PR-01`~`PR-13` 归因推导规则字典。
  - `run [--suite full|macro|micro|dq|competitors|W1~W17|COMP-C1~C4] [--save true|false]`: 真机跑批并生成带 SHA-256 指纹的实证报告（默认保存入库）。
  - `competitors [--track all|C1|C2|C3|C4] [--format markdown|json]`: 运行四大赛道热门开源竞品横向实证对标。
  - `report [--run-id <ID>] [--format table|json|csv]`: 查看指定批次或最近一次跑批报告。
  - `ledger [--window 24h|7d|30d|all] [--tool <NAME>] [--limit 20]`: 审计生产运行态按 PR 规则聚合的效能存证账本。
  - `compare <RUN_A> <RUN_B>`: 跨版本或跨批次比对 Token 节省率与耗时回归差异。
  - `export [--format markdown|json|csv] [--out <PATH>]`: 导出带 SHA-256 签名的可发表评测报告。
  - `reset`: 清理 `bench:*` 隔离沙盒测试数据。
- **调用示例**:
  ```bash
  agent-body bench workloads
  agent-body bench run --suite full --save false
  agent-body bench competitors --track all
  agent-body bench ledger --window 7d --limit 5
  ```
- **真实输出解读（`bench run --suite full --save false` 实机片段）**:
  ```markdown
  # Agent Body 实证基准评测报告 (run-ef911f10-cb74-4733-8908-7744f190e75a)

  - **版本 / Commit**: `v0.4.0` (`9ba7bca`)
  - **评测套件**: `full` · **总耗时**: `1924 ms`
  - **报告 SHA-256 存证指纹**: `19e4777aefde296b80bb36e2b1099905442c4f7d3bf797dd3630411113b8bbea`
  - **SLA 总检门禁**: ✅ PASS

  | 指标维度 | 基线 (Without Steward) | 管家赋能 (With Steward) | 净收益 |
  |---|---:|---:|---:|
  | **Prompt Token 消耗** | 76690 tok | 1381 tok | **-98.20%** (75309 tok saved) |
  | **事前拦截风险数** | - | - | **16 次** |
  | **阻断垃圾噪音文件** | - | - | **379 个** |
  ```
  > 💡 **指标解读**: 当前 Benchmark 已演进至 **`v3.7`**，覆盖 `W1~W17`（含分层图谱 `W13`、Mesh 防重放 `W14`、AOPCC 自适应并发 `W15`、双域审计 `W16`、跨大脑连续性 `W17`）、5 项微基准（`M1~M5`）、8 项质量与稳态门禁（`DQ1~DQ8`）及 `COMP-C1~C4` 竞品同机对标，全量满足「既省 Token × 上下文质量不降反升」帕累托双门禁。

---

### 2.24 `feedback` — 实际使用反馈总账、信息新鲜度对账与五点自进化探针 (Phase 30 & 38~39)
- **这是什么**: 直接操作 `user_feedback.md` 反馈总账，将已验证改进经验沉淀为长期记忆，并触发 `FreshnessReconciler` 全生命周期信息新鲜度对账与 `P1~P5` 五点对比自进化探针（对应 `body_feedback`）。
- **子命令与参数**:
  - `list`: 列出 `user_feedback.md` 中的全部反馈条目 (`PRO` / `CON` / `BENCH` / `REQ`)。
  - `add --title <TEXT> [--category PRO|CON|BENCH|REQ] [--priority P0|P1|P2] [--evidence <TEXT>] [--target <PATH>]`: 结构化追加一条新反馈。
  - `sync`: 将标为 `✅ Verified` 的反馈条目沉淀入长期记忆库。
  - `reconcile [--json]`: 执行全生命周期信息新鲜度对账（失效重算 `stale` 标记、删除级联清理、幽灵节点剪枝与同路径重复记忆归档）。
  - `probes [--auto-heal] [--auto-feedback] [--json]`: 运行 `P1~P5` 执行回路五点对比自进化探针。
- **调用示例**:
  ```bash
  agent-body feedback list
  agent-body feedback reconcile
  agent-body feedback probes --auto-heal
  ```

---

### 2.25 `audit` — 双域全链路行为审计与自进化 (Phase 43)
- **这是什么**: 查询 `IN_AGENT`（接入调用）与 `OUT_OF_AGENT`（守护巡检/Watcher 60s 折叠/GC/修复/对账）双域审计流水，触发 DuckDB OLAP 聚合与基于审计流的 AOPCC 租约自进化（对应第 42 个 MCP 工具 `body_audit_query`）。
- **调用示例**:
  ```bash
  agent-body audit summary --window 7d --days 7
  agent-body audit query --domain OUT_OF_AGENT --limit 20
  agent-body audit evolve
  agent-body audit export --output-path /root/new/tmp/audit_events_export.csv
  ```

---

### 2.26 `body` — Agent 本体中枢（六器官自省、五维稳态与跨大脑接力，Phase 44）
- **这是什么**: 面向脑体分离架构的 Agent 本体主控制命令族，提供确定性 `body_id` 自省、五维稳态体检、四级自治包络评估、跨大脑 `<300 tok` 接力胶囊写入/恢复与 `L1_AUTONOMIC` 反射自愈（对应第 43 个 MCP 工具 `body_state`）。
- **子命令与参数**:
  - `manifest [--json]`: 输出 `body_id`、宿主指纹与六大生理器官（感官/小脑/海马体/脊柱/代谢/社会）健康状态。
  - `vitals [--json]`: 计算五维本体生命体征雷达评分（`sensory_clarity` / `reflex_accuracy` / `memory_purity` / `metabolic_health` / `immune_traceability`）。
  - `envelope [--action <ACT>] [--target <PATH>] [--dry-run] [--json]`: 展示 `L0~L3` 策略矩阵或判定特定动作的自治边界。
  - `handoff --goal <TEXT> [--from-brain <NAME>] [--files <CSV>] [--facts <CSV>] [--blockers <CSV>] [--next-step <TEXT>]`: 脱敏防投毒后保存 `<=300 tok` 接力胶囊。
  - `resume [--by-brain <NAME>] [--checkpoint-id <ID>]`: 恢复最新未消费接力胶囊并标注接替大脑。
  - `heal [--json]`: 严格在 `L1_AUTONOMIC` 包络内执行过期锁清理、租约校准与新鲜度对账。
  - `peers [--json]`: 拉取多机 Mesh 节点 `/api/v1/mesh/body_manifest` 本体名片。
- **调用示例**:
  ```bash
  agent-body body manifest
  agent-body body vitals
  agent-body body envelope --action write_file --target /etc/shadow
  agent-body body handoff --from-brain antigravity --goal "完成 PG WAL 调优验证" --next-step "跑 pgbench 对比"
  agent-body body resume --by-brain claude-code
  ```


