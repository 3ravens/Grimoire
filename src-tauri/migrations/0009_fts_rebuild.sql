-- Rebuild the FTS5 index so it stores decrypted plaintext, managed by Rust.
--
-- The trigger-based approach from 0008 had a fatal flaw: SQLite triggers fire
-- with whatever is in the `notes` table at the time of INSERT/UPDATE. For
-- encrypted notes that content is ciphertext, not searchable text. This means
-- the FTS index was full of garbage for any vault or folder that has a password.
--
-- Solution: drop the triggers entirely. The Rust backend now explicitly calls
-- fts_upsert() / fts_delete() after create/update/delete operations, passing
-- the DECRYPTED title and content. This way the FTS index always contains
-- searchable plaintext.
--
-- The table is also changed from an "external content" table (content='notes')
-- to a self-contained table. External content tables prevented storing decrypted
-- text separately from the encrypted notes rows. The self-contained table stores
-- the text it was given, independently of what is in notes.
--
-- Notes in locked/inaccessible folders are never added to FTS. They are removed
-- when a folder is locked and re-added when it is unlocked (via reindex_all).
--
-- Run "Re-index all" from Settings after this migration to populate FTS for
-- existing notes.

DROP TRIGGER IF EXISTS notes_fts_insert;
DROP TRIGGER IF EXISTS notes_fts_update;
DROP TRIGGER IF EXISTS notes_fts_delete;

DROP TABLE IF EXISTS notes_fts;

CREATE VIRTUAL TABLE IF NOT EXISTS notes_fts USING fts5(
    title,
    content,
    tokenize='unicode61 remove_diacritics 2'
    -- no content= or content_rowid= : this table stores text directly
);
