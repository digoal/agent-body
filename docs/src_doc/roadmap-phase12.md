# Agent Host Steward — Phase 12: 运行依赖显式化与可移植部署

> 创建时间: 2026-11-18T16:45:00+08:00  
> 对齐: README.md ISSUE #10（运行依赖要写清楚，防止到其他环境部署失败）  
> 基线: Phase 1~11 全部完成 · 137 测试全绿 · 31 MCP 工具 · v0.3.1  
> 核心原则: **四层贯通** — 代码层面 × 配置层面 × 文档层面 × SKILL 层面

---

## 一、为什么做这件事

### 现状分析（已扫描的差距清单）

| 维度 | 当前状态 | 风险 |
|:---|:---|:---|
| **编译依赖** | `development.md` 仅列出 Rust + gcc + make，未提 cmake（DuckDB bundled 编译依赖） | 新环境 `cargo build` 报 cmake not found |
| **运行时 shared lib** | `ldd` 显示依赖 `libstdc++.so.6` / `libgcc_s.so.1` / `libm.so.6` / `libc.so.6` | 最小化 Alpine 容器缺 libstdc++ 会崩 |
| **Linux 内核功能** | 硬依赖 `/proc/net/tcp`、`/proc/[pid]/fd`、inotify | macOS/Windows/容器（pid namespace 受限）下直接 panic |
| **Systemd 假设** | `fact/automation.rs` 扫描 `/etc/systemd/system/`，硬编码路径 | 非 systemd 系统（Alpine/OpenRC/Busybox init）跳过但无清晰提示 |
| **Ollama 可选依赖** | config.toml 有 `fallback_fts_only = true`，但无版本、模型尺寸、端口探活说明 | 用户装了 Ollama 但模型名不对，静默降级无任何日志 |
| **PostgreSQL 可选依赖** | PG 连接仅在 `config.toml` 配 `storage_backend = "postgres"` 时使用 | 但文档未说明需要哪个 PG 版本、需要哪些扩展（pgvector）、DDL 如何初始化 |
| **配置文件** | `~/.steward/config.toml` 首次运行自动生成，但缺少字段注释、取值范围、依赖关系说明 | 用户改错一个字段整个服务起不来 |
| **部署脚本** | 无 Makefile / install.sh / Dockerfile | 手工 cp + systemctl，步骤散落在多份文档中 |
| **代码内检测** | 无编译期 feature-gate、无运行时依赖预检（preflight check） | 缺少 `/proc` 的容器中直接 panic 而非 graceful fallback |
| **SKILL 文件** | `agent-steward-guide` SKILL.md 中部署说明过时（MCP 工具数写 22，实际 31） | Agent 读 SKILL 获得错误信息 |

### 目标状态

1. **代码层面**: 二进制启动时执行 `preflight_check()`，逐项验证运行环境（/proc、inotify、libstdc++、可选依赖探活），不满足时给出精确诊断而非 panic。
2. **配置层面**: `config.toml` 每个字段有注释、取值范围、依赖关系；新增 `config.example.toml` 参考模板。
3. **文档层面**: 新增 `INSTALL.md` 一站式部署指南（编译依赖 / 运行依赖 / 可选依赖 / 多发行版矩阵 / Docker 部署 / 健康检查）。
4. **SKILL 层面**: 三套内置 SKILL 更新到 v0.3.1 实际状态，部署指南段落引用 `INSTALL.md`。

---

## 二、任务分解与串行执行计划

### 12.1 代码层面 — 运行时依赖预检引擎 (`preflight`)

**子任务 ID**: `12.1`  
**涉及文件**:
- `src/preflight/mod.rs`（新增，~250 行）
- `src/lib.rs`（扩展 `pub mod preflight`）
- `src/main.rs`（daemon / status 子命令启动前调用 `preflight::run_checks()`）
- `src/daemon/mod.rs`（daemon 启动序列嵌入 preflight）

**具体工作**:

1. 新增 `src/preflight/mod.rs`，实现 `PreflightReport` 结构体与 `run_checks()` 函数：
   ```
   CheckItem { name, category, status, message, severity }
   - category: Required / Optional / Informational
   - severity: Error (阻断启动) / Warning (降级运行) / Info (仅记录)
   ```

2. 必检项（Required，不满足则启动失败并给出修复指引）：
   - `check_procfs()`: 验证 `/proc/net/tcp` 可读
   - `check_inotify()`: 验证 `inotify_init` 可用（通过 notify crate 创建临时 watcher 测试）
   - `check_libstdcpp()`: 实际已经在运行了说明没问题，但在 Docker 构建指南中标注
   - `check_data_dir()`: 验证 `~/.steward/` 可写
   - `check_sqlite_db()`: 验证 SQLite DB 可打开（版本兼容）

3. 可选项（Optional，不满足时优雅降级并输出 Warning）：
   - `check_ollama()`: HTTP GET `{ollama_url}/api/tags` 探活，检查配置的 model 是否已拉取
   - `check_postgresql()`: 若配置了 PG 后端，尝试连接并验证 `pgvector` 扩展已安装
   - `check_duckdb()`: 验证 DuckDB in-process 引擎可初始化
   - `check_systemd()`: 验证 `/run/systemd/system` 存在（判断是否 systemd 管理的系统）
   - `check_codegraph()`: 验证 `codegraph` CLI 可执行

4. 信息项（Informational，仅记录便于排障）：
   - `info_os()`: 操作系统版本、内核版本、架构
   - `info_rust_version()`: 编译时 Rust 版本（编译期通过 `env!("CARGO_PKG_VERSION")` 嵌入）
   - `info_binary_features()`: 编译时开启的 feature flags
   - `info_inotify_watches()`: `/proc/sys/fs/inotify/max_user_watches` 当前值

5. CLI 新增 `agent-steward preflight` 子命令，独立输出预检报告（表格格式）。

6. `daemon` 启动序列修改：先调用 `preflight::run_checks()`，Required 项不过则 `exit(1)` 并输出诊断；Optional 项不过则 Warning 日志并标记降级状态。

**验收标准**:
```bash
# preflight 子命令可运行
agent-steward preflight
# 预期: 表格输出所有检查项及状态

# daemon 启动前自动运行 preflight
agent-steward daemon
# 预期: 日志中可见 preflight 报告
```

**单元测试**:
- `test_preflight_procfs_check`
- `test_preflight_ollama_check_offline`（模拟 Ollama 不可达时返回 Warning 而非 Error）
- `test_preflight_report_format`

**断点续传检查点**:
- [x] `src/preflight/mod.rs` 创建
- [x] Required 检查项实现
- [x] Optional 检查项实现
- [x] Informational 检查项实现
- [x] CLI `preflight` 子命令注册
- [x] `daemon` 启动序列嵌入
- [x] 单元测试通过
- [x] `cargo check --tests` 零告警

---

### 12.2 代码层面 — 编译期 Feature 与版本信息嵌入

**子任务 ID**: `12.2`  
**涉及文件**:
- `build.rs`（新增，~40 行）
- `src/preflight/mod.rs`（使用 `env!()` 嵌入的信息）

**具体工作**:

1. 新增 `build.rs`，在编译期注入环境信息到二进制中：
   ```rust
   // build.rs
   fn main() {
       // 编译时间
       println!("cargo:rustc-env=BUILD_TIMESTAMP={}", chrono::Utc::now().to_rfc3339());
       // 编译主机 OS
       println!("cargo:rustc-env=BUILD_OS={}", std::env::consts::OS);
       // 编译主机 ARCH
       println!("cargo:rustc-env=BUILD_ARCH={}", std::env::consts::ARCH);
       // Git commit hash
       let output = std::process::Command::new("git")
           .args(["rev-parse", "--short", "HEAD"])
           .output();
       let git_hash = output.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
           .unwrap_or_else(|_| "unknown".to_string());
       println!("cargo:rustc-env=BUILD_GIT_HASH={}", git_hash);
   }
   ```

2. 在 `--version` 输出中展示完整构建信息：
   ```
   steward 0.3.2 (afa65d4 2026-11-18 linux/x86_64)
   ```

3. 在 `preflight` 报告中展示编译环境 vs 运行环境对比。

**验收标准**:
```bash
agent-steward --version
# 预期包含 git hash、build timestamp、OS/ARCH
```

**断点续传检查点**:
- [x] `build.rs` 创建
- [x] `--version` 输出增强
- [x] preflight 中引用 build info
- [x] 编译通过

---

### 12.3 配置层面 — 带注释的配置参考模板

**子任务 ID**: `12.3`  
**涉及文件**:
- `config.example.toml`（新增，项目根目录）
- `src/config/mod.rs`（增强：字段校验、取值范围检查、依赖关系提示）

**具体工作**:

1. 新增 `config.example.toml`，每个字段有中英文注释、取值范围、依赖关系说明：
   ```toml
   # ============================================================
   # Agent Host Steward 配置文件参考模板
   # 实际配置文件路径: ~/.steward/config.toml
   # ============================================================

   # 数据存储目录（SQLite 数据库、缓存、日志均存于此）
   # 默认: ~/.steward
   # 要求: 目录可写，磁盘空间 >= 100MB
   data_dir = "/root/.steward"

   # SQLite 数据库文件路径
   # 默认: {data_dir}/steward.db
   db_path = "/root/.steward/steward.db"

   # MCP API 监听端口（JSON-RPC 2.0 stdio 模式下此项不生效）
   # 范围: 1024-65535
   # 默认: 9527
   api_port = 9527

   # MCP API 绑定地址
   # 推荐: "127.0.0.1"（仅本机），"0.0.0.0"（所有网卡，慎用）
   api_bind = "127.0.0.1"

   # 存储后端选择
   # 可选: "sqlite"（默认，零依赖嵌入式）| "postgres"（需要外部 PG 实例）
   # storage_backend = "sqlite"

   # PostgreSQL 连接串（仅 storage_backend = "postgres" 时需要）
   # 要求: PostgreSQL >= 14, 需安装 pgvector 扩展
   # 格式: "host=x dbname=y user=z password=w"
   # postgres_url = "host=localhost dbname=steward user=steward password=xxx"

   # ============================================================
   # [embedding] 向量嵌入配置（可选，不配则纯 FTS5 全文检索）
   # ============================================================
   [embedding]
   # Ollama 服务地址
   # 依赖: 本地安装 Ollama (https://ollama.ai)
   # 探活: curl http://localhost:11434/api/tags
   ollama_url = "http://localhost:11434"

   # 嵌入模型名称（需提前拉取: ollama pull <model>）
   # 推荐: "nomic-embed-text"（768维，轻量）或 "bge-m3:latest"（1024维，高精度）
   model = "nomic-embed-text"

   # 嵌入向量维度（必须与模型实际输出维度匹配）
   dimensions = 768

   # Ollama 不可用时是否自动降级为纯 FTS5 检索
   # true = 静默降级（推荐）, false = 报错退出
   fallback_fts_only = true

   # ============================================================
   # [assets] 资产扫描配置
   # ============================================================
   [assets]
   # 自动扫描的顶层目录列表
   # 注意: 大目录（如 / 或 /home）会显著增加首次扫描时间
   auto_scan_dirs = []

   # 忽略的目录/文件模式（glob 语法）
   ignore_patterns = [
       ".git",
       "node_modules",
       "__pycache__",
       ".steward",
       "target",
       ".venv",
   ]

   # ============================================================
   # [placement] 放置建议配置
   # ============================================================
   [placement]
   # 默认资产归属标识
   default_owner = "user:digoal"
   # 默认安全级别: "public" | "internal" | "confidential" | "secret"
   default_security_level = "internal"
   ```

2. 在 `src/config/mod.rs` 的 `load()` 方法中增加字段校验：
   - `api_port` 范围 1024~65535
   - `dimensions` 必须 > 0
   - `storage_backend` 仅接受 `"sqlite"` / `"postgres"`
   - 若 `storage_backend = "postgres"` 但 `postgres_url` 为空，报明确错误

**验收标准**:
```bash
# config.example.toml 存在且内容完整
cat config.example.toml | head -5
# 预期: 有文件头注释

# 配置校验
# 修改 api_port 为 80（< 1024），启动应报错
```

**断点续传检查点**:
- [x] `config.example.toml` 创建
- [x] config 字段校验逻辑实现
- [x] 校验单元测试通过
- [x] 编译零告警

---

### 12.4 文档层面 — 一站式部署指南 `INSTALL.md`

**子任务 ID**: `12.4`  
**涉及文件**:
- `INSTALL.md`（新增，项目根目录，~300 行）
- `README.md`（增加指向 `INSTALL.md` 的链接）

**具体工作**:

编写覆盖以下内容的一站式部署指南：

1. **系统要求矩阵**（表格）：
   ```
   | 组件 | 类型 | 版本要求 | 用途 | 缺失后果 |
   |------|------|---------|------|---------|
   | Linux kernel | 必选 | >= 4.5 (inotify_init1) | 文件监听 | daemon 无法启动 |
   | /proc filesystem | 必选 | - | 进程/端口采集 | env 模块不可用 |
   | libstdc++.so.6 | 必选 | GCC >= 9 | DuckDB C++ runtime | 二进制无法加载 |
   | Rust toolchain | 编译期 | >= 1.75 | 源码编译 | 预编译二进制可跳过 |
   | gcc + g++ | 编译期 | >= 9 | SQLite/DuckDB C 编译 | 源码编译失败 |
   | cmake | 编译期 | >= 3.15 | DuckDB bundled 编译 | 源码编译失败 |
   | make | 编译期 | - | 构建系统 | 源码编译失败 |
   | Ollama | 可选 | any | 向量嵌入混合检索 | 降级为纯 FTS5 |
   | PostgreSQL | 可选 | >= 14 + pgvector | 高并发多后端存储 | 默认 SQLite 足够 |
   | CodeGraph | 可选 | any | 微观符号级代码穿透 | 仅宏观文件级依赖 |
   | systemd | 可选 | >= 245 | 自动化拓扑扫描 | 跳过 systemd 拓扑 |
   ```

2. **多发行版安装命令**:
   - Debian/Ubuntu
   - RHEL/CentOS/Fedora
   - Alpine Linux（特别标注 libstdc++ 需要额外安装）
   - Arch Linux

3. **从源码编译**（带所有前置步骤）

4. **从预编译二进制安装**（未来 CI 产出的 release artifact）

5. **Docker 部署**:
   - 多阶段构建 Dockerfile（builder + runtime）
   - docker-compose.yml（含 PG 可选依赖）

6. **Systemd 守护进程配置**（含 Unit 文件完整内容）

7. **健康检查与故障排除**:
   - `agent-steward preflight` 一键检查
   - 常见错误速查表

8. **可选依赖逐项配置指南**:
   - Ollama 安装、模型拉取、连通性验证
   - PostgreSQL + pgvector 安装与 DDL 初始化
   - CodeGraph 安装与索引初始化

**验收标准**:
- 文档中每条命令都可在对应发行版上直接复制粘贴执行
- 覆盖编译依赖 / 运行依赖 / 可选依赖的完整安装命令

**断点续传检查点**:
- [x] INSTALL.md 框架与系统要求矩阵
- [x] 多发行版安装命令
- [x] 从源码编译指南
- [x] Docker 部署（Dockerfile + docker-compose.yml）
- [x] Systemd 配置
- [x] 健康检查与故障排除
- [x] 可选依赖配置指南
- [x] README.md 添加链接

---

### 12.5 代码层面 — Dockerfile 与 Makefile

**子任务 ID**: `12.5`  
**涉及文件**:
- `Dockerfile`（新增）
- `docker-compose.yml`（新增）
- `Makefile`（新增）
- `.dockerignore`（新增）

**具体工作**:

1. **Dockerfile**（多阶段构建）：
   ```dockerfile
   # Stage 1: Builder
   FROM rust:1.82-bookworm AS builder
   RUN apt-get update && apt-get install -y cmake g++ make pkg-config
   WORKDIR /app
   COPY . .
   RUN cargo build --release

   # Stage 2: Runtime
   FROM debian:bookworm-slim
   RUN apt-get update && apt-get install -y libstdc++6 && rm -rf /var/lib/apt/lists/*
   COPY --from=builder /app/target/release/agent-steward /usr/local/bin/
   EXPOSE 9527
   ENTRYPOINT ["agent-steward"]
   CMD ["daemon"]
   ```

2. **docker-compose.yml**（含可选 PG）

3. **Makefile**:
   ```makefile
   .PHONY: build test install preflight clean docker

   build:
       cargo build --release

   test:
       cargo test

   install: build
       install -m 755 target/release/agent-steward /usr/local/bin/

   preflight:
       agent-steward preflight

   clean:
       cargo clean

   docker:
       docker build -t agent-steward:latest .
   ```

4. **.dockerignore**

**验收标准**:
```bash
# Makefile 可用
make build
make test

# Docker 可构建（仅验证 Dockerfile 语法，实际构建耗时长可跳过）
docker build --check .   # 或 hadolint Dockerfile
```

**断点续传检查点**:
- [x] Dockerfile 创建
- [x] docker-compose.yml 创建
- [x] Makefile 创建
- [x] .dockerignore 创建
- [x] make build / make test 验证

---

### 12.6 文档层面 — 更新 `development.md`

**子任务 ID**: `12.6`  
**涉及文件**:
- `docs/src_doc/development.md`（全面重写）

**具体工作**:

1. 更新版本号（v0.2.0 → v0.3.2）
2. 更新目录树（补充 `coordination/`、`preflight/`、`brief/`、`fact/` 等新模块）
3. 更新依赖清单（补充 cmake、DuckDB bundled、PostgreSQL 可选依赖）
4. 更新测试命令（137 项测试，分 lib / integration / benchmarks）
5. 新增"运行时依赖预检"章节（`agent-steward preflight`）
6. 新增"Docker 开发环境"章节
7. 新增"多后端存储切换"章节
8. 新增"常见编译/运行问题 FAQ"

**断点续传检查点**:
- [x] development.md 重写完成
- [x] 目录树更新
- [x] 依赖清单更新
- [x] 测试命令更新
- [x] FAQ 添加

---

### 12.7 SKILL 层面 — 三套内置技能更新

**子任务 ID**: `12.7`  
**涉及文件**:
- `skills/agent-steward-guide/SKILL.md`（更新）
- `skills/agent-steward-developer/SKILL.md`（更新）
- `skills/agent-steward-doc-writer/SKILL.md`（更新）

**具体工作**:

1. **agent-steward-guide**:
   - MCP 工具数更新：22 → 31
   - 新增模块说明：`coordination/`、`preflight/`、`brief/`、`fact/`
   - 部署指南段落引用 `INSTALL.md` 而非内联
   - 新增 `preflight` 子命令使用说明
   - 新增协作锁使用说明
   - 更新测试数：40 → 137

2. **agent-steward-developer**:
   - 开发规范新增：所有新模块必须在 `preflight` 中注册依赖检查
   - 编译指南引用 `INSTALL.md` 编译依赖章节
   - 更新当前模块全景图

3. **agent-steward-doc-writer**:
   - 文档写作规范新增：涉及外部依赖时必须标注"必选/可选"与版本范围

**断点续传检查点**:
- [x] agent-steward-guide 更新
- [x] agent-steward-developer 更新
- [x] agent-steward-doc-writer 更新

---

### 12.8 集成验收 — 全量测试 + Git 提交 + 同步

**子任务 ID**: `12.8`  
**涉及文件**:
- `docs/src_doc/progress.md`（追加 Phase 12 条目）
- `Cargo.toml`（版本号升至 `0.3.2`）

**具体工作**:

1. 全量测试：
   ```bash
   cargo check --tests           # 零告警
   cargo test --lib              # 单元测试全绿
   cargo test --test coordination_integration
   cargo test --test duckdb_integration
   cargo test --test benchmarks
   ```

2. Release 构建 + 部署 + preflight 验证：
   ```bash
   cargo build --release
   install -m 755 target/release/agent-steward /usr/local/bin/
   agent-steward preflight
   systemctl restart agent-steward.service
   ```

3. 版本号升级：`Cargo.toml` version → `0.3.2`

4. 更新 `docs/src_doc/progress.md`：追加 Phase 12 条目

5. 新增 `docs/releases/v0.3.2.md` release note

6. Git 提交与 tag：
   ```bash
   git add -A
   git commit -m "feat(phase12): runtime dependency preflight, INSTALL.md, Dockerfile, config template & SKILL sync"
   git tag -a v0.3.2 -m "Release v0.3.2 — 运行依赖显式化与可移植部署"
   ```

7. 同步到 `/root/new/shared_git/agent-steward`

**断点续传检查点**:
- [x] 全量测试通过
- [x] Release 构建成功
- [x] preflight 验证通过
- [x] 守护进程重启成功
- [x] 版本号升级
- [x] progress.md 更新
- [x] release note 创建
- [x] Git 提交 + tag
- [x] shared_git 同步

---

## 三、执行顺序与依赖关系

```
12.1 preflight 引擎
  │
  ├── 12.2 build.rs 版本嵌入 (依赖 12.1 的 info 展示)
  │
  ├── 12.3 config.example.toml (独立)
  │
  12.4 INSTALL.md (引用 12.1 的 preflight 命令)
  │
  ├── 12.5 Dockerfile + Makefile (引用 12.4 的依赖清单)
  │
  ├── 12.6 development.md 重写 (引用 12.1~12.5 新增内容)
  │
  ├── 12.7 SKILL 更新 (引用 12.4 INSTALL.md)
  │
  12.8 集成验收 + 发布
```

**串行执行路径**: `12.1 → 12.2 → 12.3 → 12.4 → 12.5 → 12.6 → 12.7 → 12.8`

---

## 四、预估工作量

| 子任务 | 新增/改动行数 | 新增文件数 |
|:---:|:---:|:---:|
| 12.1 | ~300 行 Rust | 1 |
| 12.2 | ~50 行 Rust | 1 |
| 12.3 | ~80 行 TOML | 1 |
| 12.4 | ~350 行 Markdown | 1 |
| 12.5 | ~100 行 (Dockerfile + docker-compose + Makefile + .dockerignore) | 4 |
| 12.6 | ~200 行 Markdown (重写) | 0 |
| 12.7 | ~150 行 Markdown (更新 3 个 SKILL) | 0 |
| 12.8 | ~80 行 (progress + release note) | 1 |
| **合计** | **~1310 行** | **9 个新文件** |

---

## 五、关键设计决策

### 决策 7：preflight 检查不过时是阻断还是降级？

**决策**: Required 项阻断（exit 1），Optional 项降级运行。理由：
- `/proc` 不可用时，`env` 模块所有功能直接 panic，不如在入口拦截给出诊断
- Ollama 不可用时，混合检索降级为纯 FTS5，功能子集仍然可用
- 把"哪些必须有、哪些可以没有"的判断固化到代码里，而不是靠文档口口相传

### 决策 8：config.example.toml 还是在代码里生成默认配置？

**决策**: 两者都要。理由：
- `config.example.toml` 是给人看的参考文档（带注释、带说明）
- 代码中的 `Config::default()` + `toml::to_string()` 是给首次启动自动生成的（不带注释但结构正确）
- 两者互补，不矛盾

---

> **下一步**: 执行 12.1 — 创建 `src/preflight/mod.rs`，实现运行时依赖预检引擎。
