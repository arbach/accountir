-- Structured vendor attribution on journal lines (a sub-ledger dimension).
-- The GL account (e.g. 5300 Subcontractors) says WHAT kind of cost a line is;
-- vendor_id says WHO was paid, linking to the vendors master so the app can
-- show / filter / report by vendor and track W-8BEN per vendor. Nullable: most
-- lines (the bank/expense counterpart) have no vendor. Soft reference (no FK) so
-- it tolerates the ad-hoc vendors table; resolved via LEFT JOIN at read time.
ALTER TABLE journal_lines ADD COLUMN IF NOT EXISTS vendor_id uuid;
CREATE INDEX IF NOT EXISTS journal_lines_vendor_idx ON journal_lines (company_id, vendor_id);
