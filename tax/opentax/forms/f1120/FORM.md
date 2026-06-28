# Form 1120 — U.S. Corporation Income Tax Return  (SCAFFOLD — not yet implemented)

IRC §11 / §6012(a)(2) | IRS Instructions for Form 1120 | MeF: Corporate (1120) package

> Status: **spec only.** No nodes yet. Do NOT register in `catalog.ts` until tests pass.
> Build by mirroring `forms/f1040/2025/` (inputs → intermediate → outputs → index.ts → catalog).

## Who uses it (our books)
- **MAVEN FINANCIAL TECHNOLOGIES INC** — EIN 92-3379962 — C-corp. 2025 book net **−$3,886.33** (a loss).
  - Loss → **NOL carryforward** (Form 1120 line 29a / Schedule); does NOT flow to the owner's 1040.
  - Key 2025 items already in Maven's books: **$80,000 IP sale** (revenue, ref IP-SALE-2025), large
    **Subcontractors** expense ($135,410.74 Wise + $96,335.20 other), consulting revenue, grant income.

## Line map to build (start here; verify against current IRS instructions)
Income: 1a gross receipts · 1c net receipts · 3 gross profit · 4 dividends · 5 interest · 6 rents ·
8 capital gain (Sch D) · 10 other income · **11 total income**.
Deductions: 12 comp of officers · 13 salaries/wages · 17 taxes · 19 charitable · 20 depreciation (4562) ·
26 other deductions · **27 total deductions** · 28 income before NOL · 29a NOL · 30 taxable income ·
31 total tax (21% flat) · refund/owed.
Schedules: C (dividends), J (tax computation), K (other info), L/M-1/M-2 (balance sheet & book-tax —
**small-corp exception**: if receipts & assets both < $250k, Sch L/M may be omitted — check Sch K Q13).

## Inputs the bridge must supply (from Maven's ledger)
revenue accounts → income lines; expense accounts → deduction lines (officer comp split out);
prior-year NOL carryforward; fixed-asset/depreciation schedule (4562). See `tax/bridge/BRIDGE.md`.

## Done when
Computes Maven's −$3,886 (or corrected) taxable loss, $0 tax, NOL carryforward tracked; validate + PDF pass.
