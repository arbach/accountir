# Form 1120-S — U.S. Income Tax Return for an S Corporation  (SCAFFOLD — not yet implemented)

IRC §1361–1379 / §6037 | IRS Instructions for Form 1120-S | MeF: Corporate (1120-S) package

> Status: **spec only.** No nodes yet. Do NOT register in `catalog.ts` until tests pass.
> Mirror `forms/f1040/2025/`. The OpenTax `tax-build` skill can scaffold this: `tax-build 1120s`.

## Who uses it (our books) — three S-corps, all flow to the owner's 1040 via Schedule K-1
- **Hayat Health LLC** — EIN 33-2127261 — 2025 net **−$26,602.24**. Revenue = Mindwell consulting
  ($90k, 1099-NEC). Has an $18,000 intercompany loan from/to Maven via Wise. Officer-comp flag.
- **SWEET HOME KC LLC** — EIN 93-2942628 — 2025 net **+$23,809.01**. 3 KC-MO rentals → **Form 8825**
  (rental real estate) → Sch K line 2, NOT ordinary income. Depreciation schedule needed.
- **On-Chain LLC** — EIN 82-3930173 — **dissolved end-2023**. Needs **back-year** 1120-S for
  2021–2024 (largely unbooked; coordinate with bookkeeping). 2023 was filed (+$26,680) — small-corp
  exception (receipts & assets < $250k → drop Sch L/M-2); final-year return + Final K-1.

## Line map to build
Income (ordinary trade/business): 1 gross receipts · 2 COGS · 6 total income · 7 officer comp ·
8 salaries · 12 taxes · 14 depreciation · 19 other deductions · 20 total deductions · 21 ordinary
business income. **Schedule K** (shareholder pro-rata: line 1 ordinary, line 2 net rental (8825),
interest/dividends/§179, distributions line 16d). **Schedule K-1** per shareholder. Sch B (questions,
incl. Q11/Q13 small-corp exception). Sch L/M-1/M-2 unless exception applies.

## Inputs the bridge must supply (per entity ledger)
revenue/expense by account → ordinary income lines; rentals → 8825 (per-property); officer
compensation (reasonable-comp!); distributions; shareholder = Michael Arbach 100%. See `tax/bridge/BRIDGE.md`.

## Done when
Each entity's ordinary income / rental income computes to its book net; K-1 totals tie; the K-1
amounts feed Michael's 1040 Schedule E; validate + PDF pass. SweetHome rentals land on 8825/Sch K line 2.
