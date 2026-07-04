import { assertEquals } from "@std/assert";
import { schedule_k1 } from "./index.ts";

const ctx = { taxYear: 2025, formType: "f1120s" };

function k1(result: ReturnType<typeof schedule_k1.compute>): Record<string, unknown> {
  const self = result.outputs.find((o) => o.nodeType === "schedule_k1");
  return (self?.fields ?? {}) as Record<string, unknown>;
}

Deno.test("schedule_k1: 100% shareholder gets full entity totals", () => {
  const result = schedule_k1.compute(ctx, {
    shareholder_name: "Michael Arbach",
    shareholder_tin: "123-45-6789",
    ownership_percentage: 100,
    ordinary_business_income: -26_602.24,
    net_rental_real_estate: 23_809.01,
    distributions: 5_000,
    interest_income: 100,
  });
  const box = k1(result);
  assertEquals(box.box1_ordinary_business_income, -26_602.24);
  assertEquals(box.box2_net_rental_real_estate, 23_809.01);
  assertEquals(box.box16d_distributions, 5_000);
  assertEquals(box.box4_interest_income, 100);
});

Deno.test("schedule_k1: ownership percentage scales each box pro-rata", () => {
  const result = schedule_k1.compute(ctx, {
    ownership_percentage: 60,
    ordinary_business_income: 100_000,
    net_rental_real_estate: 50_000,
    section_179: 10_000,
  });
  const box = k1(result);
  assertEquals(box.box1_ordinary_business_income, 60_000);
  assertEquals(box.box2_net_rental_real_estate, 30_000);
  assertEquals(box.box11_section_179, 6_000);
});

Deno.test("schedule_k1: missing ownership defaults to 100% (full share)", () => {
  const result = schedule_k1.compute(ctx, { ordinary_business_income: 1_000 });
  const box = k1(result);
  assertEquals(box.box1_ordinary_business_income, 1_000);
});
