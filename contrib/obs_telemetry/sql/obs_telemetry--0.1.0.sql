CREATE TABLE IF NOT EXISTS ext_obs_telemetry__snapshots (
    snapshot_id TEXT PRIMARY KEY,
    captured_at TEXT NOT NULL,
    health_grade TEXT NOT NULL,
    summary_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS ext_obs_telemetry__idx_captured
    ON ext_obs_telemetry__snapshots(captured_at);
