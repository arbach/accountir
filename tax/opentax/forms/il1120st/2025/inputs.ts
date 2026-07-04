import type { InputNodeEntry } from "../../../core/types/form-definition.ts";
import { general, inputSchema as generalInputSchema } from "./nodes/inputs/general/index.ts";
import { il1120st, inputSchema as il1120stInputSchema } from "./nodes/outputs/il1120st/index.ts";

// The il1120st node is BOTH a singleton input (the federal base figures and IL
// modifications are supplied here) and the output node that assembles the
// replacement-tax computation. Mirrors the f1120 / f1120s dual-role pattern.
export const inputNodes: readonly InputNodeEntry[] = [
  { node: general, inputSchema: generalInputSchema, isArray: false },
  { node: il1120st, inputSchema: il1120stInputSchema, isArray: false },
];
