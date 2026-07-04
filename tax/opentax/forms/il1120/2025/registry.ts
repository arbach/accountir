import type { NodeRegistry } from "../../../core/types/node-registry.ts";
import { buildStartNode, inputNodes } from "./start.ts";

// ── Inputs ──────────────────────────────────────────────────────────────────
import { general } from "./nodes/inputs/general/index.ts";

// ── Outputs ───────────────────────────────────────────────────────────────────
import { il1120 } from "./nodes/outputs/il1120/index.ts";

const start = buildStartNode(inputNodes);

export const registry: NodeRegistry = {
  // ── Start ──────────────────────────────────────────────────────────────────
  start,

  // ── Inputs ─────────────────────────────────────────────────────────────────
  general,

  // ── Outputs ─────────────────────────────────────────────────────────────────
  il1120,
};
