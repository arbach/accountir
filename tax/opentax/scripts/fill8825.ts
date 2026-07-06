// fill8825.ts — fill IRS Form 8825 (rental real estate) from a bridge bundle's
// per-property f8825 items. The 8825 grid lives under
// Table_Lines2-17[0].Line{container}[0].f1_{base+col}[0]; cols A/B/C = base,+1,+2.
// (line container, base) verified from the AcroForm container names.
//
// Usage: deno run -A scripts/fill8825.ts <bundle.json> <out.pdf>
import { PDFDocument, StandardFonts } from "pdf-lib";

const PDF_URL = "https://www.irs.gov/pub/irs-pdf/f8825.pdf";
const cell = (container: string, base: number, col: number) =>
  `topmostSubform[0].Page1[0].Table_Lines2-17[0].Line${container}[0].f1_${base + col}[0]`;

// engine f8825 item field -> (8825 line container, col-A base field number)
const MAP: Record<string, [string, number]> = {
  gross_rents: ["2a", 23],
  expense_advertising: ["3", 35],
  expense_auto_travel: ["4", 39],
  expense_cleaning_maintenance: ["5", 43],
  expense_commissions: ["6", 47],
  expense_insurance: ["7", 51],
  expense_legal_professional: ["8", 55],
  expense_interest: ["9", 59],
  expense_repairs: ["10", 63],
  expense_taxes: ["11", 67],
  expense_utilities: ["12", 71],
  expense_wages_salaries: ["13", 75],
  expense_depreciation: ["14", 80],
  expense_other: ["15", 83],
};
const TOTAL_EXP: [string, number] = ["16", 87]; // Line 16 Total expenses
const NET: [string, number] = ["17", 91]; // Line 17 Income or (loss)

const [bundlePath, outPath] = Deno.args;
const bundle = JSON.parse(await Deno.readTextFile(bundlePath));
const props = (bundle.forms as Array<{ node_type: string; data: Record<string, unknown> }>)
  .filter((f) => f.node_type === "f8825")
  .map((f) => f.data);

async function template(): Promise<Uint8Array> {
  const cache = "/tmp/.opentax-pdf-cache/f8825.pdf";
  try {
    return await Deno.readFile(cache);
  } catch {
    const res = await fetch(PDF_URL);
    const b = new Uint8Array(await res.arrayBuffer());
    await Deno.mkdir("/tmp/.opentax-pdf-cache", { recursive: true });
    await Deno.writeFile(cache, b);
    return b;
  }
}

const doc = await PDFDocument.load(await template(), { ignoreEncryption: true });
const form = doc.getForm();
// deno-lint-ignore no-explicit-any
const set = (f: string, t: string) => { try { (form as any).getTextField(f).setText(t); } catch (e) { console.error("miss", f, String(e).slice(0, 40)); } };

let totalRents = 0, totalExpAll = 0;
props.slice(0, 3).forEach((p, col) => {
  let expSum = 0;
  for (const [key, [container, base]] of Object.entries(MAP)) {
    const v = p[key] as number | undefined;
    if (typeof v !== "number" || v === 0) continue;
    set(cell(container, base, col), Math.round(v).toString());
    if (key === "gross_rents") totalRents += v;
    else { expSum += v; totalExpAll += v; }
  }
  const rent = (p.gross_rents as number) ?? 0;
  set(cell(TOTAL_EXP[0], TOTAL_EXP[1], col), Math.round(expSum).toString());
  set(cell(NET[0], NET[1], col), Math.round(rent - expSum).toString());
});

const font = await doc.embedFont(StandardFonts.Helvetica);
form.updateFieldAppearances(font);
form.flatten();
await Deno.writeFile(outPath, await doc.save());
console.log(`filled 8825: ${props.length} properties, gross rents ${Math.round(totalRents)}, net ${Math.round(totalRents - totalExpAll)} → ${outPath}`);
