#!/usr/bin/env python3
"""Tests for classify.py. Run: python3 classify_test.py (no deps)."""
import classify as C


def one(num, name, typ, amt, form="f1120s", overrides=None):
    return C.classify(num, name, typ, amt, form, overrides)


def check(cond, msg):
    if not cond:
        raise AssertionError(msg)
    print(f"  ok: {msg}")


def test_named_lines():
    # Rent → 1120-S line 11 (not swept to line 19).
    r = one("5100", "Rent Expense", "expense", -10740)
    check(r.field == "line11_rents" and r.value == 10740 and r.confidence == "high",
          "rent → line 11 (abs)")
    # Advertising → line 16.
    r = one("5600", "Advertising & Marketing", "expense", -20119.15)
    check(r.field == "line16_advertising", "advertising → line 16")
    # License → taxes & licenses line 12.
    r = one("6140", "Business License & Fees", "expense", -225)
    check(r.field == "line12_taxes", "license → line 12 taxes")


def test_automobile_not_telecom():
    # Regression: 'mobile' inside 'automobile' must NOT match telecom.
    r = one("6110", "Automobile Expense", "expense", -10)
    check(r.category == "auto", "automobile classified as auto, not telecom")


def test_meals_factor_and_adjustment():
    r = one("6200", "Meals & Entertainment", "expense", -808.65)
    check(r.factor == 0.5 and abs(r.value - 404.32) < 0.01, "meals 50% factor applied to value")
    check(any("50%" in f for f in r.flags), "meals carries a 50%-limit review flag")
    check(any("factor" in n for n in r.notes), "meals factor recorded as a note (not a flag)")


def test_contractor_flagged():
    r = one("5400", "Contractor Payments", "expense", -34365.47)
    check(r.field == "line19_other_deductions", "contractor → line 19 other")
    check(any("officer" in f.lower() for f in r.flags), "contractor flagged: officer-comp risk")


def test_sweep_is_note_not_flag():
    # Telecom has no dedicated 1120-S line → swept to line 19. Benign note, not a flag.
    r = one("5500", "Telecommunications", "expense", -879.48)
    check(r.field == "line19_other_deductions", "telecom swept to line 19")
    check(r.flags == [] and any("dedicated" in n for n in r.notes),
          "telecom sweep is a note, not a review flag")


def test_income_signed():
    r = one("4000", "Service Revenue", "revenue", 90000)
    check(r.field == "line1a_gross_receipts" and r.sign == "signed" and r.value == 90000,
          "service revenue → line 1a, signed +")


def test_form_specific_targets():
    # Same category, different form → different line.
    s = one("x", "Rent Expense", "expense", -1000, form="f1120s")
    c = one("x", "Rent Expense", "expense", -1000, form="f1120")
    check(s.field == "line11_rents", "rent on 1120-S → line 11")
    check(c.field == "line26_other_deductions", "rent on 1120 → line 26 (no dedicated engine line)")
    e = one("x", "Repairs & Maintenance", "expense", -1000, form="f8825")
    check(e.field == "expense_repairs", "repairs on 8825 → expense_repairs column")


def test_unmatched():
    r = one("9998", "Zorble Fluctuation Reserve", "expense", -500)
    check(r.unmatched and r.node is None and r.confidence == "none",
          "unknown account → UNMATCHED (never silently dropped)")
    check(any("UNMATCHED" in f for f in r.flags), "unmatched carries a blocking flag")


def test_override_wins():
    ov = {"5400": {"category": "officer_comp", "flag": "reclassified per owner"}}
    r = one("5400", "Contractor Payments", "expense", -34365.47, overrides=ov)
    check(r.field == "line7_officer_compensation" and r.confidence == "override",
          "override reclassifies contractor → officer comp line 7")


def test_mortgage_split_flag():
    r = one("5000", "Mortgage Payments - Fidelity (P&I undifferentiated)", "expense", -20000, form="f8825")
    check(r.category == "mortgage_pi" and any("SPLIT" in f for f in r.flags),
          "mortgage P&I flagged to split principal vs interest")


if __name__ == "__main__":
    tests = [v for k, v in sorted(globals().items()) if k.startswith("test_")]
    for t in tests:
        print(f"\n{t.__name__}")
        t()
    print(f"\n✓ all {len(tests)} classify tests passed")
