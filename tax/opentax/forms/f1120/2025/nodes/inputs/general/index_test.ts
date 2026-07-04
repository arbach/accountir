import { assertEquals, assertThrows } from "@std/assert";
import { general, inputSchema } from "./index.ts";

const ctx = { taxYear: 2025, formType: "f1120" };

Deno.test("general: validates identity and does not re-echo (start already deposits it)", () => {
  const result = general.compute(ctx, {
    corporation_name: "MAVEN FINANCIAL TECHNOLOGIES INC",
    ein: "92-3379962",
    address: "123 Main St",
    city: "Kansas City",
    state: "MO",
    zip: "64101",
  });
  // No re-echo: avoids the executor promoting the duplicate scalar into an array.
  assertEquals(result.outputs, []);
});

Deno.test("general: requires a corporation name", () => {
  assertThrows(() => inputSchema.parse({ corporation_name: "" }));
});
