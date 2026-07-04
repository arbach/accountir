/**
 * E2E scenarios — Form 1120, TY2025 (C corporation).
 *
 * Runs complete C-corp returns through the node graph and asserts page-1
 * income (line 11), taxable income before NOL (line 28), the NOL deduction,
 * taxable income (line 30), the 21% tax (line 31), and the NOL carryforward
 * remaining that the ledger bridge feeds forward to the next year.
 *
 * Inputs use SINGULAR node-type keys (the start node routes by nodeType):
 *   general → { general: {...} }, f1120 → { f1120: {...} }
 */

import { assertEquals } from "@std/assert";
import { buildExecutionPlan } from "../../../../core/runtime/planner.ts";
import { execute, type ExecuteResult } from "../../../../core/runtime/executor.ts";
import { registry } from "../registry.ts";

const ctx = { taxYear: 2025, formType: "f1120" };
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

Deno.test("e2e: MAVEN FINANCIAL TECHNOLOGIES INC — 2025 operating loss → $0 tax, NOL carryforward", () => {
  // 2025 book net −3,886.33: 294,700 receipts; 298,586.33 deductions.
  const result = runReturn({
    general: {
      corporation_name: "MAVEN FINANCIAL TECHNOLOGIES INC",
      ein: "92-3379962",
    },
    f1120: {
      line1a_gross_receipts: 294_700,
      line26_other_deductions: 298_586.33,
    },
  });

  const f = result.pending["f1120"];
  assertEquals(r2(num(f["line28_income_before_nol"])), -3_886.33);
  assertEquals(num(f["line30_taxable_income"]), 0);
  assertEquals(num(f["line31_total_tax"]), 0);
  assertEquals(r2(num(f["nol_carryforward_generated"])), 3_886.33);
  assertEquals(r2(num(f["nol_carryforward_remaining"])), 3_886.33);

  const g = result.pending["general"];
  assertEquals(g["corporation_name"], "MAVEN FINANCIAL TECHNOLOGIES INC");
});

Deno.test("e2e: MAVEN — three-year NOL accumulation driven by the bridge", () => {
  // 2023 loss −94,625.43, 2024 loss −90,908.28, 2025 loss −3,886.33.
  // Each year the bridge feeds the prior remaining as nol_carryforward_available.
  const y2023 = runReturn({
    general: { corporation_name: "MAVEN", ein: "92-3379962" },
    f1120: { line1a_gross_receipts: 0, line26_other_deductions: 94_625.43 },
  });
  const rem2023 = r2(num(y2023.pending["f1120"]["nol_carryforward_remaining"]));
  assertEquals(rem2023, 94_625.43);

  const y2024 = runReturn({
    general: { corporation_name: "MAVEN" },
    f1120: {
      line1a_gross_receipts: 0,
      line26_other_deductions: 90_908.28,
      nol_carryforward_available: rem2023,
    },
  });
  const rem2024 = r2(num(y2024.pending["f1120"]["nol_carryforward_remaining"]));
  // 94,625.43 + 90,908.28 = 185,533.71
  assertEquals(rem2024, 185_533.71);

  const y2025 = runReturn({
    general: { corporation_name: "MAVEN" },
    f1120: {
      line1a_gross_receipts: 294_700,
      line26_other_deductions: 298_586.33,
      nol_carryforward_available: rem2024,
    },
  });
  const f2025 = y2025.pending["f1120"];
  assertEquals(num(f2025["line31_total_tax"]), 0);
  // 185,533.71 + 3,886.33 = 189,420.04
  assertEquals(r2(num(f2025["nol_carryforward_remaining"])), 189_420.04);
});

Deno.test("e2e: profit year consumes accumulated NOL then pays 21% on the remainder", () => {
  // 250,000 income before NOL with 189,420.04 carryforward available.
  // NOL absorbs 189,420.04 → taxable income 60,579.96 → tax 12,721.79.
  const result = runReturn({
    general: { corporation_name: "MAVEN" },
    f1120: {
      line1a_gross_receipts: 250_000,
      nol_carryforward_available: 189_420.04,
    },
  });
  const f = result.pending["f1120"];
  assertEquals(num(f["line28_income_before_nol"]), 250_000);
  assertEquals(r2(num(f["line29a_nol_deduction"])), 189_420.04);
  assertEquals(r2(num(f["line30_taxable_income"])), 60_579.96);
  // 60,579.96 × 0.21 = 12,721.7916 → 12,721.79
  assertEquals(num(f["line31_total_tax"]), 12_721.79);
  assertEquals(num(f["nol_carryforward_remaining"]), 0);
});
