-- Supporting-document trail for transactions (IRS audit readiness).
-- Every expense (and ideally every material tx) must be backed by a source
-- document: invoice, contract, receipt, 1099, W-8BEN, or bank/Wise/chain statement.
--
-- Model:
--   * entry_documents  = many-to-many link between journal_entries and company_files.
--     One statement backs many entries; one paid invoice backs one entry. Both hold.
--   * journal_entries.doc_status = fast, filterable audit state:
--       'attached' (>=1 doc linked) | 'missing' (needs a doc) | 'na' (transfer/clearing, no doc expected)
--       NULL = not yet assessed.
-- Idempotent so it is safe whether applied by sqlx on startup or manually.

ALTER TABLE journal_entries ADD COLUMN IF NOT EXISTS doc_status text;

CREATE TABLE IF NOT EXISTS entry_documents (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id  uuid NOT NULL,
    entry_id    uuid NOT NULL REFERENCES journal_entries(id) ON DELETE CASCADE,
    file_id     uuid NOT NULL REFERENCES company_files(id) ON DELETE CASCADE,
    doc_type    text NOT NULL DEFAULT 'other',   -- invoice | contract | receipt | 1099 | w8ben | statement | other
    note        text,
    linked_at   timestamptz NOT NULL DEFAULT now(),
    UNIQUE (entry_id, file_id)
);

ALTER TABLE entry_documents ENABLE ROW LEVEL SECURITY;
ALTER TABLE entry_documents FORCE ROW LEVEL SECURITY;
DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_policies WHERE tablename='entry_documents' AND policyname='tenant_isolation') THEN
    CREATE POLICY tenant_isolation ON entry_documents
        USING (company_id = current_company_id())
        WITH CHECK (company_id = current_company_id());
  END IF;
END $$;

CREATE INDEX IF NOT EXISTS entry_documents_entry_idx ON entry_documents (company_id, entry_id);
CREATE INDEX IF NOT EXISTS entry_documents_file_idx  ON entry_documents (company_id, file_id);
CREATE INDEX IF NOT EXISTS journal_entries_docstatus_idx ON journal_entries (company_id, doc_status);

-- Backfill: every non-void entry that hits an expense account starts as 'missing'
-- (needs an invoice/contract/receipt) until a document is linked.
UPDATE journal_entries je SET doc_status = 'missing'
WHERE je.doc_status IS NULL AND je.is_void = false
  AND EXISTS (
    SELECT 1 FROM journal_lines jl JOIN accounts a ON a.id = jl.account_id
    WHERE jl.entry_id = je.id AND a.account_type = 'expense'
  );
