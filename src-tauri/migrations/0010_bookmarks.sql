-- Bookmarks: pin frequently accessed notes above the folder tree.
-- Keyed by note_id so bookmarks survive note renames.
-- ON DELETE CASCADE ensures a deleted note's bookmark is removed automatically.
-- `position` is reserved for future drag-to-reorder support; not currently used.
CREATE TABLE IF NOT EXISTS bookmarks (
    note_id   INTEGER PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
    position  INTEGER NOT NULL DEFAULT 0,
    added_at  INTEGER NOT NULL DEFAULT (unixepoch())
);
