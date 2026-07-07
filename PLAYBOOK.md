# Accountir Bookkeeping Playbook

Canonical operating guide for **both** (a) any Claude session working these books and (b) the in-app chat agent (`agentd`). Keep this in sync with the two rule memories: `bookkeeping-hard-rules` and `evidence-first-classification`.

---

## 0. Golden rules (never violate)
1. **Trial balance = $0 always.** Every entry's lines sum to zero (positive = debit, negative = credit). Verify after every batch.
2. **USD only.** No FX blending; the ledger rejects non-USD lines.
3. **Statements / blockchain scanner are the truth.** Never guess against them; reconcile to the penny (fiat) or to the scanner (crypto).
4. **Event-sourced ledger.** Post through the app's `post_entry` path (emits a hash-chained `journal_entry_posted` event + projection). Prefer **void** over reversal. A hard DELETE leaves the event orphaned (recoverable from the `events` table — that's how deleted entries were restored).
5. **Never mix entities.** A Hayat tx never lands in Personal, etc. Transfers between an owner's own accounts/wallets are **balance-sheet transfers, never P&L**.
6. **No silent 9999.** Uncategorized is a review queue, not a dumping ground. No lump sums — itemize every transaction.

## 1. The Arbach entity map
| Entity | Type / form | Owner | Notes |
|---|---|---|---|
| **Maven Financial Technologies** | C-corp, 1120 | Michael 100% (President) | Crypto contractor payments (Wise + MetaMask). **On-Chain** (dissolved 2023) rolled in as successor. IP sale + consulting revenue. |
| **Hayat Health LLC** | S-corp, 1120-S | Michael 100% (managing member) | **Accrual.** Service Revenue = **Mindwell** consulting (NOT "Medical"). Statement-verified (Chase …5272 + card …5612). |
| **SweetHome KC LLC** | S-corp, 1120-S + 8825 | Michael 100% | Rental real estate; IL 1.5% replacement tax. |
| **Personal — Michael & Andrea Arbach** | 1040 MFJ | — | K-1 flow-throughs + $220k LTCG. Checking = Chase …7001. **1040 is source-grounded from the actual K-1s/1099s, NOT the personal ledger** (the ledger is a lumped defect). |
| **On-Chain LLC** | dissolved 2023 | — | Rolled to Maven; retains only 2023 filed history. |
| **Mindwell Inc** | — | **client, not an owned entity** | Pays Hayat. Has its own login (email/pass reset). |

## 2. Evidence-first resolution ladder (run BEFORE asking the owner anything)
1. **Address book** (`address_labels` / `list_address_labels`): counterparty labeled? `kind` (lender/income/contractor/own/exchange/swap) + `account_code` = the answer. *(The Maven "negative wallet" source — $45k Merit 7 = lender, Patrick Maguire = income — was fully in the book.)*
2. **Prior similar tx**: same counterparty/memo booked before → book it the **same way** (kills "inconsistent-classification").
3. **Source of truth**: the statement (fiat) or scanner (crypto) — confirm amount + direction.
4. **Domain rules** (§3 below).
5. **Only then ask**, phrased as the specific gap — and **persist the answer** (`set_address_label` / vendor-default) so it's never re-asked.

## 3. Classification hard rules
- **Fidelity = National Financial = CIBC** (same brokerage custody).
- **Brokerage ↔ checking transfer → Investments (1500).** No P&L, not a loan. (e.g. Robinhood/NatlFinancial credits.)
- **Wise = contractor payment → Subcontractors (5300).** Itemize each per-vendor; never a lump.
- **Crypto by address-book kind:** lender → **Loan Payable-Crypto (2490)**; income → Revenue; contractor → 5300; own → internal transfer; exchange/swap → within crypto.
- **Clearing / suspense must net ~0.** A standing non-zero clearing balance is an error.
- **Hayat revenue = Service Revenue** (Mindwell), not Medical.
- **Parse the FULL memo:** "ORIG CO NAME:PROVIDERSCAREBIL" = "Providers Care Billing", not "Providersca".
- **Credit-card payments are transfers, not expenses** (the charges are the expenses).

## 4. Reconciliation discipline
- After any import/batch: compare ledger tx count vs statement count; re-run the income statement; flag anomalies ($0 revenue vs big inflows, negative expenses, double-counted card payments).
- **Balance tie-out per statement period** (opening + net = closing, to the penny). Crypto ties to the scanner.
- Same amount in two accounts within ~3 days = one transfer OR a double-import — resolve before it distorts the books.

## 5. Known traps (learned the hard way)
- **Plaid one-login pulls duplicate other accounts into an entity** (Hayat's …5272 landed in Personal). Dedup by matching against the *other* entity's real books; the **statement** decides which copy is real.
- **Do not dedup by "same date+amount+direction" blindly** — it deleted real 7001 withdrawals. Reconcile to the statement instead.
- **Account names can be mislabeled** (Personal "…5272" was really the 7001 checking — the statement filename proved it).
- **Crypto "negative wallet" ≠ missing money** — usually inflows booked backwards or loan inflows not credited to 2490. Check the scanner + address book.

## 6. Tax process (OpenTax engine)
- Bridge at `/usr/local/lib/accountir/tax/bridge/`; maps in `maps/` (e.g. `f1040_2025_michael.json`). Personal 1040 is `manual: true` — fed from **source K-1s/1099s**, reconciled to a target, NOT the ledger.
- 7-step wizard: profile → tag accounts → compute → pull+fill forms → approve → sign → mail. `tax_account_lines` = account→line tags (don't clobber).
- Entity nets → K-1s → Personal Schedule E. Passive vs non-passive matters (Form 8582): Hayat non-passive (material participation); LP rentals (SweetHome/Petersburg/ML Sidecar) passive — net passive income must cover passive losses.
- **Extensions filed (2025)** → no late-filing/§6699. Residual risk = estimated-tax underpayment (§6654), but small because 2024 tax was low ($2,342 → safe harbor). Pay balances now to stop FTP + interest.

## 7. The auditor — `accountir-recon`
Independent, **read-only** tool (`/home/ubuntu/repos/accountir-recon`), `recon_ro` DB role, Claude via CLI. Verifies every tx against statements/scanner; emits `reports/tx_needing_attention.md`. It is the arbiter — run it to check work; fix from its findings, not from heuristics.

## 8. Response style (ESPECIALLY the in-app chat agent)
- **Very short. Lead with the answer or the result.** No preamble, no restating the question, no "I'll now…".
- One line per action taken. A tiny table/list only when it genuinely helps.
- Multi-paragraph explanations only when the user explicitly asks for depth.
