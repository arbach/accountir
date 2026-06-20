-- Wallet / counterparty address book: map a crypto wallet address (or any
-- external identifier) to a human name + optional default category, per company.
-- Used to label crypto transactions and to drive categorization.

CREATE TABLE address_labels (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id   uuid NOT NULL,
    address      text NOT NULL,            -- wallet address / identifier (stored lowercased)
    name         text NOT NULL,            -- counterparty name
    kind         text NOT NULL DEFAULT '', -- contractor | lender | exchange | own | income | expense
    account_code text NOT NULL DEFAULT '', -- optional default account code for this counterparty
    note         text NOT NULL DEFAULT '',
    created_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (company_id, address)
);

ALTER TABLE address_labels ENABLE ROW LEVEL SECURITY;
ALTER TABLE address_labels FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON address_labels
    USING (company_id = current_company_id())
    WITH CHECK (company_id = current_company_id());

CREATE INDEX idx_address_labels_company ON address_labels(company_id);
