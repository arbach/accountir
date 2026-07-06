#!/usr/bin/env python3
"""sync_tax_lines.py — push the file tag store into public.tax_account_lines (Tier 1).

The git-versioned maps/tax_lines_<entity>.json files are the source of truth; this
upserts them into the DB table so the accountir app can render a per-account
"Tax line" column. Safe: writes only the tax-subsystem table, never the ledger.

Usage:
  python3 sync_tax_lines.py --init          # create the table (idempotent DDL)
  python3 sync_tax_lines.py --all           # sync every entity's tag file
  python3 sync_tax_lines.py --entity hayat
"""
import argparse, json, os, subprocess, sys

import tags as T
from export_return import psql, DB, ENTITIES
from tag_accounts import CORPORATE

HERE = os.path.dirname(os.path.abspath(__file__))
DDL = os.path.join(HERE, "sql", "tax_account_lines.sql")


def psql_exec(sql):
    r = subprocess.run(["sudo", "-u", "postgres", "psql", DB, "-v", "ON_ERROR_STOP=1", "-c", sql],
                       capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"psql error: {r.stderr}")
    return r.stdout


def init():
    with open(DDL) as f:
        ddl = f.read()
    # Pipe via stdin — the postgres OS user can't read files under /home/ubuntu.
    r = subprocess.run(["sudo", "-u", "postgres", "psql", DB, "-v", "ON_ERROR_STOP=1"],
                       input=ddl, capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"DDL error: {r.stderr}")
    print("  tax_account_lines table ready")


def q(v):
    if v is None:
        return "NULL"
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, (int, float)):
        return str(v)
    if isinstance(v, (list, dict)):
        return "'" + json.dumps(v).replace("'", "''") + "'::jsonb"
    return "'" + str(v).replace("'", "''") + "'"


def sync(entity):
    data = T.load_tags(entity)
    if not data:
        print(f"  {entity}: no tag file — run tag_accounts.py --entity {entity}")
        return 0
    rows = psql(f"SELECT id FROM companies WHERE slug ILIKE '%{entity}%' LIMIT 1;")
    if not rows:
        print(f"  {entity}: no company")
        return 0
    cid = rows[0][0]
    form = data.get("form", "")
    n = 0
    for acct, t in data["accounts"].items():
        vals = [
            q(cid), q(acct), q(t.get("name", "")), q(form), q(t.get("category")),
            q(t.get("node")), q(t.get("field")), q(t.get("line")), q(t.get("sign")),
            q(bool(t.get("sep"))), q(t.get("factor")), q(bool(t.get("excluded"))),
            q(t.get("splits")), q(t.get("confidence")), q(t.get("status", "auto")),
            q(t.get("flags", [])),
        ]
        sql = (
            "INSERT INTO public.tax_account_lines (company_id, account_number, account_name, "
            "form_code, category, node, field, line_label, sign, separately_stated, factor, "
            "excluded, splits, confidence, status, flags) VALUES (" + ", ".join(vals) + ") "
            "ON CONFLICT (company_id, account_number) DO UPDATE SET "
            "account_name=EXCLUDED.account_name, form_code=EXCLUDED.form_code, "
            "category=EXCLUDED.category, node=EXCLUDED.node, field=EXCLUDED.field, "
            "line_label=EXCLUDED.line_label, sign=EXCLUDED.sign, "
            "separately_stated=EXCLUDED.separately_stated, factor=EXCLUDED.factor, "
            "excluded=EXCLUDED.excluded, splits=EXCLUDED.splits, confidence=EXCLUDED.confidence, "
            "status=EXCLUDED.status, flags=EXCLUDED.flags, updated_at=now();"
        )
        psql_exec(sql)
        n += 1
    print(f"  {entity:<10} synced {n} account tags → tax_account_lines")
    return n


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--init", action="store_true")
    ap.add_argument("--entity", choices=list(ENTITIES))
    ap.add_argument("--all", action="store_true")
    args = ap.parse_args()
    if args.init:
        init()
    if args.all:
        for e in CORPORATE:
            sync(e)
    elif args.entity:
        sync(args.entity)
    elif not args.init:
        sys.exit("pass --init and/or --all | --entity <name>")


if __name__ == "__main__":
    main()
