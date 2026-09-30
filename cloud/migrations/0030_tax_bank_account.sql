-- Direct-deposit / direct-debit bank account per entity, used for tax refunds
-- and balance-due payments (1040 35b-d, IL Step 10, 1120/1120-S refund lines).
--
-- The routing number is a public bank identifier (printed on every check) and is
-- stored in the clear. The ACCOUNT number is ACH-actionable, so it is encrypted
-- at rest with AES-256-GCM under the app's data key, exactly like Plaid tokens;
-- only a last-4 is kept in the clear so the UI and agents can confirm which
-- account is on file without decrypting anything.
ALTER TABLE tax_profiles
  ADD COLUMN IF NOT EXISTS bank_name              text,
  ADD COLUMN IF NOT EXISTS bank_routing           text,
  ADD COLUMN IF NOT EXISTS bank_account_last4     text,
  ADD COLUMN IF NOT EXISTS bank_account_type      text,  -- checking | savings
  ADD COLUMN IF NOT EXISTS bank_account_enc       bytea,
  ADD COLUMN IF NOT EXISTS bank_account_nonce     bytea;
