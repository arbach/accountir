#!/usr/bin/env python3
"""coa_gap.py — diff an entity's live chart of accounts against its tax-aligned
template (Tier 3), producing the actionable gap that feeds the Tier-2 change request.

Reports, for one entity:
  1. Coverage — how many tax lines have a dedicated account vs lump into "other".
  2. Missing named accounts — template lines with a REAL tax line the entity has no
     account for (add these so future transactions land on the right line).
  3. Structural flags — accounts the classifier flagged for a split/reclass
     (contractor→officer comp, mortgage P&I, contra-revenue, uncategorized lumps).

Usage:
  python3 coa_gap.py --entity hayat
  python3 coa_gap.py --all
"""
import argparse, json, os, sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "bridge"))
import classify as C
from export_return import psql, ENTITIES

HERE = os.path.dirname(os.path.abspath(__file__))
TPL_DIR = os.path.join(HERE, "templates")

# entity slug -> (entity_type, template file)
ENTITY_TEMPLATE = {
    "maven": "c_corp",
    "hayat": "s_corp",
    "sweethome": "s_corp_rental",
    "michael": "individual",
    "on-chain": "s_corp",
}


def load_template(entity_type):
    with open(os.path.join(TPL_DIR, f"{entity_type}.json")) as f:
        return json.load(f)


def pull_accounts(cid):
    """All revenue/expense accounts (structure, not just non-zero)."""
    rows = psql(f"""
        SELECT account_number, name, account_type FROM accounts
        WHERE company_id = '{cid}' AND account_type IN ('revenue','expense') AND is_active
        ORDER BY account_number;""")
    return [(r[0], r[1], r[2]) for r in rows]


def report(entity):
    if entity not in ENTITY_TEMPLATE:
        print(f"  {entity}: no template mapping"); return
    etype = ENTITY_TEMPLATE[entity]
    label, _t, form = ENTITIES[entity]
    tpl = load_template(etype)
    rows = psql(f"SELECT id FROM companies WHERE slug ILIKE '%{entity}%' LIMIT 1;")
    if not rows:
        print(f"  {entity}: no company"); return
    cid = rows[0][0]
    accts = pull_accounts(cid)

    # classify each current account → category present in the books
    present = {}   # category -> [account names]
    flags = []     # (acct, name, flag)
    for num, name, typ in accts:
        c = C.classify(num, name, typ, 0.0, form)
        present.setdefault(c.category, []).append(f"{num} {name}")
        for fl in c.flags:
            flags.append((num, name, fl))

    # template categories that have a REAL dedicated tax line (worth a named account)
    tpl_named = {}
    for a in tpl["accounts"]:
        if a.get("tax_line") and not a.get("excluded"):
            tpl_named.setdefault(a["category"], a)
    missing = [(cat, a) for cat, a in tpl_named.items() if cat not in present]

    print(f"\n{'='*78}\n{label}  ·  {form}  ·  template: {etype}\n{'='*78}")
    covered = len(tpl_named) - len(missing)
    print(f"Coverage: {covered}/{len(tpl_named)} tax-lined categories have a dedicated account.")

    if missing:
        print(f"\n  MISSING NAMED ACCOUNTS ({len(missing)}) — add so transactions land on the right line:")
        for cat, a in sorted(missing, key=lambda x: x[1]["number"]):
            note = f"  — {a['note']}" if a.get("note") else ""
            print(f"    + {a['number']} {a['name']:<38} → line {a['line']} ({cat}){note}")

    if flags:
        # de-dup identical (acct, flag)
        seen = set(); uniq = []
        for num, name, fl in flags:
            k = (num, fl[:30])
            if k not in seen:
                seen.add(k); uniq.append((num, name, fl))
        print(f"\n  STRUCTURAL FLAGS ({len(uniq)}) — split / reclass / verify:")
        for num, name, fl in uniq:
            print(f"    ⚑ {num} {name[:26]:<26} {fl}")

    return {"entity": entity, "form": form, "template": etype,
            "coverage": [covered, len(tpl_named)],
            "missing": [{"number": a["number"], "name": a["name"], "line": a["line"],
                         "category": cat, "note": a.get("note")} for cat, a in missing],
            "flags": [{"account": n, "name": nm, "flag": f} for n, nm, f in flags]}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--entity", choices=list(ENTITY_TEMPLATE))
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--json", metavar="PATH", help="write the combined gap report as JSON")
    args = ap.parse_args()
    targets = ["maven", "hayat", "sweethome"] if args.all else [args.entity]
    if not targets or targets == [None]:
        sys.exit("pass --entity <name> or --all")
    out = [report(e) for e in targets]
    if args.json:
        with open(args.json, "w") as f:
            json.dump([o for o in out if o], f, indent=2)
        print(f"\n  gap JSON → {args.json}")


if __name__ == "__main__":
    main()
