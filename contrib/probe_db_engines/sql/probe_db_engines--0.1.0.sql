-- ext-probe-db-engines v0.1.0: 多引擎数据库配置探针快照私有表
CREATE TABLE IF NOT EXISTS ext_probe_db_engines__snapshots (
    snapshot_id     TEXT PRIMARY KEY,
    engine_kind     TEXT NOT NULL,
    instance_id     TEXT NOT NULL,
    config_path     TEXT NOT NULL,
    port            INTEGER,
    data_dir        TEXT,
    params_json     TEXT NOT NULL,
    evidence_sha256 TEXT NOT NULL,
    collected_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS ext_probe_db_engines__idx_engine
    ON ext_probe_db_engines__snapshots (engine_kind);
