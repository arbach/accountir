/**
 * TY2024 Tax Constants — IRS Rev. Proc. 2023-34 (unless noted)
 *
 * All dollar amounts are in whole dollars unless noted.
 * All rates are decimals (e.g. 0.062 = 6.2%).
 *
 * Sections referenced below correspond to Rev. Proc. 2023-34 unless
 * another authority is cited explicitly.
 *
 * NOTE: TY2024 PRE-DATES the OBBBA (P.L. 119-21, enacted July 2025). The
 * OBBBA-only provisions present in the 2025 config — the senior deduction
 * (§70302), the $40,000 SALT cap with phase-out (§70002), the higher CTC/§179
 * amounts — did NOT exist in 2024. Those fields are set so they have no effect
 * (see the senior-deduction and SALT sections below).
 */

import { FilingStatus } from "../types.ts";
import type { Bracket } from "./2025.ts";

// ─── Tax Brackets ─────────────────────────────────────────────────────────────
// Rev. Proc. 2023-34, §3.01; IRC §1(a)–(d). base = cumulative tax at bracket floor.

/** IRC §1(a) — Married Filing Jointly / Qualifying Surviving Spouse */
export const BRACKETS_MFJ_2024: ReadonlyArray<Bracket> = [
  { over: 0,       upTo: 23_200,   rate: 0.10, base: 0 },
  { over: 23_200,  upTo: 94_300,   rate: 0.12, base: 2_320 },
  { over: 94_300,  upTo: 201_050,  rate: 0.22, base: 10_852 },
  { over: 201_050, upTo: 383_900,  rate: 0.24, base: 34_337 },
  { over: 383_900, upTo: 487_450,  rate: 0.32, base: 78_221 },
  { over: 487_450, upTo: 731_200,  rate: 0.35, base: 111_357 },
  { over: 731_200, upTo: Infinity, rate: 0.37, base: 196_669.50 },
] as const;

/** IRC §1(c) — Single */
export const BRACKETS_SINGLE_2024: ReadonlyArray<Bracket> = [
  { over: 0,       upTo: 11_600,   rate: 0.10, base: 0 },
  { over: 11_600,  upTo: 47_150,   rate: 0.12, base: 1_160 },
  { over: 47_150,  upTo: 100_525,  rate: 0.22, base: 5_426 },
  { over: 100_525, upTo: 191_950,  rate: 0.24, base: 17_168.50 },
  { over: 191_950, upTo: 243_725,  rate: 0.32, base: 39_110.50 },
  { over: 243_725, upTo: 609_350,  rate: 0.35, base: 55_678.50 },
  { over: 609_350, upTo: Infinity, rate: 0.37, base: 183_647.25 },
] as const;

/** IRC §1(b) — Head of Household */
export const BRACKETS_HOH_2024: ReadonlyArray<Bracket> = [
  { over: 0,       upTo: 16_550,   rate: 0.10, base: 0 },
  { over: 16_550,  upTo: 63_100,   rate: 0.12, base: 1_655 },
  { over: 63_100,  upTo: 100_500,  rate: 0.22, base: 7_241 },
  { over: 100_500, upTo: 191_950,  rate: 0.24, base: 15_469 },
  { over: 191_950, upTo: 243_700,  rate: 0.32, base: 37_417 },
  { over: 243_700, upTo: 609_350,  rate: 0.35, base: 53_977 },
  { over: 609_350, upTo: Infinity, rate: 0.37, base: 181_954.50 },
] as const;

/** IRC §1(d) — Married Filing Separately */
export const BRACKETS_MFS_2024: ReadonlyArray<Bracket> = [
  { over: 0,       upTo: 11_600,   rate: 0.10, base: 0 },
  { over: 11_600,  upTo: 47_150,   rate: 0.12, base: 1_160 },
  { over: 47_150,  upTo: 100_525,  rate: 0.22, base: 5_426 },
  { over: 100_525, upTo: 191_950,  rate: 0.24, base: 17_168.50 },
  { over: 191_950, upTo: 243_725,  rate: 0.32, base: 39_110.50 },
  { over: 243_725, upTo: 365_600,  rate: 0.35, base: 55_678.50 },
  { over: 365_600, upTo: Infinity, rate: 0.37, base: 98_334.75 },
] as const;

// ─── Standard Deduction ───────────────────────────────────────────────────────
// Rev. Proc. 2023-34, §3.15; IRC §63(c)

/** Base standard deduction by filing status (TY2024). */
export const STANDARD_DEDUCTION_BASE_2024: Record<FilingStatus, number> = {
  [FilingStatus.Single]: 14_600,
  [FilingStatus.MFJ]:    29_200,
  [FilingStatus.MFS]:    14_600,
  [FilingStatus.HOH]:    21_900,
  [FilingStatus.QSS]:    29_200,
} as const;

/**
 * Additional standard deduction per age/blindness factor (TY2024).
 * Single/HOH: $1,950 per factor; MFJ/MFS/QSS: $1,550 per factor.
 * IRC §63(f); Rev. Proc. 2023-34, §3.15
 */
export const STANDARD_DEDUCTION_ADDITIONAL_2024: Record<FilingStatus, number> = {
  [FilingStatus.Single]: 1_950,
  [FilingStatus.MFJ]:    1_550,
  [FilingStatus.MFS]:    1_550,
  [FilingStatus.HOH]:    1_950,
  [FilingStatus.QSS]:    1_550,
} as const;

// ─── Senior Deduction (OBBBA §70302) ─────────────────────────────────────────
// The OBBBA senior deduction did NOT exist for TY2024 (enacted July 2025,
// effective TY2025). All values are zero so the deduction has no effect.

/** Senior Deduction maximum per qualifying person — none in TY2024. */
export const SENIOR_DEDUCTION_MAX_2024 = 0;

/** Senior Deduction phase-out start — Single/MFS/HOH — N/A in TY2024. */
export const SENIOR_DEDUCTION_PHASEOUT_SINGLE_2024 = 0;

/** Senior Deduction phase-out start — MFJ/QSS — N/A in TY2024. */
export const SENIOR_DEDUCTION_PHASEOUT_MFJ_2024 = 0;

/** Senior Deduction phase-out rate — N/A in TY2024. */
export const SENIOR_DEDUCTION_PHASEOUT_RATE_2024 = 0;

// ─── QDCGT / Capital Gains Rate Thresholds ────────────────────────────────────
// Rev. Proc. 2023-34, §3.03; IRC §1(h)

/** Top of 0% LTCG/QD bracket (income at or below this → 0% rate). */
export const QDCGT_ZERO_CEILING_2024: Record<FilingStatus, number> = {
  [FilingStatus.Single]: 47_025,
  [FilingStatus.MFJ]:    94_050,
  [FilingStatus.MFS]:    47_025,
  [FilingStatus.HOH]:    63_000,
  [FilingStatus.QSS]:    94_050,
} as const;

/** Bottom of 20% LTCG/QD bracket (income above this → 20% rate). */
export const QDCGT_TWENTY_FLOOR_2024: Record<FilingStatus, number> = {
  [FilingStatus.Single]: 518_900,
  [FilingStatus.MFJ]:    583_750,
  [FilingStatus.MFS]:    291_850,
  [FilingStatus.HOH]:    551_350,
  [FilingStatus.QSS]:    583_750,
} as const;

// ─── AMT — Form 6251 ──────────────────────────────────────────────────────────
// Rev. Proc. 2023-34, §3.11; IRC §55(d)

/** AMT exemption amounts by filing status (TY2024). */
export const AMT_EXEMPTION_2024: Record<FilingStatus, number> = {
  [FilingStatus.Single]: 85_700,
  [FilingStatus.HOH]:    85_700,
  [FilingStatus.MFJ]:    133_300,
  [FilingStatus.QSS]:    133_300,
  [FilingStatus.MFS]:    66_650,
} as const;

/** AMT phase-out start thresholds by filing status (TY2024). */
export const AMT_PHASE_OUT_START_2024: Record<FilingStatus, number> = {
  [FilingStatus.Single]: 609_350,
  [FilingStatus.HOH]:    609_350,
  [FilingStatus.MFJ]:    1_218_700,
  [FilingStatus.QSS]:    1_218_700,
  [FilingStatus.MFS]:    609_350,
} as const;

/** AMT 26%/28% bracket threshold — standard (non-MFS) filers (TY2024). */
export const AMT_BRACKET_26_THRESHOLD_STANDARD_2024 = 232_600;

/** AMT 26%/28% bracket threshold — MFS filers (= standard / 2). */
export const AMT_BRACKET_26_THRESHOLD_MFS_2024 = 116_300;

/** Pre-computed 28%-bracket savings adjustment — standard (= 232,600 × 0.02). */
export const AMT_BRACKET_ADJUSTMENT_STANDARD_2024 = 4_652;

/** Pre-computed 28%-bracket savings adjustment — MFS (= 116,300 × 0.02). */
export const AMT_BRACKET_ADJUSTMENT_MFS_2024 = 2_326;

// ─── FICA / Social Security ───────────────────────────────────────────────────
// SSA 2024 fact sheet; IRC §3121(a)(1)

/** Social Security wage base (TY2024). */
export const SS_WAGE_BASE_2024 = 168_600;

/** Maximum SS tax per employer (= SS_WAGE_BASE_2024 × 0.062). */
export const SS_MAX_TAX_PER_EMPLOYER_2024 = 10_453.20;

// ─── Additional Medicare Tax (Form 8959) ─────────────────────────────────────
// IRC §3101(b)(2); not indexed for inflation

/** Additional Medicare Tax threshold — MFJ/QSS. */
export const ADDITIONAL_MEDICARE_THRESHOLD_MFJ_2024 = 250_000;

/** Additional Medicare Tax threshold — MFS. */
export const ADDITIONAL_MEDICARE_THRESHOLD_MFS_2024 = 125_000;

/** Additional Medicare Tax threshold — Single, HOH, QSS. */
export const ADDITIONAL_MEDICARE_THRESHOLD_OTHER_2024 = 200_000;

// ─── Net Investment Income Tax (Form 8960) ────────────────────────────────────
// IRC §1411; not indexed for inflation

/** NIIT MAGI threshold — MFJ/QSS. */
export const NIIT_THRESHOLD_MFJ_2024 = 250_000;

/** NIIT MAGI threshold — MFS. */
export const NIIT_THRESHOLD_MFS_2024 = 125_000;

/** NIIT MAGI threshold — Single, HOH. */
export const NIIT_THRESHOLD_OTHER_2024 = 200_000;

// ─── HSA Contribution Limits (Form 8889) ─────────────────────────────────────
// Rev. Proc. 2023-23; IRC §223(b)(2)–(3)

/** HSA self-only HDHP contribution limit (TY2024). */
export const HSA_SELF_ONLY_LIMIT_2024 = 4_150;

/** HSA family HDHP contribution limit (TY2024). */
export const HSA_FAMILY_LIMIT_2024 = 8_300;

/** HSA catch-up contribution for age 55+ (statutory; not indexed). */
export const HSA_CATCHUP_2024 = 1_000;

// ─── IRA Contribution Limits ──────────────────────────────────────────────────
// Notice 2023-75; IRC §219(b)(5)(A)

/** Traditional/Roth IRA contribution limit under age 50 (TY2024). */
export const IRA_CONTRIBUTION_LIMIT_2024 = 7_000;

/** Traditional/Roth IRA contribution limit age 50+ (TY2024). */
export const IRA_CONTRIBUTION_LIMIT_AGE50_2024 = 8_000;

/** IRA deduction phase-out — Single/HOH/QSS active participant, lower bound. */
export const IRA_PHASEOUT_SINGLE_LOWER_2024 = 77_000;

/** IRA deduction phase-out — Single/HOH/QSS active participant, upper bound. */
export const IRA_PHASEOUT_SINGLE_UPPER_2024 = 87_000;

/** IRA deduction phase-out — MFJ covered taxpayer, lower bound. */
export const IRA_PHASEOUT_MFJ_LOWER_2024 = 123_000;

/** IRA deduction phase-out — MFJ covered taxpayer, upper bound. */
export const IRA_PHASEOUT_MFJ_UPPER_2024 = 143_000;

/** IRA deduction phase-out — MFJ non-covered spouse (covered spouse), lower bound. */
export const IRA_PHASEOUT_NONCOVERED_MFJ_LOWER_2024 = 230_000;

/** IRA deduction phase-out — MFJ non-covered spouse (covered spouse), upper bound. */
export const IRA_PHASEOUT_NONCOVERED_MFJ_UPPER_2024 = 240_000;

/** IRA deduction phase-out — MFS active participant, lower bound. */
export const IRA_PHASEOUT_MFS_LOWER_2024 = 0;

/** IRA deduction phase-out — MFS active participant, upper bound. */
export const IRA_PHASEOUT_MFS_UPPER_2024 = 10_000;

// ─── QBI Deduction Thresholds (Form 8995A) ────────────────────────────────────
// Rev. Proc. 2023-34, §3.27; IRC §199A(b)(3)(B)(ii)

/** QBI wage limitation phase-in threshold — Single/MFS/HOH/QSS (TY2024). */
export const QBI_THRESHOLD_SINGLE_2024 = 191_950;

/** QBI wage limitation phase-in threshold — MFJ (TY2024). */
export const QBI_THRESHOLD_MFJ_2024 = 383_900;

/** QBI phase-in range width (single field; MFJ range is $100k, others $50k). */
export const QBI_PHASE_IN_RANGE_2024 = 100_000;

// ─── EITC (Earned Income Tax Credit) ─────────────────────────────────────────
// Rev. Proc. 2023-34, §3.06; IRC §32

/** EITC maximum credit amounts by number of qualifying children (0–3). */
export const EITC_MAX_CREDIT_2024: Record<number, number> = {
  0: 632,
  1: 4_213,
  2: 6_960,
  3: 7_830,
} as const;

/** EITC earned income at which phase-in ends (credit reaches maximum). */
export const EITC_PHASE_IN_END_2024: Record<number, number> = {
  0: 8_260,
  1: 12_390,
  2: 17_400,
  3: 17_400,
} as const;

/**
 * EITC phase-out start by children count: [single/hoh/mfs threshold, mfj/qss threshold].
 * TY2024 (Rev. Proc. 2023-34, §3.06; IRC §32(b)(2)).
 */
export const EITC_PHASEOUT_START_2024: Record<number, [number, number]> = {
  0: [10_330, 17_250],
  1: [22_720, 29_640],
  2: [22_720, 29_640],
  3: [22_720, 29_640],
} as const;

/** EITC income limit (disqualifying income): [single/hoh/mfs limit, mfj/qss limit]. */
export const EITC_INCOME_LIMIT_2024: Record<number, [number, number]> = {
  0: [18_591, 25_511],
  1: [49_084, 56_004],
  2: [55_768, 62_688],
  3: [59_899, 66_819],
} as const;

/** EITC investment income limit — disqualifies any EITC when exceeded. */
export const EITC_INVESTMENT_INCOME_LIMIT_2024 = 11_600;

// ─── Child Tax Credit / ACTC (Form 8812) ─────────────────────────────────────
// IRC §24 (pre-OBBBA, TCJA amounts); Rev. Proc. 2023-34, §3.05 (ACTC refundable cap)

/** Child Tax Credit per qualifying child (TY2024). */
export const CTC_PER_CHILD_2024 = 2_000;

/** Other Dependent Credit per non-child dependent (TY2024). */
export const ODC_PER_DEPENDENT_2024 = 500;

/** Additional Child Tax Credit maximum per child (TY2024; Rev. Proc. 2023-34 §3.05). */
export const ACTC_MAX_PER_CHILD_2024 = 1_700;

/** CTC phase-out threshold — MFJ (TY2024). */
export const CTC_PHASE_OUT_THRESHOLD_MFJ_2024 = 400_000;

/** CTC phase-out threshold — all other filing statuses (TY2024). */
export const CTC_PHASE_OUT_THRESHOLD_OTHER_2024 = 200_000;

/** ACTC earned income floor (minimum earned income for ACTC). */
export const ACTC_EARNED_INCOME_FLOOR_2024 = 2_500;

// ─── Saver's Credit (Form 8880) ───────────────────────────────────────────────
// Rev. Proc. 2023-34, §3.16; IRC §25B

/** Maximum contribution eligible for Saver's Credit per person. */
export const SAVERS_CREDIT_CONTRIBUTION_CAP_2024 = 2_000;

/** Saver's Credit AGI thresholds — Single/MFS/QSS: [50% rate, 20% rate, 10% rate]. */
export const SAVERS_CREDIT_AGI_SINGLE_2024 = { rate50: 23_000, rate20: 25_000, rate10: 38_250 } as const;

/** Saver's Credit AGI thresholds — HOH. */
export const SAVERS_CREDIT_AGI_HOH_2024 = { rate50: 34_500, rate20: 37_500, rate10: 57_375 } as const;

/** Saver's Credit AGI thresholds — MFJ. */
export const SAVERS_CREDIT_AGI_MFJ_2024 = { rate50: 46_000, rate20: 50_000, rate10: 76_500 } as const;

// ─── EE/I Bond Interest Exclusion (Form 8815) ────────────────────────────────
// Rev. Proc. 2023-34, §3.18; IRC §135(b)(2)(A)

/** Form 8815 phase-out start — MFJ/QSS. */
export const SAVINGS_BOND_PHASEOUT_START_MFJ_2024 = 145_200;

/** Form 8815 phase-out end — MFJ/QSS. */
export const SAVINGS_BOND_PHASEOUT_END_MFJ_2024 = 175_200;

/** Form 8815 phase-out start — Single/HOH. */
export const SAVINGS_BOND_PHASEOUT_START_SINGLE_2024 = 96_800;

/** Form 8815 phase-out end — Single/HOH. */
export const SAVINGS_BOND_PHASEOUT_END_SINGLE_2024 = 111_800;

// ─── Kiddie Tax (Form 8615) ───────────────────────────────────────────────────
// Rev. Proc. 2023-34, §3.02; IRC §1(g)

/** Net unearned income threshold for kiddie tax (TY2024). */
export const KIDDIE_TAX_UNEARNED_INCOME_THRESHOLD_2024 = 2_600;

/** Standard deduction floor for computing net unearned income (TY2024). */
export const KIDDIE_TAX_STANDARD_DEDUCTION_FLOOR_2024 = 1_300;

// ─── FEIE (Form 2555) ─────────────────────────────────────────────────────────
// Rev. Proc. 2023-34, §3.39; IRC §911(b)(2)

/** Foreign Earned Income Exclusion limit (TY2024). */
export const FEIE_LIMIT_2024 = 126_500;

/** Foreign housing exclusion base (TY2024): 16% × FEIE limit = 16% × $126,500. */
export const FEIE_HOUSING_BASE_2024 = 20_240;

// ─── Section 179 / Depreciation (Form 4562) ──────────────────────────────────
// Rev. Proc. 2023-34, §3.25; Rev. Proc. 2024-13 (luxury auto); IRC §179

/** Section 179 expensing limit (TY2024). */
export const SECTION_179_LIMIT_2024 = 1_220_000;

/** Section 179 phase-out threshold (TY2024). */
export const SECTION_179_PHASEOUT_THRESHOLD_2024 = 3_050_000;

/** Luxury auto Year 1 limit — without bonus depreciation (TY2024; Rev. Proc. 2024-13). */
export const LUXURY_AUTO_YEAR1_NO_BONUS_2024 = 12_400;

/** Luxury auto Year 1 limit — with bonus depreciation (TY2024; Rev. Proc. 2024-13). */
export const LUXURY_AUTO_YEAR1_WITH_BONUS_2024 = 20_400;

/** Luxury auto Year 2 limit (TY2024). */
export const LUXURY_AUTO_YEAR2_2024 = 19_800;

/** Luxury auto Year 3+ limit (TY2024). */
export const LUXURY_AUTO_YEAR3_PLUS_2024 = 11_900;

// ─── Schedule H — Household Employment Taxes ─────────────────────────────────
// IRC §3510; IRS Publication 926 (2024)

/** Household employment FICA threshold: wages that trigger FICA (TY2024). */
export const HOUSEHOLD_FICA_THRESHOLD_2024 = 2_700;

/** FUTA filing threshold — quarterly wages (statutory; not indexed). */
export const HOUSEHOLD_FUTA_QUARTERLY_THRESHOLD_2024 = 1_000;

// ─── Schedule A — Itemized Deductions / SALT ─────────────────────────────────
// IRC §164(b)(6) (TCJA cap). The OBBBA $40,000 cap and 30% phase-out did NOT
// exist in TY2024 — the flat $10,000 ($5,000 MFS) cap applies with no phase-out.

/** SALT (state and local tax) cap — Single/MFJ/HOH/QSS (TY2024, TCJA). */
export const SALT_CAP_2024 = 10_000;

/** SALT phase-out threshold — none in TY2024 (sentinel: never reached). */
export const SALT_PHASEOUT_THRESHOLD_2024 = Infinity;

/** SALT phase-out threshold (MFS) — none in TY2024 (sentinel: never reached). */
export const SALT_PHASEOUT_THRESHOLD_MFS_2024 = Infinity;

/** SALT phase-out rate — none in TY2024. */
export const SALT_PHASEOUT_RATE_2024 = 0;

/** SALT floor — equals the cap in TY2024 (no phase-down). */
export const SALT_FLOOR_2024 = 10_000;

/** SALT floor (MFS) — equals the MFS cap in TY2024. */
export const SALT_FLOOR_MFS_2024 = 5_000;

// ─── Dependent Care (Form 2441) ───────────────────────────────────────────────
// IRC §21; not indexed for inflation

/** Qualifying expense cap — one qualifying person. */
export const DEP_CARE_EXPENSE_CAP_ONE_2024 = 3_000;

/** Qualifying expense cap — two or more qualifying persons. */
export const DEP_CARE_EXPENSE_CAP_TWO_PLUS_2024 = 6_000;

/** Employer-provided dependent care exclusion limit — MFJ/single/HOH/QSS. */
export const DEP_CARE_EMPLOYER_EXCLUSION_2024 = 5_000;

/** Employer-provided dependent care exclusion limit — MFS. */
export const DEP_CARE_EMPLOYER_EXCLUSION_MFS_2024 = 2_500;

/** Credit rate phase-down starting AGI. */
export const DEP_CARE_CREDIT_RATE_AGI_THRESHOLD_2024 = 15_000;

/** Credit rate phase-down bracket size ($2,000 per 1% step). */
export const DEP_CARE_CREDIT_RATE_BRACKET_SIZE_2024 = 2_000;

// ─── ACA / Form 8962 ─────────────────────────────────────────────────────────
// IRC §36B; TY2024 PTC uses the 2023 HHS Federal Poverty Level (48 states).

/** Federal Poverty Level base amount for 2023 (used for TY2024 PTC). */
export const FPL_BASE_2024 = 14_580;

/** Federal Poverty Level per-person increment for 2023. */
export const FPL_INCREMENT_2024 = 5_140;

// ─── IRA Distributions (1099-R) ──────────────────────────────────────────────
// IRC §408(d)(8); IRC §72(t)(10)

/** QCD (Qualified Charitable Distribution) annual limit (TY2024). */
export const QCD_ANNUAL_LIMIT_2024 = 105_000;

/** Public Safety Officer health insurance exclusion limit. */
export const PSO_EXCLUSION_LIMIT_2024 = 3_000;

// ─── Schedule C / Schedule F — Excess Business Loss ──────────────────────────
// IRC §461(l); Rev. Proc. 2023-34, §3.32

/** Excess business loss threshold — Single/MFS/HOH/QSS (TY2024). */
export const EBL_THRESHOLD_SINGLE_2024 = 305_000;

/** Excess business loss threshold — MFJ (TY2024). */
export const EBL_THRESHOLD_MFJ_2024 = 610_000;

// ─── Form 8990 — Interest Expense Limitation ──────────────────────────────────
// IRC §163(j)(3); Rev. Proc. 2023-34, §3.31

/** Small business gross receipts threshold exempting from §163(j) (TY2024). */
export const SMALL_BIZ_GROSS_RECEIPTS_2024 = 30_000_000;

// ─── W-2 — Retirement Plan Contribution Limits ───────────────────────────────
// Notice 2023-75; IRC §402(g), §408(p)
// NOTE: the SECURE 2.0 ages 60–63 "super catch-up" first applies in TY2025, so
// in TY2024 the 63 bracket equals the 59 (age-50 catch-up) bracket.

export const RETIREMENT_LIMITS_2024: Record<string, Record<number, number>> = {
  "401k": { 49: 23_000, 59: 30_500, 63: 30_500, [Infinity]: 30_500 },
  "403b": { 49: 23_000, 59: 30_500, 63: 30_500, [Infinity]: 30_500 },
  "457b": { 49: 23_000, 59: 30_500, 63: 30_500, [Infinity]: 30_500 },
  "simple": { 49: 16_000, 59: 19_500, 63: 19_500, [Infinity]: 19_500 },
} as const;

// ─── Form 8853 — Archer MSA / LTC ────────────────────────────────────────────
// Rev. Proc. 2023-34, §3.62; IRC §7702B(d)

/** LTC per-diem daily limit for non-tax-qualified contracts (TY2024). */
export const LTC_PER_DIEM_DAILY_LIMIT_2024 = 410;

// ─── Form 4972 — Lump Sum Distributions ──────────────────────────────────────
// IRC §402(e)(1); Schedule G (Form 4972) — statutory, not indexed.

/** Minimum Distribution Allowance — maximum amount (step 1 of MDA formula). */
export const MDA_MAX_2024 = 10_000;

/** MDA phase-out start threshold. */
export const MDA_PHASE_OUT_THRESHOLD_2024 = 20_000;

/** MDA zeroes out when ordinary income reaches this amount. */
export const MDA_ZERO_THRESHOLD_2024 = 70_000;

/** Death benefit exclusion maximum. */
export const DEATH_BENEFIT_MAX_2024 = 5_000;

// ─── Form 982 — Qualified Principal Residence Indebtedness ───────────────────
// IRC §108(a)(1)(E)

/** QPRI exclusion cap — standard filers (TY2024). */
export const QPRI_CAP_STANDARD_2024 = 750_000;

/** QPRI exclusion cap — MFS filers (TY2024). */
export const QPRI_CAP_MFS_2024 = 375_000;

// ─── Form 1099-DIV / Form 1099-INT — Routing Thresholds ──────────────────────
// IRC §6012(a), Pub 550

/** Schedule B reporting threshold for ordinary dividends (unchanged). */
export const SCHEDULE_B_DIVIDEND_THRESHOLD_2024 = 1_500;

/** §199A dividend threshold (routes to 8995A) — Single/MFS/HOH/QSS (TY2024). */
export const SEC199A_SINGLE_THRESHOLD_2024 = 191_950;

/** §199A dividend threshold (routes to 8995A) — MFJ (TY2024). */
export const SEC199A_MFJ_THRESHOLD_2024 = 383_900;

// ─── Form 8396 — Mortgage Interest Credit ─────────────────────────────────────
// IRC §25(a)(1); not indexed

/** Maximum annual mortgage interest credit when MCC rate exceeds 20% (TY2024). */
export const MCC_MAX_CREDIT_HIGH_RATE_2024 = 2_000;

// ─── SEP-IRA / SIMPLE / Solo 401(k) Contribution Limits ──────────────────────
// Notice 2023-75; IRC §404(a)(8), §408(k), §415(c), §408(p)

/** SEP contribution rate — 25% of net SE compensation. */
export const SEP_CONTRIBUTION_RATE_2024 = 0.25;

/** SEP / Solo 401(k) combined annual addition limit (TY2024; §415(c)). */
export const SEP_MAX_CONTRIBUTION_2024 = 69_000;

/** SIMPLE IRA employer matching rate — 3% of compensation (statutory). */
export const SIMPLE_EMPLOYER_MATCH_RATE_2024 = 0.03;

// ─── Student Loan Interest Deduction Phase-out (IRC §221(b)) ─────────────────
// Rev. Proc. 2023-34, §3.30

/** SLI phase-out start — Single/HOH/QSS (TY2024). */
export const SLI_PHASE_OUT_START_SINGLE_2024 = 80_000;

/** SLI phase-out end — Single/HOH/QSS (TY2024). */
export const SLI_PHASE_OUT_END_SINGLE_2024 = 95_000;

/** SLI phase-out start — MFJ (TY2024). */
export const SLI_PHASE_OUT_START_MFJ_2024 = 165_000;

/** SLI phase-out end — MFJ (TY2024). */
export const SLI_PHASE_OUT_END_MFJ_2024 = 195_000;

// ─── Form 2106 — Employee Business Expenses ───────────────────────────────────
// IRC §62(a)(2)(B)(ii); not indexed for inflation

/** Performing artist AGI limit — combined AGI must not exceed $16,000. */
export const F2106_PERFORMING_ARTIST_AGI_LIMIT_2024 = 16_000;

// ─── LTC Insurance Premium Deductibility Limits ───────────────────────────────
// Rev. Proc. 2023-34, §3.28; IRC §213(d)(10)

/**
 * Age-based deductible LTC insurance premium limits (TY2024).
 * `maxAge` is the inclusive upper bound; Infinity = age 71+.
 */
export const LTC_PREMIUM_LIMITS_2024: ReadonlyArray<{ readonly maxAge: number; readonly limit: number }> = [
  { maxAge: 40,       limit:   470 },
  { maxAge: 50,       limit:   880 },
  { maxAge: 60,       limit: 1_760 },
  { maxAge: 70,       limit: 4_710 },
  { maxAge: Infinity, limit: 5_880 },
] as const;

// ─── Aggregate export (used by CONFIG_BY_YEAR barrel) ─────────────────────────
import type { F1040Config } from "./types.ts";

export const config2024: F1040Config = {
  bracketsMfj:                  BRACKETS_MFJ_2024,
  bracketsSingle:               BRACKETS_SINGLE_2024,
  bracketsHoh:                  BRACKETS_HOH_2024,
  bracketsMfs:                  BRACKETS_MFS_2024,
  standardDeductionBase:        STANDARD_DEDUCTION_BASE_2024,
  standardDeductionAdditional:  STANDARD_DEDUCTION_ADDITIONAL_2024,
  seniorDeductionMax:           SENIOR_DEDUCTION_MAX_2024,
  seniorDeductionPhaseoutSingle: SENIOR_DEDUCTION_PHASEOUT_SINGLE_2024,
  seniorDeductionPhaseoutMfj:   SENIOR_DEDUCTION_PHASEOUT_MFJ_2024,
  seniorDeductionPhaseoutRate:  SENIOR_DEDUCTION_PHASEOUT_RATE_2024,
  qdcgtZeroCeiling:             QDCGT_ZERO_CEILING_2024,
  qdcgtTwentyFloor:             QDCGT_TWENTY_FLOOR_2024,
  amtExemption:                 AMT_EXEMPTION_2024,
  amtPhaseOutStart:             AMT_PHASE_OUT_START_2024,
  amtBracket26ThresholdStandard: AMT_BRACKET_26_THRESHOLD_STANDARD_2024,
  amtBracket26ThresholdMfs:     AMT_BRACKET_26_THRESHOLD_MFS_2024,
  amtBracketAdjustmentStandard: AMT_BRACKET_ADJUSTMENT_STANDARD_2024,
  amtBracketAdjustmentMfs:      AMT_BRACKET_ADJUSTMENT_MFS_2024,
  ssWageBase:                   SS_WAGE_BASE_2024,
  ssTaxPerEmployer:             SS_MAX_TAX_PER_EMPLOYER_2024,
  additionalMedicareThresholdMfj:   ADDITIONAL_MEDICARE_THRESHOLD_MFJ_2024,
  additionalMedicareThresholdMfs:   ADDITIONAL_MEDICARE_THRESHOLD_MFS_2024,
  additionalMedicareThresholdOther: ADDITIONAL_MEDICARE_THRESHOLD_OTHER_2024,
  niitThresholdMfj:             NIIT_THRESHOLD_MFJ_2024,
  niitThresholdMfs:             NIIT_THRESHOLD_MFS_2024,
  niitThresholdOther:           NIIT_THRESHOLD_OTHER_2024,
  hsaSelfOnlyLimit:             HSA_SELF_ONLY_LIMIT_2024,
  hsaFamilyLimit:               HSA_FAMILY_LIMIT_2024,
  hsaCatchup:                   HSA_CATCHUP_2024,
  iraContributionLimit:         IRA_CONTRIBUTION_LIMIT_2024,
  iraContributionLimitAge50:    IRA_CONTRIBUTION_LIMIT_AGE50_2024,
  iraPhaseoutSingleLower:       IRA_PHASEOUT_SINGLE_LOWER_2024,
  iraPhaseoutSingleUpper:       IRA_PHASEOUT_SINGLE_UPPER_2024,
  iraPhaseoutMfjLower:          IRA_PHASEOUT_MFJ_LOWER_2024,
  iraPhaseoutMfjUpper:          IRA_PHASEOUT_MFJ_UPPER_2024,
  iraPhaseoutNoncoveredMfjLower: IRA_PHASEOUT_NONCOVERED_MFJ_LOWER_2024,
  iraPhaseoutNoncoveredMfjUpper: IRA_PHASEOUT_NONCOVERED_MFJ_UPPER_2024,
  iraPhaseoutMfsLower:          IRA_PHASEOUT_MFS_LOWER_2024,
  iraPhaseoutMfsUpper:          IRA_PHASEOUT_MFS_UPPER_2024,
  qbiThresholdSingle:           QBI_THRESHOLD_SINGLE_2024,
  qbiThresholdMfj:              QBI_THRESHOLD_MFJ_2024,
  qbiPhaseInRange:              QBI_PHASE_IN_RANGE_2024,
  eitcMaxCredit:                EITC_MAX_CREDIT_2024,
  eitcPhaseInEnd:               EITC_PHASE_IN_END_2024,
  eitcPhaseoutStart:            EITC_PHASEOUT_START_2024,
  eitcIncomeLimit:              EITC_INCOME_LIMIT_2024,
  eitcInvestmentIncomeLimit:    EITC_INVESTMENT_INCOME_LIMIT_2024,
  ctcPerChild:                  CTC_PER_CHILD_2024,
  odcPerDependent:              ODC_PER_DEPENDENT_2024,
  actcMaxPerChild:              ACTC_MAX_PER_CHILD_2024,
  ctcPhaseOutThresholdMfj:      CTC_PHASE_OUT_THRESHOLD_MFJ_2024,
  ctcPhaseOutThresholdOther:    CTC_PHASE_OUT_THRESHOLD_OTHER_2024,
  actcEarnedIncomeFloor:        ACTC_EARNED_INCOME_FLOOR_2024,
  saversCreditContributionCap:  SAVERS_CREDIT_CONTRIBUTION_CAP_2024,
  saversCreditAgiSingle:        SAVERS_CREDIT_AGI_SINGLE_2024,
  saversCreditAgiHoh:           SAVERS_CREDIT_AGI_HOH_2024,
  saversCreditAgiMfj:           SAVERS_CREDIT_AGI_MFJ_2024,
  savingsBondPhaseoutStartMfj:  SAVINGS_BOND_PHASEOUT_START_MFJ_2024,
  savingsBondPhaseoutEndMfj:    SAVINGS_BOND_PHASEOUT_END_MFJ_2024,
  savingsBondPhaseoutStartSingle: SAVINGS_BOND_PHASEOUT_START_SINGLE_2024,
  savingsBondPhaseoutEndSingle: SAVINGS_BOND_PHASEOUT_END_SINGLE_2024,
  kiddieUnearnedIncomeThreshold: KIDDIE_TAX_UNEARNED_INCOME_THRESHOLD_2024,
  kiddieStandardDeductionFloor: KIDDIE_TAX_STANDARD_DEDUCTION_FLOOR_2024,
  feieLimit:                    FEIE_LIMIT_2024,
  feieHousingBase:              FEIE_HOUSING_BASE_2024,
  section179Limit:              SECTION_179_LIMIT_2024,
  section179PhaseoutThreshold:  SECTION_179_PHASEOUT_THRESHOLD_2024,
  luxuryAutoYear1NoBonus:       LUXURY_AUTO_YEAR1_NO_BONUS_2024,
  luxuryAutoYear1WithBonus:     LUXURY_AUTO_YEAR1_WITH_BONUS_2024,
  luxuryAutoYear2:              LUXURY_AUTO_YEAR2_2024,
  luxuryAutoYear3Plus:          LUXURY_AUTO_YEAR3_PLUS_2024,
  householdFicaThreshold:       HOUSEHOLD_FICA_THRESHOLD_2024,
  householdFutaQuarterlyThreshold: HOUSEHOLD_FUTA_QUARTERLY_THRESHOLD_2024,
  saltCap:                      SALT_CAP_2024,
  saltPhaseoutThreshold:        SALT_PHASEOUT_THRESHOLD_2024,
  saltPhaseoutThresholdMfs:     SALT_PHASEOUT_THRESHOLD_MFS_2024,
  saltPhaseoutRate:             SALT_PHASEOUT_RATE_2024,
  saltFloor:                    SALT_FLOOR_2024,
  saltFloorMfs:                 SALT_FLOOR_MFS_2024,
  depCareExpenseCapOne:         DEP_CARE_EXPENSE_CAP_ONE_2024,
  depCareExpenseCapTwoPlus:     DEP_CARE_EXPENSE_CAP_TWO_PLUS_2024,
  depCareEmployerExclusion:     DEP_CARE_EMPLOYER_EXCLUSION_2024,
  depCareEmployerExclusionMfs:  DEP_CARE_EMPLOYER_EXCLUSION_MFS_2024,
  depCareCreditRateAgiThreshold: DEP_CARE_CREDIT_RATE_AGI_THRESHOLD_2024,
  depCareCreditRateBracketSize: DEP_CARE_CREDIT_RATE_BRACKET_SIZE_2024,
  fplBase:                      FPL_BASE_2024,
  fplIncrement:                 FPL_INCREMENT_2024,
  qcdAnnualLimit:               QCD_ANNUAL_LIMIT_2024,
  psoExclusionLimit:            PSO_EXCLUSION_LIMIT_2024,
  eblThresholdSingle:           EBL_THRESHOLD_SINGLE_2024,
  eblThresholdMfj:              EBL_THRESHOLD_MFJ_2024,
  smallBizGrossReceipts:        SMALL_BIZ_GROSS_RECEIPTS_2024,
  retirementLimits:             RETIREMENT_LIMITS_2024,
  ltcPerDiemDailyLimit:         LTC_PER_DIEM_DAILY_LIMIT_2024,
  mdaMax:                       MDA_MAX_2024,
  mdaPhaseOutThreshold:         MDA_PHASE_OUT_THRESHOLD_2024,
  mdaZeroThreshold:             MDA_ZERO_THRESHOLD_2024,
  deathBenefitMax:              DEATH_BENEFIT_MAX_2024,
  qpriCapStandard:              QPRI_CAP_STANDARD_2024,
  qpriCapMfs:                   QPRI_CAP_MFS_2024,
  scheduleBDividendThreshold:   SCHEDULE_B_DIVIDEND_THRESHOLD_2024,
  sec199aSingleThreshold:       SEC199A_SINGLE_THRESHOLD_2024,
  sec199aMfjThreshold:          SEC199A_MFJ_THRESHOLD_2024,
  sliPhaseOutStartSingle:       SLI_PHASE_OUT_START_SINGLE_2024,
  sliPhaseOutEndSingle:         SLI_PHASE_OUT_END_SINGLE_2024,
  sliPhaseOutStartMfj:          SLI_PHASE_OUT_START_MFJ_2024,
  sliPhaseOutEndMfj:            SLI_PHASE_OUT_END_MFJ_2024,
  f2106PerformingArtistAgiLimit: F2106_PERFORMING_ARTIST_AGI_LIMIT_2024,
  ltcPremiumLimits:             LTC_PREMIUM_LIMITS_2024,
  mccMaxCreditHighRate:         MCC_MAX_CREDIT_HIGH_RATE_2024,
  sepContributionRate:          SEP_CONTRIBUTION_RATE_2024,
  sepMaxContribution:           SEP_MAX_CONTRIBUTION_2024,
  simpleEmployerMatchRate:      SIMPLE_EMPLOYER_MATCH_RATE_2024,
};
