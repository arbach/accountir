/**
 * E2E scenarios — Form IL-1120, TY2025 (C-corp income + replacement tax).
 *
 * Runs complete IL-1120 returns through the node graph and asserts IL base
 * income, the IL NOL deduction, IL net income, the 9.5% combined tax, and the
 * IL NOL remaining that the ledger bridge feeds forward to the next year.
 *
 * Inputs use SINGULAR node-type keys (the start node routes by nodeType):
 *   general → { general: {...} }, il1120 → { il1120: {...} }
 */

import { assertEquals } from "@std/assert";
import { buildExecutionPlan } from "../../../../core/runtime/planner.ts";
import { execute, type ExecuteResult } from "../../../../core/runtime/executor.ts";
import { registry } from "../registry.ts";

const ctx = { taxYear: 2025, formType: "il1120" };
const plan = buildExecutionPlan(registry);

function runReturn(inputs: Record<string, unknown>): ExecuteResult {
  return execute(plan, registry, inputs, ctx);
}

function r2(n: number): number {
  return Math.round(n * 100) / 100;
}

function num(v: unknown): number {
  if (Array.isArray(v)) return typeof v.at(-1) === "number" ? (v.at(-1) as number) : 0;
  return typeof v === "number" ? v : 0;
}

Deno.test("e2e: MAVEN FINANCIAL TECHNOLOGIES INC — 2025 loss → $0 IL tax, IL NOL carryforward", () => {
  const result = runReturn({
    general: { corporation_name: "MAVEN FINANCIAL TECHNOLOGIES INC", fein: "92-3379962" },
    il1120: { federal_taxable_income: -3_886.33 },
  });
  const f = result.pending["il1120"];
  assertEquals(num(f["il_net_income"]), 0);
  assertEquals(num(f["total_il_tax"]), 0);
  assertEquals(r2(num(f["il_nol_remaining"])), 3_886.33);

  const g = result.pending["general"];
  assertEquals(g["corporation_name"], "MAVEN FINANCIAL TECHNOLOGIES INC");
});

Deno.test("e2e: MAVEN — three-year IL NOL accumulation driven by the bridge", () => {
  const y2023 = runReturn({
    general: { corporation_name: "MAVEN" },
    il1120: { federal_taxable_income: -94_625.43 },
  });
  const rem2023 = r2(num(y2023.pending["il1120"]["il_nol_remaining"]));
  assertEquals(rem2023, 94_625.43);

  const y2024 = runReturn({
    general: { corporation_name: "MAVEN" },
    il1120: { federal_taxable_income: -90_908.28, il_nol_available: rem2023 },
  });
  const rem2024 = r2(num(y2024.pending["il1120"]["il_nol_remaining"]));
  assertEquals(rem2024, 185_533.71);

  const y2025 = runReturn({
    general: { corporation_name: "MAVEN" },
    il1120: { federal_taxable_income: -3_886.33, il_nol_available: rem2024 },
  });
  const f2025 = y2025.pending["il1120"];
  assertEquals(num(f2025["total_il_tax"]), 0);
  assertEquals(r2(num(f2025["il_nol_remaining"])), 189_420.04);
});

Deno.test("e2e: profit year consumes accumulated IL NOL then pays 9.5% on remainder", () => {
  // 250,000 base with 189,420.04 IL NOL → net 60,579.96 × 9.5% = 5,755.0962 → 5,755.10.
  const result = runReturn({
    general: { corporation_name: "MAVEN" },
    il1120: { federal_taxable_income: 250_000, il_nol_available: 189_420.04 },
  });
  const f = result.pending["il1120"];
  assertEquals(r2(num(f["il_nol_deduction"])), 189_420.04);
  assertEquals(r2(num(f["il_net_income"])), 60_579.96);
  // income 60,579.96 × 7% = 4,240.5972 → 4,240.60; replacement × 2.5% = 1,514.499 → 1,514.50.
  assertEquals(num(f["income_tax"]), 4_240.60);
  assertEquals(num(f["replacement_tax"]), 1_514.50);
  assertEquals(num(f["total_il_tax"]), 5_755.10);
  assertEquals(num(f["il_nol_remaining"]), 0);
});
