import { z } from "zod";
import type { NodeResult } from "../../../../../../core/types/tax-node.ts";
import { TaxNode } from "../../../../../../core/types/tax-node.ts";
import { OutputNodes } from "../../../../../../core/types/output-nodes.ts";
import type { NodeContext } from "../../../../../../core/types/node-context.ts";

// General / entity-identity input for Form IL-1120-ST (S corporation).
//
// This node carries entity identification (name, FEIN, address) and self-
// deposits it for display. The replacement-tax computation lives in the
// il1120st output node.

// ─── Schema ───────────────────────────────────────────────────────────────────

export const inputSchema = z.object({
  // Entity identification
  corporation_name: z.string().min(1),
  fein: z.string().optional(),

  // Mailing address
  address: z.string().optional(),
  city: z.string().optional(),
  state: z.string().optional(),
  zip: z.string().optional(),
});

// ─── Node class ───────────────────────────────────────────────────────────────

class GeneralNode extends TaxNode<typeof inputSchema> {
  readonly nodeType = "general";
  readonly inputSchema = inputSchema;
  readonly outputNodes = new OutputNodes([]);

  // The start node already deposits the raw identity into pending["general"];
  // the node validates it but does NOT re-echo it, since a duplicate scalar
  // deposit would be promoted into an array by the executor's accumulation
  // pattern (same rationale as the f1120 general node not re-echoing identity).
  compute(_ctx: NodeContext, rawInput: z.infer<typeof inputSchema>): NodeResult {
    inputSchema.parse(rawInput);
    return { outputs: [] };
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const general = new GeneralNode();
