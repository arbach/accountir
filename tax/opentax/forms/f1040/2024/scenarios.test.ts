/**
 * E2E scenario — TY2024 MFJ wage-only return.
 *
 * Hand-verified against the TY2024 constants in
 * forms/f1040/nodes/config/2024.ts (Rev. Proc. 2023-34).
 *
 *   Wages: $100,000  |  Std ded (MFJ 2024): $29,200  |  Taxable: $70,800
 *   2024 MFJ brackets: 10% to $23,200 (base $0), 12% $23,200–$94,300 (base $2,320).
 *   Tax (12% band): $2,320 + ($70,800 − $23,200) × 0.12 = $2,320 + $5,712 = $8,032
 *   No withholding → amount owed $8,032.
 *
 * Inputs use SINGULAR node-type keys (the start node routes them to each
 * node's array input).
 */

import { assertEquals } from "@std/assert";
import { buildExecutionPlan } from "../../../core/runtime/planner.ts";
import { execute, type ExecuteResult } from "../../../core/runtime/executor.ts";
import { registry } from "../2025/registry.ts";
import { FilingStatus } from "../nodes/types.ts";
import { CONFIG_BY_YEAR } from "../nodes/config/index.ts";

const ctx = { taxYear: 2024, formType: "f1040" };
const plan = buildExecutionPlan(registry);

function runReturn(inputs: Record<string, unknown>): ExecuteResult {
  return execute(plan, registry, inputs, ctx);
}

function mfjGeneral() {
  return {
    filing_status: FilingStatus.MFJ,
    taxpayer_first_name: "Test",
    taxpayer_last_name: "Taxpayer",
    taxpayer_ssn: "111-22-3333",
    taxpayer_dob: "1985-06-15",
    spouse_first_name: "Spouse",
    spouse_last_name: "Taxpayer",
    spouse_ssn: "444-55-6666",
    spouse_dob: "1987-03-10",
  };
}

function w2Item(wages: number, withheld: number) {
  const ssWages = Math.min(wages, CONFIG_BY_YEAR[2024].ssWageBase);
  return {
    box1_wages: wages,
    box2_fed_withheld: withheld,
    box3_ss_wages: ssWages,
    box4_ss_withheld: ssWages * 0.062,
    box5_medicare_wages: wages,
    box6_medicare_withheld: wages * 0.0145,
    employer_ein: "12-3456789",
    employer_name: "ACME Corp",
    box12_entries: [],
  };
}

Deno.test("TY2024 Scenario: MFJ, W-2 $100K — taxable $70,800, owes $8,032", () => {
  const result = runReturn({
    general: mfjGeneral(),
    w2: [w2Item(100_000, 0)],
  });

  assertEquals(
    result.pending["income_tax_calculation"]?.["taxable_income"], 70_800,
    "taxable income = $100K − $29,200 MFJ std ded (2024)",
  );

  const f = result.pending["f1040"] ?? {};
  assertEquals(f["line24_total_tax"], 8_032, "total tax via 2024 MFJ brackets");
  assertEquals(f["line37_amount_owed"], 8_032, "amount owed (no withholding)");
  assertEquals(f["line35a_refund"], undefined, "no refund");
});
