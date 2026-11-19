# Agent Host Steward (Rust 版) — 本地编译、开发与测试指南

> **版本**: v0.4.0 (Rust Edition)  
> **更新时间**: 2026-11-19  
> **架构基线**: 三位一体架构 (事实图谱 + 偏好规约 + 可插拔记忆) + 多存储引擎 + 协作锁引擎 + Web UI 2.0 + 依赖预检

---

## 一、开发与构建环境要求

### 1. 必备工具链与运行库

1. **Rust 工具链**:
   - `rustc` >= 1.75.0 (推荐稳定版 1.82+)
   - `cargo` >= 1.75.0
   - **注意**: 请确保 PATH 包含 cargo bin：
     ```bash
     export PATH="$HOME/.cargo/bin:$PATH"
     ```

2. **C/C++ 编译工具链 (必须)**:
   - `gcc` (Debian 14+ / GCC 9+)
   - `g++` (提供 `libstdc++` 动态库，DuckDB 与 SQLite 必需)
   - `cmake` >= 3.15 (DuckDB bundled 源码静态构建必需)
   - `make`
   - `pkg-config`

3. **内核与系统能力**:
   - Linux 内核 >= 4.5 (提供 `inotify_init1` 与 `/proc` 进程网络接口)
   - 保证 `/proc` 挂载且可读 (`/proc/net/tcp`、`/proc/[pid]/fd`)

### 2. 可选扩展服务 (按需安装)
- **Ollama**: 本地运行 `http://127.0.0.1:11434`，模型推荐 `nomic-embed-text` 或 `bge-m3`（语义混合检索用，若无则自动降级为 FTS5）；
- **PostgreSQL**: 版本 >= 14 并安装 `pgvector` 扩展（多机高并发共享存储用，若无则使用默认 SQLite）；
- **CodeGraph**: 本地安装 `codegraph` CLI 并在目标仓库建立索引（微观符号级函数调用链穿透用）。

---

## 二、完整工程目录树规范

```
agent-steward/
├── Cargo.toml               # Rust 项目依赖与元数据配置
├── Cargo.lock               # 依赖版本精确锁定
├── build.rs                 # 编译期环境与 Git Commit 动态注入脚本
├── Makefile                 # 构建、测试、预检与部署标准工作流
├── Dockerfile               # 工业级多阶段生产镜像构建文件
├── docker-compose.yml       # 容器化编排参考配置
├── config.example.toml      # 详尽注释的配置模板参考
├── INSTALL.md               # 生产部署与依赖全景排障手册
├── releases/                # 历史各版本详细 Release Notes
│   ├── README.md            # 滚动版本索引与打 Tag 规范
│   ├── v0.1.0.md ~ v0.3.1.md
├── src_doc/                 # 研发规划与架构文档
│   ├── progress.md          # 串行任务断点续传总进度表
│   ├── development.md       # 本开发指南
│   ├── roadmap-phase10-11.md# Phase 10~11 详细设计文档
│   └── roadmap-phase12.md   # Phase 12 依赖显式化与可移植路线图
├── tests/                   # 独立集成与性能基准测试套件
│   ├── coordination_integration.rs # 多 Agent 协作锁并发集成测试 (8 项)
│   ├── duckdb_integration.rs       # DuckDB 分析与增量管道测试 (5 项)
│   └── benchmarks.rs               # 亚毫秒 OLAP 与微观向量基准测试 (5 项)
└── src/                     # Rust 核心源码目录
    ├── main.rs              # CLI 入口与子命令解析 (Clap derive)
    ├── lib.rs               # 核心库导出与版本元信息
    ├── config/              # 配置文件与环境变量管理、参数校验
    ├── preflight/           # 运行时环境与依赖完整性预检引擎
    ├── storage/             # 存储层 SPI (49 方法) + SQLite/PG/DuckDB 三引擎 + 热迁移与增量同步
    ├── coordination/        # 多 Agent 协作锁中枢 (Port/Dir/File 互斥、TTL、Advisory Lock)
    ├── fact/                # 宿主机客观事实图谱 (工具能力/端口/Systemd/Cron/资料编目)
    ├── brief/               # 自适应情境组装引擎 (Dynamic Brief Assembler)
    ├── placement/           # 新对象放置建议引擎 (Rules 2.0 / 意图打分 / 纠偏自学习)
    ├── scanner/             # 资产分级颗粒度扫描器 (Deep/Shallow/Restricted + 断点续扫)
    ├── env/                 # 宿主系统感知与 /proc 动态任务采集器
    ├── memory/              # 记忆管理与可插拔适配器 (内置/PowerContext/Mem0/Letta)
    ├── links/               # 宏微双层依赖拓扑与 CodeGraph 联邦桥接
    ├── watcher/             # inotify 实时监听与防抖 ChangeHandler
    ├── daemon/              # 后台常驻守护进程与自主巡查调度器
    ├── session/             # 会话生命周期自动绑定与安全归档
    ├── hooks/               # HOOK 事件总线、自动同步、客户端生成与 Pipeline 编排
    ├── web/                 # 内嵌 HTTP/SSE 监控大屏服务
    ├── tui/                 # 终端全景实时交互看板
    ├── ecosystem/           # MCP/SKILL 生态目录、外部服务通讯录与 Context 防爆路由
    ├── platform/            # 跨平台原生探针 (Linux/macOS/Windows)
    ├── mesh/                # 轻量多机联邦 HTTP Peer Mesh
    └── api/                 # 39 个原生 MCP 工具端点与 Hook 门禁
```

---

## 三、常用构建、测试与开发命令

### 1. 快速检查与语法校验 (零警告标准)
```bash
make check
# 等价于: cargo check --tests
```

### 2. 运行环境与依赖预检
```bash
make preflight
# 等价于: cargo run -- preflight
```

### 3. 运行测试套件
```bash
# 运行全部单元测试 (119+ 项)
cargo test --lib

# 运行协作锁并发集成测试 (8 项)
cargo test --test coordination_integration

# 运行 DuckDB 列存分析集成测试 (5 项)
cargo test --test duckdb_integration

# 运行性能基准测试 (5 项)
cargo test --test benchmarks

# 一键运行 Makefile 全量测试
make test
```

### 4. 构建与部署发布版
```bash
# 编译 release
make build

# 安装至 /usr/local/bin
sudo make install

# 重启 systemd 服务验证
sudo make restart
```

---

## 四、本地多后端切换调试

### 1. 默认 SQLite 本地测试
无需任何前置命令，运行 `./target/release/agent-steward status` 即自动在 `~/.steward/steward.db` 建库。

### 2. 测试 PostgreSQL 后端
1. 准备本地 PG 实例并创建数据库：
   ```sql
   CREATE DATABASE steward_test;
   \c steward_test
   CREATE EXTENSION IF NOT EXISTS vector;
   ```
2. 执行迁移：
   ```bash
   ./target/release/agent-steward storage migrate --to postgres --to-url "host=127.0.0.1 dbname=steward_test user=postgres"
   ```

### 3. 测试 DuckDB 分析副本同步
```bash
./target/release/agent-steward storage sync --to duckdb
```

---

## 五、开发与提交流程规范

1. **零警告原则**: 任何提交前必须执行 `make check`，确保没有任何 `warning`；
2. **测试驱动 (TDD)**: 新增功能必须在 `tests/` 或对应模块内提供自动化测试；
3. **断点续传推进**: 严格遵循 `src_doc/progress.md` 进度状态，串行推进，完成一步打勾一步并同步至 `/root/new/shared_git/agent-steward`。
