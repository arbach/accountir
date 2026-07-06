// fill_il.ts — fill an Illinois return's key computed lines. IL PDFs use
// descriptive AcroForm field names ("Step 7: Line 51 - Enter your net income"),
// so fields are addressed by name. Usage:
//   deno run -A scripts/fill_il.ts <form> <template.pdf> <out.pdf> <netIncome> <tax>
// form: il1120st | il1120 | il1040
import { PDFDocument, StandardFonts } from "pdf-lib";

const [form, tmpl, outPath, netIncome, tax] = Deno.args;
const doc = await PDFDocument.load(await Deno.readFile(tmpl), { ignoreEncryption: true });
const f = doc.getForm();
// deno-lint-ignore no-explicit-any
const set = (name: string, t: string) => { try { (f as any).getTextField(name).setText(t); } catch { console.error("miss:", name); } };

const NET: Record<string, string> = {
  il1120st: "Step 7: Line 51 - Enter your net income",
  il1120: "Step 6: Line 50 - Enter your net income",
  il1040: "Net income. Subtract Line 12 from Line 11",
};
const TAX: Record<string, string> = {
  il1120st: "Step 8: Line 56 - Net replacement tax. Subtract Line 55 from Line 54",
  il1120: "Step 8: Line 61 - Net income and replacement tax",
  il1040: "Income tax. Multiply Line 13 by 4.95%",
};

if (+netIncome && NET[form]) set(NET[form], Math.round(+netIncome).toString());
if (TAX[form]) set(TAX[form], Math.round(+tax).toString());

const font = await doc.embedFont(StandardFonts.Helvetica);
f.updateFieldAppearances(font);
f.flatten();
await Deno.writeFile(outPath, await doc.save());
console.log(`filled ${form}: net ${netIncome}, tax ${tax} → ${outPath}`);
