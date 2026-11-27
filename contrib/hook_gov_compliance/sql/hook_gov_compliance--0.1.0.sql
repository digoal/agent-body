CREATE TABLE IF NOT EXISTS ext_hook_gov_compliance__rules (
    rule_id     TEXT PRIMARY KEY,
    pattern     TEXT NOT NULL,
    replacement TEXT NOT NULL,
    enabled     INTEGER NOT NULL DEFAULT 1,
    updated_at  TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS ext_hook_gov_compliance__idx_enabled
    ON ext_hook_gov_compliance__rules(enabled);
