// fillpdf.ts — fill an IRS form PDF from the engine's computed lines.
//
// Reads a bridge fill JSON ({lines: {"<node>.<field>": value, ...}, filer?}) and
// calls the form's buildPdfBytes with a reconstructed pending dict. This is the
// reliable fill path (the CLI `return export` re-executes the pipeline; here we
// fill directly from the already-computed, reconciled numbers).
//
// Usage:
//   deno run --allow-read --allow-write --allow-net scripts/fillpdf.ts \
//     <formCode> <fill.json> <out.pdf> [filerName] [filerEin]
import { buildPdfBytes as f1120sPdf } from "../forms/f1120s/2025/pdf/builder.ts";
import { buildPdfBytes as f1120Pdf } from "../forms/f1120/2025/pdf/builder.ts";
import { buildPdfBytes as f1040Pdf } from "../forms/f1040/2025/pdf/builder.ts";

const builders: Record<string, (p: Record<string, unknown>, f: unknown, c?: string) => Promise<Uint8Array>> = {
  f1120s: f1120sPdf,
  f1120: f1120Pdf,
  f1040: f1040Pdf,
};

const [formCode, fillPath, outPath, filerName, filerEin] = Deno.args;
if (!formCode || !fillPath || !outPath) {
  console.error("usage: fillpdf.ts <formCode> <fill.json> <out.pdf> [filerName] [filerEin]");
  Deno.exit(2);
}
const builder = builders[formCode];
if (!builder) {
  console.error(`no PDF builder for ${formCode} (have: ${Object.keys(builders).join(", ")})`);
  Deno.exit(2);
}

const fill = JSON.parse(await Deno.readTextFile(fillPath));
// Reconstruct pending: {"<node>.<field>": value} -> {node: {field: value}}
const pending: Record<string, Record<string, number>> = {};
for (const [k, v] of Object.entries(fill.lines ?? {})) {
  const dot = k.indexOf(".");
  if (dot < 0) continue;
  const node = k.slice(0, dot);
  const field = k.slice(dot + 1);
  (pending[node] ??= {})[field] = v as number;
}

// The f1040 main-form descriptor reads pending["f1040"], but the engine spreads
// 1040 line values across calc nodes (agi_aggregator, income_tax_calculation,
// schedule1, standard_deduction, form8995, f8812, schedule3, …). Merge every
// node's line* values into f1040 so the main form fills; the per-schedule
// descriptors still read their own nodes.
if (formCode === "f1040") {
  const merged: Record<string, unknown> = {};
  for (const dict of Object.values(pending)) {
    for (const [k, v] of Object.entries(dict as Record<string, unknown>)) {
      if (k.startsWith("line") && !(k in merged)) merged[k] = v;
    }
  }
  pending["f1040"] = { ...merged, ...(pending["f1040"] ?? {}) };
}

const filer = fill.filer ??
  { data: { general: { corporation_name: filerName, ein: filerEin } } };

const cacheDir = Deno.env.get("PDF_CACHE") ?? "/tmp/.opentax-pdf-cache";
const bytes = await builder(pending, filer, cacheDir);
await Deno.writeFile(outPath, bytes);
console.log(`filled ${formCode} → ${outPath} (${bytes.length} bytes)`);
