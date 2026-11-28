-- ============================================================
-- Agent Body (Agent 本体) — PostgreSQL Schema (Production Scale)
-- ============================================================

-- embedding 存储为可移植 JSON TEXT，无需强依赖 pgvector 扩展

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
    owner           TEXT DEFAULT 'user:local',
    security_level  TEXT DEFAULT 'internal' CHECK (security_level IN ('public','internal','confidential','secret')),
    scan_tier       TEXT DEFAULT 'deep' CHECK (scan_tier IN ('deep','shallow','restricted')),
    description     TEXT,
    desc_kv         JSONB DEFAULT '{}'::jsonb,
    usage           TEXT,
    size_bytes      BIGINT,
    file_count      BIGINT,
    tags            JSONB DEFAULT '[]'::jsonb,
    status          TEXT DEFAULT 'active' CHECK (status IN ('active','archived','missing')),
    embedding       TEXT DEFAULT '[]',
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
    sensitive       BOOLEAN DEFAULT FALSE,
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
    embedding   TEXT DEFAULT '[]',
    updated_at  TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_pg_fact_nodes_type ON fact_nodes(node_type);
CREATE INDEX IF NOT EXISTS idx_pg_fact_nodes_name ON fact_nodes(name);

CREATE TABLE IF NOT EXISTS fact_edges (
    id             BIGSERIAL PRIMARY KEY,
    source_id      TEXT NOT NULL,
    target_id      TEXT NOT NULL,
    relation       TEXT NOT NULL,
    properties     JSONB DEFAULT '{}'::jsonb,
    confidence     REAL DEFAULT 1.0,
    evidence_tier  TEXT NOT NULL DEFAULT 'L1_EXPLICIT',
    weight         REAL NOT NULL DEFAULT 1.0,
    direction_kind TEXT NOT NULL DEFAULT 'forward_dependency',
    updated_at     TIMESTAMPTZ DEFAULT NOW(),
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

-- ─── Phase 17: 协作锁等待队列 (Lock Wait Queue) ────────────────────────
CREATE TABLE IF NOT EXISTS lock_wait_queue (
    wait_id       TEXT PRIMARY KEY,
    resource_type TEXT NOT NULL,
    resource_id   TEXT NOT NULL,
    scope         TEXT NOT NULL DEFAULT 'exclusive',
    waiting_agent TEXT NOT NULL,
    priority      INTEGER DEFAULT 100,
    enqueued_at   TEXT NOT NULL,
    timeout_at    TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'waiting'
);

CREATE INDEX IF NOT EXISTS idx_pg_lock_wait_res ON lock_wait_queue(resource_type, resource_id, status);

-- ─── Phase 27: 实证 Benchmark 与生产效能存证账本 ───────────────────────
CREATE TABLE IF NOT EXISTS benchmark_runs (
    run_id                    TEXT PRIMARY KEY,
    suite_name                TEXT NOT NULL,
    git_commit                TEXT NOT NULL,
    app_version               TEXT NOT NULL,
    host_fingerprint          TEXT NOT NULL,
    workload_count            INTEGER NOT NULL,
    total_baseline_tokens     BIGINT NOT NULL,
    total_body_tokens      BIGINT NOT NULL,
    overall_token_savings_pct DOUBLE PRECISION NOT NULL,
    overall_speedup_factor    DOUBLE PRECISION NOT NULL,
    total_hazards_intercepted BIGINT NOT NULL,
    total_noise_blocked       BIGINT NOT NULL,
    sla_passed                BOOLEAN NOT NULL,
    results_json              TEXT NOT NULL,
    report_sha256             TEXT NOT NULL,
    duration_ms               BIGINT NOT NULL,
    created_at                TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_benchmark_runs_created ON benchmark_runs(created_at);

CREATE TABLE IF NOT EXISTS effectiveness_ledger (
    event_id           TEXT PRIMARY KEY,
    session_id         TEXT NOT NULL DEFAULT 'default',
    source_channel     TEXT NOT NULL,
    tool_or_action     TEXT NOT NULL,
    provenance_rule    TEXT NOT NULL,
    baseline_tokens    BIGINT NOT NULL DEFAULT 0,
    body_tokens     BIGINT NOT NULL DEFAULT 0,
    tokens_saved       BIGINT NOT NULL DEFAULT 0,
    baseline_rounds    BIGINT NOT NULL DEFAULT 1,
    body_rounds     BIGINT NOT NULL DEFAULT 1,
    rounds_saved       BIGINT NOT NULL DEFAULT 0,
    hazard_intercepted BIGINT NOT NULL DEFAULT 0,
    noise_blocked      BIGINT NOT NULL DEFAULT 0,
    elapsed_us         BIGINT NOT NULL DEFAULT 0,
    evidence_summary   TEXT NOT NULL,
    created_at         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_ledger_created ON effectiveness_ledger(created_at);
CREATE INDEX IF NOT EXISTS idx_pg_ledger_tool ON effectiveness_ledger(tool_or_action);
CREATE INDEX IF NOT EXISTS idx_pg_ledger_rule ON effectiveness_ledger(provenance_rule);

-- ─── Phase 41: 联邦网格跨机结构化对话与受控沙盒文件投递 ─────────────────
CREATE TABLE IF NOT EXISTS mesh_dialogues (
    dialogue_id        TEXT PRIMARY KEY,
    thread_id          TEXT NOT NULL,
    from_node          TEXT NOT NULL,
    from_agent         TEXT NOT NULL,
    to_node            TEXT NOT NULL,
    to_agent           TEXT NOT NULL DEFAULT '',
    intent             TEXT NOT NULL,
    content            TEXT NOT NULL,
    redaction_hits     INTEGER DEFAULT 0,
    injection_flagged  INTEGER DEFAULT 0,
    verified_signature INTEGER DEFAULT 0,
    status             TEXT NOT NULL DEFAULT 'received',
    created_at         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_mesh_dialogues_thread ON mesh_dialogues(thread_id);
CREATE INDEX IF NOT EXISTS idx_pg_mesh_dialogues_status ON mesh_dialogues(status);

CREATE TABLE IF NOT EXISTS mesh_transfers (
    transfer_id        TEXT PRIMARY KEY,
    from_node          TEXT NOT NULL,
    from_agent         TEXT NOT NULL,
    to_node            TEXT NOT NULL,
    file_name          TEXT NOT NULL,
    size_bytes         BIGINT NOT NULL,
    sha256             TEXT NOT NULL,
    inbox_path         TEXT NOT NULL,
    secret_warnings    INTEGER DEFAULT 0,
    verified_signature INTEGER DEFAULT 0,
    status             TEXT NOT NULL DEFAULT 'quarantined',
    accepted_path      TEXT,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_mesh_transfers_status ON mesh_transfers(status);

-- ─── Phase 43: 双域全链路行为审计账本 (IN_AGENT + OUT_OF_AGENT) ──────────
CREATE TABLE IF NOT EXISTS audit_events (
    event_id           TEXT PRIMARY KEY,
    trace_id           TEXT NOT NULL,
    session_id         TEXT NOT NULL DEFAULT 'system',
    domain             TEXT NOT NULL CHECK (domain IN ('IN_AGENT', 'OUT_OF_AGENT')),
    actor_kind         TEXT NOT NULL,
    actor_id           TEXT NOT NULL DEFAULT 'agent-body',
    source_channel     TEXT NOT NULL,
    category           TEXT NOT NULL,
    action             TEXT NOT NULL,
    target_type        TEXT NOT NULL DEFAULT 'none',
    target_id          TEXT NOT NULL DEFAULT '',
    concurrency_mode   TEXT DEFAULT NULL,
    outcome            TEXT NOT NULL CHECK (outcome IN ('success', 'warn', 'blocked', 'failed', 'recovered', 'dry_run')),
    before_after_delta TEXT NOT NULL DEFAULT '{}',
    elapsed_us         BIGINT NOT NULL DEFAULT 0,
    evidence_sha256    TEXT NOT NULL,
    created_at         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_pg_audit_domain_time ON audit_events(domain, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_pg_audit_trace ON audit_events(trace_id);
CREATE INDEX IF NOT EXISTS idx_pg_audit_actor_cat ON audit_events(actor_kind, category, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_pg_audit_target ON audit_events(target_id, created_at DESC);

-- ─── Phase 44: Agent 本体跨大脑连续性接力胶囊 (ContinuityCapsule) ──────────
CREATE TABLE IF NOT EXISTS body_checkpoints (
    checkpoint_id       TEXT PRIMARY KEY,
    body_id             TEXT NOT NULL,
    from_brain          TEXT NOT NULL DEFAULT 'unknown',
    session_id          TEXT NOT NULL DEFAULT '',
    goal_summary        TEXT NOT NULL,
    active_files_json   TEXT NOT NULL DEFAULT '[]',
    active_locks_json   TEXT NOT NULL DEFAULT '[]',
    verified_facts_json TEXT NOT NULL DEFAULT '[]',
    open_blockers_json  TEXT NOT NULL DEFAULT '[]',
    next_step_hint      TEXT NOT NULL DEFAULT '',
    token_budget        INTEGER NOT NULL DEFAULT 300,
    status              TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'resumed', 'expired', 'archived')),
    evidence_sha256     TEXT NOT NULL,
    created_at          TEXT NOT NULL,
    resumed_by_brain    TEXT DEFAULT NULL,
    resumed_at          TEXT DEFAULT NULL,
    tx_epoch            BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_pg_body_checkpoints_status_time
    ON body_checkpoints(status, created_at DESC);
