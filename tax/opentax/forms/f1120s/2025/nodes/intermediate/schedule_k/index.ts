import { z } from "zod";
import type { NodeOutput, NodeResult } from "../../../../../../core/types/tax-node.ts";
import { TaxNode } from "../../../../../../core/types/tax-node.ts";
import { OutputNodes } from "../../../../../../core/types/output-nodes.ts";
import type { NodeContext } from "../../../../../../core/types/node-context.ts";
import { sumNumericArrayFields } from "../../../../../f1040/nodes/utils.ts";
import { schedule_k1 } from "../../outputs/schedule_k1/index.ts";

// Schedule K (Form 1120-S) — Shareholders' Pro Rata Share Items (entity totals)
//
// Aggregates the S corporation's separately- and non-separately-stated items:
//   Line 1  — Ordinary business income (loss)          (from f1120s line 21)
//   Line 2  — Net rental real estate income (loss)     (from Form 8825)
//   Line 4  — Interest income
//   Line 5a — Ordinary dividends
//   Line 11 — Section 179 deduction
//   Line 16, code D — Distributions
//
// Note: net rental real estate income (line 2) is a SEPARATELY stated item and
// must NOT be folded into ordinary business income (line 1 / Form 1120-S line 21).
//
// Routes the entity totals to schedule_k1 for the shareholder's pro-rata share.

// ─── Schema ───────────────────────────────────────────────────────────────────

const inputObject = z.object({
  line1_ordinary_business_income: z.number().optional(),
  line2_net_rental_real_estate: z.number().optional(),
  line4_interest_income: z.number().optional(),
  line5a_ordinary_dividends: z.number().optional(),
  line11_section_179: z.number().optional(),
  line16d_distributions: z.number().optional(),
});

// f1120s, f8825, and general all deposit into this node; numeric collisions are
// summed by the executor's accumulation pattern before parsing.
export const inputSchema = z.preprocess(sumNumericArrayFields, inputObject);

type ScheduleKInput = z.infer<typeof inputObject>;

// ─── Pure helpers ─────────────────────────────────────────────────────────────

// Round to whole cents to avoid binary floating-point dust.
function r2(n: number): number {
  return Math.round(n * 100) / 100;
}

function assembleScheduleK(input: ScheduleKInput): Record<string, number> {
  return {
    line1_ordinary_business_income: r2(input.line1_ordinary_business_income ?? 0),
    line2_net_rental_real_estate: r2(input.line2_net_rental_real_estate ?? 0),
    line4_interest_income: r2(input.line4_interest_income ?? 0),
    line5a_ordinary_dividends: r2(input.line5a_ordinary_dividends ?? 0),
    line11_section_179: r2(input.line11_section_179 ?? 0),
    line16d_distributions: r2(input.line16d_distributions ?? 0),
  };
}

// ─── Node class ───────────────────────────────────────────────────────────────

class ScheduleKNode extends TaxNode<typeof inputSchema> {
  readonly nodeType = "schedule_k";
  readonly inputSchema = inputSchema;
  readonly outputNodes = new OutputNodes([schedule_k1]);

  compute(_ctx: NodeContext, rawInput: ScheduleKInput): NodeResult {
    const input = inputSchema.parse(rawInput);
    const assembled = assembleScheduleK(input);

    const outputs: NodeOutput[] = [
      // Self-deposit the entity-level Schedule K totals for display.
      { nodeType: this.nodeType, fields: assembled },
      // Route entity totals to the shareholder's K-1.
      this.outputNodes.output(schedule_k1, {
        ordinary_business_income: assembled.line1_ordinary_business_income,
        net_rental_real_estate: assembled.line2_net_rental_real_estate,
        interest_income: assembled.line4_interest_income,
        ordinary_dividends: assembled.line5a_ordinary_dividends,
        section_179: assembled.line11_section_179,
        distributions: assembled.line16d_distributions,
      }),
    ];

    return { outputs };
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const schedule_k = new ScheduleKNode();
