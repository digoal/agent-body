CREATE TABLE IF NOT EXISTS ext_probe_jvm_middleware__topology (
    node_id      TEXT PRIMARY KEY,
    kind         TEXT NOT NULL,
    summary_json TEXT NOT NULL,
    updated_at   TEXT NOT NULL
);
