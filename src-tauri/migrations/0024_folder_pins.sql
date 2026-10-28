-- Folder pins: keep chosen notes at the top of that folder's note list.
-- Keyed by note_id (one pin per note). folder_id must match the note's current folder;
-- move_note clears the row when the note leaves the folder.
CREATE TABLE IF NOT EXISTS folder_pins (
    note_id   INTEGER PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
    folder_id INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    position  INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_folder_pins_folder_position
    ON folder_pins(folder_id, position);
