-- Crypto wallets: map an on-chain address to a local ledger account.
CREATE TABLE IF NOT EXISTS crypto_wallets (
    id TEXT PRIMARY KEY,
    chain TEXT NOT NULL,
    address TEXT NOT NULL,
    local_account_id TEXT NOT NULL REFERENCES accounts(id),
    label TEXT,
    explorer_base_url TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(chain, address)
);

-- Imported crypto transactions: the on-chain hash + explorer link + live-verification
-- record. PRIMARY KEY(chain, tx_hash) prevents the same on-chain tx being imported twice.
CREATE TABLE IF NOT EXISTS crypto_transactions (
    chain TEXT NOT NULL,
    tx_hash TEXT NOT NULL,
    entry_id TEXT NOT NULL REFERENCES journal_entries(id),
    wallet_id TEXT REFERENCES crypto_wallets(id) ON DELETE SET NULL,
    address TEXT NOT NULL,
    amount INTEGER NOT NULL,
    asset TEXT NOT NULL,
    block_number INTEGER,
    block_time TEXT,
    explorer_url TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    verified INTEGER NOT NULL DEFAULT 0,
    verified_at TEXT,
    verify_error TEXT,
    imported_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (chain, tx_hash)
);

CREATE INDEX IF NOT EXISTS idx_crypto_tx_entry ON crypto_transactions(entry_id);
CREATE INDEX IF NOT EXISTS idx_crypto_tx_wallet ON crypto_transactions(wallet_id);
