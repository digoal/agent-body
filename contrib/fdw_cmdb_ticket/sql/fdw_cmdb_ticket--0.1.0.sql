CREATE TABLE IF NOT EXISTS ext_fdw_cmdb_ticket__packs (
    pack_id      TEXT PRIMARY KEY,
    host_id      TEXT NOT NULL,
    pack_sha256  TEXT NOT NULL,
    created_at   TEXT NOT NULL,
    summary_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS ext_fdw_cmdb_ticket__idx_time
    ON ext_fdw_cmdb_ticket__packs(created_at);
