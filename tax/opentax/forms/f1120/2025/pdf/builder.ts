import { PDFDocument, StandardFonts } from "pdf-lib";
import { join } from "@std/path";

// IRS Form 1120 (2025), page 1. AcroForm field names verified empirically by
// filling each field with a marker and reading `pdftotext -layout`. Engine domain
// keys are mapped to fields by position.
const PDF_URL = "https://www.irs.gov/pub/irs-pdf/f1120.pdf";
const P1 = (n: string) => `topmostSubform[0].Page1[0].${n}[0]`;
const NAME = (n: string) => `topmostSubform[0].Page1[0].NameFieldsReadOrder[0].${n}[0]`;

const LINE_FIELDS: Record<string, string> = {
  line1a_gross_receipts: P1("f1_14"),
  line1c_net_receipts: P1("f1_16"),
  line2_cogs: P1("f1_17"),
  line3_gross_profit: P1("f1_18"),
  line4_dividends: P1("f1_19"),
  line5_interest: P1("f1_20"),
  line6_gross_rents: P1("f1_21"),
  line8_capital_gain: P1("f1_23"),
  line10_other_income: P1("f1_25"),
  line11_total_income: P1("f1_26"),
  line12_officer_compensation: P1("f1_27"),
  line13_salaries_wages: P1("f1_28"),
  line17_taxes_licenses: P1("f1_32"),
  line19_charitable: P1("f1_34"),
  line20_depreciation: P1("f1_35"),
  line26_other_deductions: P1("f1_41"),
  line27_total_deductions: P1("f1_42"),
  line28_income_before_nol: P1("f1_43"),
  line29a_nol_deduction: P1("f1_44"),
  line30_taxable_income: P1("f1_47"),
  line31_total_tax: P1("f1_48"),
};

function scalar(v: unknown): number | undefined {
  if (Array.isArray(v)) {
    const nums = v.filter((x): x is number => typeof x === "number");
    return nums.length ? nums[nums.length - 1] : undefined;
  }
  return typeof v === "number" ? v : undefined;
}

async function fetchTemplate(cacheDir: string): Promise<Uint8Array> {
  const path = join(cacheDir, "f1120.pdf");
  try {
    return await Deno.readFile(path);
  } catch {
    const res = await fetch(PDF_URL);
    if (!res.ok) throw new Error(`fetch IRS f1120 failed (${res.status})`);
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
    // field not present — skip
  }
}

/** Fill IRS Form 1120 page 1 from the engine's computed lines (pending["f1120"]). */
export function buildPdfBytes(
  pending: Record<string, unknown>,
  filer: unknown,
  cacheDir = ".pdf-cache",
): Promise<Uint8Array> {
  return (async () => {
    const bytes = await fetchTemplate(cacheDir);
    const doc = await PDFDocument.load(bytes, { ignoreEncryption: true });
    const form = doc.getForm();

    const lines = (pending["f1120"] ?? {}) as Record<string, unknown>;
    for (const [key, field] of Object.entries(LINE_FIELDS)) {
      const v = scalar(lines[key]);
      if (v === undefined || v === 0) continue;
      setText(form, field, Math.round(v).toString());
    }

    // deno-lint-ignore no-explicit-any
    const f = (filer ?? {}) as any;
    const data = f?.data?.general ?? f?.data ?? f ?? {};
    if (data.corporation_name) setText(form, NAME("f1_4"), String(data.corporation_name));

    const font = await doc.embedFont(StandardFonts.Helvetica);
    form.updateFieldAppearances(font);
    form.flatten();
    return await doc.save();
  })();
}
