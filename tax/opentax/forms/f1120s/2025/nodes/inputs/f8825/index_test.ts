import { assertEquals } from "@std/assert";
import { f8825 } from "./index.ts";

const ctx = { taxYear: 2025, formType: "f1120s" };

function scheduleKLine2(result: ReturnType<typeof f8825.compute>): number {
  const out = result.outputs.find((o) => o.nodeType === "schedule_k");
  return (out?.fields["line2_net_rental_real_estate"] as number) ?? 0;
}

function selfLines(result: ReturnType<typeof f8825.compute>): Record<string, unknown> {
  const self = result.outputs.find((o) => o.nodeType === "f8825");
  return (self?.fields ?? {}) as Record<string, unknown>;
}

Deno.test("f8825: net rental income = gross rents − expenses, routes to Schedule K line 2", () => {
  const result = f8825.compute(ctx, {
    f8825s: [{
      property_address: "123 Main St, Kansas City, MO",
      gross_rents: 24_000,
      expense_insurance: 1_200,
      expense_repairs: 2_000,
      expense_taxes: 3_000,
      expense_depreciation: 4_000,
    }],
  });
  // 24,000 − (1,200 + 2,000 + 3,000 + 4,000) = 13,800
  assertEquals(scheduleKLine2(result), 13_800);
  const l = selfLines(result);
  assertEquals(l.line18a_total_gross_rents, 24_000);
  assertEquals(l.line18b_total_expenses, 10_200);
  assertEquals(l.line19_net_rental_real_estate, 13_800);
});

Deno.test("f8825: multiple properties sum to a single Schedule K line 2 amount", () => {
  const result = f8825.compute(ctx, {
    f8825s: [
      { property_address: "A", gross_rents: 12_000, expense_repairs: 2_000 },
      { property_address: "B", gross_rents: 18_000, expense_taxes: 4_000 },
      { property_address: "C", gross_rents: 10_000, expense_depreciation: 9_809.01 },
    ],
  });
  // (12,000−2,000) + (18,000−4,000) + (10,000−9,809.01)
  //  = 10,000 + 14,000 + 190.99 = 24,190.99
  assertEquals(Math.round(scheduleKLine2(result) * 100) / 100, 24_190.99);
});

Deno.test("f8825: net rental does NOT route to ordinary income line 1", () => {
  const result = f8825.compute(ctx, {
    f8825s: [{ property_address: "A", gross_rents: 10_000 }],
  });
  const line1 = result.outputs.find(
    (o) => o.nodeType === "schedule_k" && "line1_ordinary_business_income" in o.fields,
  );
  assertEquals(line1, undefined);
});
