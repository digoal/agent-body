# 21. Agent Body 全栈可观测性与遥测闭环设计规格说明书 (`Phase 74 · ISSUE #53`)

> **适用范围**：微内核底座 (`kernel/body-sdk`, `kernel/body-kernel`)、第 10 个周边扩展 (`contrib/obs_telemetry`)、精简发行版 (`distributions/aiops-probe`, `distributions/dba-sentinel`) 与旗舰版 (`src/`)。

---

## 一、设计原则与分层隔离边界

1. **纯机制与外部协议彻底分离**：
   - `kernel/` 严禁出现 Prometheus、OTLP、Grafana、Loki、Jaeger 等外部观测协议/产品依赖或词汇（由 [scripts/arch_boundary_check.py](../../scripts/arch_boundary_check.py) 强制拦截）。
   - 微内核仅提供通用、低开销的**纯机制原语**：`KernelWaitRegistry`（11 种机制级等待直方图）、`KernelStatsRegistry`、`InFlightRegistry`（RAII 自动清理）、`SlowOpRing`（256 条有界环）与只读 Trait `ObservabilityProvider`。
   - 外部导出格式（OpenMetrics / Prometheus 0.0.4、W3C `traceparent`、JSONL 事件）与自动化异常诊断规则全部下沉到周边扩展 [contrib/obs_telemetry/](../../contrib/obs_telemetry/)。
2. **全链路单 `trace_id` 一致性保证**：
   - 修复 [pipeline.rs](../../kernel/body-kernel/src/dispatcher/pipeline.rs) 中 `emit_kernel_audit` 另起 UUID 的断链缺陷，确保单次请求的 `DispatchSuccess.trace_id`、`AuditEnvelope.trace_id`、`SlowOpRecord.trace_id` 与 SIEM RFC 5424 `SD-PARAM trace_id="..."` 100% 为同一标识。
   - 对 `run_auth_gate`（401/403）、`run_object_access_gate`、`scan_fdw`（含轨 B 沙盒超时/失败）及 `BgWorkerRoutine` 均执行同一 `trace_id` 的审计与可观测入账。

---

## 二、微内核原语 (`kernel/body-sdk` & `kernel/body-kernel`)

| 组件 | 位置 | 对标概念 | 核心能力 |
|---|---|---|---|
| `ObservabilityProvider` | `kernel/body-sdk/src/observability.rs` | 只读视图目录 | 向 `KernelServices` 挂载 `.observability_snapshot()`，保障 `contrib/` 仅单向依赖 `body-sdk` |
| `KernelStatsRegistry` | `kernel/body-kernel/src/observability.rs` | `pg_stat_statements` | 按 `(kind, target)` 聚合 `calls` / `ok` / `error` / `denied` / `timeout` / `total_us` / `max_us` |
| `InFlightRegistry` + `InFlightGuard` | `kernel/body-kernel/src/observability.rs` | `pg_stat_activity` | RAII 自动登记与退出清理活跃调用（`proc` / `fdw` / `bgworker`），防止异常泄漏 |
| `SlowOpRing` | `kernel/body-kernel/src/observability.rs` | `auto_explain` | 256 条有界环，默认阈值 `50_000us` (50ms)，自动捕获慢调用与所有非 `ok` 调用 |

---

## 三、`contrib/obs_telemetry/` 标准五件套与异常自诊断规则

- **单点四端派生算子**：
  - `body_obs_metrics` (`GET /api/v1/obs/metrics` / CLI `obs metrics` / MCP): 输出标准 OpenMetrics 0.0.4（单调递增累计 `_bucket{le="..."}`、`+Inf == _count`）或结构化 JSON 快照，并零外依读取 `/proc/self/status` RSS 与 `/proc/self/fd`（非 Linux 优雅回退 0）及 SIEM `dropped_total`。
  - `body_obs_diagnose` (`POST /api/v1/obs/diagnose` / CLI `obs diagnose` / MCP): 返回 `health_grade` (`healthy` / `degraded` / `critical`)、`active_in_flight`、`top_operations`、`wait_hotspots`、`recent_slow_or_failed`（附标准化 W3C `traceparent`）与 5 条确定性异常规则：
    - `OBS-01`：鉴权 / 访问控制拒绝数达到告警阈值（触发 `warn`）
    - `OBS-02`：外部探针或轨 B 沙盒执行超时数 `>= 1`（触发 `critical`）
    - `OBS-03`：算子/探针整体错误率超过阈值（默认 `10.0%`，触发 `critical`）
    - `OBS-04`：微内核机制等待事件估算 P95 超过 `250ms`（触发 `warn`）
    - `OBS-05`：SIEM 环形审计缓冲出现丢弃计数（触发 `warn`）

---

## 四、旗舰版 `src/` 可观测闭环 (`GET /metrics` & W3C Trace & JSONL)

1. **Prometheus / OpenMetrics 0.0.4 端点 (`GET /metrics` / `/api/v1/metrics`)**：
   - [metrics_export.rs](../../src/body/perf_awr/metrics_export.rs) 聚合现有 18 种 `WaitEventId` 直方图（`agent_body_wait_event_duration_seconds_bucket`）、`KernelSelfTelemetry`（RSS/HWM/FD/SQLite DB & WAL 字节）、活跃协作锁数、五维稳态分及 24h 双域审计分布，零新增重型三方 Crate。
2. **W3C `traceparent` 与 `X-Trace-Id` 全响应头透传**：
   - [obs_log.rs](../../src/body/obs_log.rs) 与 [server.rs](../../src/web/server.rs) 自动解析入站 `traceparent`（延用 `trace_id` 并轮换 16 位 `span_id`）或 `X-Trace-Id`，并在所有 HTTP 响应头及写操作审计 `delta` 注入跟踪标识；Web 工作台内置 **📊 Prometheus /metrics** 实时预览卡片。
3. **环境变量控制结构化 JSONL 日志 (`AGENT_BODY_LOG_FORMAT=json`)**：
   - 默认保持人类可读控制台输出；设置 `AGENT_BODY_LOG_FORMAT=json`（或 `jsonl`）后，Web 与守护进程关键事件自动输出单行结构化 JSONL，直接对接 Loki / Filebeat / OTel Collector。
