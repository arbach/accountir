-- Per-company uploaded file store (statements, tax documents, anything).
-- Files live on disk under FILES_DIR/<company_id>/<sha256>; metadata here.
-- UNIQUE(company_id, sha256) enforces content-level de-duplication: the same
-- file uploaded twice for a company is stored once.

CREATE TABLE company_files (
    id           uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    company_id   uuid NOT NULL,
    category     text NOT NULL DEFAULT 'other',   -- statement | tax | other
    filename     text NOT NULL,
    content_type text NOT NULL DEFAULT 'application/octet-stream',
    size_bytes   bigint NOT NULL,
    sha256       text NOT NULL,
    stored_path  text NOT NULL,
    uploaded_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (company_id, sha256)
);

ALTER TABLE company_files ENABLE ROW LEVEL SECURITY;
ALTER TABLE company_files FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON company_files
    USING (company_id = current_company_id())
    WITH CHECK (company_id = current_company_id());

CREATE INDEX company_files_idx ON company_files (company_id, category, uploaded_at DESC);
