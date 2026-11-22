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
    embedding       VARCHAR DEFAULT '[]',
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
    embedding   VARCHAR DEFAULT '[]',
    updated_at  VARCHAR
);

CREATE TABLE IF NOT EXISTS fact_edges (
    id             BIGINT PRIMARY KEY DEFAULT nextval('fact_edges_seq'),
    source_id      VARCHAR NOT NULL,
    target_id      VARCHAR NOT NULL,
    relation       VARCHAR NOT NULL,
    properties     VARCHAR DEFAULT '{}',
    confidence     FLOAT DEFAULT 1.0,
    evidence_tier  VARCHAR DEFAULT 'L1_EXPLICIT',
    weight         FLOAT DEFAULT 1.0,
    direction_kind VARCHAR DEFAULT 'forward_dependency',
    updated_at     VARCHAR
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

-- ─── Phase 16: 生态认知目录与外部服务通讯录 ──────────────────────────
CREATE TABLE IF NOT EXISTS mcp_catalog (
    id               VARCHAR PRIMARY KEY,
    server_name      VARCHAR NOT NULL UNIQUE,
    client_source    VARCHAR NOT NULL,
    transport        VARCHAR DEFAULT 'stdio',
    command_or_url   VARCHAR NOT NULL,
    tools_json       VARCHAR DEFAULT '[]',
    tool_count       INTEGER DEFAULT 0,
    est_total_tokens INTEGER DEFAULT 0,
    status           VARCHAR DEFAULT 'active',
    last_scanned_at  VARCHAR
);

CREATE TABLE IF NOT EXISTS skills_catalog (
    id              VARCHAR PRIMARY KEY,
    skill_name      VARCHAR NOT NULL UNIQUE,
    root_source     VARCHAR NOT NULL,
    skill_path      VARCHAR NOT NULL,
    description     VARCHAR NOT NULL,
    triggers_json   VARCHAR DEFAULT '[]',
    est_tokens      INTEGER DEFAULT 0,
    has_scripts     BOOLEAN DEFAULT FALSE,
    has_references  BOOLEAN DEFAULT FALSE,
    status          VARCHAR DEFAULT 'active',
    last_scanned_at VARCHAR
);

CREATE TABLE IF NOT EXISTS external_services (
    id              VARCHAR PRIMARY KEY,
    service_name    VARCHAR NOT NULL UNIQUE,
    category        VARCHAR NOT NULL,
    endpoint        VARCHAR NOT NULL,
    protocol        VARCHAR DEFAULT 'https',
    capabilities    VARCHAR NOT NULL,
    auth_type       VARCHAR DEFAULT 'none',
    auth_env_var    VARCHAR,
    health_endpoint VARCHAR,
    usage_example   VARCHAR,
    status          VARCHAR DEFAULT 'unknown',
    last_checked_at VARCHAR
);

-- ─── Phase 17: 协作锁等待队列 (Lock Wait Queue) ────────────────────────
CREATE TABLE IF NOT EXISTS lock_wait_queue (
    wait_id       VARCHAR PRIMARY KEY,
    resource_type VARCHAR NOT NULL,
    resource_id   VARCHAR NOT NULL,
    scope         VARCHAR DEFAULT 'exclusive',
    waiting_agent VARCHAR NOT NULL,
    priority      INTEGER DEFAULT 100,
    enqueued_at   VARCHAR NOT NULL,
    timeout_at    VARCHAR NOT NULL,
    status        VARCHAR DEFAULT 'waiting'
);

-- ─── Phase 27: 实证 Benchmark 与生产效能存证账本 ───────────────────────
CREATE TABLE IF NOT EXISTS benchmark_runs (
    run_id                    VARCHAR PRIMARY KEY,
    suite_name                VARCHAR NOT NULL,
    git_commit                VARCHAR NOT NULL,
    app_version               VARCHAR NOT NULL,
    host_fingerprint          VARCHAR NOT NULL,
    workload_count            INTEGER NOT NULL,
    total_baseline_tokens     BIGINT NOT NULL,
    total_steward_tokens      BIGINT NOT NULL,
    overall_token_savings_pct DOUBLE NOT NULL,
    overall_speedup_factor    DOUBLE NOT NULL,
    total_hazards_intercepted BIGINT NOT NULL,
    total_noise_blocked       BIGINT NOT NULL,
    sla_passed                BOOLEAN NOT NULL,
    results_json              VARCHAR NOT NULL,
    report_sha256             VARCHAR NOT NULL,
    duration_ms               BIGINT NOT NULL,
    created_at                VARCHAR NOT NULL
);

CREATE TABLE IF NOT EXISTS effectiveness_ledger (
    event_id           VARCHAR PRIMARY KEY,
    session_id         VARCHAR DEFAULT 'default',
    source_channel     VARCHAR NOT NULL,
    tool_or_action     VARCHAR NOT NULL,
    provenance_rule    VARCHAR NOT NULL,
    baseline_tokens    BIGINT DEFAULT 0,
    steward_tokens     BIGINT DEFAULT 0,
    tokens_saved       BIGINT DEFAULT 0,
    baseline_rounds    BIGINT DEFAULT 1,
    steward_rounds     BIGINT DEFAULT 1,
    rounds_saved       BIGINT DEFAULT 0,
    hazard_intercepted BIGINT DEFAULT 0,
    noise_blocked      BIGINT DEFAULT 0,
    elapsed_us         BIGINT DEFAULT 0,
    evidence_summary   VARCHAR NOT NULL,
    created_at         VARCHAR NOT NULL
);

-- ─── Phase 41: 联邦网格跨机结构化对话与受控沙盒文件投递 ─────────────────
CREATE TABLE IF NOT EXISTS mesh_dialogues (
    dialogue_id        VARCHAR PRIMARY KEY,
    thread_id          VARCHAR NOT NULL,
    from_node          VARCHAR NOT NULL,
    from_agent         VARCHAR NOT NULL,
    to_node            VARCHAR NOT NULL,
    to_agent           VARCHAR DEFAULT '',
    intent             VARCHAR NOT NULL,
    content            VARCHAR NOT NULL,
    redaction_hits     INTEGER DEFAULT 0,
    injection_flagged  INTEGER DEFAULT 0,
    verified_signature INTEGER DEFAULT 0,
    status             VARCHAR DEFAULT 'received',
    created_at         VARCHAR NOT NULL
);

CREATE TABLE IF NOT EXISTS mesh_transfers (
    transfer_id        VARCHAR PRIMARY KEY,
    from_node          VARCHAR NOT NULL,
    from_agent         VARCHAR NOT NULL,
    to_node            VARCHAR NOT NULL,
    file_name          VARCHAR NOT NULL,
    size_bytes         BIGINT NOT NULL,
    sha256             VARCHAR NOT NULL,
    inbox_path         VARCHAR NOT NULL,
    secret_warnings    INTEGER DEFAULT 0,
    verified_signature INTEGER DEFAULT 0,
    status             VARCHAR DEFAULT 'quarantined',
    accepted_path      VARCHAR,
    created_at         VARCHAR NOT NULL,
    updated_at         VARCHAR NOT NULL
);

-- ─── Phase 43: 双域全链路行为审计账本 (IN_AGENT + OUT_OF_AGENT) ──────────
CREATE TABLE IF NOT EXISTS audit_events (
    event_id           VARCHAR PRIMARY KEY,
    trace_id           VARCHAR NOT NULL,
    session_id         VARCHAR DEFAULT 'system',
    domain             VARCHAR NOT NULL,
    actor_kind         VARCHAR NOT NULL,
    actor_id           VARCHAR DEFAULT 'steward',
    source_channel     VARCHAR NOT NULL,
    category           VARCHAR NOT NULL,
    action             VARCHAR NOT NULL,
    target_type        VARCHAR DEFAULT 'none',
    target_id          VARCHAR DEFAULT '',
    concurrency_mode   VARCHAR,
    outcome            VARCHAR NOT NULL,
    before_after_delta VARCHAR DEFAULT '{}',
    elapsed_us         BIGINT DEFAULT 0,
    evidence_sha256    VARCHAR NOT NULL,
    created_at         VARCHAR NOT NULL
);

-- ─── Phase 44: Agent 本体跨大脑连续性接力胶囊 (ContinuityCapsule) ──────────
CREATE TABLE IF NOT EXISTS body_checkpoints (
    checkpoint_id       VARCHAR PRIMARY KEY,
    body_id             VARCHAR NOT NULL,
    from_brain          VARCHAR DEFAULT 'unknown',
    session_id          VARCHAR DEFAULT '',
    goal_summary        VARCHAR NOT NULL,
    active_files_json   VARCHAR DEFAULT '[]',
    active_locks_json   VARCHAR DEFAULT '[]',
    verified_facts_json VARCHAR DEFAULT '[]',
    open_blockers_json  VARCHAR DEFAULT '[]',
    next_step_hint      VARCHAR DEFAULT '',
    token_budget        INTEGER DEFAULT 300,
    status              VARCHAR DEFAULT 'active',
    evidence_sha256     VARCHAR NOT NULL,
    created_at          VARCHAR NOT NULL,
    resumed_by_brain    VARCHAR,
    resumed_at          VARCHAR
);
