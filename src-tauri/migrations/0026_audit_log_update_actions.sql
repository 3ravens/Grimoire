-- Allow update_check / update_download audit actions (CHECK must be rebuilt).
CREATE TABLE audit_log_new (
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
                      'wikipedia_read',
                      'update_check',
                      'update_download'
                  )),
    resource_type TEXT,
    resource_id   INTEGER,
    resource_name TEXT,
    detail        TEXT,
    created_at    INTEGER NOT NULL
);

INSERT INTO audit_log_new (id, action, resource_type, resource_id, resource_name, detail, created_at)
SELECT id, action, resource_type, resource_id, resource_name, detail, created_at FROM audit_log;

DROP TABLE audit_log;
ALTER TABLE audit_log_new RENAME TO audit_log;

CREATE INDEX IF NOT EXISTS idx_audit_log_created_at ON audit_log (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_log_action     ON audit_log (action);
