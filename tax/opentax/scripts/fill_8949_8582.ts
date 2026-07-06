// Fill Form 8949 (Part II, box F — LT not reported on 1099-B) for the LLC-interest
// sale, and Form 8582 (line 2 — all other passive activities) for the LP rentals.
import { PDFDocument, StandardFonts } from "pdf-lib";

const SP = "/tmp/claude-1000/-tmp-taxdev/eccafa54-6d08-4f47-8dfb-fcad26322625/scratchpad";
const num = (n: number) => Math.round(n).toLocaleString("en-US");

// ---- Form 8949 ----
{
  const doc = await PDFDocument.load(await Deno.readFile(`${SP}/f8949.pdf`), { ignoreEncryption: true });
  const form = doc.getForm();
  // deno-lint-ignore no-explicit-any
  const set = (f: string, t: string) => { try { (form as any).getTextField(f).setText(t); } catch { console.error("8949 miss", f); } };
  // Box F (long-term, not reported on 1099-B)
  try { form.getRadioGroup("topmostSubform[0].Page2[0].c2_1[0]").select("3"); }
  catch { try { (form as any).getCheckBox("topmostSubform[0].Page2[0].c2_1[2]").check(); } catch { console.error("box F miss"); } }
  const R = "topmostSubform[0].Page2[0].Table_Line1_Part2[0].Row1[0]";
  set(`${R}.f2_03[0]`, "Sale of LLC interest");
  set(`${R}.f2_04[0]`, "Various");
  set(`${R}.f2_05[0]`, "12/31/2025");
  set(`${R}.f2_06[0]`, num(220000));   // (d) proceeds
  set(`${R}.f2_07[0]`, num(0));        // (e) cost basis (from sale agreement — see note)
  set(`${R}.f2_10[0]`, num(220000));   // (h) gain
  const font = await doc.embedFont(StandardFonts.Helvetica);
  form.updateFieldAppearances(font); form.flatten();
  await Deno.writeFile(`${SP}/f8949_filled.pdf`, await doc.save());
  console.log("filled 8949: LLC sale, LT gain 220,000 (box F)");
}

// ---- Form 8582 ----
{
  const doc = await PDFDocument.load(await Deno.readFile(`${SP}/f8582.pdf`), { ignoreEncryption: true });
  const form = doc.getForm();
  // deno-lint-ignore no-explicit-any
  const set = (f: string, t: string) => { try { (form as any).getTextField(f).setText(t); } catch { console.error("8582 miss", f); } };
  const P = "topmostSubform[0].Page1[0]";
  set(`${P}.f1_07[0]`, num(15413));       // 2a net income (Petersburg)
  set(`${P}.f1_08[0]`, `(${num(19107)})`); // 2b net loss (ML Sidecar)
  set(`${P}.f1_09[0]`, num(0));           // 2c prior unallowed
  set(`${P}.f1_10[0]`, `(${num(3694)})`);  // 2d combine
  const font = await doc.embedFont(StandardFonts.Helvetica);
  form.updateFieldAppearances(font); form.flatten();
  await Deno.writeFile(`${SP}/f8582_filled.pdf`, await doc.save());
  console.log("filled 8582: line 2 all-other-passive, net (3,694) unallowed → suspended");
}
