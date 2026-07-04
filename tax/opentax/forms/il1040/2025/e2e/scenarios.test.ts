/**
 * E2E scenarios — Form IL-1040, TY2025 (individual flat 4.95% tax).
 *
 * Runs complete IL-1040 returns through the node graph and asserts IL base
 * income, the personal-exemption allowance, IL net income, and the 4.95% tax
 * computed from the federal AGI the ledger bridge feeds from the computed 1040.
 *
 * Inputs use SINGULAR node-type keys (the start node routes by nodeType):
 *   general → { general: {...} }, il1040 → { il1040: {...} }
 */

import { assertEquals } from "@std/assert";
import { buildExecutionPlan } from "../../../../core/runtime/planner.ts";
import { execute, type ExecuteResult } from "../../../../core/runtime/executor.ts";
import { registry } from "../registry.ts";

const ctx = { taxYear: 2025, formType: "il1040" };
const plan = buildExecutionPlan(registry);

function runReturn(inputs: Record<string, unknown>, year = 2025): ExecuteResult {
  return execute(plan, registry, inputs, { taxYear: year, formType: "il1040" });
}

function num(v: unknown): number {
  if (Array.isArray(v)) return typeof v.at(-1) === "number" ? (v.at(-1) as number) : 0;
  return typeof v === "number" ? v : 0;
}

Deno.test("e2e: Michael & Andrea Arbach — 2025 AGI 233,673.65, 5 exemptions → 10,861.47", () => {
  const result = runReturn({
    general: {
      taxpayer_name: "Michael Arbach",
      spouse_name: "Andrea Arbach",
      filing_status: "MFJ",
    },
    il1040: {
      federal_agi: 233_673.65,
      exemption_count: 5,
    },
  });
  const f = result.pending["il1040"];
  assertEquals(num(f["total_exemptions"]), 14_250);
  assertEquals(num(f["il_net_income"]), 219_423.65);
  assertEquals(num(f["il_tax"]), 10_861.47);

  const g = result.pending["general"];
  assertEquals(g["taxpayer_name"], "Michael Arbach");
});

Deno.test("e2e: prior-year (2024) uses the 2024 exemption of 2,775 by default", () => {
  const result = runReturn({
    general: { taxpayer_name: "Michael Arbach", spouse_name: "Andrea Arbach" },
    il1040: { federal_agi: 233_673.65, exemption_count: 5 },
  }, 2024);
  const f = result.pending["il1040"];
  // 5 × 2,775 = 13,875 → net 219,798.65 × 4.95% = 10,880.03 (10,880.03317…).
  assertEquals(num(f["total_exemptions"]), 13_875);
  assertEquals(num(f["il_net_income"]), 219_798.65);
  assertEquals(num(f["il_tax"]), 10_880.03);
});
