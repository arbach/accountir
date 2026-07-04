import { z } from "zod";
import type { NodeOutput, NodeResult } from "../../../../../../core/types/tax-node.ts";
import { TaxNode } from "../../../../../../core/types/tax-node.ts";
import { OutputNodes } from "../../../../../../core/types/output-nodes.ts";
import type { NodeContext } from "../../../../../../core/types/node-context.ts";
import { sumNumericArrayFields } from "../../../../../f1040/nodes/utils.ts";

// Form IL-1120-ST — Illinois Small Business Corporation Replacement Tax Return
// (S corporation).
//
// Illinois imposes a 1.5% Personal Property Replacement Tax on the net income
// of S corporations (and partnerships/trusts). There is no Illinois income tax
// at the entity level for an S corporation; income passes through to the
// shareholders' IL-1040s. See 35 ILCS 5/201(c) (replacement tax rate 1.5% for
// partnerships, trusts, and S corporations).
// Rate confirmed: https://tax.illinois.gov/research/taxrates/income.html
//
// IL net income starts from the federal S-corporation base — ordinary business
// income (Form 1120-S, page 1, line 21) plus separately-stated income that IL
// taxes (here net rental real-estate income, Schedule K line 2) — adjusted by
// Illinois additions and subtractions (Schedule M modifications). The
// replacement tax is computed on positive net income only; a net loss yields a
// $0 base (unlike the income tax, the replacement tax carries no NOL here).

// ─── Schema ───────────────────────────────────────────────────────────────────

const inputObject = z.object({
  // Federal base figures supplied by the ledger bridge from the computed f1120s.
  federal_ordinary_income: z.number().optional(), // f1120s line 21 (may be a loss)
  federal_net_rental: z.number().optional(), // Schedule K line 2
  // Illinois Schedule M modifications.
  il_additions: z.number().nonnegative().optional(),
  il_subtractions: z.number().nonnegative().optional(),
});

// Input fields arrive once from start, but defensive summing keeps the node
// robust if any field is deposited by more than one upstream contributor (the
// ledger bridge emits one entry per account mapped to a field).
export const inputSchema = z.preprocess(sumNumericArrayFields, inputObject);

type Il1120stInput = z.infer<typeof inputObject>;

// Illinois Personal Property Replacement Tax rate for S corporations,
// 35 ILCS 5/201(c). Stable across 2023–2025.
const REPLACEMENT_TAX_RATE = 0.015;

// ─── Pure helpers ─────────────────────────────────────────────────────────────

// Round to whole cents to avoid binary floating-point dust on book-net amounts.
function r2(n: number): number {
  return Math.round(n * 100) / 100;
}

function assemble(input: Il1120stInput): Record<string, number> {
  const ilNetIncome = (input.federal_ordinary_income ?? 0) +
    (input.federal_net_rental ?? 0) +
    (input.il_additions ?? 0) -
    (input.il_subtractions ?? 0);

  // Replacement tax applies to a positive base only; a net loss → $0.
  const replacementTax = r2(Math.max(0, ilNetIncome) * REPLACEMENT_TAX_RATE);

  return {
    il_net_income: r2(ilNetIncome),
    replacement_tax: replacementTax,
  };
}

// ─── Node class ───────────────────────────────────────────────────────────────

class Il1120stNode extends TaxNode<typeof inputSchema> {
  readonly nodeType = "il1120st";
  readonly inputSchema = inputSchema;
  readonly outputNodes = new OutputNodes([]);

  compute(_ctx: NodeContext, rawInput: Il1120stInput): NodeResult {
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

export const il1120st = new Il1120stNode();
