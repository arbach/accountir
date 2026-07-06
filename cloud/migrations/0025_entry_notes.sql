-- Free-text user note on a transaction (journal entry). An annotation on top of
-- the event-sourced ledger (like entry_categories), separate from the entry's
-- memo. One note per entry.
CREATE TABLE IF NOT EXISTS entry_notes (
    company_id uuid NOT NULL,
    entry_id   uuid NOT NULL,
    note       text NOT NULL,
    updated_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (company_id, entry_id)
);

ALTER TABLE entry_notes ENABLE ROW LEVEL SECURITY;
ALTER TABLE entry_notes FORCE ROW LEVEL SECURITY;
DO $$ BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE tablename = 'entry_notes' AND policyname = 'tenant_isolation') THEN
        CREATE POLICY tenant_isolation ON entry_notes
            USING (company_id = current_company_id())
            WITH CHECK (company_id = current_company_id());
    END IF;
END $$;
