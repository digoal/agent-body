# 01 — 快速上手与编译部署（Getting Started）

> **章节目标**: 从零准备依赖、在本地编译出生产二进制文件、完成首次初始化并在终端跑通冒烟测试。

---

## 1. 环境准备

Agent Body 是脑体分离架构下驻留宿主机的 **Agent 本体（宿主机具身底座）**，采用 Rust 语言编写，底层基于嵌入式 SQLite 与 Linux 内核机制，具备单二进制、零重型运行时依赖的特点。

### 1.1 系统需求
- **操作系统**: Linux（内核版本 >= 4.18，支持 inotify 与 `/proc` 文件系统）
- **CPU 架构**: x86_64 / aarch64
- **内存要求**: 基础运行仅需 20MB ~ 50MB 内存；构建期推荐 >= 2GB 可用内存。

### 1.2 依赖检查
在编译前，请确保环境中已具备 Rust 与 Cargo 工具链：

```bash
# 检查 Rust 编译器版本 (要求 >= 1.75，推荐 1.99.0)
rustc --version
# 输出示例: rustc 1.99.0-nightly (...)

# 若提示命令未找到，请加载环境变量
export PATH="/root/.cargo/bin:$PATH"
```

> [!NOTE]
> SQLite 采用 Bundled 静态内嵌特性编译（含 FTS5 全文搜索模块），无需宿主机提前安装系统级 `libsqlite3-dev`，不会产生动态链接库版本冲突。

---

## 2. 源码获取与编译

### 2.1 编译 Release 生产二进制
进入项目根目录，直接使用 Cargo 执行生产编译优化：

```bash
cd /root/new/work/agent-body
export PATH="/root/.cargo/bin:$PATH"

# 执行编译 (开启 LTO 与 Release 级优化)
cargo build --release
```

编译完成后，产物二进制文件位于：
`target/release/agent-body`（静态内嵌 SQLite FTS5、DuckDB 列存引擎与 Web UI 2.0，单二进制约 79MB）。

### 2.2 (可选) 安装到系统可执行路径
为了方便在任意目录直接使用 `agent-body` 命令，可以将其拷贝至系统 PATH：

```bash
cp target/release/agent-body /usr/local/bin/
chmod +x /usr/local/bin/agent-body
```

---

## 3. 首次启动与数据初始化

大管家采用 **零配置引导（Zero-Config Bootstrap）** 机制。无需手动执行 SQL 建表脚本，大管家在首次运行时会自动检测并创建所有必要目录和数据结构。

### 3.1 运行状态检查
在终端中执行：

```bash
./target/release/agent-body status
```

**真实输出示例**:
```text
╔═══════════════════════════════════════════════════════╗
║      Agent Body (Rust 版) — 系统状态          ║
╚═══════════════════════════════════════════════════════╝
版本:           v1.0.0 (<git-commit>, linux/x86_64)
数据目录:       "<user_home>/.agent-body"
数据库文件:     "<user_home>/.agent-body/body.db"
已纳管资产数:   26091
依赖关系边数:   6407 条
放置规范规则:   8 条
分级扫描策略:   12 条
当前动态进程:   96 个
```

### 3.2 自动初始化的文件系统结构
首次运行后，将在当前用户家目录下生成以下文件：

```
~/.agent-body/
├── config.toml    # 主配置文件 (从内置模板自动生成)
└── body.db     # SQLite 嵌入式数据库 (包含 WAL 日志与 FTS5 虚拟表)
```

---

## 4. 首次冒烟测试：获取宿主上下文简报

体验大管家最核心的命令之一：`brief`。该命令聚合了操作系统指标、正在运行的动态任务进程以及跨会话记忆：

```bash
./target/release/agent-body brief
```

**真实输出片段示例**:
```text
=== Steward Brief 上下文简报 ===
系统: Linux 13 Debian GNU/Linux | 内核: 6.12.107+deb13-amd64 | 内存使用率: 20.3%
活跃动态任务进程 (前 5):
  - [PID 951] ollama (CPU 0.0%, 内存 53.9MB) -> /usr/local/bin/ollama serve
  - [PID 420979] llama-server (CPU 0.0%, 内存 877.7MB) -> /usr/lib/ollama/llama-server ...
  - [PID 2112] gateway-control (CPU 0.0%, 内存 703.4MB) -> python3 ...
最近更新资产:
  *(暂无已纳管资产，可通过 agent-body asset scan 扫描录入)*
最近长期记忆:
  - [decision] 决定选用 Rust 1.99 与 SQLite 编写 Agent Body 核心进程 (ID: mem-82ecbb7b...)
```

至此，Agent Body 已在您的宿主环境中顺利就绪！

---

## 5. 下一步阅读
- 想要理解大管家的白名单深度递归与黑名单阻断原则？请阅读 [02-core-concepts.md](02-core-concepts.md)。
- 想要查看完整的命令行参数与实操大全？请阅读 [03-cli-reference.md](03-cli-reference.md)。
- 想要让 Claude Desktop / Cursor / Antigravity Agent 直接调用大管家？请阅读 [04-mcp-integration.md](04-mcp-integration.md)。
