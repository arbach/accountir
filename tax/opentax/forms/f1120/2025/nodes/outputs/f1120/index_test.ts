import { assertEquals } from "@std/assert";
import { f1120 } from "./index.ts";

const ctx = { taxYear: 2025, formType: "f1120" };

function lines(result: ReturnType<typeof f1120.compute>): Record<string, unknown> {
  const self = result.outputs.find((o) => o.nodeType === "f1120");
  return (self?.fields ?? {}) as Record<string, unknown>;
}

Deno.test("f1120: total income assembles from receipts, profit and other income lines", () => {
  // 1c = 500,000 − 10,000 = 490,000; gross profit = 490,000 − 200,000 = 290,000.
  // line11 = 290,000 + 4,000 + 1,000 + 6,000 + 8,000 + 2,000 = 311,000.
  const result = f1120.compute(ctx, {
    line1a_gross_receipts: 500_000,
    line1b_returns_allowances: 10_000,
    line2_cogs: 200_000,
    line4_dividends: 4_000,
    line5_interest: 1_000,
    line6_gross_rents: 6_000,
    line8_capital_gain: 8_000,
    line10_other_income: 2_000,
  });
  const l = lines(result);
  assertEquals(l.line1c_net_receipts, 490_000);
  assertEquals(l.line3_gross_profit, 290_000);
  assertEquals(l.line11_total_income, 311_000);
});

Deno.test("f1120: income before NOL = total income − total deductions; 21% tax on positive income", () => {
  // line11 = 300,000. Deductions: 80,000 + 60,000 + 5,000 + 1,000 + 12,000 + 20,000 = 178,000.
  // line28 = 300,000 − 178,000 = 122,000. No NOL. line30 = 122,000.
  // line31 = 122,000 × 0.21 = 25,620.
  const result = f1120.compute(ctx, {
    line1a_gross_receipts: 300_000,
    line12_officer_compensation: 80_000,
    line13_salaries_wages: 60_000,
    line17_taxes_licenses: 5_000,
    line19_charitable: 1_000,
    line20_depreciation: 12_000,
    line26_other_deductions: 20_000,
  });
  const l = lines(result);
  assertEquals(l.line27_total_deductions, 178_000);
  assertEquals(l.line28_income_before_nol, 122_000);
  assertEquals(l.line29a_nol_deduction, 0);
  assertEquals(l.line30_taxable_income, 122_000);
  assertEquals(l.line31_total_tax, 25_620);
});

Deno.test("f1120: loss year → negative line 28, zero tax, carryforward generated", () => {
  // 294,700 receipts; 298,586.33 other deductions → line28 = −3,886.33 (MAVEN 2025).
  const result = f1120.compute(ctx, {
    line1a_gross_receipts: 294_700,
    line26_other_deductions: 298_586.33,
  });
  const l = lines(result);
  assertEquals(l.line28_income_before_nol, -3_886.33);
  assertEquals(l.line29a_nol_deduction, 0);
  assertEquals(l.line30_taxable_income, 0);
  assertEquals(l.line31_total_tax, 0);
  assertEquals(l.nol_carryforward_generated, 3_886.33);
  assertEquals(l.nol_carryforward_remaining, 3_886.33);
});

Deno.test("f1120: NOL deduction capped at positive income (cannot create a loss)", () => {
  // line28 = 50,000 profit; 200,000 NOL available. NOL limited to 50,000.
  // line30 = 0, tax = 0. Remaining carryforward = 200,000 − 50,000 = 150,000.
  const result = f1120.compute(ctx, {
    line1a_gross_receipts: 50_000,
    nol_carryforward_available: 200_000,
  });
  const l = lines(result);
  assertEquals(l.line28_income_before_nol, 50_000);
  assertEquals(l.line29a_nol_deduction, 50_000);
  assertEquals(l.line30_taxable_income, 0);
  assertEquals(l.line31_total_tax, 0);
  assertEquals(l.nol_carryforward_generated, 0);
  assertEquals(l.nol_carryforward_remaining, 150_000);
});

Deno.test("f1120: NOL deduction capped at available carryforward; tax on remainder", () => {
  // line28 = 100,000 profit; only 30,000 NOL available. NOL = 30,000.
  // line30 = 70,000. tax = 70,000 × 0.21 = 14,700. Remaining = 0.
  const result = f1120.compute(ctx, {
    line1a_gross_receipts: 100_000,
    nol_carryforward_available: 30_000,
  });
  const l = lines(result);
  assertEquals(l.line28_income_before_nol, 100_000);
  assertEquals(l.line29a_nol_deduction, 30_000);
  assertEquals(l.line30_taxable_income, 70_000);
  assertEquals(l.line31_total_tax, 14_700);
  assertEquals(l.nol_carryforward_remaining, 0);
});

Deno.test("f1120: loss year with prior carryforward — no absorption, pool grows", () => {
  // line28 = −10,000 loss; 25,000 NOL already available. No income to offset.
  // generated = 10,000; remaining = 25,000 − 0 + 10,000 = 35,000.
  const result = f1120.compute(ctx, {
    line1a_gross_receipts: 40_000,
    line26_other_deductions: 50_000,
    nol_carryforward_available: 25_000,
  });
  const l = lines(result);
  assertEquals(l.line28_income_before_nol, -10_000);
  assertEquals(l.line29a_nol_deduction, 0);
  assertEquals(l.nol_carryforward_generated, 10_000);
  assertEquals(l.nol_carryforward_remaining, 35_000);
});

Deno.test("f1120: empty input yields zero across the board", () => {
  const l = lines(f1120.compute(ctx, {}));
  assertEquals(l.line11_total_income, 0);
  assertEquals(l.line28_income_before_nol, 0);
  assertEquals(l.line30_taxable_income, 0);
  assertEquals(l.line31_total_tax, 0);
  assertEquals(l.nol_carryforward_remaining, 0);
});
