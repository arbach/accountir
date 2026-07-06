// fill_sche.ts — fill Schedule E Part II (Income/Loss from Partnerships & S-corps)
// from the taxpayer's K-1s, then output the filled Schedule E. Line 28 grid rows
// A–D: (a) name, (b) type P/S, (d) EIN; income/loss columns g/h/i/j/k.
import { PDFDocument, StandardFonts } from "pdf-lib";

const [tmpl, dataPath, outPath] = Deno.args;
const rows = JSON.parse(await Deno.readTextFile(dataPath)); // {rows:[{row,name,type,ein,col,amount}], totals:{field:val}, taxpayer, ssn}

const P = "topmostSubform[0].Page2[0]";
const NAMEF: Record<string, string> = { A: "f2_3", B: "f2_6", C: "f2_9", D: "f2_12" };
const TYPEF: Record<string, string> = { A: "f2_4", B: "f2_7", C: "f2_10", D: "f2_13" };
const EINF: Record<string, string> = { A: "f2_5", B: "f2_8", C: "f2_11", D: "f2_14" };
// g,h,i,j,k income/loss field numbers per row
const COLF: Record<string, Record<string, number>> = {
  A: { g: 15, h: 16, i: 17, j: 18, k: 19 },
  B: { g: 20, h: 21, i: 22, j: 23, k: 24 },
  C: { g: 25, h: 26, i: 27, j: 28, k: 29 },
  D: { g: 30, h: 31, i: 32, j: 33, k: 34 },
};

const doc = await PDFDocument.load(await Deno.readFile(tmpl), { ignoreEncryption: true });
const form = doc.getForm();
const set = (leaf: string, container: string, val: string) => {
  const name = `${P}.${container}[0].${leaf}[0]`;
  try { form.getTextField(name).setText(val); } catch { console.error("miss:", name); }
};
const nfmt = (n: number) => Math.round(Math.abs(n)).toLocaleString("en-US");

for (const r of rows.rows) {
  set(NAMEF[r.row], "Table_Line28a-f[0].Row" + r.row, r.name);
  set(TYPEF[r.row], "Table_Line28a-f[0].Row" + r.row, r.type);
  set(EINF[r.row], "Table_Line28a-f[0].Row" + r.row, r.ein);
  set("f2_" + COLF[r.row][r.col], "Table_Line28g-k[0].Row" + r.row, nfmt(r.amount));
}
// totals (line 29a/29b/30/31/32) — flat page-2 fields
for (const [leaf, val] of Object.entries(rows.totals as Record<string, number>)) {
  const name = `${P}.${leaf}[0]`;
  const s = val < 0 ? `(${nfmt(val)})` : nfmt(val);
  try { form.getTextField(name).setText(s); } catch { console.error("miss total:", name); }
}

const font = await doc.embedFont(StandardFonts.Helvetica);
form.updateFieldAppearances(font);
form.flatten();
await Deno.writeFile(outPath, await doc.save());
console.log(`filled Schedule E Part II (${rows.rows.length} K-1 activities) → ${outPath}`);
