import { z } from "zod";
import type { NodeOutput, NodeResult } from "../../../../../../core/types/tax-node.ts";
import { TaxNode, output, type AtLeastOne } from "../../../../../../core/types/tax-node.ts";
import { OutputNodes } from "../../../../../../core/types/output-nodes.ts";
import type { NodeContext } from "../../../../../../core/types/node-context.ts";
import { schedule_k } from "../../intermediate/schedule_k/index.ts";
import { schedule_k1 } from "../../outputs/schedule_k1/index.ts";

// General / officer-identity input — entity and shareholder identification plus
// entity-level Schedule K pass-through items that are not part of ordinary income.
//
// Identity + ownership route to the shareholder's Schedule K-1; the separately
// stated pass-through items (interest, dividends, §179, distributions) route to
// Schedule K.

// ─── Schema ───────────────────────────────────────────────────────────────────

export const inputSchema = z.object({
  // Entity identification
  corporation_name: z.string().min(1),
  ein: z.string().optional(),

  // Shareholder (single 100% owner for our entities)
  shareholder_name: z.string().min(1),
  shareholder_tin: z.string().optional(),
  ownership_percentage: z.number().min(0).max(100).default(100),

  // Officer compensation flag (reasonable-comp indicator; Form 1125-E)
  is_officer_compensated: z.boolean().optional(),

  // Entity-level Schedule K pass-through items
  line4_interest_income: z.number().optional(),
  line5a_ordinary_dividends: z.number().optional(),
  line11_section_179: z.number().nonnegative().optional(),
  line16d_distributions: z.number().nonnegative().optional(),
});

type GeneralInput = z.infer<typeof inputSchema>;

// ─── Pure helpers ─────────────────────────────────────────────────────────────

// Route shareholder identity + ownership percentage → Schedule K-1.
function scheduleK1Output(input: GeneralInput): NodeOutput {
  return output(schedule_k1, {
    shareholder_name: input.shareholder_name,
    shareholder_tin: input.shareholder_tin,
    ownership_percentage: input.ownership_percentage,
  });
}

// Route entity-level pass-through items → Schedule K (only when present).
function scheduleKOutputs(input: GeneralInput): NodeOutput[] {
  const fields: Record<string, number> = {};
  if (input.line4_interest_income !== undefined) {
    fields.line4_interest_income = input.line4_interest_income;
  }
  if (input.line5a_ordinary_dividends !== undefined) {
    fields.line5a_ordinary_dividends = input.line5a_ordinary_dividends;
  }
  if (input.line11_section_179 !== undefined) {
    fields.line11_section_179 = input.line11_section_179;
  }
  if (input.line16d_distributions !== undefined) {
    fields.line16d_distributions = input.line16d_distributions;
  }
  if (Object.keys(fields).length === 0) return [];
  return [output(schedule_k, fields as AtLeastOne<z.infer<typeof schedule_k.inputSchema>>)];
}

// ─── Node class ───────────────────────────────────────────────────────────────

class GeneralNode extends TaxNode<typeof inputSchema> {
  readonly nodeType = "general";
  readonly inputSchema = inputSchema;
  readonly outputNodes = new OutputNodes([schedule_k, schedule_k1]);

  compute(_ctx: NodeContext, rawInput: z.infer<typeof inputSchema>): NodeResult {
    const input = inputSchema.parse(rawInput);

    const outputs: NodeOutput[] = [
      // Self-deposit entity identity for display.
      {
        nodeType: this.nodeType,
        fields: {
          corporation_name: input.corporation_name,
          ein: input.ein,
          shareholder_name: input.shareholder_name,
          ownership_percentage: input.ownership_percentage,
        },
      },
      scheduleK1Output(input),
      ...scheduleKOutputs(input),
    ];

    return { outputs };
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const general = new GeneralNode();
