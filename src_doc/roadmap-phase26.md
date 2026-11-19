# Agent Host Steward — Phase 26 演进路线图：数据治理硬化、真实依赖图谱闭环与 v0.4.0 四层全量对齐

> **编制**: Antigravity · **日期**: 2026-11-19  
> **基线 Commit**: `e11d052` (`v0.3.2`, Phase 1~25 全部交付，194 tests passed，39 个原生 MCP 工具黑白屏 100% 同步)  
> **对齐基准**: [README.md 核心设计原则与约束 §1-§17](../README.md#L207-L280)  
> **断点续传检查点表**: [progress-phase26.md](progress-phase26.md)

---

## 一、目标与核心设计规格

### 1. 分级扫描硬化（黑名单零逃逸 + `restricted` 真正落地 + 噪音门禁强化）
- **`PolicyMatcher` 双向阻断**：当 `shallow` 策略形如 `**/<dir>/**` 时自动同时注册 `**/<dir>`，并将 `**/__pycache__`、`**/__pycache__/**`、`**/.codegraph`、`**/.codegraph/**`、`**/.venv`、`**/.venv/**` 纳入默认 `shallow` 集合；`Scanner`、`ScanStateMachine`、`ChangeHandler` 在文件分支遇到 `tier == "shallow"` 直接拦截跳过，杜绝子文件逃逸入库。
- **`restricted` 受限深度与入口过滤**：在 `Scanner` 与 `ScanStateMachine` 中实装 `restricted` 规则——仅索引目录入口文档（`README*`、`INSTALL*`、`SKILL.md`）、工程清单（`Cargo.toml`、`pyproject.toml`、`package.json`、`Makefile`、`Dockerfile`）与顶层入口脚本（相对深度 `<= 2`）；并在工作区内自动识别第三方克隆/对比源码树（含多套 `test/`/`ext/`/`src/` 的非主工程嵌套树）或由策略降级为 `restricted`，防止数千个无关测试文件打爆资产表。
- **噪音过滤硬化**：在 `NoiseFilter` 中对 `.pyc`、`.pyo`、`.swp`、`.swx`、`.swo`、`.tmp`、`.log`、`.lock`、`CACHEDIR.TAG`、`.rustc_info.json` 实施确定性噪声否决；在 `FileWatcher` 与 `ChangeHandler::process_events` 中接入 `NoiseFilter` 与 `shallow` 阻断，防止 `.log` / `.swx` 刷爆 `changes` 与 `assets`。

### 2. 宏观依赖图谱（`LinksEngine`）全链路接线与幂等更新
- **存储层按 `source_id` 幂等清理**：在 `StorageProvider` SPI（SQLite / PG / DuckDB）与 `Storage` 门面中新增 `delete_auto_links_for_source(&self, source_id: &str) -> Result<usize>`，确保单文件重新扫描时先清理旧 `auto_discovered = 1` 边再重新抽取，避免重复或陈旧死边堆积。
- **全链路接线**：在 `LinksEngine` 新增 `refresh_links_for_asset(&self, asset_path: &str) -> Result<usize>`，并在 `Scanner::scan_path`、`ScanStateMachine`、`ChangeHandler::process_events` 中对脚本/配置/文档自动触发依赖图谱抽取，使 `links` 表、`links audit`、`links impact` 反映真实宿主机依赖拓扑。

### 3. 现网数据库净化自愈、`status` 精确统计与 `build.rs` 动态指纹修复
- **`build.rs` 与版本升级**：监听 `.git/HEAD` 与 `.git/logs/HEAD` 变更，将 `Cargo.toml` 版本升至 `0.4.0`。
- **`status` 真实计数**：修复 `agent-steward status` 资产总数（不再受 `list_assets(1000)` 截断显示 `1000`），并展示 `links` 关系统计。
- **现网库自愈清洗**：在 `HostGarbageCollector`（或 `asset scan --prune`）支持清理物理不存在的幽灵资产、黑名单逃逸子文件与旧噪音变更，并在现网 `~/.steward/steward.db` 完成净化重扫、L1 元数据补齐与 `links` 依赖图谱回填。

### 4. 四层贯通文档、Release Notes、内置 SKILL 与公众号长文全量刷新
- 新建 `releases/v0.4.0.md`，更新 `releases/README.md`；
- 刷新 `manual/README.md`（01~09 全卷）与各章节、`arch/` 3 篇、`src_doc/{development,product-manual,hooks-and-integration}.md`、`src_doc/progress.md`；
- 刷新 3 套内置 SKILL 并同步至 `/root/new/skills/` 与 `/root/.gemini/antigravity-cli/skills/`；
- 升级 `markdown/why-agent-needs-host-steward.md` 至 v0.4.0 全景架构；
- 全量门禁测试、`systemctl restart agent-steward.service` 热部署与 `/root/new/shared_git/agent-steward` 同步。

---

## 二、Phase 26 原子任务清单

| 步骤 ID | 原子任务 | 涉及文件 | 验收标准 |
|:---:|---|---|---|
| **26.1** | 登记 Phase 26 路线图与断点检查点表 | `src_doc/roadmap-phase26.md`, `src_doc/progress-phase26.md` | 路线图与检查点表落盘 |
| **26.2** | 分级扫描硬化（黑名单零逃逸 + `restricted` 落地 + 噪音门禁） | `src/scanner/{matcher,mod,state_machine,importance}.rs`, `src/watcher/{mod,handler}.rs` | 黑名单子文件 0 入库、`restricted` 仅留入口/README、Watcher 过滤 `.log`/`.swx`/`.pyc`、新增单测全绿 |
| **26.3** | `LinksEngine` 全链路接线与幂等更新 | `src/storage/{spi,mod,sqlite_provider,pg_provider,duckdb_provider}.rs`, `src/links/mod.rs`, `src/scanner/mod.rs`, `src/watcher/handler.rs` | `delete_auto_links_for_source` 三引擎实现、扫描/变更自动抽取 `links`、单测全绿 |
| **26.4** | `build.rs` 修复、`v0.4.0` 升级、`status` 精确统计与现网库净化 | `build.rs`, `Cargo.toml`, `src/main.rs`, `src/env/gc.rs` | `agent-steward --version` 为 `0.4.0`、`status` 显示真实资产与 `links` 数、现网库 0 逃逸/0 幽灵/`links > 100` |
| **26.5** | 四层贯通文档、Release Notes、SKILL 三镜像与公众号长文刷新 | `releases/`, `manual/`, `arch/`, `src_doc/`, `skills/`, `markdown/` | 全文版本/工具数(39)/测试数 100% 一致，SKILL 三处 sha256 一致，0 死链 |
| **26.6** | 全量门禁回归、现网热部署与开源镜像同步 | 全项目 + `/root/new/shared_git/agent-steward` | `make lint-panic` 0 违规、`cargo check --tests` 0 warning、`cargo test` 全绿、Daemon 在线、镜像一致 |
