import type { InputNodeEntry } from "../../../core/types/form-definition.ts";
import { general, inputSchema as generalInputSchema } from "./nodes/inputs/general/index.ts";
import { il1120, inputSchema as il1120InputSchema } from "./nodes/outputs/il1120/index.ts";

// The il1120 node is BOTH a singleton input (the federal taxable income and IL
// modifications are supplied here) and the output node that assembles the
// income/replacement-tax computation. Mirrors the f1120 dual-role pattern.
export const inputNodes: readonly InputNodeEntry[] = [
  { node: general, inputSchema: generalInputSchema, isArray: false },
  { node: il1120, inputSchema: il1120InputSchema, isArray: false },
];
