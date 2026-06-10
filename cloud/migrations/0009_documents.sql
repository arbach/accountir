-- Generated report / tax documents (stored as print-ready HTML fragments,
-- saved to PDF from the browser). Created by users or the AI agent.
CREATE TABLE documents (
    id         uuid PRIMARY KEY,
    company_id uuid NOT NULL,
    kind       text NOT NULL DEFAULT 'report',   -- 'report' | 'tax'
    title      text NOT NULL,
    html       text NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX documents_company_idx ON documents (company_id, created_at DESC);

ALTER TABLE documents ENABLE ROW LEVEL SECURITY;
ALTER TABLE documents FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON documents
    USING (company_id = current_company_id())
    WITH CHECK (company_id = current_company_id());
