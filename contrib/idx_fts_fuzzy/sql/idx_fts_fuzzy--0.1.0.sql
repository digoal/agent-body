-- ext-idx-fts-fuzzy v0.1.0: 分词与模糊索引状态持久化私有表
CREATE TABLE IF NOT EXISTS ext_idx_fts_fuzzy__terms (
    entity_type TEXT NOT NULL,
    entity_id   TEXT NOT NULL,
    term        TEXT NOT NULL,
    term_freq   INTEGER NOT NULL DEFAULT 1,
    doc_len     INTEGER NOT NULL DEFAULT 1,
    updated_at  TEXT NOT NULL,
    PRIMARY KEY (entity_type, entity_id, term)
);

CREATE INDEX IF NOT EXISTS ext_idx_fts_fuzzy__idx_term
    ON ext_idx_fts_fuzzy__terms (term);
