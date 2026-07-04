import type { InputNodeEntry } from "../../../core/types/form-definition.ts";
import { general, inputSchema as generalInputSchema } from "./nodes/inputs/general/index.ts";
import { il1040, inputSchema as il1040InputSchema } from "./nodes/outputs/il1040/index.ts";

// The il1040 node is BOTH a singleton input (the federal AGI, IL modifications,
// and exemption inputs are supplied here) and the output node that assembles the
// tax computation. Mirrors the f1120 dual-role pattern.
export const inputNodes: readonly InputNodeEntry[] = [
  { node: general, inputSchema: generalInputSchema, isArray: false },
  { node: il1040, inputSchema: il1040InputSchema, isArray: false },
];
