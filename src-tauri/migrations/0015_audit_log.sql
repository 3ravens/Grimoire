-- Audit log: append-only record of every privacy-sensitive action.
-- Entries are never updated — only inserted, and optionally cleared in bulk by the user.
-- The `audit_enabled` setting (default true) gates all writes.
-- File-scanner actions are additionally gated by `log_file_access`.

CREATE TABLE IF NOT EXISTS audit_log (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    action        TEXT    NOT NULL CHECK(action IN (
                      'note_open',
                      'note_create',
                      'note_update',
                      'note_delete',
                      'note_export',
                      'folder_create',
                      'folder_rename',
                      'folder_delete',
                      'search_fts',
                      'search_semantic',
                      'search_combined',
                      'llm_chat',
                      'llm_improve',
                      'file_scan',
                      'file_import',
                      'wikipedia_read'
                  )),
    resource_type TEXT,             -- 'note', 'folder', 'file', 'wikipedia', etc.
    resource_id   INTEGER,          -- SQLite row id when applicable (notes, folders)
    resource_name TEXT,             -- human-readable name at time of action
    detail        TEXT,             -- query text, destination path, sub-action label, etc.
    created_at    INTEGER NOT NULL  -- Unix timestamp (seconds)
);

CREATE INDEX IF NOT EXISTS idx_audit_log_created_at ON audit_log (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_action     ON audit_log (action);
