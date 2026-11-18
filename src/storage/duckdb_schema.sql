-- ============================================================
-- Agent Host Steward — DuckDB Schema (Analytical & OLAP Replica)
-- ============================================================

CREATE SEQUENCE IF NOT EXISTS changes_seq START 1;
CREATE SEQUENCE IF NOT EXISTS links_seq START 1;
CREATE SEQUENCE IF NOT EXISTS fact_edges_seq START 1;

CREATE TABLE IF NOT EXISTS placement_rules (
    id              VARCHAR PRIMARY KEY,
    rule_name       VARCHAR NOT NULL UNIQUE,
    pattern_type    VARCHAR NOT NULL,
    target_base_dir VARCHAR NOT NULL,
    naming_pattern  VARCHAR,
    description     VARCHAR,
    guideline       VARCHAR,
    priority        INTEGER DEFAULT 100,
    created_at      VARCHAR,
    updated_at      VARCHAR
);

CREATE TABLE IF NOT EXISTS scan_policies (
    id              VARCHAR PRIMARY KEY,
    path_pattern    VARCHAR NOT NULL UNIQUE,
    tier            VARCHAR NOT NULL,
    max_depth       INTEGER DEFAULT -1,
    enable_fts      BOOLEAN DEFAULT TRUE,
    enable_vec      BOOLEAN DEFAULT TRUE,
    enable_graph    BOOLEAN DEFAULT TRUE,
    notes           VARCHAR,
    created_at      VARCHAR
);

CREATE TABLE IF NOT EXISTS assets (
    id              VARCHAR PRIMARY KEY,
    path            VARCHAR NOT NULL UNIQUE,
    type            VARCHAR NOT NULL,
    owner           VARCHAR DEFAULT 'user:digoal',
    security_level  VARCHAR DEFAULT 'internal',
    scan_tier       VARCHAR DEFAULT 'deep',
    description     VARCHAR,
    desc_kv         VARCHAR DEFAULT '{}',
    usage           VARCHAR,
    size_bytes      BIGINT,
    file_count      BIGINT,
    tags            VARCHAR DEFAULT '[]',
    status          VARCHAR DEFAULT 'active',
    last_scanned_at VARCHAR,
    last_modified_at VARCHAR,
    created_at      VARCHAR,
    updated_at      VARCHAR
);

CREATE TABLE IF NOT EXISTS active_processes (
    id              VARCHAR PRIMARY KEY,
    pid             INTEGER NOT NULL UNIQUE,
    name            VARCHAR NOT NULL,
    command         VARCHAR NOT NULL,
    owner           VARCHAR DEFAULT 'user',
    associated_path VARCHAR,
    listening_ports VARCHAR DEFAULT '[]',
    status          VARCHAR DEFAULT 'running',
    cpu_pct         FLOAT DEFAULT 0.0,
    mem_bytes       BIGINT DEFAULT 0,
    gpu_allocated_mb INTEGER DEFAULT 0,
    started_at      VARCHAR,
    last_seen_at    VARCHAR
);

CREATE TABLE IF NOT EXISTS memories (
    id              VARCHAR PRIMARY KEY,
    type            VARCHAR NOT NULL,
    content         VARCHAR NOT NULL,
    summary         VARCHAR,
    source_agent    VARCHAR,
    source_session  VARCHAR,
    tags            VARCHAR DEFAULT '[]',
    weight          FLOAT DEFAULT 1.0,
    pinned          BOOLEAN DEFAULT FALSE,
    status          VARCHAR DEFAULT 'active',
    embedding       VARCHAR DEFAULT '[]',
    created_at      VARCHAR,
    updated_at      VARCHAR,
    last_accessed_at VARCHAR,
    access_count    INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS changes (
    id              BIGINT PRIMARY KEY DEFAULT nextval('changes_seq'),
    type            VARCHAR NOT NULL,
    target          VARCHAR NOT NULL,
    description     VARCHAR,
    details         VARCHAR DEFAULT '{}',
    source          VARCHAR DEFAULT 'system',
    created_at      VARCHAR
);

CREATE TABLE IF NOT EXISTS links (
    id              BIGINT PRIMARY KEY DEFAULT nextval('links_seq'),
    source_type     VARCHAR NOT NULL,
    source_id       VARCHAR NOT NULL,
    target_type     VARCHAR NOT NULL,
    target_id       VARCHAR NOT NULL,
    relation        VARCHAR NOT NULL,
    confidence      FLOAT DEFAULT 1.0,
    auto_discovered BOOLEAN DEFAULT FALSE,
    line_number     INTEGER,
    snippet         VARCHAR,
    provenance      VARCHAR DEFAULT 'static_scan',
    created_at      VARCHAR
);

CREATE TABLE IF NOT EXISTS scan_jobs (
    id              VARCHAR PRIMARY KEY,
    root_path       VARCHAR NOT NULL,
    status          VARCHAR NOT NULL,
    last_path       VARCHAR,
    scanned_dirs    BIGINT DEFAULT 0,
    scanned_files   BIGINT DEFAULT 0,
    total_assets    BIGINT DEFAULT 0,
    shallow_blocked BIGINT DEFAULT 0,
    error_message   VARCHAR,
    started_at      VARCHAR,
    paused_at       VARCHAR,
    completed_at    VARCHAR,
    created_at      VARCHAR,
    updated_at      VARCHAR
);

CREATE TABLE IF NOT EXISTS fact_nodes (
    id          VARCHAR PRIMARY KEY,
    node_type   VARCHAR NOT NULL,
    name        VARCHAR NOT NULL,
    label       VARCHAR NOT NULL,
    properties  VARCHAR DEFAULT '{}',
    updated_at  VARCHAR
);

CREATE TABLE IF NOT EXISTS fact_edges (
    id          BIGINT PRIMARY KEY DEFAULT nextval('fact_edges_seq'),
    source_id   VARCHAR NOT NULL,
    target_id   VARCHAR NOT NULL,
    relation    VARCHAR NOT NULL,
    properties  VARCHAR DEFAULT '{}',
    confidence  FLOAT DEFAULT 1.0,
    updated_at  VARCHAR
);

-- 水位线追踪表（用于主库 → DuckDB 增量同步）
CREATE TABLE IF NOT EXISTS sync_watermarks (
    table_name      VARCHAR PRIMARY KEY,
    last_sync_ts    VARCHAR NOT NULL,
    synced_rows     BIGINT DEFAULT 0,
    updated_at      VARCHAR
);

-- ─── 多 Agent 协作锁 (Coordination Locks) ───────────────────────────
CREATE TABLE IF NOT EXISTS coordination_locks (
    lock_id       VARCHAR PRIMARY KEY,
    resource_type VARCHAR NOT NULL,
    resource_id   VARCHAR NOT NULL,
    scope         VARCHAR DEFAULT 'exclusive',
    holder_agent  VARCHAR NOT NULL,
    holder_pid    INTEGER,
    acquired_at   VARCHAR NOT NULL,
    expires_at    VARCHAR NOT NULL,
    metadata      VARCHAR DEFAULT '{}'
);
