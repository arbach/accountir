-- Coinbase integration (Wise-style: pull + cache + reconcile; read-only vs the
-- ledger — restatement goes through the normal event-sourced entry flow).
-- Auth: CDP API key (Ed25519) per company, used to mint per-request EdDSA JWTs.

CREATE TABLE coinbase_connections (
    company_id      UUID PRIMARY KEY REFERENCES companies(id),
    key_name        TEXT NOT NULL,
    private_key_b64 TEXT NOT NULL,
    label           TEXT NOT NULL DEFAULT '',
    last_synced     TIMESTAMPTZ
);
ALTER TABLE coinbase_connections ENABLE ROW LEVEL SECURITY;
ALTER TABLE coinbase_connections FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON coinbase_connections
    USING (company_id = current_company_id())
    WITH CHECK (company_id = current_company_id());

CREATE TABLE coinbase_transactions (
    company_id   UUID NOT NULL REFERENCES companies(id),
    tx_id        TEXT NOT NULL,
    account_id   TEXT NOT NULL,
    account_name TEXT NOT NULL,
    created_at   TIMESTAMPTZ,
    tx_type      TEXT NOT NULL,
    amount       NUMERIC NOT NULL,          -- native asset units (USDC 6dp, ETH 18dp…)
    currency     TEXT NOT NULL,
    native_cents BIGINT NOT NULL,           -- USD value in cents, signed
    status       TEXT NOT NULL,
    counterparty TEXT,                      -- destination address / resource for sends
    network_hash TEXT,
    raw          JSONB NOT NULL,
    PRIMARY KEY (company_id, tx_id)
);
ALTER TABLE coinbase_transactions ENABLE ROW LEVEL SECURITY;
ALTER TABLE coinbase_transactions FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON coinbase_transactions
    USING (company_id = current_company_id())
    WITH CHECK (company_id = current_company_id());
