# Agent Body — Phase 40 ~ 41 演进路线图：多维证据有向加权图谱（ISSUE #23）与联邦网格信封协同协议（ISSUE #24）

> **编制**: Antigravity · **日期**: 2026-11-22  
> **基线版本**: `v0.4.0`（`b9927a6`）+ Phase 1~39 全部完成（Benchmark `v3.3`、`DQ1~DQ5 100%`、237 tests PASS、41 MCP 工具）  
> **对齐约束**: [README.md 规约 #2, #5, #13, #15, #16, #19, #23, #24](../../README.md) · [14-graph-intelligence-and-mesh-protocol-spec.md](../design_doc/14-graph-intelligence-and-mesh-protocol-spec.md) · [EVOLUTION_GUIDE.md](../../EVOLUTION_GUIDE.md) · [user_feedback.md](../../user_feedback.md)  
> **当前状态**: ✅ Phase 40 ~ 41 已全部完成并经真机实测验收（全库 `246` 项测试 PASS、现网 `fact_edges = 776`、`project` 连通率 **`215/215 (100.0%)`**、`W1~W14` 全绿、`DQ6 100.0%`）

---

## 一、现网基线实证诊断与演进目标

| 编号 | 现网实测基线 (`2026-11-22`) | Phase 40~41 演进目标 | 关联 ISSUE |
|---|---|---|---|
| **`CON-16` 图谱关系稀疏与孤岛断层** | `fact_nodes = 355`（`354` 含向量），但 `fact_edges` 仅 `49` 条；`project` 节点 `214/215` 孤立（**99.5%**），`tool` `16/16` 孤立（**100%**）；`links` `5,116` 条边未上卷；无方向权重分级 | 落地 `L1_EXPLICIT` / `L2_STRUCTURAL` / `L3_SEMANTIC` 三层有向加权本体、`links→fact_edges` 上卷、服务同体桥接、Vector+Tag 隐式语义发现（Top-$K=3$）与加权衰减爆炸半径 | **ISSUE #23** / Phase 40 |
| **`CON-17` Mesh 无信封协议、无防重放与协同缺位** | `src/mesh/mod.rs` 仅发裸 HTTP + 全局共享 Bearer Token；`src/web/server.rs` Body 限 `64KB`；缺跨机对话与文件沙盒投递 | 落地 `MeshEnvelope v1.0` HMAC-SHA256 签名与 $\pm 300\text{s}$ Nonce 防重放、Peer Scopes 授权、跨机结构化对话（脱敏+抗投毒）、`<=10MB` `~/.agent-body/mesh_inbox/` 沙盒投递 | **ISSUE #24** / Phase 41 |
| **`BENCH-10` 缺少图谱发现与 Mesh 安全协同门禁** | Benchmark `v3.3` (`W1~W12`, `DQ1~DQ5`) 未覆盖多维加权语义图谱连通性及 Mesh 协议防篡改/防重放/沙盒协同 | 升级 Benchmark 至 **`v3.4`**，新增 `W13`、`W14` 与 `DQ6` 现网图谱健康度门禁 | **ISSUE #23, #24** / Phase 41.5 |

---

## 二、Phase 40 ~ 41 串行原子任务拆解与断点续传检查点表

| 阶段 | 步骤 ID | 原子任务摘要 | 精确代码/文档触点 | 验收门禁 (DoD) | 状态 | 完成时间 |
|:---:|:---:|---|---|---|:---:|:---:|
| **规划立项** | **40.1** | 现网图谱/Mesh 实证诊断、规约体系优化与新增规约 #23/#24、设计规格书 `14-*.md`、反馈总账与 Phase 40~41 路线图立项 | `README.md`, `docs/design_doc/14-*.md`, `docs/src_doc/roadmap-phase40-41.md`, `docs/src_doc/progress.md`, `docs/src_doc/product-boundaries-and-governance.md`, `EVOLUTION_GUIDE.md`, `user_feedback.md`, `src/placement/conventions.rs`, `skills/agent-body-developer/SKILL.md` | 规约 #2/#5/#13/#15/#16/#19 改进落地，规约 #23/#24 新增并同步代码与 SKILL；`make evolution-check` & `cargo test` 全绿 | ✅ 已完成 | 2026-11-22 |
| **Phase 40<br/>图谱增强** | **40.2** | **存储层与数据模型扩展（三后端 SPI 同步）**：`fact_edges` 增补 `evidence_tier` (`L1_EXPLICIT`/`L2_STRUCTURAL`/`L3_SEMANTIC`)、`weight`、`direction_kind`；新增幂等迁移与三后端 CRUD；支持按 `evidence_tier`/`min_weight` 过滤子图遍历 | `src/storage/{schema,pg_schema,duckdb_schema}.sql`, `src/storage/models.rs`, `src/storage/migration.rs`, `src/storage/spi.rs`, `src/storage/{sqlite,pg,duckdb}_provider.rs`, `src/storage/mod.rs` | 兼容旧库无缝热迁移；三后端 `fact_edges` 插入/查询/遍历全通过单测 | ✅ 已完成 | 2026-11-22 |
| | **40.3** | **L1/L2 结构上卷与服务同体桥接器**：① `links` 5,116 条文件引用按项目前缀上卷聚合为 `project→project` / `project→shared_proj` `L2` 加权边（对数饱和公式）；② 补全 systemd `exec_start`/`referenced_files` 至 `project` 及 `unit ↔ runtime` `runs_as` 边；③ 抽取 `project/service → tool` `uses_tool` 边；④ 剪除失活 `service`/`cron` 幽灵节点（清理 `test_web_dashbo` 等） | `src/fact/property_graph.rs`, `src/fact/semantic_discovery.rs` (新) | 现网 `project` 孤立率显著下降，`service` 与 `project`/`proc` 联通，幽灵服务节点清零 | ✅ 已完成 | 2026-11-22 |
| | **40.4** | **L3 隐式语义边挖掘与加权爆炸半径引擎**：① 利用 `fact_nodes` 向量余弦 ($\ge 0.78$) + 标签 Jaccard 混合打分，Top-$K=3$ 稀疏剪枝生成跨项目/服务/工具 `L3_SEMANTIC` 边；② 升级 `ImpactAnalyzer`（融合 `links` + `fact_edges` 统一遍历、按 `weight * 0.8^(hop-1)` 计算有效冲击力与分层归因）；③ 升级 `Web Topology` 边权重/证据层级样式与 `Mermaid` 实线/虚线区分 | `src/fact/semantic_discovery.rs`, `src/links/impact.rs`, `src/links/mod.rs`, `src/web/topology.rs`, `src/web/assets/mod.rs` | `agentfs ↔ agentfs_articles` 等显隐关联稳定发现；物理硬边与隐式语义边分层可过滤、可解释 | ✅ 已完成 | 2026-11-22 |
| | **40.5** | **图谱能力黑白屏 1:1 同步与单测回归**：扩展 `body_links_query`、`body_links_impact`、`body_links_visualize`（支持 `include_semantic`/`min_weight`/`discover_semantic` 参数）、CLI `links` / `fact graph` 与 Web REST/拓扑面板交互同步 | `src/api/mcp_tools.rs`, `src/main.rs`, `src/web/parity_api.rs`, `src/web/assets/mod.rs` | MCP/CLI/Web 1:1 对齐；图谱模块单测与 `cargo test` 全绿 | ✅ 已完成 | 2026-11-22 |
| **Phase 41<br/>Mesh 协议** | **41.1** | **存储与配置扩展（Mesh 信Envelope/对话/沙盒表）**：① 新增 `mesh_dialogues` 与 `mesh_transfers`（总表 `23 → 25`）及 SPI 方法；② 扩展 `MeshConfig`（`node_id`, `shared_secret`, `peer_keys`, `peer_scopes`, `replay_window_secs`, `max_transfer_bytes`, `inbox_dir`），同步 `config.example.toml` 与 `PreflightChecker` 安全基线 | `src/storage/{schema,pg_schema,duckdb_schema}.sql`, `src/storage/{models,spi,mod,migration}.rs`, `src/storage/{sqlite,pg,duckdb}_provider.rs`, `src/config/mod.rs`, `config.example.toml`, `src/preflight/mod.rs` | 三后端支持对话/投递收件箱持久化；`PreflightChecker` 校验 `mesh_inbox/` `0700` 与密钥配置 | ✅ 已完成 | 2026-11-22 |
| | **41.2** | **Mesh Wire Protocol `v1.0` 与零信任认证内核**：实现 `MeshEnvelope`、`MeshAck`、规范字节串 `HMAC-SHA256` 签名/验签、$\pm 300\text{s}$ 时间窗 + `NonceCache` 防重放、Peer Scopes 权限校验器与 `MeshClient` 升级 | `src/mesh/protocol.rs` (新), `src/mesh/auth.rs` (新), `src/mesh/mod.rs` | 篡改签名返回 `401`、重放 `nonce` 返回 `409`、越权 Scope 返回 `403`、合法签名 `100%` 放行 | ✅ 已完成 | 2026-11-22 |
| | **41.3** | **跨机结构化 Agent 对话引擎与联邦图谱探查**：① 实现 `POST /api/v1/mesh/dialogue` 与 `GET /api/v1/mesh/graph`；② 对话载荷经 `SecretRedactor` 双向脱敏与 `PromptInjectionScanner` 抗投毒扫描，支持 `ASK_CONTEXT`/`HANDOFF_TASK`/`SHARE_MEMORY`/`REPLY`；③ 补全 `WebServer` 路由挂载与 `MeshClient` 对话/联邦图谱客户端方法 | `src/mesh/dialogue.rs` (新), `src/mesh/mod.rs`, `src/web/server.rs` | 两节点实测互发对话与 `REPLY` 回执成功；含密钥或越狱文本被脱敏/标记隔离 | ✅ 已完成 | 2026-11-22 |
| | **41.4** | **受控沙盒文件投递引擎 (`<=10MB`)**：① 放宽 `src/web/server.rs` 仅针对 `/api/v1/mesh/transfer` 的 Body 限额至 `max_transfer_bytes`（其它接口维持 `64KB` 防 DoS）；② 校验 `payload_sha256`、阻止目录穿越与 `.env`/`.ssh`/`*.pem`/`*.key` 等敏感文件；③ 隔离写入 `~/.agent-body/mesh_inbox/` (`0600`) 并提供 `accept`/`reject` 安全落位原语 | `src/mesh/transfer.rs` (新), `src/mesh/mod.rs`, `src/web/server.rs` | 合法小文件精确校验 SHA-256 落入 `mesh_inbox`；超限返回 `413`、敏感文件/路径穿越返回 `403` | ✅ 已完成 | 2026-11-22 |
| | **41.5** | **Mesh 协同黑白屏 1:1 同步 & Benchmark `v3.4` (`W13`/`W14`/`DQ6`)**：扩展 `body_mesh_query` 子动作（`graph`/`dialogue_*`/`transfer_*`）、CLI `agent-body mesh` 子命令与 Web UI 协同工作台；在 `src/benchmark/` 上线 `W13`、`W14` 与 `DQ6` 并刷新 `evolution_manifest.py` 与 `EVOLUTION_GUIDE.md` | `src/api/mcp_tools.rs`, `src/main.rs`, `src/web/parity_api.rs`, `src/web/assets/mod.rs`, `src/benchmark/{mod,workloads,dq,report}.rs`, `scripts/evolution_manifest.py`, `EVOLUTION_GUIDE.md` | 黑白屏 100% Parity；Benchmark `v3.4` 全绿通过帕累托与安全门禁 | ✅ 已完成 | 2026-11-22 |
| | **41.6** | **四层文档全量对齐、现网实测验收与镜像交付**：更新 `docs/manual/03,04,06,08,10`、`INSTALL.md`、`docs/arch/` 与 `skills/`；跑通 `lint-panic`/`check`/`test`/`build --release`/现网 `build_host_graph` + `DQ6` 实测、重启守护服务并镜像同步 `/root/new/shared_git/agent-body` | 全系统文档 + `skills/` + Release 编译部署 + `/root/.agent-body/body.db` | 全测试 100% PASS、0 警告、0 panic、`make evolution-check` 0 漂移、现网健康分 & 连通率达标 | ✅ 已完成 | 2026-11-22 |

---

## 三、断点续传与依赖指引

- **串行依赖顺序**：`40.2 (存储) → 40.3 (L1/L2上卷) → 40.4 (L3语义与加权推演) → 40.5 (图谱黑白屏)` → `41.1 (Mesh存储与配置) → 41.2 (信封与HMAC认证) → 41.3 (对话与联邦图谱) → 41.4 (沙盒文件投递) → 41.5 (黑白屏与Benchmark v3.4) → 41.6 (文档与现网验收)`。
- **中断恢复方法**：新会话只需读取 `EVOLUTION_GUIDE.md` 与本路线图 `docs/src_doc/roadmap-phase40-41.md`，定位首个 `⏳` 任务即可零重扫续接。
