import { assertEquals } from "@std/assert";
import { il1040 } from "./index.ts";
import { CONFIG_BY_YEAR } from "../../../config.ts";

const ctx = { taxYear: 2025, formType: "il1040" };

function lines(result: ReturnType<typeof il1040.compute>): Record<string, unknown> {
  const self = result.outputs.find((o) => o.nodeType === "il1040");
  return (self?.fields ?? {}) as Record<string, unknown>;
}

Deno.test("il1040: Arbach 2025 — AGI 233,673.65, 5 exemptions × 2,850 → 10,861.47 tax", () => {
  const result = il1040.compute(ctx, {
    federal_agi: 233_673.65,
    exemption_count: 5,
  });
  const l = lines(result);
  assertEquals(l.exemption_per_person, 2_850);
  assertEquals(l.total_exemptions, 14_250);
  // 233,673.65 − 14,250 = 219,423.65 × 4.95% = 10,861.470675 → 10,861.47.
  assertEquals(l.il_net_income, 219_423.65);
  assertEquals(l.il_tax, 10_861.47);
});

Deno.test("il1040: defaults exemption per person from per-year config (2024)", () => {
  const result = il1040.compute({ taxYear: 2024, formType: "il1040" }, {
    federal_agi: 100_000,
    exemption_count: 2,
  });
  const l = lines(result);
  assertEquals(l.exemption_per_person, CONFIG_BY_YEAR[2024].exemptionPerPerson); // 2,775
  assertEquals(l.total_exemptions, 5_550);
  // 100,000 − 5,550 = 94,450 × 4.95% = 4,675.275 → 4,675.28.
  assertEquals(l.il_net_income, 94_450);
  assertEquals(l.il_tax, 4_675.28);
});

Deno.test("il1040: 2023 exemption default is 2,425", () => {
  const result = il1040.compute({ taxYear: 2023, formType: "il1040" }, {
    federal_agi: 50_000,
    exemption_count: 1,
  });
  const l = lines(result);
  assertEquals(l.exemption_per_person, 2_425);
  assertEquals(l.il_net_income, 47_575);
});

Deno.test("il1040: IL additions and subtractions adjust the base", () => {
  // 80,000 + 2,000 − 12,000 (e.g. retirement income IL exempts) = 70,000.
  const result = il1040.compute(ctx, {
    federal_agi: 80_000,
    il_additions: 2_000,
    il_subtractions: 12_000,
    exemption_count: 0,
  });
  const l = lines(result);
  assertEquals(l.il_base_income, 70_000);
  assertEquals(l.il_net_income, 70_000);
  // 70,000 × 4.95% = 3,465.
  assertEquals(l.il_tax, 3_465);
});

Deno.test("il1040: explicit exemption_per_person overrides the config default", () => {
  const result = il1040.compute(ctx, {
    federal_agi: 40_000,
    exemption_count: 2,
    exemption_per_person: 1_000,
  });
  const l = lines(result);
  assertEquals(l.exemption_per_person, 1_000);
  assertEquals(l.total_exemptions, 2_000);
  assertEquals(l.il_net_income, 38_000);
});

Deno.test("il1040: exemptions cannot drive net income below zero", () => {
  const result = il1040.compute(ctx, {
    federal_agi: 5_000,
    exemption_count: 5,
  });
  const l = lines(result);
  assertEquals(l.il_net_income, 0);
  assertEquals(l.il_tax, 0);
});

Deno.test("il1040: empty input yields zero", () => {
  const l = lines(il1040.compute(ctx, {}));
  assertEquals(l.il_base_income, 0);
  assertEquals(l.il_net_income, 0);
  assertEquals(l.il_tax, 0);
});
