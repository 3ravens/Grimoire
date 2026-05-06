-- File scanner: per-path exclude patterns (newline-separated gitignore-style globs)
-- and default global excludes setting.

ALTER TABLE scanned_paths ADD COLUMN exclude_patterns TEXT NOT NULL DEFAULT '';

INSERT OR IGNORE INTO settings (key, value) VALUES ('file_scanner_global_excludes', '');
