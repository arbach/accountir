import { z } from "zod";
import type { NodeOutput, NodeResult } from "../../../../../../core/types/tax-node.ts";
import { TaxNode } from "../../../../../../core/types/tax-node.ts";
import { OutputNodes } from "../../../../../../core/types/output-nodes.ts";
import type { NodeContext } from "../../../../../../core/types/node-context.ts";
import { sumNumericArrayFields } from "../../../../../f1040/nodes/utils.ts";

// Form 1120, page 1 — U.S. Corporation Income Tax Return (C corporation)
//
// This node is BOTH an input node (the corporation's income/deduction lines are
// supplied here) and the output node that assembles page 1 of the return.
//
// Income:
//   Line 1a — Gross receipts or sales
//   Line 1b — Returns and allowances
//   Line 1c — Net receipts (1a − 1b)
//   Line 2  — Cost of goods sold (Form 1125-A)
//   Line 3  — Gross profit (1c − 2)
//   Line 4  — Dividends and inclusions (Schedule C)
//   Line 5  — Interest
//   Line 6  — Gross rents
//   Line 8  — Capital gain net income (Schedule D)
//   Line 10 — Other income
//   Line 11 — Total income (3 + 4 + 5 + 6 + 8 + 10)
// Deductions:
//   Line 12 — Compensation of officers (Form 1125-E)
//   Line 13 — Salaries and wages
//   Line 17 — Taxes and licenses
//   Line 19 — Charitable contributions
//   Line 20 — Depreciation (Form 4562)
//   Line 26 — Other deductions
//   Line 27 — Total deductions
//   Line 28 — Taxable income before NOL deduction (11 − 27)
//   Line 29a — Net operating loss deduction
//   Line 30 — Taxable income (28 − 29a, not below zero)
//   Line 31 — Total tax (21% flat rate, IRC §11)
//
// Net operating loss (IRC §172): a current-year loss (negative line 28) becomes
// a carryforward; an available carryforward offsets positive line-28 income only,
// capped at that income (post-2017 NOLs cannot reduce taxable income below zero).

// ─── Schema ───────────────────────────────────────────────────────────────────

const inputObject = z.object({
  // Income
  line1a_gross_receipts: z.number().optional(),
  line1b_returns_allowances: z.number().optional(),
  line2_cogs: z.number().optional(),
  line4_dividends: z.number().optional(),
  line5_interest: z.number().optional(),
  line6_gross_rents: z.number().optional(),
  line8_capital_gain: z.number().optional(),
  line10_other_income: z.number().optional(),
  // Deductions
  line12_officer_compensation: z.number().nonnegative().optional(),
  line13_salaries_wages: z.number().nonnegative().optional(),
  line17_taxes_licenses: z.number().nonnegative().optional(),
  line19_charitable: z.number().nonnegative().optional(),
  line20_depreciation: z.number().nonnegative().optional(),
  line26_other_deductions: z.number().nonnegative().optional(),
  // Net operating loss carryforward available from prior years
  nol_carryforward_available: z.number().nonnegative().optional(),
});

// Input fields arrive once from start, but defensive summing keeps the node
// robust if any line is deposited by more than one upstream contributor (the
// ledger bridge emits one entry per account mapped to a line).
export const inputSchema = z.preprocess(sumNumericArrayFields, inputObject);

type F1120Input = z.infer<typeof inputObject>;

// Corporate flat tax rate, IRC §11 (Tax Cuts and Jobs Act, effective 2018+).
const CORP_TAX_RATE = 0.21;

// ─── Pure helpers ─────────────────────────────────────────────────────────────

// Round to whole cents to avoid binary floating-point dust on book-net amounts.
function r2(n: number): number {
  return Math.round(n * 100) / 100;
}

function netReceipts(input: F1120Input): number {
  return (input.line1a_gross_receipts ?? 0) - (input.line1b_returns_allowances ?? 0);
}

function grossProfit(input: F1120Input): number {
  return netReceipts(input) - (input.line2_cogs ?? 0);
}

function totalIncome(input: F1120Input): number {
  return (
    grossProfit(input) +
    (input.line4_dividends ?? 0) +
    (input.line5_interest ?? 0) +
    (input.line6_gross_rents ?? 0) +
    (input.line8_capital_gain ?? 0) +
    (input.line10_other_income ?? 0)
  );
}

function totalDeductions(input: F1120Input): number {
  return (
    (input.line12_officer_compensation ?? 0) +
    (input.line13_salaries_wages ?? 0) +
    (input.line17_taxes_licenses ?? 0) +
    (input.line19_charitable ?? 0) +
    (input.line20_depreciation ?? 0) +
    (input.line26_other_deductions ?? 0)
  );
}

function assemblePageOne(input: F1120Input): Record<string, number> {
  const line1c = netReceipts(input);
  const line3 = grossProfit(input);
  const line11 = totalIncome(input);
  const line27 = totalDeductions(input);
  const line28 = line11 - line27;

  // NOL only offsets positive income and is capped at the available carryforward;
  // it cannot create or deepen a loss (post-2017, IRC §172(a)).
  const nolAvailable = input.nol_carryforward_available ?? 0;
  const line29a = Math.min(Math.max(line28, 0), nolAvailable);
  const line30 = Math.max(0, line28 - line29a);
  const line31 = r2(line30 * CORP_TAX_RATE);

  // This year's loss (positive amount) feeds the NOL pool for future years.
  const nolGenerated = Math.max(0, -line28);
  const nolRemaining = nolAvailable - line29a + nolGenerated;

  return {
    line1c_net_receipts: r2(line1c),
    line3_gross_profit: r2(line3),
    line11_total_income: r2(line11),
    line27_total_deductions: r2(line27),
    line28_income_before_nol: r2(line28),
    line29a_nol_deduction: r2(line29a),
    line30_taxable_income: r2(line30),
    line31_total_tax: line31,
    nol_carryforward_generated: r2(nolGenerated),
    nol_carryforward_remaining: r2(nolRemaining),
  };
}

// ─── Node class ───────────────────────────────────────────────────────────────

class F1120Node extends TaxNode<typeof inputSchema> {
  readonly nodeType = "f1120";
  readonly inputSchema = inputSchema;
  readonly outputNodes = new OutputNodes([]);

  compute(_ctx: NodeContext, rawInput: F1120Input): NodeResult {
    const input = inputSchema.parse(rawInput);
    const assembled = assemblePageOne(input);

    const outputs: NodeOutput[] = [
      // Self-deposit page-1 lines for display.
      { nodeType: this.nodeType, fields: assembled },
    ];

    return { outputs };
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const f1120 = new F1120Node();
