# 05 — 架构设计（Architecture）

> Agent Host Steward 产品设计 · 交付件 5/7
> 版本: v1.0 · 更新: 2026-11-15

---

## 一、架构目标

| 目标 | 量化指标 |
|------|----------|
| 低资源占用 | 常驻内存 < 100MB，CPU 空闲时 < 1% |
| 低延迟 | 单次查询 < 200ms（万级条目），brief 汇总 < 500ms |
| 高可靠 | 单进程 crash 后自动恢复，数据零丢失 |
| 可扩展 | 新模块可插拔，不需要改核心 |
| 零外部依赖部署 | 一个二进制 + 一个 SQLite 文件即可跑起来 |
| 隐私安全 | 全部数据在本地，无外传通道 |

---

## 二、整体架构

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Agent / 用户                                │
│          (Antigravity, Cursor, Aider, LangChain, 自定义...)         │
└──────────┬──────────────┬───────────────────┬───────────────────────┘
           │ MCP          │ REST              │ CLI
           ▼              ▼                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        API Gateway                                  │
│   认证(本地 token) · 限流 · 路由 · 请求日志                          │
└──────────┬──────────────┬───────────────────┬───────────────────────┘
           │              │                   │
     ┌─────▼─────┐ ┌─────▼──────┐ ┌──────────▼────────┐
     │  Memory    │ │  Assets    │ │  Environment      │
     │  Service   │ │  Service   │ │  Service           │
     └─────┬─────┘ └─────┬──────┘ └──────────┬────────┘
           │              │                   │
     ┌─────▼─────┐ ┌─────▼──────┐ ┌──────────▼────────┐
     │  Changes   │ │  Links     │ │  Scheduler        │
     │  Service   │ │  Service   │ │  Service           │
     └─────┬─────┘ └─────┬──────┘ └──────────┬────────┘
           │              │                   │
           └──────────────┼───────────────────┘
                          │
              ┌───────────▼───────────┐
              │     Core Engine       │
              │  ┌─────────────────┐  │
              │  │  Storage Layer  │  │
              │  │  SQLite + FTS5  │  │
              │  │  + Vec Extension│  │
              │  └─────────────────┘  │
              │  ┌─────────────────┐  │
              │  │  Index Manager  │  │
              │  │  FTS · Vec · Tag│  │
              │  └─────────────────┘  │
              │  ┌─────────────────┐  │
              │  │ Embedding Engine│  │
              │  │ Local Ollama    │  │
              │  └─────────────────┘  │
              └───────────────────────┘
```

---

## 三、技术选型

### 3.1 运行时与语言

| 组件 | 选型 | 理由 |
|------|------|------|
| MVP 阶段 | **Go** | 开发效率高、单二进制、SQLite/MCP 生态绑定成熟、适合快速验证产品价值 |
| 1.0+ 评估 | Rust | 若 MVP 验证成功且性能/内存有瓶颈，评估用 Rust 重写核心热路径 |
| 构建 | `go build` | 标准工具链，交叉编译简单 |
| 测试 | `go test` + 集成测试 | 覆盖率目标 > 80% |

> **决策理由**: steward 是单机本地工具（非高并发服务器），MVP 阶段的核心风险是"产品定义对不对"而非"性能够不够"。Go 的开发速度约为 Rust 的 2-3 倍，可以更快验证核心场景（brief / memory / env）是否真正解决用户痛点。Rust 的优势（零 GC 停顿、极低内存占用）在用户量和数据规模上来后才会成为决定性因素。

### 3.2 存储

| 组件 | 选型 | 理由 |
|------|------|------|
| 主存储 | **SQLite** | 零配置、单文件、ACID、嵌入式、无需网络 |
| 全文搜索 | **FTS5** (SQLite 内置) | 支持 BM25 排序、中文分词通过 ICU 或 simple tokenizer + jieba 分词预处理 |
| 向量搜索 | **sqlite-vec** 扩展 | 嵌入在同一 SQLite 中，无需额外向量数据库 |
| 文件存储 | 本地文件系统 | 大文件（PDF、图片）不入库，只存路径和元数据 |
| 配置 | TOML | 人可读、编辑友好 |

### 3.3 向量与嵌入

| 组件 | 选型 | 理由 |
|------|------|------|
| Embedding 模型 | **nomic-embed-text** via Ollama | 本地运行、不外传、768维、多语言、性能好 |
| 备选 | mxbai-embed-large, bge-m3 | 都可通过 Ollama 本地运行 |
| 向量维度 | 768 | 平衡精度和存储 |
| 相似度 | 余弦距离 | 标准选择 |

### 3.5 技术风险项

#### sqlite-vec 成熟度风险

sqlite-vec（Alex Garcia）是相对年轻的扩展，需注意：
- **HNSW 增量插入/删除**：需实测验证。MVP 阶段数据量 < 10K，可先用暴力扫描（`vec0` 默认 flat index），性能已足够（10K × 768d 暴力搜索约 20ms）。
- **与 FTS5 触发器的共存**：两者都通过虚拟表工作，共用同一个 SQLite 连接，目前无已知冲突，但需要集成测试覆盖。
- **降级策略**：若 sqlite-vec 在特定平台上编译困难，可降级为纯 FTS5 模式（关键词搜索），向量搜索通过外部进程（如 Python faiss-lite）补充。

**决策**: MVP 用 sqlite-vec flat index。数据量超 10K 后评估切 HNSW 或外部向量库。

#### FTS5 中文分词方案

FTS5 内置的 `unicode61` tokenizer 按 Unicode 字符类别分词，对中文效果差（按字拆分而非按词）。

**决策: 入库前 jieba 分词（路径 1）**

```
写入流程:
  原始中文文本
       │
       ▼
  jieba.cut(text)  →  "项目 steward 主 存储 选型 确定 为 SQLite"
       │
       ▼
  空格分隔的分词结果存入 FTS5
       │
       ▼
  FTS5 用 unicode61 tokenizer 按空格切分（已是词级别）

查询流程:
  用户查询 query
       │
       ▼
  jieba.cut(query)  →  分词
       │
       ▼
  FTS5 MATCH 查询
```

Go 生态可用 `yanyiwu/gojieba` 或 `go-ego/gse`（纯 Go，无 cgo 依赖）。

这种方式的优势是 FTS5 不需要自定义 tokenizer，也不依赖 ICU 编译，兼容性最好。代价是写入和查询都多一步分词，但开销极小（< 1ms/条）。

### 3.4 接口协议

| 协议 | 用途 | 实现 |
|------|------|------|
| **MCP** (Model Context Protocol) | Agent 调用主通道 | stdio transport (MVP) + SSE transport (P1) |
| **REST** | 外部集成、Web UI、脚本调用 | HTTP/JSON, 绑定 127.0.0.1 |
| **CLI** | 人工操作、调试、脚本集成 | steward 子命令 |

---

## 四、进程模型

```
                  ┌──────────────────┐
                  │   steward daemon  │
                  │   (主进程)         │
                  │                   │
                  │  ┌──────────────┐ │
      systemd     │  │ MCP Server   │ │◄─── Agent 通过 stdio 连接
    或 launchd ───►│  │ (stdio)      │ │
      启动管理     │  └──────────────┘ │
                  │  ┌──────────────┐ │
                  │  │ REST Server  │ │◄─── curl / Web UI / 脚本
                  │  │ (HTTP 9527)  │ │
                  │  └──────────────┘ │
                  │  ┌──────────────┐ │
                  │  │ Scheduler    │ │     内置定时器
                  │  │ (cron-like)  │ │     环境快照、巡检、维护
                  │  └──────────────┘ │
                  │  ┌──────────────┐ │
                  │  │ File Watcher │ │     inotify / FSEvents
                  │  │ (可选)       │ │     监听关注目录变更
                  │  └──────────────┘ │
                  └──────┬───────────┘
                         │
                   ┌─────▼─────┐
                   │ SQLite DB │  ~/.steward/steward.db
                   │ + FTS5    │  ~/.steward/config.toml
                   │ + Vec     │  ~/.steward/logs/
                   └───────────┘
```

### 4.1 进程生命周期

1. **安装**: `curl -sSL https://steward.dev/install | sh` 下载二进制到 `/usr/local/bin/steward`
2. **初始化**: `steward init` → 创建 `~/.steward/` 目录、初始化 SQLite 数据库、生成默认配置
3. **启动**: `steward start` → 以 daemon 模式运行，同时启动 MCP/REST/Scheduler
4. **连接**: Agent 通过 `steward mcp` 子命令获得 stdio MCP 通道
5. **停止**: `steward stop` → 优雅关闭，刷盘所有数据
6. **自恢复**: 进程崩溃后 systemd/launchd 自动重启，SQLite WAL 保证数据一致性

### 4.2 MCP 连接模式

Agent 连接 steward 有两种模式：

**模式 A: stdio（MVP）**
```
Agent Process
    │ spawn
    ▼
steward mcp   ←──stdin/stdout──► Agent
    │
    ▼ (内部连接)
steward daemon
```

Agent 启动 `steward mcp` 子进程，这个子进程通过 Unix Socket 连接到 daemon，自身作为 Agent 和 daemon 之间的 MCP 桥接。

**模式 B: SSE（P1）**
```
Agent Process ──── HTTP/SSE ────► steward daemon:9527/mcp
```

Agent 直接通过 HTTP SSE 连接 daemon 的 MCP endpoint，无需额外进程。

---

## 五、存储架构

### 5.1 数据库 Schema 概览

```
steward.db
├── memories          # 记忆表
├── memories_fts      # FTS5 全文索引
├── memories_vec      # 向量索引 (sqlite-vec)
├── assets            # 资产表
├── assets_fts        # 资产全文索引
├── asset_tags        # 资产标签关联
├── services          # 服务/环境表
├── env_snapshots     # 环境快照表
├── changes           # 变更事件表 (P1)
├── links             # 关联关系表 (P2)
├── config            # 运行时配置 KV
└── migrations        # 版本迁移记录
```

### 5.2 SQLite 配置优化

```sql
-- WAL 模式，支持并发读
PRAGMA journal_mode = WAL;

-- 合理的 cache size（默认 2000 pages = ~8MB）
PRAGMA cache_size = -8000;  -- 8MB

-- 同步模式：NORMAL，在 WAL 下依然安全
PRAGMA synchronous = NORMAL;

-- 启用外键
PRAGMA foreign_keys = ON;

-- mmap 加速大表读取（64MB）
PRAGMA mmap_size = 67108864;
```

### 5.3 Embedding 流程

```
写入记忆/资产 content
       │
       ▼
  是否有 Ollama 可用？
       │
   ┌───┴───┐
   │ Yes   │ No
   ▼       ▼
Ollama API  纯 FTS5 模式
(nomic-     (降级: 只做关键词搜索,
embed-text)  向量字段留空,
   │         后台排队等 Ollama 恢复)
   ▼
768维向量
   │
   ▼
写入 sqlite-vec 虚拟表
```

**关键决策**: Embedding 不阻塞写入。如果 Ollama 不可用，先写入原始内容和 FTS 索引，向量列为空，标记 `embedding_status = pending`。后台 Scheduler 定期扫描 pending 项，在 Ollama 恢复后补充向量。

---

## 六、搜索架构

### 6.1 混合搜索（Hybrid Search）

```
用户查询 query
       │
       ├────────────────────────┐
       ▼                        ▼
  FTS5 全文搜索              向量语义搜索
  (BM25 打分)                (余弦相似度)
       │                        │
       ▼                        ▼
  FTS 结果集                 Vec 结果集
  [(id, bm25_score)]         [(id, cosine_sim)]
       │                        │
       └────────────┬───────────┘
                    ▼
            RRF (Reciprocal Rank Fusion)
            score = Σ 1/(k + rank_i)
                    │
                    ▼
            合并排序结果
                    │
                    ▼
            应用过滤条件
            (type, time_range, tags, status)
                    │
                    ▼
            应用权重修正
            (weight 字段加权)
                    │
                    ▼
            Top-N 返回
```

### 6.2 RRF 参数

```
k = 60  (标准值，平衡精确匹配与语义匹配)
α = 0.5 (FTS 和 Vec 的相对权重，可配置)
```

用户可通过 API 参数调整 `α`：
- `α = 1.0`：纯关键词搜索
- `α = 0.0`：纯语义搜索
- `α = 0.5`：均衡混合（默认）

---

## 七、文件监听架构

### 7.1 监听策略

不做全盘监听（太重），采用 **注册制 + 智能默认**：

```
注册关注目录列表
(config.toml 配置)
       │
       ▼
  inotify / FSEvents
  监听注册的目录树
       │
       ▼
  事件过滤
  (忽略 .git/, node_modules/,
   __pycache__/, .steward/ 等)
       │
       ▼
  防抖 (debounce 300ms)
  同文件连续修改合并为一次
       │
       ▼
  变更事件入库
  changes 表 + 关联更新
```

### 7.2 智能默认目录

初始化时自动注册这些目录（如果存在）：

| 目录 | 理由 |
|------|------|
| `~/projects/` | 常见项目根 |
| `~/work/` | 工作目录 |
| `~/Documents/` | 文档 |
| 用户 `$HOME` 下的 `.bashrc`, `.zshrc`, `.gitconfig` 等 | 环境配置 |

用户可通过 `steward watch add <path>` 增加、`steward watch rm <path>` 移除。

---

## 八、环境感知架构

### 8.1 数据采集器

```
┌────────────────────────┐
│   Environment Service  │
│                        │
│  ┌─────────────────┐   │
│  │ System Collector │   │  uname, cpu, mem, disk, gpu
│  ├─────────────────┤   │
│  │ Service Detector │   │  ss -tlnp → 端口扫描 → 服务识别
│  ├─────────────────┤   │  systemctl/launchctl/docker
│  │ Software Scanner │   │  dpkg/rpm/brew → 已安装软件
│  ├─────────────────┤   │
│  │ Network Checker  │   │  ip addr, 代理检测, DNS
│  ├─────────────────┤   │
│  │ Cron Scanner     │   │  crontab, systemd timers, hermes cron
│  ├─────────────────┤   │
│  │ Shell Env        │   │  $PATH, 关键环境变量
│  └─────────────────┘   │
│           │             │
│           ▼             │
│    Snapshot Assembler   │  → env_snapshots 表
│           │             │
│           ▼             │
│    Diff Engine (P1)     │  → changes 表
└────────────────────────┘
```

### 8.2 服务识别策略

不依赖固定列表，使用 **端口 → 进程 → 特征** 三层识别：

```
ss -tlnp 获取监听端口列表
          │
          ▼
    每个端口查 pid
          │
          ▼
    /proc/<pid>/exe  → 进程名
    /proc/<pid>/cmdline → 启动参数
          │
          ▼
    特征匹配规则库
    ┌─────────────────────────────────┐
    │ postgres  → PostgreSQL          │
    │ mysqld    → MySQL               │
    │ redis     → Redis               │
    │ ollama    → Ollama              │
    │ node      → Node.js App         │
    │ docker    → Docker Engine       │
    │ ...       → 扩展规则文件         │
    └─────────────────────────────────┘
          │
          ▼
    尝试获取版本
    (pg: SELECT version(),
     redis: INFO server,
     mysql: SELECT @@version)
```

---

## 九、安全架构

### 9.1 访问控制

| 通道 | 认证方式 | 说明 |
|------|----------|------|
| CLI | 本地 Unix 用户身份 | 同用户直接访问，无需额外认证 |
| MCP (stdio) | 继承父进程权限 | Agent spawn 的子进程，天然可信 |
| REST | Bearer Token | `steward init` 时生成随机 token，存 `~/.steward/token` |
| REST (远程) | 默认禁止 | 只绑 127.0.0.1，需显式配置才能绑 0.0.0.0（不推荐） |

### 9.2 数据安全

- **数据库文件权限**: `chmod 600 ~/.steward/steward.db`
- **配置文件权限**: `chmod 600 ~/.steward/config.toml`
- **Token 文件权限**: `chmod 600 ~/.steward/token`
- **无网络外传**: 不含任何 telemetry、auto-update、cloud sync 代码
- **敏感信息标记**: 记忆/资产可标记 `sensitive: true`，标记后：
  - REST 响应中该字段用 `***` 掩码
  - 不出现在 `steward brief` 的汇总中
  - 导出时自动跳过

---

## 十、部署架构

### 10.1 安装方式

| 方式 | 命令 | 适用场景 |
|------|------|----------|
| 一键安装 | `curl -sSL https://steward.dev/install \| sh` | 普通用户 |
| Homebrew | `brew install steward` | macOS |
| apt/dnf | `apt install steward` | Debian/Ubuntu, Fedora |
| Docker | `docker run -v ~/.steward:/data steward` | 容器化部署 |
| 源码编译 | `go build -o steward ./cmd/steward` | 开发者/定制 |

### 10.2 目录结构

```
~/.steward/
├── steward.db          # 主数据库（SQLite）
├── steward.db-wal      # WAL 文件
├── steward.db-shm      # 共享内存
├── config.toml         # 配置文件
├── token               # REST API token
├── rules/              # 服务识别规则扩展
│   └── custom.toml
├── logs/
│   ├── steward.log     # 主日志
│   └── steward.log.1   # 轮转日志
├── backups/            # 自动备份
│   ├── steward-2026-11-14.db
│   └── steward-2026-11-13.db
└── plugins/            # 插件目录 (P2)
```

### 10.3 自动备份

Scheduler 每天 03:00 自动备份：
```
SQLite Online Backup API
→ ~/.steward/backups/steward-<date>.db
→ 保留最近 7 天
→ 超出自动删除
```

---

## 十一、性能设计

### 11.1 热路径优化

| 操作 | 目标延迟 | 优化手段 |
|------|----------|----------|
| 写入 1 条记忆 | < 50ms | WAL + async embedding |
| 关键词搜索 | < 100ms | FTS5 索引 |
| 语义搜索 | < 200ms | sqlite-vec HNSW, embedding 预计算 |
| 混合搜索 | < 200ms | 并行执行 FTS + Vec |
| `steward brief` | < 500ms | 缓存上次环境快照 + 最近记忆 top-10 |
| 环境快照（冷） | < 5s | 并发采集各维度 |
| 环境快照（热） | < 500ms | 增量更新 + 缓存 |

### 11.2 缓存策略

```
┌─────────────────────┐
│   In-Memory Cache    │
│                      │
│  env_snapshot_cache  │  最近一次完整快照, TTL 5min
│  brief_cache         │  最近一次 brief 结果, TTL 1min
│  service_cache       │  服务列表, TTL 1min
│  recent_memories     │  最近 100 条记忆, LRU
└──────────┬──────────┘
           │ miss
           ▼
    SQLite 查询
```

### 11.3 容量规划

| 规模 | 记忆条数 | 资产条数 | 数据库大小（估算） | 内存占用 |
|------|----------|----------|--------------------| ---------|
| 小型（个人） | < 10K | < 50K | < 100MB | ~50MB |
| 中型（重度用户） | 10K-100K | 50K-500K | 100MB - 1GB | ~100MB |
| 大型（极端） | > 100K | > 500K | > 1GB | ~200MB |

超过 100K 记忆建议开启自动归档（archived 状态的记忆移到 archive.db）。

---

## 十二、可扩展性设计

### 12.1 插件机制（P2）

```
steward daemon
       │
       ▼
  Plugin Manager
       │
  ┌────┼────────────────┐
  │    │                 │
  ▼    ▼                 ▼
Hook  Tool              Collector
Plugin Plugin           Plugin
│      │                 │
│  Agent 可调用的      新的环境采集器
│  自定义工具           (如 K8s 集群信息)
│
事件钩子
(on_memory_write,
 on_asset_change,
 on_env_diff)
```

插件通过 WASM 沙箱运行（安全隔离），或通过子进程 + stdin/stdout 通信（简单但需信任）。

### 12.2 多 Agent 并发

```
Agent A ──┐
Agent B ──┼──► steward daemon ──► SQLite (WAL)
Agent C ──┘
```

SQLite WAL 模式支持多个并发读者 + 1 个写者。steward daemon 是唯一写者，通过内部写入队列序列化写操作。多个 Agent 同时调用 MCP 不会冲突。

---

## 十三、冷启动策略

新用户首次 `steward init` 后的冷启动流程：

```
steward init
    │
    ├─ 1. 创建 ~/.steward/ 及数据库
    │
    ├─ 2. 快速环境扫描 (~2s)
    │      uname, cpu, mem, disk
    │      ss -tlnp → 识别运行中的服务
    │      dpkg/rpm list → 已安装软件(后台异步)
    │
    ├─ 3. 智能目录发现 (~1s)
    │      扫描 $HOME 下第一层目录
    │      识别项目目录 (.git, package.json, Cargo.toml 等)
    │      注册为资产
    │
    ├─ 4. Agent 历史导入提示
    │      检测 ~/.gemini/ → 提示导入 Antigravity 历史
    │      检测 ~/.cursor/ → 提示导入 Cursor 历史
    │      检测 ~/.chatgpt/ → 提示导入 ChatGPT 导出
    │
    ├─ 5. Ollama 检测
    │      curl http://localhost:11434/api/tags
    │      有 → 记录可用模型列表, 自动选 embedding 模型
    │      无 → 提示安装, 降级为纯 FTS 模式
    │
    └─ 6. 启动 daemon
           开始监听 MCP + REST
           后台继续深度扫描（不阻塞）
```

**冷启动到可用：< 5 秒。** 后台深度扫描可能持续 30s-2min，但不阻塞用户。

---

## 十四、数据迁移与版本管理

### 14.1 Schema 版本

每次 schema 变更在 `migrations` 表记录版本号。启动时检查：

```
当前 db schema_version = N
代码期望 schema_version = M

if N < M:
    逐版本执行 migration N+1, N+2, ..., M
    更新 schema_version = M
if N > M:
    报错: 数据库版本高于软件版本, 请升级 steward
if N == M:
    正常启动
```

### 14.2 数据导出/导入

```bash
# 导出全部数据为 JSON（可选过滤）
steward export --format json --output backup.json

# 导出为 SQLite 副本
steward export --format sqlite --output backup.db

# 导入
steward import --from backup.json

# 从另一台机器迁移
scp remote:~/.steward/steward.db .
steward import --from steward.db --merge
```

---

*下一步: 06-api-spec.md — MCP/REST/CLI 接口定义*
