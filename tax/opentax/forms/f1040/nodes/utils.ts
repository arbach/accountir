export function normalizeArray<T>(v: T | T[] | undefined): T[] {
  if (v === undefined) return [];
  return Array.isArray(v) ? v : [v];
}

/**
 * Collapse array-valued numeric fields to their sum.
 *
 * The executor's accumulation pattern promotes multiple upstream nodes depositing
 * the same scalar key into an array (e.g. an S-corp K-1 and a partnership K-1 both
 * depositing `line5_schedule_e`). Aggregator/sink schemas that declare those fields
 * as `z.number()` would otherwise fail Zod validation on the array. Use this as a
 * `z.preprocess` so any all-numeric array contribution is summed before parsing.
 * Scalars, strings, booleans, and mixed arrays pass through untouched.
 */
export function sumNumericArrayFields(raw: unknown): unknown {
  if (raw === null || typeof raw !== "object" || Array.isArray(raw)) return raw;
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
    out[k] = Array.isArray(v) && v.length > 0 && v.every((x) => typeof x === "number")
      ? (v as number[]).reduce((a, b) => a + b, 0)
      : v;
  }
  return out;
}
