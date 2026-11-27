# Phase 70~73 演进路线图：微内核 (`kernel/`) 与周边 (`contrib/`) 分离架构重构及四大发行版 (ISSUE #52)

> **对应 ISSUE**: [ISSUE.md #52](../../ISSUE.md#L247)  
> **架构规格**: [20-plugin-microkernel-architecture-spec.md](../design_doc/20-plugin-microkernel-architecture-spec.md)  
> **创建时间**: 2026-11-27 16:08 CST  
> **当前状态**: ✅ 已完成（Phase K1~K5 全量落地 · 2026-11-27 CST）  
> **关联规约**: 规约 #1, #2, #5, #8, #9, #11, #13, #15, #16, #18, #19, #20, #28

---

## 一、演进铁律与四阶段总览

严格遵守 **先纯化微内核 → 再剥离存量周边 → 最后在 `contrib/` 落地新 GAP** 的串行演进秩序：

1. **Phase 70 (K1)**：纯微内核契约层 `kernel/body-sdk`、`kernel/body-protocol`、`kernel/body-kernel` 目录与 Hook 底座 + `scripts/arch_boundary_check.py` 防腐门禁（**0 业务/运维新 GAP 混入**）。
2. **Phase 71 (K2)**：`body-kernel` `TableAmRoutine` + `ScopedSqlExecutor` + `RrfMergeExecutor` + `KernelDispatcher` 四端派生 + 轨 B 沙盒代理 + `distributions/kernel-minimal`。
3. **Phase 72 (K3)**：建立顶层 `contrib/` 目录，落地标准五件套基础周边模块（`idx_fts_fuzzy`、`gov_placement`、`probe_db_engines`）。
4. **Phase 73 (K4)**：在 `contrib/` 全新实现 6 大运维与政务云 AIOps GAP 模块（`probe_linux_ops`、`probe_jvm_middleware`、`hook_auth_mtls_rbac`、`hook_gov_compliance`、`hook_audit_siem`、`fdw_cmdb_ticket`），并组装验证 `distributions/aiops-probe` 与 `distributions/dba-sentinel`。
5. **收口验证 (K5)**：`Makefile` / `code_hygiene.py` / `Dockerfile` / `.github/workflows/ci.yml` / `README.md` / `ISSUE.md` / `progress.md` / `make codegraph` 全量门禁与三发行版实证验收。

---

## 二、原子任务断点续传追踪表

| 任务 ID | 所属阶段 | 描述 | 状态 | 完成摘要 |
|---|---|---|---|---|
| **P70-01** | K1 | Workspace 根配置与 `roadmap-phase70-73.md` / 索引建立 | ✅ 已完成 | 根 `Cargo.toml` 追加 `[workspace]` (`default-members = ["."]`)，建立断点路线图与指针 |
| **P70-02** | K1 | `kernel/body-sdk`（6 大 Routine、7 大 Hook、`EvidenceTuple`、`body_module_magic!`） | ✅ 已完成 | `types.rs` / `routines.rs` / `hooks.rs` / `extension.rs` 落地，4 单测通过 |
| **P70-03** | K1 | `kernel/body-protocol`（`.control.toml` 解析校验与 JSON-RPC 2.0 线协议） | ✅ 已完成 | `ExtensionControlManifest` 前缀与 `L0` 沙盒校验 + JSON-RPC 帧落地，5 单测通过 |
| **P70-04** | K1 | `kernel/body-kernel` 核心前半部分（8 张 `body_cat_*` 目录表、`HookChain`、`KernelWaitRegistry`） | ✅ 已完成 | `catalog.rs` / `hook_chain.rs` / `wait_events.rs` + 集成测试全绿（363 workspace tests） |
| **P70-05** | K1 | 新增 `scripts/arch_boundary_check.py` 防腐门禁并验证 `K1` 编译全绿 | ✅ 已完成 | 5 大防腐规则校验全绿，0 跨层依赖、0 业务词污染 |
| **P71-01** | K2 | `body-kernel` `SqliteCoreTableAm`（5 基表 + RLS 下推 + `ScopedSqlExecutor`）与 `MemoryTableAm` | ✅ 已完成 | `sqlite_am.rs` + `mem_am.rs` + `scoped_sql.rs` 落地，前缀隔离与 RLS 过滤全绿 |
| **P71-02** | K2 | `body-kernel` `RrfMergeExecutor`、`KernelDispatcher`（四端自动派生）与 `OutOfProcessFdwAdapter` | ✅ 已完成 | 四端自动派生、`RrfMergeExecutor` 离线重归一化及轨 B 超时强杀落地 |
| **P71-03** | K2 | 新建 `distributions/kernel-minimal`（0 `contrib` 依赖）并验证纯内核独立运行 | ✅ 已完成 | `agent-body-kernel-minimal` 0 `contrib` 通过全部端到端与 `self-check`（373 workspace tests） |
| **P72-01** | K3 | 新建 `contrib/idx_fts_fuzzy`（驼峰 BM25 + Pure-Rust Fuzzy 五件套） | ✅ 已完成 | `idx_fts_fuzzy` 五件套落地，驼峰/模糊/BM25 RRF 回归全绿 |
| **P72-02** | K3 | 新建 `contrib/gov_placement`（个人放置规约与规则表五件套，0 硬编码） | ✅ 已完成 | `gov_placement` 五件套落地，动态推导工作区、四端自动挂载与守卫通过 |
| **P72-03** | K3 | 新建 `contrib/probe_db_engines`（PG/达梦/人大金仓配置解析探针五件套） | ✅ 已完成 | `probe_db_engines` 五件套 + 内核 `scan_fdw` 托管流水线落地全绿 |
| **P73-01** | K4 | 新建 `contrib/probe_linux_ops`（`started_at`/长命令/ESTABLISHED/RHEL Cron/Systemd/CapEff/漂移 Worker） | ✅ 已完成 | 完整解析 `/proc` (`started_at`/全长 cmdline/`CapEff`/多监听/ESTABLISHED) 与 RHEL cron/systemd drop-in，含增量 `Worker` 与 SQL 迁移 |
| **P73-02** | K4 | 新建 `contrib/probe_jvm_middleware`（Java/Tomcat/东方通/Nginx upstream 拓扑） | ✅ 已完成 | 解析多 JVM 实例、`spring_configs`、Tomcat/东方通 与 Nginx `include`/`upstream` 拓扑 |
| **P73-03** | K4 | 新建 `contrib/hook_auth_mtls_rbac`（强制读/写鉴权、mTLS 与多租户 RLS） | ✅ 已完成 | 强制全端读/写 `401` 门禁、mTLS CN / Bearer RBAC 与多租户 RLS 隔离 |
| **P73-04** | K4 | 新建 `contrib/hook_gov_compliance`（Java `-D` 精准掩码、PII 脱敏、全源覆盖与 `host_labels`） | ✅ 已完成 | `-D`/`--` 精准值掩码、URI/PEM/Bearer/Cron 脱敏、身份证/手机号 PII 与 `host_label:*` 注入 |
| **P73-05** | K4 | 新建 `contrib/hook_audit_siem`（只读取证全量审计 + RFC5424 Syslog/OTLP） | ✅ 已完成 | 4096 容量有界环形缓冲、RFC 5424 & OTLP 格式化及 `worker:siem_flush` 落盘流 |
| **P73-06** | K4 | 新建 `contrib/fdw_cmdb_ticket`（轨 B 真实 JSON-RPC 脚本 + AIOps 交接证据包算子） | ✅ 已完成 | 零依赖 Python3 轨 B 适配器 + `body_aiops_evidence_pack` 四端派发与超时强杀验证 |
| **P73-07** | K4 | 组装 `distributions/aiops-probe` 与 `distributions/dba-sentinel` 并跑通端到端集成测试 | ✅ 已完成 | 双发行版 0 告警通过 `aiops_e2e`、`dba_e2e` 与内置 `self-check`（0 DuckDB / 0 C++） |
| **P73-08** | K5 | 更新 `Makefile`/`code_hygiene.py`/`Dockerfile`/`ci.yml`/文档/总账/`codegraph` 并执行终极全绿验收 | ✅ 已完成 | 六重 `quality-gate`、全工作区 `--workspace` clippy/测试、三发行版 release `self-check` 与 `codegraph` 全绿 |
