-- statement_lines was created in 0007 AFTER the bulk RLS loop in 0002 and never
-- had row-level security enabled, leaving tenant isolation for parsed bank
-- statement lines dependent on app-level WHERE clauses alone. Bring it in line
-- with its plaid_* / documents siblings: enable + FORCE RLS (so the table-owning
-- `accountir` role is also subject to the policy) with the standard tenant filter.
ALTER TABLE statement_lines ENABLE ROW LEVEL SECURITY;
ALTER TABLE statement_lines FORCE ROW LEVEL SECURITY;
CREATE POLICY tenant_isolation ON statement_lines
    USING (company_id = current_company_id())
    WITH CHECK (company_id = current_company_id());
