# Chart-of-Accounts Change Request — tax-alignment

> Generated from `coa_gap.py`. These are **bookkeeping-side** changes. accountir is event-sourced with a cryptographic audit chain, so apply them **through the app**, not by writing `accounts`/`journal_lines` directly. Items marked **needs source doc** must not be guessed — pull the document first. The tax engine already reflects the intended result via split tags, so the 2025 returns are correct before the books are physically restructured.

**Summary:** 29 accounts to add · 19 structural fixes across 3 entities.


## Maven — f1120 (template: c_corp)

Coverage: **22/28** tax-lined categories have a dedicated account.


### Accounts to add

| New # | Account | Tax line | Why |
|---|---|---|---|
| 4010 | Returns & Allowances | 1b | so returns transactions land on line 1b |
| 4400 | Dividend Income | 4 | so dividend income transactions land on line 4 |
| 4600 | Gross Rents | 6 | so rental income transactions land on line 6 |
| 4800 | Capital Gain Net Income | 8 | so capital gain income transactions land on line 8 |
| 6120 | Officer Compensation | 12 | so officer comp transactions land on line 12 |
| 6330 | Interest Expense | 26 | so interest expense transactions land on line 26 |

### Structural fixes (split / reclassify / verify)

- **5300 Subcontractors (5300) — Split into Officer Compensation vs Contract Labor** **[needs source doc]**
  Create a dedicated **Officer Compensation** account and move the owner's own pay there (it becomes W-2 wages on line 7 and must run through payroll). Leave only true third-party 1099 contractors in Contract Labor. Tax total is unchanged; the *character* changes and S-corp reasonable-comp compliance is satisfied.
- **6210 Meals & Entertainment (6210) — Keep Meals in a dedicated 50% account**
  Book meals to a single Meals (50%) account so the §274(n) limit applies mechanically and the disallowed half is a clean M-1 add-back. Split off any 100%-deductible items (e.g. company events) into their own account.
- **6400 Reimbursable Expense (6400) — Break out the uncategorized / miscellaneous lump** **[needs source doc]**
  A catch-all account can't be placed on a return with confidence. Re-code its transactions to the specific expense accounts so each lands on its proper line.
- **6800 Charitable Donations (6800) — Route charitable to a separately-stated account**
  For an S-corp, charitable contributions are separately stated on Schedule K (they don't reduce ordinary income); for a C-corp they're limited to 10% of taxable income. Give them their own account.
- **7050 Miscellaneous (7050) — Break out the uncategorized / miscellaneous lump** **[needs source doc]**
  A catch-all account can't be placed on a return with confidence. Re-code its transactions to the specific expense accounts so each lands on its proper line.
- **7150 Other Expense (7150) — Break out the uncategorized / miscellaneous lump** **[needs source doc]**
  A catch-all account can't be placed on a return with confidence. Re-code its transactions to the specific expense accounts so each lands on its proper line.
- **8100 Bank Revaluations (8100) — Confirm treatment of FX / revaluation accounts** **[needs source doc]**
  Unrealized currency revaluations are usually book-only and non-deductible (an M-1 item). Confirm which portion is a realized gain/loss before it touches the return.
- **8150 Unrealized Currency Gains (8150) — Confirm treatment of FX / revaluation accounts** **[needs source doc]**
  Unrealized currency revaluations are usually book-only and non-deductible (an M-1 item). Confirm which portion is a realized gain/loss before it touches the return.
- **8200 Realized Currency Gains (8200) — Confirm treatment of FX / revaluation accounts** **[needs source doc]**
  Unrealized currency revaluations are usually book-only and non-deductible (an M-1 item). Confirm which portion is a realized gain/loss before it touches the return.

## Hayat — f1120s (template: s_corp)

Coverage: **17/31** tax-lined categories have a dedicated account.


### Accounts to add

| New # | Account | Tax line | Why |
|---|---|---|---|
| 4010 | Returns & Allowances | 1b | so returns transactions land on line 1b |
| 4700 | Other Income (Page 1) | 5 | so other income transactions land on line 5 |
| 5000 | Cost of Goods Sold | 2 | so cogs transactions land on line 2 |
| 6070 | Officer Compensation | 7 | Owner W-2 pay — S-corp reasonable-comp. Keep SEPARATE from contract labor. |
| 6080 | Salaries & Wages (non-officer) | 8 | so wages transactions land on line 8 |
| 6090 | Repairs & Maintenance | 9 | so repairs transactions land on line 9 |
| 6100 | Bad Debts | 10 | so bad debt transactions land on line 10 |
| 6130 | Interest Expense | 13 | so interest expense transactions land on line 13 |
| 6140 | Depreciation (Form 4562) | 14 | so depreciation transactions land on line 14 |
| 6170 | Pension, Profit-Sharing Plans | 17 | so pension transactions land on line 17 |
| 6180 | Employee Benefit Programs | 18 | so benefits transactions land on line 18 |
| 6350 | Utilities | 19 | so utilities transactions land on line 19 |
| 8100 | Interest Income (Schedule K) | K-4 | so interest income transactions land on line K-4 |
| 8110 | Dividend Income (Schedule K) | K-5a | so dividend income transactions land on line K-5a |

### Structural fixes (split / reclassify / verify)

- **4100 Prior Year Revenue (2024 Collections) — Confirm cash vs accrual on prior-year collections** **[needs source doc]**
  Revenue collected this year for prior-year work is current-year gross receipts on the cash basis. Confirm the entity's method so the timing is right.
- **5200 Medical Billing - Providersca — Confirm gross vs net revenue (platform/billing fees)** **[needs source doc]**
  If the platform withholds its fee before depositing, the booked revenue is NET. Either gross up revenue and keep the fee as an expense, or book the fee as contra-revenue — but do not do both (double-count). This likely flips the entity from a loss to a profit.
- **5300 Medical Platform - Zocdoc — Confirm gross vs net revenue (platform/billing fees)** **[needs source doc]**
  If the platform withholds its fee before depositing, the booked revenue is NET. Either gross up revenue and keep the fee as an expense, or book the fee as contra-revenue — but do not do both (double-count). This likely flips the entity from a loss to a profit.
- **5400 Contractor Payments — Split into Officer Compensation vs Contract Labor** **[needs source doc]**
  Create a dedicated **Officer Compensation** account and move the owner's own pay there (it becomes W-2 wages on line 7 and must run through payroll). Leave only true third-party 1099 contractors in Contract Labor. Tax total is unchanged; the *character* changes and S-corp reasonable-comp compliance is satisfied.
- **5900 Donations & Miscellaneous — Route charitable to a separately-stated account**
  For an S-corp, charitable contributions are separately stated on Schedule K (they don't reduce ordinary income); for a C-corp they're limited to 10% of taxable income. Give them their own account.
- **6200 Meals & Entertainment — Keep Meals in a dedicated 50% account**
  Book meals to a single Meals (50%) account so the §274(n) limit applies mechanically and the disallowed half is a clean M-1 add-back. Split off any 100%-deductible items (e.g. company events) into their own account.
- **7203 Travel - Meals — Keep Meals in a dedicated 50% account**
  Book meals to a single Meals (50%) account so the §274(n) limit applies mechanically and the disallowed half is a clean M-1 add-back. Split off any 100%-deductible items (e.g. company events) into their own account.
- **9999 Uncategorized — Break out the uncategorized / miscellaneous lump** **[needs source doc]**
  A catch-all account can't be placed on a return with confidence. Re-code its transactions to the specific expense accounts so each lands on its proper line.

## Sweethome — f1120s (template: s_corp_rental)

Coverage: **5/14** tax-lined categories have a dedicated account.


### Accounts to add

| New # | Account | Tax line | Why |
|---|---|---|---|
| 6200 | Advertising — Rental | 8825-3 | so advertising transactions land on line 8825-3 |
| 6210 | Auto & Travel — Rental | 8825-4 | so travel transactions land on line 8825-4 |
| 6220 | Cleaning & Maintenance | 8825-5 | so cleaning transactions land on line 8825-5 |
| 6230 | Commissions | 8825-6 | so commissions transactions land on line 8825-6 |
| 6240 | Insurance — Rental | 8825-7 | so insurance transactions land on line 8825-7 |
| 6250 | Legal & Professional — Rental | 8825-8 | so professional fees transactions land on line 8825-8 |
| 6260 | Mortgage Interest — Rental | 8825-9 | INTEREST ONLY. Principal is not deductible — book it to the loan liability, never here. |
| 6290 | Utilities — Rental | 8825-13 | so utilities transactions land on line 8825-13 |
| 6300 | Wages & Salaries — Rental | 8825-14 | so wages transactions land on line 8825-14 |

### Structural fixes (split / reclassify / verify)

- **5000 Mortgage Payments - Fidelity (P&I undifferentiated) — Split Mortgage Payment into Interest vs Principal** **[needs source doc]**
  Principal is NOT a P&L expense — it pays down the loan liability on the balance sheet. Split each payment: interest → the deductible interest account (8825 line 9); principal → the mortgage loan liability. Pull the amortization schedule to get the split per period.
- **5500 Miscellaneous Business Expenses — Break out the uncategorized / miscellaneous lump** **[needs source doc]**
  A catch-all account can't be placed on a return with confidence. Re-code its transactions to the specific expense accounts so each lands on its proper line.


---
_After the books are updated, re-run `tag_accounts.py --all` and `sync_tax_lines.py --all`; confirmed tags carry forward and only the changed accounts refresh._
