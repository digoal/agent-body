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
    owner           TEXT DEFAULT 'user:digoal',
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
    node_type   TEXT NOT NULL CHECK (node_type IN ('project','tool','port','service','cron','file','authoritative_site')),
    name        TEXT NOT NULL,
    label       TEXT NOT NULL,
    properties  TEXT DEFAULT '{}',
    embedding   TEXT DEFAULT '[]',
    updated_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_fact_nodes_type ON fact_nodes(node_type);
CREATE INDEX IF NOT EXISTS idx_fact_nodes_name ON fact_nodes(name);

CREATE TABLE IF NOT EXISTS fact_edges (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    source_id   TEXT NOT NULL,
    target_id   TEXT NOT NULL,
    relation    TEXT NOT NULL CHECK (relation IN ('depends_on','listens_on','scheduled_by','reads','writes','part_of','calls','contains')),
    properties  TEXT DEFAULT '{}',
    confidence  REAL DEFAULT 1.0,
    updated_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
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

-- 预置规范规则
INSERT OR IGNORE INTO placement_rules (id, rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority) VALUES
    ('rule-1', 'shared_tools', 'script', '/root/new/shared_proj/', '{name}/', '长期沉淀的可复用脚本与程序库', '建立独立子目录，配备 README.md 与入口脚本，严禁硬编码敏感信息', 10),
    ('rule-2', 'work_projects', 'project', '/root/new/work/', '{name}/', '常规立项的工作工程与开发目录', '每个工程独立版本控制，优先采用设计文档闭环验证', 20),
    ('rule-3', 'ai_skills', 'skill', '/root/new/skills/', '{name}/', '沉淀的通用高频高质量 AI 技能', '必须包含标准 SKILL.md，保障被高频调用的稳定性', 30),
    ('rule-4', 'open_source_src', 'src', '/root/new/src/', '{name}/', '浅克隆的开源项目源码库', '克隆后进入根目录优先使用 codegraph init 生成代码地图', 40),
    ('rule-5', 'notes_and_lists', 'note', '/root/new/note/', '{name}.md', '重要笔记、知识点与权威网站清单', '好记性不如烂笔头，用于跨周期翻阅沉淀', 50),
    ('rule-6', 'cron_jobs', 'other', '/root/new/cron/', '{name}/', '定时任务的 prompt 与输出产物', '多 cron 必须先查撞点表错峰，粒度至少 30 分钟', 60),
    ('rule-7', 'temp_files', 'tmp', '/root/new/tmp/', '{name}', '可随时删除的临时文件与调试日志', '明确生命周期，定期巡检清理', 70),
    ('rule-8', 'open_source_shared', 'other', '/root/new/shared_git/', '{name}/', '沉淀的可对外开源精品项目', '高质量开源工程，结构规整文档齐全', 80);

-- 预置扫描颗粒度规则
INSERT OR IGNORE INTO scan_policies (id, path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes) VALUES
    ('policy-1', '/root/new/shared_proj/**', 'deep', -1, 1, 1, 1, '白名单: 核心工具库，深度递归到底'),
    ('policy-2', '/root/new/work/**', 'deep', -1, 1, 1, 1, '白名单: 核心工作区，深度递归到底'),
    ('policy-3', '/root/new/skills/**', 'deep', -1, 1, 1, 1, '白名单: 技能库，全量递归'),
    ('policy-4', '**/node_modules/**', 'shallow', 0, 0, 0, 0, '黑名单: 仅记录当前目录实体本身标量，严禁递归下钻'),
    ('policy-5', '**/.cache/**', 'shallow', 0, 0, 0, 0, '黑名单: 缓存目录，仅记录当前目录标量，不下钻'),
    ('policy-6', '**/.git/**', 'shallow', 0, 0, 0, 0, '黑名单: Git 内部目录，仅记录当前目录标量，不下钻'),
    ('policy-7', '/root/.ssh/**', 'shallow', 0, 0, 0, 0, '黑名单: 私密目录，仅记录目录存在，严禁提取内容'),
    ('policy-8', '/proc/**', 'shallow', 0, 0, 0, 0, '黑名单: 系统虚拟文件系统，仅记录根目录，严禁递归'),
    ('policy-9', '/root/new/src/**', 'restricted', 2, 1, 0, 0, '受限区: 开源项目库，仅索引项目入口与 README'),
    ('policy-10', '/root/new/tmp/**', 'restricted', 1, 0, 0, 0, '受限区: 临时目录，仅记录浅层文件');

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
