-- Note version history snapshots for explicit saves and restores.
-- One row stores the full pre-change note state; locked-note snapshots remain
-- encrypted (same ciphertext format as notes.title/content).

CREATE TABLE IF NOT EXISTS note_versions (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    note_id      INTEGER NOT NULL,
    title        TEXT NOT NULL,
    content      TEXT NOT NULL,
    is_encrypted INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL DEFAULT (unixepoch()),
    FOREIGN KEY (note_id) REFERENCES notes(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_note_versions_note_created
    ON note_versions(note_id, created_at DESC, id DESC);
