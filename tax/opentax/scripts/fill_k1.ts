// fill_k1.ts — fill Schedule K-1 (Form 1120-S) Part III income boxes from the
// engine's schedule_k1 output. Boxes 1–12 live under RightCol[0].Lines1-12[0],
// box N amount = f1_{20+N}. Verified by read-back.
//
// Usage: deno run -A scripts/fill_k1.ts <fill.json> <out.pdf> [corpName] [shareholderName]
import { PDFDocument, StandardFonts } from "pdf-lib";

const BOX = (n: number) => `topmostSubform[0].Page1[0].RightCol[0].Lines1-12[0].f1_${20 + n}[0]`;
const MAP: Record<string, number> = {
  box1_ordinary_business_income: 1,
  box2_net_rental_real_estate: 2,
  box4_interest_income: 4,
  box5a_ordinary_dividends: 5,
};

// Box 16 "Items affecting shareholder basis" grid (RightCol Lines13-17): row 1 =
// code field f1_80 + amount f1_81. Distributions are code D. Verified by position.
const BOX16_CODE = "topmostSubform[0].Page1[0].RightCol[0].Lines13-17[0].f1_80[0]";
const BOX16_AMT = "topmostSubform[0].Page1[0].RightCol[0].Lines13-17[0].f1_81[0]";

const [fillPath, outPath, distArg] = Deno.args;
const fill = JSON.parse(await Deno.readTextFile(fillPath));
// pull schedule_k1.* values from the fill lines
const k1: Record<string, number> = {};
for (const [k, v] of Object.entries(fill.lines ?? {})) {
  if (k.startsWith("schedule_k1.")) k1[k.slice("schedule_k1.".length)] = v as number;
}

const cache = "/tmp/.opentax-pdf-cache/f1120ssk.pdf";
let bytes: Uint8Array;
try {
  bytes = await Deno.readFile(cache);
} catch {
  const res = await fetch("https://www.irs.gov/pub/irs-pdf/f1120ssk.pdf");
  bytes = new Uint8Array(await res.arrayBuffer());
  await Deno.mkdir("/tmp/.opentax-pdf-cache", { recursive: true });
  await Deno.writeFile(cache, bytes);
}
const doc = await PDFDocument.load(bytes, { ignoreEncryption: true });
const form = doc.getForm();
// deno-lint-ignore no-explicit-any
const set = (f: string, t: string) => { try { (form as any).getTextField(f).setText(t); } catch {} };

let filled = 0;
for (const [key, box] of Object.entries(MAP)) {
  const v = k1[key];
  if (typeof v === "number" && v !== 0) {
    set(BOX(box), Math.round(v).toString());
    filled++;
  }
}

// Box 16d — Distributions (code D). Reduces stock basis; from equity draws
// (passed in, since it's off the P&L) or schedule_k1.box16d_distributions.
const dist = distArg ? Number(distArg) : (k1.box16d_distributions ?? 0);
if (dist && dist > 0) {
  set(BOX16_CODE, "D");
  set(BOX16_AMT, Math.round(dist).toString());
  filled++;
}

const font = await doc.embedFont(StandardFonts.Helvetica);
form.updateFieldAppearances(font);
form.flatten();
await Deno.writeFile(outPath, await doc.save());
console.log(`filled K-1: ${filled} income box(es) → ${outPath}`);
