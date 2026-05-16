-- Settings key/value store.
-- Introduced for hardware override flag; used for all future persistent settings.
CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR IGNORE INTO settings (key, value) VALUES ('llm_force_enabled', 'false');
