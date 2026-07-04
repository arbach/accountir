-- Signing provenance for tax forms. A form must be user-approved, then signed
-- (signature stamped onto the PDF) before it can be mailed. status flows:
-- filled -> approved -> signed -> mailed.
ALTER TABLE tax_forms ADD COLUMN IF NOT EXISTS signed_at timestamptz;
ALTER TABLE tax_forms ADD COLUMN IF NOT EXISTS signed_by text;
