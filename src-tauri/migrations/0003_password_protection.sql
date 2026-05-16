-- Password protection schema
--
-- vault_lock: stores the Argon2id salt and an encrypted sentinel for the vault-level password.
-- One row maximum (id = 1). If this table is empty, no vault password is set.
CREATE TABLE IF NOT EXISTS vault_lock (
    id       INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    salt     TEXT NOT NULL,     -- base64-encoded 32-byte Argon2id salt
    sentinel TEXT NOT NULL      -- base64-encoded AES-256-GCM encrypted sentinel blob
);

-- Per-folder lock support.
-- locked = 1 means this folder has a password set.
-- salt/sentinel are the same format as vault_lock, scoped to this folder.
ALTER TABLE folders ADD COLUMN locked   INTEGER NOT NULL DEFAULT 0;
ALTER TABLE folders ADD COLUMN salt     TEXT;
ALTER TABLE folders ADD COLUMN sentinel TEXT;
