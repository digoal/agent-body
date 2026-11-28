-- ext-mem-pg-tx v0.1.0 微内核目录元数据表（严格遵守 ext_mem_pg_tx__ 前缀隔离）
CREATE TABLE IF NOT EXISTS ext_mem_pg_tx__catalog_meta (
    meta_key    TEXT PRIMARY KEY,
    meta_json   TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
