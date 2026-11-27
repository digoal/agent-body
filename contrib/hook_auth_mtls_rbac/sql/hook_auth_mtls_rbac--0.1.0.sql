CREATE TABLE IF NOT EXISTS ext_hook_auth_mtls_rbac__policies (
    principal_id TEXT PRIMARY KEY,
    tenant_id    TEXT NOT NULL,
    project_id   TEXT,
    env          TEXT,
    roles_json   TEXT NOT NULL
);
