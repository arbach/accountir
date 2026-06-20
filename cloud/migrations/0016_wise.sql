-- Wise (TransferWise) integration: per-company API connection + cached pulls.
CREATE TABLE IF NOT EXISTS wise_connections (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id   uuid NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    profile_id   text NOT NULL,                 -- Wise profile id
    label        text NOT NULL DEFAULT '',      -- e.g. "MAVEN FINANCIAL TECHNOLOGIES INC."
    api_token    text NOT NULL,                 -- Wise API token (read)
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_synced  timestamptz,
    UNIQUE (company_id, profile_id)
);
ALTER TABLE wise_connections ENABLE ROW LEVEL SECURITY;
ALTER TABLE wise_connections FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON wise_connections
    USING (company_id = current_company_id())
    WITH CHECK (company_id = current_company_id());

-- Cached transfers pulled from Wise (the authoritative payment record).
CREATE TABLE IF NOT EXISTS wise_transfers (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id      uuid NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    profile_id      text NOT NULL,
    transfer_id     bigint NOT NULL,            -- Wise transfer id
    created         date,
    status          text,
    source_value    bigint NOT NULL,            -- cents (USD source)
    source_currency text,
    target_value    bigint,                     -- minor units of target ccy
    target_currency text,
    recipient       text,                       -- accountHolderName or "acct <id>"
    is_self_topup   boolean NOT NULL DEFAULT false,
    UNIQUE (company_id, transfer_id)
);
ALTER TABLE wise_transfers ENABLE ROW LEVEL SECURITY;
ALTER TABLE wise_transfers FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON wise_transfers
    USING (company_id = current_company_id())
    WITH CHECK (company_id = current_company_id());
CREATE INDEX IF NOT EXISTS wise_transfers_company_idx ON wise_transfers (company_id, created DESC);
