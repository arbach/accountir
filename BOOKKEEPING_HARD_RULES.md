# Bookkeeping HARD RULES — Arbach entities (canonical classification ruleset)

The rules we apply consistently when classifying transactions. Owner-confirmed; update as new
rules are set. Last updated 2026-06 (this engagement).

## 0. Governing principles
- **Over-pay to stay safe; never underpay the IRS; claim every *legitimate* deduction.**
- Books must reconcile **to actuals (bank/Wise/chain statements)** AND **to the filed returns** where a return was filed; the filed return is the authority for a filed year.
- **Every figure traces to a statement or source document.** Never invent numbers.
- **No lump sums** — itemize per vendor / per payment.
- Clearing accounts (**9000 / 9999 / 2500 Suspense**) must net to ~$0 for the books to be "done."
- Ambiguous items are flagged for the owner, not guessed.

## 1. Amounts
- Use the **CHARGED (larger, fee-inclusive) amount** for Wise/crypto, not the net received — the fee is itself a deductible expense. (Wise shows two numbers; take the larger.)

## 2. Wise
- **Wise outflow to a third party = contractor expense (5300)** — itemized per vendor, and **every payment must be identified** (named recipient).
- **bank → Wise (Chase/PNC → Wise) = funding / transfer** into the 1010 Wise account, NOT expense.
- **Wise → our own bank / own exchange = transfer** (not expense).

## 3. Crypto
- **Crypto OUT to a contractor wallet (EOA) = expense (5300)**, itemized per wallet/vendor.
- **To our OWN exchange/wallet (e.g. Kraken `0x3d1b…` = "MavenFin @Kraken"), or a bridge to our own wallet = transfer**, NOT expense. (Verify with `eth_getCode` / own-wallet list.)
- **DEX swap** (same-tx in+out, e.g. USDC↔USDT) = NOT a payment — nets out, no P&L.
- **Crypto IN:** client payment → **Revenue (4xxx)**; loan → **Loan Payable (2490 crypto)**; from our own wallet/bridge → **transfer** (net to source); capital → **Equity**.

## 4. Brokerage accounts  ← (added 2026-06, owner)
- **Fidelity and Robinhood are PERSONAL brokerage accounts.**
- **Money going TO Fidelity/Robinhood = first a LOAN PAYBACK** (if a loan from that source is outstanding) → Dr Loan Payable; **if no outstanding loan, it's a DISTRIBUTION** → Dr Owner Distributions (equity). Never income/expense.
- **Bank-statement aliases that all = Fidelity** (confirmed 2026-06): **"Fid Bkg Svc LLC Moneyline"**, **"National Financial Services LLC"** (Fidelity's clearing broker-dealer; "B/O National Financial Services" = a Fidelity transfer), and **"Wire Via CIBC Bank USA"** (a brokerage wire routed through CIBC). Treat all three as Fidelity → apply the loan-payback-then-distribution rule.

## 5. Entity attribution
- **On-Chain LLC dissolved end-2023** → 2024 On-Chain activity rolls to **Maven**.
- On-Chain contractors paid via **Maven's Chase** in 2023 are still **On-Chain's** expense (intercompany).

## 5b. Intercompany transfers  ← (added 2026-06, owner)
- **Any transfer between the owner's companies** (Maven · On-Chain · Hayat · SweetHome · Michael personal)
  = **first a LOAN PAYBACK** (reduce the existing intercompany loan between those two parties, if one is
  outstanding); **if none outstanding, it's a new LOAN.** Never income, expense, or distribution *between* companies.
- **Maintain a clear intercompany debt ledger** (who owes whom) — every intercompany move updates it.
  Use the `Due to/from <Entity>` / `Loan Payable/Receivable - <Entity>` accounts; net them per counterparty.

## 6. Distributions / owner
- **All entities are S-corp or C-corp — there are NO "draws."** Owner money out = **S-corp distribution** / **C-corp dividend** / **shareholder loan** (per the loan ledger), never a "draw," never expense.
- Confirmed: 2023 On-Chain $220k→Robinhood + $100k transfer = **distributions**.

## 6c. Hayat SOFT rules (entity-specific)
- **Hayat "Remote Deposit" / large check deposit > $10,000 = Service Revenue** (Mindwell pays by check). Under $10k = review.
- A deposit that is a **check earned in a prior year** → **Prior Year Revenue (4100)**, with a note (e.g. "Jun-2025 $20k = check from 2024"; "Jun-2026 $30k = Service Revenue for 2025").

## 6d. Brokerage — the loan STAYS; outflows are mostly distribution (clarifies §4)
- **We still owe ~$180,000 to the brokerage (Fidelity loan) — it STAYS outstanding.** Brokerage
  transfers do NOT auto-net against it.
- **Most money OUT to Fidelity/Robinhood = DISTRIBUTION** (profits already distributed from the
  entities, then personally invested) — **not** loan payback. Only a *specifically identified*
  payback reduces the loan; default outflow = distribution / personal investment.
- Net brokerage flow (2024–2026) = **$525,087 out − $400,000 in = $125,087 net out** (not $625k).

## 6b. Account naming
- **Hayat revenue is "Service Revenue" (generic) — NEVER "Medical Revenue."** Hayat earns generic
  consulting/service revenue (e.g. Mindwell). Do not use "Medical Revenue" anywhere (memos,
  accounts, forms). (Enforced 2026-06.)

## 7. Loans
- Crypto/DeFi inflows that are borrowings (e.g. MakerDAO mint, lender wallets) = **Loan Payable**, not income. The off-ramp of borrowed funds to the bank is **not** taxable income.
