import { assertEquals } from "@std/assert";
import { il1120st } from "./index.ts";

const ctx = { taxYear: 2025, formType: "il1120st" };

function lines(result: ReturnType<typeof il1120st.compute>): Record<string, unknown> {
  const self = result.outputs.find((o) => o.nodeType === "il1120st");
  return (self?.fields ?? {}) as Record<string, unknown>;
}

Deno.test("il1120st: replacement tax on net rental income (SWEET HOME KC)", () => {
  // Net rental 23,809.01, no ordinary income → 23,809.01 × 1.5% = 357.135 → 357.14.
  const result = il1120st.compute(ctx, {
    federal_ordinary_income: 0,
    federal_net_rental: 23_809.01,
  });
  const l = lines(result);
  assertEquals(l.il_net_income, 23_809.01);
  assertEquals(l.replacement_tax, 357.14);
});

Deno.test("il1120st: ordinary income plus rental, IL modifications", () => {
  // 100,000 + 20,000 + 5,000 additions − 3,000 subtractions = 122,000.
  // 122,000 × 1.5% = 1,830.
  const result = il1120st.compute(ctx, {
    federal_ordinary_income: 100_000,
    federal_net_rental: 20_000,
    il_additions: 5_000,
    il_subtractions: 3_000,
  });
  const l = lines(result);
  assertEquals(l.il_net_income, 122_000);
  assertEquals(l.replacement_tax, 1_830);
});

Deno.test("il1120st: net loss → $0 base, $0 replacement tax", () => {
  const result = il1120st.compute(ctx, {
    federal_ordinary_income: -50_000,
    federal_net_rental: 10_000,
  });
  const l = lines(result);
  assertEquals(l.il_net_income, -40_000);
  assertEquals(l.replacement_tax, 0);
});

Deno.test("il1120st: empty input yields zero", () => {
  const l = lines(il1120st.compute(ctx, {}));
  assertEquals(l.il_net_income, 0);
  assertEquals(l.replacement_tax, 0);
});
