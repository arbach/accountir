#!/usr/bin/env python3
"""Bridge: accountir ledger -> OpenTax return, computed + reconciled.

Map-driven. For one entity-year it:
  1. loads the checked-in account->node map (bridge/maps/<form>_<year>_<entity>.json),
  2. pulls that entity-year's P&L (grouped by GL account) from accountir_cloud (CENTS -> dollars),
  3. builds an OpenTax input bundle {year, forms:[{node_type, data}]} by applying the map
     (one form entry per non-zero mapped account + the static filer/identity block),
  4. writes the bundle to bridge/out/<entity>_<year>_input.json,
  5. with --compute: drives the OpenTax CLI (return create / form add / return get) and
     reconciles the computed total income against the book net, printing any delta.

The books are the source of truth. Every figure is pulled live; nothing is re-keyed.
A return that doesn't reconcile to the book net is a bug -> the delta is surfaced, not hidden.

Usage:
  python3 export_return.py --entity michael --year 2025            # emit bundle only
  python3 export_return.py --entity michael --year 2025 --compute  # emit + compute + reconcile
"""
import argparse, json, os, subprocess, sys

DB = "accountir_cloud"
HERE = os.path.dirname(os.path.abspath(__file__))
MAPS_DIR = os.path.join(HERE, "maps")
# Output dir is env-configurable so the bridge can run from a read-only install
# (e.g. the app service writes to a data dir it owns).
OUT_DIR = os.environ.get("BRIDGE_OUT", os.path.join(HERE, "out"))
OPENTAX = os.path.join(HERE, "..", "opentax", "cli", "main.ts")

# slug fragment -> (label, entity_type, federal form)
ENTITIES = {
    "maven":     ("MAVEN FINANCIAL TECHNOLOGIES INC", "c_corp",     "f1120"),
    "hayat":     ("Hayat Health LLC",                 "s_corp",     "f1120s"),
    "sweethome": ("SWEET HOME KC LLC",                "s_corp",     "f1120s"),
    "on-chain":  ("On-Chain LLC",                     "s_corp",     "f1120s"),
    "michael":   ("Michael & Andrea Arbach",          "individual", "f1040"),
}

# Illinois state form per entity, and how to feed the federal base into it.
# (state_form, [(il_field, federal_node, federal_field), ...], il_tax_field)
STATE_IL = {
    "f1040":  ("il1040",   [("federal_agi", "f1040", "line11_agi")], "il_tax"),
    "f1120":  ("il1120",   [("federal_taxable_income", "f1120", "line28_income_before_nol")], "total_il_tax"),
    "f1120s": ("il1120st", [("federal_ordinary_income", "f1120s", "line21_ordinary_business_income"),
                            ("federal_net_rental", "schedule_k", "line2_net_rental_real_estate")], "replacement_tax"),
}
# IL personal exemption count for Michael (taxpayer + spouse + dependents) by year.
IL_EXEMPTION_COUNT = {2023: 4, 2024: 4, 2025: 5}

# How to reconcile each form against the entity book net. For f1040/f1120 a single
# computed line is the target. For f1120s the book net is split across Schedule K income
# items (ordinary on the f1120s node + net rental/interest/dividends on schedule_k), so we
# SUM those components — an S-corp's total Schedule K income must equal its book net.
RECON_SINGLE = {
    "f1040": ("f1040", "line9_total_income"),
    "f1120": ("f1120", "line28_income_before_nol"),
}
RECON_SUM = {
    # form: list of (node, field) whose sum must equal the book net. Missing nodes/fields
    # count as 0, so this is safe to extend as more Schedule K income items appear.
    "f1120s": [
        ("f1120s", "line21_ordinary_business_income"),
        ("schedule_k", "line2_net_rental_real_estate"),
    ],
}


# When running via DATABASE_URL as the (non-superuser, RLS-enforced) app role, every
# tenant-scoped query must run with app.company_id set or RLS returns nothing. The
# `sudo -u postgres` path is superuser and bypasses RLS, so no tenant is needed there.
_TENANT = None


def set_db_tenant(cid):
    """Scope subsequent DATABASE_URL queries to one company (for RLS)."""
    global _TENANT
    _TENANT = cid


def psql(sql):
    # Portable DB access: prefer DATABASE_URL (so the bridge runs as any user — e.g.
    # the accountir-cloud app service). Fall back to `sudo -u postgres` for local/dev.
    url = os.environ.get("DATABASE_URL")
    env = None
    if url:
        cmd = ["psql", url, "-tAF\t", "-c", sql]
        if _TENANT:
            # Set the tenant GUC at connection time — no in-band SET statement, so
            # nothing pollutes the tuple output.
            env = {**os.environ, "PGOPTIONS": f"-c app.company_id={_TENANT}"}
    else:
        cmd = ["sudo", "-u", "postgres", "psql", DB, "-tAF\t", "-c", sql]
    r = subprocess.run(cmd, capture_output=True, text=True, env=env)
    if r.returncode != 0:
        sys.exit(f"psql error: {r.stderr}")
    return [ln.split("\t") for ln in r.stdout.strip().splitlines() if ln]


def deno(*args):
    # Use a deno on PATH (system install) if present, else the user-local install.
    # DENO_DIR must be writable by the running user; default to a per-process temp so
    # the app service (whose $HOME may be non-writable) can still cache.
    local_bin = os.path.expanduser("~/.deno/bin")
    path = os.environ.get("PATH", "")
    if os.path.isdir(local_bin):
        path = local_bin + ":" + path
    env = {**os.environ, "PATH": path}
    env.setdefault("DENO_DIR", os.environ.get("DENO_DIR", "/tmp/.deno-cache"))
    # The engine writes its return state to .state/ relative to cwd — run it from a
    # writable output dir so it works from a read-only install (app-service user).
    os.makedirs(OUT_DIR, exist_ok=True)
    r = subprocess.run(
        ["deno", "run", "--allow-read", "--allow-write", "--allow-run", "--allow-env", OPENTAX, *args],
        capture_output=True, text=True, env=env, cwd=OUT_DIR,
    )
    if r.returncode != 0:
        sys.exit(f"opentax error ({' '.join(args[:3])}): {r.stderr}\n{r.stdout}")
    return r.stdout


def load_map(form, year, entity):
    # Prefer a year-specific map; fall back to a year-agnostic one (corporate
    # account->line mappings are stable across years, so one map serves all years).
    for cand in (f"{form}_{year}_{entity}.json", f"{form}_{entity}.json"):
        path = os.path.join(MAPS_DIR, cand)
        if os.path.exists(path):
            with open(path) as f:
                return json.load(f), path
    sys.exit(f"no map for {entity} {form} {year} — create maps/{form}_{entity}.json "
             f"(or maps/{form}_{year}_{entity}.json)")


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


def build_bundle(themap, pl, year):
    """Apply the map to the P&L -> OpenTax input bundle. Returns (bundle, unmapped, book_net).

    Each mapped account contributes (node_type, field, value) to a form entry. Accounts that
    share an "item_group" merge into ONE entry (fields summed) — needed for singleton corporate
    forms (one 1120-S, fields accumulated) and for Form 8825 (one item per property, which
    REQUIRES gross_rents + its expenses together). Without item_group each account is its own
    entry (the default, used by 1040 K-1/1099 array nodes where each account is a distinct item).
    "abs": true passes the magnitude (corporate deduction / 8825 expense lines are nonnegative);
    1040 passthrough loss accounts stay signed.
    """
    unmapped = []
    book_net = round(sum(amt for _, _, _, amt in pl), 2)
    groups = {}   # (node_type, group_key) -> {"data": {...}, "order": int}
    order = 0
    for acct_no, name, _atype, amt in pl:
        if abs(amt) < 0.005:
            continue  # skip zero accounts
        m = themap["accounts"].get(acct_no)
        if not m:
            unmapped.append((acct_no, name, amt))
            continue
        value = abs(amt) if m.get("abs") else amt
        gkey = m.get("item_group", f"__{acct_no}")  # default: one entry per account
        key = (m["node_type"], gkey)
        if key not in groups:
            groups[key] = {"data": dict(m.get("static", {})), "order": order}
            order += 1
        data = groups[key]["data"]
        data.update(m.get("static", {}))
        data[m["field"]] = round(data.get(m["field"], 0) + value, 2)

    forms = [themap["filer"]]  # static identity block (general / start)
    for (node_type, _gkey), g in sorted(groups.items(), key=lambda kv: kv[1]["order"]):
        forms.append({"node_type": node_type, "data": g["data"]})
    return {"year": year, "forms": forms}, unmapped, book_net


def _scalar(v):
    # We always read a FORM-OUTPUT node's assembled line (f1040 line9, f1120 line28,
    # f1120s line21, schedule_k line2). Such a node runs after its inputs and re-deposits
    # its computed value LAST into its own pending slot; when it both receives and re-emits
    # the same key (e.g. schedule_k passes f8825's line2 through), the slot is an array whose
    # LAST element is the node's authoritative output. Take that, don't sum (summing would
    # double-count the self-pass-through).
    if isinstance(v, list):
        nums = [x for x in v if isinstance(x, (int, float))]
        return nums[-1] if nums else 0.0
    return v if isinstance(v, (int, float)) else 0.0


def compute_and_reconcile(bundle, form, book_net):
    create_args = ["return", "create", "--year", str(bundle["year"]), "--json"]
    if form != "f1040":
        create_args += ["--form", form]
    rid = json.loads(deno(*create_args))["returnId"]
    print(f"  return: {rid}")
    for f in bundle["forms"]:
        deno("form", "add", "--returnId", rid, "--node_type", f["node_type"], json.dumps(f["data"]), "--json")
    result = json.loads(deno("return", "get", "--returnId", rid, "--json"))
    pending = result.get("pending", {})

    print(f"\n  RECONCILIATION ({form})")
    print(f"    book net          : {book_net:>14,.2f}")
    if form in RECON_SUM:
        computed = 0.0
        for node, field in RECON_SUM[form]:
            part = _scalar(pending.get(node, {}).get(field))
            if part:
                print(f"    {node}.{field:<28}: {part:>14,.2f}")
            computed += part
    else:
        node, field = RECON_SINGLE[form]
        computed = _scalar(pending.get(node, {}).get(field))
        print(f"    computed {node}.{field:<20}: {computed:>14,.2f}")
    delta = round(computed - book_net, 2)
    flag = "OK (ties)" if abs(delta) < 0.5 else "DELTA -> review"
    print(f"    {'computed total':<18}: {computed:>14,.2f}")
    print(f"    delta             : {delta:>14,.2f}   {flag}")

    warns = [w for w in result.get("warnings", []) if "EXECUTOR_NODE_FAILURE" in w or "8949" in w]
    if warns:
        print(f"\n  {len(warns)} engine warning(s):")
        for w in warns[:10]:
            print(f"    - {w.splitlines()[0][:140]}")
    return result


def compute_state_il(entity, year, form, fed_pending):
    """Chain the federal result into the matching Illinois form and print IL tax."""
    state_form, feeds, tax_field = STATE_IL[form]
    data = {}
    for il_field, fed_node, fed_field in feeds:
        data[il_field] = round(_scalar(fed_pending.get(fed_node, {}).get(fed_field)), 2)
    if state_form == "il1040":
        data["exemption_count"] = IL_EXEMPTION_COUNT.get(year, 0)
    rid = json.loads(deno("return", "create", "--form", state_form, "--year", str(year), "--json"))["returnId"]
    deno("form", "add", "--returnId", rid, "--node_type", state_form, json.dumps(data), "--json")
    res = json.loads(deno("return", "get", "--returnId", rid, "--json"))
    node = res.get("pending", {}).get(state_form, {})
    il_tax = _scalar(node.get(tax_field))
    il_net = _scalar(node.get("il_net_income"))
    print(f"\n  ILLINOIS ({state_form})")
    for il_field, _n, _f in feeds:
        print(f"    {il_field:<26}: {data[il_field]:>14,.2f}")
    if state_form == "il1040":
        print(f"    exemptions ({data['exemption_count']} x)         : {_scalar(node.get('total_exemptions')):>14,.2f}")
    print(f"    IL net income             : {il_net:>14,.2f}")
    print(f"    {tax_field:<26}: {il_tax:>14,.2f}")
    return res


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--entity", required=True, choices=list(ENTITIES))
    ap.add_argument("--year", type=int, default=2025)
    ap.add_argument("--compute", action="store_true", help="compute via OpenTax and reconcile")
    ap.add_argument("--state", action="store_true", help="also chain the federal result into the IL state form")
    ap.add_argument("--fill", action="store_true",
                    help="write the full computed line set (engine output) to out/<entity>_<year>_fill.json")
    args = ap.parse_args()
    label, etype, form = ENTITIES[args.entity]

    rows = psql(f"SELECT id, slug FROM companies WHERE slug ILIKE '%{args.entity}%';")
    if not rows:
        sys.exit(f"no company matching '{args.entity}'")
    cid, slug = rows[0]
    set_db_tenant(cid)   # scope RLS to this company (DATABASE_URL / app-role runs)

    themap, map_path = load_map(form, args.year, args.entity)

    if themap.get("manual"):
        # Source-grounded return: inputs come from cited source docs (1099s/K-1s/filed return),
        # NOT the ledger (used when the books are unreliable). Reconcile to a stated target.
        bundle = {"year": args.year, "forms": [themap["filer"]] + themap["manual_forms"]}
        unmapped = []
        book_net = float(themap.get("reconcile_target", 0.0))
    else:
        pl = pull_pl(cid, args.year)
        if not pl:
            sys.exit(f"no {args.year} ledger data for {label} — cannot build a return (books required).")
        bundle, unmapped, book_net = build_bundle(themap, pl, args.year)
    os.makedirs(OUT_DIR, exist_ok=True)
    out_path = os.path.join(OUT_DIR, f"{args.entity}_{args.year}_input.json")
    with open(out_path, "w") as f:
        json.dump(bundle, f, indent=2)

    print(f"Entity   : {label} ({etype})  slug={slug}")
    print(f"Form     : {form}  Year: {args.year}")
    print(f"Map      : {os.path.relpath(map_path, HERE)}")
    print(f"Book net : {book_net:,.2f}")
    print(f"Bundle   : {os.path.relpath(out_path, HERE)}  ({len(bundle['forms'])} form entries)")
    if unmapped:
        print(f"\n  UNMAPPED non-zero accounts (add to the map):")
        for acct_no, name, amt in unmapped:
            print(f"    {acct_no}  {name:<40} {amt:>14,.2f}")

    if args.compute or args.state or args.fill:
        result = compute_and_reconcile(bundle, form, book_net)
        if args.state:
            compute_state_il(args.entity, args.year, form, result.get("pending", {}))
        if args.fill:
            pend = result.get("pending", {})
            if form in RECON_SUM:
                computed = round(sum(_scalar(pend.get(n, {}).get(f)) for n, f in RECON_SUM[form]), 2)
            else:
                n, f = RECON_SINGLE[form]
                computed = round(_scalar(pend.get(n, {}).get(f)), 2)
            delta = round(computed - book_net, 2)
            lines = {}
            for node, flds in pend.items():
                if not isinstance(flds, dict):
                    continue
                for fld, v in flds.items():
                    v = v[-1] if isinstance(v, list) else v
                    if isinstance(v, (int, float)):
                        lines[f"{node}.{fld}"] = round(float(v), 2)
            fill = {"entity": args.entity, "form": form, "year": args.year,
                    "reconciles": abs(delta) < 0.5, "delta": delta,
                    "book_net": book_net, "computed": computed, "lines": dict(sorted(lines.items()))}
            with open(os.path.join(OUT_DIR, f"{args.entity}_{args.year}_fill.json"), "w") as f2:
                json.dump(fill, f2, indent=2)
            print(f"  engine fill output ({len(lines)} lines) → {args.entity}_{args.year}_fill.json")


if __name__ == "__main__":
    main()
