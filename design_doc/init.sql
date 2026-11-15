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
    type            TEXT NOT NULL CHECK (type IN ('project','script','document','dataset','config','media','other')),
    description     TEXT,
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
CREATE INDEX IF NOT EXISTS idx_assets_status ON assets(status);
CREATE INDEX IF NOT EXISTS idx_assets_path ON assets(path);
CREATE INDEX IF NOT EXISTS idx_assets_last_modified ON assets(last_modified_at DESC);

CREATE VIRTUAL TABLE IF NOT EXISTS assets_fts USING fts5(
    path,
    description,
    usage,
    tags,
    content=assets,
    content_rowid=rowid,
    tokenize='unicode61 remove_diacritics 2'
);

CREATE TRIGGER IF NOT EXISTS assets_fts_insert AFTER INSERT ON assets BEGIN
    INSERT INTO assets_fts(rowid, path, description, usage, tags)
    VALUES (NEW.rowid, NEW.path, NEW.description, NEW.usage, NEW.tags);
END;

CREATE TRIGGER IF NOT EXISTS assets_fts_delete AFTER DELETE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, path, description, usage, tags)
    VALUES ('delete', OLD.rowid, OLD.path, OLD.description, OLD.usage, OLD.tags);
END;

CREATE TRIGGER IF NOT EXISTS assets_fts_update AFTER UPDATE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, path, description, usage, tags)
    VALUES ('delete', OLD.rowid, OLD.path, OLD.description, OLD.usage, OLD.tags);
    INSERT INTO assets_fts(rowid, path, description, usage, tags)
    VALUES (NEW.rowid, NEW.path, NEW.description, NEW.usage, NEW.tags);
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
-- 4. Env 域: services + env_snapshots + env_snapshot_items
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
-- 5. Change 域: changes (P1, 但 v1 建表)
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
-- 6. Link 域: links (P2, 但 v1 建表)
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
-- 7. 初始配置数据
-- ────────────────────────────────────────────────────
INSERT OR IGNORE INTO config(key, value, updated_at) VALUES
    ('schema_version', '1', strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    ('steward_version', '0.1.0', strftime('%Y-%m-%dT%H:%M:%f+00:00','now'));

INSERT OR IGNORE INTO migrations(version, description, applied_at) VALUES
    (1, 'initial schema', strftime('%Y-%m-%dT%H:%M:%f+00:00','now'));

-- ============================================================
-- 初始化完成
-- 表总计: 14 (含 3 FTS5 虚拟表 + 2 Vec 虚拟表)
-- 触发器: 8 (3 FTS 同步 × 2 域 + 2 updated_at)
-- 索引: 15 (含 1 唯一索引)
--
-- 使用前请确保:
--   1. SQLite >= 3.35 (编译时启用 FTS5)
--   2. 加载 sqlite-vec 扩展: .load vec0
-- ============================================================
