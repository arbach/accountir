#!/usr/bin/env python3
"""Fill an Illinois return's key computed lines. IL PDFs use long descriptive
AcroForm field names; we match the target /Tx field by substring and set it via
pypdf, then flag NeedAppearances so viewers render the values."""
import sys
from pypdf import PdfReader, PdfWriter
from pypdf.generic import NameObject, BooleanObject

# form -> [(value, [substrings that identify the field, case-insensitive])]
SPECS = {
    "il1120st": [("net", ["line 51 - enter your net income"]),
                 ("tax", ["line 56 - net replacement tax"])],
    "il1120":   [("net", ["line 39 - net income"]),
                 ("tax", ["line 58 - total net income and replacement"])],
    "il1040":   [("net", ["illinois base income"]),
                 ("tax", ["total tax from page 1"])],
}


def find_key(flds, subs):
    for k, v in flds.items():
        if v.get("/FT") == "/Tx" and all(s in k.lower() for s in subs):
            return k
    return None


def fill(form, tmpl, out, net, tax):
    r = PdfReader(tmpl)
    flds = r.get_fields() or {}
    vals = {}
    for which, subs in SPECS[form]:
        amt = net if which == "net" else tax
        if amt == 0:
            continue
        key = find_key(flds, subs)
        if key:
            # Losses print in parentheses (IL convention); tax is never negative.
            vals[key] = f"({abs(round(amt)):,})" if amt < 0 else f"{round(amt):,}"
        else:
            print(f"  !! no field for {which} ({subs})")
    w = PdfWriter()
    w.append(r)
    for p in w.pages:
        try:
            w.update_page_form_field_values(p, vals)
        except Exception:
            pass
    # NeedAppearances so viewers render the set values
    try:
        w._root_object["/AcroForm"][NameObject("/NeedAppearances")] = BooleanObject(True)
    except Exception:
        pass
    with open(out, "wb") as fh:
        w.write(fh)
    # verify
    f2 = PdfReader(out).get_fields() or {}
    got = {k.split(" - ")[0][:16]: f2.get(k, {}).get("/V") for k in vals}
    print(f"  {form}: set {got}")


if __name__ == "__main__":
    form, tmpl, out, net, tax = sys.argv[1], sys.argv[2], sys.argv[3], float(sys.argv[4]), float(sys.argv[5])
    fill(form, tmpl, out, net, tax)
