CREATE TABLE IF NOT EXISTS ext_probe_linux_ops__snapshots (
    snapshot_id   TEXT PRIMARY KEY,
    host_id       TEXT NOT NULL,
    captured_at   TEXT NOT NULL,
    summary_json  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS ext_probe_linux_ops__idx_time
    ON ext_probe_linux_ops__snapshots(captured_at);
