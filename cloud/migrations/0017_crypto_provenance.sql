-- On-chain provenance for crypto journal entries.
--
-- Crypto activity is booked as ordinary journal_entries (source 'manual'), but the
-- entries only carried the counterparty ADDRESS in their memo — no on-chain tx hash,
-- no link to the explorer, and no way to verify them or detect duplicates. This table
-- attaches, per entry, the authoritative on-chain transaction hash + a clickable
-- explorer link + an on-chain verification result + a dedup key. It is additive: it
-- never mutates the books.
--
-- Idempotent so it can be applied manually for an immediate backfill and re-run by
-- sqlx on the next deploy without error.
CREATE TABLE IF NOT EXISTS crypto_provenance (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id       uuid NOT NULL,
    entry_id         uuid NOT NULL REFERENCES journal_entries(id),
    tx_hash          text NOT NULL,
    chain            text NOT NULL,                 -- 'eth' | 'bsc'
    counterparty     text,
    symbol           text,
    direction        text,                          -- 'IN' | 'OUT'
    amount_usd_cents bigint,
    explorer_url     text NOT NULL,                 -- https://etherscan.io/tx/0x… etc.
    dedup_key        text NOT NULL,                 -- chain:hash:dir:counterparty:symbol:amount
    verified         boolean NOT NULL DEFAULT false,
    verified_at      timestamptz,
    verify_error     text,
    created_at       timestamptz NOT NULL DEFAULT now(),
    UNIQUE (company_id, entry_id)
);

CREATE INDEX IF NOT EXISTS crypto_provenance_entry_idx ON crypto_provenance (entry_id);
CREATE INDEX IF NOT EXISTS crypto_provenance_dedup_idx ON crypto_provenance (company_id, dedup_key);
CREATE INDEX IF NOT EXISTS crypto_provenance_hash_idx ON crypto_provenance (company_id, tx_hash);

-- Tenant isolation, matching the plaid_* / statement_lines siblings.
ALTER TABLE crypto_provenance ENABLE ROW LEVEL SECURITY;
ALTER TABLE crypto_provenance FORCE ROW LEVEL SECURITY;
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_policies
        WHERE schemaname = 'public' AND tablename = 'crypto_provenance'
          AND policyname = 'tenant_isolation'
    ) THEN
        CREATE POLICY tenant_isolation ON crypto_provenance
            USING (company_id = current_company_id())
            WITH CHECK (company_id = current_company_id());
    END IF;
END$$;
