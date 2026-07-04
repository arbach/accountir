import { z } from "zod";
import type { NodeOutput, NodeResult } from "../../../../../../core/types/tax-node.ts";
import { TaxNode } from "../../../../../../core/types/tax-node.ts";
import { OutputNodes } from "../../../../../../core/types/output-nodes.ts";
import type { NodeContext } from "../../../../../../core/types/node-context.ts";
import { sumNumericArrayFields } from "../../../../../f1040/nodes/utils.ts";

// Form IL-1120 — Illinois Corporation Income and Replacement Tax Return
// (C corporation).
//
// Illinois imposes two taxes on C-corporation net income:
//   • Corporate income tax — 7.0%   (35 ILCS 5/201(b)(14))
//   • Replacement tax      — 2.5%   (35 ILCS 5/201(d))
//   ⇒ combined 9.5% on Illinois net income.
// Rates confirmed: https://tax.illinois.gov/research/taxrates/income.html
//
// IL base income starts from federal taxable income BEFORE the federal NOL
// deduction (Form 1120 line 28), adjusted by Illinois additions and
// subtractions (Schedule M modifications — e.g. the federal NOL deduction and
// Illinois income/replacement tax are added back). Illinois maintains its OWN
// net operating loss: an available IL NOL offsets positive base income only,
// capped at that income (it cannot create or deepen a loss). A current-year
// loss feeds the IL NOL pool the bridge carries forward.

// ─── Schema ───────────────────────────────────────────────────────────────────

const inputObject = z.object({
  // Federal taxable income before federal NOL (Form 1120 line 28); may be a loss.
  federal_taxable_income: z.number().optional(),
  // Illinois Schedule M modifications.
  il_additions: z.number().nonnegative().optional(),
  il_subtractions: z.number().nonnegative().optional(),
  // Illinois NOL carryforward available from prior years.
  il_nol_available: z.number().nonnegative().optional(),
});

// Input fields arrive once from start, but defensive summing keeps the node
// robust if any field is deposited by more than one upstream contributor (the
// ledger bridge emits one entry per account mapped to a field).
export const inputSchema = z.preprocess(sumNumericArrayFields, inputObject);

type Il1120Input = z.infer<typeof inputObject>;

// Illinois corporate income tax rate, 35 ILCS 5/201(b)(14). Stable across 2023–2025.
const INCOME_TAX_RATE = 0.07;
// Illinois replacement tax rate for corporations, 35 ILCS 5/201(d). Stable across 2023–2025.
const REPLACEMENT_TAX_RATE = 0.025;

// ─── Pure helpers ─────────────────────────────────────────────────────────────

// Round to whole cents to avoid binary floating-point dust on book-net amounts.
function r2(n: number): number {
  return Math.round(n * 100) / 100;
}

function assemble(input: Il1120Input): Record<string, number> {
  const ilBaseIncome = (input.federal_taxable_income ?? 0) +
    (input.il_additions ?? 0) -
    (input.il_subtractions ?? 0);

  // IL NOL only offsets positive base income and is capped at the available
  // carryforward; it cannot create or deepen a loss.
  const nolAvailable = input.il_nol_available ?? 0;
  const ilNolDeduction = Math.min(Math.max(ilBaseIncome, 0), nolAvailable);
  const ilNetIncome = Math.max(0, ilBaseIncome - ilNolDeduction);

  const incomeTax = r2(ilNetIncome * INCOME_TAX_RATE);
  const replacementTax = r2(ilNetIncome * REPLACEMENT_TAX_RATE);
  const totalIlTax = r2(incomeTax + replacementTax);

  // This year's loss (positive amount) feeds the IL NOL pool for future years.
  const nolGenerated = Math.max(0, -ilBaseIncome);
  const ilNolRemaining = nolAvailable - ilNolDeduction + nolGenerated;

  return {
    il_base_income: r2(ilBaseIncome),
    il_nol_deduction: r2(ilNolDeduction),
    il_net_income: r2(ilNetIncome),
    income_tax: incomeTax,
    replacement_tax: replacementTax,
    total_il_tax: totalIlTax,
    il_nol_generated: r2(nolGenerated),
    il_nol_remaining: r2(ilNolRemaining),
  };
}

// ─── Node class ───────────────────────────────────────────────────────────────

class Il1120Node extends TaxNode<typeof inputSchema> {
  readonly nodeType = "il1120";
  readonly inputSchema = inputSchema;
  readonly outputNodes = new OutputNodes([]);

  compute(_ctx: NodeContext, rawInput: Il1120Input): NodeResult {
    const input = inputSchema.parse(rawInput);
    const assembled = assemble(input);

    const outputs: NodeOutput[] = [
      // Self-deposit lines for display.
      { nodeType: this.nodeType, fields: assembled },
    ];

    return { outputs };
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const il1120 = new Il1120Node();
