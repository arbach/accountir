# Tax-aligned chart of accounts (books → tax, all tiers)

Making the books map cleanly to tax forms, so step-4 form-filling is deterministic
and auditable instead of guesswork. Three tiers, least to most invasive.

## Tier 1 — Tax-line tagging (non-destructive)
Every account carries the tax-form line its book amount flows to (accountir's
equivalent of QuickBooks' tax-line mapping). **Source of truth = git-versioned files**
`bridge/maps/tax_lines_<entity>.json`; the classifier (`bridge/classify.py`) only
*proposes* the initial tags. Human decisions (status `confirmed`/`override`) are
preserved across regenerations and carry forward every year.

```
python3 bridge/tag_accounts.py --all       # generate/refresh tag files (rules → tags)
python3 bridge/sync_tax_lines.py --init --all   # push tags → tax_account_lines DB table
python3 bridge/step4.py --entity hayat --year 2025   # fill from tags, reconcile
```
`tax_account_lines` is a plain, non-event-sourced table (mirrors `tax_profiles`),
so the accountir UI can surface a per-account "Tax line" column. DDL:
`bridge/sql/tax_account_lines.sql` — adopt into the app's sqlx migrations when ready.

## Tier 2 — Structural fixes (bookkeeping change request)
Some accounts need a split or reclass the tag layer can only partly represent:
- **Split tags** (`splits: [{node, field, amount|pct}]`) let one book account feed
  several tax lines, so returns are correct *before* the books are physically
  restructured (e.g. Contractor → Officer Comp + Contract Labor).
- **`coa/CHANGE_REQUEST.md`** (generated) lists accounts to add and accounts to
  split/verify, each marked whether it needs a source document. Because accountir is
  event-sourced with a merkle/owner-signature audit chain, these are applied **through
  the app**, never by writing `accounts`/`journal_lines` directly.

```
python3 coa/coa_gap.py --all --json /tmp/gap.json
python3 coa/change_request.py --gap /tmp/gap.json --out coa/CHANGE_REQUEST.md
```

## Tier 3 — Tax-aligned CoA templates + gap report
Canonical charts of accounts keyed 1:1 to tax lines, per entity type
(`coa/templates/{c_corp,s_corp,s_corp_rental,individual}.json`). `coa_gap.py` diffs
an entity's live CoA against its template → coverage, missing named accounts, and
structural flags. Adopt the template for new entities/years so mapping is 1:1 by
construction.

## Guardrails
- Tagging and templates never touch the ledger; only the tax-subsystem tag store.
- Ledger restructuring is a bookkeeping-side operation (the change request).
- Nothing is invented: source-doc-gated items are flagged, not filled.
