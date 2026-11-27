# Agent Body Roadmap — Phase 65~66: 效果真实性与可信度量攻坚 + 跨平台打磨与 GitHub CI/CD

> **文档状态**：✅ **已实施** | **基线版本**：`v1.1.0` (`6ef83aa`)  
> **定位**：依照“方案 A”，先解决真实运行态暴露的 5 类核心品质短板（Shadow A/B 检索精度与延迟、`vitals`/`DQ6` 图谱边截断失真、`DQ2` 口径矛盾、macOS 预检/Makefile Linux 残留、缺少 GitHub Actions CI/CD）。

---

## 1. Phase 65 — 效果真实性与可信度量攻坚

| 任务项 | 现网痛点证据 | 修复措施 | 对应文件 |
|---|---|---|---|
| **DQ2 口径一致 + DQ6 防截断** | `DQ2` 展示目标 `99.0%` 与通过门槛 `95.0%` 打架；`DQ6` 因 `list_all_fact_edges(5000)` 在 157k 边库截断，报告工程节点连通 `0/12` | `DQ2` 统一常量 `DQ2_TARGET_PCT = 95.0`；`DQ6` 对未命中采样边的 `project:*` 节点走 `traverse_fact_graph(&p.id, 1)` 索引定向核查并补充单测 | [`src/benchmark/dq.rs`](../../src/benchmark/dq.rs) |
| **五维稳态 `vitals` 防截断** | 感官清晰度因 `list_all_fact_edges(4000)` 截断误报 `60.0 [critical]` | 对未出现在采样边中的 `project:*` 节点补充 `traverse_fact_graph(pid, 1)` 索引核查并新增单测 | [`src/body/vitals.rs`](../../src/body/vitals.rs) |
| **`expand_graph_for_query` 降延迟** | 每次代码词检索对子词执行 55k 节点 `LIKE` 全表扫描 + Hub 2 跳爆炸（`257ms` -> 触发 `H3_LATENCY_DRAG`） | `get_fact_node(&tid)` 主键命中时跳过 `find_fact_nodes_by_name`；复合代码标识符优先精确节点锚定；1-hop `edge_cnt <= 40` 才进 2-hop | [`src/fact/knowledge_graph.rs`](../../src/fact/knowledge_graph.rs) |
| **`AssetSearchEngine` & `ShadowAbEngine` 精准提速** | 复合代码标识符 `ShadowAbEngine` 被含单子词 `"engine"` 的旧文档挤出 Top-K；循环内重复构造 `AliasCanonicalizer` | 抽出 `PreparedFuzzyQuery` 预计算；对复合代码词单子词命中降权并引入强权重 `exact_id` RRF 通道；`ShadowAbEngine` 对代码词聚焦 1-hop 图扩散 | [`src/scanner/asset_search.rs`](../../src/scanner/asset_search.rs), [`src/body/runtime_ab.rs`](../../src/body/runtime_ab.rs) |

---

## 2. Phase 66 — 跨平台打磨与 GitHub Actions CI/CD

| 任务项 | 现状与问题 | 改进举措 | 对应文件 |
|---|---|---|---|
| **macOS 预检零误报** | `preflight` 在 macOS 误报 `⚠️ WARN Systemd`、`inotify` 与 `max_user_watches = 未知` | 按 `std::env::consts::OS` 分支输出 `FSEvents`、`LaunchAgent` 与平台句柄信息，并将版本名统一为 `Agent Body` | [`src/preflight/mod.rs`](../../src/preflight/mod.rs) |
| **Daemon 跨平台日志** | `daemon` 在 macOS 仍打印 `inotify` 与 `轮询 /proc` | 按平台打印 `FSEvents`/`ReadDirectoryChangesW`/`inotify` 与 `macOS libproc/lsof` | [`src/daemon/mod.rs`](../../src/daemon/mod.rs) |
| **Makefile 跨平台移植** | `restart` 硬编码 `systemctl`，`sync-mirror` 硬编码 `/root/new/` | 自动识别 `Darwin`/`Linux`（`launchctl kickstart` vs `systemctl`、`~/.local/bin` vs `/usr/local/bin`、SDK C++ 头与 `$(HOME)` 镜像路径） | [`Makefile`](../../Makefile) |
| **GitHub Actions CI & Release** | 仓库缺少 `.github/workflows/` | 新增双平台（Ubuntu + macOS）`ci.yml` 与 tag 触发的 `release.yml` 自动打包 `.tar.gz` + `.sha256` | [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml), [`.github/workflows/release.yml`](../../.github/workflows/release.yml) |
