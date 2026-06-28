# Bridge — accountir ledger → OpenTax return inputs

The books are the source of truth. This bridge turns one entity-year of the `accountir_cloud`
ledger into OpenTax `form add` inputs so a return computes from actuals (no re-keying).

## Data source
Postgres `accountir_cloud` (read-only here). Amounts are in **cents** (÷100 for dollars).
- `companies` (slug, id, owner_user_id, is_personal, base_currency)
- `accounts` (account_number, name, account_type: asset/liability/equity/revenue/expense)
- `journal_entries` (date, memo, reference, source, is_void) + `journal_lines` (amount [+debit/−credit], account_id, **vendor_id**)
- `tax_profiles` (entity_type, legal_name, ein, address)
- `vendors` (name, country, tax_status, required_tax_form) — for 1099/W-8 + per-vendor expense
Access: `sudo -u postgres psql accountir_cloud`. Net income for a period =
`-SUM(jl.amount) over revenue+expense accounts, is_void=false, date in [start,end]`.

## Mapping model (per entity type)
The bridge groups the period's ledger by GL account and emits OpenTax input-node payloads:
- **Revenue accounts** → income input nodes (1040: business/interest/dividend/cap-gain; 1120/1120-S: receipts/other income).
- **Expense accounts** → deduction input nodes (split **officer compensation** out for S/C-corp reasonableness).
- **Rental** accounts (SweetHome) → 8825 per-property inputs (Sch K line 2, not ordinary).
- **Capital** transactions → Sch D / 8949 (e.g. Michael's $220k Maven-stock LTCG, Box F).
- **Per-vendor** expense (via `journal_lines.vendor_id` → `vendors`) → 1099-NEC / W-8 tracking.
- `tax_profiles` → the filer/officer identity (name, EIN/SSN, address).

OpenTax node input schemas are the contract — discover them with:
`deno task tax node list` and `deno task tax node inspect --node_type <type>`.
Map each GL account (or account range) to the correct node + field. Keep the mapping in a
checked-in table per form (e.g. `bridge/map_f1120s.json`) so it's auditable and reviewable.

## Reconciliation requirement
After emitting inputs and computing the return, assert the computed taxable income / ordinary
income **ties to the entity's book net** (and to any prior filed return); log any delta. A return
that doesn't reconcile to the ledger is a bug, not a rounding nuance.

## Starter
`export_return.py` is a scaffold: it connects, pulls the entity-year P&L grouped by account, and
prints a proposed node mapping for review. Flesh out the per-form account→node maps, then have it
emit actual `opentax form add` calls (or write the return JSON OpenTax persists under its `.state`).
