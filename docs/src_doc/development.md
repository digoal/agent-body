# Agent Body (Rust 版) — 本地编译、开发与测试指南

> **版本**: v1.1.0 (Rust Edition)  
> **更新时间**: 2026-11-28  
> **架构基线**: 脑体分离 Agent 本体（27 顶层模块 · 43 MCP · 30 CLI · 88 SPI · 27 表 · 事务式老库无损 Schema 自愈）+ 微内核与周边扩展体系 (`3 kernel / 11 contrib / 3 slim distributions`) + 分层加权图谱 + MeshEnvelope v1.0 + AOPCC + 内生 18 Wait Events / AWR / Prometheus `/metrics` / W3C Trace + Benchmark v4.1 (`W22 / M6 / DQ12 / COMP-C4 / PR-16`) + **435 项全工作区测试全绿**

---

## 一、开发与构建环境要求

### 1. 必备工具链与运行库

1. **Rust 工具链**：
   - `rustc` >= 1.88.0（因 `Cargo.lock` 含 `edition = "2024"` 依赖；现网实测 `rustc 1.99.0`）；
   - `cargo` >= 1.88.0，确保 PATH 包含 Cargo 二进制目录：
     ```bash
     export PATH="$HOME/.cargo/bin:$PATH"
     ```
2. **C/C++ 编译工具链（仅旗舰版 DuckDB bundled 必需）**：
   - `gcc` >= 9、`g++`（提供 `libstdc++6`）、`cmake` >= 3.15、`make`、`pkg-config`；
   - **提示**：若仅构建三大精简发行版（`agent-body-kernel-minimal` / `agent-body-probe` / `agent-body-dba`），**无需 CMake 与 DuckDB，运行时零 C++ 依赖**。
3. **内核与系统能力**：
   - Linux 内核 >= 4.5（`inotify_init1` 与 `/proc` 可读）；macOS 原生适配 `FSEvents` / `libproc` / `LaunchAgent`；Windows 适配 `ReadDirectoryChangesW` / `netstat`。

### 2. 可选扩展服务（按需安装，缺失均自动优雅降级）

- **Ollama**：本地 `http://127.0.0.1:11434`，嵌入模型推荐 `nomic-embed-text` 或 `bge-m3`，轻量标注/Know-How 抽取推荐 `qwen3.5:0.8b`；
- **PostgreSQL**：版本 >= 14（`pgvector` 可选，默认应用层余弦运算，支持多机共享存储与 Advisory Lock）；
- **CodeGraph**：本地安装 `codegraph` CLI 并在目标仓库建立 `.codegraph/` 索引（微观函数符号调用穿透）；
- **Playwright（Python）**：用于运行无头浏览器验收门禁 `make verify-webui` 与 `make verify-decks`。

---

## 二、完整工程目录树规范

```text
agent-body/
├── Cargo.toml               # Rust Workspace 与旗舰依赖配置
├── Cargo.lock               # 依赖版本精确锁定
├── build.rs                 # 编译期 Git Hash / 构建时间 / 目标平台注入
├── Makefile                 # 构建、六重门禁、435 测试、Playwright 双门禁、热重启与镜像同步
├── Dockerfile               # 多阶段镜像（旗舰版 + 零 C++ --target probe 精简镜像）
├── docker-compose.yml       # 容器化编排参考配置
├── config.example.toml      # 完整注释的配置模板（含 [workspace]、[memory].pg_tx 与三态 [runtime]）
├── EVOLUTION_GUIDE.md       # Tier-0 零重扫演进活地图与 10 大场景变更触点矩阵
├── INSTALL.md               # 生产部署与依赖全景排障手册
├── docs/releases/           # 历史各版本 Release Notes (v0.1.0 ~ v1.1.0)
├── docs/src_doc/            # 研发指南、边界治理、进度总表、活跃路线图与 archive/
├── kernel/                  # 纯机制微内核 (body-sdk / body-protocol / body-kernel)
├── contrib/                 # 11 个独立周边标准五件套扩展 (含 obs_telemetry 与 mem_pg_tx)
├── distributions/           # 3 个零 C++/DuckDB 精简发行版 (kernel-minimal / aiops-probe / dba-sentinel)
├── tests/                   # 19 个集成、契约、PG 18 压测/睡眠整理/M3百万流、迁移回归、全量 MCP/Web E2E 与基准套件 (全工作区 435 项测试)
│   ├── schema_migration_regression.rs # 老库列改名/补列无损自愈、FTS5 重建、WAL 上限与 CJK 回退回归
│   ├── storage_spi_contract.rs        # SQLite & DuckDB 12 域 StorageProvider SPI 等价契约测试
│   ├── pg_tx_memory_integration.rs    # PostgreSQL 18.6 16线程MVCC压测、写穿无僵尸召回、SQLite降级与老库tx_epoch迁移测试
│   ├── sleep_compaction_integration.rs # M2 空闲睡眠整理、QueryPredictabilityGate 旁路与 OUT_OF_AGENT 审计测试
│   ├── fast_weight_m3_integration.rs  # M3 旁路快权重三门禁/定向遗忘/epoch回滚/Continuity v2/PG 18 100万事件流实测
│   ├── mcp_all_tools_e2e.rs           # 43/43 原生 MCP 工具全矩阵调用与必填参数校验 E2E
│   ├── web_full_e2e.rs                # Web 写鉴权、Benchmark 报错透传、Mesh 路由与记忆类型归一 E2E
│   ├── test_web_dashboard.rs          # Web UI 2.0 端点与 SSE 推流测试
│   ├── cli_e2e.rs                     # CLI 全链路端到端集成测试
│   ├── coordination_integration.rs    # 多 Agent 协作锁并发与层级互斥测试
│   ├── deadlock_wait_integration.rs   # WFG 死锁检测与等待队列唤醒测试
│   ├── duckdb_integration.rs          # DuckDB 列存分析与单事务增量管道测试
│   ├── ecosystem_integration.rs       # MCP/SKILL 生态目录与防爆路由测试
│   ├── mesh_integration.rs            # 多机 MeshEnvelope HMAC 签名防重放与收件箱隔离测试
│   ├── audit_integration.rs           # 双域审计脱敏、防洪与统计测试
│   ├── body_awr_integration.rs        # 本体身份证、AWR 快照、黄金基线与 L0~L3 包络测试
│   ├── hooks_integration.rs           # Hook 自动同步与 <2ms 碰壁处方自愈测试
│   ├── multimodal_integration.rs      # 纯 Rust 多模态 L1A 解析与 kv_filter 检索测试
│   └── benchmarks.rs                  # 亚毫秒微基准与 v4.1 实证基准套件
└── src/                     # Rust 旗舰源码 (27 个顶层模块，全量 <1000L)
    ├── main.rs / cli_args.rs / cli_subcommands.rs / cli_handlers/
    ├── lib.rs / config/ / preflight/
    ├── storage/             # 88 SPI + SQLite/PG/DuckDB 分域实现 + schema_reconcile.rs 无损自愈 + 热迁移
    ├── audit/ / body/ / brief/ / changes/ / coordination/ / daemon/ / demo/
    ├── ecosystem/ / env/ / fact/ / feedback/ / hooks/ / links/ / memory/
    ├── mesh/ / placement/ / platform/ / scanner/ / session/ / tui/ / watcher/ / web/ / api/
```

---

## 三、常用构建、测试与质量门禁命令

### 1. 六重确定性质量门禁（零警告 / 零漂移 / 零 Panic）

```bash
export PATH="$HOME/.cargo/bin:$PATH"
make quality-gate
# 串联执行: evolution-check + arch-check + lint-panic + lint-hygiene + lint-strict
```

### 2. 全工作区 435 项测试（根包 `387` + `kernel` `27` + `contrib` `17` + `distributions` `4`）

```bash
# 推荐设置 RUST_TEST_THREADS=1 避免微基准 SLA 断言受多核 CPU 争抢抖动
RUST_TEST_THREADS=1 make test
# 等价于: RUST_TEST_THREADS=1 cargo test --workspace

# 定向运行高价值回归套件示例：
cargo test --test schema_migration_regression
cargo test --test mcp_all_tools_e2e
cargo test --test web_full_e2e
cargo test --test storage_spi_contract
cargo test --test pg_tx_memory_integration
```

### 3. Playwright 双端无头验真（WebUI 9 工作台 + 9 份演说 HTML）

```bash
# 验证真实老库无损迁移 + WebUI 9 大工作台交互零报错（scripts/verify_webui_e2e.py）
make verify-webui

# 验证全套 9 份单文件演说 HTML 零溢出/零重叠/快捷键全通（scripts/verify_presentations.py）
make verify-decks
```

### 4. 编译、部署与现网对账

```bash
# 编译旗舰 Release 并安装重启守护服务（自动平滑迁移现网库）
make build && sudo make restart

# 按需编译零 C++ 精简发行版并自检
cargo build --release -p agent-body-kernel-minimal -p agent-body-probe -p agent-body-dba
./target/release/agent-body-probe self-check && ./target/release/agent-body-dba self-check

# 核验发版二进制哈希与现网数据新鲜度
agent-body --version && agent-body preflight && agent-body feedback reconcile
```

---

## 四、本地多后端切换与 Schema 演进规范

1. **默认 SQLite（带自动无损 Schema 自愈）**：首次运行或版本升级启动时自动触发 `src/storage/sqlite_provider/schema_reconcile.rs`，在单事务内完成老库历史列改名/补列与 FTS5 脱同步自动重建，确保升级零停机、零数据丢失。
2. **PostgreSQL 后端**：
   ```sql
   CREATE DATABASE body_test;
   -- pgvector 可选，未安装时自动使用应用层余弦相似度
   ```
   ```bash
   ./target/release/agent-body storage migrate --to postgres --to-url "host=127.0.0.1 dbname=body_test user=postgres"
   ```
3. **DuckDB 列存分析副本**：
   ```bash
   ./target/release/agent-body storage sync --to duckdb
   ```

---

## 五、开发与提交流程规范

1. **零重扫开工**：新会话先读 [EVOLUTION_GUIDE.md](../../EVOLUTION_GUIDE.md) + 最新活跃 `roadmap-phase*.md`，跑 `make evolution-check` 并用 `codegraph explore` 定位符号；
2. **Schema 变更铁律**：凡修改 `schema.sql` / `pg_schema.sql` / `duckdb_schema.sql`，必须同步更新 `schema_reconcile.rs` 并在 `tests/schema_migration_regression.rs` 补充旧库升级用例；
3. **提交前全绿收口**：依次通过 `make quality-gate && RUST_TEST_THREADS=1 make test`（改动 Web/演说时追加 `make verify-webui` / `make verify-decks`），刷新 [progress.md](progress.md) 并按需执行 `make sync-mirror`。
