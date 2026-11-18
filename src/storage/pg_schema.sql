-- ============================================================
-- Agent Host Steward — PostgreSQL Schema (Production Scale)
-- ============================================================

CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS placement_rules (
    id              TEXT PRIMARY KEY,
    rule_name       TEXT NOT NULL UNIQUE,
    pattern_type    TEXT NOT NULL,
    target_base_dir TEXT NOT NULL,
    naming_pattern  TEXT,
    description     TEXT,
    guideline       TEXT,
    priority        INTEGER DEFAULT 100,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pg_placement_type ON placement_rules(pattern_type);
CREATE INDEX IF NOT EXISTS idx_pg_placement_priority ON placement_rules(priority);

CREATE TABLE IF NOT EXISTS scan_policies (
    id              TEXT PRIMARY KEY,
    path_pattern    TEXT NOT NULL UNIQUE,
    tier            TEXT NOT NULL CHECK (tier IN ('deep','shallow','restricted')),
    max_depth       INTEGER DEFAULT -1,
    enable_fts      BOOLEAN DEFAULT TRUE,
    enable_vec      BOOLEAN DEFAULT TRUE,
    enable_graph    BOOLEAN DEFAULT TRUE,
    notes           TEXT,
    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pg_scan_policies_tier ON scan_policies(tier);

CREATE TABLE IF NOT EXISTS assets (
    id              TEXT PRIMARY KEY,
    path            TEXT NOT NULL UNIQUE,
    type            TEXT NOT NULL CHECK (type IN ('directory','project','script','document','dataset','config','media','other')),
    owner           TEXT DEFAULT 'user:digoal',
    security_level  TEXT DEFAULT 'internal' CHECK (security_level IN ('public','internal','confidential','secret')),
    scan_tier       TEXT DEFAULT 'deep' CHECK (scan_tier IN ('deep','shallow','restricted')),
    description     TEXT,
    desc_kv         JSONB DEFAULT '{}'::jsonb,
    usage           TEXT,
    size_bytes      BIGINT,
    file_count      BIGINT,
    tags            JSONB DEFAULT '[]'::jsonb,
    status          TEXT DEFAULT 'active' CHECK (status IN ('active','archived','missing')),
    last_scanned_at TIMESTAMPTZ,
    last_modified_at TIMESTAMPTZ,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),
    fts_doc         TSVECTOR GENERATED ALWAYS AS (
        to_tsvector('simple', coalesce(path, '') || ' ' || coalesce(description, '') || ' ' || coalesce(usage, ''))
    ) STORED
);

CREATE INDEX IF NOT EXISTS idx_pg_assets_fts ON assets USING GIN(fts_doc);
CREATE INDEX IF NOT EXISTS idx_pg_assets_type ON assets(type);
CREATE INDEX IF NOT EXISTS idx_pg_assets_scan_tier ON assets(scan_tier);

CREATE TABLE IF NOT EXISTS active_processes (
    id              TEXT PRIMARY KEY,
    pid             INTEGER NOT NULL UNIQUE,
    name            TEXT NOT NULL,
    command         TEXT NOT NULL,
    owner           TEXT DEFAULT 'user',
    associated_path TEXT,
    listening_ports JSONB DEFAULT '[]'::jsonb,
    status          TEXT DEFAULT 'running',
    cpu_pct         REAL DEFAULT 0.0,
    mem_bytes       BIGINT DEFAULT 0,
    gpu_allocated_mb INTEGER DEFAULT 0,
    started_at      TEXT,
    last_seen_at    TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS memories (
    id              TEXT PRIMARY KEY,
    type            TEXT NOT NULL CHECK (type IN ('decision','task','experience','preference','fact','note')),
    content         TEXT NOT NULL,
    summary         TEXT,
    source_agent    TEXT,
    source_session  TEXT,
    tags            JSONB DEFAULT '[]'::jsonb,
    weight          REAL DEFAULT 1.0 CHECK (weight >= 0 AND weight <= 1),
    pinned          BOOLEAN DEFAULT FALSE,
    status          TEXT DEFAULT 'active' CHECK (status IN ('active','archived','outdated')),
    embedding       TEXT DEFAULT '[]',
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW(),
    last_accessed_at TIMESTAMPTZ,
    access_count    INTEGER DEFAULT 0,
    fts_doc         TSVECTOR GENERATED ALWAYS AS (
        to_tsvector('simple', coalesce(content, '') || ' ' || coalesce(summary, ''))
    ) STORED
);

CREATE INDEX IF NOT EXISTS idx_pg_memories_fts ON memories USING GIN(fts_doc);
CREATE INDEX IF NOT EXISTS idx_pg_memories_status ON memories(status);

CREATE TABLE IF NOT EXISTS changes (
    id              BIGSERIAL PRIMARY KEY,
    type            TEXT NOT NULL,
    target          TEXT NOT NULL,
    description     TEXT,
    details         JSONB DEFAULT '{}'::jsonb,
    source          TEXT DEFAULT 'system',
    created_at      TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pg_changes_type ON changes(type);
CREATE INDEX IF NOT EXISTS idx_pg_changes_created_at ON changes(created_at DESC);

CREATE TABLE IF NOT EXISTS links (
    id              BIGSERIAL PRIMARY KEY,
    source_type     TEXT NOT NULL,
    source_id       TEXT NOT NULL,
    target_type     TEXT NOT NULL,
    target_id       TEXT NOT NULL,
    relation        TEXT NOT NULL,
    confidence      REAL DEFAULT 1.0,
    auto_discovered BOOLEAN DEFAULT FALSE,
    line_number     INTEGER,
    snippet         TEXT,
    provenance      TEXT DEFAULT 'static_scan',
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(source_type, source_id, target_type, target_id, relation)
);

CREATE INDEX IF NOT EXISTS idx_pg_links_source ON links(source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_pg_links_target ON links(target_type, target_id);

CREATE TABLE IF NOT EXISTS scan_jobs (
    id              TEXT PRIMARY KEY,
    root_path       TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('pending','running','paused','completed','failed')),
    last_path       TEXT,
    scanned_dirs    BIGINT DEFAULT 0,
    scanned_files   BIGINT DEFAULT 0,
    total_assets    BIGINT DEFAULT 0,
    shallow_blocked BIGINT DEFAULT 0,
    error_message   TEXT,
    started_at      TEXT,
    paused_at       TEXT,
    completed_at    TEXT,
    created_at      TEXT,
    updated_at      TEXT
);

CREATE INDEX IF NOT EXISTS idx_pg_scan_jobs_status ON scan_jobs(status);

CREATE TABLE IF NOT EXISTS fact_nodes (
    id          TEXT PRIMARY KEY,
    node_type   TEXT NOT NULL,
    name        TEXT NOT NULL,
    label       TEXT NOT NULL,
    properties  JSONB DEFAULT '{}'::jsonb,
    updated_at  TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pg_fact_nodes_type ON fact_nodes(node_type);
CREATE INDEX IF NOT EXISTS idx_pg_fact_nodes_name ON fact_nodes(name);

CREATE TABLE IF NOT EXISTS fact_edges (
    id          BIGSERIAL PRIMARY KEY,
    source_id   TEXT NOT NULL,
    target_id   TEXT NOT NULL,
    relation    TEXT NOT NULL,
    properties  JSONB DEFAULT '{}'::jsonb,
    confidence  REAL DEFAULT 1.0,
    updated_at  TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(source_id, target_id, relation)
);

CREATE INDEX IF NOT EXISTS idx_pg_fact_edges_source ON fact_edges(source_id);
CREATE INDEX IF NOT EXISTS idx_pg_fact_edges_target ON fact_edges(target_id);

-- ─── 多 Agent 协作锁 (Coordination Locks) ───────────────────────────
CREATE TABLE IF NOT EXISTS coordination_locks (
    lock_id       TEXT PRIMARY KEY,
    resource_type TEXT NOT NULL,
    resource_id   TEXT NOT NULL,
    scope         TEXT NOT NULL DEFAULT 'exclusive',
    holder_agent  TEXT NOT NULL,
    holder_pid    INTEGER,
    acquired_at   TEXT NOT NULL,
    expires_at    TEXT NOT NULL,
    metadata      JSONB DEFAULT '{}'::jsonb
);

CREATE INDEX IF NOT EXISTS idx_pg_locks_resource ON coordination_locks(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_pg_locks_expires ON coordination_locks(expires_at);

-- ─── Phase 16: 生态认知目录与外部服务通讯录 ──────────────────────────
CREATE TABLE IF NOT EXISTS mcp_catalog (
    id               TEXT PRIMARY KEY,
    server_name      TEXT NOT NULL UNIQUE,
    client_source    TEXT NOT NULL,
    transport        TEXT NOT NULL DEFAULT 'stdio',
    command_or_url   TEXT NOT NULL,
    tools_json       JSONB DEFAULT '[]'::jsonb,
    tool_count       INTEGER DEFAULT 0,
    est_total_tokens INTEGER DEFAULT 0,
    status           TEXT DEFAULT 'active',
    last_scanned_at  TEXT
);

CREATE TABLE IF NOT EXISTS skills_catalog (
    id              TEXT PRIMARY KEY,
    skill_name      TEXT NOT NULL UNIQUE,
    root_source     TEXT NOT NULL,
    skill_path      TEXT NOT NULL,
    description     TEXT NOT NULL,
    triggers_json   JSONB DEFAULT '[]'::jsonb,
    est_tokens      INTEGER DEFAULT 0,
    has_scripts     BOOLEAN DEFAULT FALSE,
    has_references  BOOLEAN DEFAULT FALSE,
    status          TEXT DEFAULT 'active',
    last_scanned_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_pg_skills_fts ON skills_catalog USING GIN(to_tsvector('simple', skill_name || ' ' || description));

CREATE TABLE IF NOT EXISTS external_services (
    id              TEXT PRIMARY KEY,
    service_name    TEXT NOT NULL UNIQUE,
    category        TEXT NOT NULL,
    endpoint        TEXT NOT NULL,
    protocol        TEXT NOT NULL DEFAULT 'https',
    capabilities    TEXT NOT NULL,
    auth_type       TEXT DEFAULT 'none',
    auth_env_var    TEXT,
    health_endpoint TEXT,
    usage_example   TEXT,
    status          TEXT DEFAULT 'unknown',
    last_checked_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_pg_ext_svc_fts ON external_services USING GIN(to_tsvector('simple', service_name || ' ' || category || ' ' || capabilities));
