-- Store the vendor's tax identification (from a W-9 for US payees, or the
-- foreign TIN declared on a W-8BEN/W-8BEN-E) so 1099s/withholding forms can be
-- completed. Nullable — most are collected over time; audit-readiness tracks the gap.
ALTER TABLE vendors ADD COLUMN IF NOT EXISTS tax_id text;
ALTER TABLE vendors ADD COLUMN IF NOT EXISTS tax_id_type text;   -- ssn | ein | foreign_tin
ALTER TABLE vendors ADD COLUMN IF NOT EXISTS tax_form_requested_at timestamptz;  -- when we last requested the form
