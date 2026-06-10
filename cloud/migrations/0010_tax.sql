-- Tax filing pipeline: company tax profile + tracked tax forms
-- (pulled from irs.gov -> filled -> user-approved -> mailed via Lob).

CREATE TABLE tax_profiles (
    company_id  uuid PRIMARY KEY,
    entity_type text NOT NULL DEFAULT '',     -- schedule_c | s_corp | partnership | c_corp
    legal_name  text NOT NULL DEFAULT '',
    ein         text NOT NULL DEFAULT '',
    address     jsonb NOT NULL DEFAULT '{}'::jsonb,  -- {line1,line2,city,state,zip}
    updated_at  timestamptz NOT NULL DEFAULT now()
);
ALTER TABLE tax_profiles ENABLE ROW LEVEL SECURITY;
ALTER TABLE tax_profiles FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON tax_profiles
    USING (company_id = current_company_id())
    WITH CHECK (company_id = current_company_id());

CREATE TABLE tax_forms (
    id          uuid PRIMARY KEY,
    company_id  uuid NOT NULL,
    year        int NOT NULL,
    form_code   text NOT NULL,                -- e.g. f1040sc, f1120s, f1099nec
    title       text NOT NULL,
    status      text NOT NULL DEFAULT 'pulled',  -- pulled | filled | approved | mailed
    file_path   text NOT NULL,
    fields      jsonb,                        -- last values applied
    lob_id      text,
    lob_status  text,
    to_address  jsonb,
    mailed_at   timestamptz,
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX tax_forms_company_idx ON tax_forms (company_id, year, created_at DESC);
ALTER TABLE tax_forms ENABLE ROW LEVEL SECURITY;
ALTER TABLE tax_forms FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON tax_forms
    USING (company_id = current_company_id())
    WITH CHECK (company_id = current_company_id());
