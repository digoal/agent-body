# Agent Body Phase 74 演进路线图 (`ISSUE #53` — 全栈工业级可观测闭环)

> **定位**：紧接 [roadmap-phase70-73.md](roadmap-phase70-73.md)（微内核与周边物理重构）之后的**全栈可观测性深化阶段**。严守微内核零协议/零业务词污染边界，在 `kernel/` 落地纯机制观测原语并修复 `trace_id` 断链，在 `contrib/obs_telemetry/` 落地标准导出与自诊断五件套扩展，并在旗舰版打通 Prometheus `/metrics`、W3C `traceparent`、`AGENT_BODY_LOG_FORMAT=json` 与 Web UI 预览。

---

## 检查点追踪表 (Checkpoints)

| 阶段 | 任务清单 | 对应文件 | 状态 |
|---|---|---|:---:|
| **Phase 74-A** | `body-sdk` 新增 `ObservabilityProvider` 与快照契约，打通 `KernelServices` 只读入口 | [kernel/body-sdk/src/observability.rs](../../kernel/body-sdk/src/observability.rs) · [routines.rs](../../kernel/body-sdk/src/routines.rs) | ✅ 已完成 |
| **Phase 74-B** | `body-kernel` 落地 `KernelStatsRegistry` / `InFlightRegistry` / `SlowOpRing` 并修复全链路 `trace_id` 断链与沙盒超时入账 | [kernel/body-kernel/src/observability.rs](../../kernel/body-kernel/src/observability.rs) · [pipeline.rs](../../kernel/body-kernel/src/dispatcher/pipeline.rs) · [engine_fdw.rs](../../kernel/body-kernel/src/engine_fdw.rs) | ✅ 已完成 |
| **Phase 74-C** | 新增第 10 个周边五件套 `contrib/obs_telemetry`（Prometheus 0.0.4、W3C Trace、JSONL、`OBS-01~05` 自诊断）并挂载 `aiops-probe` / `dba-sentinel` | [contrib/obs_telemetry/src/lib.rs](../../contrib/obs_telemetry/src/lib.rs) · [distributions/aiops-probe/](../../distributions/aiops-probe/src/lib.rs) · [distributions/dba-sentinel/](../../distributions/dba-sentinel/src/lib.rs) | ✅ 已完成 |
| **Phase 74-D** | 旗舰版 `src/` 落地 `GET /metrics`、`X-Trace-Id` / `traceparent` 响应头、`AGENT_BODY_LOG_FORMAT=json` 与 Web UI 预览卡片 | [src/body/perf_awr/metrics_export.rs](../../src/body/perf_awr/metrics_export.rs) · [src/body/obs_log.rs](../../src/body/obs_log.rs) · [src/web/server.rs](../../src/web/server.rs) | ✅ 已完成 |
| **Phase 74-E** | 增强 `arch_boundary_check.py` 禁词防线（含 `prometheus`/`otlp`）、同步一等公民文档并通过全工作区门禁 | [scripts/arch_boundary_check.py](../../scripts/arch_boundary_check.py) · [docs/design_doc/21-observability-and-telemetry-spec.md](../design_doc/21-observability-and-telemetry-spec.md) | ✅ 已完成 |

---

## 关联文档与验证入口
- 架构规格书：[21-observability-and-telemetry-spec.md](../design_doc/21-observability-and-telemetry-spec.md)
- 一键验证：`make quality-gate && RUST_TEST_THREADS=1 cargo test --workspace`
- 发行版验证：`cargo build --release -p agent-body-kernel-minimal -p agent-body-probe -p agent-body-dba && ./target/release/agent-body-probe self-check && ./target/release/agent-body-probe metrics && ./target/release/agent-body-probe diagnose`
