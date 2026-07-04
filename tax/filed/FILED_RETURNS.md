# Filed / Prepared Tax Returns — Extracted Line Items

Source: filed-return PDFs parsed with `pdftotext -layout` (and `tesseract` OCR where noted).
All figures are the **filed/prepared figure per line**, exact dollars. Prepared in TurboTax / TaxWise (self-prepared where shown).

Entities owned by Michael Arbach (SSN 813-14-0923); spouse Andrea Arbach (SSN 402-37-4451 / 4451).

---

## Personal 2024 — Form 1040 (TurboTax, filed Oct 15)

File: `/tmp/y2024/2024/Personal/Personal Fed + State Filled Oct 15.pdf`
Taxpayers: michael & andrea arbach. **Filing status: Married Filing Jointly** (std deduction $29,200; both names on return).
Dependents: Alexander Arbach (son, CTC), Suria Arbach (daughter, CTC).

### Form 1040
| Line | Item | Amount |
|---|---|---|
| 1z | Wages (W-2 box 1) | 0 |
| 2b | Taxable interest | 26,283 |
| 3a / 3b | Qualified / Ordinary dividends | 1,292 / 3,257 |
| 4b / 5b / 6b | IRA / Pension / Social Security taxable | 0 / 0 / 0 |
| 7 | Capital gain (Schedule D) | 68,178 |
| 8 | Additional income (Schedule 1 line 10) | 45,284 |
| **9** | **Total income** | **143,002** |
| 10 | Adjustments | 0 |
| **11** | **Adjusted gross income** | **143,002** |
| 12 | Standard deduction | 29,200 |
| 13 | QBI deduction (Form 8995) | 9,108 |
| 14 | Lines 12 + 13 | 38,308 |
| **15** | **Taxable income** | **104,694** |
| 16 | Tax | 6,344 |
| 18 | Lines 16+17 | 6,344 |
| 19 | Child tax credit (Sch 8812) | 4,000 |
| 20 | Sch 3 line 8 | 2 |
| 22 | Subtract 21 from 18 | 2,342 |
| 23 | Other taxes (SE tax etc.) | 0 |
| **24** | **Total tax** | **2,342** |
| 26 | 2024 estimated payments | 15,000 |
| 33 | Total payments | 15,000 |
| 34 | Overpayment | 12,658 |
| 35a | Refunded | 12,566 |
| 36 | Applied to 2025 | (remainder) |
| 38 | Estimated tax penalty | 92 |

### Personal 2024 income breakdown (components of $143,002 total income)
| Component | Amount |
|---|---|
| Taxable interest (2b) | 26,283 |
| Ordinary dividends (3b) | 3,257 |
| Capital gain (line 7) | 68,178 |
| Schedule 1 line 10 → Schedule E (line 8) | 45,284 |
| **Total income (line 9)** | **143,002** |

### Schedule D / Form 8949 (2024)
| | Amount |
|---|---|
| Line 5 — Net **short-term** gain from K-1s (partnerships/S-corps) | 8,178 |
| Line 7 — Net short-term capital gain | 8,178 |
| Line 10 — **Long-term**, Form 8949 **Box F** (not reported on 1099-B) | proceeds 60,000 / basis 0 / gain 60,000 |
| Line 15 — Net long-term capital gain | 60,000 |
| **Line 16 — Total capital gain** | **68,178** |

Form 8949 Part II (Box F) detail: "Sold all Shares in On-Chain LLC" acquired 03/01/21, sold 07/07/24 — two lines of $30,000 (proceeds 60,000, basis 0, gain 60,000). The $8,178 short-term came via K-1 (see Petersburg below).

### Schedule 1 (2024)
- Line 3 (Schedule C business income): **0**
- Line 5 (Rental RE, partnerships, S-corps — Schedule E): **45,284**
- Line 10 (total additional income): **45,284**

### Schedule E detail (2024) — Part II Partnerships & S corporations
| | Entity | Type | Passive income (h) | Nonpassive loss (i) | Nonpassive income (k) |
|---|---|---|---|---|---|
| A | PETERSBURG PLACE INVESTORS LLC (84-3764182) | P (partnership) | 7,860 | | |
| B | On-Chain LLC (82-3930173) | S | | 1 | |
| C | Sweet Home KC (93-2942628) | S | 15,328 | | |
| D | Hayat Health LLC (33-2127261) | S | | | 22,097 |

- Line 29a totals: passive income (h) 23,188 · nonpassive income (k) 22,097
- Line 29b totals: nonpassive loss (i) 1
- Line 30 (h+k): 45,285 · Line 31 (loss): (1)
- **Line 32 / Line 41 — Total partnership & S-corp income: 45,284**

Note on character: Sweet Home KC income ($15,328) is reported as **passive rental** income (K-1 box 2). Hayat ($22,097) is **nonpassive ordinary** (K-1 box 1). On-Chain is a $1 nonpassive ordinary loss (entity winding down). Petersburg ($7,860) is passive.

### IL-1040 (2024)
| Line | Item | Amount |
|---|---|---|
| 1 / 4 | Federal AGI / Total income | 143,002 |
| 9 | Illinois base income | 143,002 |
| 10 | Exemption allowance | 11,100 |
| 11 | Net income | 131,902 |
| 12 | Tax (4.95%) | 6,529 |
| 19 / 23 | Total tax | 6,529 |
| 26 | Estimated payments | 4,500 |
| 33 / 41 | Amount owed | 2,029 |

---

## Personal 2023 — Form 1040 (TurboTax)

File: `/tmp/migrate/taxall/Tax/2023/2023_TaxReturn (1).pdf` (also identical copy in `taxzip`).
Filing status: **Married Filing Jointly** (std deduction $27,700).

### Form 1040 (2023)
| Line | Item | Amount |
|---|---|---|
| 1z | Wages | 0 |
| 2b | Taxable interest | 8,619 |
| 3a / 3b | Qualified / Ordinary dividends | 1,123 / 10,987 |
| 7 | Capital gain (Schedule D) | 126,361 |
| 8 | Additional income (Sch 1 line 10) | 27,723 |
| **9** | **Total income** | **173,690** |
| **11** | **AGI** | **173,690** |
| 12 | Standard deduction | 27,700 |
| 13 | QBI deduction | 3,701 |
| **15** | **Taxable income** | **142,289** |
| 16 | Tax | 9,439 |
| 19 | Child tax credit | 0 (blank) |
| 20 | Sch 3 line 8 | 1 |
| 22 | | 9,438 |
| **24** | **Total tax** | **9,438** |

Income breakdown (= 173,690): interest 8,619 + dividends 10,987 + capital gain 126,361 + Schedule E 27,723.

### Schedule D (2023)
- Net short-term (line 7): 0
- Long-term Box E: proceeds 1,561 / basis 200 / gain 1,361
- Long-term **Box F (not on 1099-B): proceeds 125,000 / basis 0 / gain 125,000** (On-Chain LLC units sale)
- Line 15 net long-term: 126,361 · **Line 16 total: 126,361**

### Schedule E (2023)
- Part I rental real estate (line 26): **1,043** (net: income 11,244 less losses 10,201 across rental properties incl. the Kansas City houses — reported directly as rentals, i.e. **disregarded**, NOT through an S-corp in 2023)
- Part II S corporations (line 32): **26,680** — On-Chain LLC (82-3930173), nonpassive ordinary (K-1 box 1 = 26,680)
- **Line 41 total: 27,723**

### IL-1040 (2023)
| Item | Amount |
|---|---|
| Base income (line 9) | 173,690 |
| Exemption allowance (line 10) | 4,850 |
| Net income (line 11) | 168,840 |
| Tax 4.95% (line 12) | 8,358 |
| Total tax (line 23) | 8,358 |

---

## Hayat Health LLC 2024 — Form 1120-S (S corp)

File: `/tmp/y2024/2024/Hayat/2024 Hayat Health LLC Form 1120S  S Corps Tax Return_Filing.pdf`
EIN **33-2127261**. S corporation (Form 2553 election referenced). Short year K-1 (07/19/2024–12/31/2024).

| Line | Item | Amount |
|---|---|---|
| 1a / 1c | Gross receipts | 68,000 |
| 2 | COGS | 0 |
| 3 | Gross profit | 68,000 |
| **6** | **Total income** | **68,000** |
| 7 | Compensation of officers | 0 |
| 8 | Salaries & wages | 0 |
| 12 | Taxes & licenses | 0 |
| 14 | Depreciation | 0 |
| 16 | Advertising | 2,537 |
| 20 | Other deductions (statement) | 43,366 |
| **21** | **Total deductions** | **45,903** |
| **22** | **Ordinary business income** | **22,097** |

**Schedule K:** line 1 ordinary 22,097; line 2 net rental 0; interest/dividends/§179 0; 16d distributions 0.
**Schedule K-1:** shareholder Michael Arbach, **100%**, **box 1 = 22,097**, box 2 = 0, box 16d distributions = 0.

---

## SweetHomeKC LLC 2024 — Form 1120-S (filed as S corp) — *see filing-status finding below*

Files: `/tmp/y2024/2024/Sweet Home/2024 SweetHomeKC LLC Form 1120S  S Corps Tax Return_Filing.pdf` (+ `_Records.pdf`)
EIN **93-2942628**.

| Line | Item | Amount |
|---|---|---|
| 1a–6 | Gross receipts / COGS / Total income (page 1) | **0 (blank)** |
| 21 | Total deductions (page 1) | 0 |
| 22 | Ordinary business income | **0** |
| Sch K line 1 | Ordinary business income | 0 |
| **Sch K line 2** | **Net rental real estate income (Form 8825)** | **15,328** |
| Sch K line 18 | Income (loss) | 15,328 |
| 16d | Distributions | 0 |

**Form 8825 (rental real estate) — 3 Kansas City–area properties:**
| Property | Gross rents | Total expenses | Net income |
|---|---|---|---|
| A — 4023 E 115th, Kansas City MO 64137 | 12,000 | 7,577 | 4,423 |
| B — 11211 Norby Rd, Kansas City MO 64137 | 13,200 | 8,620 | 4,580 |
| C — 13209 Fuller Ave, Grandview MO 64030 | 12,000 | 5,675 | 6,325 |
| **18a / 18b / 21 Totals** | **37,200** | **(21,872)** | **15,328** |

**Schedule K-1:** shareholder Michael Arbach, **100%**, **box 1 = 0, box 2 (net rental) = 15,328**, box 16d distributions = 0.

### SweetHome filing-status finding
- **SweetHomeKC LLC is FILED as an 1120-S (S corporation).** The return is the IRS "U.S. Income Tax Return for an S Corporation," and an **S-election is indicated: "S election effective date 01/01/2024"** with the box "is attaching Form 2553 to elect to be an S corporation" present. EIN **93-2942628**.
- **The S-corp has NO operating business** — page 1 (gross receipts, COGS, total income, ordinary business income line 22) is entirely blank/zero. Its only activity is **rental real estate on Form 8825** (3 KC houses), flowing to **Schedule K line 2 = $15,328** (net rental), and to the owner's Schedule E as **passive** income.
- **Supporting history:** In **2023** these same Kansas City properties were reported directly on the owner's **Schedule E Part I as rental real estate** (disregarded — line 26 = $1,043 net for all rentals), i.e. NOT through an S-corp. The 3 properties were quit-claimed from Michael Arbach personally into "SWEET HOME KC, LLC" by deeds recorded **10/16/2023** (Jackson County, MO — see the `2023E0074967/968/969` deeds). The S-election then took effect 01/01/2024.
- **Determination:** Electing S-corp status for a single-member LLC that holds nothing but rental real estate is generally inappropriate and disadvantageous — rentals are passive and self-rental/passive-loss rules and reasonable-comp issues do not benefit from S status; a single-member rental LLC is normally a **disregarded entity** reporting on Schedule E of the 1040. The filed treatment (1120-S via a 1/1/2024 Form 2553) **conflicts with the owner's belief that SweetHome should be a disregarded LLC**, and the owner's position is well-supported by the 2023 reporting and the nature of the activity. (Reversing would require revoking the S-election / correcting the entity classification.)

---

## On-Chain LLC 2023 — Form 1120-S (S corp)

Files: original/as-filed client copy `/tmp/migrate/taxall/Tax/2023/OnChain/2023 Tax Return (ON-CHAIN LLC)-CLIENT COPY.PDF`; self-prepared amended/final package in `/tmp/migrate/onchainzip/onchain/`.
EIN **82-3930173**. 100% shareholder Michael Arbach.

| Line | Item | Amount |
|---|---|---|
| 1a / 1c | Gross receipts | 132,729 |
| **6** | **Total income** | **132,729** |
| **21** | **Total deductions** | **106,049** |
| **22** | **Ordinary business income** | **26,680** |
| Sch K line 2 | Net rental | 0 |

**Schedule K-1 (original):** box 1 = 26,680; box 16d distributions = blank.
**Schedule L (original):** line 7 Loans to shareholders end-of-year **686,107** ← flagged as erroneous.

Amended/FINAL package (self-prepared, `ON-CHAIN_2023_FINAL_FILING_PACKAGE`): income/tax **unchanged** (ordinary 26,680). Corrections: removes bogus $686,107 "loan to shareholder" (was actually distributions); restates Schedule L (total assets 25,409.68; liabilities 43,557.29; equity (18,147.61); loans to shareholders = 0); Schedule M-2 reports 2023 distributions ≈ **$333,870**; K-1 box 16D distributions ≈ 333,870; marks Final return + Amended. Open issue flagged: distributions exceeding stock basis → possible LT capital gain to Michael.
IL-1120-ST 2023: ordinary income 26,680; replacement tax ≈ $400 (IL charter 07396902). Files: `/tmp/migrate/il1120st_2023.pdf`, `il1120stx_2023.pdf`.

(On the **2024** personal return On-Chain shows only a **$1 nonpassive loss** on Schedule E and the **$60,000 share-sale** gain on Form 8949 — the entity was wound down / sold in 2024.)

---

## Maven Financial Technologies Inc 2024 — Form 1120 (C corp)

File: `/tmp/y2024/2024/Maven/2024 MAVEN FINANCIAL TECHNOLOGIES INC Form 1120  Corporations Tax Return_Filing.pdf`
EIN **92-3379962**.

| Line | Item | Amount |
|---|---|---|
| 1a | Gross receipts | 197,301 |
| **11** | **Total income** | **197,301** |
| 16 | Rents | 19,945 |
| 26 | Other deductions (statement) | 187,842 |
| **27** | **Total deductions** | **207,787** |
| **28** | **Taxable income before NOL** | **-10,486** |
| 29a | NOL deduction | 0 |
| **30** | **Taxable income** | **-10,486** |
| **31** | **Total tax** | **0** |

**NOL:** the NOL carryover worksheet shows **no prior-year NOL carryover used**; the 2024 loss of **$10,486** becomes a new NOL carryforward (TCJA — carries forward indefinitely). Note Maven also had a **2023 loss of $(5,461)** (see below), so accumulated federal NOL ≈ **$15,947** (2023 $5,461 + 2024 $10,486), though the 2024 worksheet did not populate the prior carryover.
**IL-1120 (2024):** income/loss -10,486; replacement tax 0.

## Maven Financial Technologies Inc 2023 — Form 1120

File: `/tmp/migrate/taxall/Tax/2023/Maven/2023 Tax Return Documents (MAVEN FINANCIAL TECHNO)-CLIENT COPY.PDF`

| Line | Item | Amount |
|---|---|---|
| 1a / 11 | Gross receipts / Total income | 134,849 |
| 27 | Total deductions | 140,310 |
| **28 / 30** | **Taxable income before/after NOL** | **(5,461)** |
| 31 | Total tax | 0 |

---

## Petersburg Place Investors LLC 2024 — Schedule K-1 (Form 1065 partnership)

File: `/tmp/y2024/2024/2024_K_1_Package___Petersburg_8_Arbach__Andrea.pdf`
Partnership **PETERSBURG PLACE INVESTORS LLC**, EIN **84-3764182**. Partner: Andrea Arbach (SSN 402-37-4551). **Limited partner / domestic — passive.**

| Box | Item | Amount |
|---|---|---|
| 1 | Ordinary business income | 0 |
| **2** | **Net rental real estate income** | **7,858** |
| 5 | Interest income | 2 |
| 8 | Net short-term capital gain | **8,178** |
| 20 code A | (Investment income) | 8,243 |
| 20 code N | Business interest expense | 8,178 |
| L | Current year net income per K-1 | 7,860 |

This K-1 supplies the personal return's: Schedule E passive income **7,860** (= box 2 rental 7,858 + box 5 interest 2) and Schedule D line 5 **short-term $8,178** (box 8).

---

## Image-only / non-return files

- **`/tmp/y2024/2024/2023E0074967.pdf`, `2023E0074968.pdf`, `2023E0074969.pdf`** — image-only (no embedded text; OCR'd with tesseract). These are **NOT tax returns**. They are **Jackson County, Missouri Quit Claim Deeds** (dated September 2023, recorded 10/16/2023, $27 fee, 3 pages each) transferring property from **Michael Arbach f/k/a Abdu Arbach → SWEET HOME KC, LLC** (3 deeds = the 3 KC rental properties on SweetHome's Form 8825).

## 2025 tax-source documents (listed only, not parsed) — `/tmp/tax_extract/Tax Forms/`
- 2025 K-1 - Michael Arbach.pdf
- 2025_Draft_K_1_Package___Petersburg_Place_8_Arbach__Andrea (1).pdf
- ML Sidecar Oscar Investors, LLC - 2025 K-1 - Michael Arbach.pdf
- 2025-Individual-TOD-1925-Consolidated-Form-1099.pdf; 2025-Individual-TOD-3868-CORRECTED-Consolidated-Form-1099.pdf
- Robinhood_Securities_2025_Consolidated_1099.pdf
- Hayat_Health_LLC_1099-NEC.pdf; Hayat_Health_LLC_1099-NEC_2025.pdf
- 2025-Health-Savings-Account 1099-SA & 5498-SA instructions
