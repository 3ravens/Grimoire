-- File Scanner: user-selected external files/folders as RAG context sources.
-- Nothing in this table is inside the vault — vault files are indexed separately.

CREATE TABLE IF NOT EXISTS scanned_paths (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    path            TEXT    NOT NULL UNIQUE,
    kind            TEXT    NOT NULL CHECK(kind IN ('file', 'folder')),
    added_at        INTEGER NOT NULL,
    last_scanned_at INTEGER,         -- NULL until first scan completes
    enabled         INTEGER NOT NULL DEFAULT 1,
    file_count      INTEGER NOT NULL DEFAULT 0,
    error_msg       TEXT             -- last error, cleared on successful scan
);

-- One row per indexed file (individual file or member of a scanned folder).
-- mtime is the file's modification time at last index, used to detect staleness.
CREATE TABLE IF NOT EXISTS scanned_files (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    path_id     INTEGER NOT NULL REFERENCES scanned_paths(id) ON DELETE CASCADE,
    file_path   TEXT    NOT NULL UNIQUE,
    mime_type   TEXT    NOT NULL DEFAULT 'text/plain',
    indexed_at  INTEGER,
    mtime       INTEGER          -- Unix timestamp (seconds) of file at last index
);
