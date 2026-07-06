#!/usr/bin/env python3
"""step4.py — classifier-driven books→tax fill (the auditable path).

For one entity-year this:
  1. pulls the live P&L (grouped by GL account) from accountir_cloud,
  2. classifies every account to a specific tax line via classify.py
     (+ persisted per-account overrides — the drag-drop decisions),
  3. builds the OpenTax input bundle with GRANULAR lines (rent→11, advertising→16,
     …) plus a line-19/26 itemized "other deductions" statement,
  4. computes via the OpenTax CLI and reconciles to book net PLUS the explicit
     book-tax adjustments (meals 50%, etc. — a mini Schedule M-1),
  5. emits a full per-account audit trace + a machine-readable mapping JSON that
     the step-4 mapping page consumes (books left / tax lines right / flags).

Unlike export_return.py's `manual` maps (source-grounded snapshots), this reads
the LEDGER and shows its work: every dollar is traced from an account to a line.

Usage:
  python3 step4.py --entity hayat --year 2025            # classify + trace + reconcile
  python3 step4.py --entity hayat --year 2025 --json OUT # + write mapping JSON for the UI
"""
import argparse, json, os, sys
from collections import defaultdict

import classify as C
import tags as T
from export_return import psql, deno, ENTITIES, load_map

HERE = os.path.dirname(os.path.abspath(__file__))
OUT_DIR = os.path.join(HERE, "out")

# Rental-only S-corps file Form 1120-S but their income/expenses are separately
# stated on Form 8825 (→ Schedule K line 2), NOT page-1 ordinary. So their accounts
# are CLASSIFIED against the 8825 column set even though the filing form is f1120s.
RENTAL_ENTITIES = {"sweethome"}


def classify_form(entity, filing_form):
    return "f8825" if entity in RENTAL_ENTITIES else filing_form


def pull_pl(cid, year):
    rows = psql(f"""
        SELECT a.account_number, a.name, a.account_type,
               ROUND(-SUM(jl.amount)/100.0, 2) AS book_net
        FROM journal_entries je
        JOIN journal_lines jl ON jl.entry_id = je.id
        JOIN accounts a ON a.id = jl.account_id
        WHERE je.company_id = '{cid}' AND je.is_void = false
          AND a.account_type IN ('revenue','expense')
          AND je.date BETWEEN '{year}-01-01' AND '{year}-12-31'
        GROUP BY a.account_number, a.name, a.account_type
        ORDER BY a.account_number;""")
    return [(r[0], r[1], r[2], float(r[3])) for r in rows]


def resolve_splits(tag, amount):
    """Turn a tag's split spec into resolved contributions. Each split carries an
    absolute `amount` or a `pct` of the account's book magnitude."""
    base = abs(amount)
    out = []
    for sp in tag["splits"]:
        val = round(base * sp["pct"] / 100.0, 2) if sp.get("pct") is not None else round(abs(sp["amount"]), 2)
        out.append({"node": sp["node"], "field": sp["field"], "line": sp.get("line"), "value": val})
    return out


def load_filer(form, entity):
    """Reuse the identity/filer block from the existing hand map (stable across years)."""
    try:
        themap, _ = load_map(form, 9999, entity)  # 9999 forces the year-agnostic file
    except SystemExit:
        return None
    return themap.get("filer")


def build(entity, year, form):
    rows = psql(f"SELECT id FROM companies WHERE slug ILIKE '%{entity}%' LIMIT 1;")
    if not rows:
        sys.exit(f"no company matching '{entity}'")
    cid = rows[0][0]
    pl = pull_pl(cid, year)
    if not pl:
        sys.exit(f"no {year} ledger for {entity}")

    # Two authorities, cleanly separated:
    #   • the classifier does the ANALYSIS (category, confidence, review flags),
    #   • the tag store owns the MAPPING (node/field/factor/sign/sep/splits).
    # A tag applies its mapping over the classifier's result; a human-reviewed tag
    # (confirmed/override) also clears the classifier's flags — those questions are
    # answered. Book AMOUNTS always come live from the ledger.
    tags = T.load_tags(entity)
    tag_path = T.tag_path(entity) if tags else None
    tag_accounts = (tags or {}).get("accounts", {})
    cform = classify_form(entity, form)             # 8825 column set for rental entities

    classified, unmatched, untagged = [], [], []
    for num, name, typ, amt in pl:
        if abs(amt) < 0.005:
            continue
        r = C.classify(num, name, typ, amt, cform)   # analysis: natural flags/confidence
        tag = tag_accounts.get(num)
        r.splits = None
        if tag:
            r.tag_status = tag["status"]
            if tag.get("node") is not None:          # apply the tag's mapping
                r.node, r.field = tag["node"], tag["field"]
                r.line_label = tag.get("line", r.line_label)
            if tag.get("sign"):
                r.sign = tag["sign"]
            r.sep = tag.get("sep", r.sep)
            r.factor = tag.get("factor")
            base = abs(amt) if r.sign == "abs" else amt
            r.value = round(base * (r.factor if r.factor is not None else 1), 2)
            if tag.get("splits"):
                r.splits = resolve_splits(tag, amt)
            if tag["status"] in T.HUMAN_STATUSES:    # reviewed → questions answered
                r.flags = []
        else:
            r.tag_status = "untagged"
            untagged.append(r)
        classified.append(r)
        if r.unmatched:
            unmatched.append(r)

    book_net = round(sum(r.amount for r in classified), 2)
    return cid, classified, unmatched, untagged, book_net, tag_path


def to_bundle(classified, form, filer, year):
    """Fold classifications into the OpenTax input bundle. Accounts sharing a
    (node, field) accumulate; a split account contributes to several lines;
    Form-8825 accounts assemble per property (one income account = one property;
    pooled expenses are allocated across properties pro-rata by gross rents)."""
    fields = defaultdict(float)          # (node, field) -> value, non-8825 nodes
    rentals = []                         # (property_address, gross_rents)
    pooled_exp = defaultdict(float)      # 8825 expense field -> pooled total
    for r in classified:
        if getattr(r, "splits", None):
            for sp in r.splits:
                fields[(sp["node"], sp["field"])] += sp["value"]
        elif r.node == "f8825":
            if r.field == "gross_rents":
                rentals.append((r.account_name, r.value))
            else:
                pooled_exp[r.field] += r.value
        elif not r.unmatched:
            fields[(r.node, r.field)] += r.value

    per_node = defaultdict(dict)
    for (node, fld), v in fields.items():
        per_node[node][fld] = round(v, 2)

    forms = [filer] if filer else []
    for node, data in per_node.items():
        forms.append({"node_type": node, "data": data})

    # One Form-8825 entry per property; the engine accumulates them into f8825s.
    total_rent = sum(v for _, v in rentals)
    for addr, rent in rentals:
        item = {"property_address": addr, "gross_rents": round(rent, 2)}
        share = (rent / total_rent) if total_rent else 0
        for fld, amt in pooled_exp.items():
            if amt:
                item[fld] = round(amt * share, 2)
        forms.append({"node_type": "f8825", "data": item})

    return {"year": year, "forms": forms}, per_node


def adjustments(classified):
    """Book-tax differences the classifier introduced (factors < 1), i.e. the
    disallowed portion added back to income — a mini Schedule M-1."""
    adj = []
    for r in classified:
        if r.factor is not None and r.factor < 1:
            full = abs(r.amount)
            disallowed = round(full - r.value, 2)
            if disallowed:
                adj.append((r.account_number, r.account_name, full, r.value, disallowed))
    return adj


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--entity", required=True, choices=list(ENTITIES))
    ap.add_argument("--year", type=int, default=2025)
    ap.add_argument("--json", metavar="PATH", help="write machine-readable mapping JSON for the UI")
    ap.add_argument("--no-compute", action="store_true", help="skip the engine compute")
    args = ap.parse_args()
    label, etype, form = ENTITIES[args.entity]

    cid, classified, unmatched, untagged, book_net, tag_path = build(args.entity, args.year, form)
    filer = load_filer(form, args.entity)

    method_rows = psql(f"SELECT accounting_method FROM tax_profiles WHERE company_id = '{cid}';")
    method = (method_rows[0][0] if method_rows and method_rows[0][0] else "not set — configure tax_profiles.accounting_method")
    print(f"Entity   : {label} ({etype})")
    print(f"Form     : {form}   Year: {args.year}   Method: {method}")
    print(f"Tag store: {os.path.relpath(tag_path, HERE) if tag_path else '(none — run tag_accounts.py; using live rules)'}")
    print(f"Book net : {book_net:,.2f}\n")

    # ── Audit trace ──────────────────────────────────────────────────────────
    print("AUDIT TRACE  (every account → tax line)")
    print(f"  {'acct':<5} {'account name':<32} {'book':>12}  {'→ line':<32} {'val':>11} {'status':<9}")
    print("  " + "-" * 118)
    for r in sorted(classified, key=lambda x: x.account_number):
        if getattr(r, "splits", None):
            line = "SPLIT → " + " + ".join(sp.get("line") or sp["field"] for sp in r.splits)
        else:
            line = r.line_label or "‼ UNMATCHED — assign a line"
        char = " (K)" if r.sep else ""
        st = getattr(r, "tag_status", "untagged")
        print(f"  {r.account_number:<5} {r.account_name[:32]:<32} {r.amount:>12,.2f}  "
              f"{(line + char)[:32]:<32} {r.value:>11,.2f} {st:<9}")
        if getattr(r, "splits", None):
            for sp in r.splits:
                print(f"  {'':<5} {'  ↳ ' + (sp.get('line') or sp['field']):<32} {'':>12}  {'':<32} {sp['value']:>11,.2f}")

    # ── Line-19/26 itemized statement ────────────────────────────────────────
    other_field = C.OTHER_LINE.get(form)
    if other_field:
        items = [r for r in classified if r.field == other_field.field and not r.unmatched]
        if items:
            tot = sum(r.value for r in items)
            print(f"\n  {other_field.label} — itemized statement")
            for r in sorted(items, key=lambda x: -x.value):
                print(f"    {r.account_name[:40]:<40} {r.value:>12,.2f}")
            print(f"    {'TOTAL':<40} {tot:>12,.2f}")

    # ── Schedule M-1 style book-tax adjustments ──────────────────────────────
    adj = adjustments(classified)
    recon_target = book_net
    if adj:
        print(f"\n  BOOK-TAX ADJUSTMENTS (added back to income)")
        for num, name, full, allowed, dis in adj:
            print(f"    {name[:34]:<34} book {full:>10,.2f}  allowed {allowed:>10,.2f}  add-back {dis:>9,.2f}")
        recon_target = round(book_net + sum(a[4] for a in adj), 2)

    # ── Review flags ─────────────────────────────────────────────────────────
    flagged = [r for r in classified if r.flags]
    if flagged:
        print(f"\n  NEEDS_REVIEW ({len(flagged)}):")
        for r in flagged:
            for fl in r.flags:
                print(f"    [{r.account_number}] {r.account_name[:28]:<28} {fl}")

    if untagged:
        print(f"\n  ⚠ UNTAGGED ({len(untagged)}) — not yet in the tag store (using live rule fallback):")
        for r in untagged:
            print(f"    {r.account_number}  {r.account_name[:40]:<40} → {r.line_label or 'UNMATCHED'}")
        print(f"    run: python3 tag_accounts.py --entity {args.entity}")

    if unmatched:
        print(f"\n  ‼ UNMATCHED ({len(unmatched)}) — these block a clean fill:")
        for r in unmatched:
            print(f"    {r.account_number}  {r.account_name}  {r.amount:,.2f}")

    # ── Compute + reconcile ──────────────────────────────────────────────────
    bundle, per_node = to_bundle(classified, form, filer, args.year)
    os.makedirs(OUT_DIR, exist_ok=True)
    bundle_path = os.path.join(OUT_DIR, f"{args.entity}_{args.year}_step4_input.json")
    with open(bundle_path, "w") as f:
        json.dump(bundle, f, indent=2)

    computed = None
    if not args.no_compute:
        create = ["return", "create", "--year", str(args.year), "--json"]
        if form != "f1040":
            create += ["--form", form]
        rid = json.loads(deno(*create))["returnId"]
        for fm in bundle["forms"]:
            deno("form", "add", "--returnId", rid, "--node_type", fm["node_type"], json.dumps(fm["data"]), "--json")
        result = json.loads(deno("return", "get", "--returnId", rid, "--json"))
        pend = result.get("pending", {})

        def scalar(node, field):
            v = pend.get(node, {}).get(field)
            v = v[-1] if isinstance(v, list) else v
            return float(v) if isinstance(v, (int, float)) else 0.0

        # An S-corp's book net splits across Schedule K: ordinary (line 21) + net
        # rental real estate (Sch K line 2, via Form 8825). Sum them to reconcile.
        if form == "f1120s":
            computed = round(scalar("f1120s", "line21_ordinary_business_income")
                             + scalar("schedule_k", "line2_net_rental_real_estate"), 2)
            line_key = ("f1120s", "line21 + Sch K-2 net rental")
        else:
            key = {"f1120": ("f1120", "line28_income_before_nol"),
                   "f1040": ("f1040", "line9_total_income")}[form]
            computed = scalar(key[0], key[1])
            line_key = key

        print(f"\n  RECONCILIATION ({form})")
        print(f"    book net                    : {book_net:>14,.2f}")
        if adj:
            print(f"    + book-tax add-backs        : {sum(a[4] for a in adj):>14,.2f}")
            print(f"    = tax ordinary (target)     : {recon_target:>14,.2f}")
        print(f"    computed {line_key[1][:18]:<18} : {(computed or 0):>14,.2f}")
        delta = round((computed or 0) - recon_target, 2)
        print(f"    delta                       : {delta:>14,.2f}   "
              f"{'OK (ties)' if abs(delta) < 0.5 else 'DELTA → review'}")

    # ── Machine-readable mapping for the UI ──────────────────────────────────
    if args.json:
        payload = {
            "entity": args.entity, "label": label, "form": form, "year": args.year,
            "book_net": book_net, "recon_target": recon_target, "computed": computed,
            "accounts": [{
                "account_number": r.account_number, "account_name": r.account_name,
                "account_type": r.account_type, "amount": r.amount,
                "category": r.category, "node": r.node, "field": r.field,
                "line_label": r.line_label, "value": r.value, "sep": r.sep,
                "confidence": r.confidence, "flags": r.flags, "notes": r.notes,
                "factor": r.factor, "unmatched": r.unmatched,
                "tag_status": getattr(r, "tag_status", "untagged"),
                "splits": getattr(r, "splits", None),
            } for r in sorted(classified, key=lambda x: x.account_number)],
        }
        with open(args.json, "w") as f:
            json.dump(payload, f, indent=2)
        print(f"\n  mapping JSON → {args.json}")


if __name__ == "__main__":
    main()
