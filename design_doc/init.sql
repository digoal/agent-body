-- ============================================================
-- Agent Host Steward — init.sql
-- 初始化脚本 · 生成自 07-data-model.md
-- 目标: SQLite 3.35+ with FTS5 + sqlite-vec 扩展
-- 用法: sqlite3 ~/.steward/steward.db < init.sql
-- ============================================================

-- ────────────────────────────────────────────────────
-- 0. PRAGMA 设置
-- ────────────────────────────────────────────────────
PRAGMA journal_mode = WAL;
PRAGMA synchronous  = NORMAL;       -- WAL 模式下 NORMAL 足够安全
PRAGMA cache_size   = -64000;       -- 64 MB 页缓存
PRAGMA mmap_size    = 268435456;    -- 256 MB mmap
PRAGMA foreign_keys = ON;
PRAGMA busy_timeout = 5000;         -- 5 秒锁等待

-- ────────────────────────────────────────────────────
-- 1. System 域: config + migrations
-- ────────────────────────────────────────────────────
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

-- ────────────────────────────────────────────────────
-- 2. Memory 域: memories + FTS5 + Vec
-- ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS memories (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
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
    embedding_status TEXT DEFAULT 'pending' CHECK (embedding_status IN ('pending','done','failed')),
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    updated_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    last_accessed_at TEXT,
    access_count    INTEGER DEFAULT 0,
    task_status     TEXT CHECK (task_status IS NULL OR task_status IN ('todo','in_progress','done','abandoned')),
    task_progress   TEXT
);

CREATE TRIGGER IF NOT EXISTS memories_updated_at
    AFTER UPDATE ON memories
    FOR EACH ROW
BEGIN
    UPDATE memories SET updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now') WHERE id = NEW.id;
END;

CREATE INDEX IF NOT EXISTS idx_memories_type ON memories(type);
CREATE INDEX IF NOT EXISTS idx_memories_status ON memories(status);
CREATE INDEX IF NOT EXISTS idx_memories_created_at ON memories(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_memories_weight ON memories(weight DESC);
CREATE INDEX IF NOT EXISTS idx_memories_task_status ON memories(task_status) WHERE task_status IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_memories_embedding_status ON memories(embedding_status) WHERE embedding_status = 'pending';
CREATE INDEX IF NOT EXISTS idx_memories_last_accessed ON memories(last_accessed_at DESC) WHERE last_accessed_at IS NOT NULL;

-- FTS5 全文索引 (unicode61 分词; 中文需 simple tokenizer 或预分词)
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

-- sqlite-vec 向量索引 (768 维, 余弦距离)
-- 注意: 需先加载 sqlite-vec 扩展: .load vec0
CREATE VIRTUAL TABLE IF NOT EXISTS memories_vec USING vec0(
    memory_id TEXT PRIMARY KEY,
    embedding FLOAT[768] distance_metric=cosine
);

-- ────────────────────────────────────────────────────
-- 3. Asset 域: assets + FTS5 + Vec + Tags
-- ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS assets (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
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
    indicators      TEXT DEFAULT '[]',
    tags            TEXT DEFAULT '[]',
    dependencies    TEXT DEFAULT '[]',
    sensitive       INTEGER DEFAULT 0,
    status          TEXT DEFAULT 'active' CHECK (status IN ('active','archived','missing')),
    embedding_status TEXT DEFAULT 'pending',
    last_scanned_at TEXT,
    last_modified_at TEXT,
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    updated_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE TRIGGER IF NOT EXISTS assets_updated_at
    AFTER UPDATE ON assets
    FOR EACH ROW
BEGIN
    UPDATE assets SET updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now') WHERE id = NEW.id;
END;

CREATE INDEX IF NOT EXISTS idx_assets_type ON assets(type);
CREATE INDEX IF NOT EXISTS idx_assets_owner ON assets(owner);
CREATE INDEX IF NOT EXISTS idx_assets_security_level ON assets(security_level);
CREATE INDEX IF NOT EXISTS idx_assets_scan_tier ON assets(scan_tier);
CREATE INDEX IF NOT EXISTS idx_assets_status ON assets(status);
CREATE INDEX IF NOT EXISTS idx_assets_path ON assets(path);
CREATE INDEX IF NOT EXISTS idx_assets_last_modified ON assets(last_modified_at DESC);

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

CREATE VIRTUAL TABLE IF NOT EXISTS assets_vec USING vec0(
    asset_id TEXT PRIMARY KEY,
    embedding FLOAT[768] distance_metric=cosine
);

CREATE TABLE IF NOT EXISTS asset_tags (
    asset_id    TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    tag         TEXT NOT NULL,
    PRIMARY KEY (asset_id, tag)
);

CREATE INDEX IF NOT EXISTS idx_asset_tags_tag ON asset_tags(tag);

-- ────────────────────────────────────────────────────
-- 4. Placement 域: placement_rules (放置规范与建议)
-- ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS placement_rules (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    rule_name       TEXT NOT NULL UNIQUE,
    pattern_type    TEXT NOT NULL,               -- project, script, note, tmp, dataset, src, skill, other
    target_base_dir TEXT NOT NULL,               -- e.g. /root/new/shared_proj/
    naming_pattern  TEXT,                        -- e.g. "{name}/"
    description     TEXT,
    guideline       TEXT,
    priority        INTEGER DEFAULT 100,
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    updated_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_placement_type ON placement_rules(pattern_type);
CREATE INDEX IF NOT EXISTS idx_placement_priority ON placement_rules(priority);

-- ────────────────────────────────────────────────────
-- 5. Scan 策略域: scan_policies (白黑名单与颗粒度分级)
-- ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS scan_policies (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    path_pattern    TEXT NOT NULL UNIQUE,
    tier            TEXT NOT NULL CHECK (tier IN ('deep','shallow','restricted')),
    max_depth       INTEGER DEFAULT -1,          -- deep=-1, shallow=0 (仅记目录实体本身)
    enable_fts      INTEGER DEFAULT 1,
    enable_vec      INTEGER DEFAULT 1,
    enable_graph    INTEGER DEFAULT 1,
    notes           TEXT,
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_scan_policies_tier ON scan_policies(tier);

-- ────────────────────────────────────────────────────
-- 6. Dynamic 动态域: services + active_processes
-- ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS services (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name            TEXT NOT NULL,
    version         TEXT,
    port            INTEGER,
    pid             INTEGER,
    status          TEXT DEFAULT 'running' CHECK (status IN ('running','stopped','unknown')),
    binary_path     TEXT,
    config_path     TEXT,
    data_path       TEXT,
    details         TEXT DEFAULT '{}',
    last_seen_at    TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    updated_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_services_name_port ON services(name, port);

CREATE TABLE IF NOT EXISTS active_processes (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
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

-- ────────────────────────────────────────────────────
-- 7. Snapshot 域: env_snapshots + env_snapshot_items
-- ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS env_snapshots (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    label           TEXT,
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    summary         TEXT DEFAULT '{}'
);

CREATE TABLE IF NOT EXISTS env_snapshot_items (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id     TEXT NOT NULL REFERENCES env_snapshots(id) ON DELETE CASCADE,
    category        TEXT NOT NULL,
    key             TEXT NOT NULL,
    value           TEXT NOT NULL,
    UNIQUE(snapshot_id, category, key)
);

CREATE INDEX IF NOT EXISTS idx_snapshot_items_snapshot ON env_snapshot_items(snapshot_id);
CREATE INDEX IF NOT EXISTS idx_snapshot_items_category ON env_snapshot_items(category);

-- ────────────────────────────────────────────────────
-- 8. Change 域: changes (P1)
-- ────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS changes (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    type            TEXT NOT NULL CHECK (type IN (
        'file_created','file_modified','file_deleted',
        'service_started','service_stopped','service_restarted',
        'memory_added','memory_updated',
        'asset_registered','asset_changed',
        'env_diff','cron_output','manual'
    )),
    target          TEXT NOT NULL,
    description     TEXT,
    details         TEXT DEFAULT '{}',
    source          TEXT,
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX IF NOT EXISTS idx_changes_type ON changes(type);
CREATE INDEX IF NOT EXISTS idx_changes_created_at ON changes(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_changes_target ON changes(target);

-- ────────────────────────────────────────────────────
-- 9. Link 域: links (P2)
-- ────────────────────────────────────────────────────
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
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    UNIQUE(source_type, source_id, target_type, target_id, relation)
);

CREATE INDEX IF NOT EXISTS idx_links_source ON links(source_type, source_id);
CREATE INDEX IF NOT EXISTS idx_links_target ON links(target_type, target_id);
CREATE INDEX IF NOT EXISTS idx_links_relation ON links(relation);

-- ────────────────────────────────────────────────────
-- 10. 预置数据: 系统配置 + 放置规则 + 白黑名单扫描策略
-- ────────────────────────────────────────────────────
INSERT OR IGNORE INTO config(key, value, updated_at) VALUES
    ('schema_version', '2', strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    ('steward_version', '0.2.0', strftime('%Y-%m-%dT%H:%M:%f+00:00','now'));

INSERT OR IGNORE INTO migrations(version, description, applied_at) VALUES
    (1, 'initial schema', strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    (2, 'add placement, scan policies, active processes, and rich metadata', strftime('%Y-%m-%dT%H:%M:%f+00:00','now'));

-- 预置宿主机目录放置规范 (Placement Rules)
INSERT OR IGNORE INTO placement_rules (rule_name, pattern_type, target_base_dir, naming_pattern, description, guideline, priority) VALUES
    ('shared_tools', 'script', '/root/new/shared_proj/', '{name}/', '长期沉淀的可复用脚本与程序库', '建立独立子目录，配备 README.md、入口脚本与依赖说明；严禁硬编码敏感信息', 10),
    ('work_projects', 'project', '/root/new/work/', '{name}/', '常规立项的工作工程与开发目录', '每个工程独立版本控制，优先采用设计文档闭环验证', 20),
    ('ai_skills', 'skill', '/root/new/skills/', '{name}/', '沉淀的通用高频高质量 AI 技能', '必须包含标准 SKILL.md，保障被高频调用的稳定性', 30),
    ('open_source_src', 'src', '/root/new/src/', '{name}/', '浅克隆的开源项目源码库', '克隆后进入根目录优先使用 codegraph init 生成代码地图', 40),
    ('notes_and_lists', 'note', '/root/new/note/', '{name}.md', '重要笔记、知识点与权威网站清单', '好记性不如烂笔头，用于跨周期翻阅沉淀', 50),
    ('cron_jobs', 'other', '/root/new/cron/', '{name}/', '定时任务的 prompt 与输出产物', '多 cron 必须先查撞点表错峰，粒度至少 30 分钟', 60),
    ('temp_files', 'tmp', '/root/new/tmp/', '{name}', '可随时删除的临时文件与调试日志', '明确生命周期，定期巡检清理', 70),
    ('open_source_shared', 'other', '/root/new/shared_git/', '{name}/', '沉淀的可对外开源精品项目', '高质量开源工程，结构规整文档齐全', 80);

-- 预置扫描颗粒度与白黑名单策略 (Scan Policies)
INSERT OR IGNORE INTO scan_policies (path_pattern, tier, max_depth, enable_fts, enable_vec, enable_graph, notes) VALUES
    ('/root/new/shared_proj/**', 'deep', -1, 1, 1, 1, '白名单: 核心工具库，深度递归到底，全量提取标量+desc_kv+全文+向量+图谱'),
    ('/root/new/work/**', 'deep', -1, 1, 1, 1, '白名单: 核心工作区，深度递归到底，全量提取元数据'),
    ('/root/new/skills/**', 'deep', -1, 1, 1, 1, '白名单: 技能库，全量递归解析 SKILL.md 与脚本'),
    ('**/node_modules/**', 'shallow', 0, 0, 0, 0, '黑名单: 仅记录当前目录实体本身标量，严禁递归下钻，严禁向量与FTS，杜绝扫爆数据库'),
    ('**/.cache/**', 'shallow', 0, 0, 0, 0, '黑名单: 缓存目录，仅记录当前目录标量，不下钻'),
    ('**/.git/**', 'shallow', 0, 0, 0, 0, '黑名单: Git 元数据内部目录，仅记录目录标量，不下钻'),
    ('/root/.ssh/**', 'shallow', 0, 0, 0, 0, '黑名单: 私密凭证目录，仅记录目录存在与所有权，严禁提取内容'),
    ('/proc/**', 'shallow', 0, 0, 0, 0, '黑名单: 系统虚拟文件系统，仅记录根目录，严禁递归'),
    ('/root/new/src/**', 'restricted', 2, 1, 0, 0, '受限区: 开源项目库，仅索引项目入口与 README 文本，大文件跳过向量化'),
    ('/root/new/tmp/**', 'restricted', 1, 0, 0, 0, '受限区: 临时目录，仅记录浅层文件，不提取深层内容');

-- ============================================================
-- 初始化完成
-- 表总计: 17 (含 3 FTS5 虚拟表 + 2 Vec 虚拟表)
-- 触发器: 8 (3 FTS 同步 × 2 域 + 2 updated_at)
-- 索引: 23
-- ============================================================
