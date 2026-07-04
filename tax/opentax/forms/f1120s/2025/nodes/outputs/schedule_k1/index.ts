import { z } from "zod";
import type { NodeResult } from "../../../../../../core/types/tax-node.ts";
import { TaxNode } from "../../../../../../core/types/tax-node.ts";
import { OutputNodes } from "../../../../../../core/types/output-nodes.ts";
import type { NodeContext } from "../../../../../../core/types/node-context.ts";
import { sumNumericArrayFields } from "../../../../../f1040/nodes/utils.ts";

// Schedule K-1 (Form 1120-S) — Shareholder's Share of Income, Deductions, Credits
//
// Sink node. Receives the entity-level Schedule K totals (from schedule_k) and
// the shareholder identity + ownership percentage (from general), then computes
// the single shareholder's pro-rata share of each box per IRC §1366.
//
// Box 1  — Ordinary business income (loss)
// Box 2  — Net rental real estate income (loss)        (Form 8825)
// Box 4  — Interest income
// Box 5a — Ordinary dividends
// Box 11 — Section 179 deduction
// Box 16, code D — Distributions

// ─── Schema ───────────────────────────────────────────────────────────────────

const inputObject = z.object({
  // Identity (routed from the general node)
  shareholder_name: z.string().optional(),
  shareholder_tin: z.string().optional(),
  // Ownership percentage 0–100. Defaults to 100 (single 100% shareholder).
  ownership_percentage: z.number().min(0).max(100).optional(),

  // Entity-level Schedule K totals (routed from schedule_k)
  ordinary_business_income: z.number().optional(),
  net_rental_real_estate: z.number().optional(),
  interest_income: z.number().optional(),
  ordinary_dividends: z.number().optional(),
  section_179: z.number().optional(),
  distributions: z.number().optional(),
});

// Both schedule_k and general deposit into this node; numeric collisions are
// summed by the executor's accumulation pattern before parsing.
export const inputSchema = z.preprocess(sumNumericArrayFields, inputObject);

type ScheduleK1Input = z.infer<typeof inputObject>;

// ─── Pure helpers ─────────────────────────────────────────────────────────────

function ownershipFraction(input: ScheduleK1Input): number {
  if (input.ownership_percentage === undefined) return 1;
  return input.ownership_percentage / 100;
}

function shareOf(amount: number | undefined, fraction: number): number {
  return Math.round((amount ?? 0) * fraction * 100) / 100;
}

// Identity (shareholder_name / shareholder_tin) and ownership_percentage are
// routed in by the general node and already live in this node's pending slot;
// they are intentionally NOT re-echoed here to avoid the executor promoting the
// duplicate scalar into an array.
function assembleK1(input: ScheduleK1Input): Record<string, number> {
  const fraction = ownershipFraction(input);
  return {
    box1_ordinary_business_income: shareOf(input.ordinary_business_income, fraction),
    box2_net_rental_real_estate: shareOf(input.net_rental_real_estate, fraction),
    box4_interest_income: shareOf(input.interest_income, fraction),
    box5a_ordinary_dividends: shareOf(input.ordinary_dividends, fraction),
    box11_section_179: shareOf(input.section_179, fraction),
    box16d_distributions: shareOf(input.distributions, fraction),
  };
}

// ─── Node class ───────────────────────────────────────────────────────────────

class ScheduleK1Node extends TaxNode<typeof inputSchema> {
  readonly nodeType = "schedule_k1";
  readonly inputSchema = inputSchema;
  readonly outputNodes = new OutputNodes([]);

  compute(_ctx: NodeContext, rawInput: ScheduleK1Input): NodeResult {
    const input = inputSchema.parse(rawInput);
    const assembled = assembleK1(input);
    return { outputs: [{ nodeType: this.nodeType, fields: assembled }] };
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const schedule_k1 = new ScheduleK1Node();
