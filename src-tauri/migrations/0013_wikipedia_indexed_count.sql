-- Track cumulative articles embedded so the progress display is correct
-- when resuming from a checkpoint (indexed resets to 0 each session otherwise).
ALTER TABLE wikipedia_index_checkpoint
    ADD COLUMN indexed_count INTEGER NOT NULL DEFAULT 0;
