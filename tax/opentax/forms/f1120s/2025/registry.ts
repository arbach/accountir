import type { NodeRegistry } from "../../../core/types/node-registry.ts";
import { buildStartNode, inputNodes } from "./start.ts";

// ── Inputs ──────────────────────────────────────────────────────────────────
import { general } from "./nodes/inputs/general/index.ts";
import { f8825 } from "./nodes/inputs/f8825/index.ts";

// ── Intermediates ─────────────────────────────────────────────────────────────
import { schedule_k } from "./nodes/intermediate/schedule_k/index.ts";

// ── Outputs ───────────────────────────────────────────────────────────────────
import { f1120s } from "./nodes/outputs/f1120s/index.ts";
import { schedule_k1 } from "./nodes/outputs/schedule_k1/index.ts";

const start = buildStartNode(inputNodes);

export const registry: NodeRegistry = {
  // ── Start ──────────────────────────────────────────────────────────────────
  start,

  // ── Inputs ─────────────────────────────────────────────────────────────────
  general,
  f8825,

  // ── Intermediates ───────────────────────────────────────────────────────────
  schedule_k,

  // ── Outputs ─────────────────────────────────────────────────────────────────
  f1120s,
  schedule_k1,
};
