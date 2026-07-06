#!/usr/bin/env python3
"""classify.py — books → tax-line classifier (the step-4 mapping brain).

The problem this solves: a chart-of-accounts line (e.g. "6160 Advertising & Marketing")
must land on a *specific* tax-form line (1120-S line 16 Advertising), with the right
sign, the right character (ordinary vs separately-stated), and a paper trail for audit.
The old bridge dumped almost everything into "other deductions" — losing the line
detail and the officer-comp/contractor distinction. This module fixes that.

Design — two layers, both data-driven and auditable:

  layer 1  account (number, name, type)  ──rules──▶  canonical CATEGORY
  layer 2  (CATEGORY, form)              ──CATEGORY_LINES──▶ tax line (node, field, sign)

Layer 1 matches primarily on the account NAME (semantic), because the numbering
convention differs across entities (Maven 6xxx, Hayat/SweetHome 5xxx). The account
NUMBER and TYPE are hints/guards. Layer 2 knows each form's real line structure, so
the same category ("rent") lands on 1120-S line 11, 1120 line 16, or 8825 expense_other
depending on the form.

Every classification returns a full trace: which rule fired, the category, the target
line, a confidence, and any review flag. Nothing is ever silently dropped — an account
that matches no rule comes back as UNMATCHED (confidence 0) so the caller must surface it.

This module is pure (no DB, no engine) so it is unit-testable and drives both the
headless bridge and the interactive step-4 mapping page.
"""
from __future__ import annotations
import re
from dataclasses import dataclass, field
from typing import Optional

# ─────────────────────────────────────────────────────────────────────────────
# Layer 1 — account → canonical category
#
# Ordered rules; FIRST match wins, so put specific patterns before general ones.
# A rule matches on the account NAME (case-insensitive regex). `only` optionally
# restricts a rule to revenue|expense. `flag` attaches a review note that rides
# along to the caller (e.g. contractor-vs-officer-comp, meals 50%).
# ─────────────────────────────────────────────────────────────────────────────

@dataclass(frozen=True)
class Rule:
    pattern: str
    category: str
    only: Optional[str] = None          # "revenue" | "expense" | None (either)
    flag: Optional[str] = None          # review note surfaced to the caller
    confidence: str = "high"            # high | medium (medium => worth a glance)

RULES: list[Rule] = [
    # ── Income ──────────────────────────────────────────────────────────────
    Rule(r"\brental income|\brent(al)? .*income|rental .*\b(unit|property|apt|house)", "rental_income", "revenue"),
    Rule(r"ip sales|intellectual property.*sale", "ip_sales_income", "revenue"),
    Rule(r"grant income", "grant_income", "revenue"),
    Rule(r"interest income", "interest_income", "revenue"),
    Rule(r"dividend", "dividend_income", "revenue"),
    Rule(r"capital gain|long-?term|short-?term", "capital_gain_income", "revenue"),
    # Individual (1040) passthrough: K-1 income/loss from S-corps, partnerships, LPs,
    # and direct rentals — all land on Schedule E. Applies to revenue and expense
    # (a passthrough LOSS account stays signed).
    Rule(r"schedule e\b|k-?1|passthrough|s-?corp|partnership|\blp\b", "k1_passthrough", None),
    Rule(r"\bhsa\b|health savings", "hsa", "expense"),
    Rule(r"management fee|consulting|technology .*(service|revenue)|service revenue|"
         r"\bservice\b|markup|reimbursable", "service_revenue", "revenue"),
    Rule(r"prior year revenue|collections", "service_revenue", "revenue",
         flag="cash-basis prior-year collection — confirm cash vs accrual"),
    Rule(r"currency gain|revaluation|unrealized|realized", "other_income", "revenue",
         flag="FX/revaluation — book artifact; confirm it is real taxable income", confidence="medium"),
    Rule(r"currency (gain|loss)|revaluation|unrealized|realized (currency|fx|gain|loss)|fx (gain|loss)",
         "other_expense", "expense",
         flag="FX/revaluation booked as expense — often book-only/unrealized; confirm M-1 treatment (may be non-deductible)",
         confidence="medium"),
    Rule(r"vendor refund|uncategorized income|other income", "other_income", "revenue", confidence="medium"),

    # ── Cost of goods sold ──────────────────────────────────────────────────
    Rule(r"cost of goods|cogs|purchase discount", "cogs", "expense"),

    # ── Named deduction lines (order matters — specific first) ───────────────
    Rule(r"subcontractor|contractor payment|contract labor|1099", "contractor", "expense",
         flag="contractor vs officer/employee comp — verify 1099 issued & no disguised owner pay",
         confidence="medium"),
    Rule(r"officer comp|owner.*(salary|comp|draw)|shareholder.*comp", "officer_comp", "expense"),
    Rule(r"payroll tax", "taxes_licenses", "expense"),
    Rule(r"wages|salaries|payroll(?! tax)", "wages", "expense"),
    Rule(r"pension|profit.?sharing|401\(?k\)?|retirement plan|sep.?ira|simple.?ira", "pension", "expense"),
    Rule(r"employee benefit|health insurance.*employee|group insurance|hsa contribution", "benefits", "expense"),
    Rule(r"repairs?|maintenance", "repairs", "expense"),
    Rule(r"bad debt", "bad_debt", "expense"),
    Rule(r"mortgage|principal & interest|\bp&i\b|principal and interest", "mortgage_pi", "expense",
         flag="mortgage P&I undifferentiated — SPLIT principal (non-deductible) vs interest (deductible)"),
    Rule(r"interest expense|loan interest|\binterest\b", "interest_expense", "expense"),
    Rule(r"rent expense|lease expense|\brent\b|\blease\b", "rent", "expense"),
    Rule(r"business.*(tax|licen)|licen[sc]e|business tax|franchise tax|registration fee", "taxes_licenses", "expense"),
    Rule(r"property tax|real estate tax", "taxes_licenses", "expense"),
    Rule(r"depreciation|amortization|depletion|179", "depreciation", "expense"),
    Rule(r"advertis|marketing|promotion", "advertising", "expense"),
    Rule(r"charitable|donation|contribution", "charitable", "expense",
         flag="charitable — separately-stated on Sch K (S-corp) / limited on 1120 (C-corp)"),

    # ── Ordinary operating expenses (→ line 19 'other', itemized) ────────────
    Rule(r"meals? ?(&|and)? ?entertainment|meals?\b|entertainment", "meals", "expense",
         flag="meals — apply 50% limit (§274(n)); some categories are 100%/0%", confidence="medium"),
    Rule(r"travel|hotel|accommodation|airfare|lodging", "travel", "expense"),
    Rule(r"insurance", "insurance", "expense"),
    Rule(r"professional (fee|service)|accounting fee|legal|attorney|bookkeep|cpa|tax prep|consulting fee", "professional_fees", "expense"),
    Rule(r"medical billing|medical platform|zocdoc|providersca|billing service", "outside_services", "expense",
         flag="platform/billing fee — confirm it is a real expense, not netted-out contra-revenue",
         confidence="medium"),
    Rule(r"bank .*(charge|fee)|merchant .*fee|financial fee|service charge", "bank_fees", "expense"),
    Rule(r"computer|software|hosting|technology|saas|subscription.*(software|tech)", "software", "expense"),
    Rule(r"dues|subscription|membership", "dues", "expense"),
    Rule(r"telephone|telecom|internet|\bphone\b|\bmobile\b", "telecom", "expense"),
    Rule(r"utilit|electric|gas & electric|water|sewer", "utilities", "expense"),
    Rule(r"office (supplies|expense)|supplies|postage|freight|delivery|shipping", "office", "expense"),
    Rule(r"automobile|vehicle|fuel|mileage|auto expense", "auto", "expense"),
    Rule(r"home warranty|warranty|service plan", "other_expense", "expense"),
    Rule(r"commission", "commissions", "expense"),
    Rule(r"cleaning", "cleaning", "expense"),
    Rule(r"miscellaneous|other expense|uncategorized|reimbursable expense", "other_expense", "expense",
         flag="uncategorized/misc — needs an account-level breakdown for the return", confidence="medium"),
]

# ─────────────────────────────────────────────────────────────────────────────
# Layer 2 — (category, form) → tax line
#
# node/field must match the OpenTax input node's schema for that form. `sign`:
#   "abs"    → pass magnitude (deductions/8825 expenses are nonnegative lines)
#   "signed" → pass as-is (1040 passthrough losses keep their sign)
# `sep` marks separately-stated items (do NOT reduce ordinary income). `label` is
# the human line name shown in the audit trace and the mapping UI.
# ─────────────────────────────────────────────────────────────────────────────

@dataclass(frozen=True)
class Line:
    node: str
    field: str
    label: str
    sign: str = "abs"
    sep: bool = False            # separately-stated (Schedule K), not ordinary
    factor: Optional[float] = None   # e.g. meals 0.5 (advisory; applied by caller/UI)

# form -> category -> Line. A category absent for a form falls through to that
# form's "other deductions" line (see OTHER_LINE) so nothing is dropped.
CATEGORY_LINES: dict[str, dict[str, Line]] = {
    "f1120s": {
        "service_revenue":  Line("f1120s", "line1a_gross_receipts", "1a Gross receipts", "signed"),
        "grant_income":     Line("f1120s", "line5_other_income",     "5 Other income",    "signed"),
        "ip_sales_income":  Line("f1120s", "line5_other_income",     "5 Other income",    "signed"),
        "other_income":     Line("f1120s", "line5_other_income",     "5 Other income",    "signed"),
        "interest_income":  Line("schedule_k", "line4_interest_income", "Sch K-4 Interest income", "signed", sep=True),
        "dividend_income":  Line("schedule_k", "line5a_ordinary_dividends", "Sch K-5a Dividends", "signed", sep=True),
        "rental_income":    Line("f8825", "gross_rents", "8825 Gross rents", "abs", sep=True),
        "cogs":             Line("f1120s", "line2_cogs", "2 Cost of goods sold"),
        "officer_comp":     Line("f1120s", "line7_officer_compensation", "7 Compensation of officers"),
        "contractor":       Line("f1120s", "line19_other_deductions", "19 Other (contract labor)"),
        "wages":            Line("f1120s", "line8_salaries_wages", "8 Salaries and wages"),
        "repairs":          Line("f1120s", "line9_repairs_maintenance", "9 Repairs and maintenance"),
        "bad_debt":         Line("f1120s", "line10_bad_debts", "10 Bad debts"),
        "rent":             Line("f1120s", "line11_rents", "11 Rents"),
        "taxes_licenses":   Line("f1120s", "line12_taxes", "12 Taxes and licenses"),
        "interest_expense": Line("f1120s", "line13_interest", "13 Interest"),
        "depreciation":     Line("f1120s", "line14_depreciation", "14 Depreciation"),
        "advertising":      Line("f1120s", "line16_advertising", "16 Advertising"),
        "pension":          Line("f1120s", "line17_pension_profit_sharing", "17 Pension/profit-sharing"),
        "benefits":         Line("f1120s", "line18_employee_benefits", "18 Employee benefit programs"),
        "charitable":       Line("schedule_k", "line12a_charitable", "Sch K-12a Charitable contributions", "abs", sep=True),
        "meals":            Line("f1120s", "line19_other_deductions", "19 Other (meals, 50%)", factor=0.5),
        # everything else deductible → line 19 (itemized statement)
    },
    "f1120": {
        "service_revenue":  Line("f1120", "line1a_gross_receipts", "1a Gross receipts", "signed"),
        "ip_sales_income":  Line("f1120", "line1a_gross_receipts", "1a Gross receipts", "signed"),
        "grant_income":     Line("f1120", "line10_other_income", "10 Other income", "signed"),
        "other_income":     Line("f1120", "line10_other_income", "10 Other income", "signed"),
        "interest_income":  Line("f1120", "line5_interest", "5 Interest", "signed"),
        "dividend_income":  Line("f1120", "line4_dividends", "4 Dividends", "signed"),
        "rental_income":    Line("f1120", "line6_gross_rents", "6 Gross rents", "signed"),
        "capital_gain_income": Line("f1120", "line8_capital_gain", "8 Capital gain net income", "signed"),
        "cogs":             Line("f1120", "line2_cogs", "2 Cost of goods sold"),
        "officer_comp":     Line("f1120", "line12_officer_compensation", "12 Compensation of officers"),
        "contractor":       Line("f1120", "line26_other_deductions", "26 Other (contract labor)"),
        "wages":            Line("f1120", "line13_salaries_wages", "13 Salaries and wages"),
        "taxes_licenses":   Line("f1120", "line17_taxes_licenses", "17 Taxes and licenses"),
        "charitable":       Line("f1120", "line19_charitable", "19 Charitable contributions"),
        "depreciation":     Line("f1120", "line20_depreciation", "20 Depreciation"),
        "meals":            Line("f1120", "line26_other_deductions", "26 Other (meals, 50%)", factor=0.5),
        # repairs/interest/rent/advertising: no dedicated 1120 line in-engine → line 26 other
    },
    # Form 1040 (individual). The books are schedule-organized, so each account
    # maps to its schedule line. All signed (passthrough losses stay negative).
    # These fields match the Accounts-page dropdown options (node "start"). The
    # DETAILED return still computes from source docs (K-1 boxes, 8949 basis) via
    # the manual map; this covers tagging + a total-income reconciliation.
    "f1040": {
        "interest_income":    Line("start", "line2b_interest", "Sch B — Interest income", "signed"),
        "dividend_income":    Line("start", "line3b_dividends", "Sch B — Ordinary dividends", "signed"),
        "qualified_dividend": Line("start", "line3b_dividends", "Sch B — Dividends", "signed"),
        "capital_gain_income":Line("start", "line7_capital_gain", "Sch D — Capital gain (8949)", "signed"),
        "k1_passthrough":     Line("start", "line5_schedule_e", "Sch E — Passthrough/rental (K-1)", "signed"),
        "k1_rental":          Line("start", "line5_schedule_e", "Sch E — Rental (K-1)", "signed"),
        "other_income":       Line("start", "schedule1_other_income", "Sch 1 — Other income", "signed"),
        "hsa":                Line("start", "schedule1_hsa", "Sch 1 — HSA adjustment", "abs"),
    },
    # Form 8825 (rentals) — one item per property. Categories map to the 8825
    # expense columns; the property grouping is handled by the caller (per-account
    # property tag), not here.
    "f8825": {
        "rental_income":    Line("f8825", "gross_rents", "2 Gross rents", "abs"),
        "advertising":      Line("f8825", "expense_advertising", "3 Advertising"),
        "auto":             Line("f8825", "expense_auto_travel", "4 Auto and travel"),
        "travel":           Line("f8825", "expense_auto_travel", "4 Auto and travel"),
        "cleaning":         Line("f8825", "expense_cleaning_maintenance", "5 Cleaning and maintenance"),
        "repairs":          Line("f8825", "expense_repairs", "12 Repairs"),
        "commissions":      Line("f8825", "expense_commissions", "6 Commissions"),
        "insurance":        Line("f8825", "expense_insurance", "7 Insurance"),
        "professional_fees":Line("f8825", "expense_legal_professional", "8 Legal and professional"),
        "interest_expense": Line("f8825", "expense_interest", "9 Interest"),
        "mortgage_pi":      Line("f8825", "expense_interest", "9 Interest (from P&I — principal excluded)"),
        "taxes_licenses":   Line("f8825", "expense_taxes", "11 Taxes"),
        "utilities":        Line("f8825", "expense_utilities", "13 Utilities"),
        "wages":            Line("f8825", "expense_wages_salaries", "14 Wages and salaries"),
        "depreciation":     Line("f8825", "expense_depreciation", "14 Depreciation"),
        "bank_fees":        Line("f8825", "expense_other", "15 Other"),
        "other_expense":    Line("f8825", "expense_other", "15 Other"),
    },
}

# Fallback "other deductions" line per form — where any unlisted deductible
# expense category lands (with an itemized statement). Income/COGS/separately-
# stated categories that are unlisted are NOT swept here (they'd distort the base);
# they surface as UNMATCHED instead.
OTHER_LINE: dict[str, Line] = {
    "f1120s": Line("f1120s", "line19_other_deductions", "19 Other deductions"),
    "f1120":  Line("f1120", "line26_other_deductions", "26 Other deductions"),
    "f8825":  Line("f8825", "expense_other", "15 Other"),
}

# Categories that are deductible operating expenses (safe to sweep to "other"
# when a form has no dedicated line for them). Income / separately-stated /
# capital categories are intentionally excluded.
_DEDUCTIBLE_EXPENSE_CATEGORIES = {
    "contractor", "outside_services", "meals", "travel", "insurance",
    "professional_fees", "bank_fees", "software", "dues", "telecom",
    "utilities", "office", "auto", "commissions", "cleaning", "repairs",
    "rent", "advertising", "interest_expense", "taxes_licenses", "wages",
    "officer_comp", "pension", "benefits", "bad_debt", "depreciation",
    "other_expense", "mortgage_pi",
}


# ─────────────────────────────────────────────────────────────────────────────
# Result type + classify()
# ─────────────────────────────────────────────────────────────────────────────

@dataclass
class Classification:
    account_number: str
    account_name: str
    account_type: str
    amount: float                 # book net (signed; revenue +, expense −)
    category: str                 # canonical category or "UNMATCHED"
    node: Optional[str]           # target engine input node
    field: Optional[str]          # target line field
    line_label: Optional[str]     # human line name
    sign: str                     # abs | signed
    sep: bool                     # separately-stated (not ordinary)
    value: float                  # amount actually fed to the engine (abs applied)
    confidence: str               # high | medium | low | none
    matched_pattern: Optional[str]
    flags: list[str] = field(default_factory=list)   # genuine review concerns (tax risk)
    notes: list[str] = field(default_factory=list)   # benign placement/mechanics notes
    factor: Optional[float] = None

    @property
    def unmatched(self) -> bool:
        return self.category == "UNMATCHED" or self.node is None


def categorize(name: str, account_type: str) -> Optional[Rule]:
    """Layer 1: first rule whose pattern matches the name and whose `only` fits."""
    low = name.lower()
    for rule in RULES:
        if rule.only and rule.only != account_type:
            continue
        if re.search(rule.pattern, low):
            return rule
    return None


def classify(account_number: str, name: str, account_type: str, amount: float,
             form: str, overrides: Optional[dict] = None) -> Classification:
    """Classify one account for one form. `overrides` maps account_number → a dict
    forcing {category|node|field|line_label|sign|sep|factor|flag}; the persisted,
    human-confirmed drag-drop decisions live here and win over the rules."""
    flags: list[str] = []
    ov = (overrides or {}).get(account_number)

    if ov and ov.get("category"):
        category, rule_conf, matched = ov["category"], "override", "user-override"
        if ov.get("flag"):
            flags.append(ov["flag"])
    else:
        rule = categorize(name, account_type)
        if rule is None:
            return Classification(
                account_number, name, account_type, amount, "UNMATCHED",
                None, None, None, "signed", False, 0.0, "none", None,
                flags=[f"UNMATCHED: no rule for '{name}' — assign a tax line before filing"],
            )
        category, rule_conf, matched = rule.category, rule.confidence, rule.pattern
        if rule.flag:
            flags.append(rule.flag)

    lines = CATEGORY_LINES.get(form, {})
    line = lines.get(category)
    conf = rule_conf
    notes: list[str] = []

    if line is None:
        # No dedicated line on this form. Sweep deductible expenses to "other"
        # (a legitimate presentation — line 19/26 carries an itemized statement).
        # This is a benign placement NOTE, not a review flag. Non-deductible /
        # income / separately-stated categories can't be swept → flagged instead.
        if category in _DEDUCTIBLE_EXPENSE_CATEGORIES and form in OTHER_LINE:
            line = OTHER_LINE[form]
            notes.append(f"no dedicated {form} line for '{category}' → line {line.field} (itemized statement)")
        else:
            return Classification(
                account_number, name, account_type, amount, category,
                None, None, None, "signed", False, 0.0, "low", matched,
                flags=flags + [f"'{category}' has no {form} line and is not sweepable — assign manually"],
                factor=None,
            )

    # Apply overrides to the resolved line (node/field/sign/sep/factor/label).
    node = (ov or {}).get("node", line.node)
    field_ = (ov or {}).get("field", line.field)
    label = (ov or {}).get("line_label", line.label)
    sign = (ov or {}).get("sign", line.sign)
    sep = (ov or {}).get("sep", line.sep)
    factor = (ov or {}).get("factor", line.factor)

    value = abs(amount) if sign == "abs" else amount
    if factor is not None:
        base = abs(amount) if sign == "abs" else amount
        value = round(value * factor, 2)
        notes.append(f"factor {factor:g} applied ({base:,.2f} → {value:,.2f})")

    return Classification(
        account_number, name, account_type, amount, category,
        node, field_, label, sign, sep, round(value, 2),
        "override" if (ov and ov.get("category")) else conf,
        matched, flags=flags, notes=notes, factor=factor,
    )
