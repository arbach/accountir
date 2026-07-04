/**
 * E2E scenarios — Form 1120-S, TY2025.
 *
 * Runs complete S-corp returns through the node graph and asserts page-1
 * ordinary income (line 21), Schedule K separately-stated items, and the
 * single 100% shareholder's Schedule K-1 boxes.
 *
 * Inputs use SINGULAR node-type keys (the start node routes by nodeType):
 *   general → { general: {...} }, f1120s → { f1120s: {...} }, f8825 → { f8825: [...] }
 */

import { assertEquals } from "@std/assert";
import { buildExecutionPlan } from "../../../../core/runtime/planner.ts";
import { execute, type ExecuteResult } from "../../../../core/runtime/executor.ts";
import { registry } from "../registry.ts";

const ctx = { taxYear: 2025, formType: "f1120s" };
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

Deno.test("e2e: Hayat Health LLC — ordinary trade/business loss reconciles to book net", () => {
  // 2025 book net −26,602.24. $90,000 consulting revenue; deductions exceed it.
  const result = runReturn({
    general: {
      corporation_name: "Hayat Health LLC",
      ein: "33-2127261",
      shareholder_name: "Michael Arbach",
      shareholder_tin: "111-22-3333",
      ownership_percentage: 100,
      is_officer_compensated: true,
    },
    f1120s: {
      line1a_gross_receipts: 90_000,
      line7_officer_compensation: 70_000,
      line19_other_deductions: 46_602.24,
    },
  });

  const f = result.pending["f1120s"];
  assertEquals(r2(num(f["line21_ordinary_business_income"])), -26_602.24);

  const k = result.pending["schedule_k"];
  assertEquals(r2(num(k["line1_ordinary_business_income"])), -26_602.24);
  assertEquals(num(k["line2_net_rental_real_estate"]), 0);

  const k1 = result.pending["schedule_k1"];
  assertEquals(r2(num(k1["box1_ordinary_business_income"])), -26_602.24);
  assertEquals(num(k1["box2_net_rental_real_estate"]), 0);
  assertEquals(k1["shareholder_name"], "Michael Arbach");
});

Deno.test("e2e: SWEET HOME KC LLC — rentals (8825) reconcile to book net on Schedule K line 2", () => {
  // 2025 book net +23,809.01, entirely from rental real estate via Form 8825.
  const result = runReturn({
    general: {
      corporation_name: "SWEET HOME KC LLC",
      ein: "93-2942628",
      shareholder_name: "Michael Arbach",
      shareholder_tin: "111-22-3333",
      ownership_percentage: 100,
      line16d_distributions: 10_000,
    },
    f8825: [
      { property_address: "Prop 1, KC MO", gross_rents: 18_000, expense_repairs: 3_000, expense_taxes: 2_000 },
      { property_address: "Prop 2, KC MO", gross_rents: 20_000, expense_depreciation: 6_000, expense_insurance: 1_190.99 },
      { property_address: "Prop 3, KC MO", gross_rents: 12_000, expense_utilities: 4_000, expense_interest: 10_000.00 },
    ],
  });

  // (18,000−5,000) + (20,000−7,190.99) + (12,000−14,000)
  //  = 13,000 + 12,809.01 + (−2,000) = 23,809.01 → ties to 2025 book net.
  const k = result.pending["schedule_k"];
  // Rental net is separately stated on line 2; ordinary income (line 1) stays 0.
  assertEquals(num(k["line1_ordinary_business_income"]), 0);
  assertEquals(r2(num(k["line2_net_rental_real_estate"])), 23_809.01);
  assertEquals(num(k["line16d_distributions"]), 10_000);

  const k1 = result.pending["schedule_k1"];
  assertEquals(num(k1["box1_ordinary_business_income"]), 0);
  assertEquals(r2(num(k1["box2_net_rental_real_estate"])), 23_809.01);
  assertEquals(num(k1["box16d_distributions"]), 10_000);
});

Deno.test("e2e: combined ordinary + rental keeps line 1 and line 2 separated", () => {
  const result = runReturn({
    general: {
      corporation_name: "Combo LLC",
      shareholder_name: "Owner",
      ownership_percentage: 100,
      line4_interest_income: 250,
    },
    f1120s: {
      line1a_gross_receipts: 200_000,
      line2_cogs: 50_000,
      line8_salaries_wages: 40_000,
      line19_other_deductions: 10_000,
    },
    f8825: [{ property_address: "Rental", gross_rents: 30_000, expense_taxes: 5_000 }],
  });

  const k = result.pending["schedule_k"];
  // Ordinary: 200,000 − 50,000 − 40,000 − 10,000 = 100,000
  assertEquals(num(k["line1_ordinary_business_income"]), 100_000);
  // Rental: 30,000 − 5,000 = 25,000 (NOT added to ordinary)
  assertEquals(num(k["line2_net_rental_real_estate"]), 25_000);
  assertEquals(num(k["line4_interest_income"]), 250);

  const k1 = result.pending["schedule_k1"];
  assertEquals(num(k1["box1_ordinary_business_income"]), 100_000);
  assertEquals(num(k1["box2_net_rental_real_estate"]), 25_000);
  assertEquals(num(k1["box4_interest_income"]), 250);
});
