-- Period/tax year a document pertains to (NOT the upload date). Nullable:
-- unknown until detected from the file or set by the user/agent.
ALTER TABLE company_files ADD COLUMN IF NOT EXISTS doc_year INT;

-- Ordering for the Documents view: newest period year first, unknowns last.
CREATE INDEX IF NOT EXISTS company_files_year_idx
    ON company_files (company_id, doc_year DESC NULLS LAST, uploaded_at DESC);
