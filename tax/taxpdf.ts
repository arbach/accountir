// Single PDF helper for tax forms.
//   fill <in.pdf> <spec.json> <out.pdf>   spec: {amounts:{f:n}, text:{f:s}, check:[f], select:[{field,state}]}
//   dump <in.pdf>                         -> "T\t<field>\t<value>" and "C\t<checked box>"
import { PDFDocument, PDFName, StandardFonts } from "npm:pdf-lib@1.17.1";

export function fmtAmount(n: number): string {
  if (typeof n !== "number" || Number.isNaN(n)) return String(n);
  const s = Math.round(Math.abs(n)).toLocaleString("en-US");
  return n < 0 ? `(${s})` : s;
}

const [cmd, inPdf, a2, a3] = Deno.args;
const doc = await PDFDocument.load(await Deno.readFile(inPdf));
const form = doc.getForm();

if (cmd === "dump") {
  for (const f of form.getFields()) {
    const t = f.constructor.name;
    if (t === "PDFTextField") { const v = (f as any).getText(); if (v && v.trim()) console.log(`T\t${f.getName()}\t${v}`); }
    else if (t === "PDFCheckBox") {
      const f2 = f as any; let on = false, state = "";
      try { const v = f2.acroField.dict.lookup(PDFName.of("V")); if (v) { state = v.toString(); on = state !== "/Off" && state !== ""; } } catch {}
      if (!on && f2.isChecked && f2.isChecked()) on = true;
      if (on) console.log(`C\t${f.getName()}\t${state.replace(/^\//, "")}`);
    }
  }
} else if (cmd === "fill") {
  const spec = JSON.parse(await Deno.readTextFile(a2));
  let ok = 0; const miss: string[] = [];
  for (const [n, v] of Object.entries(spec.amounts ?? {})) {
    try { const tf = form.getTextField(n);
      try { tf.setText(fmtAmount(v as number)); }
      catch { tf.setText(String(Math.round(v as number))); }   // maxLength combs: no commas
      ok++;
    } catch { miss.push("amt:" + n); }
  }
  for (const [n, v] of Object.entries(spec.text ?? {})) { try { form.getTextField(n).setText(String(v)); ok++; } catch { miss.push("txt:" + n); } }
  for (const n of spec.check ?? []) { try { form.getCheckBox(n).check(); ok++; } catch { miss.push("chk:" + n); } }
  // radio-as-checkbox: set a multi-widget field to one named on-state
  for (const sel of spec.select ?? []) {
    try {
      const f: any = form.getField(sel.field);
      const state = PDFName.of(sel.state);
      f.acroField.dict.set(PDFName.of("V"), state);
      for (const w of f.acroField.getWidgets()) {
        const apN: any = w.dict.lookup(PDFName.of("AP"))?.lookup(PDFName.of("N"));
        const has = apN && apN.has && apN.has(state);
        w.dict.set(PDFName.of("AS"), has ? state : PDFName.of("Off"));
      }
      ok++;
    } catch (e) { miss.push("sel:" + sel.field); }
  }
  await Deno.writeFile(a3, await doc.save());
  console.log(JSON.stringify({ filled: ok, missed: miss }));
} else if (cmd === "stamp") {
  const spec = JSON.parse(await Deno.readTextFile(a2));
  const font = await doc.embedFont(StandardFonts.Helvetica);
  const pages = doc.getPages();
  for (const st of spec.stamps ?? []) {            // image stamps (signatures)
    const png = await doc.embedPng(await Deno.readFile(st.image));
    const w = st.w ?? 120, h = w * (png.height / png.width);
    pages[st.page ?? 0].drawImage(png, { x: st.x, y: st.y, width: w, height: h });
  }
  for (const t of spec.texts ?? []) {              // text marks (dates, etc.)
    pages[t.page ?? 0].drawText(String(t.text), { x: t.x, y: t.y, size: t.size ?? 10, font });
  }
  await Deno.writeFile(a3, await doc.save());
  console.log(JSON.stringify({ pages: pages.length, ok: true }));
} else if (cmd === "list") {
  const out = form.getFields().map((f) => ({ name: f.getName(), type: f.constructor.name }));
  console.log(JSON.stringify({ fields: out, pages: doc.getPageCount() }));
} else { console.error("usage: taxpdf.ts fill|dump|list ..."); Deno.exit(2); }
