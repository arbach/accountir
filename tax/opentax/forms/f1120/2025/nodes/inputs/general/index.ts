import { z } from "zod";
import type { NodeResult } from "../../../../../../core/types/tax-node.ts";
import { TaxNode } from "../../../../../../core/types/tax-node.ts";
import { OutputNodes } from "../../../../../../core/types/output-nodes.ts";
import type { NodeContext } from "../../../../../../core/types/node-context.ts";

// General / entity-identity input for Form 1120 (C corporation).
//
// A C corporation files its own return and pays its own tax; there is no
// pass-through to shareholders and no Schedule K-1. This node therefore only
// carries entity identification (name, EIN, address) and self-deposits it for
// display.

// ─── Schema ───────────────────────────────────────────────────────────────────

export const inputSchema = z.object({
  // Entity identification
  corporation_name: z.string().min(1),
  ein: z.string().optional(),

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
  // pattern (same rationale as the Schedule K-1 node not re-echoing identity).
  compute(_ctx: NodeContext, rawInput: z.infer<typeof inputSchema>): NodeResult {
    inputSchema.parse(rawInput);
    return { outputs: [] };
  }
}

// ─── Singleton export ─────────────────────────────────────────────────────────

export const general = new GeneralNode();
