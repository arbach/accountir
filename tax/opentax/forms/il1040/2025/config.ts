export const IL1040_2025_CONFIG = {
  formType: "il1040" as const,
  taxYear: 2025 as const,
  // IL returns are not yet wired to MeF; this is a placeholder for the stub
  // FormDefinition contract (buildMefXml throws).
  mefSchemaVersion: "2025v1.0" as const,
} as const;

// Per-year Illinois individual constants. Unlike the corporate replacement/income
// taxes (flat rates stable across years), the IL personal exemption allowance is
// adjusted annually for inflation, so the il1040 node defaults
// `exemption_per_person` from CONFIG_BY_YEAR[ctx.taxYear] (the f1040 pattern),
// which lets prior tax years reuse this same node graph via a taxYear override.
//
// Illinois individual income tax rate: 4.95% (flat, since 2017) — 35 ILCS 5/201(b)(5.4).
//
// Personal exemption allowance per person (taxpayer, spouse, and each dependent):
//   2023: $2,425  — IDOR Informational Bulletin FY 2024-02
//   2024: $2,775  — IDOR Informational Bulletin FY 2024-02
//   2025: $2,850  — IDOR "What's New for 2025?" (IL-1040 instructions)
// Sources:
//   https://tax.illinois.gov/research/publications/bulletins/fy-2024-02.html
//   https://tax.illinois.gov/forms/incometax/currentyear/individual/il-1040-instr/what-is-new.html
//
// NOTE: Illinois disallows the personal exemption above an AGI threshold
// (e.g. > $250,000 single / > $500,000 MFJ for recent years). The il1040 node is
// kept simple: the ledger bridge passes `exemption_count` (already reduced to 0
// when the cap applies), so the node multiplies count × per-person directly.

export const IL_INDIVIDUAL_TAX_RATE = 0.0495;

export interface Il1040YearConfig {
  readonly exemptionPerPerson: number;
}

export const CONFIG_BY_YEAR: Record<number, Il1040YearConfig> = {
  2023: { exemptionPerPerson: 2_425 },
  2024: { exemptionPerPerson: 2_775 },
  2025: { exemptionPerPerson: 2_850 },
};
