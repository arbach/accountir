# Tax sub-system — brief for the Tax Agent

You are taking over the **tax-calculation** work for the accountir books. This document
is your starting point. Read it fully, then `tax/README.md` and `tax/opentax/CLAUDE.md`.

## Mission
Build a reliable, auditable tax-return pipeline for the owner's entities by **building on
[OpenTax](https://github.com/filedcom/opentax)** (a TypeScript/Deno federal tax engine — a
graph of pure-function nodes defined with Zod schemas) and **adding the forms our entities
need on top of it**. Then wire it to the accountir books so a return can be produced from the
ledger and exported (MeF XML / filled PDF).

Owner's standing rules (inherited): **be audit-ready; when in doubt over-pay the IRS; claim
every legitimate deduction; never underpay.** Every figure must trace to the books or a source
document. Do not fabricate numbers or sign forms.

## Why OpenTax
- Open-source (AGPL v3, same as accountir), built *for* AI agents, no vendor lock-in.
- Each tax line is a pure node → traceable to the IRS instruction. Test cases from IRS VITA/Pub 17.
- Ships its own agent skills (`tax-preparer`, `tax-reviewer`) + dev skills (`tax-build`, `tax-fix`,
  `tax-cases`, `tax-status`) under `tax/opentax/.claude/` and `tax/opentax/skills/`. **Use them.**
- Today it supports **f1040 : 2025** only (131 input nodes). Everything else we need is greenfield.

## The entities and the forms each needs (TY2025)
| Entity | Type | EIN | 2025 book net | Federal form(s) | State |
|---|---|---|---|---|---|
| Michael & Andrea Arbach | Individual | SSN 813-14-0923 | **+$233,673.65** (total income) | **f1040** (✅ in opentax) + Sch E, Sch D/**8949** ($220k LTCG on Maven stock sale), Sch B, **8995** (QBI) | **IL-1040** |
| MAVEN FINANCIAL TECHNOLOGIES INC | **C-corp** | 92-3379962 | **−$3,886.33** (loss → NOL) | **f1120** ⬅ build | IL-1120 |
| Hayat Health LLC | S-corp | 33-2127261 | **−$26,602.24** | **f1120-S** + Sch K-1 ⬅ build | IL-1120-ST |
| SWEET HOME KC LLC | S-corp | 93-2942628 | **+$23,809.01** | **f1120-S** + **8825** (rentals) + K-1 ⬅ build | IL-1120-ST |
| On-Chain LLC | S-corp (dissolved 2023) | 82-3930173 | n/a 2025 | f1120-S **2021–2024** (back years) ⬅ build | IL-1120-ST |

Notes that matter for the math:
- **Maven is a C-corp** → its loss is an **NOL carryforward**, it does NOT flow to the 1040.
- All S-corps + the rentals + personal flow to Michael's **1040 / Schedule E**.
- The **$220,000 LTCG** is Michael selling his **Maven C-corp stock** (installment, elected out → all 2025; Sch D + **8949 Box F**; not §1202). Distinct from the **$80,000 Maven IP sale** (corporate revenue on Maven's 1120) and the **2023 $125k LTCG**.
- 3 dependent children (Alexander 2019, Suria 2021, Palymra b. 08/20/2025 SSN 154-29-4975) → **CTC** (Sch 8812).
- IL flat 4.95% + $2,850/person exemption; IL S-corp 1.5% replacement tax.

## Current state of the books (your inputs come from here)
All five entities' books **balance to $0** and are in Postgres DB `accountir_cloud` (see the bridge).
The 2025 nets above are authoritative. The accountir app also already has a `tax_forms` table and a
PDF-fill pipeline (Rust side) that pre-dates opentax — treat opentax as the **calculation engine of
record going forward**; the old PDF-fill is a fallback. Prior hand-filled 2025 drafts exist in each
entity's Documents tab (for reference/cross-check, NOT as truth).

## Your structure (what's already scaffolded)
```
tax/
  opentax/            # vendored OpenTax engine (our working fork). Add our forms here.
    forms/f1040/2025/ # the working reference implementation — STUDY THIS PATTERN
    forms/f1120/      # ⬅ scaffolded FORM.md spec — implement nodes
    forms/f1120s/     # ⬅ scaffolded FORM.md spec — implement nodes
    catalog.ts        # register new forms here ("f1120:2025": f1120_2025)
    core/types/form-definition.ts  # the contract every form must export
    CLAUDE.md         # opentax coding conventions — READ FIRST
  bridge/             # accountir books → opentax inputs (BRIDGE.md + export_return.py)
  forms/              # (reserved) our IRS line-mapping notes per form
  README.md           # how to run everything
  AGENT_BRIEF.md      # this file
```

## How a form is built in opentax (the pattern to follow)
1. `forms/<form>/<year>/nodes/inputs/` — one node per source document (Zod `itemSchema`).
2. `nodes/intermediate/` — computed forms / worksheets / aggregators (pure functions).
3. `nodes/outputs/` — final line aggregator for the form.
4. `<year>/index.ts` — export a `FormDefinition` (formType, taxYear, mefSchemaVersion, inputNodes,
   registry, buildMefXml, buildPdfBytes, buildPending).
5. Register in `catalog.ts`.
6. Tests next to each node (`*_test.ts`) using IRS examples. Run `deno task test`.
A node file's shape (per CLAUDE.md): imports → enums → Zod schemas → helpers → class → singleton.

## Definition of done (per form)
- Nodes implemented + unit tests from IRS instructions pass (`deno task test`).
- The form computes the right numbers for our entity from the bridge-exported inputs.
- `opentax return validate` passes MeF business rules.
- `opentax return export --type pdf` produces a correctly-filled IRS PDF (cross-check vs the books).
- Numbers reconcile to the entity's book net and to any prior filed return (flag deltas).

## Suggested order
1. **Bridge first** (`tax/bridge/`): get one entity's 2025 ledger → opentax `form add` inputs end-to-end for f1040 (already supported) so the loop works. Verify Michael's 1040 computes ≈ the prior draft.
2. **f1120-S** (covers Hayat, SweetHome, On-Chain) + Sch K-1 + 8825.
3. **f1120** (Maven C-corp, NOL).
4. **State**: IL-1040, IL-1120-ST, IL-1120.
5. Back years for On-Chain (2021–2024) once the S-corp form exists.

## Guardrails
- Never invent figures; pull from the books via the bridge or cite a source doc.
- Keep each node pure + tested; mirror opentax conventions exactly.
- Don't register a half-built form in `catalog.ts` (it breaks the engine build) — register when its tests pass.
- Coordinate book changes with the bookkeeping side; you consume the ledger, you don't rewrite it.
- The accountir Rust app and its CI are unaffected by `tax/` (it's outside the cargo workspace).
