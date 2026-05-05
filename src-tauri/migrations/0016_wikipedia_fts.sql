-- Full-text search over indexed Wikipedia article titles and intro text.
-- Rust-managed self-contained FTS5 (same pattern as notes_fts post-0009).
-- Used as a lexical fallback blended with LanceDB semantic search in chat RAG.

CREATE VIRTUAL TABLE IF NOT EXISTS wikipedia_articles_fts USING fts5(
    article_id   UNINDEXED,
    bundle_id    UNINDEXED,
    article_path UNINDEXED,
    title,
    content,
    tokenize='unicode61 remove_diacritics 2'
);
