# Phase 52 演进路线图：P0 安全止血、编译阻断修复、运行时排雷与损毁文档/SKILL 急救 (ISSUE #36)

> **对应 ISSUE**: [ISSUE.md #36](../../ISSUE.md)  
> **所属计划**: 方案 A · `v1.0.0 GA` 工业级收口路线（Phase 52 ~ Phase 57）  
> **创建时间**: 2026-11-22 21:05 CST  
> **当前状态**: ✅ 全部完成（P52-01 ~ P52-07 全部 7 项原子任务已验收交付）  
> **关联规约**: README 规约 #8, #9, #10, #11, #12, #13, #14, #19, #20, #24, #27, #28

---

## 一、阶段目标与治理边界

1. **P52-01 方案 A 全景立项**：在 `ISSUE.md` 登记 `#36~#41`（对应 Phase 52~57）、在 `user_feedback.md` 补录本次全仓审计发现（`CON-20~24` / `REQ-19~24`）并修复 2 处历史断链，刷新 `EVOLUTION_GUIDE.md` Tier-0 入口。
2. **P52-02 容器构建阻断修复**：升级 `Dockerfile` 基础镜像至 `rust:1.88-bookworm`、补齐 `COPY presentation ./presentation`、修正 `EXPOSE 8765` 与 `HEALTHCHECK`；清理 `docker-compose.yml` 冲突 `/proc` 挂载与硬编码工作区路径并补 `healthcheck`；对齐 `.dockerignore`；加固 `build.rs` 无 `.git` 时的安全回退。
3. **P52-03 Tokio 与边界 Panic 排雷**：`McpServer::run_stdio` 迁移至 `tokio::task::spawn_blocking` 隔离；`ExternalContactsManager` / `EmbeddingClient` 同步 HTTP 隔离防 Tokio drop panic；修复 `PlacementEngine`/`PlacementEngineV2` 空规则 `&rules[0]` 越界、`PortChecker` `u16` 65535 溢出、`PowerContextAdapter` UTF-8 字节切片、`ScanStateMachine` 断点文件被删跳过全树 Bug。
4. **P52-04 安全纵深补洞与 Web 鉴权**：修复 `body_bridge.py` 无端口破坏性命令绕过；分离 `GET`（纯只读/预览）与 `POST /api/v1/body/perf/awr` 写语义并将 `POST/PUT/DELETE`、`links/mermaid?discover_semantic=true`、`mesh/locks/acquire` 统一纳入常量时间鉴权与双域审计；补充 Mesh `read:facts` 作用域校验、`SKILL.md` 抗注入、`env/port.rs` & `env/gc.rs` cmdline 脱敏及严格路径组件校验、`AutonomyEnvelope` 未知动作 fail-closed。
5. **P52-05 核心逻辑、账本、Clap 与模板纠错**：修复 `ledger.rs` 死锁 100% 误报与 `"denied"` 漏记；修复 `handle_mesh_graph` URL 解码；修复 `cli_args.rs` 4 处 Clap `bool` 无法传 `false` 与 `name = "agent-body"`；修复 `client_gen.rs` & `diagnoser.rs` 3 条无效 CLI 命令模板。
6. **P52-06 P0 损毁文档与 3 套 SKILL 急救**：修复 `04-agent-body-philosophy.md` 与 `17-agent-embodiment-and-continuity-spec.md` Phase 48 批量替换语义损毁；修复 `skills/agent-body-*/SKILL.md` 全部 12 处断链、`MemoryMax=64M`、幻觉命令与错误 MCP 参数，并同步全局 SKILL 镜像。
7. **P52-07 回归验证与现网交付**：补全回归单测与集成测试，通过 `make quality-gate && make test`、`make restart`、现网 `feedback reconcile` 与 `make sync-mirror`。

---

## 二、原子任务断点续传追踪表

| 任务 ID | 里程碑 | 原子任务描述 | 状态 | 完成时间 | 验证摘要 |
|---|---|---|---|---|---|
| **P52-01** | M1 | 方案 A 立项：`ISSUE.md` (`#36~#41`)、`user_feedback.md` (`CON-20~24` / `REQ-19~24`)、`roadmap-phase52.md`、`EVOLUTION_GUIDE.md` | ✅ 已完成 | 2026-11-22 21:08 | 全景规划落账，修复 `user_feedback.md` 2 处断链，Tier-0 对齐 |
| **P52-02** | M1 | 修复 `Dockerfile` / `docker-compose.yml` / `.dockerignore` / `build.rs` 容器构建阻断与健康检查 | ✅ 已完成 | 2026-11-22 21:07 | `rust:1.88-bookworm`、补 `presentation/`、`EXPOSE 8765`、`HEALTHCHECK`、`build.rs` 降级 |
| **P52-03** | M2 | 消除 Tokio blocking panic (`mcp.rs`/`contacts.rs`/`embedding.rs`) 与 5 处边界 Panic/遍历 Bug | ✅ 已完成 | 2026-11-22 21:19 | `spawn_blocking` / `thread::scope` 隔离、`placement` 空表、`port` 65535、`powercontext` 中文、`state_machine` 删文件续扫全测通过 |
| **P52-04** | M2 | 修复 `body_bridge.py` 安全绕过、Web `GET` 写库/鉴权审计、Mesh scope、SKILL 抗注入、`gc`/`port` 与 `AutonomyEnvelope` fail-closed | ✅ 已完成 | 2026-11-22 21:19 | `GET`/`POST perf/awr` 分离、`mermaid` 写鉴权、`mesh/locks` 审计、`gc` 防 `/etc/templates`、`Envelope` L2 兜底全绿 |
| **P52-05** | M3 | 修复 `ledger.rs` 死锁误报/漏记、`handle_mesh_graph` URL 解码、`cli_args.rs` bool/name 与模板错误 CLI | ✅ 已完成 | 2026-11-22 21:15 | `ledger` 死锁误报=0 & 拒锁精准记账、URL 中文编码命中、Clap `--hybrid=false` 解析通过 |
| **P52-06** | M3 | 修复 `04`/`17` 改名损毁与 3 套 `SKILL.md` 12 处断链/`MemoryMax`/幻觉命令并同步全局镜像 | ✅ 已完成 | 2026-11-22 21:15 | 0 断链校验通过，`/root/.agents/skills/agent-body-*` 0 diff 同步 |
| **P52-07** | M4 | 补全回归测试，通过 `make quality-gate && make test`、`make restart`、现网对账与 `make sync-mirror` | ✅ 已完成 | 2026-11-22 21:26 | `quality-gate` 0 drift/0 warning、`281/281` 测试全绿、现网重启与对账通过 |
