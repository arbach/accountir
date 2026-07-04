import { z } from "zod";
import type { NodeOutput, NodeResult } from "../../../../../../core/types/tax-node.ts";
import { TaxNode, output } from "../../../../../../core/types/tax-node.ts";
import { OutputNodes } from "../../../../../../core/types/output-nodes.ts";
import type { NodeContext } from "../../../../../../core/types/node-context.ts";
import { schedule_k } from "../../intermediate/schedule_k/index.ts";

// Form 8825 — Rental Real Estate Income and Expenses of a Partnership or an
// S Corporation.
//
// One item per rental property. Net rental real estate income/loss is a
// separately-stated item that flows to Schedule K line 2 (NOT to ordinary
// business income / Form 1120-S line 21).
//
// IRS Form 8825: https://www.irs.gov/pub/irs-pdf/f8825.pdf

// ─── Schema ───────────────────────────────────────────────────────────────────

export const itemSchema = z.object({
  // Identification (Form 8825 lines 1 / column A–H)
  property_address: z.string().min(1),
  property_type: z.string().optional(),

  // Line 2 — Gross rents
  gross_rents: z.number().nonnegative(),

  // Lines 3–15 — Expenses
  expense_advertising: z.number().nonnegative().optional(),
  expense_auto_travel: z.number().nonnegative().optional(),
  expense_cleaning_maintenance: z.number().nonnegative().optional(),
  expense_commissions: z.number().nonnegative().optional(),
  expense_insurance: z.number().nonnegative().optional(),
  expense_legal_professional: z.number().nonnegative().optional(),
  expense_interest: z.number().nonnegative().optional(),
  expense_repairs: z.number().nonnegative().optional(),
  expense_taxes: z.number().nonnegative().optional(),
  expense_utilities: z.number().nonnegative().optional(),
  expense_wages_salaries: z.number().nonnegative().optional(),
  expense_depreciation: z.number().nonnegative().optional(),
  expense_other: z.number().nonnegative().optional(),
});

export const inputSchema = z.object({
  f8825s: z.array(itemSchema).min(1),
});

type F8825Item = z.infer<typeof itemSchema>;
type F8825Items = F8825Item[];

// ─── Pure helpers ─────────────────────────────────────────────────────────────

// Round to whole cents to avoid binary floating-point dust.
function r2(n: number): number {
  return Math.round(n * 100) / 100;
}

function totalExpenses(item: F8825Item): number {
  return (
    (item.expense_advertising ?? 0) +
    (item.expense_auto_travel ?? 0) +
    (item.expense_cleaning_maintenance ?? 0) +
    (item.expense_commissions ?? 0) +
    (item.expense_insurance ?? 0) +
    (item.expense_legal_professional ?? 0) +
    (item.expense_interest ?? 0) +
    (item.expense_repairs ?? 0) +
    (item.expense_taxes ?? 0) +
    (item.expense_utilities ?? 0) +
    (item.expense_wages_salaries ?? 0) +
    (item.expense_depreciation ?? 0) +
    (item.expense_other ?? 0)
  );
}

function netRentalIncome(item: F8825Item): number {
  return item.gross_rents - totalExpenses(item);
}

// Route total net rental real estate income → Schedule K line 2.
function scheduleKOutputs(items: F8825Items): NodeOutput[] {
  const totalNet = items.reduce((sum, item) => sum + netRentalIncome(item), 0);
  return [output(schedule_k, { line2_net_rental_real_estate: r2(totalNet) })];
}

// ─── Node class ───────────────────────────────────────────────────────────────

class F8825Node extends TaxNode<typeof inputSchema> {
  readonly nodeType = "f8825";
  readonly inputSchema = inputSchema;
  readonly outputNodes = new OutputNodes([schedule_k]);

  compute(_ctx: NodeContext, input: z.infer<typeof inputSchema>): NodeResult {
    const { f8825s } = inputSchema.parse(input);

    const grossRents = f8825s.reduce((sum, item) => sum + item.gross_rents, 0);
    const expenses = f8825s.reduce((sum, item) => sum + totalExpenses(item), 0);
    const net = grossRents - expenses;

    const outputs: NodeOutput[] = [
      // Self-deposit Form 8825 totals for display.
      {
        nodeType: this.nodeType,
        fields: {
          line18a_total_gross_rents: r2(grossRents),
          line18b_total_expenses: r2(expenses),
          line19_net_rental_real_estate: r2(net),
        },
      },
      ...scheduleKOutputs(f8825s),
    ];

    return { outputs };
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const f8825 = new F8825Node();
