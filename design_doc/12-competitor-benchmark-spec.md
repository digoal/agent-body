# 12 — 热门开源竞品横向对标与 Benchmark v3.0 规格说明书

> **Agent Host Steward 产品设计 · 交付件 12**  
> **版本**: v3.0 设计基线 · **编制**: Antigravity · **日期**: 2026-11-21  
> **对齐约束**: [README.md 规约 #5, #18, #20](../README.md) · [ISSUE #18](../README.md#L386) · [roadmap-phase34-35.md](../src_doc/roadmap-phase34-35.md)

---

## 一、设计背景与实证铁律

Benchmark `v2.1` 已建立轨道 A（`W1~W11` 裸奔自对比）与轨道 B（`PR-01~10` 生产存证账本），但缺乏**轨道 C（横向热门开源产品对标）**。本规格书确立双层实证机制：

1. **`LIVE_BENCHMARKED`（同机同数据集物理实跑）**：在 `src/benchmark/competitors.rs` 中，于同一隔离沙盒、同一硬件、同一数据集上真实运行对照组与 Steward，由 `std::time::Instant`、`estimate_tokens()` 与物理字节/计数得出实测结果，严禁主观估算或抄袭外部宣传数字。
2. **`ARCH_VERIFIED`（源码核验 12 维架构能力矩阵）**：基于本机 `/root/new/src/` 已克隆的真实源码仓库核验各产品技术栈、存储结构、能力边界与生态互补关系。

---

## 二、四大赛道 10 大对标产品与本地源码依据

| 赛道 | 对标产品 | 本地源码 / 运行环境依据 | 核心机制与边界 | 与 Steward 关系 |
|---|---|---|---|---|
| **C1 · Agent 长期记忆** | Mem0, Letta, PowerContext, Shodh-Memory, Hindsight | `/root/new/src/{mem0,letta,powercontext,shodh-memory,hindsight}` | 专注对话/认知记忆；Mem0/Letta 依赖 Python+LLM 抽取；Shodh-Memory 为 Rust 嵌入式记忆；均缺宿主机 `/proc`/端口/脱敏全链路绑定 | 竞争（内置零网络 `<1ms` + 脱敏）+ **SPI 互补外挂** |
| **C2 · 上下文数据库与路由** | OpenViking, 静态全量 `SKILL.md` 注入 | `/root/new/src/OpenViking` (`viking://` + `.abstract.md` L0 / `.overview.md` L1 / 正文 L2) | OpenViking 需导入虚拟目录并编排 L0/L1 摘要；静态注入导致百级 SKILL 打爆 Context | 原位零搬迁 + Rust 微秒 L1/Ollama L2 + `ContextBudgetRouter` |
| **C3 · Agent 文件与协同** | AgentFS, OpenEnv | `/root/new/src/agentfs/SPEC.md` (`fs_inode`/`fs_dentry`/`fs_data`/`tool_calls` SQLite VFS), `/root/new/src/openenv` | AgentFS 全量写入 SQLite VFS 并审计 `tool_calls`，无黑白名单阻断、无放置规约与 WFG 死锁锁 | 原生 FS 零开销 + 黑名单零下钻 + 放置拦截 + WFG 死锁破局 |
| **C4 · 宿主探针与代码图** | `rg` + `psutil` + `ss` + `git`, CodeGraph | `/usr/bin/rg`, Python `psutil`, `/usr/bin/ss`, `codegraph` | 零散 CLI 每次派生多子进程、无跨会话状态、无跨介质 3 跳依赖图 | `BriefAssembler` `<10ms` 融合 + `CodeGraphBridge` 宏微联邦 |

---

## 三、轨道 C 实测负载规格 (`COMP-C1 ~ COMP-C4`)

| 编号 | 赛道 | Competitor 对照组实跑逻辑 | Steward 组实跑逻辑 | 核心对比维度 |
|---|---|---|---|---|
| **`COMP-C1`** | 长期记忆、脱敏与检索 | SQLite 裸文本记忆（无 `SecretRedactor` 脱敏、无 FTS5 倒排，全量扫描检索 60 条含 5 条凭据记忆） | `SecretRedactor` + `MemoryManager` + FTS5 索引检索 | 延迟 (`μs`)、凭据泄露率 (`0%` vs `100%`)、Token 效率 |
| **`COMP-C2`** | 分层上下文与技能路由 | 按 OpenViking L0/L1/L2 文件树构建 30 项目 `.abstract.md`/`.overview.md` 并全量扫描 + 静态 SKILL 注入 | `HeuristicMetadataExtractor` + `ContextBudgetRouter` (`budget=600`) | 耗时 (`μs`)、注入 Token 压缩率 |
| **`COMP-C3`** | 文件系统与工作区治理 | 按 `AgentFS SPEC.md` 四表 Schema 建库并全量导入 290 混合树文件（含 `node_modules/.git`） | `Scanner` + `PolicyMatcher` + `PlacementEngineV2` + `CoordinationEngine` | 入库条目数、SQLite 体积、黑名单逃逸数 (`0`) |
| **`COMP-C4`** | 宿主感知与依赖拓扑 | 真实拉起 `rg --files` + `ss -tln` + `python3 psutil` 子进程组合探测 | `BriefAssembler::assemble` (Index-First) + `LinksEngine` 3 跳查询 | 耗时 (`μs`)、子进程派生数 (`0` vs `>=3`)、输出 Token |

---

## 四、Benchmark `v3.0 → v3.1` 兼容性、帕累托质量门禁与黑白屏 1:1 同步

- `BenchmarkReportPayload` 增加 `competitors: Vec<CompetitorComparisonResult>` 与 `competitor_matrix: Vec<CompetitorCapabilityRow>`，带 `#[serde(default)]` 向后兼容读取 `v1.0`/`v2.0`/`v2.1`/`v3.0` 历史批次。
- `v3.1` 增补竞品侧与 Steward 侧成对上下文质量评分（`competitor_quality_score` vs `steward_quality_score`、`quality_delta`、`info_density_gain` 及 `required_facts_total` 核验），确保横向对标同时证明「省 Token + 上下文质量不降反升」。
- `v3.2` 校准 `COMP-C4` 等价物理采集基线（`25` 文件工程树上的 `rg -n` 上下文搜索 + `ss -tln` + `psutil` 进程明细输出），消除 `psutil` 仅输出单整数导致的负省 Token 失真（`BENCH-08`）。
- **MCP**：`steward_benchmark(action="competitors", suite="competitors" | "COMP-C1".."COMP-C4")`
- **CLI**：`agent-steward bench competitors [--track all|C1|C2|C3|C4] [--format table|json|markdown]`
- **Web REST**：`GET /api/v1/benchmark/competitors` & `POST /api/v1/benchmark/run` (`suite="competitors"`)
