-- Add a JSON column to templates for storing a seed set of property definitions.
-- Each entry in the JSON array is a {name, type, options} object.
-- When a note is created from a template, these specs are applied to the folder's
-- property_defs schema (INSERT OR IGNORE, so existing columns are never overwritten).
ALTER TABLE templates ADD COLUMN properties TEXT NOT NULL DEFAULT '[]';
