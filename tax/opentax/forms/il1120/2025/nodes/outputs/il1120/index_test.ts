import { assertEquals } from "@std/assert";
import { il1120 } from "./index.ts";

const ctx = { taxYear: 2025, formType: "il1120" };

function lines(result: ReturnType<typeof il1120.compute>): Record<string, unknown> {
  const self = result.outputs.find((o) => o.nodeType === "il1120");
  return (self?.fields ?? {}) as Record<string, unknown>;
}

Deno.test("il1120: 9.5% combined tax on positive net income (no IL NOL)", () => {
  // base = 100,000. net = 100,000. income tax 7,000; replacement 2,500; total 9,500.
  const result = il1120.compute(ctx, { federal_taxable_income: 100_000 });
  const l = lines(result);
  assertEquals(l.il_base_income, 100_000);
  assertEquals(l.il_net_income, 100_000);
  assertEquals(l.income_tax, 7_000);
  assertEquals(l.replacement_tax, 2_500);
  assertEquals(l.total_il_tax, 9_500);
  assertEquals(l.il_nol_remaining, 0);
});

Deno.test("il1120: IL additions and subtractions adjust the base", () => {
  // 80,000 + 5,000 − 2,000 = 83,000. tax = 83,000 × 9.5% = 7,885.
  const result = il1120.compute(ctx, {
    federal_taxable_income: 80_000,
    il_additions: 5_000,
    il_subtractions: 2_000,
  });
  const l = lines(result);
  assertEquals(l.il_base_income, 83_000);
  assertEquals(l.il_net_income, 83_000);
  assertEquals(l.income_tax, 5_810);
  assertEquals(l.replacement_tax, 2_075);
  assertEquals(l.total_il_tax, 7_885);
});

Deno.test("il1120: loss year → $0 tax, IL NOL generated", () => {
  // MAVEN-style operating loss → base −3,886.33, net 0, $0 tax, NOL pool grows.
  const result = il1120.compute(ctx, { federal_taxable_income: -3_886.33 });
  const l = lines(result);
  assertEquals(l.il_base_income, -3_886.33);
  assertEquals(l.il_net_income, 0);
  assertEquals(l.total_il_tax, 0);
  assertEquals(l.il_nol_generated, 3_886.33);
  assertEquals(l.il_nol_remaining, 3_886.33);
});

Deno.test("il1120: IL NOL capped at positive base (cannot create a loss)", () => {
  // base 50,000; 200,000 NOL available → deduction 50,000, net 0, $0 tax.
  // remaining = 200,000 − 50,000 = 150,000.
  const result = il1120.compute(ctx, {
    federal_taxable_income: 50_000,
    il_nol_available: 200_000,
  });
  const l = lines(result);
  assertEquals(l.il_nol_deduction, 50_000);
  assertEquals(l.il_net_income, 0);
  assertEquals(l.total_il_tax, 0);
  assertEquals(l.il_nol_remaining, 150_000);
});

Deno.test("il1120: IL NOL capped at available carryforward; 9.5% on remainder", () => {
  // base 100,000; only 30,000 NOL → net 70,000 × 9.5% = 6,650. remaining 0.
  const result = il1120.compute(ctx, {
    federal_taxable_income: 100_000,
    il_nol_available: 30_000,
  });
  const l = lines(result);
  assertEquals(l.il_nol_deduction, 30_000);
  assertEquals(l.il_net_income, 70_000);
  assertEquals(l.income_tax, 4_900);
  assertEquals(l.replacement_tax, 1_750);
  assertEquals(l.total_il_tax, 6_650);
  assertEquals(l.il_nol_remaining, 0);
});

Deno.test("il1120: loss with prior carryforward — no absorption, pool grows", () => {
  // base −10,000; 25,000 NOL available. remaining = 25,000 − 0 + 10,000 = 35,000.
  const result = il1120.compute(ctx, {
    federal_taxable_income: -10_000,
    il_nol_available: 25_000,
  });
  const l = lines(result);
  assertEquals(l.il_nol_deduction, 0);
  assertEquals(l.il_nol_generated, 10_000);
  assertEquals(l.il_nol_remaining, 35_000);
});

Deno.test("il1120: empty input yields zero across the board", () => {
  const l = lines(il1120.compute(ctx, {}));
  assertEquals(l.il_base_income, 0);
  assertEquals(l.il_net_income, 0);
  assertEquals(l.total_il_tax, 0);
  assertEquals(l.il_nol_remaining, 0);
});
