-- tax_profiles_extend — make the entity tax profile comprehensive enough to file.
--
-- The base tax_profiles carried only name/EIN/type/address. A return needs more:
-- formation, business activity/NAICS, S-election, officers/owners, and (for an
-- individual) filing status, spouse, and dependents. These are additive, nullable
-- columns on the existing plain (non-event-sourced) tax_profiles table — safe to
-- add out-of-band; adopt into the app's sqlx migrations when convenient.

ALTER TABLE public.tax_profiles
  ADD COLUMN IF NOT EXISTS fiscal_year_end     text,                         -- 'MM-DD' (calendar = '12-31')
  ADD COLUMN IF NOT EXISTS date_formed         date,                         -- incorporation / formation
  ADD COLUMN IF NOT EXISTS state_of_formation  text,                         -- e.g. 'IL', 'MO'
  ADD COLUMN IF NOT EXISTS naics_code          text,                         -- principal business activity code
  ADD COLUMN IF NOT EXISTS business_activity   text,                         -- e.g. 'Medical consulting'
  ADD COLUMN IF NOT EXISTS product_or_service  text,                         -- e.g. 'Consulting services'
  ADD COLUMN IF NOT EXISTS s_election_effective date,                        -- S-corps: Form 2553 effective date
  ADD COLUMN IF NOT EXISTS entity_status       text NOT NULL DEFAULT 'active', -- active | dissolved | revoking
  ADD COLUMN IF NOT EXISTS dissolved_date      date,
  ADD COLUMN IF NOT EXISTS filing_status       text,                         -- individual: 'mfj' etc.
  ADD COLUMN IF NOT EXISTS phone               text,
  ADD COLUMN IF NOT EXISTS officers_owners     jsonb NOT NULL DEFAULT '[]',  -- [{name,tin,title,ownership_pct,shares}]
  ADD COLUMN IF NOT EXISTS dependents          jsonb NOT NULL DEFAULT '[]',  -- [{first_name,last_name,dob,ssn,relationship,ctc}]
  ADD COLUMN IF NOT EXISTS spouse              jsonb;                        -- {first_name,last_name,ssn}
