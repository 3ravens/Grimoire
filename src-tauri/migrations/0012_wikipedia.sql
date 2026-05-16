-- Wikipedia local knowledge source data layer.
-- Tracks installed Kiwix bundles, their indexing state, and user highlights.

-- One row per installed ZIM bundle.
CREATE TABLE IF NOT EXISTS wikipedia_bundles (
    id              TEXT PRIMARY KEY,   -- Kiwix catalogue entry ID (UUID string)
    name            TEXT NOT NULL,      -- e.g. "wikipedia_en_wp"
    flavour         TEXT NOT NULL,      -- always "nopic" for our use-case
    title           TEXT,               -- Human-readable title from catalogue
    article_count   INTEGER,            -- article count from ZIM header
    size_bytes      INTEGER,            -- file size in bytes
    zim_path        TEXT,               -- absolute path to the .zim file on disk
    installed_at    TEXT,               -- ISO-8601 timestamp
    last_synced     TEXT,               -- ISO-8601 timestamp of last successful index run
    indexing_state  TEXT NOT NULL DEFAULT 'none'
                        CHECK(indexing_state IN ('none', 'queued', 'indexing', 'done', 'error'))
);

-- Checkpointing for incremental indexing. One row per bundle.
-- last_indexed_entry is the 0-based ZIM entry offset we left off at,
-- so a resumed job can skip forward without re-reading everything.
CREATE TABLE IF NOT EXISTS wikipedia_index_checkpoint (
    bundle_id           TEXT PRIMARY KEY REFERENCES wikipedia_bundles(id) ON DELETE CASCADE,
    last_indexed_entry  INTEGER NOT NULL DEFAULT 0,
    total_entries       INTEGER NOT NULL DEFAULT 0,
    completed_at        TEXT    -- NULL until fully complete
);

-- User highlights inside Wikipedia articles.
-- Highlights are never silently deleted — status transitions to 'orphaned'
-- when the source bundle is removed, preserving the user's data.
CREATE TABLE IF NOT EXISTS wikipedia_highlights (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    bundle_id           TEXT    NOT NULL REFERENCES wikipedia_bundles(id),
    article_path        TEXT    NOT NULL,   -- ZIM entry path, e.g. "A/Albert_Einstein"
    highlighted_text    TEXT    NOT NULL,
    context_before      TEXT,               -- up to 100 chars before the highlight for re-anchoring
    context_after       TEXT,               -- up to 100 chars after the highlight for re-anchoring
    created_at          TEXT    NOT NULL,
    status              TEXT    NOT NULL DEFAULT 'active'
                            CHECK(status IN ('active', 'orphaned'))
);

-- Default settings for the wikipedia feature.
INSERT OR IGNORE INTO settings (key, value) VALUES ('wikipedia_enabled',      'false');
INSERT OR IGNORE INTO settings (key, value) VALUES ('wikipedia_storage_path', '');
