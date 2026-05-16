-- Add template_id tracking to notes and property_defs.
--
-- Both columns are nullable. Existing rows get NULL, meaning
-- "created before template tracking was introduced". This is
-- intentional and backward-compatible — the app handles NULL
-- template_id gracefully everywhere.
--
-- notes.template_id     → which template was used to create this note
-- property_defs.template_id → which template introduced this column
--
-- ON DELETE SET NULL: if a template is deleted, the tracking is simply
-- cleared; the notes and column definitions themselves are preserved.

ALTER TABLE notes ADD COLUMN template_id INTEGER REFERENCES templates(id) ON DELETE SET NULL;
ALTER TABLE property_defs ADD COLUMN template_id INTEGER REFERENCES templates(id) ON DELETE SET NULL;
