-- Checkpointed full vault note re-index (LanceDB + per-note FTS during bulk reindex).
-- Invalidated when the notes vector table is dropped (clear_notes_index), on vault lock
-- (Lance purge), or when explicitly abandoned. Queue is a frozen ordered note_id list;
-- next_pos is the next queue row to process (0-based).

CREATE TABLE IF NOT EXISTS vault_reindex_state (
    id                INTEGER PRIMARY KEY CHECK (id = 1),
    embedding_model   TEXT    NOT NULL,
    started_at        TEXT    NOT NULL,
    next_pos          INTEGER NOT NULL,
    total             INTEGER NOT NULL,
    indexed_ok        INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS vault_reindex_queue (
    pos     INTEGER NOT NULL,
    note_id INTEGER NOT NULL,
    PRIMARY KEY (pos)
);
