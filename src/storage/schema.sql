-- ============================================================
-- Agent Host Steward — 内置 Schema (Rust Edition)
-- ============================================================

PRAGMA journal_mode = WAL;
PRAGMA synchronous  = NORMAL;
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;

CREATE TABLE IF NOT EXISTS config (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE TABLE IF NOT EXISTS migrations (
    version     INTEGER PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE TABLE IF NOT EXISTS memories (
    id              TEXT PRIMARY KEY,
    type            TEXT NOT NULL CHECK (type IN ('decision','task','experience','preference','fact','note')),
    content         TEXT NOT NULL,
    summary         TEXT,
    source_agent    TEXT,
    source_session  TEXT,
    tags            TEXT DEFAULT '[]',
    weight          REAL DEFAULT 1.0 CHECK (weight >= 0 AND weight <= 1),
    pinned          INTEGER DEFAULT 0,
    sensitive       INTEGER DEFAULT 0,
    status          TEXT DEFAULT 'active' CHECK (status IN ('active','archived','outdated')),
    embedding       TEXT DEFAULT '[]',
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    updated_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    last_accessed_at TEXT,
    access_count    INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_memories_type ON memories(type);
CREATE INDEX IF NOT EXISTS idx_memories_status ON memories(status);

CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    content,
    summary,
    tags,
    content=memories,
    content_rowid=rowid,
    tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS memories_fts_insert AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, content, summary, tags)
    VALUES (NEW.rowid, NEW.content, NEW.summary, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS memories_fts_delete AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, content, summary, tags)
    VALUES ('delete', OLD.rowid, OLD.content, OLD.summary, OLD.tags);
END;

CREATE TRIGGER IF NOT EXISTS memories_fts_update AFTER UPDATE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, content, summary, tags)
    VALUES ('delete', OLD.rowid, OLD.content, OLD.summary, OLD.tags);
    INSERT INTO memories_fts(rowid, content, summary, tags)
    VALUES (NEW.rowid, NEW.content, NEW.summary, NEW.tags);
END;

CREATE TABLE IF NOT EXISTS assets (
    id              TEXT PRIMARY KEY,
    path            TEXT NOT NULL UNIQUE,
    type            TEXT NOT NULL CHECK (type IN ('directory','project','script','document','dataset','config','media','other')),
    owner           TEXT DEFAULT 'user:local',
    security_level  TEXT DEFAULT 'internal' CHECK (security_level IN ('public','internal','confidential','secret')),
    scan_tier       TEXT DEFAULT 'deep' CHECK (scan_tier IN ('deep','shallow','restricted')),
    description     TEXT,
    desc_kv         TEXT DEFAULT '{}',
    usage           TEXT,
    size_bytes      INTEGER,
    file_count      INTEGER,
    tags            TEXT DEFAULT '[]',
    status          TEXT DEFAULT 'active' CHECK (status IN ('active','archived','missing')),
    embedding       TEXT DEFAULT '[]',
    last_scanned_at TEXT,
    last_modified_at TEXT,
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    updated_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_assets_type ON assets(type);
CREATE INDEX IF NOT EXISTS idx_assets_owner ON assets(owner);
CREATE INDEX IF NOT EXISTS idx_assets_security_level ON assets(security_level);
CREATE INDEX IF NOT EXISTS idx_assets_scan_tier ON assets(scan_tier);
CREATE INDEX IF NOT EXISTS idx_assets_status ON assets(status);
CREATE INDEX IF NOT EXISTS idx_assets_path ON assets(path);

CREATE VIRTUAL TABLE IF NOT EXISTS assets_fts USING fts5(
    path,
    description,
    desc_kv,
    usage,
    tags,
    content=assets,
    content_rowid=rowid,
    tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS assets_fts_insert AFTER INSERT ON assets BEGIN
    INSERT INTO assets_fts(rowid, path, description, desc_kv, usage, tags)
    VALUES (NEW.rowid, NEW.path, NEW.description, NEW.desc_kv, NEW.usage, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS assets_fts_delete AFTER DELETE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, path, description, desc_kv, usage, tags)
    VALUES ('delete', OLD.rowid, OLD.path, OLD.description, OLD.desc_kv, OLD.usage, OLD.tags);
END;

CREATE TRIGGER IF NOT EXISTS assets_fts_update AFTER UPDATE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, path, description, desc_kv, usage, tags)
    VALUES ('delete', OLD.rowid, OLD.path, OLD.description, OLD.desc_kv, OLD.usage, OLD.tags);
    INSERT INTO assets_fts(rowid, path, description, desc_kv, usage, tags)
    VALUES (NEW.rowid, NEW.path, NEW.description, NEW.desc_kv, NEW.usage, NEW.tags);
END;

CREATE TABLE IF NOT EXISTS placement_rules (
    id              TEXT PRIMARY KEY,
    rule_name       TEXT NOT NULL UNIQUE,
    pattern_type    TEXT NOT NULL,
    target_base_dir TEXT NOT NULL,
    naming_pattern  TEXT,
    description     TEXT,
    guideline       TEXT,
    priority        INTEGER DEFAULT 100,
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    updated_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_placement_type ON placement_rules(pattern_type);
CREATE INDEX IF NOT EXISTS idx_placement_priority ON placement_rules(priority);

CREATE TABLE IF NOT EXISTS scan_policies (
    id              TEXT PRIMARY KEY,
    path_pattern    TEXT NOT NULL UNIQUE,
    tier            TEXT NOT NULL CHECK (tier IN ('deep','shallow','restricted')),
    max_depth       INTEGER DEFAULT -1,
    enable_fts      INTEGER DEFAULT 1,
    enable_vec      INTEGER DEFAULT 1,
    enable_graph    INTEGER DEFAULT 1,
    notes           TEXT,
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_scan_policies_tier ON scan_policies(tier);

CREATE TABLE IF NOT EXISTS active_processes (
    id              TEXT PRIMARY KEY,
    pid             INTEGER NOT NULL UNIQUE,
    name            TEXT NOT NULL,
    command         TEXT NOT NULL,
    owner           TEXT DEFAULT 'user',
    associated_path TEXT,
    listening_ports TEXT DEFAULT '[]',
    status          TEXT DEFAULT 'running',
    cpu_pct         REAL DEFAULT 0.0,
    mem_bytes       INTEGER DEFAULT 0,
    gpu_allocated_mb INTEGER DEFAULT 0,
    started_at      TEXT,
    last_seen_at    TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_active_processes_owner ON active_processes(owner);
CREATE INDEX IF NOT EXISTS idx_active_processes_status ON active_processes(status);

CREATE TABLE IF NOT EXISTS changes (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    type            TEXT NOT NULL,
    target          TEXT NOT NULL,
    description     TEXT,
    details         TEXT DEFAULT '{}',
    source          TEXT DEFAULT 'system',
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_changes_type ON changes(type);
CREATE INDEX IF NOT EXISTS idx_changes_created_at ON changes(created_at DESC);

CREATE TABLE IF NOT EXISTS links (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    source_type     TEXT NOT NULL CHECK (source_type IN ('memory','asset','service','change')),
    source_id       TEXT NOT NULL,
    target_type     TEXT NOT NULL CHECK (target_type IN ('memory','asset','service','change')),
    target_id       TEXT NOT NULL,
    relation        TEXT NOT NULL CHECK (relation IN (
        'references','depends_on','produced_by','related_to',
        'supersedes','contradicts','confirms'
    )),
    confidence      REAL DEFAULT 1.0 CHECK (confidence >= 0 AND confidence <= 1),
    auto_discovered INTEGER DEFAULT 0,
    line_number     INTEGER DEFAULT NULL,
    snippet         TEXT DEFAULT NULL,
    provenance      TEXT DEFAULT 'static_scan',
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    UNIQUE(source_type, source_id, target_type, target_id, relation)
);

CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_links_relation ON links(relation);

CREATE TABLE IF NOT EXISTS scan_jobs (
    id              TEXT PRIMARY KEY,
    root_path       TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('pending','running','paused','completed','failed')),
    last_path       TEXT,
    scanned_dirs    INTEGER DEFAULT 0,
    scanned_files   INTEGER DEFAULT 0,
    total_assets    INTEGER DEFAULT 0,
    shallow_blocked INTEGER DEFAULT 0,
    error_message   TEXT,
    started_at      TEXT,
    paused_at       TEXT,
    completed_at    TEXT,
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    updated_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_scan_jobs_status ON scan_jobs(status);

CREATE TABLE IF NOT EXISTS fact_nodes (
    id          TEXT PRIMARY KEY,
    node_type   TEXT NOT NULL CHECK (node_type IN ('project','tool','port','service','cron','file','authoritative_site','section','domain_term','knowhow')),
    name        TEXT NOT NULL,
    label       TEXT NOT NULL,
    properties  TEXT DEFAULT '{}',
    embedding   TEXT DEFAULT '[]',
    updated_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_fact_nodes_type ON fact_nodes(node_type);
CREATE INDEX IF NOT EXISTS idx_fact_nodes_name ON fact_nodes(name);

CREATE TABLE IF NOT EXISTS fact_edges (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id      TEXT NOT NULL,
    target_id      TEXT NOT NULL,
    relation       TEXT NOT NULL CHECK (relation IN ('depends_on','listens_on','scheduled_by','reads','writes','part_of','calls','contains','uses_tool','runs_as','documented_by','semantically_related','mentions','co_occurs_with','causes','mitigates','contrasts_with','applies_when','supersedes')),
    properties     TEXT DEFAULT '{}',
    confidence     REAL DEFAULT 1.0,
    evidence_tier  TEXT NOT NULL DEFAULT 'L1_EXPLICIT',
    weight         REAL NOT NULL DEFAULT 1.0,
    direction_kind TEXT NOT NULL DEFAULT 'forward_dependency',
    updated_at     TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    UNIQUE(source_id, target_id, relation),
    FOREIGN KEY(source_id) REFERENCES fact_nodes(id) ON DELETE CASCADE,
    FOREIGN KEY(target_id) REFERENCES fact_nodes(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_fact_edges_source ON fact_edges(source_id);
CREATE INDEX IF NOT EXISTS idx_fact_edges_target ON fact_edges(target_id);
CREATE INDEX IF NOT EXISTS idx_fact_edges_relation ON fact_edges(relation);

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
    metadata      TEXT DEFAULT '{}'
);

CREATE INDEX IF NOT EXISTS idx_locks_resource ON coordination_locks(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_locks_expires ON coordination_locks(expires_at);

-- ─── Phase 16: 生态认知目录与外部服务通讯录 ──────────────────────────
CREATE TABLE IF NOT EXISTS mcp_catalog (
    id              TEXT PRIMARY KEY,
    server_name     TEXT NOT NULL UNIQUE,
    client_source   TEXT NOT NULL,
    transport       TEXT NOT NULL DEFAULT 'stdio',
    command_or_url  TEXT NOT NULL,
    tools_json      TEXT DEFAULT '[]',
    tool_count      INTEGER DEFAULT 0,
    est_total_tokens INTEGER DEFAULT 0,
    status          TEXT DEFAULT 'active',
    last_scanned_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_mcp_catalog_source ON mcp_catalog(client_source);

CREATE TABLE IF NOT EXISTS skills_catalog (
    id              TEXT PRIMARY KEY,
    skill_name      TEXT NOT NULL UNIQUE,
    root_source     TEXT NOT NULL,
    skill_path      TEXT NOT NULL,
    description     TEXT NOT NULL,
    triggers_json   TEXT DEFAULT '[]',
    est_tokens      INTEGER DEFAULT 0,
    has_scripts     INTEGER DEFAULT 0,
    has_references  INTEGER DEFAULT 0,
    status          TEXT DEFAULT 'active',
    last_scanned_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_skills_catalog_source ON skills_catalog(root_source);

CREATE VIRTUAL TABLE IF NOT EXISTS skills_catalog_fts USING fts5(
    skill_name,
    description,
    triggers_json,
    content=skills_catalog,
    content_rowid=rowid,
    tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS skills_fts_insert AFTER INSERT ON skills_catalog BEGIN
    INSERT INTO skills_catalog_fts(rowid, skill_name, description, triggers_json)
    VALUES (NEW.rowid, NEW.skill_name, NEW.description, NEW.triggers_json);
END;

CREATE TRIGGER IF NOT EXISTS skills_fts_delete AFTER DELETE ON skills_catalog BEGIN
    INSERT INTO skills_catalog_fts(skills_catalog_fts, rowid, skill_name, description, triggers_json)
    VALUES ('delete', OLD.rowid, OLD.skill_name, OLD.description, OLD.triggers_json);
END;

CREATE TRIGGER IF NOT EXISTS skills_fts_update AFTER UPDATE ON skills_catalog BEGIN
    INSERT INTO skills_catalog_fts(skills_catalog_fts, rowid, skill_name, description, triggers_json)
    VALUES ('delete', OLD.rowid, OLD.skill_name, OLD.description, OLD.triggers_json);
    INSERT INTO skills_catalog_fts(rowid, skill_name, description, triggers_json)
    VALUES (NEW.rowid, NEW.skill_name, NEW.description, NEW.triggers_json);
END;

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
    last_checked_at TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_external_services_cat ON external_services(category);

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

CREATE INDEX IF NOT EXISTS idx_lock_wait_res ON lock_wait_queue(resource_type, resource_id, status);
CREATE INDEX IF NOT EXISTS idx_lock_wait_timeout ON lock_wait_queue(timeout_at);

CREATE VIRTUAL TABLE IF NOT EXISTS external_services_fts USING fts5(
    service_name,
    category,
    capabilities,
    usage_example,
    content=external_services,
    content_rowid=rowid,
    tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS ext_svc_fts_insert AFTER INSERT ON external_services BEGIN
    INSERT INTO external_services_fts(rowid, service_name, category, capabilities, usage_example)
    VALUES (NEW.rowid, NEW.service_name, NEW.category, NEW.capabilities, NEW.usage_example);
END;

CREATE TRIGGER IF NOT EXISTS ext_svc_fts_delete AFTER DELETE ON external_services BEGIN
    INSERT INTO external_services_fts(external_services_fts, rowid, service_name, category, capabilities, usage_example)
    VALUES ('delete', OLD.rowid, OLD.service_name, OLD.category, OLD.capabilities, OLD.usage_example);
END;

CREATE TRIGGER IF NOT EXISTS ext_svc_fts_update AFTER UPDATE ON external_services BEGIN
    INSERT INTO external_services_fts(external_services_fts, rowid, service_name, category, capabilities, usage_example)
    VALUES ('delete', OLD.rowid, OLD.service_name, OLD.category, OLD.capabilities, OLD.usage_example);
    INSERT INTO external_services_fts(rowid, service_name, category, capabilities, usage_example)
    VALUES (NEW.rowid, NEW.service_name, NEW.category, NEW.capabilities, NEW.usage_example);
END;

-- 预置数据
INSERT OR IGNORE INTO config(key, value) VALUES
    ('schema_version', '2'),
    ('steward_version', '0.2.0');

INSERT OR IGNORE INTO migrations(version, description) VALUES
    (1, 'initial schema v2');

-- 注: 初始 placement_rules 与 scan_policies 由各存储 Provider 的 init() 按宿主机可移植基准目录幂等注入。

-- ─── Phase 27: 实证 Benchmark 与生产效能存证账本 ───────────────────────
CREATE TABLE IF NOT EXISTS benchmark_runs (
    run_id                    TEXT PRIMARY KEY,
    suite_name                TEXT NOT NULL,
    git_commit                TEXT NOT NULL,
    app_version               TEXT NOT NULL,
    host_fingerprint          TEXT NOT NULL,
    workload_count            INTEGER NOT NULL,
    total_baseline_tokens     INTEGER NOT NULL,
    total_steward_tokens      INTEGER NOT NULL,
    overall_token_savings_pct REAL NOT NULL,
    overall_speedup_factor    REAL NOT NULL,
    total_hazards_intercepted INTEGER NOT NULL,
    total_noise_blocked       INTEGER NOT NULL,
    sla_passed                INTEGER NOT NULL,
    results_json              TEXT NOT NULL,
    report_sha256             TEXT NOT NULL,
    duration_ms               INTEGER NOT NULL,
    created_at                TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_benchmark_runs_created ON benchmark_runs(created_at);

CREATE TABLE IF NOT EXISTS effectiveness_ledger (
    event_id           TEXT PRIMARY KEY,
    session_id         TEXT NOT NULL DEFAULT 'default',
    source_channel     TEXT NOT NULL,
    tool_or_action     TEXT NOT NULL,
    provenance_rule    TEXT NOT NULL,
    baseline_tokens    INTEGER NOT NULL DEFAULT 0,
    steward_tokens     INTEGER NOT NULL DEFAULT 0,
    tokens_saved       INTEGER NOT NULL DEFAULT 0,
    baseline_rounds    INTEGER NOT NULL DEFAULT 1,
    steward_rounds     INTEGER NOT NULL DEFAULT 1,
    rounds_saved       INTEGER NOT NULL DEFAULT 0,
    hazard_intercepted INTEGER NOT NULL DEFAULT 0,
    noise_blocked      INTEGER NOT NULL DEFAULT 0,
    elapsed_us         INTEGER NOT NULL DEFAULT 0,
    evidence_summary   TEXT NOT NULL,
    created_at         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_ledger_created ON effectiveness_ledger(created_at);
CREATE INDEX IF NOT EXISTS idx_ledger_tool ON effectiveness_ledger(tool_or_action);
CREATE INDEX IF NOT EXISTS idx_ledger_rule ON effectiveness_ledger(provenance_rule);

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

CREATE INDEX IF NOT EXISTS idx_mesh_dialogues_thread ON mesh_dialogues(thread_id);
CREATE INDEX IF NOT EXISTS idx_mesh_dialogues_status ON mesh_dialogues(status);

CREATE TABLE IF NOT EXISTS mesh_transfers (
    transfer_id        TEXT PRIMARY KEY,
    from_node          TEXT NOT NULL,
    from_agent         TEXT NOT NULL,
    to_node            TEXT NOT NULL,
    file_name          TEXT NOT NULL,
    size_bytes         INTEGER NOT NULL,
    sha256             TEXT NOT NULL,
    inbox_path         TEXT NOT NULL,
    secret_warnings    INTEGER DEFAULT 0,
    verified_signature INTEGER DEFAULT 0,
    status             TEXT NOT NULL DEFAULT 'quarantined',
    accepted_path      TEXT,
    created_at         TEXT NOT NULL,
    updated_at         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_mesh_transfers_status ON mesh_transfers(status);

-- ─── Phase 43: 双域全链路行为审计账本 (IN_AGENT + OUT_OF_AGENT) ──────────
CREATE TABLE IF NOT EXISTS audit_events (
    event_id           TEXT PRIMARY KEY,
    trace_id           TEXT NOT NULL,
    session_id         TEXT NOT NULL DEFAULT 'system',
    domain             TEXT NOT NULL CHECK (domain IN ('IN_AGENT', 'OUT_OF_AGENT')),
    actor_kind         TEXT NOT NULL,
    actor_id           TEXT NOT NULL DEFAULT 'steward',
    source_channel     TEXT NOT NULL,
    category           TEXT NOT NULL,
    action             TEXT NOT NULL,
    target_type        TEXT NOT NULL DEFAULT 'none',
    target_id          TEXT NOT NULL DEFAULT '',
    concurrency_mode   TEXT DEFAULT NULL,
    outcome            TEXT NOT NULL CHECK (outcome IN ('success', 'warn', 'blocked', 'failed', 'recovered', 'dry_run')),
    before_after_delta TEXT NOT NULL DEFAULT '{}',
    elapsed_us         INTEGER NOT NULL DEFAULT 0,
    evidence_sha256    TEXT NOT NULL,
    created_at         TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_audit_domain_time ON audit_events(domain, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_trace ON audit_events(trace_id);
CREATE INDEX IF NOT EXISTS idx_audit_actor_cat ON audit_events(actor_kind, category, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_target ON audit_events(target_id, created_at DESC);

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
    resumed_at          TEXT DEFAULT NULL
);

CREATE INDEX IF NOT EXISTS idx_body_checkpoints_status_time
    ON body_checkpoints(status, created_at DESC);
