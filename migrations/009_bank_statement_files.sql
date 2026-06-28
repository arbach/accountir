-- Statement files: one row per imported bank statement file. file_hash is the SHA-256 of
-- the file contents so re-importing the same statement can be detected (dedup).
CREATE TABLE IF NOT EXISTS statement_files (
    id INTEGER PRIMARY KEY,
    file_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    file_hash TEXT NOT NULL UNIQUE,
    bank_id TEXT,
    account_id TEXT REFERENCES accounts(id),
    imported_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Link each imported journal entry to the statement file it came from.
CREATE TABLE IF NOT EXISTS entry_statement_files (
    entry_id TEXT PRIMARY KEY REFERENCES journal_entries(id),
    statement_file_id INTEGER NOT NULL REFERENCES statement_files(id)
);

CREATE INDEX IF NOT EXISTS idx_entry_statement_file ON entry_statement_files(statement_file_id);
