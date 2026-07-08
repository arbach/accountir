// Generate a draft formspec (field -> nearest line label) for an IRS/IL AcroForm PDF.
//   genspec.ts <blank.pdf> <bbox.html>   (bbox from: pdftotext -bbox blank.pdf bbox.html)
// Emits {generated_at:null, fields:[{field,type,page,x,y,label,line}]} — a human/AI confirms it.
import { PDFDocument } from "npm:pdf-lib@1.17.1";

const doc = await PDFDocument.load(await Deno.readFile(Deno.args[0]));
const bbox = await Deno.readTextFile(Deno.args[1]);

// page heights + words (label, page, x, y-from-bottom)
type W = { page: number; x: number; y: number; text: string };
const heights: number[] = []; const words: W[] = []; let pg = -1;
for (const ln of bbox.split("\n")) {
  const pm = ln.match(/<page width="[\d.]+" height="([\d.]+)"/);
  if (pm) { pg++; heights[pg] = parseFloat(pm[1]); continue; }
  const wm = ln.match(/<word xMin="([\d.]+)" yMin="[\d.]+" xMax="[\d.]+" yMax="([\d.]+)">(.*?)<\/word>/);
  if (wm && pg >= 0) words.push({ page: pg, x: +wm[1], y: heights[pg] - +wm[2], text: wm[3] });
}

// For a field at (page,x,y), the label = the run of words on the same row, left of the field.
function labelFor(page: number, x: number, y: number): { label: string; line: string } {
  const row = words.filter((w) => w.page === page && Math.abs(w.y - y) < 6 && w.x < x)
                   .sort((a, b) => a.x - b.x);
  const label = row.map((w) => w.text).join(" ").replace(/\s+/g, " ").trim().slice(0, 60);
  const m = label.match(/^(\d+[a-c]?)\b/);           // leading line number, if any
  return { label, line: m ? m[1] : "" };
}

const out: any[] = [];
for (const f of doc.getForm().getFields()) {
  const t = f.constructor.name;
  const w: any = (f as any).acroField.getWidgets()[0];
  if (!w) continue;
  const r = w.getRectangle();
  const page = doc.getPages().indexOf(doc.getPages().find((p) => p.ref === w.P()) ?? doc.getPages()[0]);
  const { label, line } = labelFor(Math.max(page, 0), r.x, r.y);
  out.push({ field: f.getName(), type: t, page: Math.max(page, 0), x: Math.round(r.x), y: Math.round(r.y), label, line });
}
console.log(JSON.stringify({ generated_at: null, fields: out }, null, 1));
