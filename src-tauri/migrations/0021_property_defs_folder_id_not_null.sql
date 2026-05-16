-- CodeRabbit: property_defs.folder_id must be NOT NULL so UNIQUE(folder_id, name) is meaningful.
-- Orphan defs (NULL folder) are removed; their values are dropped first (no valid folder to scope).

PRAGMA foreign_keys = OFF;

DELETE FROM note_properties
WHERE def_id IN (SELECT id FROM property_defs WHERE folder_id IS NULL);

DELETE FROM property_defs WHERE folder_id IS NULL;

CREATE TABLE property_defs_new (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    folder_id  INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    name       TEXT    NOT NULL,
    type       TEXT    NOT NULL CHECK(type IN ('text','number','date','boolean','select')),
    options    TEXT,
    position   INTEGER NOT NULL DEFAULT 0,
    UNIQUE(folder_id, name)
);

INSERT INTO property_defs_new (id, folder_id, name, type, options, position)
SELECT id, folder_id, name, type, options, position FROM property_defs;

DROP TABLE property_defs;
ALTER TABLE property_defs_new RENAME TO property_defs;

-- Preserve AUTOINCREMENT sequence after table rebuild
DELETE FROM sqlite_sequence WHERE name = 'property_defs';
INSERT INTO sqlite_sequence (name, seq)
SELECT 'property_defs', COALESCE((SELECT MAX(id) FROM property_defs), 0);

PRAGMA foreign_keys = ON;
