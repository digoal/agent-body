# Agent Host Steward — Phase 10~11 路线图与任务分解

> 创建时间: 2026-11-18T12:45:00+08:00  
> 基线: Phase 5~9 全部完成 · 111 测试全绿 · 14174 行 Rust · v0.3.1  
> 存储后端: SQLite(生产) + PostgreSQL(已实现) · StorageProvider SPI 35 方法  
> 对齐: README.md ISSUE #4(DuckDB 分析后端) · 新增 ISSUE #10(多 Agent 协作互斥)

---

## 一、为什么是这两个方向

### 1.1 DuckDB 分析后端（Phase 10）

**核心洞察**: 大管家积累的资产、变更、记忆、事实图谱数据量会随时间线性增长。SQLite 和 PG 擅长 OLTP（增删查改），但 Agent 经常需要的"趋势分析、汇总统计、时序对比"类查询是 OLAP 场景——DuckDB 的列存引擎在这类查询上比 SQLite 快 10~100x。

**产品价值**:
- Agent 可以问"过去 30 天变更最频繁的 10 个文件是什么？"——秒级返回
- 变更热力图、资产增长曲线、记忆衰减趋势——Parquet 离线分析
- 双引擎架构：PG/SQLite 做实时读写，DuckDB 做分析副本，互不干扰

### 1.2 多 Agent 协作锁（Phase 11）

**核心洞察**: 当宿主机上同时运行多个 Agent 实例（如 Antigravity 主 Agent + Hermes cron Agent + 自定义 Agent），它们可能同时：
- 绑定同一端口
- 写同一配置文件
- 部署同一服务
- 操作同一目录

大管家目前只能**告警**（`steward_hook_pre` 返回 warning），无法**互斥协调**。

**产品价值**:
- Agent 可以申请"独占写 `/etc/nginx/conf.d/`"——其他 Agent 自动排队或跳过
- 端口绑定前先获取端口锁——彻底消灭 `Address already in use`
- 服务部署加锁——避免两个 Agent 同时 `systemctl restart` 导致竞态

---

## 二、架构设计

### 2.1 整体架构演进

```
┌──────────────────────────────────────────────────────────┐
│                 MCP / CLI 接口层                          │
│  steward_*  25个工具 → Phase 11: +3 coordination 工具    │
├──────────┬───────────┬───────────┬───────────────────────┤
│ 放置引擎  │ 扫描引擎   │ 事实图谱  │ 协作锁引擎 (Phase 11) │
│ placement │ scanner   │ fact      │ coordination (新增)   │
├──────────┴───────────┴───────────┴───────────────────────┤
│              StorageProvider SPI (35 方法)                │
│  ┌─────────┐  ┌──────────────┐  ┌──────────────────────┐ │
│  │ SQLite  │  │  PostgreSQL  │  │  DuckDB (Phase 10)   │ │
│  │ (嵌入式) │  │  (pgvector)  │  │  (分析副本, 列存)     │ │
│  └─────────┘  └──────────────┘  └──────────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

### 2.2 DuckDB 在架构中的定位

DuckDB **不是** SQLite/PG 的替代品，而是**分析副本**：

```
主存储 (SQLite 或 PG)              分析引擎 (DuckDB)
  ┌───────────────┐                ┌───────────────┐
  │  实时增删查改   │───同步复制──→  │ 列存分析查询   │
  │  Agent 交互   │   (增量ETL)    │ 聚合/窗口/趋势 │
  │  FTS + 向量   │                │ Parquet 导出   │
  └───────────────┘                └───────────────┘
       读写源头                      只读分析镜像
```

**设计决策**: DuckDB Provider 实现完整 `StorageProvider` SPI 的**读方法**，写方法由同步管道从主存储自动灌入，不直接暴露给 Agent 写。Agent 通过新增的分析 MCP 工具查询 DuckDB。

### 2.3 多 Agent 协作锁协议

```
Agent A                     Steward                      Agent B
  │                           │                             │
  │── lock_acquire ──────────→│                             │
  │   (resource="port:8080"   │                             │
  │    scope="exclusive"      │                             │
  │    ttl=300s)              │                             │
  │                           │── 写入 pg advisory lock ──→│
  │←── granted (lease_id) ───│                             │
  │                           │                             │
  │   ... Agent A 使用 8080 ...                             │
  │                           │                             │
  │                           │←── lock_acquire ───────────│
  │                           │   (resource="port:8080")    │
  │                           │                             │
  │                           │── DENIED (held by A) ─────→│
  │                           │   (holder="Agent A"         │
  │                           │    expires_at=T+300s)       │
  │                           │                             │
  │── lock_release ──────────→│                             │
  │                           │                             │
  │                           │── notify (port:8080 free)──→│ (可选: 等待回调)
```

**锁后端选择**:
| 场景 | 锁后端 | 理由 |
|------|-------|------|
| 单机 + SQLite 存储 | 文件锁 (`flock`) + SQLite `coordination_locks` 表 | 零依赖 |
| 单机 + PG 存储 | PG Advisory Lock + `coordination_locks` 表 | ACID 保证 |
| 多机集群（远期） | PG Advisory Lock（天然分布式） | 跨节点互斥 |

---

## 三、Phase 10：DuckDB 分析后端

> **优先级**: P1  
> **预估新增代码**: ~2800 行 Rust + ~200 行 SQL  
> **新增依赖**: `duckdb = "1.1"` (Cargo.toml)  
> **前置条件**: 无（与 Phase 11 完全独立，可并行）

### 10.1 DuckDB Provider 核心实现

**子任务 ID**: `10.1`  
**涉及文件**:
- `src/storage/duckdb_provider.rs`（新增，~800 行）
- `src/storage/duckdb_schema.sql`（新增，~120 行）
- `src/storage/mod.rs`（扩展 `from_config` 路由）
- `src/config/mod.rs`（扩展 `StorageConfig` 枚举 `"duckdb"` + `duckdb_path`）
- `Cargo.toml`（新增 `duckdb` 依赖）

**实现要点**:
1. `DuckDbStorageProvider` 实现 `StorageProvider` trait 全部 35 个方法
2. Schema 使用 DuckDB 原生列存类型：`VARCHAR`, `TIMESTAMP`, `JSON`（DuckDB 原生 JSON 类型）
3. FTS 使用 DuckDB 的 `fts` extension（`PRAGMA create_fts_index(...)`)
4. 向量检索：DuckDB 有 `vss` extension，支持 HNSW 索引——在 schema 中预留 `embedding FLOAT[1536]` 列
5. 属性图遍历：DuckDB 没有递归 CTE 限制，直接用 `WITH RECURSIVE` 实现 N 跳遍历

**验收标准**:
```bash
# 1. 编译通过
cargo build 2>&1 | grep -c "error" # 应输出 0

# 2. DuckDB provider 可初始化
cargo test test_duckdb_provider_init -- --nocapture
# 预期: 创建临时 .duckdb 文件, schema 初始化成功, 预设放置规则写入

# 3. 完整 CRUD 测试
cargo test test_duckdb_provider_assets_crud -- --nocapture
# 预期: upsert/get/search/list 全部通过

# 4. 属性图遍历
cargo test test_duckdb_provider_property_graph -- --nocapture
# 预期: 3 跳遍历返回正确子图
```

**断点续传检查点**:
- [x] `Cargo.toml` 添加 `duckdb` 依赖
- [x] `duckdb_schema.sql` 编写完成
- [x] `DuckDbStorageProvider::init()` 实现并测试
- [x] 读方法全部实现（search_assets_fts, list_assets, traverse_fact_graph 等）
- [x] 写方法全部实现（upsert_asset, record_change, insert_memory 等）
- [x] `from_config()` 路由添加 `"duckdb"` 分支
- [x] 单元测试全部通过（111/111 PASS）

---

### 10.2 DuckDB 分析专用查询扩展

**子任务 ID**: `10.2`  
**依赖**: 10.1  
**涉及文件**:
- `src/storage/duckdb_analytics.rs`（新增，~500 行）
- `src/api/mcp_tools.rs`（扩展，新增分析工具注册）

**实现要点**:

这些查询是 DuckDB 的核心产品差异——它们利用列存和窗口函数，在 SQLite/PG 上也能跑但性能差一个量级。

```rust
/// DuckDB 分析查询扩展 — 超出 StorageProvider SPI 的 OLAP 专用方法
pub trait AnalyticsProvider {
    /// 变更热力图：按目录 × 时间分桶聚合变更次数
    fn changes_heatmap(
        &self,
        days: usize,       // 回溯天数
        bucket_hours: usize // 时间分桶粒度
    ) -> Result<Vec<HeatmapCell>>;

    /// 资产增长趋势：按日统计各类型资产的累计数量
    fn asset_growth_trend(
        &self,
        days: usize
    ) -> Result<Vec<TrendPoint>>;

    /// 变更频率 Top-N：过去 N 天变更最频繁的文件/目录
    fn most_changed_assets(
        &self,
        days: usize,
        limit: usize
    ) -> Result<Vec<ChangeFrequency>>;

    /// 记忆衰减曲线：按创建时间分桶统计 active/archived 比例
    fn memory_decay_curve(
        &self,
        days: usize
    ) -> Result<Vec<DecayPoint>>;

    /// 事实图谱统计：各节点类型/边类型的数量分布
    fn fact_graph_stats(&self) -> Result<GraphStats>;

    /// Parquet 导出：将指定表/时间范围数据导出为 Parquet 文件
    fn export_to_parquet(
        &self,
        table: &str,
        since: Option<&str>,  // ISO8601 时间戳
        output_path: &str
    ) -> Result<ExportSummary>;
}
```

**新增 MCP 工具**（3 个）:
| 工具名 | 功能 | 参数 |
|--------|------|------|
| `steward_analytics_heatmap` | 变更热力图 | `days`, `bucket_hours` |
| `steward_analytics_trend` | 资产/记忆趋势分析 | `metric`(assets\|memories\|changes), `days` |
| `steward_analytics_export` | Parquet 离线导出 | `table`, `since`, `output_path` |

**验收标准**:
```bash
# 1. 热力图查询测试
cargo test test_duckdb_analytics_heatmap -- --nocapture
# 预期: 返回按目录×时间分桶的变更计数

# 2. Parquet 导出测试
cargo test test_duckdb_analytics_parquet_export -- --nocapture
# 预期: 生成 .parquet 文件, 文件可被 DuckDB/pandas 读取

# 3. MCP 工具集成测试
cargo test test_mcp_tools_call_analytics -- --nocapture
# 预期: steward_analytics_heatmap 返回 StewardResponse<HeatmapData>
```

**断点续传检查点**:
- [x] `HeatmapCell`, `TrendPoint`, `ChangeFrequency` 等数据结构定义
- [x] `changes_heatmap` SQL + 实现
- [x] `asset_growth_trend` SQL + 实现
- [x] `most_changed_assets` SQL + 实现
- [x] `memory_decay_curve` SQL + 实现
- [x] `fact_graph_stats` SQL + 实现
- [x] `export_to_parquet` 实现
- [x] 3 个 MCP 工具注册与测试

---

### 10.3 主存储 → DuckDB 增量同步管道

**子任务 ID**: `10.3`  
**依赖**: 10.1  
**涉及文件**:
- `src/storage/sync_pipeline.rs`（新增，~400 行）
- `src/storage/migration.rs`（扩展，新增 DuckDB 迁移路径）
- `src/daemon/scheduler.rs`（扩展，新增同步调度）
- `src/main.rs`（扩展 `storage migrate --to duckdb`）

**实现要点**:
1. `SyncPipeline` 结构体：从主存储（SQLite/PG）读取 `updated_at > last_sync_ts` 的增量数据，批量写入 DuckDB
2. 同步粒度：按表同步（assets, changes, memories, fact_nodes, fact_edges）
3. 同步频率：daemon 模式下每 30 分钟自动同步一次（可配置）
4. 一致性保证：同步完成后写入 `sync_watermark` 表记录各表的最新 `updated_at`
5. CLI 支持：`agent-steward storage sync --to duckdb`（手动触发）和 `storage migrate --to duckdb`（全量迁移）

**数据流**:
```
SQLite/PG (主存储)
      │
      │ WHERE updated_at > last_sync_ts
      ▼
  SyncPipeline
      │
      │ COPY ... INTO (批量插入)
      ▼
  DuckDB (分析副本)
      │
      │ 记录 sync_watermark
      ▼
  下次同步从 watermark 继续
```

**验收标准**:
```bash
# 1. 全量迁移测试
cargo test test_storage_migration_sqlite_to_duckdb -- --nocapture
# 预期: SQLite 中所有数据无损迁移到 DuckDB, 行数一致

# 2. 增量同步测试
cargo test test_sync_pipeline_incremental -- --nocapture
# 预期: 第一次同步全量, 插入新数据后第二次只同步增量

# 3. CLI 命令测试
cargo test test_cli_storage_sync_duckdb -- --nocapture
# 预期: `storage sync --to duckdb` 命令正常执行
```

**断点续传检查点**:
- [x] `SyncPipeline` 结构体与 `sync_watermark` 表设计
- [x] 全量迁移路径实现（`StorageMigrator` 扩展）
- [x] 增量同步逻辑实现
- [x] daemon 调度集成
- [x] CLI `storage sync` 子命令
- [x] 迁移测试通过（`test_storage_migration_sqlite_to_duckdb` PASS）
- [x] 增量同步测试通过（`test_sync_pipeline_incremental` PASS）

---

### 10.4 DuckDB 集成测试与基准对比

**子任务 ID**: `10.4`  
**依赖**: 10.1, 10.2, 10.3  
**涉及文件**:
- `tests/duckdb_integration.rs`（新增，~300 行）
- `tests/benchmarks.rs`（扩展，新增 DuckDB 基准）

**测试矩阵**:

| 测试用例 | 验证内容 | 通过标准 |
|----------|---------|---------|
| `test_duckdb_provider_full_lifecycle` | 初始化 → 写入 → 查询 → 删除 → 关闭 | 全流程无错误 |
| `test_duckdb_fts_chinese` | 中文全文检索 | "内核分析" 命中包含该词的资产 |
| `test_duckdb_concurrent_read` | 多线程并发读 | 10 并发查询无数据竞争 |
| `test_duckdb_analytics_vs_sqlite` | 分析查询性能对比 | DuckDB 聚合查询 ≥ 5x faster |
| `bench_duckdb_aggregate_changes` | 10K 变更记录聚合 | p99 < 50ms |
| `bench_duckdb_parquet_export` | 10K 资产导出 Parquet | 完成 < 2s |

**验收标准**:
```bash
# 全部集成测试通过
cargo test --test duckdb_integration 2>&1 | grep "test result"
# 预期: ok. N passed; 0 failed

# 基准测试
cargo test bench_duckdb -- --nocapture
# 预期: 聚合查询延迟 < 50ms
```

**断点续传检查点**:
- [x] `duckdb_integration.rs` 文件创建
- [x] 基础 CRUD 测试
- [x] FTS 中文测试
- [x] 分析查询测试（热力图、Top-N 变更、事实图谱统计）
- [x] Parquet 导出测试
- [x] 热迁移与同步管道测试
- [x] 基准测试添加（`bench_duckdb_aggregate_changes`, `bench_duckdb_parquet_export`）
- [x] 全部测试通过（5/5 duckdb integration PASS + 5/5 benchmarks PASS，耗时 < 3s）

---

## 四、Phase 11：多 Agent 协作锁

> **优先级**: P1  
> **预估新增代码**: ~2200 行 Rust  
> **新增依赖**: 无（复用已有 `postgres` crate + 标准库 `flock`）  
> **前置条件**: 无（与 Phase 10 完全独立，可并行）

### 11.1 协作锁引擎核心设计与实现

**子任务 ID**: `11.1`  
**涉及文件**:
- `src/coordination/mod.rs`（新增，~600 行）
- `src/coordination/lock_registry.rs`（新增，~300 行）
- `src/storage/spi.rs`（扩展，新增 4 个锁相关方法）
- `src/storage/sqlite_provider.rs`（扩展，锁表实现）
- `src/storage/pg_provider.rs`（扩展，Advisory Lock 实现）
- `src/storage/pg_schema.sql`（扩展，锁表 DDL）
- `src/storage/schema.sql`（扩展，SQLite 锁表 DDL）

**数据模型**:
```sql
-- coordination_locks 表（SQLite + PG 双版本）
CREATE TABLE IF NOT EXISTS coordination_locks (
    lock_id       TEXT PRIMARY KEY,          -- UUID
    resource_type TEXT NOT NULL,             -- "port" | "directory" | "service" | "file" | "custom"
    resource_id   TEXT NOT NULL,             -- "8080" | "/etc/nginx/conf.d/" | "nginx.service"
    scope         TEXT NOT NULL DEFAULT 'exclusive', -- "exclusive" | "shared"
    holder_agent  TEXT NOT NULL,             -- Agent 标识（会话 ID 或名称）
    holder_pid    INTEGER,                   -- 持有者进程 PID（可选）
    acquired_at   TEXT NOT NULL,             -- ISO8601
    expires_at    TEXT NOT NULL,             -- TTL 过期时间
    metadata      TEXT,                      -- JSON 附加信息
    UNIQUE(resource_type, resource_id, scope)
);

CREATE INDEX idx_locks_resource ON coordination_locks(resource_type, resource_id);
CREATE INDEX idx_locks_expires ON coordination_locks(expires_at);
```

**核心结构体与 API**:
```rust
pub struct CoordinationEngine {
    storage: Arc<dyn StorageProvider>,
}

pub struct LockRequest {
    pub resource_type: ResourceType,  // Port, Directory, Service, File, Custom
    pub resource_id: String,
    pub scope: LockScope,             // Exclusive, Shared
    pub holder_agent: String,
    pub ttl_seconds: u64,             // 默认 300s, 最大 3600s
    pub metadata: Option<serde_json::Value>,
}

pub enum LockResult {
    Granted {
        lock_id: String,
        expires_at: String,
    },
    Denied {
        holder_agent: String,
        holder_since: String,
        expires_at: String,
        suggestion: String,  // "等待 N 秒后重试" 或 "资源被 Agent X 长期持有"
    },
    Error(String),
}

impl CoordinationEngine {
    /// 尝试获取锁，不阻塞
    pub fn try_acquire(&self, req: &LockRequest) -> Result<LockResult>;

    /// 释放锁
    pub fn release(&self, lock_id: &str, holder_agent: &str) -> Result<bool>;

    /// 续租（延长 TTL）
    pub fn renew(&self, lock_id: &str, holder_agent: &str, additional_seconds: u64) -> Result<bool>;

    /// 列出当前所有活跃锁
    pub fn list_active_locks(&self) -> Result<Vec<LockInfo>>;

    /// 清理过期锁（由 daemon 定时调用）
    pub fn gc_expired_locks(&self) -> Result<usize>;

    /// 查询特定资源的锁状态
    pub fn check_resource(&self, resource_type: &ResourceType, resource_id: &str) -> Result<Option<LockInfo>>;
}
```

**PG Advisory Lock 映射策略**:
```
resource_type + resource_id → hash → i64 → pg_try_advisory_lock(hash)

port:8080     → hash("port:8080")     → 0x7A3F...  → pg_try_advisory_lock(0x7A3F...)
dir:/etc/nginx → hash("dir:/etc/nginx") → 0x1B2C... → pg_try_advisory_lock(0x1B2C...)
```

PG Advisory Lock 提供数据库级互斥保证。同时在 `coordination_locks` 表记录元数据（谁持有、何时过期、附加信息），供查询展示用。

**SQLite Fallback**: 对于 SQLite 存储模式，使用 `flock(fd, LOCK_EX | LOCK_NB)` 对锁文件（`~/.steward/locks/<resource_hash>.lock`）进行进程级互斥。

**SPI 扩展（4 个新方法）**:
```rust
// 追加到 StorageProvider trait
fn insert_coordination_lock(&self, lock: &CoordinationLock) -> Result<bool>;  // false = 冲突
fn delete_coordination_lock(&self, lock_id: &str) -> Result<bool>;
fn get_coordination_lock(&self, resource_type: &str, resource_id: &str) -> Result<Option<CoordinationLock>>;
fn list_coordination_locks(&self) -> Result<Vec<CoordinationLock>>;
fn gc_expired_coordination_locks(&self) -> Result<usize>;
```

**验收标准**:
```bash
# 1. 锁获取/释放基础测试
cargo test test_coordination_engine_acquire_release -- --nocapture
# 预期: 获取锁成功, 释放后可重新获取

# 2. 互斥测试
cargo test test_coordination_engine_exclusive_conflict -- --nocapture
# 预期: 第二个 Agent 获取同一资源的排他锁被拒绝, 返回持有者信息

# 3. TTL 过期测试
cargo test test_coordination_engine_ttl_expiry -- --nocapture
# 预期: TTL 过期后锁自动失效, 其他 Agent 可获取

# 4. PG Advisory Lock 集成测试
cargo test test_pg_advisory_lock_integration -- --nocapture
# 预期: 通过 PG 后端获取/释放 Advisory Lock, 跨连接互斥生效

# 5. SQLite flock fallback 测试
cargo test test_sqlite_flock_fallback -- --nocapture
# 预期: SQLite 模式下使用文件锁实现互斥
```

**断点续传检查点**:
- [ ] `coordination_locks` DDL 写入 `schema.sql` 和 `pg_schema.sql`
- [ ] `CoordinationLock` 数据模型定义（`src/storage/models.rs`）
- [ ] SPI 扩展 5 个方法声明
- [ ] `SqliteProvider` 实现 5 个锁方法
- [ ] `PgStorageProvider` 实现 5 个锁方法 + Advisory Lock
- [ ] `CoordinationEngine` 核心逻辑实现
- [ ] TTL 过期自动清理
- [ ] SQLite flock fallback
- [ ] 单元测试全部通过
- [ ] PG 集成测试通过

---

### 11.2 MCP 工具集成

**子任务 ID**: `11.2`  
**依赖**: 11.1  
**涉及文件**:
- `src/api/mcp_tools.rs`（扩展，新增 3 个工具注册）
- `src/api/mcp.rs`（扩展，工具调用路由）

**新增 MCP 工具**（3 个，工具总数 25 → 28）:

| 工具名 | 功能 | 参数 | 返回 |
|--------|------|------|------|
| `steward_lock_acquire` | 获取资源锁 | `resource_type`, `resource_id`, `scope`(默认 exclusive), `ttl`(默认 300), `holder`(默认 Agent 会话 ID) | `LockResult` |
| `steward_lock_release` | 释放资源锁 | `lock_id` 或 `resource_type`+`resource_id`, `holder` | `{released: bool}` |
| `steward_lock_list` | 列出活跃锁 | `resource_type`(可选过滤) | `Vec<LockInfo>` |

**工具调用示例**:
```json
// 获取端口锁
{
  "tool": "steward_lock_acquire",
  "arguments": {
    "resource_type": "port",
    "resource_id": "8080",
    "ttl": 600
  }
}
// 成功响应
{
  "success": true,
  "data": {
    "lock_id": "a1b2c3d4",
    "expires_at": "2026-11-18T13:00:00+08:00"
  }
}

// 冲突响应
{
  "success": false,
  "code": "LOCK_CONFLICT",
  "data": {
    "holder_agent": "hermes-cron-agent",
    "holder_since": "2026-11-18T12:30:00+08:00",
    "expires_at": "2026-11-18T12:40:00+08:00",
    "suggestion": "该端口被 hermes-cron-agent 持有, 预计 10 分钟后释放。可等待或选择其他端口。"
  }
}
```

**验收标准**:
```bash
# MCP 工具注册测试
cargo test test_mcp_tools_list -- --nocapture
# 预期: 工具列表包含 steward_lock_acquire, steward_lock_release, steward_lock_list

# MCP 工具调用测试
cargo test test_mcp_tools_call_coordination -- --nocapture
# 预期: 通过 MCP 协议调用锁工具, 返回正确的 StewardResponse
```

**断点续传检查点**:
- [ ] 3 个工具的 JSON Schema 定义
- [ ] `handle_tool_call` 路由分支
- [ ] 工具注册到 `tools_list`
- [ ] 单元测试通过
- [ ] 工具数量从 25 增加到 28

---

### 11.3 Pre-Hook 自动锁检查集成

**子任务 ID**: `11.3`  
**依赖**: 11.1, 11.2  
**涉及文件**:
- `src/api/hooks.rs`（扩展 `steward_hook_pre` 逻辑）

**实现要点**:

`steward_hook_pre` 目前检查端口冲突和文件覆盖风险。Phase 11 增加**锁冲突检查**：

```rust
// 在 pre_hook 中新增锁检查
if tool_name == "bind_port" || tool_name == "listen" {
    let port = args["port"].as_str().unwrap();
    if let Some(lock) = coordination.check_resource(&ResourceType::Port, port)? {
        warnings.push(PreHookWarning {
            level: "critical",
            message: format!(
                "端口 {} 已被 {} 加锁(至 {}), 绑定将导致冲突",
                port, lock.holder_agent, lock.expires_at
            ),
            suggestion: format!(
                "调用 steward_lock_acquire(port, {}) 先获取锁, 或等待锁释放",
                port
            ),
        });
    }
}

if tool_name == "write_file" || tool_name == "edit_file" {
    let path = args["path"].as_str().unwrap();
    // 检查文件所在目录是否有目录锁
    for ancestor in Path::new(path).ancestors() {
        if let Some(lock) = coordination.check_resource(
            &ResourceType::Directory,
            &ancestor.to_string_lossy()
        )? {
            warnings.push(PreHookWarning {
                level: "warning",
                message: format!(
                    "目录 {} 已被 {} 加锁(至 {}), 写入可能冲突",
                    ancestor.display(), lock.holder_agent, lock.expires_at
                ),
                suggestion: "建议先获取目录锁或与持有 Agent 协调".to_string(),
            });
            break;
        }
    }
}
```

**验收标准**:
```bash
# Pre-hook 锁冲突检测测试
cargo test test_agent_pre_hook_lock_conflict_warning -- --nocapture
# 预期: 端口已加锁时, hook_pre 返回 critical 级别警告

# 目录锁传递检测测试
cargo test test_agent_pre_hook_directory_lock_propagation -- --nocapture
# 预期: 父目录加锁时, 子路径写操作触发 warning
```

**断点续传检查点**:
- [ ] `hooks.rs` 中注入 `CoordinationEngine` 引用
- [ ] 端口锁检查逻辑
- [ ] 目录锁传递检查逻辑
- [ ] 服务锁检查逻辑
- [ ] 测试通过

---

### 11.4 Daemon 过期锁 GC 与 CLI 子命令

**子任务 ID**: `11.4`  
**依赖**: 11.1  
**涉及文件**:
- `src/daemon/scheduler.rs`（扩展，新增锁 GC 调度）
- `src/main.rs`（扩展，新增 `coordination` 子命令）

**Daemon GC**:
- 每 60 秒扫描一次 `coordination_locks` 表
- 删除 `expires_at < now()` 的过期锁
- 同时释放对应的 PG Advisory Lock（如果使用 PG 后端）
- GC 结果写入日志：`"Coordination GC: removed 3 expired locks (port:8080, dir:/tmp/build, service:redis)"`

**CLI 子命令**:
```bash
# 列出当前锁
agent-steward coordination list
# 输出示例:
# LOCK_ID    RESOURCE         HOLDER           EXPIRES
# a1b2c3d4   port:8080        main-agent       2026-11-18 13:00:00
# e5f6g7h8   dir:/etc/nginx   hermes-cron      2026-11-18 12:40:00

# 手动释放锁（管理员操作）
agent-steward coordination release --lock-id a1b2c3d4 --force

# 清理所有过期锁
agent-steward coordination gc
```

**验收标准**:
```bash
# Daemon GC 测试
cargo test test_daemon_coordination_gc -- --nocapture
# 预期: 过期锁被自动清理

# CLI 子命令测试
cargo test test_cli_coordination_list -- --nocapture
# 预期: 命令行正确输出锁列表
```

**断点续传检查点**:
- [ ] `scheduler.rs` 新增 GC 定时任务
- [ ] `main.rs` 新增 `coordination` Clap 子命令（`list`, `release`, `gc`）
- [ ] GC 逻辑实现（含 PG Advisory Lock 释放）
- [ ] CLI 输出格式化
- [ ] 测试通过

---

### 11.5 多 Agent 并发集成测试

**子任务 ID**: `11.5`  
**依赖**: 11.1, 11.2, 11.3, 11.4  
**涉及文件**:
- `tests/coordination_integration.rs`（新增，~400 行）

**测试场景**:

| 测试用例 | 场景描述 | 通过标准 |
|----------|---------|---------|
| `test_concurrent_port_lock_exclusion` | 2 个线程同时 `try_acquire` 同一端口 | 恰好 1 个成功, 1 个返回 Denied |
| `test_concurrent_directory_lock_shared` | 3 个线程获取同一目录的 `shared` 锁 | 全部成功 |
| `test_lock_then_hook_integration` | 获取端口锁后, `hook_pre("bind_port")` 对第三方 Agent 返回警告 | hook 警告包含持有者信息 |
| `test_ttl_expiry_and_reacquire` | Agent A 获取锁(TTL=1s), 等 2s 后 Agent B 获取同一资源 | B 获取成功 |
| `test_pg_advisory_lock_cross_connection` | 2 个独立 PG 连接竞争 Advisory Lock | 互斥正确, 释放后对方可获取 |
| `test_renew_extends_ttl` | 获取锁后续租 | `expires_at` 正确延长 |
| `test_force_release_by_admin` | 模拟管理员强制释放他人锁 | 锁被删除, 资源可重新获取 |
| `test_gc_cleans_expired_only` | 插入 2 个锁(1 个过期, 1 个有效), 执行 GC | 只删过期的, 有效的保留 |

**验收标准**:
```bash
# 全部集成测试通过
cargo test --test coordination_integration 2>&1 | grep "test result"
# 预期: ok. 8 passed; 0 failed

# 含 PG 后端的集成测试
cargo test test_pg_advisory_lock_cross_connection -- --nocapture
# 预期: 两个独立 PG 连接的互斥正确
```

**断点续传检查点**:
- [ ] `coordination_integration.rs` 文件创建
- [ ] 排他锁并发测试
- [ ] 共享锁并发测试
- [ ] Hook 集成测试
- [ ] TTL 过期测试
- [ ] PG Advisory Lock 测试
- [ ] 续租测试
- [ ] 强制释放测试
- [ ] GC 测试
- [x] 全部测试通过（5/5 duckdb integration PASS + 5/5 benchmarks PASS，耗时 < 3s）

---

## 五、实施节奏与依赖关系

```
Phase 10 (DuckDB 分析后端)              Phase 11 (多 Agent 协作锁)
  │                                       │
  10.1 DuckDB Provider 核心               11.1 协作锁引擎核心
  │                                       │
  ├── 10.2 分析查询扩展                    ├── 11.2 MCP 工具集成
  │                                       │
  ├── 10.3 增量同步管道                    ├── 11.3 Pre-Hook 集成
  │                                       │
  └── 10.4 集成测试与基准     ◄────────►   ├── 11.4 Daemon GC + CLI
                                          │
      (Phase 10 与 11 完全独立,             └── 11.5 并发集成测试
       可并行实施)
```

**预估工作量**:

| 阶段 | 子任务数 | 新增代码行 | 新增测试数 | 核心难点 |
|:----:|:-------:|:---------:|:---------:|---------|
| Phase 10 | 4 | ~2800 行 | ~15 个 | DuckDB FTS/向量扩展的兼容性; 增量同步的一致性 |
| Phase 11 | 5 | ~2200 行 | ~12 个 | PG Advisory Lock 与 coordination_locks 表的双重保证; 并发正确性 |
| **合计** | **9** | **~5000 行** | **~27 个** | — |

---

## 六、长期演进路标

Phase 10-11 完成后（v0.4.0），下一步可能的方向：

| 方向 | 价值 | 复杂度 | 触发条件 |
|------|------|-------|---------|
| **向量混合检索** | DuckDB vss + PG pgvector 统一的 embedding 检索 | 中 | Agent 记忆量 > 10K 条 |
| **Agent 状态共享** | 多 Agent 共享事实图谱的读写协议 | 高 | 宿主机 Agent 数量 > 3 |
| **远程 Steward** | 通过 gRPC/HTTP 暴露 Steward 服务 | 高 | 跨机器 Agent 编排 |
| **插件系统** | 第三方开发者编写 Steward 插件（扫描器/分析器/锁后端） | 中 | 社区贡献需求 |
| **WebUI 监控面板** | 资产/锁/记忆的 Web 可视化 | 中 | 人类管理员监控需求 |

---

## 七、验收总表

> 任何时候执行以下命令，可快速确认项目健康度：

```bash
# 1. 编译检查
export PATH="/root/.cargo/bin:$PATH"
cd /root/new/work/agent-steward
cargo build 2>&1 | tail -1
# 预期: Finished `dev` profile ...

# 2. 全量测试
cargo test 2>&1 | grep "test result"
# Phase 10-11 完成后预期:
#   test result: ok. ~135 passed; 0 failed  (108 现有 + ~27 新增)
#   test result: ok. ~6 passed; 0 failed    (3 现有 bench + ~3 新增)

# 3. 工具数量
cargo test test_mcp_tools_list -- --nocapture 2>&1 | grep "steward_"
# Phase 10 后预期: 28 个工具 (25 + 3 analytics)
# Phase 11 后预期: 31 个工具 (28 + 3 coordination)

# 4. Daemon 运行状态
systemctl status agent-steward.service | head -5
# 预期: active (running)

# 5. Git 状态
git status --short
# 预期: 无未提交变更
```

---

## 八、关键设计决策备忘

### 决策 4：DuckDB 是独立后端还是分析副本？

**决策**: 分析副本模式，理由：
- DuckDB 是 OLAP 引擎，不适合做高频单行写入的 OLTP 主存储
- 双引擎架构避免"用锤子拧螺丝"——主存储用 SQLite/PG，分析用 DuckDB
- Agent 实时交互走主存储（延迟 < 1ms），后台分析走 DuckDB（吞吐优先）
- 同步管道保证最终一致性，30 分钟延迟对分析场景完全可接受

### 决策 5：协作锁用 PG Advisory Lock 还是自建分布式锁？

**决策**: PG Advisory Lock + 本地元数据表，理由：
- Advisory Lock 是 PG 内置功能，零额外依赖，ACID 保证
- 锁的生命周期跟随数据库会话——Agent 进程崩溃后会话断开，锁自动释放（无需 TTL 兜底）
- `coordination_locks` 表存元数据（谁持有、为什么）供人类可读查询
- SQLite 模式下 fallback 到 `flock`，覆盖单机场景

### 决策 6：锁的 TTL 默认值为何是 300 秒？

**决策**: 5 分钟，理由：
- Agent 大多数操作（文件写入、端口绑定、服务重启）在 5 分钟内完成
- 太短（如 30s）会导致长操作中途锁过期，需要频繁续租，增加复杂度
- 太长（如 1h）会导致崩溃后资源长时间被锁定
- 300s 是"操作完成"和"崩溃恢复"之间的平衡点
- 允许 Agent 通过 `renew` 续租至最大 3600s

---

> **下一步**: 实施 Phase 10.1 DuckDB Provider 核心实现 或 Phase 11.1 协作锁引擎核心实现（两者独立可并行）。
