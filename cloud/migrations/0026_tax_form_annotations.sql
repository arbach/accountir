-- Editor annotations (text boxes, highlights, redactions, placed signatures)
-- overlaid on a tax form. Stored as data so they stay editable; baked onto the
-- PDF (over a pristine .base snapshot) on each save.
ALTER TABLE tax_forms ADD COLUMN IF NOT EXISTS annotations jsonb NOT NULL DEFAULT '[]'::jsonb;
