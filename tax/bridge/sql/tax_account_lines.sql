-- tax_account_lines — persistent per-account tax-line tag (Tier 1).
--
-- The accountir equivalent of QuickBooks' "tax-line mapping": each expense/revenue
-- account carries the tax-form line its book amount flows to. Non-event-sourced
-- tax-subsystem metadata (mirrors public.tax_profiles / public.tax_forms), so it is
-- safe to upsert without touching the event-sourced ledger, events, or the merkle/
-- owner-signature audit chain.
--
-- SOURCE OF TRUTH is the git-versioned files tax/bridge/maps/tax_lines_<entity>.json;
-- sync_tax_lines.py upserts them here so the accountir app can surface the column.
-- This DDL is idempotent; adopt it into the app's sqlx migrations when convenient.

CREATE TABLE IF NOT EXISTS public.tax_account_lines (
  company_id         uuid        NOT NULL REFERENCES public.companies(id) ON DELETE CASCADE,
  account_number     text        NOT NULL,
  account_name       text        NOT NULL,
  form_code          text        NOT NULL,           -- f1120 | f1120s | f1040 | f8825
  category           text,                            -- canonical classifier category
  node               text,                            -- opentax input node
  field              text,                            -- tax line field on that node
  line_label         text,                            -- human line name ("11 Rents")
  sign               text,                            -- abs | signed
  separately_stated  boolean     NOT NULL DEFAULT false,  -- Schedule K item (not ordinary)
  factor             numeric,                         -- e.g. 0.5 meals; null = 100%
  excluded           boolean     NOT NULL DEFAULT false,  -- non-deductible
  splits             jsonb,                           -- [{node,field,line,amount|pct}] or null
  confidence         text,                            -- high | medium | low | none | override
  status             text        NOT NULL DEFAULT 'auto',  -- auto | confirmed | override
  flags              jsonb       NOT NULL DEFAULT '[]'::jsonb,
  updated_at         timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (company_id, account_number)
);

ALTER TABLE public.tax_account_lines ENABLE ROW LEVEL SECURITY;

DO $$ BEGIN
  IF NOT EXISTS (
    SELECT 1 FROM pg_policies WHERE tablename = 'tax_account_lines' AND policyname = 'tenant_isolation'
  ) THEN
    CREATE POLICY tenant_isolation ON public.tax_account_lines
      USING (company_id = current_company_id())
      WITH CHECK (company_id = current_company_id());
  END IF;
END $$;

-- The app connects as the non-superuser role `accountir` (RLS-enforced). It must
-- have table privileges or the Accounts page's LEFT JOIN fails. Mirrors tax_profiles.
GRANT SELECT, INSERT, UPDATE, DELETE ON public.tax_account_lines TO accountir;
