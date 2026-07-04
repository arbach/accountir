/**
 * E2E scenarios — Form IL-1120-ST, TY2025 (S-corp replacement tax).
 *
 * Runs complete IL-1120-ST returns through the node graph and asserts the IL
 * net income and the 1.5% replacement tax that the ledger bridge feeds from the
 * already-computed federal Form 1120-S.
 *
 * Inputs use SINGULAR node-type keys (the start node routes by nodeType):
 *   general → { general: {...} }, il1120st → { il1120st: {...} }
 */

import { assertEquals } from "@std/assert";
import { buildExecutionPlan } from "../../../../core/runtime/planner.ts";
import { execute, type ExecuteResult } from "../../../../core/runtime/executor.ts";
import { registry } from "../registry.ts";

const ctx = { taxYear: 2025, formType: "il1120st" };
const plan = buildExecutionPlan(registry);

function runReturn(inputs: Record<string, unknown>): ExecuteResult {
  return execute(plan, registry, inputs, ctx);
}

function num(v: unknown): number {
  if (Array.isArray(v)) return typeof v.at(-1) === "number" ? (v.at(-1) as number) : 0;
  return typeof v === "number" ? v : 0;
}

Deno.test("e2e: SWEET HOME KC LLC — replacement tax on net rental income", () => {
  const result = runReturn({
    general: { corporation_name: "SWEET HOME KC LLC", fein: "00-0000000" },
    il1120st: {
      federal_ordinary_income: 0,
      federal_net_rental: 23_809.01,
    },
  });
  const f = result.pending["il1120st"];
  assertEquals(num(f["il_net_income"]), 23_809.01);
  assertEquals(num(f["replacement_tax"]), 357.14);

  const g = result.pending["general"];
  assertEquals(g["corporation_name"], "SWEET HOME KC LLC");
});

Deno.test("e2e: Hayat Health LLC — ordinary income → 1.5% replacement tax", () => {
  const result = runReturn({
    general: { corporation_name: "Hayat Health LLC" },
    il1120st: {
      federal_ordinary_income: 200_000,
    },
  });
  const f = result.pending["il1120st"];
  assertEquals(num(f["il_net_income"]), 200_000);
  // 200,000 × 1.5% = 3,000.
  assertEquals(num(f["replacement_tax"]), 3_000);
});

Deno.test("e2e: loss year → $0 replacement tax", () => {
  const result = runReturn({
    general: { corporation_name: "Hayat Health LLC" },
    il1120st: { federal_ordinary_income: -12_345.67 },
  });
  const f = result.pending["il1120st"];
  assertEquals(num(f["replacement_tax"]), 0);
});
