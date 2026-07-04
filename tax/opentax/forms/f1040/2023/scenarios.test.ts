/**
 * E2E scenario — TY2023 MFJ wage-only return.
 *
 * Hand-verified against the TY2023 constants in
 * forms/f1040/nodes/config/2023.ts (Rev. Proc. 2022-38).
 *
 *   Wages: $100,000  |  Std ded (MFJ 2023): $27,700  |  Taxable: $72,300
 *   2023 MFJ brackets: 10% to $22,000 (base $0), 12% $22,000–$89,450 (base $2,200).
 *   Tax (12% band): $2,200 + ($72,300 − $22,000) × 0.12 = $2,200 + $6,036 = $8,236
 *   No withholding → amount owed $8,236.
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

const ctx = { taxYear: 2023, formType: "f1040" };
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
  const ssWages = Math.min(wages, CONFIG_BY_YEAR[2023].ssWageBase);
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

Deno.test("TY2023 Scenario: MFJ, W-2 $100K — taxable $72,300, owes $8,236", () => {
  const result = runReturn({
    general: mfjGeneral(),
    w2: [w2Item(100_000, 0)],
  });

  assertEquals(
    result.pending["income_tax_calculation"]?.["taxable_income"], 72_300,
    "taxable income = $100K − $27,700 MFJ std ded (2023)",
  );

  const f = result.pending["f1040"] ?? {};
  assertEquals(f["line24_total_tax"], 8_236, "total tax via 2023 MFJ brackets");
  assertEquals(f["line37_amount_owed"], 8_236, "amount owed (no withholding)");
  assertEquals(f["line35a_refund"], undefined, "no refund");
});
