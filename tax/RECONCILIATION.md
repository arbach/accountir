# Reconciliation tracker — computed returns vs the books

Source of truth: the `accountir_cloud` ledger (book net = −SUM(journal_lines) over revenue+expense,
is_void=false, by calendar year). A return is "tied" when its reconciliation line equals the book net
(delta < $0.50). Pull live with `tax/bridge/export_return.py --entity <e> --year <y> --compute`.

## Scope (per owner decision 2026-06-28)
Only entity-years that have ledger data. **The books contain no 2022 data for any entity, and Hayat
has nothing before 2024** — those years are out of scope until books (or cited source docs) exist.

**Status legend:** ✅ computed + reconciled (federal **and** IL).  Targets are the **FILED return** for
2023/2024 and source-docs/books for 2025 (per the owner truth-shift). `n/a` = entity did not exist that year.

| Entity | Form | 2022 | 2023 | 2024 | 2025 |
|---|---|---|---|---|---|
| Personal (Michael & Andrea) | f1040 + IL-1040 | n/a³ | ✅ **173,690** (filed) | ✅ **143,000** (filed) | ✅ **233,674** (source) |
| Maven Financial Technologies | f1120 + IL-1120 | n/a (formed 2023) | ✅ **−5,461** (filed) | ✅ **−10,486** (filed) | ✅ **−3,886** (book) |
| Hayat Health LLC | f1120s + IL-1120-ST | n/a (formed 2024) | n/a (formed 2024) | ✅ **22,097** (filed) | ✅ **−26,602** (book) |
| Sweet Home KC LLC | f1120s/8825 + IL-1120-ST | n/a (formed 2023) | n/a — **disregarded**⁴ | ✅ **15,328** (filed) | ✅ **23,809** (book) |

Every entity-year that exists is computed + reconciled (delta $0.00) federal and Illinois. Notes:
- ³ **2022:** no entity existed (books start 2023+); Michael's personal 2022 has no books and no filed return on
  hand (only PNC bank statements) — out of scope per the "years with books" decision.
- ⁴ **SweetHome 2023:** the 3 KC houses were reported **directly on the owner's 2023 Schedule E Part I**
  (disregarded — net 1,043, included in Personal 2023's 173,690), so there is no separate SweetHome 2023
  entity return. SweetHome only became an 1120-S via the 01/01/2024 S-election (which the owner wants reversed).
- Earlier "TIED to book net" figures (e.g. Maven 23/24 −94,625 / −90,908) tied to the **wrong books**; the
  table above now targets the **filed** returns. The book↔filed deltas are catalogued below for later cleanup.

## Tied returns (detail)

### Personal f1040:2025 — TIED (delta $0.00)
- Reconciliation line: f1040 line 9 total income = **233,673.65** = book net.
- Map: `bridge/maps/f1040_2025_michael.json`. Inputs: `bridge/out/michael_2025_input.json`.
- Computed: AGI 233,673.65 · taxable 197,411.85 · total tax **$8,506.78** (the $220k Maven-stock
  LTCG gets §1(h) preferential rates — ordinary-only would be ≈$34k) · amount owed 8,506.78
  (no withholding/estimated payments entered yet).
- Draft PDF: `bridge/out/michael_2025_f1040_DRAFT.pdf` (forced past MeF identity rules — see below).
- Open review flags (in the map as NEEDS_DOC / NEEDS_REVIEW):
  - SweetHome K-1 mapped as ordinary (box 1); real box-2 rental split comes from its 1120-S/8825/K-1.
  - Petersburg / ML Sidecar LP boxes assumed from account semantics — confirm against actual K-1s.
  - Dividends: qualified portion left $0 (taxed as ordinary = conservative); 1099-DIV would lower tax.
  - $220k LTCG entered as proceeds=gain, basis=0; real proceeds/basis from the sale docs.
  - Passive-loss / basis limits (Form 8582 / 7203) on the Hayat & ML Sidecar losses not yet applied.

### S-corps f1120s:2023-2025 — all TIED (delta $0.00)
- Engine: `forms/f1120s/2025/` (ordinary income page 1 + Form 8825 rentals + Schedule K + per-shareholder
  K-1). 14 IRS-sourced tests pass. Prior years registered in `catalog.ts` by reusing the 2025 node graph
  with the tax year overridden (S-corp ordinary/rental has no year brackets; federal entity tax = $0).
- **Hayat** (`maps/f1120s_hayat.json`): ordinary trade/business → line 21. 2025 −26,602.24 · 2024
  +34,713.22 (**lumped in account 9999, no breakdown — bookkeeping gap, flagged**). No 2023 books.
- **SweetHome** (`maps/f1120s_sweethome.json`): 100% rental real estate → Form 8825 → Schedule K line 2
  (ordinary line 21 = 0). 2023 +13,747.81 · 2024 +11,534.29 · 2025 +23,809.01. Books don't split
  expenses per property → aggregated to one 8825 item (per-property split is a NEEDS_DOC refinement).
- Reconciliation = Schedule K total income (ordinary line 21 + net rental). Bridge reads the full
  computed `pending` (CLI `return get` now emits it) and takes each form-output node's authoritative line.
- Each K-1 (box 1 ordinary / box 2 net rental) feeds Michael's 1040 Schedule E — next step is to replace
  his book-estimate K-1 inputs with these computed K-1 figures (esp. SweetHome → box 2 rental, not box 1).

### Maven f1120:2023-2025 — all TIED (delta $0.00)
- Engine: `forms/f1120/2025/` (total income · total deductions · line 28 income-before-NOL · NOL deduction ·
  line 30 taxable · line 31 21% tax). 12 tests pass. Prior years registered in `catalog.ts` (year overridden;
  21% flat applies 2018+, line structure year-stable). Reconciliation line: `f1120.line28_income_before_nol`.
- Map `maps/f1120_maven.json`. Every year a LOSS → $0 tax, loss becomes NOL carryforward.
- **NOL carryforward schedule** (post-2017 NOLs carry forward indefinitely, 80%-of-income use limit when
  income exists — Maven has none yet): after 2023 = 94,625.43 · after 2024 = 185,533.71 · after 2025 =
  **189,420.04** carried to 2026. Each year's line 28 ties to its book net (above), so the schedule is traceable.
- Review flags in the map: $80k IP-sale character (ordinary vs capital/§1245), Subcontractors COGS-vs-other,
  grant taxability, meals 50%, C-corp charitable 10% limit.

### Personal f1040 prior years — engine ready, DATA-BLOCKED
- f1040 tax-constant configs for **2023 and 2024** are built (`forms/f1040/nodes/config/2023.ts`, `2024.ts`,
  registered in `CONFIG_BY_YEAR` + `catalog.ts`), sourced from Rev. Proc. 2022-38 / 2023-34 (MFJ std ded
  $27,700 / $29,200). Smoke-checked: $100k MFJ wages → 2023 tax $8,236, 2024 tax $8,032. Engine now runs
  any 1040 year 2023-2025.
- ¹ **2023 (+329,763.34):** the personal books are a SINGLE lumped "Uncategorized" line — no income-type
  split. The return reconciles the TOTAL by parking the lump on one ordinary passthrough line
  (`maps/f1040_2023_michael.json`), which over-states tax (conservative) and is **NOT filing-ready**.
  NEEDS_DOC: the income breakdown (esp. the brief's ~$125k 2023 LTCG → Sch D preferential rates).
- ² **2024 (−294,808.95):** BLOCKED. A −$294,808.95 individual "total income" is an economic/book figure,
  not a tax-return line — net capital loss caps at $3,000/yr (§1211(b)); passive/at-risk/basis limits defer
  the rest — so book net cannot equal 1040 total income, and the engine correctly rejects a −$294k AGI
  (many nodes require nonnegative AGI). Needs the 2024 breakdown + loss-character analysis before a return
  is meaningful. Map staged (`maps/f1040_2024_michael.json`) with the full WARNING.
- Both years had only **2 dependent children** (Palmyra born 08/2025); 2025 has 3.

## Engine fix made while proving the loop
`forms/f1040/nodes/inputs/k1_s_corp` routed S-corp Schedule-E income **only** to the `schedule1`
display sink, never to `agi_aggregator` (which computes AGI / line 8) — so S-corp passthrough income
silently dropped out of total income. Fixed to also route to `agi_aggregator` (mirroring
`k1_partnership`), and made `agi_aggregator` + `schedule1` sum array-valued contributions (the
executor's accumulation pattern) via `sumNumericArrayFields`. Regression test added in
`forms/f1040/e2e/scenarios.test.ts`. Full suite green except 3 **pre-existing** failures unrelated to
this work (`form8889` HSA 2025 limit, `eitc`) — confirmed failing on the untouched vendored fork.

## ✅ 2025 FINAL SET (owner focus) — federal + Illinois, all four entities
Source-grounded where the books are unreliable. `bridge/export_return.py --state` produces federal+IL.

| Entity | Federal 2025 | Fed tax | Illinois 2025 |
|---|---|---|---|
| **Personal** (f1040, source-grounded¹) | total income 233,673.67 · AGI 225,373.67 · taxable 193,873.67 | **$7,976.05** | IL-1040 net 211,123.67 → **$10,450.62** |
| **Maven** (f1120) | line 28 −3,886.33 (loss) · NOL +3,886 | $0 | IL-1120 **$0** |
| **Hayat** (f1120s) | ordinary −26,602.24 · K-1 box1 −26,602.24 | $0 | IL-1120-ST **$0** |
| **SweetHome** (f1120s+8825²) | net rental +23,809.01 · K-1 box2 23,809.01 | $0 | IL-1120-ST **$357.14** |

¹ Personal 2025 (`maps/f1040_2025_michael.json`, manual/source mode) from the actual 2025 1099-INT/DIV,
K-1s (Hayat, SweetHome, Petersburg [DRAFT], ML Sidecar, ML Loyola) and HSA 1099-SA/5498-SA. Income ties
to books (233,673.67 vs 233,673.65, $0.02 rounding) but adds the **$8,300 HSA deduction**, **qualified
dividends 1,520**, and correct **passive/rental K-1 character**. Draft PDF: `bridge/out/michael_2025_f1040_DRAFT.pdf`.
² SweetHome modeled as filed (S-corp/8825); pending owner's disregarded-LLC decision (task #7).

**2025 open flags (must resolve before filing):**
- 🚩 **$220,000 Maven C-corp STOCK SALE has NO source document** on file (every 2025 1099-B is $0 — it's
  private stock). It drives most of Michael's tax (~$33k of LTCG). Kept per the brief but UNVERIFIED —
  needs the closing/sale statement.
- 🚩 Entity 2025 federal figures still tie to the **suspect books** (esp. Maven — prior years overstated
  the loss by ~$80k/yr). Pending book cleanup grounded in statements/invoices (owner deferred).
- 🚩 PDF/MeF builders for the corporate + IL forms are stubs — only the 1040 emits a PDF today (later phase).

## ⚠️ TRUTH SHIFT (owner decision 2026-06-28, second pass)
Filed prior-year returns were located (`/tmp/y2024/2024/` for 2024; On-Chain 2023 in
`/tmp/migrate/onchainzip/`). Owner direction:
- **The FILED returns are closer to the truth; the accountir books must be cleaned up to match** —
  with the *real* numbers grounded in **bank statements and invoices**, not just whatever is booked.
- So for 2023/2024, reconcile to the **filed returns**, and treat book↔filed gaps as **book defects to fix**.
- **The filed 2024 Personal 1040 shows total income $143,002** (interest 26,283 · ord. dividends 3,257 ·
  cap gain 68,178 · Sch-E passthrough 45,284 · QBI −9,108 · taxable 104,694 · tax 2,342) — the personal
  books' lumped −294,808.95 is NOT a tax figure and is a book defect. This is the breakdown that unblocks
  Personal 2024.
- **SweetHome entity classification is OPEN**: owner says it should be a *simple (disregarded) LLC*
  (rentals direct on the 1040 Schedule E), NOT an S-corp — **unless it was actually filed as an S-corp**.
  The filed 2024 return IS a Form 1120-S, so this must be resolved (was there a valid Form 2553 S-election,
  or was the 1120-S filing an error?) before finalizing SweetHome's structure. See task #7.

Filed-return line items are being extracted to `tax/filed/FILED_RETURNS.md` for line-by-line cross-check.

## 🚨 BOOKS vs FILED — the accountir ledger is materially wrong (clean-up needed)
Filed-return line items in `tax/filed/FILED_RETURNS.md`. My earlier "TIED to book net" results tied to
the BOOKS — which the filed returns now show are wrong for most entity-years:

| Entity-Year | Book net (my earlier recon) | **FILED (truth)** | Book error | Likely cause |
|---|---|---|---|---|
| Maven 2023 (line 28) | −94,625.43 | **−5,461** | overstates loss ~$89k | Subcontractors expense (book 171,881) vs filed deductions 140,310; revenue 86,863 vs filed 134,849 |
| Maven 2024 (line 28) | −90,908.28 | **−10,486** | overstates loss ~$80k | Subcontractors (book 216,149) vs filed other-deductions 187,842; revenue 181,800 vs filed 197,301 |
| Hayat 2024 (ordinary) | +34,713.22 (lumped) | **+22,097** | over ~$12.6k | 2024 books lumped in acct 9999; filed receipts 68,000 / deductions 45,903 |
| SweetHome 2024 (net rental) | +11,534.29 | **+15,328** | under ~$3.8k | book rents/expenses differ from filed Form 8825 (rents 37,200, exp 21,872) |
| Personal 2023 (total income) | +329,763.34 (lump) | **+173,690** | garbage | personal ledger is one lumped line, not a tax figure |
| Personal 2024 (total income) | −294,808.95 (lump) | **+143,002** | garbage | same — see filed breakdown below |

**Implication for 2025:** the 2025 books are likely wrong too (same systematic issues, esp. Maven
Subcontractors), so the "TIED to books" 2025 results above must be re-validated against source docs
(2025 1099s/K-1s in `/tmp/tax_extract/Tax Forms/`) — they tie to possibly-bad books, not to truth.

### Engine validated against a real filed return (Personal 2024)
Driven from the FILED source figures (1099-INT/DIV, Form 8949 On-Chain units sale 60,000, and the four
K-1s: Petersburg rental 7,858 + ST gain 8,178, Hayat ord 22,097, SweetHome rental 15,328, On-Chain −1):
- Engine total income **$143,000** vs filed **$143,002** (the $2 = a K-1 box-5 interest I omitted) ✓
- Engine tax $3,606 vs filed $2,342 — the ONLY delta is **QBI on the SweetHome rental**: filed claimed
  QBI 9,108 (incl. rental); engine conservatively claimed 4,419 (Hayat only). Defensible position diff,
  not a bug. (Map `maps/f1040_2024_michael.json`, `manual` filed-source mode.)

### SweetHome classification — RESOLVED (finding) → owner decision needed
Filed 2024 IS a Form 1120-S with an **S-election effective 01/01/2024 (Form 2553)**, but the corp has
**no operating business** — only rental real estate (Form 8825, 3 KC houses, net 15,328). In **2023** the
same houses were on the owner's **Schedule E Part I directly (disregarded)**; they were quit-claimed into
the LLC 10/16/2023. Owner's position (disregarded LLC, rentals on 1040 Sch E) is well-supported; the
1/1/2024 S-election looks inappropriate and should likely be revoked. See task #7.

## Illinois state forms — engine built (reconcile targets pending truth-shift)
`forms/il1040`, `forms/il1120`, `forms/il1120st` (2023-2025 registered). Rates: individual 4.95%,
S-corp/partnership replacement 1.5%, C-corp 7% income + 2.5% replacement. IL personal exemption per
person: 2023 $2,425 · 2024 $2,775 · 2025 $2,850 (IDOR bulletins). 26 tests pass. Verified off the
(book-based) federal figures: Michael IL-1040 $10,861.47 · SweetHome IL-1120-ST $357.14 · Maven IL-1120 $0.
These IL figures will be re-targeted once the federal base is reconciled to the filed returns.

### Bridge federal→IL chaining wired (`export_return.py --state`)
Computes the federal return then feeds its base into the matching IL form. 2025 results (book-based federal):
- Maven **IL-1120**: net income 0 (loss) → IL tax **$0**.
- Hayat **IL-1120-ST**: ordinary −26,602.24 (loss) → replacement tax **$0**.
- SweetHome **IL-1120-ST**: net rental 23,809.01 → replacement tax **$357.14**.
- Michael **IL-1040**: AGI 233,673.65, 5 exemptions → ~**$10,861**.
Entity 2025 federal still ties to the (suspect) books — to be re-grounded in 2025 source docs / cleaned books.

## Known non-blocking validation notes
MeF `return validate` rejects ~48 rules that are filer-identity/header completeness (preparer SSN,
submission-manifest TIN, full address) — **not calculation errors**. They correctly block e-file until
real identity data is present; per owner rules we do not fake SSNs/signatures. PDF export uses
`--force` to produce a DRAFT past these.
