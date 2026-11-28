//! PostgreSQL 16/18 前后向 100% 兼容四表 DDL（方案 A 事务底座 + 方案 B 预留血缘表）
//!
//! 严格遵守 `ext_mem_pg_tx__` 物理表前缀隔离，并同步提供逻辑视图：
//! - `agent_memory_events`        -> `ext_mem_pg_tx__events`
//! - `agent_memory_segments`      -> `ext_mem_pg_tx__segments`
//! - `fast_weight_snapshots`      -> `ext_mem_pg_tx__fast_weight_snapshots`
//! - `compaction_jobs`            -> `ext_mem_pg_tx__compaction_jobs`

/// PostgreSQL 事务级海马体核心四表 + 单调纪元序列 + 解耦检索索引 DDL
pub const PG_TX_SCHEMA_SQL: &str = r#"
CREATE SEQUENCE IF NOT EXISTS ext_mem_pg_tx__epoch_seq START WITH 1 INCREMENT BY 1;

-- 1) 不可变追加 + 双时态记忆事件主表（对齐三层证据与 AOPCC tx_epoch）
CREATE TABLE IF NOT EXISTS ext_mem_pg_tx__events (
    event_id          BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    body_id           TEXT        NOT NULL,
    tenant_scope      TEXT        NOT NULL DEFAULT 'default',
    fact_key          TEXT        NOT NULL,
    memory_type       TEXT        NOT NULL DEFAULT 'fact',
    evidence_tier     TEXT        NOT NULL DEFAULT 'L3_SEMANTIC'
                                  CHECK (evidence_tier IN ('L1_EXPLICIT','L2_STRUCTURAL','L3_SEMANTIC')),
    payload_redacted  JSONB       NOT NULL,
    evidence_sha256   BYTEA       NOT NULL,
    valid_range       TSTZRANGE   NOT NULL DEFAULT tstzrange(clock_timestamp(), NULL, '[)'),
    tx_epoch          BIGINT      NOT NULL,
    embedding_status  TEXT        NOT NULL DEFAULT 'pending'
                                  CHECK (embedding_status IN ('pending','fresh','stale','tombstoned')),
    salience_score    REAL        NOT NULL DEFAULT 0.0,
    consolidation_st  TEXT        NOT NULL DEFAULT 'raw'
                                  CHECK (consolidation_st IN ('raw','sleep_compacted','weight_merged','purged')),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

-- 活跃事实唯一性：同一逻辑键在当前有效窗内严禁双写撕裂（不要求 superuser 安装 btree_gist）
CREATE UNIQUE INDEX IF NOT EXISTS idx_ext_mem_pg_tx_active_fact_unique
    ON ext_mem_pg_tx__events (body_id, tenant_scope, fact_key)
    WHERE upper_inf(valid_range) AND embedding_status <> 'tombstoned';

CREATE INDEX IF NOT EXISTS idx_ext_mem_pg_tx_active_epoch
    ON ext_mem_pg_tx__events (body_id, tenant_scope, tx_epoch DESC)
    WHERE upper_inf(valid_range) AND embedding_status <> 'tombstoned';

CREATE INDEX IF NOT EXISTS idx_ext_mem_pg_tx_fact_history
    ON ext_mem_pg_tx__events (body_id, tenant_scope, fact_key, tx_epoch DESC);

CREATE INDEX IF NOT EXISTS idx_ext_mem_pg_tx_scope_epoch
    ON ext_mem_pg_tx__events (tenant_scope, tx_epoch DESC);

CREATE INDEX IF NOT EXISTS idx_ext_mem_pg_tx_scope_active
    ON ext_mem_pg_tx__events (tenant_scope, tx_epoch DESC)
    WHERE upper_inf(valid_range) AND embedding_status <> 'tombstoned';

-- 2) LSM 式解耦检索段：写穿失效后立即可见度隔离，杜绝僵尸向量召回
CREATE TABLE IF NOT EXISTS ext_mem_pg_tx__segments (
    segment_id        BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    event_id          BIGINT      NOT NULL REFERENCES ext_mem_pg_tx__events(event_id) ON DELETE CASCADE,
    fts_tsv           TSVECTOR    NOT NULL,
    embedding_ref     TEXT,
    is_deleted        BOOLEAN     NOT NULL DEFAULT FALSE,
    updated_at        TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS idx_ext_mem_pg_tx_seg_fts
    ON ext_mem_pg_tx__segments USING GIN (fts_tsv)
    WHERE NOT is_deleted;

CREATE INDEX IF NOT EXISTS idx_ext_mem_pg_tx_seg_event
    ON ext_mem_pg_tx__segments (event_id)
    WHERE NOT is_deleted;

-- 3) 方案 B 预留：租户级快权重快照元数据（仅存 URI/血缘/锚定纪元，严禁存 GB 权重 BLOB）
CREATE TABLE IF NOT EXISTS ext_mem_pg_tx__fast_weight_snapshots (
    snapshot_id       UUID        PRIMARY KEY,
    body_id           TEXT        NOT NULL,
    tenant_scope      TEXT        NOT NULL DEFAULT 'default',
    base_model_id     TEXT        NOT NULL,
    parent_snapshot   UUID        REFERENCES ext_mem_pg_tx__fast_weight_snapshots(snapshot_id),
    anchored_epoch    BIGINT      NOT NULL,
    adapter_kind      TEXT        NOT NULL CHECK (adapter_kind IN ('inplace_ttt','atlas_state','larimar_mem','scol_lora')),
    artifact_uri      TEXT        NOT NULL,
    norm_ratio        REAL        NOT NULL,
    status            TEXT        NOT NULL CHECK (status IN ('warming','active','rolled_back','compacted')),
    created_at        TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS idx_ext_mem_pg_tx_fw_active
    ON ext_mem_pg_tx__fast_weight_snapshots (body_id, tenant_scope, anchored_epoch DESC)
    WHERE status = 'active';

-- 4) M2 空闲睡眠整理 & M3 神经 Compaction / 定向遗忘作业队列
CREATE TABLE IF NOT EXISTS ext_mem_pg_tx__compaction_jobs (
    job_id            BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    job_kind          TEXT        NOT NULL CHECK (job_kind IN ('sleep_context','fast_to_warm_lora','selective_unlearn')),
    tenant_scope      TEXT        NOT NULL DEFAULT 'default',
    from_epoch        BIGINT      NOT NULL,
    to_epoch          BIGINT      NOT NULL,
    tombstone_ids     BIGINT[]    NOT NULL DEFAULT '{}',
    eval_glue_delta   REAL,
    unlearn_leak_rate REAL,
    state             TEXT        NOT NULL DEFAULT 'queued'
                                  CHECK (state IN ('queued','running','verified','committed','aborted')),
    audit_trace_id    TEXT        NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS idx_ext_mem_pg_tx_jobs_state
    ON ext_mem_pg_tx__compaction_jobs (state, created_at DESC);

-- 逻辑兼容只读视图（对齐交付件 22 规范命名，便于 DBA / OLAP 直接查询）
CREATE OR REPLACE VIEW agent_memory_events AS SELECT * FROM ext_mem_pg_tx__events;
CREATE OR REPLACE VIEW agent_memory_segments AS SELECT * FROM ext_mem_pg_tx__segments;
CREATE OR REPLACE VIEW fast_weight_snapshots AS SELECT * FROM ext_mem_pg_tx__fast_weight_snapshots;
CREATE OR REPLACE VIEW compaction_jobs AS SELECT * FROM ext_mem_pg_tx__compaction_jobs;
"#;
