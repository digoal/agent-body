# Agent Host Steward — 生产安装与可移植部署指南 (INSTALL.md)

> **适用版本**: v0.4.0+  
> **更新时间**: 2026-11-19  
> **文档目标**: 本文档全面说明 Agent Host Steward 在不同操作系统与容器环境下的**编译依赖**、**运行时依赖**、**可选增强服务**与**一键部署流程**，防止因环境差异导致部署或运行失败。

---

## 一、系统与运行依赖全景矩阵

大管家采用 Rust 开发，核心设计理念是**“零外部强依赖、单二进制开箱即用、渐进式功能增强”**。

| 组件 / 依赖项 | 依赖类型 | 最低版本要求 | 核心用途 | 缺失后果与降级表现 |
|:---|:---:|:---:|:---|:---|
| **Linux Kernel** | **REQUIRED** | >= 4.5 | 提供 `inotify` 文件事件监听、`/proc` 进程网络拓扑 | 非 Linux 系统（或受限容器）无法启动守护进程 |
| **Linux procfs** | **REQUIRED** | — | `/proc/net/tcp` 端口占用、`/proc/[pid]/` 动态进程识别 | 无法进行端口防撞预警与动态任务识别 |
| **C++ 运行时库** | **REQUIRED** | GCC >= 9 (libstdc++.so.6) | 供 DuckDB 列存引擎与 SQLite Bundled 动态调用 | 极简 Alpine/Busybox 容器若无 `libstdc++6` 将报找不到动态库 |
| **Rust 工具链** | *编译期* | >= 1.75.0 (推荐 1.82+) | 从源码编译 Rust 二进制 | 若使用预编译发布二进制包，则**无需安装** |
| **C/C++ 编译器** | *编译期* | gcc / g++ >= 9 或 clang | 编译 `rusqlite` 与 `duckdb` 内置 C/C++ 引擎 | 源码编译失败 |
| **CMake & Make** | *编译期* | CMake >= 3.15, GNU Make | DuckDB Bundled 静态构建支持 | 源码编译构建阶段报错退出 |
| **Ollama 向量服务** | OPTIONAL | >= 0.1.20 (`nomic-embed-text` / `bge-m3`) | 长期记忆双路向量混合检索 (RRF) | **自动降级**为内置 SQLite FTS5 纯全文检索，绝不崩溃 |
| **Ollama 元数据标注模型** | OPTIONAL | >= 0.1.20 (`qwen3.5:0.8b`) | 资产 L2 本地异步脱敏语义标注精炼 | **自动降级**为纯 L1 Rust 确定性语法提取，零云端 Token |
| **PostgreSQL** | OPTIONAL | >= 14 (需 pgvector) | 跨机多 Agent 高并发共享存储后端 | 默认采用内置嵌入式 SQLite，无需安装外部数据库 |
| **Systemd** | OPTIONAL | >= 245 | 系统自动化拓扑扫描、常驻后台守护服务托管 | 容器或 OpenRC 环境下跳过拓扑，可使用 Docker/nohup 运行 |
| **CodeGraph CLI** | OPTIONAL | >= 0.1.0 | 宏微双层联邦微观代码函数符号穿透 | **自动降级**为文件与目录级宏观拓扑分析 |

---

## 二、多 Linux 发行版环境准备命令

### 1. Debian / Ubuntu 系统 (推荐)
```bash
# 1. 基础编译与运行时依赖安装
sudo apt-get update && sudo apt-get install -y \
    build-essential \
    gcc \
    g++ \
    cmake \
    make \
    pkg-config \
    libstdc++6 \
    curl \
    git

# 2. 安装 Rust 工具链 (若已有 rustc 则跳过)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

### 2. RHEL / CentOS Stream / Rocky Linux / Fedora
```bash
# 1. 基础编译与运行时依赖安装
sudo dnf groupinstall -y "Development Tools"
sudo dnf install -y gcc gcc-c++ cmake make libstdc++ curl git

# 2. 安装 Rust 工具链
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

### 3. Alpine Linux (轻量容器环境注意)
> ⚠️ **关键注意**: Alpine 默认采用 musl libc，必须显式安装 `libstdc++` 和 `gcompat`，且确保 `/proc` 已挂载。
```bash
apk update && apk add --no-cache \
    build-base \
    gcc \
    g++ \
    cmake \
    make \
    libstdc++ \
    curl \
    git \
    bash
```

---

## 三、一键编译与生产安装

### 方案 A：从源码直接编译安装 (推荐)

```bash
# 1. 克隆代码仓库
git clone https://github.com/digoal/agent-steward.git
cd agent-steward

# 2. 执行全量编译 (生成工业级优化 release 二进制)
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release

# 3. 安装到全局 PATH 路径
sudo install -m 755 target/release/agent-steward /usr/local/bin/agent-steward

# 4. 执行一键运行环境依赖预检 (Preflight)
agent-steward preflight
```

### 方案 B：使用 Makefile 自动化构建
```bash
make build       # 编译 release
make test        # 运行测试
sudo make install # 安装到 /usr/local/bin
make preflight   # 预检依赖
```

---

## 四、运行依赖一键预检 (Preflight Check)

大管家内置了环境诊断引擎，在上线前可通过以下命令全面验证环境状态：

```bash
agent-steward preflight
```

**预期输出示例**：
```
╔══════════════════════════════════════════════════════════════════════════════════════════╗
║                   Agent Host Steward — 运行环境与依赖预检报告 (Preflight)                ║
╚══════════════════════════════════════════════════════════════════════════════════════════╝
系统架构: linux-unix (x86_64)  |  检查时间: 2026-11-19T16:16:25.242977676+08:00
--------------------------------------------------------------------------------------------
状态     类型           检查项                      详情 / 修复指引
--------------------------------------------------------------------------------------------
✅ PASS   REQUIRED     Linux /proc 进程与网络接口      /proc/net/tcp 及进程空间可正常读取
✅ PASS   REQUIRED     inotify 实时文件系统监听         内核 inotify 事件机制就绪
✅ PASS   REQUIRED     数据存储目录写入权限               数据目录 "/root/.steward" 可读写
✅ PASS   REQUIRED     SQLite3 + FTS5 存储引擎      嵌入式 SQLite 及 FTS5 全文检索模块运行正常
✅ PASS   OPTIONAL     DuckDB 列存分析引擎            嵌入式 DuckDB 内存引擎就绪
✅ PASS   OPTIONAL     Systemd 守护环境             系统由 Systemd 纳管，支持自动化拓扑采集与 service 后台运行
✅ PASS   OPTIONAL     CodeGraph 宏微联邦引擎         已安装 codegraph CLI: /root/.nvm/versions/node/v24.20.0/bin/codegraph
✅ PASS   OPTIONAL     宿主机 SKILL 生态目录           已发现 2 个 SKILL 根目录: /root/new/skills, /root/.gemini/antigravity-cli/skills
✅ PASS   OPTIONAL     Ollama 向量服务与模型           服务正常且已拉取嵌入模型 `nomic-embed-text`
✅ PASS   OPTIONAL     Ollama 元数据标注模型           服务正常且已拉取小模型 `qwen3.5:0.8b` (L2 自动语义标注就绪)
✅ PASS   INFO         系统内核 inotify watch 限制    max_user_watches = 123055 (建议大项目 >= 65536)
✅ PASS   INFO         Agent Steward 软件包版本      v0.4.0 (9ba7bca, linux/x86_64, built at 2026-11-19T07:53:18Z)
--------------------------------------------------------------------------------------------
🚀 结论: 所有 REQUIRED 基础环境依赖全部满足，大管家可稳定上线运行。
```

> **自动化集成提示**: CI/CD 或 Agent 脚本可执行 `agent-steward preflight --json`，退出码为 `0` 代表满足，非 `0` 代表缺少关键基础依赖。

---

## 五、Systemd 常驻守护进程配置

在生产 Linux 宿主机上，推荐将大管家配置为 Systemd 托管的开机自启常驻服务。

### 1. 创建 Unit 配置文件
在 `/etc/systemd/system/agent-steward.service` 写入：

```ini
[Unit]
Description=Agent Host Steward Daemon (宿主环境大管家后台守护)
After=network.target

[Service]
Type=simple
ExecStart=/usr/local/bin/agent-steward daemon
Restart=always
RestartSec=5s
User=root
WorkingDirectory=/root
Environment="RUST_LOG=info"
Environment="PATH=/root/.cargo/bin:/root/.local/bin:/root/.nvm/versions/node/v24.20.0/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
```

### 2. 启用与管理命令
```bash
sudo systemctl daemon-reload
sudo systemctl enable agent-steward.service
sudo systemctl start agent-steward.service

# 查看服务状态
systemctl status agent-steward.service

# 查看实时日志
journalctl -u agent-steward.service -f
```

---

## 六、Docker 容器化部署

若在容器隔离环境中使用大管家，请务必保证 **`/proc`** 具有透传访问权限。

### 1. 标准 Docker 启动命令
```bash
docker run -d \
  --name agent-steward \
  --restart unless-stopped \
  --pid host \
  -v /proc:/proc:ro \
  -v /root/.steward:/root/.steward \
  -v /root/new:/root/new \
  agent-steward:latest daemon
```

> 💡 **参数关键点**:
> - `--pid host`: 允许大管家感知宿主机所有非系统活跃进程；
> - `-v /proc:/proc:ro`: 赋予 `/proc/net/tcp` 监听端口采集权限；
> - `-v /root/.steward:/root/.steward`: 持久化元数据库。

### 2. 使用 Docker Compose 一键启动 (含可选 Ollama)
直接运行 `docker compose up -d` 即可。详见仓库附带的 [docker-compose.yml](docker-compose.yml)。

---

## 七、可选增强依赖配置指南

### 1. 配置 Ollama 向量混合检索 (RRF) 与资产 L2 自动标注
1. 安装 Ollama：
   ```bash
   curl -fsSL https://ollama.com/install.sh | sh
   ```
2. 拉取嵌入模型与轻量元数据标注模型（`qwen3.5:0.8b` 为可选增强，缺失时自动降级为纯 L1 确定性语法提取）：
   ```bash
   ollama pull nomic-embed-text
   ollama pull qwen3.5:0.8b
   ```
3. 在 `~/.steward/config.toml` 中配置：
   ```toml
   [embedding]
   ollama_url = "http://localhost:11434"
   model = "nomic-embed-text"
   dimensions = 768
   fallback_fts_only = true

   [annotation]
   enabled = true
   ollama_url = "http://localhost:11434"
   model = "qwen3.5:0.8b"
   timeout_ms = 15000
   max_snippet_chars = 1200
   auto_enrich_batch_size = 10
   ```

### 2. 配置 PostgreSQL + pgvector 高并发后端
1. 在 PostgreSQL 中创建数据库并安装向量扩展：
   ```sql
   CREATE DATABASE steward;
   \c steward
   CREATE EXTENSION IF NOT EXISTS vector;
   ```
2. 执行跨引擎一键数据迁移：
   ```bash
   agent-steward storage migrate --to postgres --to-url "host=127.0.0.1 dbname=steward user=postgres password=secret"
   ```

---

## 八、常见故障与排障速查表 (FAQ)

| 现象 / 报错信息 | 根因分析 | 解决办法 |
|:---|:---|:---|
| `cmake: command not found` | 编译期缺少 CMake 工具链 | 执行 `apt-get install -y cmake` 或 `dnf install cmake` |
| `cannot open regular file ... Text file busy` | 二进制正在被 running 状态的 Systemd 服务占用 | 使用 `install -m 755` 替换旧二进制，或先 `systemctl stop agent-steward.service` |
| `/proc/net/tcp 不存在` | 运行在非 Linux 环境或极端受限容器内 | 在 Linux 物理机/虚拟机运行，或 Docker 启动加 `--pid host` |
| `max_user_watches 耗尽` | 监控目录下的文件树极大，超出了内核 inotify 上限 | 执行 `sudo sysctl -w fs.inotify.max_user_watches=524288` |
| `Ollama 连接失败警告` | 本地未安装 Ollama 或服务未启动 | 系统已自动安全降级为纯 FTS5 检索，不影响基本功能；若需向量请启动 Ollama |
| `api_port 不能小于 1024` | 配置文件填写的端口为系统特权端口 | 修改 `~/.steward/config.toml` 中的 `api_port` 为 1024~65535 之间（默认 9527） |
