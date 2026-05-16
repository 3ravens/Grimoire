-- CodeRabbit: allow deleting wikipedia_bundles without blocking on highlights;
-- bundle_id becomes NULL (orphan) per ON DELETE SET NULL.

PRAGMA foreign_keys = OFF;

CREATE TABLE wikipedia_highlights_new (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    bundle_id           TEXT    REFERENCES wikipedia_bundles(id) ON DELETE SET NULL,
    article_path        TEXT    NOT NULL,
    highlighted_text    TEXT    NOT NULL,
    context_before      TEXT,
    context_after       TEXT,
    created_at          TEXT    NOT NULL,
    status              TEXT    NOT NULL DEFAULT 'active'
                            CHECK(status IN ('active', 'orphaned'))
);

INSERT INTO wikipedia_highlights_new (
    id, bundle_id, article_path, highlighted_text, context_before, context_after, created_at, status
)
SELECT
    id, bundle_id, article_path, highlighted_text, context_before, context_after, created_at, status
FROM wikipedia_highlights;

DROP TABLE wikipedia_highlights;
ALTER TABLE wikipedia_highlights_new RENAME TO wikipedia_highlights;

DELETE FROM sqlite_sequence WHERE name = 'wikipedia_highlights';
INSERT INTO sqlite_sequence (name, seq)
SELECT 'wikipedia_highlights', COALESCE((SELECT MAX(id) FROM wikipedia_highlights), 0);

PRAGMA foreign_keys = ON;
