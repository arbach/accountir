import type { InputNodeEntry } from "../../../core/types/form-definition.ts";
import { general, inputSchema as generalInputSchema } from "./nodes/inputs/general/index.ts";
import { f1120, inputSchema as f1120InputSchema } from "./nodes/outputs/f1120/index.ts";

// The f1120 node is BOTH a singleton input (the income/deduction lines are
// supplied here) and the page-1 output node. Mirrors the f1120-S / f1040
// schedule_d dual-role pattern.
export const inputNodes: readonly InputNodeEntry[] = [
  { node: general, inputSchema: generalInputSchema, isArray: false },
  { node: f1120, inputSchema: f1120InputSchema, isArray: false },
];
