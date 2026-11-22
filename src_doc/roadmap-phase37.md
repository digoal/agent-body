# Agent Host Steward — Phase 37 演进路线图：生产运行态深度对齐、拓扑零误报硬化与 Benchmark `v3.2` 等价校准（ISSUE #20）

> **编制**: Antigravity · **日期**: 2026-11-21  
> **基线版本**: `v0.4.0`（`4853910`）+ Phase 1~36 全部完成（Benchmark `v3.1` 帕累托双门禁、41/41 黑白屏同步、原生 `hooks.json` 对接）  
> **对齐约束**: [README.md 规约 #5, #10, #12, #16, #18](../README.md) · [ISSUE #20](../ISSUE.md#L100) · [EVOLUTION_GUIDE.md](../EVOLUTION_GUIDE.md) · [user_feedback.md](../user_feedback.md) (`CON-08~11`, `BENCH-07~08`, `REQ-11`)  
> **当前状态**: ✅ Phase 37 已完成（233 项测试全绿，现网健康分 `100/100`，Benchmark `v3.2` 全量 PASS）

---

## 一、立项背景与现网第一性原理逆向剖析

在对宿主机现网生产态（`/root/.steward/steward.db`、`agent-steward.service`、全局 `~/.gemini/config/hooks.json` 与 `bench report`）开展端到端核验后，暴露出 4 项真实运行痛点（`CON-08~11`）与 2 项 Benchmark 口径失真（`BENCH-07~08`）：

| 编号 | 级别 | 现网现象与根因定位 | 归属步骤 |
|:---:|:---:|---|:---:|
| **`CON-08`** | `P0` | `CLI agent-steward brief` (`src/main.rs`) 仍停留在 Phase 1 极简输出，未调用 `BriefAssembler`（丢失 17 条记忆、放置规约与工程红线）；且 `src/env/mod.rs` 原样抓取多行 `node -e` / `python3 -c` 脚本导致 `SessionStart` 简报刷屏截断。 | **37.1** |
| **`CON-11`** | `P1` | `agent-steward links audit \| head` 遇管道提前关闭抛出 `Broken pipe (os error 32)` Panic，违反规约 #12 零 Panic。 | **37.1** |
| **`CON-09`** | `P0` | `LinksEngine` 扫描 `.rs` 测试代码（`tests/`、`#[cfg(test)]`、`placeholder=`）与 `.md` 代码块示例路径，导致现网 `links audit` 产生 **107 处假断链（健康分 `0/100`）**。 | **37.2** |
| **`CON-10`** | `P1` | `agent-steward.service` 在 Systemd 最小 `PATH` 下无法发现 `/root/.nvm/versions/node/*/bin/codegraph`，持续产生 Preflight 降级告警。 | **37.3** |
| **`BENCH-07`** | `P0` | `DQ3` 仅校验静态模板特征而未核验物理断链，导致 `DQ3 = 97.7% ✅` 与 `links audit = 0/100` 口径背离。 | **37.4** |
| **`BENCH-08`** | `P1` | `COMP-C4` 竞品侧 `psutil` 仅输出 `len()` 单个数字（1 token）导致报告出现 `Token 节省: -112.9%` 不等价负值。 | **37.4** |

---

## 二、方案对比与架构决策

1. **CLI `brief` 对齐方案**：
   - **方案 A（采纳）**：`Commands::Brief` 默认复用 `BriefAssembler`（索引优先 `<10ms`，新增 `--query` / `--session-id` / `--budget-tokens` / `--legacy` 参数保持向后兼容），同时在 `BriefAssembler::build_fact_section` 从 `active_processes` 索引补充前 3 个活跃应用任务，并在 `src/env/mod.rs` 与 `src/fact/process_net.rs` 统一折叠与截断多行内联脚本命令行。
   - **方案 B（放弃）**：保留旧版只扫 `/proc` 的 CLI `brief`——会持续与 MCP `steward_brief` 分裂（违背规约 #16）且浪费库内记忆资产。
2. **依赖拓扑抗自污染方案**：
   - **方案 A（采纳）**：在 `LinksEngine` 抽取层与 `LinkAuditor` 审计层共享 `is_test_or_example_link()` 规则，跳过 `tests/`、`#[cfg(test)]` 块、注释行、UI `placeholder=` 及 `.md` 中物理不存在的示例代码路径，同时**显式豁免 `/tmp/` 隔离沙盒**确保 `W5`/`W10` 与单测夹具零误伤。
   - **方案 B（放弃）**：完全不扫描 `.rs` 与 `.md`——会漏检真实脚本引用与有效文档文件链接。

---

## 三、Phase 37 串行原子任务拆解表（`37.0 ~ 37.6`）

| 步骤 ID | 原子任务 | 涉及文件 | 具体改造方案 | 验收门禁 (DoD) |
|:---:|---|---|---|---|
| **37.0** | **登记总账与立项规划** | `user_feedback.md`, `src_doc/roadmap-phase37.md`, `README.md`, `EVOLUTION_GUIDE.md` | 登记 `CON-08~11`、`BENCH-07~08`、`REQ-11`，建立 Phase 37 路线图与 ISSUE #20。 | 反馈、路线图与导航链接齐备。 |
| **37.1** | **CLI `brief` 统一、`normalize_process_command` 与 `SIGPIPE` 零 Panic** | `src/env/mod.rs`, `src/fact/process_net.rs`, `src/brief/assembler.rs`, `src/main.rs` | 1. 新增 `normalize_process_command()` 折叠空白与截断超长内联脚本，过滤 `CODEGRAPH_NO_WATCHDOG`；<br/>2. `BriefAssembler` 补充活跃任务索引行；<br/>3. `Commands::Brief` 接入 `BriefAssembler`；<br/>4. `main()` 重置 Unix `SIGPIPE`。 | `agent-steward brief` 输出三位一体简报且 `<15ms`；`links audit \| head` 零 Panic。 |
| **37.2** | **`LinksEngine` & `LinkAuditor` 测试/注释/文档示例免疫** | `src/links/mod.rs`, `src/links/audit.rs` | 1. 新增 `is_test_or_example_link()`；<br/>2. `.rs` 遇到 `#[cfg(test)]` 截断扫描、忽略 `/tests/`（保留 `/tmp/` 沙盒）；<br/>3. `.md` 仅保留存在的文件路径或显式链接。 | 单测通过，`links audit --fix` 后现网健康分达 `100/100`。 |
| **37.3** | **Systemd `CodeGraph` 工具链探测与四层贯通** | `src/preflight/mod.rs`, `/etc/systemd/system/agent-steward.service`, `INSTALL.md` | 1. `resolve_binary_path()` 探测 NVM/Cargo/local 工具链；<br/>2. 更新 Systemd Unit 与 `INSTALL.md` `PATH` 模板。 | `systemctl restart` 后日志无 CodeGraph 降级告警。 |
| **37.4** | **Benchmark `v3.2`：`DQ3` 联合健康分与 `COMP-C4` 等价校准** | `src/benchmark/{mod,dq,competitors,workloads}.rs`, `design_doc/{10,12}-*.md`, `manual/10-*.md` | 1. `BENCHMARK_SPEC_VERSION` 升至 `"3.2"`；<br/>2. `DQ3` 联合检验物理存在性（排除测试/示例/模板假边）；<br/>3. `COMP-C4` 竞品侧真实采集 `rg` 匹配与 `psutil` 进程明细；<br/>4. `W10` 增补 `#[cfg(test)]` 与 `.md` 示例抗误报测试。 | `cargo test` 全绿；`COMP-C4` 呈正向省 Token 与质量跃升。 |
| **37.5** | **全量门禁、编译构建与热部署重启** | 全仓代码 + Systemd 服务 | 运行 `make evolution-check && make lint-panic && make check && make test && make build` 与 `systemctl daemon-reload && make restart`。 | 0 警告、0 panic 违规、全量测试 PASS。 |
| **37.6** | **现网自愈、Benchmark `v3.2` 存证、记忆沉淀与镜像同步** | `/root/.steward/steward.db`, `user_feedback.md`, `src_doc/*`, `README.md` | 运行 `links audit --fix`、`brief`、`bench run --suite full --save true`、`feedback sync`，更新状态为 `✅` 并执行 `make codegraph && make sync-mirror`。 | 现网健康分 `100/100`、`v3.2` PASS、镜像零差异。 |

---

## 四、断点续传检查点追踪表 (Phase 37)

| 阶段 | 步骤 ID | 任务摘要 | 对应反馈 / ISSUE | 状态 | 完成时间 |
|:---:|:---:|---|---|:---:|---|
| **前置规划** | **37.0** | 登记 `CON-08~11`、`BENCH-07~08`、`REQ-11` 与 Phase 37 路线图 | `ISSUE #20` | ✅ 已完成 | 2026-11-21 |
| **Phase 37** | **37.1** | 统一 CLI `brief` 至 `BriefAssembler`、进程命令规范化与 `SIGPIPE` 零 Panic | `CON-08`, `CON-11` | ✅ 已完成 | 2026-11-21 |
| | **37.2** | `LinksEngine` & `LinkAuditor` 测试/注释/文档示例免疫（保留 `/tmp/` 沙盒） | `CON-09` | ✅ 已完成 | 2026-11-21 |
| | **37.3** | `PreflightChecker::resolve_binary_path` 与 Systemd `PATH` 四层贯通 | `CON-10` | ✅ 已完成 | 2026-11-21 |
| | **37.4** | Benchmark `v3.2`：`DQ3` 真实健康分、`COMP-C4` 等价校准与 `W10` 扩充 | `BENCH-07`, `BENCH-08` | ✅ 已完成 | 2026-11-21 |
| | **37.5** | 全量门禁检查、全套单测/集成测试、Release 编译与守护服务热部署 | Phase 37 DoD | ✅ 已完成 | 2026-11-21 |
| | **37.6** | 现网拓扑自愈、`v3.2` 实跑存证、记忆同步、文档状态闭合与镜像同步 | `REQ-11`, Phase 37 DoD | ✅ 已完成 | 2026-11-21 |

---

## 五、总体验收红线 (Definition of Done)

1. **CLI/Hook 一体化与零刷屏红线**：`agent-steward brief` 默认在 `< 15ms` 内输出包含宿主机事实、8 大规约、长期记忆与生态路由的完整简报；进程命令行无多行脚本刷屏。
2. **零 Panic 管道红线**：`agent-steward links audit | head` 等管道截断命令退出正常，0 Panic。
3. **现网拓扑纯净红线**：`agent-steward links audit` 断链数归 `0`、健康分 `100/100`，`DQ3 >= 95%`，同时 `W5`/`W10` 全部通过。
4. **Benchmark `v3.2` 实证红线**：`COMP-C1~C4` 与 `W1~W11` 100% 帕累托 PASS（无负省 Token 失真），`DQ1~DQ4` 全 PASS。
5. **四层贯通与防漂移红线**：`make evolution-check && make lint-panic && make check && make test && make sync-mirror` 全部通过。
