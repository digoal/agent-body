# Agent Host Steward (Rust 版) — 产品使用手册

> 版本: v0.2.0 · 核心实现: Rust 1.99 + SQLite 3 (rusqlite bundled with FTS5)  
> 存放路径: `src_doc/product-manual.md`  
> 更新时间: 2026-11-15

---

## 一、产品定位与核心使命

> **宿主环境大管家：驻留在 Agent 宿主机中，管记忆、管素材、管软件环境、管一切。**

Agent Host Steward 不是为了某一次任务会话而生的临时工具，而是让 Agent 更清晰地了解自己所处的宿主系统环境、动态进程与静态资产，更好完成复杂工程与多 Agent 协作。

### 核心三大支柱

1. **对象位置大管家与放置建议先行（Placement Advisor）**：
   在产生新对象（项目、脚本、笔记、临时文件）之前，主动指导“放哪里”，制定并执行宿主机目录规范与安全脱敏约定。
2. **拒绝“一股脑生成”的分级颗粒度（Granularity Tiering）**：
   * **白名单（Deep Tier）**：细到最深层，全量递归解析，生成标量属性、结构化 `desc_kv`、全文 FTS5、BM25 与向量；
   * **黑名单（Shallow Tier）**：**止于当前目录层级**（如 `node_modules`、`.cache`、敏感目录），仅记录目录实体本身标量，绝不深入递归解析子文件，杜绝扫爆数据库和泄露敏感信息。
3. **动态与静态全景纳管**：
   不仅管理文件与代码资产，更实时采集和监控 Agent 或脚本发起的动态任务进程（Active Processes），杜绝后台死锁与资源冲突。

---

## 二、快速开始

### 1. 本地编译与构建

确保环境变量包含 Rust 工具链：
```bash
export PATH="/root/.cargo/bin:$PATH"

# 检查代码与运行单元测试
cargo test

# 构建高性能发布版本
cargo build --release
```
二进制文件将生成于 `target/release/agent-steward`。

### 2. 初始化与状态查看

无需复杂配置，单二进制即可自举初始化数据库（默认数据目录位于 `~/.steward/`）：

```bash
# 查看大管家状态与纳管统计
./target/release/agent-steward status
```

---

## 三、核心功能与 CLI 操作命令

### 1. 新对象放置与位置建议 (`steward place`)

Agent 或开发者在准备落盘新代码、新脚本或临时数据时调用：

```bash
# 询问新脚本应该放哪里
./target/release/agent-steward place suggest \
  --type script \
  --name "sec_scraper" \
  --intent "长期复用的美股财报抓取工具" \
  --security "internal"

# 输出示例:
# 推荐目标路径:   "/root/new/shared_proj/sec_scraper/"
# 命中规范规则:   shared_tools
# 规则理由:       长期沉淀的可复用脚本与程序库
# 命名建议:       推荐命名格式: {name}/
# 冲突状态:       ✅ 路径空闲可用
# 伴随开发指引:
#   * 建立独立子目录，配备 README.md 与入口脚本，严禁硬编码敏感信息
#   * 工程规范: 建议在同目录下配备 README.md 说明用途、依赖库及执行示例。

# 查看当前生效的宿主机放置规范
./target/release/agent-steward place rules
```

### 2. 资产分级扫描与纳管 (`steward asset`)

大管家遵循内置的 `scan_policies` 白黑名单规则执行扫描：

```bash
# 扫描指定目录 (白名单深入遍历，黑名单止于当层)
./target/release/agent-steward asset scan /root/new/work/

# 全文搜索资产 (支持 BM25 与结构化 desc_kv 匹配)
./target/release/agent-steward asset search "finance_tool"

# 查看最近索引的资产清单
./target/release/agent-steward asset list --limit 10
```

### 3. 宿主环境与动态任务进程 (`steward env`)

```bash
# 查看系统硬件规格 (CPU、内存、磁盘挂载)
./target/release/agent-steward env show

# 查看除系统守护进程外的活跃任务与脚本进程 (过滤底噪)
./target/release/agent-steward env processes

# 输出示例:
# PID      NAME       CPU%   MEM(MB)    OWNER              COMMAND
# 14208    python3    98.2   420.5      agent:hermes_cron  python /root/new/shared_proj/earnings_scan/scan_sec.py
```

### 4. 一键上下文简报 (`steward brief`)

Agent 新开会话的前 10 秒调用此命令，一键获取全局环境、活跃进程与关键资产摘要：

```bash
./target/release/agent-steward brief
```

### 5. 跨会话长期记忆 (`steward memory`)

```bash
# 记录关键架构决策
./target/release/agent-steward memory add \
  --type decision \
  --content "选用 Rust 编写 Agent Host Steward 核心引擎，保证单二进制高性能与内存安全" \
  --summary "选用 Rust"

# 检索历史记忆
./target/release/agent-steward memory search "Rust"
```

---

## 四、宿主机规范拓扑与默认规则

大管家内嵌宿主机标准目录组织约定：

| 规则标识 | 纳管类型 | 推荐基准路径 | 命名约定 | 规范说明与指引 |
|---|---|---|---|---|
| `shared_tools` | script | `/root/new/shared_proj/` | `{name}/` | 长期沉淀的复用工具，必须配备独立目录、README 与入口脚本 |
| `work_projects` | project | `/root/new/work/` | `{name}/` | 常规立项工程，独立版本控制与设计文档闭环 |
| `ai_skills` | skill | `/root/new/skills/` | `{name}/` | AI 技能资产，强制包含标准 `SKILL.md` |
| `open_source_src`| src | `/root/new/src/` | `{name}/` | 浅克隆开源项目，进入后优先使用 `codegraph init` |
| `notes_and_lists`| note | `/root/new/note/` | `{name}.md` | 重要笔记与权威清单，好记性不如烂笔头 |
| `cron_jobs` | other | `/root/new/cron/` | `{name}/` | 定时任务 prompt 与产物，执行前须错峰排期 |
| `temp_files` | tmp | `/root/new/tmp/` | `{name}` | 临时文件与日志，声明周期有限，随时可清理 |
| `open_source_shared` | other | `/root/new/shared_git/` | `{name}/` | 可对外开源精品工程，结构规整文档齐全 |

---

## 五、配置文件说明 (`~/.steward/config.toml`)

启动时自动在 `~/.steward/config.toml` 生成默认配置：

```toml
data_dir = "/root/.steward"
db_path = "/root/.steward/steward.db"
api_port = 9527
api_bind = "127.0.0.1"

[embedding]
ollama_url = "http://localhost:11434"
model = "nomic-embed-text"
dimensions = 768
fallback_fts_only = true

[assets]
auto_scan_dirs = []
ignore_patterns = [".git", "node_modules", "__pycache__", ".steward", "target", ".venv"]

[placement]
default_owner = "user:digoal"
default_security_level = "internal"
```
