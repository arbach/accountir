import { assertEquals } from "@std/assert";
import { schedule_k } from "./index.ts";

const ctx = { taxYear: 2025, formType: "f1120s" };

function self(result: ReturnType<typeof schedule_k.compute>): Record<string, unknown> {
  const out = result.outputs.find((o) => o.nodeType === "schedule_k");
  return (out?.fields ?? {}) as Record<string, unknown>;
}

function toK1(result: ReturnType<typeof schedule_k.compute>): Record<string, unknown> {
  const out = result.outputs.find((o) => o.nodeType === "schedule_k1");
  return (out?.fields ?? {}) as Record<string, unknown>;
}

Deno.test("schedule_k: keeps ordinary (line 1) and net rental (line 2) separate", () => {
  const result = schedule_k.compute(ctx, {
    line1_ordinary_business_income: 100_000,
    line2_net_rental_real_estate: 25_000,
    line4_interest_income: 250,
    line16d_distributions: 5_000,
  });
  const k = self(result);
  assertEquals(k.line1_ordinary_business_income, 100_000);
  assertEquals(k.line2_net_rental_real_estate, 25_000);
  assertEquals(k.line4_interest_income, 250);
  assertEquals(k.line16d_distributions, 5_000);
});

Deno.test("schedule_k: routes entity totals to schedule_k1", () => {
  const result = schedule_k.compute(ctx, {
    line1_ordinary_business_income: -26_602.24,
    line2_net_rental_real_estate: 23_809.01,
  });
  const k1 = toK1(result);
  assertEquals(k1.ordinary_business_income, -26_602.24);
  assertEquals(k1.net_rental_real_estate, 23_809.01);
});
