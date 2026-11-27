CREATE TABLE IF NOT EXISTS ext_hook_audit_siem__outbox (
    event_id    TEXT PRIMARY KEY,
    format      TEXT NOT NULL,
    payload     TEXT NOT NULL,
    recorded_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS ext_hook_audit_siem__idx_recorded
    ON ext_hook_audit_siem__outbox(recorded_at);
