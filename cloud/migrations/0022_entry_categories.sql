-- User-assigned category tag on a transaction (journal entry), e.g. "Loan",
-- "Transfer", "Owner". This is an ANNOTATION on top of the event-sourced ledger
-- (not an accounting fact), so it lives in its own table and survives
-- re-projection of journal_entries. One category per entry.
CREATE TABLE entry_categories (
    company_id uuid NOT NULL,
    entry_id   uuid NOT NULL,
    category   text NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (company_id, entry_id)
);

ALTER TABLE entry_categories ENABLE ROW LEVEL SECURITY;
ALTER TABLE entry_categories FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON entry_categories
    USING (company_id = current_company_id())
    WITH CHECK (company_id = current_company_id());

CREATE INDEX entry_categories_cat_idx ON entry_categories (company_id, category);
