# Agent Host Steward (Rust 版) — 本地编译与测试环境开发指南

> 版本: v0.2.0 (Rust Edition)  
> 更新时间: 2026-11-15

---

## 一、环境要求

1. **Rust 工具链**:
   - `rustc` >= 1.75.0 (本机当前为 `1.99.0`)
   - `cargo` >= 1.75.0
   - **注意**: 若终端提示 `cargo: command not found`，请确保 PATH 包含 cargo bin：
     ```bash
     export PATH="/root/.cargo/bin:$PATH"
     ```

2. **C 工具链**:
   - `gcc` (Debian 14+) 或 `clang`
   - `make`
   - `pkg-config` (可选)

3. **SQLite 运行时**:
   - 依赖项采用 `rusqlite` 的 `bundled` 特性，编译时自动嵌入 SQLite 引擎，并显式开启 `bundled-full` 与 `modern_sqlite` 特性，保证 FTS5（全文检索）原生支持，无需依赖外部动态链接库。

---

## 二、工程目录规范

```
agent-steward/
├── Cargo.toml               # Rust 项目依赖与元数据配置
├── Cargo.lock               # 依赖版本锁定
├── src_doc/                 # 开发指南与使用手册目录
│   ├── progress.md          # 串行任务断点续传进度表
│   ├── development.md       # 本地开发与编译环境说明
│   └── product-manual.md    # 用户与 Agent 操作使用手册
├── design_doc/              # 产品设计工程交付件 (01~07)
├── idea_doc/                # 初始思考与设计推导
└── src/                     # Rust 源码目录
    ├── main.rs              # CLI 入口与子命令分发
    ├── lib.rs               # 核心库导出
    ├── config/              # 配置文件与环境变量管理
    ├── storage/             # SQLite 数据库初始化、FTS5 与 CRUD 抽象
    ├── placement/           # 新对象放置建议引擎 (Placement Advisor)
    ├── scanner/             # 资产分级颗粒度扫描器 (Deep/Shallow/Restricted)
    ├── env/                 # 宿主系统感知与动态进程采集器
    ├── memory/              # 记忆管理与检索服务
    └── cli/                 # 终端命令处理逻辑
```

---

## 三、常用构建与测试命令

### 1. 语法检查与快速编译
```bash
export PATH="/root/.cargo/bin:$PATH"
cargo check
```

### 2. 运行单元测试
```bash
# 运行所有测试
cargo test

# 运行特定模块测试并打印输出
cargo test test_storage -- --nocapture
cargo test test_placement -- --nocapture
cargo test test_scanner -- --nocapture
```

### 3. 构建发布版二进制
```bash
cargo build --release
# 生成的单二进制文件位于: target/release/steward
```

### 4. 本地启动运行
```bash
./target/release/steward status
./target/release/steward place suggest --type script --name sec_scraper --intent "美股财报抓取"
./target/release/steward env processes
```
