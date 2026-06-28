-- Source provenance for every non-crypto journal entry: the statement file / Plaid
-- sync / Wise / reclassification it originated from. Crypto entries keep their richer
-- provenance in crypto_provenance. Additive; never mutates the books. Idempotent.
CREATE TABLE IF NOT EXISTS entry_sources (
    company_id  uuid NOT NULL,
    entry_id    uuid PRIMARY KEY REFERENCES journal_entries(id),
    source_kind text NOT NULL,                 -- statement | plaid | wise | reclass | manual
    source_file text,                          -- statement filename, when known
    source_detail text,                        -- free-form (e.g. plaid txn id, reference)
    created_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS entry_sources_company_idx ON entry_sources (company_id);
CREATE INDEX IF NOT EXISTS entry_sources_kind_idx ON entry_sources (company_id, source_kind);

ALTER TABLE entry_sources ENABLE ROW LEVEL SECURITY;
ALTER TABLE entry_sources FORCE ROW LEVEL SECURITY;
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_policies
        WHERE schemaname='public' AND tablename='entry_sources' AND policyname='tenant_isolation') THEN
        CREATE POLICY tenant_isolation ON entry_sources
            USING (company_id = current_company_id())
            WITH CHECK (company_id = current_company_id());
    END IF;
END$$;

-- Full from/to wallet addresses for crypto entries, so the detail page can show the
-- complete sending/receiving wallet with an explorer link.
ALTER TABLE crypto_provenance ADD COLUMN IF NOT EXISTS from_address text;
ALTER TABLE crypto_provenance ADD COLUMN IF NOT EXISTS to_address   text;
