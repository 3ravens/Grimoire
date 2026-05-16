-- Property definitions: one row per column in a folder's "database".
-- folder_id = NULL would mean a note-local property, but for now all
-- properties are scoped to a folder (the "database schema").
CREATE TABLE IF NOT EXISTS property_defs (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    folder_id  INTEGER REFERENCES folders(id) ON DELETE CASCADE,
    name       TEXT    NOT NULL,
    type       TEXT    NOT NULL CHECK(type IN ('text','number','date','boolean','select')),
    options    TEXT,   -- JSON array of strings, only used when type = 'select'
    position   INTEGER NOT NULL DEFAULT 0,
    UNIQUE(folder_id, name)
);

-- Actual property values per note.
-- Cascades on both note and property-def deletion.
CREATE TABLE IF NOT EXISTS note_properties (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    note_id    INTEGER NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    def_id     INTEGER NOT NULL REFERENCES property_defs(id) ON DELETE CASCADE,
    value      TEXT    NOT NULL DEFAULT '',
    UNIQUE(note_id, def_id)
);
