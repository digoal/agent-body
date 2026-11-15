# 07 — 数据模型（Data Model）

> Agent Host Steward 产品设计 · 交付件 7/7
> 版本: v1.0 · 更新: 2026-11-15

---

## 一、总览

steward 的全部持久化数据存放在单个 SQLite 数据库文件中（`~/.steward/steward.db`），配合 FTS5 全文索引和 sqlite-vec 向量索引。

核心表共 12 张，分属 5 个域：

```
┌────────── Memory 域 ──────────┐   ┌────────── Asset 域 ───────────┐
│  memories                     │   │  assets                       │
│  memories_fts  (FTS5 虚拟表)  │   │  assets_fts   (FTS5 虚拟表)   │
│  memories_vec  (vec 虚拟表)   │   │  assets_vec   (vec 虚拟表)    │
└───────────────────────────────┘   │  asset_tags                   │
                                    └───────────────────────────────┘

┌────────── Env 域 ─────────────┐   ┌────────── Change 域 (P1) ────┐
│  services                     │   │  changes                      │
│  env_snapshots                │   └───────────────────────────────┘
│  env_snapshot_items           │
└───────────────────────────────┘   ┌────────── Link 域 (P2) ──────┐
                                    │  links                        │
┌────────── System 域 ──────────┐   └───────────────────────────────┘
│  config                       │
│  migrations                   │
└───────────────────────────────┘
```

---

## 二、DDL 定义

### 2.1 memories — 记忆表

```sql
CREATE TABLE memories (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    type            TEXT NOT NULL CHECK (type IN ('decision','task','experience','preference','fact','note')),
    content         TEXT NOT NULL,
    summary         TEXT,                       -- 一句话摘要
    source_agent    TEXT,                       -- 来源 Agent 名称
    source_session  TEXT,                       -- 来源会话 ID
    tags            TEXT DEFAULT '[]',          -- JSON 数组
    weight          REAL DEFAULT 1.0 CHECK (weight >= 0 AND weight <= 1),
    pinned          INTEGER DEFAULT 0,          -- 0/1
    sensitive       INTEGER DEFAULT 0,          -- 0/1
    status          TEXT DEFAULT 'active' CHECK (status IN ('active','archived','outdated')),
    embedding_status TEXT DEFAULT 'pending' CHECK (embedding_status IN ('pending','done','failed')),
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    updated_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),

    -- 引用追踪（衰减/强化策略依赖）
    last_accessed_at TEXT,                      -- 最后被引用的时间
    access_count    INTEGER DEFAULT 0,          -- 被引用次数

    -- 任务类型专用字段
    task_status     TEXT CHECK (task_status IS NULL OR task_status IN ('todo','in_progress','done','abandoned')),
    task_progress   TEXT                        -- 进度描述
);

-- 更新时自动刷新 updated_at
CREATE TRIGGER memories_updated_at
    AFTER UPDATE ON memories
    FOR EACH ROW
BEGIN
    UPDATE memories SET updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now') WHERE id = NEW.id;
END;

-- 索引
CREATE INDEX idx_memories_type ON memories(type);
CREATE INDEX idx_memories_status ON memories(status);
CREATE INDEX idx_memories_created_at ON memories(created_at DESC);
CREATE INDEX idx_memories_weight ON memories(weight DESC);
CREATE INDEX idx_memories_task_status ON memories(task_status) WHERE task_status IS NOT NULL;
CREATE INDEX idx_memories_embedding_status ON memories(embedding_status) WHERE embedding_status = 'pending';
CREATE INDEX idx_memories_last_accessed ON memories(last_accessed_at DESC) WHERE last_accessed_at IS NOT NULL;
```

### 2.2 memories_fts — 全文搜索索引

```sql
-- FTS5 虚拟表，使用 porter + unicode61 分词器
-- 中文使用 simple tokenizer 或预处理分词后入库
CREATE VIRTUAL TABLE memories_fts USING fts5(
    content,
    summary,
    tags,
    content=memories,
    content_rowid=rowid,
    tokenize='unicode61 remove_diacritics 2'
);

-- 触发器：memories 增删改时同步 FTS
CREATE TRIGGER memories_fts_insert AFTER INSERT ON memories BEGIN
    INSERT INTO memories_fts(rowid, content, summary, tags)
    VALUES (NEW.rowid, NEW.content, NEW.summary, NEW.tags);
END;

CREATE TRIGGER memories_fts_delete AFTER DELETE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, content, summary, tags)
    VALUES ('delete', OLD.rowid, OLD.content, OLD.summary, OLD.tags);
END;

CREATE TRIGGER memories_fts_update AFTER UPDATE ON memories BEGIN
    INSERT INTO memories_fts(memories_fts, rowid, content, summary, tags)
    VALUES ('delete', OLD.rowid, OLD.content, OLD.summary, OLD.tags);
    INSERT INTO memories_fts(rowid, content, summary, tags)
    VALUES (NEW.rowid, NEW.content, NEW.summary, NEW.tags);
END;
```

### 2.3 memories_vec — 向量索引

```sql
-- sqlite-vec 虚拟表 (768 维, 余弦距离)
CREATE VIRTUAL TABLE memories_vec USING vec0(
    memory_id TEXT PRIMARY KEY,
    embedding FLOAT[768] distance_metric=cosine
);
```

写入流程：
1. 记忆写入 `memories` 表，`embedding_status = 'pending'`
2. 异步调用 Ollama embedding API
3. 得到向量后写入 `memories_vec`，更新 `embedding_status = 'done'`

查询流程：
```sql
-- 语义搜索
SELECT m.*, v.distance
FROM memories_vec v
JOIN memories m ON m.id = v.memory_id
WHERE v.embedding MATCH :query_vector
  AND k = :limit
ORDER BY v.distance;
```

---

### 2.4 assets — 资产表

```sql
CREATE TABLE assets (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    path            TEXT NOT NULL UNIQUE,        -- 绝对路径
    type            TEXT NOT NULL CHECK (type IN ('project','script','document','dataset','config','media','other')),
    description     TEXT,
    usage           TEXT,                        -- 使用方法
    size_bytes      INTEGER,                     -- 文件/目录大小
    file_count      INTEGER,                     -- 目录内文件数
    indicators      TEXT DEFAULT '[]',           -- JSON: 识别标记 [".git", "Cargo.toml"]
    tags            TEXT DEFAULT '[]',           -- JSON 数组
    dependencies    TEXT DEFAULT '[]',           -- JSON: 依赖的其他资产 ID
    sensitive       INTEGER DEFAULT 0,
    status          TEXT DEFAULT 'active' CHECK (status IN ('active','archived','missing')),
    embedding_status TEXT DEFAULT 'pending',
    last_scanned_at TEXT,                        -- 上次扫描时间
    last_modified_at TEXT,                       -- 文件系统上的最后修改时间
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    updated_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE TRIGGER assets_updated_at
    AFTER UPDATE ON assets
    FOR EACH ROW
BEGIN
    UPDATE assets SET updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00','now') WHERE id = NEW.id;
END;

CREATE INDEX idx_assets_type ON assets(type);
CREATE INDEX idx_assets_status ON assets(status);
CREATE INDEX idx_assets_path ON assets(path);
CREATE INDEX idx_assets_last_modified ON assets(last_modified_at DESC);
```

### 2.5 assets_fts — 资产全文搜索

```sql
CREATE VIRTUAL TABLE assets_fts USING fts5(
    path,
    description,
    usage,
    tags,
    content=assets,
    content_rowid=rowid,
    tokenize='unicode61 remove_diacritics 2'
);

-- 同步触发器 (同 memories_fts 模式)
CREATE TRIGGER assets_fts_insert AFTER INSERT ON assets BEGIN
    INSERT INTO assets_fts(rowid, path, description, usage, tags)
    VALUES (NEW.rowid, NEW.path, NEW.description, NEW.usage, NEW.tags);
END;

CREATE TRIGGER assets_fts_delete AFTER DELETE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, path, description, usage, tags)
    VALUES ('delete', OLD.rowid, OLD.path, OLD.description, OLD.usage, OLD.tags);
END;

CREATE TRIGGER assets_fts_update AFTER UPDATE ON assets BEGIN
    INSERT INTO assets_fts(assets_fts, rowid, path, description, usage, tags)
    VALUES ('delete', OLD.rowid, OLD.path, OLD.description, OLD.usage, OLD.tags);
    INSERT INTO assets_fts(rowid, path, description, usage, tags)
    VALUES (NEW.rowid, NEW.path, NEW.description, NEW.usage, NEW.tags);
END;
```

### 2.6 assets_vec — 资产向量索引

```sql
-- sqlite-vec 虚拟表 (768 维, 余弦距离)
-- 基于 asset 的 description + usage + tags 生成 embedding
CREATE VIRTUAL TABLE assets_vec USING vec0(
    asset_id TEXT PRIMARY KEY,
    embedding FLOAT[768] distance_metric=cosine
);
```

写入流程与 memories_vec 一致：写入 assets 表时标记 `embedding_status = 'pending'`，异步 embedding 完成后写入 assets_vec 并更新状态为 `'done'`。

---

### 2.7 asset_tags — 标签表（方便按标签查询）

```sql
CREATE TABLE asset_tags (
    asset_id    TEXT NOT NULL REFERENCES assets(id) ON DELETE CASCADE,
    tag         TEXT NOT NULL,
    PRIMARY KEY (asset_id, tag)
);

CREATE INDEX idx_asset_tags_tag ON asset_tags(tag);
```

> 注：memories 的标签直接用 JSON 数组存 `tags` 字段，通过 FTS 搜索；assets 多加一张 tag 表是因为资产数量通常比记忆大一个量级，按标签精确过滤是高频操作。

---

### 2.7 services — 服务表

```sql
CREATE TABLE services (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    name            TEXT NOT NULL,               -- 服务名称 (PostgreSQL, Redis, ...)
    version         TEXT,
    port            INTEGER,
    pid             INTEGER,
    status          TEXT DEFAULT 'running' CHECK (status IN ('running','stopped','unknown')),
    binary_path     TEXT,                        -- 可执行文件路径
    config_path     TEXT,                        -- 配置文件路径
    data_path       TEXT,                        -- 数据目录
    details         TEXT DEFAULT '{}',           -- JSON: 服务特有的详细信息
    last_seen_at    TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    updated_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE UNIQUE INDEX idx_services_name_port ON services(name, port);
```

---

### 2.8 env_snapshots / env_snapshot_items — 环境快照

```sql
CREATE TABLE env_snapshots (
    id              TEXT PRIMARY KEY DEFAULT (lower(hex(randomblob(16)))),
    label           TEXT,                        -- 可选标签
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    summary         TEXT DEFAULT '{}'            -- JSON: 各维度统计数
);

CREATE TABLE env_snapshot_items (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    snapshot_id     TEXT NOT NULL REFERENCES env_snapshots(id) ON DELETE CASCADE,
    category        TEXT NOT NULL,               -- system, service, software, network, cron, env_var
    key             TEXT NOT NULL,               -- 唯一标识 (如 service:postgresql, software:python3)
    value           TEXT NOT NULL,               -- JSON: 该项的完整信息
    UNIQUE(snapshot_id, category, key)
);

CREATE INDEX idx_snapshot_items_snapshot ON env_snapshot_items(snapshot_id);
CREATE INDEX idx_snapshot_items_category ON env_snapshot_items(category);
```

快照对比查询（P1）：
```sql
-- 找出两个快照之间的差异
-- 注: SQLite < 3.39 不支持 FULL OUTER JOIN, 使用 LEFT JOIN + UNION 模式兼容

-- 在 B 中有对应项的: changed 或 unchanged
SELECT
    a.category,
    a.key,
    CASE
        WHEN a.value != b.value THEN 'changed'
        ELSE NULL
    END AS diff_type,
    a.value AS old_value,
    b.value AS new_value
FROM env_snapshot_items a
LEFT JOIN env_snapshot_items b
    ON a.category = b.category AND a.key = b.key AND b.snapshot_id = :snapshot_b
WHERE a.snapshot_id = :snapshot_a
  AND (b.id IS NULL OR a.value != b.value)

UNION ALL

-- 仅在 B 中存在的: added
SELECT
    b.category,
    b.key,
    'added' AS diff_type,
    NULL AS old_value,
    b.value AS new_value
FROM env_snapshot_items b
LEFT JOIN env_snapshot_items a
    ON b.category = a.category AND b.key = a.key AND a.snapshot_id = :snapshot_a
WHERE b.snapshot_id = :snapshot_b
  AND a.id IS NULL;

-- 第一段 LEFT JOIN 中 b.id IS NULL 的行即为 'removed' (old_value 有值, new_value 为 NULL)
```

---

### 2.9 changes — 变更表（P1）

```sql
CREATE TABLE changes (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    type            TEXT NOT NULL CHECK (type IN (
        'file_created','file_modified','file_deleted',
        'service_started','service_stopped','service_restarted',
        'memory_added','memory_updated',
        'asset_registered','asset_changed',
        'env_diff','cron_output','manual'
    )),
    target          TEXT NOT NULL,               -- 变更对象 (文件路径、服务名、记忆ID等)
    description     TEXT,
    details         TEXT DEFAULT '{}',           -- JSON: 变更详情
    source          TEXT,                        -- 变更来源 (watch, scheduler, api, ...)
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

CREATE INDEX idx_changes_type ON changes(type);
CREATE INDEX idx_changes_created_at ON changes(created_at DESC);
CREATE INDEX idx_changes_target ON changes(target);

-- 自动清理：只保留 90 天的变更记录
-- 由 scheduler 定期执行:
-- DELETE FROM changes WHERE created_at < datetime('now', '-90 days');
```

---

### 2.10 links — 关联表（P2）

```sql
CREATE TABLE links (
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
    auto_discovered INTEGER DEFAULT 0,          -- 0=手动, 1=自动发现
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now')),
    UNIQUE(source_type, source_id, target_type, target_id, relation)
);

CREATE INDEX idx_links_source ON links(source_type, source_id);
CREATE INDEX idx_links_target ON links(target_type, target_id);
CREATE INDEX idx_links_relation ON links(relation);
```

关联查询示例：
```sql
-- 查找与某条记忆相关的所有实体
SELECT l.relation, l.target_type, l.target_id,
       CASE l.target_type
           WHEN 'memory' THEN (SELECT summary FROM memories WHERE id = l.target_id)
           WHEN 'asset' THEN (SELECT path || ': ' || description FROM assets WHERE id = l.target_id)
           WHEN 'service' THEN (SELECT name || ' ' || version FROM services WHERE id = l.target_id)
       END AS target_desc
FROM links l
WHERE l.source_type = 'memory' AND l.source_id = :memory_id

UNION ALL

SELECT l.relation, l.source_type, l.source_id,
       CASE l.source_type
           WHEN 'memory' THEN (SELECT summary FROM memories WHERE id = l.source_id)
           WHEN 'asset' THEN (SELECT path || ': ' || description FROM assets WHERE id = l.source_id)
           WHEN 'service' THEN (SELECT name || ' ' || version FROM services WHERE id = l.source_id)
       END AS source_desc
FROM links l
WHERE l.target_type = 'memory' AND l.target_id = :memory_id;
```

---

### 2.11 config — 运行时配置 KV

```sql
CREATE TABLE config (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

-- 初始配置
INSERT INTO config VALUES ('schema_version', '1', strftime('%Y-%m-%dT%H:%M:%f+00:00','now'));
INSERT INTO config VALUES ('steward_version', '0.1.0', strftime('%Y-%m-%dT%H:%M:%f+00:00','now'));
```

### 2.12 migrations — 版本迁移记录

```sql
CREATE TABLE migrations (
    version     INTEGER PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%f+00:00','now'))
);

INSERT INTO migrations VALUES (1, 'initial schema', strftime('%Y-%m-%dT%H:%M:%f+00:00','now'));
```

---

## 三、ER 关系图

```
┌────────────┐        ┌─────────────┐        ┌──────────────┐
│  memories  │───fts──│memories_fts │        │   services   │
│            │───vec──│memories_vec │        │              │
└──────┬─────┘        └─────────────┘        └───────┬──────┘
       │                                             │
       │              ┌─────────────┐                │
       ├──── link ────│    links    │──── link ──────┤
       │              └──────┬──────┘                │
       │                     │                       │
┌──────┴─────┐               │               ┌──────┴──────┐
│   assets   │───── link ────┘               │  changes    │
│            │                               │             │
├────────────┤                               └─────────────┘
│ asset_tags │
├────────────┤               ┌──────────────────────┐
│ assets_fts │               │    env_snapshots      │
└────────────┘               │    env_snapshot_items  │
                             └──────────────────────┘

                             ┌──────────────────────┐
                             │  config / migrations  │
                             └──────────────────────┘
```

---

## 四、查询模式汇总

### 4.1 高频查询（需优化）

| 查询 | 频率 | 优化手段 |
|------|------|----------|
| `steward brief` (最近 N 条记忆 + 环境 + 活跃项目) | 每次新会话 | 缓存 + 复合索引 |
| 记忆语义搜索 | 每次对话 | sqlite-vec HNSW |
| 记忆关键词搜索 | 每次对话 | FTS5 BM25 |
| 资产搜索 | 每次任务 | FTS5 + 标签索引 |
| 服务状态查询 | 每分钟（健康检查） | 内存缓存 |

### 4.2 中频查询

| 查询 | 频率 | 优化手段 |
|------|------|----------|
| 写入记忆 | 每次对话几条 | WAL + async embedding |
| 环境快照 | 每天 1 次 | 增量采集 |
| 变更列表 | 每天查几次 | 时间戳索引 |

### 4.3 低频查询

| 查询 | 频率 | 优化手段 |
|------|------|----------|
| 快照对比 | 每周 | FULL OUTER JOIN |
| 关联图遍历 | 按需 | 双向索引 |
| 全量导出 | 按需 | SQLite backup API |

---

## 五、数据生命周期

```
              写入                    活跃使用
Memory   ────────────► active ───────────────────────►
                          │                           │
                          │ 权重衰减 (30 天周期)       │ 手动/自动归档
                          │ weight *= 0.9             │
                          ▼                           ▼
                      weight 降低              archived
                          │                           │
                          │ 环境矛盾检测 (P2)          │ 90 天后
                          ▼                           ▼
                      outdated                   可清理

Asset    ────────────► active ───────────────────────►
                          │
                          │ 扫描发现路径不存在
                          ▼
                      missing (7 天后可清理)

Service  ────────────► running ──────────────────────►
                          │
                          │ 检测不到进程
                          ▼
                      stopped → 7 天无恢复 → unknown

Change   ────────────► 写入 ─────────────────────────►
                                              90 天后自动删除
```

---

## 六、空间估算

| 数据类型 | 单条大小（估算） | 1 万条 | 10 万条 |
|----------|------------------|--------|---------|
| Memory (不含向量) | ~500 bytes | 5 MB | 50 MB |
| Memory 向量 (768d float32) | 3,072 bytes | 30 MB | 300 MB |
| Memory FTS 索引 | ~200 bytes | 2 MB | 20 MB |
| Asset | ~300 bytes | 3 MB | 30 MB |
| Service | ~500 bytes | 通常 < 100 条 | — |
| Snapshot | ~50 KB/次 | 365 次/年 = 18 MB | — |
| Change | ~200 bytes | 2 MB | 20 MB |

**典型个人用户 1 年估算**: 1 万记忆 + 5 万资产 + 365 快照 ≈ **~100 MB 数据库文件**。

加上向量索引 ≈ **~130 MB**。完全在 SQLite 的舒适区内。

---

## 七、初始化 SQL 脚本

完整的建库脚本见 [init.sql](init.sql)（已生成并通过语法验证）。

初始化顺序：
1. PRAGMA 设置 (WAL, cache_size, mmap_size, foreign_keys)
2. 创建 `config` + `migrations` 表
3. 创建 `memories` + FTS + Vec 表
4. 创建 `assets` + FTS + Tags 表
5. 创建 `services` + `env_snapshots` + `env_snapshot_items` 表
6. 创建 `changes` 表 (P1, 但表结构在 v1 就创建好)
7. 创建 `links` 表 (P2, 同上)
8. 插入初始配置

---

*全部 7 个交付件完成。*
