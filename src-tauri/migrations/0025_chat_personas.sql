-- User-managed chat personas (built-ins are code constants, not rows).
-- Active persona id is settings key `chat_active_persona` (`builtin:…` / `user:<id>` / empty = None).
CREATE TABLE IF NOT EXISTS chat_personas (
    id                    INTEGER PRIMARY KEY AUTOINCREMENT,
    title                 TEXT    NOT NULL,
    body                  TEXT    NOT NULL,
    also_apply_verbosity  INTEGER NOT NULL DEFAULT 0,
    created_at            INTEGER NOT NULL DEFAULT (unixepoch())
);
