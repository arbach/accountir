import { z } from "zod";
import type { NodeOutput, NodeResult } from "../../../../../../core/types/tax-node.ts";
import { TaxNode } from "../../../../../../core/types/tax-node.ts";
import { OutputNodes } from "../../../../../../core/types/output-nodes.ts";
import type { NodeContext } from "../../../../../../core/types/node-context.ts";
import { sumNumericArrayFields } from "../../../../../f1040/nodes/utils.ts";
import { CONFIG_BY_YEAR, IL_INDIVIDUAL_TAX_RATE } from "../../../config.ts";

// Form IL-1040 — Illinois Individual Income Tax Return.
//
// Illinois taxes individual net income at a flat 4.95% (35 ILCS 5/201(b)(5.4),
// since 2017). IL base income starts from federal adjusted gross income
// (Form 1040 line 11), adjusted by Illinois additions (Schedule M) and
// subtractions (e.g. federally-taxed retirement income and Social Security,
// which Illinois exempts). The personal exemption allowance (per taxpayer,
// spouse, and dependent) reduces base income; the per-person amount is set
// per tax year (CONFIG_BY_YEAR) and may be overridden by the bridge.
// Rate confirmed: https://tax.illinois.gov/research/taxrates/income.html

// ─── Schema ───────────────────────────────────────────────────────────────────

const inputObject = z.object({
  // Federal adjusted gross income (Form 1040 line 11).
  federal_agi: z.number().optional(),
  // Illinois Schedule M modifications.
  il_additions: z.number().nonnegative().optional(),
  il_subtractions: z.number().nonnegative().optional(),
  // Number of personal + dependent exemptions claimed (0 when the AGI cap
  // disallows the exemption — the bridge applies that test).
  exemption_count: z.number().nonnegative().optional(),
  // Per-person exemption amount; when omitted the node defaults from the
  // per-year config (CONFIG_BY_YEAR[ctx.taxYear]).
  exemption_per_person: z.number().nonnegative().optional(),
});

// Input fields arrive once from start, but defensive summing keeps the node
// robust if any field is deposited by more than one upstream contributor (the
// ledger bridge emits one entry per account mapped to a field).
export const inputSchema = z.preprocess(sumNumericArrayFields, inputObject);

type Il1040Input = z.infer<typeof inputObject>;

// ─── Pure helpers ─────────────────────────────────────────────────────────────

// Round to whole cents to avoid binary floating-point dust on book-net amounts.
function r2(n: number): number {
  return Math.round(n * 100) / 100;
}

function assemble(input: Il1040Input, exemptionPerPerson: number): Record<string, number> {
  const ilBaseIncome = (input.federal_agi ?? 0) +
    (input.il_additions ?? 0) -
    (input.il_subtractions ?? 0);

  const perPerson = input.exemption_per_person ?? exemptionPerPerson;
  const totalExemptions = (input.exemption_count ?? 0) * perPerson;

  const ilNetIncome = Math.max(0, ilBaseIncome - totalExemptions);
  const ilTax = r2(ilNetIncome * IL_INDIVIDUAL_TAX_RATE);

  return {
    il_base_income: r2(ilBaseIncome),
    exemption_per_person: r2(perPerson),
    total_exemptions: r2(totalExemptions),
    il_net_income: r2(ilNetIncome),
    il_tax: ilTax,
  };
}

// ─── Node class ───────────────────────────────────────────────────────────────

class Il1040Node extends TaxNode<typeof inputSchema> {
  readonly nodeType = "il1040";
  readonly inputSchema = inputSchema;
  readonly outputNodes = new OutputNodes([]);

  compute(ctx: NodeContext, rawInput: Il1040Input): NodeResult {
    const input = inputSchema.parse(rawInput);
    const exemptionPerPerson = CONFIG_BY_YEAR[ctx.taxYear]?.exemptionPerPerson ?? 0;
    const assembled = assemble(input, exemptionPerPerson);

    const outputs: NodeOutput[] = [
      // Self-deposit lines for display.
      { nodeType: this.nodeType, fields: assembled },
    ];

    return { outputs };
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const il1040 = new Il1040Node();
