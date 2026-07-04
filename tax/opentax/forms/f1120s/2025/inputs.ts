import type { InputNodeEntry } from "../../../core/types/form-definition.ts";
import { general, inputSchema as generalInputSchema } from "./nodes/inputs/general/index.ts";
import { f8825, itemSchema as f8825ItemSchema } from "./nodes/inputs/f8825/index.ts";
import { f1120s, inputSchema as f1120sInputSchema } from "./nodes/outputs/f1120s/index.ts";

// The f1120s node is BOTH a singleton input (the ordinary income/deduction lines
// are supplied here) and the page-1 output node. Mirrors the f1040 schedule_d /
// form8889 dual-role pattern.
export const inputNodes: readonly InputNodeEntry[] = [
  { node: general, inputSchema: generalInputSchema, isArray: false },
  { node: f1120s, inputSchema: f1120sInputSchema, isArray: false },
  { node: f8825, itemSchema: f8825ItemSchema, isArray: true },
];
