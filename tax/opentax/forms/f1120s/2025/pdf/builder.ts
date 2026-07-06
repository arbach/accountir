import { PDFDocument, StandardFonts } from "pdf-lib";
import { join } from "@std/path";

// IRS Form 1120-S (2025), page 1. AcroForm field names verified empirically by
// filling each field with a marker and reading `pdftotext -layout`. The 2025 form
// inserted line 19 (energy-efficient buildings) and renumbered Other deductions →
// 20, Total deductions → 21, Ordinary income → 22, so the engine's internal keys
// are mapped to fields BY POSITION, not by IRS line number.
const PDF_URL = "https://www.irs.gov/pub/irs-pdf/f1120s.pdf";
const P1 = (n: string) => `topmostSubform[0].Page1[0].${n}[0]`;
const HDR = (n: string) => `topmostSubform[0].Page1[0].Date_Name_ReadOrder[0].${n}[0]`;

const LINE_FIELDS: Record<string, string> = {
  line1a_gross_receipts: P1("f1_17"),
  line1b_returns_allowances: P1("f1_18"),
  line1c_net_receipts: P1("f1_19"),
  line2_cogs: P1("f1_20"),
  line3_gross_profit: P1("f1_21"),
  line4_net_gain_4797: P1("f1_22"),
  line5_other_income: P1("f1_23"),
  line6_total_income: P1("f1_24"),
  line7_officer_compensation: P1("f1_25"),
  line8_salaries_wages: P1("f1_26"),
  line9_repairs_maintenance: P1("f1_27"),
  line10_bad_debts: P1("f1_28"),
  line11_rents: P1("f1_29"),
  line12_taxes: P1("f1_30"),
  line13_interest: P1("f1_31"),
  line14_depreciation: P1("f1_32"),
  line16_advertising: P1("f1_34"),
  line17_pension_profit_sharing: P1("f1_35"),
  line18_employee_benefits: P1("f1_36"),
  line19_other_deductions: P1("f1_38"), // IRS line 20 "Other deductions"
  line20_total_deductions: P1("f1_39"), // IRS line 21 "Total deductions"
  line21_ordinary_business_income: P1("f1_40"), // IRS line 22 "Ordinary business income"
};

function scalar(v: unknown): number | undefined {
  if (Array.isArray(v)) {
    const nums = v.filter((x): x is number => typeof x === "number");
    return nums.length ? nums[nums.length - 1] : undefined;
  }
  return typeof v === "number" ? v : undefined;
}

async function fetchTemplate(cacheDir: string): Promise<Uint8Array> {
  const path = join(cacheDir, "f1120s.pdf");
  try {
    return await Deno.readFile(path);
  } catch {
    const res = await fetch(PDF_URL);
    if (!res.ok) throw new Error(`fetch IRS f1120s failed (${res.status})`);
    const bytes = new Uint8Array(await res.arrayBuffer());
    await Deno.mkdir(cacheDir, { recursive: true });
    await Deno.writeFile(path, bytes);
    return bytes;
  }
}

// deno-lint-ignore no-explicit-any
function setText(form: any, field: string, text: string): void {
  try {
    form.getTextField(field).setText(text);
  } catch {
    // field not present on this form version — skip silently
  }
}

/**
 * Fill IRS Form 1120-S page 1 from the engine's computed lines (pending["f1120s"])
 * plus the corporate filer identity. Returns a flattened PDF.
 */
export function buildPdfBytes(
  pending: Record<string, unknown>,
  filer: unknown,
  cacheDir = ".pdf-cache",
): Promise<Uint8Array> {
  return (async () => {
    const bytes = await fetchTemplate(cacheDir);
    const doc = await PDFDocument.load(bytes, { ignoreEncryption: true });
    const form = doc.getForm();

    const lines = (pending["f1120s"] ?? {}) as Record<string, unknown>;
    for (const [key, field] of Object.entries(LINE_FIELDS)) {
      const v = scalar(lines[key]);
      if (v === undefined || v === 0) continue; // IRS convention: leave zero fields blank
      setText(form, field, Math.round(v).toString());
    }

    // Distributions (owner draws — an equity item off the P&L, injected from the
    // ledger's distribution accounts): Schedule K line 16d + Schedule M-2 line 7,
    // column (a) AAA (reduces the accumulated adjustments account). Also on K-1 box 16d.
    const dist = scalar(lines["line16d_distributions"]);
    if (dist && dist !== 0) {
      const d = Math.round(dist).toString();
      setText(form, "topmostSubform[0].Page3[0].f3_46[0]", d);
      setText(form, "topmostSubform[0].Page5[0].Table_SchM-2[0].Line7[0].f5_43[0]", d);
    }

    // Header identity (best-effort — accepts the general node's data or a flat object).
    // deno-lint-ignore no-explicit-any
    const f = (filer ?? {}) as any;
    const data = f?.data?.general ?? f?.data ?? f ?? {};
    if (data.corporation_name) setText(form, HDR("f1_4"), String(data.corporation_name));
    if (data.ein) setText(form, P1("f1_13"), String(data.ein));

    const font = await doc.embedFont(StandardFonts.Helvetica);
    form.updateFieldAppearances(font);
    form.flatten();
    return await doc.save();
  })();
}
