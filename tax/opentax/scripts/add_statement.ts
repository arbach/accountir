// add_statement.ts — append an itemized supporting statement page to a filled
// form PDF (e.g. Form 1120-S line 20 "Other deductions", which the IRS requires
// be itemized on an attached statement). Usage:
//   deno run -A scripts/add_statement.ts <statement.json> <form.pdf> <out.pdf>
import { PDFDocument, StandardFonts, rgb } from "pdf-lib";

const [stmtPath, formPdf, outPath] = Deno.args;
const s = JSON.parse(await Deno.readTextFile(stmtPath));

const doc = await PDFDocument.load(await Deno.readFile(formPdf), { ignoreEncryption: true });
const font = await doc.embedFont(StandardFonts.Helvetica);
const bold = await doc.embedFont(StandardFonts.HelveticaBold);
const page = doc.addPage([612, 792]); // US Letter
let y = 740;
const L = 60, R = 552;
const line = (text: string, f = font, size = 10, x = L) => {
  page.drawText(text, { x, y, size, font: f, color: rgb(0, 0, 0) });
};
const right = (text: string, f = font, size = 10) => {
  const w = f.widthOfTextAtSize(text, size);
  page.drawText(text, { x: R - w, y, size, font: f });
};

line(s.title, bold, 13);
y -= 18;
line(`${s.entity}   EIN ${s.ein}   Tax year ${s.year}`, font, 10);
y -= 24;
line("Description", bold, 10);
right("Amount", bold, 10);
y -= 6;
page.drawLine({ start: { x: L, y }, end: { x: R, y }, thickness: 1, color: rgb(0, 0, 0) });
y -= 16;
for (const [name, amt] of s.items as [string, number][]) {
  line(String(name).slice(0, 70));
  right(Number(amt).toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 }));
  y -= 16;
  if (y < 70) { y = 740; doc.addPage([612, 792]); }
}
y -= 4;
page.drawLine({ start: { x: L, y }, end: { x: R, y }, thickness: 1, color: rgb(0, 0, 0) });
y -= 16;
line("Total", bold);
right(Number(s.total).toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 }), bold);

await Deno.writeFile(outPath, await doc.save());
console.log(`appended statement (${(s.items as unknown[]).length} items, total ${s.total}) → ${outPath}`);
