-- Full-text search index over note titles and content.
--
-- We use an FTS5 "external content" table (content='notes', content_rowid='id').
-- This means the FTS index stores only its inverted index structure — note text
-- is NOT duplicated. When snippet() or highlight() need the source text they
-- read it from the `notes` table directly.
--
-- Tokenizer choice: unicode61 with diacritic removal.
-- - unicode61 is the default FTS5 tokenizer and handles non-ASCII text correctly.
-- - remove_diacritics 2 folds accented characters (é → e) so "resume" matches
--   "résumé" and vice versa. Level 2 applies to the full Unicode range.
--
-- Three triggers keep the index in sync with the notes table automatically.
-- They fire on every INSERT, UPDATE, and DELETE on notes.
--
-- The initial INSERT at the end populates the index for all existing notes.
-- Running it again would cause duplicates, but SQLite migrations run once so
-- this is safe.

CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    title,
    content,
    content='notes',
    content_rowid='id',
    tokenize='unicode61 remove_diacritics 2'
);

-- Keep the FTS index in sync with the notes table.
CREATE TRIGGER IF NOT EXISTS notes_fts_insert
    AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts(rowid, title, content)
        VALUES (new.id, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS notes_fts_update
    AFTER UPDATE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, content)
        VALUES ('delete', old.id, old.title, old.content);
    INSERT INTO notes_fts(rowid, title, content)
        VALUES (new.id, new.title, new.content);
END;

CREATE TRIGGER IF NOT EXISTS notes_fts_delete
    AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, content)
        VALUES ('delete', old.id, old.title, old.content);
END;

-- Populate the index for all notes that already exist before this migration ran.
INSERT INTO notes_fts(rowid, title, content)
    SELECT id, title, content FROM notes;
