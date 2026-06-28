#!/usr/bin/env python3
"""Bridge scaffold: accountir ledger -> proposed OpenTax return inputs.

Pulls one entity-year's P&L (grouped by GL account) from accountir_cloud and prints a
proposed account->node mapping for review. This is a STARTING POINT for the tax agent:
- fill in ACCOUNT_NODE_MAP per form (account_number -> opentax node_type + field),
- then emit real `opentax form add` calls (or write OpenTax's return JSON).

Usage:  python3 export_return.py --entity maven --year 2025
Source of truth: the books. Amounts in the DB are CENTS. Reconcile to the entity's book net.
"""
import argparse, json, subprocess, sys

DB = "accountir_cloud"
# slug fragment -> (label, entity_type, federal form). Slugs are like 'maven-8a7f105e'.
ENTITIES = {
    "maven":     ("MAVEN FINANCIAL TECHNOLOGIES INC", "c_corp",     "f1120"),
    "hayat":     ("Hayat Health LLC",                  "s_corp",     "f1120s"),
    "sweethome": ("SWEET HOME KC LLC",                 "s_corp",     "f1120s"),  # + 8825 rentals
    "on-chain":  ("On-Chain LLC",                      "s_corp",     "f1120s"),
    "michael":   ("Michael & Andrea Arbach",           "individual", "f1040"),
}

def psql(sql):
    r = subprocess.run(["sudo", "-u", "postgres", "psql", DB, "-tAF\t", "-c", sql],
                       capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"psql error: {r.stderr}")
    return [ln.split("\t") for ln in r.stdout.strip().splitlines() if ln]

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--entity", required=True, choices=list(ENTITIES))
    ap.add_argument("--year", type=int, default=2025)
    args = ap.parse_args()
    label, etype, form = ENTITIES[args.entity]
    y = args.year

    # resolve company_id by slug fragment
    rows = psql(f"SELECT id, slug FROM companies WHERE slug ILIKE '%{args.entity}%';")
    if not rows:
        sys.exit(f"no company matching '{args.entity}'")
    cid, slug = rows[0]

    # P&L grouped by GL account for the year
    pl = psql(f"""
        SELECT a.account_number, a.name, a.account_type,
               ROUND(SUM(jl.amount)/100.0, 2) AS net_cents_dollars
        FROM journal_entries je
        JOIN journal_lines jl ON jl.entry_id = je.id
        JOIN accounts a ON a.id = jl.account_id
        WHERE je.company_id = '{cid}' AND je.is_void = false
          AND a.account_type IN ('revenue','expense')
          AND je.date BETWEEN '{y}-01-01' AND '{y}-12-31'
        GROUP BY a.account_number, a.name, a.account_type
        ORDER BY a.account_type, a.account_number;""")

    income = [r for r in pl if r[2] == "revenue"]
    expense = [r for r in pl if r[2] == "expense"]
    # book net = -(sum of revenue+expense signed amounts); revenue is credit-normal (negative)
    net = -sum(float(r[3]) for r in pl)

    print(f"# {label}  ({slug})  TY{y}  -> federal form {form}  (entity_type={etype})")
    print(f"# Book net income {y} = ${net:,.2f}   <-- the computed return MUST reconcile to this\n")
    print("## REVENUE accounts -> map to income input nodes")
    for num, name, _t, amt in income:
        print(f"  {num:6} {name[:34]:34} ${-float(amt):>12,.2f}   -> node_type: TODO  field: TODO")
    print("\n## EXPENSE accounts -> map to deduction input nodes (split out officer comp)")
    for num, name, _t, amt in expense:
        print(f"  {num:6} {name[:34]:34} ${float(amt):>12,.2f}   -> node_type: TODO  field: TODO")

    print(f"\n# NEXT: discover node schemas with `deno task tax node inspect --node_type <t>`,")
    print(f"#       fill the account->node map (save as bridge/map_{form}.json),")
    print(f"#       then emit `opentax form add --returnId <id> --node_type <t> '{{...}}'` per group,")
    print(f"#       and assert the computed taxable income ties to ${net:,.2f}.")

if __name__ == "__main__":
    main()
