import { z } from "zod";
import type { NodeOutput, NodeResult } from "../../../../../../core/types/tax-node.ts";
import { TaxNode } from "../../../../../../core/types/tax-node.ts";
import { OutputNodes } from "../../../../../../core/types/output-nodes.ts";
import type { NodeContext } from "../../../../../../core/types/node-context.ts";
import { sumNumericArrayFields } from "../../../../../f1040/nodes/utils.ts";
import { schedule_k } from "../../intermediate/schedule_k/index.ts";

// Form 1120-S, page 1 — Ordinary trade or business income (loss)
//
// This node is BOTH an input node (the entity's ordinary income/deduction lines
// are supplied here) and the output node that assembles page 1 of the return.
//
// Income:
//   Line 1a — Gross receipts or sales
//   Line 1b — Returns and allowances
//   Line 1c — Net receipts (1a − 1b)
//   Line 2  — Cost of goods sold (Form 1125-A)
//   Line 3  — Gross profit (1c − 2)
//   Line 4  — Net gain (loss) from Form 4797
//   Line 5  — Other income (loss)
//   Line 6  — Total income (3 + 4 + 5)
// Deductions:
//   Line 7  — Compensation of officers (Form 1125-E)
//   Line 8  — Salaries and wages
//   Line 12 — Taxes and licenses
//   Line 14 — Depreciation (Form 4562)
//   Line 19 — Other deductions
//   Line 20 — Total deductions
//   Line 21 — Ordinary business income (loss) (6 − 20)
//
// Entity-level federal income tax (line 22) is generally $0 for an S corporation;
// income passes through to shareholders. Built-in gains tax (§1374) and the
// excess net passive income tax (§1375) are out of scope and intentionally omitted.

// ─── Schema ───────────────────────────────────────────────────────────────────

const inputObject = z.object({
  // Income
  line1a_gross_receipts: z.number().nonnegative().optional(),
  line1b_returns_allowances: z.number().nonnegative().optional(),
  line2_cogs: z.number().nonnegative().optional(),
  line4_net_gain_4797: z.number().optional(),
  line5_other_income: z.number().optional(),
  // Deductions
  line7_officer_compensation: z.number().nonnegative().optional(),
  line8_salaries_wages: z.number().nonnegative().optional(),
  line12_taxes: z.number().nonnegative().optional(),
  line14_depreciation: z.number().nonnegative().optional(),
  line19_other_deductions: z.number().nonnegative().optional(),
});

// Input fields arrive once from start, but defensive summing keeps the node
// robust if any line is deposited by more than one upstream contributor.
export const inputSchema = z.preprocess(sumNumericArrayFields, inputObject);

type F1120sInput = z.infer<typeof inputObject>;

// ─── Pure helpers ─────────────────────────────────────────────────────────────

// Round to whole cents to avoid binary floating-point dust on book-net amounts.
function r2(n: number): number {
  return Math.round(n * 100) / 100;
}

function netReceipts(input: F1120sInput): number {
  return (input.line1a_gross_receipts ?? 0) - (input.line1b_returns_allowances ?? 0);
}

function grossProfit(input: F1120sInput): number {
  return netReceipts(input) - (input.line2_cogs ?? 0);
}

function totalIncome(input: F1120sInput): number {
  return grossProfit(input) + (input.line4_net_gain_4797 ?? 0) + (input.line5_other_income ?? 0);
}

function totalDeductions(input: F1120sInput): number {
  return (
    (input.line7_officer_compensation ?? 0) +
    (input.line8_salaries_wages ?? 0) +
    (input.line12_taxes ?? 0) +
    (input.line14_depreciation ?? 0) +
    (input.line19_other_deductions ?? 0)
  );
}

function assemblePageOne(input: F1120sInput): Record<string, number> {
  const line1c = netReceipts(input);
  const line3 = grossProfit(input);
  const line6 = totalIncome(input);
  const line20 = totalDeductions(input);
  return {
    line1c_net_receipts: r2(line1c),
    line3_gross_profit: r2(line3),
    line6_total_income: r2(line6),
    line20_total_deductions: r2(line20),
    line21_ordinary_business_income: r2(line6 - line20),
  };
}

// ─── Node class ───────────────────────────────────────────────────────────────

class F1120sNode extends TaxNode<typeof inputSchema> {
  readonly nodeType = "f1120s";
  readonly inputSchema = inputSchema;
  readonly outputNodes = new OutputNodes([schedule_k]);

  compute(_ctx: NodeContext, rawInput: F1120sInput): NodeResult {
    const input = inputSchema.parse(rawInput);
    const assembled = assemblePageOne(input);

    const outputs: NodeOutput[] = [
      // Self-deposit page-1 lines for display.
      { nodeType: this.nodeType, fields: assembled },
      // Line 21 ordinary business income → Schedule K line 1.
      this.outputNodes.output(schedule_k, {
        line1_ordinary_business_income: assembled.line21_ordinary_business_income,
      }),
    ];

    return { outputs };
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const f1120s = new F1120sNode();
