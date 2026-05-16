-- CodeRabbit: FK join / cascade indexes and folder lock-state enforcement.
-- Forward-only migration (do not edit applied baseline migrations).

-- Indexes for common joins and cascades
CREATE INDEX IF NOT EXISTS idx_folders_parent_id ON folders(parent_id);
CREATE INDEX IF NOT EXISTS idx_notes_folder_id ON notes(folder_id);
CREATE INDEX IF NOT EXISTS idx_note_tags_tag_id ON note_tags(tag_id);
CREATE INDEX IF NOT EXISTS idx_note_links_target_id ON note_links(target_id);
CREATE INDEX IF NOT EXISTS idx_scanned_files_path_id ON scanned_files(path_id);

-- SQLite cannot ADD cross-column CHECK on existing tables; enforce invariants on write.
DROP TRIGGER IF EXISTS folders_lock_chk_bi;
CREATE TRIGGER folders_lock_chk_bi
BEFORE INSERT ON folders
FOR EACH ROW
WHEN NEW.locked NOT IN (0, 1)
   OR (NEW.locked = 1 AND (NEW.salt IS NULL OR NEW.sentinel IS NULL))
BEGIN
    SELECT RAISE(ABORT, 'folders: invalid lock state (locked must be 0/1; locked=1 requires salt and sentinel)');
END;

DROP TRIGGER IF EXISTS folders_lock_chk_bu;
CREATE TRIGGER folders_lock_chk_bu
BEFORE UPDATE ON folders
FOR EACH ROW
WHEN NEW.locked NOT IN (0, 1)
   OR (NEW.locked = 1 AND (NEW.salt IS NULL OR NEW.sentinel IS NULL))
BEGIN
    SELECT RAISE(ABORT, 'folders: invalid lock state (locked must be 0/1; locked=1 requires salt and sentinel)');
END;
