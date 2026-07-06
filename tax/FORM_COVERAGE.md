# Form coverage — every form each entity needs (grounded in the 2024 filings)

Enumerated from the **2024 filed returns** (`tax/filed/FILED_RETURNS.md`) and the **2025
source docs** (`tax/filed/SOURCE_DOCS_2025.md`). Columns: **Engine** = an OpenTax node
exists; **Fed** = the bridge/map supplies the inputs; **Generated** = a filled PDF exists
in `tax_forms` for 2025. Gaps are the supporting forms not yet generated.

## Michael & Andrea Arbach — Form 1040 (individual, MFJ)

The personal return is the hub: it aggregates **K-1s from five passthroughs** (three are
outside investment partnerships) plus brokerage 1099s and the Maven stock sale.

| Form / schedule | Purpose | Engine | Fed | Generated | Notes |
|---|---|---|---|---|---|
| 1040 | Main return | ✅ | ✅ | ✅ | |
| Schedule 1 | Additional income → Sch E | ✅ | ✅ | ✅ | |
| Schedule B | Interest + dividends | ✅ | ✅ | ✅ | Robinhood/Fidelity 1099s |
| Schedule D + **8949** | Cap gains — Maven stock $220k (Box F) + K-1 ST gains | ✅ | ✅ | ✅ | |
| **Schedule E (Part II)** | K-1s from all passthroughs | ✅ | ✅ | ✅ | aggregates the 5 K-1s below |
| Form 8995 | QBI deduction | ✅ | ✅ | ✅ | |
| Form 8889 | HSA | ✅ | ✅ | ✅ | |
| **Schedule 8812** | Child Tax Credit (3 kids → $6,600) | ✅ (f8812) | ⚠️ | ❌ | **GAP — generate** (was on 2024 return) |
| **Schedule 3** | Nonrefundable credits | ✅ | ⚠️ | ❌ | 2024 had line 8 = 2; confirm 2025 |
| IL-1040 | Illinois | ✅ | ✅ | ✅ | |

**K-1s Michael *receives* (inputs to Schedule E — not separately filed):**
| Issuer | Form | Character | 2025 box detail (fed) |
|---|---|---|---|
| Hayat Health LLC (S) | 1120-S K-1 | nonpassive ordinary | box 1 = −26,602 |
| Sweet Home KC LLC (S) | 1120-S K-1 | **passive rental** | box 2 = 23,809 |
| Petersburg Place Investors LLC | 1065 K-1 | passive rental | box 2 = 15,413; box 5 int = 3 *(DRAFT K-1)* |
| ML Sidecar Oscar Investors LLC | 1065 K-1 | passive rental | box 2 = **−19,107**; box 5 int = 890 |
| ML Loyola Target LLC | 1065 K-1 | investment | box 5 int = 3,323 (K-3 checked) |

## Hayat Health LLC — Form 1120-S (S-corp)
| Form | Engine | Fed | Generated | Notes |
|---|---|---|---|---|
| 1120-S | ✅ | ✅ | ✅ | ordinary business (medical consulting) |
| Schedule K-1 | ✅ | ✅ | ✅ | 100% Michael, box 1 |
| IL-1120-ST | ✅ | ✅ | ✅ | |
| 1125-E (officer comp) | ✅ | n/a | n/a | no officer comp (owner-confirmed) |
| 4562 (depreciation) | — | n/a | n/a | none in 2025 |

## Sweet Home KC LLC — Form 1120-S + rental
| Form | Engine | Fed | Generated | Notes |
|---|---|---|---|---|
| 1120-S | ✅ | ✅ | ✅ | $0 ordinary (rental-only) |
| **Form 8825** | ✅ | ✅ | ✅ | 3 properties, per-property columns |
| Schedule K-1 | ✅ | ✅ | ✅ | box 2 net rental = 23,809 |
| IL-1120-ST | ✅ | ✅ | ✅ | |
| **Form 4562** | ⚠️ | ⚠️ | ❌ | **GAP** — $4,862 depreciation on the rentals → file 4562 |
| Revoke-S statement | n/a | n/a | draft | `tax/sweethome/` — eff 1/1/2027 |

## Maven Financial Technologies Inc — Form 1120 (C-corp)
| Form | Engine | Fed | Generated | Notes |
|---|---|---|---|---|
| 1120 | ✅ | ✅ | ✅ | NOL carryforward (loss) |
| **1125-A (COGS)** | ⚠️ | ⚠️ | ❌ | **GAP** — Maven books have a COGS account → 1125-A |
| **Form 4562** | ⚠️ | ⚠️ | ❌ | if depreciation booked → 4562 |
| 1099-NEC | ✅ | ✅ | ✅ | contractor payments issued |
| IL-1120 | ✅ | ✅ | ✅ | |

## On-Chain LLC — Form 1120-S (dissolved 2023)
| Form | Notes |
|---|---|
| 1120-S 2021–2024 (final/amended) | back-years; final 2024. On the 2025 personal return On-Chain is only a $1 Sch E loss + the $60k share-sale gain (8949) — the entity is wound down. No 2025 entity return. |

---

## Summary of gaps to generate (build supports all of these)
1. **Michael — Schedule 8812** (CTC $6,600) and **Schedule 3** — were on the 2024 return.
2. **SweetHome — Form 4562** (depreciation $4,862).
3. **Maven — Form 1125-A** (COGS) and **Form 4562** (if depreciation).

The OpenTax engine has nodes for all of these; they are input/generation tasks, not engine
gaps. Everything related to **rental properties** (8825 per-property, Sch E passive rental
character, K-1 box 2) and **investment K-1s** (ML Loyola / ML Sidecar Oscar / Petersburg,
1065 partnerships with box 2 rental + box 5 interest) is supported and fed today.
