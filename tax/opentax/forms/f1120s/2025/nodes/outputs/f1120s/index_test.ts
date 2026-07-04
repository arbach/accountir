import { assertEquals } from "@std/assert";
import { f1120s } from "./index.ts";

const ctx = { taxYear: 2025, formType: "f1120s" };

function lines(result: ReturnType<typeof f1120s.compute>): Record<string, unknown> {
  const self = result.outputs.find((o) => o.nodeType === "f1120s");
  return (self?.fields ?? {}) as Record<string, unknown>;
}

function scheduleKLine1(result: ReturnType<typeof f1120s.compute>): number {
  const out = result.outputs.find((o) => o.nodeType === "schedule_k");
  return (out?.fields["line1_ordinary_business_income"] as number) ?? 0;
}

Deno.test("f1120s: ordinary business income = receipts − COGS − deductions", () => {
  // Receipts 500,000 − returns 10,000 = 490,000 net; − COGS 200,000 = 290,000 gross
  // profit. Deductions: 80,000 + 60,000 + 5,000 + 12,000 + 20,000 = 177,000.
  // Line 21 = 290,000 − 177,000 = 113,000.
  const result = f1120s.compute(ctx, {
    line1a_gross_receipts: 500_000,
    line1b_returns_allowances: 10_000,
    line2_cogs: 200_000,
    line7_officer_compensation: 80_000,
    line8_salaries_wages: 60_000,
    line12_taxes: 5_000,
    line14_depreciation: 12_000,
    line19_other_deductions: 20_000,
  });

  const l = lines(result);
  assertEquals(l.line1c_net_receipts, 490_000);
  assertEquals(l.line3_gross_profit, 290_000);
  assertEquals(l.line6_total_income, 290_000);
  assertEquals(l.line20_total_deductions, 177_000);
  assertEquals(l.line21_ordinary_business_income, 113_000);
  assertEquals(scheduleKLine1(result), 113_000);
});

Deno.test("f1120s: negative ordinary income (loss) flows to Schedule K line 1", () => {
  // Hayat Health-style loss: $90,000 receipts, no COGS, deductions exceed income.
  const result = f1120s.compute(ctx, {
    line1a_gross_receipts: 90_000,
    line7_officer_compensation: 70_000,
    line19_other_deductions: 46_602.24,
  });
  const l = lines(result);
  assertEquals(l.line6_total_income, 90_000);
  assertEquals(l.line20_total_deductions, 116_602.24);
  assertEquals(l.line21_ordinary_business_income, -26_602.24);
  assertEquals(scheduleKLine1(result), -26_602.24);
});

Deno.test("f1120s: empty input yields zero ordinary income", () => {
  const result = f1120s.compute(ctx, {});
  const l = lines(result);
  assertEquals(l.line21_ordinary_business_income, 0);
});
