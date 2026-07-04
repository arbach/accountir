/**
 * TY2023 Tax Constants — IRS Rev. Proc. 2022-38 (unless noted)
 *
 * All dollar amounts are in whole dollars unless noted.
 * All rates are decimals (e.g. 0.062 = 6.2%).
 *
 * Sections referenced below correspond to Rev. Proc. 2022-38 unless
 * another authority is cited explicitly.
 *
 * NOTE: TY2023 PRE-DATES the OBBBA (P.L. 119-21, enacted July 2025). The
 * OBBBA-only provisions present in the 2025 config — the senior deduction
 * (§70302), the $40,000 SALT cap with phase-out (§70002), the higher CTC/§179
 * amounts — did NOT exist in 2023. Those fields are set so they have no effect
 * (see the senior-deduction and SALT sections below).
 */

import { FilingStatus } from "../types.ts";
import type { Bracket } from "./2025.ts";

// ─── Tax Brackets ─────────────────────────────────────────────────────────────
// Rev. Proc. 2022-38, §3.01; IRC §1(a)–(d). base = cumulative tax at bracket floor.

/** IRC §1(a) — Married Filing Jointly / Qualifying Surviving Spouse */
export const BRACKETS_MFJ_2023: ReadonlyArray<Bracket> = [
  { over: 0,       upTo: 22_000,   rate: 0.10, base: 0 },
  { over: 22_000,  upTo: 89_450,   rate: 0.12, base: 2_200 },
  { over: 89_450,  upTo: 190_750,  rate: 0.22, base: 10_294 },
  { over: 190_750, upTo: 364_200,  rate: 0.24, base: 32_580 },
  { over: 364_200, upTo: 462_500,  rate: 0.32, base: 74_208 },
  { over: 462_500, upTo: 693_750,  rate: 0.35, base: 105_664 },
  { over: 693_750, upTo: Infinity, rate: 0.37, base: 186_601.50 },
] as const;

/** IRC §1(c) — Single */
export const BRACKETS_SINGLE_2023: ReadonlyArray<Bracket> = [
  { over: 0,       upTo: 11_000,   rate: 0.10, base: 0 },
  { over: 11_000,  upTo: 44_725,   rate: 0.12, base: 1_100 },
  { over: 44_725,  upTo: 95_375,   rate: 0.22, base: 5_147 },
  { over: 95_375,  upTo: 182_100,  rate: 0.24, base: 16_290 },
  { over: 182_100, upTo: 231_250,  rate: 0.32, base: 37_104 },
  { over: 231_250, upTo: 578_125,  rate: 0.35, base: 52_832 },
  { over: 578_125, upTo: Infinity, rate: 0.37, base: 174_238.25 },
] as const;

/** IRC §1(b) — Head of Household */
export const BRACKETS_HOH_2023: ReadonlyArray<Bracket> = [
  { over: 0,       upTo: 15_700,   rate: 0.10, base: 0 },
  { over: 15_700,  upTo: 59_850,   rate: 0.12, base: 1_570 },
  { over: 59_850,  upTo: 95_350,   rate: 0.22, base: 6_868 },
  { over: 95_350,  upTo: 182_100,  rate: 0.24, base: 14_678 },
  { over: 182_100, upTo: 231_250,  rate: 0.32, base: 35_498 },
  { over: 231_250, upTo: 578_100,  rate: 0.35, base: 51_226 },
  { over: 578_100, upTo: Infinity, rate: 0.37, base: 172_623.50 },
] as const;

/** IRC §1(d) — Married Filing Separately */
export const BRACKETS_MFS_2023: ReadonlyArray<Bracket> = [
  { over: 0,       upTo: 11_000,   rate: 0.10, base: 0 },
  { over: 11_000,  upTo: 44_725,   rate: 0.12, base: 1_100 },
  { over: 44_725,  upTo: 95_375,   rate: 0.22, base: 5_147 },
  { over: 95_375,  upTo: 182_100,  rate: 0.24, base: 16_290 },
  { over: 182_100, upTo: 231_250,  rate: 0.32, base: 37_104 },
  { over: 231_250, upTo: 346_875,  rate: 0.35, base: 52_832 },
  { over: 346_875, upTo: Infinity, rate: 0.37, base: 93_300.75 },
] as const;

// ─── Standard Deduction ───────────────────────────────────────────────────────
// Rev. Proc. 2022-38, §3.15; IRC §63(c)

/** Base standard deduction by filing status (TY2023). */
export const STANDARD_DEDUCTION_BASE_2023: Record<FilingStatus, number> = {
  [FilingStatus.Single]: 13_850,
  [FilingStatus.MFJ]:    27_700,
  [FilingStatus.MFS]:    13_850,
  [FilingStatus.HOH]:    20_800,
  [FilingStatus.QSS]:    27_700,
} as const;

/**
 * Additional standard deduction per age/blindness factor (TY2023).
 * Single/HOH: $1,850 per factor; MFJ/MFS/QSS: $1,500 per factor.
 * IRC §63(f); Rev. Proc. 2022-38, §3.15
 */
export const STANDARD_DEDUCTION_ADDITIONAL_2023: Record<FilingStatus, number> = {
  [FilingStatus.Single]: 1_850,
  [FilingStatus.MFJ]:    1_500,
  [FilingStatus.MFS]:    1_500,
  [FilingStatus.HOH]:    1_850,
  [FilingStatus.QSS]:    1_500,
} as const;

// ─── Senior Deduction (OBBBA §70302) ─────────────────────────────────────────
// The OBBBA senior deduction did NOT exist for TY2023 (enacted July 2025,
// effective TY2025). All values are zero so the deduction has no effect.

/** Senior Deduction maximum per qualifying person — none in TY2023. */
export const SENIOR_DEDUCTION_MAX_2023 = 0;

/** Senior Deduction phase-out start — Single/MFS/HOH — N/A in TY2023. */
export const SENIOR_DEDUCTION_PHASEOUT_SINGLE_2023 = 0;

/** Senior Deduction phase-out start — MFJ/QSS — N/A in TY2023. */
export const SENIOR_DEDUCTION_PHASEOUT_MFJ_2023 = 0;

/** Senior Deduction phase-out rate — N/A in TY2023. */
export const SENIOR_DEDUCTION_PHASEOUT_RATE_2023 = 0;

// ─── QDCGT / Capital Gains Rate Thresholds ────────────────────────────────────
// Rev. Proc. 2022-38, §3.03; IRC §1(h)

/** Top of 0% LTCG/QD bracket (income at or below this → 0% rate). */
export const QDCGT_ZERO_CEILING_2023: Record<FilingStatus, number> = {
  [FilingStatus.Single]: 44_625,
  [FilingStatus.MFJ]:    89_250,
  [FilingStatus.MFS]:    44_625,
  [FilingStatus.HOH]:    59_750,
  [FilingStatus.QSS]:    89_250,
} as const;

/** Bottom of 20% LTCG/QD bracket (income above this → 20% rate). */
export const QDCGT_TWENTY_FLOOR_2023: Record<FilingStatus, number> = {
  [FilingStatus.Single]: 492_300,
  [FilingStatus.MFJ]:    553_850,
  [FilingStatus.MFS]:    276_900,
  [FilingStatus.HOH]:    523_050,
  [FilingStatus.QSS]:    553_850,
} as const;

// ─── AMT — Form 6251 ──────────────────────────────────────────────────────────
// Rev. Proc. 2022-38, §3.11; IRC §55(d)

/** AMT exemption amounts by filing status (TY2023). */
export const AMT_EXEMPTION_2023: Record<FilingStatus, number> = {
  [FilingStatus.Single]: 81_300,
  [FilingStatus.HOH]:    81_300,
  [FilingStatus.MFJ]:    126_500,
  [FilingStatus.QSS]:    126_500,
  [FilingStatus.MFS]:    63_250,
} as const;

/** AMT phase-out start thresholds by filing status (TY2023). */
export const AMT_PHASE_OUT_START_2023: Record<FilingStatus, number> = {
  [FilingStatus.Single]: 578_150,
  [FilingStatus.HOH]:    578_150,
  [FilingStatus.MFJ]:    1_156_300,
  [FilingStatus.QSS]:    1_156_300,
  [FilingStatus.MFS]:    578_150,
} as const;

/** AMT 26%/28% bracket threshold — standard (non-MFS) filers (TY2023). */
export const AMT_BRACKET_26_THRESHOLD_STANDARD_2023 = 220_700;

/** AMT 26%/28% bracket threshold — MFS filers (= standard / 2). */
export const AMT_BRACKET_26_THRESHOLD_MFS_2023 = 110_350;

/** Pre-computed 28%-bracket savings adjustment — standard (= 220,700 × 0.02). */
export const AMT_BRACKET_ADJUSTMENT_STANDARD_2023 = 4_414;

/** Pre-computed 28%-bracket savings adjustment — MFS (= 110,350 × 0.02). */
export const AMT_BRACKET_ADJUSTMENT_MFS_2023 = 2_207;

// ─── FICA / Social Security ───────────────────────────────────────────────────
// SSA 2023 fact sheet; IRC §3121(a)(1)

/** Social Security wage base (TY2023). */
export const SS_WAGE_BASE_2023 = 160_200;

/** Maximum SS tax per employer (= SS_WAGE_BASE_2023 × 0.062). */
export const SS_MAX_TAX_PER_EMPLOYER_2023 = 9_932.40;

// ─── Additional Medicare Tax (Form 8959) ─────────────────────────────────────
// IRC §3101(b)(2); not indexed for inflation

/** Additional Medicare Tax threshold — MFJ/QSS. */
export const ADDITIONAL_MEDICARE_THRESHOLD_MFJ_2023 = 250_000;

/** Additional Medicare Tax threshold — MFS. */
export const ADDITIONAL_MEDICARE_THRESHOLD_MFS_2023 = 125_000;

/** Additional Medicare Tax threshold — Single, HOH, QSS. */
export const ADDITIONAL_MEDICARE_THRESHOLD_OTHER_2023 = 200_000;

// ─── Net Investment Income Tax (Form 8960) ────────────────────────────────────
// IRC §1411; not indexed for inflation

/** NIIT MAGI threshold — MFJ/QSS. */
export const NIIT_THRESHOLD_MFJ_2023 = 250_000;

/** NIIT MAGI threshold — MFS. */
export const NIIT_THRESHOLD_MFS_2023 = 125_000;

/** NIIT MAGI threshold — Single, HOH. */
export const NIIT_THRESHOLD_OTHER_2023 = 200_000;

// ─── HSA Contribution Limits (Form 8889) ─────────────────────────────────────
// Rev. Proc. 2022-24; IRC §223(b)(2)–(3)

/** HSA self-only HDHP contribution limit (TY2023). */
export const HSA_SELF_ONLY_LIMIT_2023 = 3_850;

/** HSA family HDHP contribution limit (TY2023). */
export const HSA_FAMILY_LIMIT_2023 = 7_750;

/** HSA catch-up contribution for age 55+ (statutory; not indexed). */
export const HSA_CATCHUP_2023 = 1_000;

// ─── IRA Contribution Limits ──────────────────────────────────────────────────
// Notice 2022-55; IRC §219(b)(5)(A)

/** Traditional/Roth IRA contribution limit under age 50 (TY2023). */
export const IRA_CONTRIBUTION_LIMIT_2023 = 6_500;

/** Traditional/Roth IRA contribution limit age 50+ (TY2023). */
export const IRA_CONTRIBUTION_LIMIT_AGE50_2023 = 7_500;

/** IRA deduction phase-out — Single/HOH/QSS active participant, lower bound. */
export const IRA_PHASEOUT_SINGLE_LOWER_2023 = 73_000;

/** IRA deduction phase-out — Single/HOH/QSS active participant, upper bound. */
export const IRA_PHASEOUT_SINGLE_UPPER_2023 = 83_000;

/** IRA deduction phase-out — MFJ covered taxpayer, lower bound. */
export const IRA_PHASEOUT_MFJ_LOWER_2023 = 116_000;

/** IRA deduction phase-out — MFJ covered taxpayer, upper bound. */
export const IRA_PHASEOUT_MFJ_UPPER_2023 = 136_000;

/** IRA deduction phase-out — MFJ non-covered spouse (covered spouse), lower bound. */
export const IRA_PHASEOUT_NONCOVERED_MFJ_LOWER_2023 = 218_000;

/** IRA deduction phase-out — MFJ non-covered spouse (covered spouse), upper bound. */
export const IRA_PHASEOUT_NONCOVERED_MFJ_UPPER_2023 = 228_000;

/** IRA deduction phase-out — MFS active participant, lower bound. */
export const IRA_PHASEOUT_MFS_LOWER_2023 = 0;

/** IRA deduction phase-out — MFS active participant, upper bound. */
export const IRA_PHASEOUT_MFS_UPPER_2023 = 10_000;

// ─── QBI Deduction Thresholds (Form 8995A) ────────────────────────────────────
// Rev. Proc. 2022-38, §3.27; IRC §199A(b)(3)(B)(ii)

/** QBI wage limitation phase-in threshold — Single/MFS/HOH/QSS (TY2023). */
export const QBI_THRESHOLD_SINGLE_2023 = 182_100;

/** QBI wage limitation phase-in threshold — MFJ (TY2023). */
export const QBI_THRESHOLD_MFJ_2023 = 364_200;

/** QBI phase-in range width (single field; MFJ range is $100k, others $50k). */
export const QBI_PHASE_IN_RANGE_2023 = 100_000;

// ─── EITC (Earned Income Tax Credit) ─────────────────────────────────────────
// Rev. Proc. 2022-38, §3.06; IRC §32

/** EITC maximum credit amounts by number of qualifying children (0–3). */
export const EITC_MAX_CREDIT_2023: Record<number, number> = {
  0: 600,
  1: 3_995,
  2: 6_604,
  3: 7_430,
} as const;

/** EITC earned income at which phase-in ends (credit reaches maximum). */
export const EITC_PHASE_IN_END_2023: Record<number, number> = {
  0: 7_840,
  1: 11_750,
  2: 16_510,
  3: 16_510,
} as const;

/**
 * EITC phase-out start by children count: [single/hoh/mfs threshold, mfj/qss threshold].
 * TY2023 (Rev. Proc. 2022-38, §3.06; IRC §32(b)(2)).
 */
export const EITC_PHASEOUT_START_2023: Record<number, [number, number]> = {
  0: [9_800, 16_370],
  1: [21_560, 28_120],
  2: [21_560, 28_120],
  3: [21_560, 28_120],
} as const;

/** EITC income limit (disqualifying income): [single/hoh/mfs limit, mfj/qss limit]. */
export const EITC_INCOME_LIMIT_2023: Record<number, [number, number]> = {
  0: [17_640, 24_210],
  1: [46_560, 53_120],
  2: [52_918, 59_478],
  3: [56_838, 63_398],
} as const;

/** EITC investment income limit — disqualifies any EITC when exceeded. */
export const EITC_INVESTMENT_INCOME_LIMIT_2023 = 11_000;

// ─── Child Tax Credit / ACTC (Form 8812) ─────────────────────────────────────
// IRC §24 (pre-OBBBA, TCJA amounts); Rev. Proc. 2022-38, §3.05 (ACTC refundable cap)

/** Child Tax Credit per qualifying child (TY2023). */
export const CTC_PER_CHILD_2023 = 2_000;

/** Other Dependent Credit per non-child dependent (TY2023). */
export const ODC_PER_DEPENDENT_2023 = 500;

/** Additional Child Tax Credit maximum per child (TY2023; Rev. Proc. 2022-38 §3.05). */
export const ACTC_MAX_PER_CHILD_2023 = 1_600;

/** CTC phase-out threshold — MFJ (TY2023). */
export const CTC_PHASE_OUT_THRESHOLD_MFJ_2023 = 400_000;

/** CTC phase-out threshold — all other filing statuses (TY2023). */
export const CTC_PHASE_OUT_THRESHOLD_OTHER_2023 = 200_000;

/** ACTC earned income floor (minimum earned income for ACTC). */
export const ACTC_EARNED_INCOME_FLOOR_2023 = 2_500;

// ─── Saver's Credit (Form 8880) ───────────────────────────────────────────────
// Notice 2022-55; IRC §25B. Values are the AGI ceiling for each credit-rate tier.

/** Maximum contribution eligible for Saver's Credit per person. */
export const SAVERS_CREDIT_CONTRIBUTION_CAP_2023 = 2_000;

/** Saver's Credit AGI thresholds — Single/MFS/QSS: [50% rate, 20% rate, 10% rate]. */
export const SAVERS_CREDIT_AGI_SINGLE_2023 = { rate50: 21_750, rate20: 23_750, rate10: 36_500 } as const;

/** Saver's Credit AGI thresholds — HOH. */
export const SAVERS_CREDIT_AGI_HOH_2023 = { rate50: 32_625, rate20: 35_625, rate10: 54_750 } as const;

/** Saver's Credit AGI thresholds — MFJ. */
export const SAVERS_CREDIT_AGI_MFJ_2023 = { rate50: 43_500, rate20: 47_500, rate10: 73_000 } as const;

// ─── EE/I Bond Interest Exclusion (Form 8815) ────────────────────────────────
// Rev. Proc. 2022-38, §3.18; IRC §135(b)(2)(A)

/** Form 8815 phase-out start — MFJ/QSS. */
export const SAVINGS_BOND_PHASEOUT_START_MFJ_2023 = 137_800;

/** Form 8815 phase-out end — MFJ/QSS. */
export const SAVINGS_BOND_PHASEOUT_END_MFJ_2023 = 167_800;

/** Form 8815 phase-out start — Single/HOH. */
export const SAVINGS_BOND_PHASEOUT_START_SINGLE_2023 = 91_850;

/** Form 8815 phase-out end — Single/HOH. */
export const SAVINGS_BOND_PHASEOUT_END_SINGLE_2023 = 106_850;

// ─── Kiddie Tax (Form 8615) ───────────────────────────────────────────────────
// Rev. Proc. 2022-38, §3.02 / §3.15; IRC §1(g)

/** Net unearned income threshold for kiddie tax (TY2023). */
export const KIDDIE_TAX_UNEARNED_INCOME_THRESHOLD_2023 = 2_500;

/** Standard deduction floor for computing net unearned income (TY2023). */
export const KIDDIE_TAX_STANDARD_DEDUCTION_FLOOR_2023 = 1_250;

// ─── FEIE (Form 2555) ─────────────────────────────────────────────────────────
// Rev. Proc. 2022-38, §3.39; IRC §911(b)(2)

/** Foreign Earned Income Exclusion limit (TY2023). */
export const FEIE_LIMIT_2023 = 120_000;

/** Foreign housing exclusion base (TY2023): 16% × FEIE limit = 16% × $120,000. */
export const FEIE_HOUSING_BASE_2023 = 19_200;

// ─── Section 179 / Depreciation (Form 4562) ──────────────────────────────────
// Rev. Proc. 2022-38, §3.25; Rev. Proc. 2023-14 (luxury auto); IRC §179

/** Section 179 expensing limit (TY2023). */
export const SECTION_179_LIMIT_2023 = 1_160_000;

/** Section 179 phase-out threshold (TY2023). */
export const SECTION_179_PHASEOUT_THRESHOLD_2023 = 2_890_000;

/** Luxury auto Year 1 limit — without bonus depreciation (TY2023; Rev. Proc. 2023-14). */
export const LUXURY_AUTO_YEAR1_NO_BONUS_2023 = 12_200;

/** Luxury auto Year 1 limit — with bonus depreciation (TY2023; Rev. Proc. 2023-14). */
export const LUXURY_AUTO_YEAR1_WITH_BONUS_2023 = 20_200;

/** Luxury auto Year 2 limit (TY2023). */
export const LUXURY_AUTO_YEAR2_2023 = 19_500;

/** Luxury auto Year 3+ limit (TY2023). */
export const LUXURY_AUTO_YEAR3_PLUS_2023 = 11_700;

// ─── Schedule H — Household Employment Taxes ─────────────────────────────────
// IRC §3510; IRS Publication 926 (2023)

/** Household employment FICA threshold: wages that trigger FICA (TY2023). */
export const HOUSEHOLD_FICA_THRESHOLD_2023 = 2_600;

/** FUTA filing threshold — quarterly wages (statutory; not indexed). */
export const HOUSEHOLD_FUTA_QUARTERLY_THRESHOLD_2023 = 1_000;

// ─── Schedule A — Itemized Deductions / SALT ─────────────────────────────────
// IRC §164(b)(6) (TCJA cap). The OBBBA $40,000 cap and 30% phase-out did NOT
// exist in TY2023 — the flat $10,000 ($5,000 MFS) cap applies with no phase-out.

/** SALT (state and local tax) cap — Single/MFJ/HOH/QSS (TY2023, TCJA). */
export const SALT_CAP_2023 = 10_000;

/** SALT phase-out threshold — none in TY2023 (sentinel: never reached). */
export const SALT_PHASEOUT_THRESHOLD_2023 = Infinity;

/** SALT phase-out threshold (MFS) — none in TY2023 (sentinel: never reached). */
export const SALT_PHASEOUT_THRESHOLD_MFS_2023 = Infinity;

/** SALT phase-out rate — none in TY2023. */
export const SALT_PHASEOUT_RATE_2023 = 0;

/** SALT floor — equals the cap in TY2023 (no phase-down). */
export const SALT_FLOOR_2023 = 10_000;

/** SALT floor (MFS) — equals the MFS cap in TY2023. */
export const SALT_FLOOR_MFS_2023 = 5_000;

// ─── Dependent Care (Form 2441) ───────────────────────────────────────────────
// IRC §21; not indexed for inflation

/** Qualifying expense cap — one qualifying person. */
export const DEP_CARE_EXPENSE_CAP_ONE_2023 = 3_000;

/** Qualifying expense cap — two or more qualifying persons. */
export const DEP_CARE_EXPENSE_CAP_TWO_PLUS_2023 = 6_000;

/** Employer-provided dependent care exclusion limit — MFJ/single/HOH/QSS. */
export const DEP_CARE_EMPLOYER_EXCLUSION_2023 = 5_000;

/** Employer-provided dependent care exclusion limit — MFS. */
export const DEP_CARE_EMPLOYER_EXCLUSION_MFS_2023 = 2_500;

/** Credit rate phase-down starting AGI. */
export const DEP_CARE_CREDIT_RATE_AGI_THRESHOLD_2023 = 15_000;

/** Credit rate phase-down bracket size ($2,000 per 1% step). */
export const DEP_CARE_CREDIT_RATE_BRACKET_SIZE_2023 = 2_000;

// ─── ACA / Form 8962 ─────────────────────────────────────────────────────────
// IRC §36B; TY2023 PTC uses the 2022 HHS Federal Poverty Level (48 states).

/** Federal Poverty Level base amount for 2022 (used for TY2023 PTC). */
export const FPL_BASE_2023 = 13_590;

/** Federal Poverty Level per-person increment for 2022. */
export const FPL_INCREMENT_2023 = 4_720;

// ─── IRA Distributions (1099-R) ──────────────────────────────────────────────
// IRC §408(d)(8); IRC §72(t)(10)

/** QCD (Qualified Charitable Distribution) annual limit (TY2023; indexing began 2024). */
export const QCD_ANNUAL_LIMIT_2023 = 100_000;

/** Public Safety Officer health insurance exclusion limit. */
export const PSO_EXCLUSION_LIMIT_2023 = 3_000;

// ─── Schedule C / Schedule F — Excess Business Loss ──────────────────────────
// IRC §461(l); Rev. Proc. 2022-38, §3.32

/** Excess business loss threshold — Single/MFS/HOH/QSS (TY2023). */
export const EBL_THRESHOLD_SINGLE_2023 = 289_000;

/** Excess business loss threshold — MFJ (TY2023). */
export const EBL_THRESHOLD_MFJ_2023 = 578_000;

// ─── Form 8990 — Interest Expense Limitation ──────────────────────────────────
// IRC §163(j)(3); Rev. Proc. 2022-38, §3.31

/** Small business gross receipts threshold exempting from §163(j) (TY2023). */
export const SMALL_BIZ_GROSS_RECEIPTS_2023 = 29_000_000;

// ─── W-2 — Retirement Plan Contribution Limits ───────────────────────────────
// Notice 2022-55; IRC §402(g), §408(p)
// NOTE: the SECURE 2.0 ages 60–63 "super catch-up" first applies in TY2025, so
// in TY2023 the 63 bracket equals the 59 (age-50 catch-up) bracket.

export const RETIREMENT_LIMITS_2023: Record<string, Record<number, number>> = {
  "401k": { 49: 22_500, 59: 30_000, 63: 30_000, [Infinity]: 30_000 },
  "403b": { 49: 22_500, 59: 30_000, 63: 30_000, [Infinity]: 30_000 },
  "457b": { 49: 22_500, 59: 30_000, 63: 30_000, [Infinity]: 30_000 },
  "simple": { 49: 15_500, 59: 19_000, 63: 19_000, [Infinity]: 19_000 },
} as const;

// ─── Form 8853 — Archer MSA / LTC ────────────────────────────────────────────
// Rev. Proc. 2022-38, §3.61; IRC §7702B(d)

/** LTC per-diem daily limit for non-tax-qualified contracts (TY2023). */
export const LTC_PER_DIEM_DAILY_LIMIT_2023 = 420;

// ─── Form 4972 — Lump Sum Distributions ──────────────────────────────────────
// IRC §402(e)(1); Schedule G (Form 4972) — statutory, not indexed.

/** Minimum Distribution Allowance — maximum amount (step 1 of MDA formula). */
export const MDA_MAX_2023 = 10_000;

/** MDA phase-out start threshold. */
export const MDA_PHASE_OUT_THRESHOLD_2023 = 20_000;

/** MDA zeroes out when ordinary income reaches this amount. */
export const MDA_ZERO_THRESHOLD_2023 = 70_000;

/** Death benefit exclusion maximum (legacy §101(b) carryover; same as 2025 config). */
export const DEATH_BENEFIT_MAX_2023 = 5_000;

// ─── Form 982 — Qualified Principal Residence Indebtedness ───────────────────
// IRC §108(a)(1)(E)

/** QPRI exclusion cap — standard filers (TY2023). */
export const QPRI_CAP_STANDARD_2023 = 750_000;

/** QPRI exclusion cap — MFS filers (TY2023). */
export const QPRI_CAP_MFS_2023 = 375_000;

// ─── Form 1099-DIV / Form 1099-INT — Routing Thresholds ──────────────────────
// IRC §6012(a), Pub 550

/** Schedule B reporting threshold for ordinary dividends (unchanged). */
export const SCHEDULE_B_DIVIDEND_THRESHOLD_2023 = 1_500;

/** §199A dividend threshold (routes to 8995A) — Single/MFS/HOH/QSS (TY2023). */
export const SEC199A_SINGLE_THRESHOLD_2023 = 182_100;

/** §199A dividend threshold (routes to 8995A) — MFJ (TY2023). */
export const SEC199A_MFJ_THRESHOLD_2023 = 364_200;

// ─── Form 8396 — Mortgage Interest Credit ─────────────────────────────────────
// IRC §25(a)(1); not indexed

/** Maximum annual mortgage interest credit when MCC rate exceeds 20% (TY2023). */
export const MCC_MAX_CREDIT_HIGH_RATE_2023 = 2_000;

// ─── SEP-IRA / SIMPLE / Solo 401(k) Contribution Limits ──────────────────────
// Notice 2022-55; IRC §404(a)(8), §408(k), §415(c), §408(p)

/** SEP contribution rate — 25% of net SE compensation. */
export const SEP_CONTRIBUTION_RATE_2023 = 0.25;

/** SEP / Solo 401(k) combined annual addition limit (TY2023; §415(c)). */
export const SEP_MAX_CONTRIBUTION_2023 = 66_000;

/** SIMPLE IRA employer matching rate — 3% of compensation (statutory). */
export const SIMPLE_EMPLOYER_MATCH_RATE_2023 = 0.03;

// ─── Student Loan Interest Deduction Phase-out (IRC §221(b)) ─────────────────
// Rev. Proc. 2022-38, §3.30

/** SLI phase-out start — Single/HOH/QSS (TY2023). */
export const SLI_PHASE_OUT_START_SINGLE_2023 = 75_000;

/** SLI phase-out end — Single/HOH/QSS (TY2023). */
export const SLI_PHASE_OUT_END_SINGLE_2023 = 90_000;

/** SLI phase-out start — MFJ (TY2023). */
export const SLI_PHASE_OUT_START_MFJ_2023 = 155_000;

/** SLI phase-out end — MFJ (TY2023). */
export const SLI_PHASE_OUT_END_MFJ_2023 = 185_000;

// ─── Form 2106 — Employee Business Expenses ───────────────────────────────────
// IRC §62(a)(2)(B)(ii); not indexed for inflation

/** Performing artist AGI limit — combined AGI must not exceed $16,000. */
export const F2106_PERFORMING_ARTIST_AGI_LIMIT_2023 = 16_000;

// ─── LTC Insurance Premium Deductibility Limits ───────────────────────────────
// Rev. Proc. 2022-38, §3.34; IRC §213(d)(10)

/**
 * Age-based deductible LTC insurance premium limits (TY2023).
 * `maxAge` is the inclusive upper bound; Infinity = age 71+.
 */
export const LTC_PREMIUM_LIMITS_2023: ReadonlyArray<{ readonly maxAge: number; readonly limit: number }> = [
  { maxAge: 40,       limit:   480 },
  { maxAge: 50,       limit:   890 },
  { maxAge: 60,       limit: 1_790 },
  { maxAge: 70,       limit: 4_770 },
  { maxAge: Infinity, limit: 5_960 },
] as const;

// ─── Aggregate export (used by CONFIG_BY_YEAR barrel) ─────────────────────────
import type { F1040Config } from "./types.ts";

export const config2023: F1040Config = {
  bracketsMfj:                  BRACKETS_MFJ_2023,
  bracketsSingle:               BRACKETS_SINGLE_2023,
  bracketsHoh:                  BRACKETS_HOH_2023,
  bracketsMfs:                  BRACKETS_MFS_2023,
  standardDeductionBase:        STANDARD_DEDUCTION_BASE_2023,
  standardDeductionAdditional:  STANDARD_DEDUCTION_ADDITIONAL_2023,
  seniorDeductionMax:           SENIOR_DEDUCTION_MAX_2023,
  seniorDeductionPhaseoutSingle: SENIOR_DEDUCTION_PHASEOUT_SINGLE_2023,
  seniorDeductionPhaseoutMfj:   SENIOR_DEDUCTION_PHASEOUT_MFJ_2023,
  seniorDeductionPhaseoutRate:  SENIOR_DEDUCTION_PHASEOUT_RATE_2023,
  qdcgtZeroCeiling:             QDCGT_ZERO_CEILING_2023,
  qdcgtTwentyFloor:             QDCGT_TWENTY_FLOOR_2023,
  amtExemption:                 AMT_EXEMPTION_2023,
  amtPhaseOutStart:             AMT_PHASE_OUT_START_2023,
  amtBracket26ThresholdStandard: AMT_BRACKET_26_THRESHOLD_STANDARD_2023,
  amtBracket26ThresholdMfs:     AMT_BRACKET_26_THRESHOLD_MFS_2023,
  amtBracketAdjustmentStandard: AMT_BRACKET_ADJUSTMENT_STANDARD_2023,
  amtBracketAdjustmentMfs:      AMT_BRACKET_ADJUSTMENT_MFS_2023,
  ssWageBase:                   SS_WAGE_BASE_2023,
  ssTaxPerEmployer:             SS_MAX_TAX_PER_EMPLOYER_2023,
  additionalMedicareThresholdMfj:   ADDITIONAL_MEDICARE_THRESHOLD_MFJ_2023,
  additionalMedicareThresholdMfs:   ADDITIONAL_MEDICARE_THRESHOLD_MFS_2023,
  additionalMedicareThresholdOther: ADDITIONAL_MEDICARE_THRESHOLD_OTHER_2023,
  niitThresholdMfj:             NIIT_THRESHOLD_MFJ_2023,
  niitThresholdMfs:             NIIT_THRESHOLD_MFS_2023,
  niitThresholdOther:           NIIT_THRESHOLD_OTHER_2023,
  hsaSelfOnlyLimit:             HSA_SELF_ONLY_LIMIT_2023,
  hsaFamilyLimit:               HSA_FAMILY_LIMIT_2023,
  hsaCatchup:                   HSA_CATCHUP_2023,
  iraContributionLimit:         IRA_CONTRIBUTION_LIMIT_2023,
  iraContributionLimitAge50:    IRA_CONTRIBUTION_LIMIT_AGE50_2023,
  iraPhaseoutSingleLower:       IRA_PHASEOUT_SINGLE_LOWER_2023,
  iraPhaseoutSingleUpper:       IRA_PHASEOUT_SINGLE_UPPER_2023,
  iraPhaseoutMfjLower:          IRA_PHASEOUT_MFJ_LOWER_2023,
  iraPhaseoutMfjUpper:          IRA_PHASEOUT_MFJ_UPPER_2023,
  iraPhaseoutNoncoveredMfjLower: IRA_PHASEOUT_NONCOVERED_MFJ_LOWER_2023,
  iraPhaseoutNoncoveredMfjUpper: IRA_PHASEOUT_NONCOVERED_MFJ_UPPER_2023,
  iraPhaseoutMfsLower:          IRA_PHASEOUT_MFS_LOWER_2023,
  iraPhaseoutMfsUpper:          IRA_PHASEOUT_MFS_UPPER_2023,
  qbiThresholdSingle:           QBI_THRESHOLD_SINGLE_2023,
  qbiThresholdMfj:              QBI_THRESHOLD_MFJ_2023,
  qbiPhaseInRange:              QBI_PHASE_IN_RANGE_2023,
  eitcMaxCredit:                EITC_MAX_CREDIT_2023,
  eitcPhaseInEnd:               EITC_PHASE_IN_END_2023,
  eitcPhaseoutStart:            EITC_PHASEOUT_START_2023,
  eitcIncomeLimit:              EITC_INCOME_LIMIT_2023,
  eitcInvestmentIncomeLimit:    EITC_INVESTMENT_INCOME_LIMIT_2023,
  ctcPerChild:                  CTC_PER_CHILD_2023,
  odcPerDependent:              ODC_PER_DEPENDENT_2023,
  actcMaxPerChild:              ACTC_MAX_PER_CHILD_2023,
  ctcPhaseOutThresholdMfj:      CTC_PHASE_OUT_THRESHOLD_MFJ_2023,
  ctcPhaseOutThresholdOther:    CTC_PHASE_OUT_THRESHOLD_OTHER_2023,
  actcEarnedIncomeFloor:        ACTC_EARNED_INCOME_FLOOR_2023,
  saversCreditContributionCap:  SAVERS_CREDIT_CONTRIBUTION_CAP_2023,
  saversCreditAgiSingle:        SAVERS_CREDIT_AGI_SINGLE_2023,
  saversCreditAgiHoh:           SAVERS_CREDIT_AGI_HOH_2023,
  saversCreditAgiMfj:           SAVERS_CREDIT_AGI_MFJ_2023,
  savingsBondPhaseoutStartMfj:  SAVINGS_BOND_PHASEOUT_START_MFJ_2023,
  savingsBondPhaseoutEndMfj:    SAVINGS_BOND_PHASEOUT_END_MFJ_2023,
  savingsBondPhaseoutStartSingle: SAVINGS_BOND_PHASEOUT_START_SINGLE_2023,
  savingsBondPhaseoutEndSingle: SAVINGS_BOND_PHASEOUT_END_SINGLE_2023,
  kiddieUnearnedIncomeThreshold: KIDDIE_TAX_UNEARNED_INCOME_THRESHOLD_2023,
  kiddieStandardDeductionFloor: KIDDIE_TAX_STANDARD_DEDUCTION_FLOOR_2023,
  feieLimit:                    FEIE_LIMIT_2023,
  feieHousingBase:              FEIE_HOUSING_BASE_2023,
  section179Limit:              SECTION_179_LIMIT_2023,
  section179PhaseoutThreshold:  SECTION_179_PHASEOUT_THRESHOLD_2023,
  luxuryAutoYear1NoBonus:       LUXURY_AUTO_YEAR1_NO_BONUS_2023,
  luxuryAutoYear1WithBonus:     LUXURY_AUTO_YEAR1_WITH_BONUS_2023,
  luxuryAutoYear2:              LUXURY_AUTO_YEAR2_2023,
  luxuryAutoYear3Plus:          LUXURY_AUTO_YEAR3_PLUS_2023,
  householdFicaThreshold:       HOUSEHOLD_FICA_THRESHOLD_2023,
  householdFutaQuarterlyThreshold: HOUSEHOLD_FUTA_QUARTERLY_THRESHOLD_2023,
  saltCap:                      SALT_CAP_2023,
  saltPhaseoutThreshold:        SALT_PHASEOUT_THRESHOLD_2023,
  saltPhaseoutThresholdMfs:     SALT_PHASEOUT_THRESHOLD_MFS_2023,
  saltPhaseoutRate:             SALT_PHASEOUT_RATE_2023,
  saltFloor:                    SALT_FLOOR_2023,
  saltFloorMfs:                 SALT_FLOOR_MFS_2023,
  depCareExpenseCapOne:         DEP_CARE_EXPENSE_CAP_ONE_2023,
  depCareExpenseCapTwoPlus:     DEP_CARE_EXPENSE_CAP_TWO_PLUS_2023,
  depCareEmployerExclusion:     DEP_CARE_EMPLOYER_EXCLUSION_2023,
  depCareEmployerExclusionMfs:  DEP_CARE_EMPLOYER_EXCLUSION_MFS_2023,
  depCareCreditRateAgiThreshold: DEP_CARE_CREDIT_RATE_AGI_THRESHOLD_2023,
  depCareCreditRateBracketSize: DEP_CARE_CREDIT_RATE_BRACKET_SIZE_2023,
  fplBase:                      FPL_BASE_2023,
  fplIncrement:                 FPL_INCREMENT_2023,
  qcdAnnualLimit:               QCD_ANNUAL_LIMIT_2023,
  psoExclusionLimit:            PSO_EXCLUSION_LIMIT_2023,
  eblThresholdSingle:           EBL_THRESHOLD_SINGLE_2023,
  eblThresholdMfj:              EBL_THRESHOLD_MFJ_2023,
  smallBizGrossReceipts:        SMALL_BIZ_GROSS_RECEIPTS_2023,
  retirementLimits:             RETIREMENT_LIMITS_2023,
  ltcPerDiemDailyLimit:         LTC_PER_DIEM_DAILY_LIMIT_2023,
  mdaMax:                       MDA_MAX_2023,
  mdaPhaseOutThreshold:         MDA_PHASE_OUT_THRESHOLD_2023,
  mdaZeroThreshold:             MDA_ZERO_THRESHOLD_2023,
  deathBenefitMax:              DEATH_BENEFIT_MAX_2023,
  qpriCapStandard:              QPRI_CAP_STANDARD_2023,
  qpriCapMfs:                   QPRI_CAP_MFS_2023,
  scheduleBDividendThreshold:   SCHEDULE_B_DIVIDEND_THRESHOLD_2023,
  sec199aSingleThreshold:       SEC199A_SINGLE_THRESHOLD_2023,
  sec199aMfjThreshold:          SEC199A_MFJ_THRESHOLD_2023,
  sliPhaseOutStartSingle:       SLI_PHASE_OUT_START_SINGLE_2023,
  sliPhaseOutEndSingle:         SLI_PHASE_OUT_END_SINGLE_2023,
  sliPhaseOutStartMfj:          SLI_PHASE_OUT_START_MFJ_2023,
  sliPhaseOutEndMfj:            SLI_PHASE_OUT_END_MFJ_2023,
  f2106PerformingArtistAgiLimit: F2106_PERFORMING_ARTIST_AGI_LIMIT_2023,
  ltcPremiumLimits:             LTC_PREMIUM_LIMITS_2023,
  mccMaxCreditHighRate:         MCC_MAX_CREDIT_HIGH_RATE_2023,
  sepContributionRate:          SEP_CONTRIBUTION_RATE_2023,
  sepMaxContribution:           SEP_MAX_CONTRIBUTION_2023,
  simpleEmployerMatchRate:      SIMPLE_EMPLOYER_MATCH_RATE_2023,
};
