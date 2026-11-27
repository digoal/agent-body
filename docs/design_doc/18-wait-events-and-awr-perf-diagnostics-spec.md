# 18 — 本体等待事件 (Wait Events)、`/proc/self` 内核遥测、AWR 快照差分与 ADDM 自优化设计规格说明书

> **版本**: v1.0.0 (Phase 50~56 · ISSUE #32, #38, #40)  
> **作者**: digoal 德哥 & Agent Body Architecture Team  
> **状态**: 已批准并实装 (`src/body/perf_awr/{mod,wait_events,awr_engine,metrics_export}.rs`，`18/18` WaitEventId 全埋点强制门禁守护；Phase 74 已在此基础上拓展 OpenMetrics `/metrics`、微内核 `ObservabilityProvider` 与 `contrib/obs_telemetry`，详见 [21-observability-and-telemetry-spec.md](21-observability-and-telemetry-spec.md))  
> **对齐规约**: 规约 #2 (递弱代偿), #8 (四级自治包络), #9 (极速与轻量化), #16 (黑白屏 1:1:1 子动作同步), #18 (实证基准), #26 (双域审计), #27 (Agent 本体稳态), #28 (位置无关与错误可观测性)

---

## 一、设计动机与第一性原理

在数据库内核工程（PostgreSQL / Oracle）中，回答“系统为什么变慢、内存为什么上涨、并发为什么卡顿”从来不靠猜，而是依靠四大经典基础设施：
1. **等待事件（Wait Events，如 PostgreSQL `pg_stat_activity.wait_event_type / wait_event`）**：把黑盒耗时正交拆解为 `Lock`、`IO`、`Net`、`CPU`、`IPC`；
2. **操作级累计统计（Operation Statistics，如 `pg_stat_statements`）**：跟踪每类操作的调用频次、总耗时、均值、最大毛刺与 P50/P95/P99 分位延迟；
3. **操作系统/内核态轻量探针（eBPF / `/proc/self` Telemetry）**：直接观察进程 RSS/HWM 内存水位、打开 FD 句柄数、主动/被动上下文切换（`voluntary/nonvoluntary_ctxt_switches`）与物理磁盘读写字节；
4. **AWR 快照基线差分与 ADDM 自动诊断（Automatic Workload Repository & Automatic Database Diagnostic Monitor）**：抓取时间窗快照 `Snap_A`、`Snap_B` 或设定黄金基准 `Golden Baseline`，通过差分 $\Delta(\text{Curr} - \text{Base})$ 自动定位 Top-N 瓶颈并触发可逆自优化。

**Agent Body 作为永驻宿主机的 Agent 本体**，在 `#30`（DuckDB 同步引发 CPU 600% + 7,825 个 inotify 句柄）与 `#33`（并发锁争用与 2.1 万资产全量加载推高 RSS）中证明：本体自身必须具备一套**零外部依赖、纳秒级采样开销（`< 100 ns`）、免 `CAP_BPF` 特权即可在任意容器/宿主机运行的内生 AWR + ADDM 性能诊断与自调优引擎**。

---

## 二、架构总览与六大等待事件分类体系

### 2.1 Body Time 时间分解模型 (Time Model)

定义任意采样窗口 $[t_1, t_2]$ 内的 **本体活跃总耗时（`Body DB Time`）**：

$$T_{\text{body}} = T_{\text{cpu}} + T_{\text{wait}} = \underbrace{T_{\text{cpu\_regex}} + T_{\text{cpu\_cosine}} + T_{\text{cpu\_redact}}}_{\text{CPU 密集计算}} + \underbrace{T_{\text{lock}} + T_{\text{io}} + T_{\text{net}} + T_{\text{ipc}}}_{\text{非 CPU 等待时间}}$$

### 2.2 6 大类 18 种标准等待事件 (`WaitEventId`)

| 类别 (`WaitClass`) | 事件标识 (`event_name`) | 触发切面与物理含义 | 典型瓶颈根因 |
|---|---|---|---|
| **`Lock`** | `Lock::SqliteMutex` | 获取 `SqliteStorageProvider` `Mutex<Connection>` 等待耗时 | 长事务或高频单行写入未批处理 |
| **`Lock`** | `Lock::DuckDbSync` | `DuckDbAnalyticsEngine` 单飞同步锁等待 | 并发打开治理分析页或大批导表 |
| **`Lock`** | `Lock::Coordination` | 多 Agent 协作锁 (`CoordinationEngine`) 获取/排队等待 | 跨会话排他锁争用或死锁环 |
| **`IO`** | `IO::SqliteRead` | SQLite 查询/FTS5/图遍历执行耗时 | 缺少索引或统计信息陈旧 |
| **`IO`** | `IO::SqliteWrite` | SQLite `INSERT/UPDATE/DELETE` 落盘耗时 | WAL 检查点堆积或磁盘 fsync 慢 |
| **`IO`** | `IO::DiskWalk` | 工作区文件系统遍历与 `stat/read` (`Scanner`/`Reconciler`) | 黑名单漏配或单次全表扫描过大 |
| **`IO`** | `IO::ProcFs` | Linux `/proc` 进程、Socket 端口与 `/etc/systemd` 扫描 | `/proc` 轮询过频或进程底噪过多 |
| **`IO`** | `IO::InotifyPoll` | `FileWatcher` 事件拉取与去抖合并 | 监听目录未剪枝（如误挂 `target/`） |
| **`Net`** | `Net::OllamaEmbed` | 本地 Ollama `/api/embeddings` 向量推理请求 | 本地模型冷加载或批大小过大 |
| **`Net`** | `Net::OllamaEnrich` | 本地 Ollama L2 语义元数据精炼请求 | 小模型推理队列拥塞 |
| **`Net`** | `Net::MeshRpc` | 跨机联邦 `MeshEnvelope v1.0` HTTP 调用 | 远端 Peer 网络延迟或超时 |
| **`CPU`** | `CPU::LinkRegex` | `LinksEngine` 多语言依赖正则与 AST 模式提取 | 超大文本或复杂正则回溯 |
| **`CPU`** | `CPU::VectorCosine` | `GraphRelationshipMiner` / 混合检索余弦相似度计算 | 候选向量集未预过滤 |
| **`CPU`** | `CPU::SecretRedact` | `SecretRedactor` + `PromptInjectionScanner` 全链路扫描 | 超大载荷重复正则扫描 |
| **`CPU`** | `CPU::BriefAssemble` | `BriefAssembler` 自适应 Token 预算裁剪与乐观租约签发 | 候选上下文过多 |
| **`IPC`** | `IPC::EventBus` | 内部 `EventBus` / SSE 广播分发 | 慢消费者阻塞通道 |
| **`IPC`** | `IPC::HookSubprocess` | `HookDispatcher` 外部命令子进程执行 | 外部 Hook 脚本卡顿 |
| **`Idle`** | `Idle::DaemonLoop` | 守护进程 `tokio::select!` 空闲休眠（不计入活跃 `T_body`） | 正常空闲态 |

### 2.3 零锁原子直方图与 8 档对数分位桶

为保证探针本身绝不成为性能瓶颈（规约 #9），`WaitEventRegistry` 采用编译期固定大小静态数组 `[AtomicEventSlot; 18]`，每个槽位仅含 `AtomicU64` 计数器（`Relaxed` 原子累加，零 `Mutex`、零堆分配）：
- `calls`: 累计触发次数
- `total_us`: 累计耗时（微秒）
- `max_us`: 历史最大单次毛刺（CAS 更新）
- `buckets[8]`: 8 档对数延迟分位桶：
  - `B0`: `< 100 μs`（微秒级极速）
  - `B1`: `100 ~ 500 μs`
  - `B2`: `500 μs ~ 2 ms`
  - `B3`: `2 ~ 10 ms`
  - `B4`: `10 ~ 50 ms`
  - `B5`: `50 ~ 250 ms`
  - `B6`: `250 ms ~ 1 s`
  - `B7`: `>= 1 s`（长尾毛刺）

---

## 三、零特权 `/proc/self` 内核探针（轻量 eBPF 等价层）

无需 `CAP_BPF` 或内核头文件，直接解析当前进程与宿主机内核账本：

| 指标字段 | 采集源 | 诊断价值 |
|---|---|---|
| `rss_kb` / `hwm_kb` | `/proc/self/status` (`VmRSS`, `VmHWM`) | 进程真实匿名+驻留物理内存与历史峰值（排除 cgroup 文件缓存干扰，守卫 `< 90 MB` 红线） |
| `open_fds` | `/proc/self/fd` 目录项计数 | 实时监控打开文件/Socket/inotify 句柄数，防止句柄泄漏 |
| `voluntary_ctxt_switches` | `/proc/self/status` | **主动让出 CPU 次数**：每次 `Mutex` 阻塞、磁盘 I/O 等待或网络 Socket 等待均触发 +1 |
| `nonvoluntary_ctxt_switches` | `/proc/self/status` | **被动时间片抢占次数**：高增长代表 CPU 饱和或线程争抢 |
| `read_bytes` / `write_bytes` | `/proc/self/io` | 进程实际触发的底层块设备读/写字节数（衡量真实写放大） |
| `syscr` / `syscw` | `/proc/self/io` | `read()` / `write()` 系统调用次数 |
| `sqlite_db_bytes` / `sqlite_wal_bytes` | `db_path` 及 `db_path-wal` `stat` | 监控 SQLite 主库与 WAL 日志体积膨胀 |

---

## 四、AWR 快照、黄金基准 (`Golden Baseline`) 与差分报告

1. **快照抓取 (`perf_snapshot`)**：
   - 将当前时刻的 `WaitEventRegistry` 全量计数、`OperationStatsRegistry` Top-N 统计、`/proc/self` 内核探针与五维稳态分打包为 `AwrSnapshot`。
   - 持久化至 `system_meta` 表：
     - `awr:snap:<snap_id>`：快照 JSON 正文；
     - `awr:latest_snap_id`：最新快照 ID；
     - `awr:snap_index`：滚动保留最近 32 份快照元数据摘要（超出自动淘汰最旧快照，防库膨胀）；
     - 若指定 `set_baseline = true`，同步写入 `awr:baseline` 作为**黄金性能基准**。
2. **AWR 差分报告 (`perf_awr`)**：
   - 支持两种模式：
     - **时间窗增量模式 (`Snap_A → Snap_B`)**：用 `Snap_B` 累计计数减去 `Snap_A` 累计计数，精准还原该时间段内的真实等待分布、上下文切换增量与 I/O 增量；
     - **黄金基准回归模式 (`Golden Baseline vs Current`)**：对比当前快照与黄金基线在平均单次等待耗时、P95 分位桶、内存 `rss_kb`、句柄 `open_fds`、WAL 体积上的漂移百分比（`drift_pct`），判定是否发生性能退化（`regression_detected`）。

---

## 五、ADDM 根因诊断矩阵与 `L1_AUTONOMIC` 自优化 (`perf_tune`)

| 规则编号 | 诊断维度 | 触发阈值 | 根因判定 | `L1_AUTONOMIC` 自动调优动作 (`perf_tune`) |
|---|---|---|---|---|
| **ADDM-01** | `Lock::SqliteMutex` 锁争用 | 锁等待占 `T_body > 20%` 或 `max_us > 50ms` | 并发读写争抢单连接锁或缺少批量事务合并 | 执行 `PRAGMA busy_timeout = 5000; PRAGMA wal_checkpoint(PASSIVE);` 并建议写操作走单事务批处理 |
| **ADDM-02** | 进程内存水位偏高 | `rss_kb > 75,000` (`~73 MB`) | 大批量资产查询后 SQLite 页缓存或 glibc 堆页未归还 | 执行 `PRAGMA shrink_memory;` + 流式分批对账 + Linux `malloc_trim(0)` 立即回收物理页 |
| **ADDM-03** | SQLite WAL 膨胀或统计陈旧 | `sqlite_wal_bytes > 32 MB` 或 `IO::SqliteRead` P95 `> 5ms` | WAL 未及时截断或查询优化器统计信息滞后 | 执行 `PRAGMA optimize; PRAGMA wal_checkpoint(TRUNCATE);` |
| **ADDM-04** | 文件句柄 / inotify 水位偏高 | `open_fds > 1024` | 监听器挂载了过多子目录或连接未及时关闭 | 审计 `/proc/self/fd` 分布并提示检查 `FileWatcher` 剪枝策略 |
| **ADDM-05** | 本地 Ollama 推理等待占比高 | `Net::Ollama*` 占 `T_body > 40%` 或平均 `> 300ms` | 本地模型推理成为主耗时点 | 建议保持后台异步小批量回填，主查询链路优先走 FTS5 + 已缓存向量 |

---

## 六、黑白屏 1:1:1 接口契约（43 工具 & 27 表零膨胀）

遵循规约 #16，全部内聚于第 43 原生工具 `body_state`：

| 子动作 (`action`) | MCP 调用 (`body_state`) | CLI 子命令 (`agent-body body ...`) | Web REST API |
|---|---|---|---|
| **抓取快照 / 设定基准** | `body_state(action="perf_snapshot", label="...", set_baseline=true)` | `agent-body body perf-snapshot [--label ...] [--set-baseline]` | `POST /api/v1/body/perf/snapshot` |
| **生成 AWR 诊断报告** | `body_state(action="perf_awr", snap_a="...", snap_b="...", vs_baseline=true)` | `agent-body body perf-awr [--snap-a ...] [--snap-b ...] [--vs-baseline]` | `GET/POST /api/v1/body/perf/awr` |
| **执行 L1 自优化** | `body_state(action="perf_tune")` | `agent-body body perf-tune` | `POST /api/v1/body/perf/tune` |
| **清理快照或 AWR 报告** | `body_state(action="delete_snapshot" \| "delete_awr_report", target_id="...")` | `agent-body body perf-delete --kind snapshot\|report --id <ID>` | `POST /api/v1/body/perf/delete` |
