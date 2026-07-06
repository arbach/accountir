#!/usr/bin/env python3
"""tag_accounts.py — generate/refresh the tax-line tag store (Tier 1).

Runs the classifier over an entity's live chart of accounts and writes
maps/tax_lines_<entity>.json. Re-running is safe: human-confirmed / overridden
tags are preserved (see tags.merge); only 'auto' tags and new accounts refresh.

Usage:
  python3 tag_accounts.py --entity hayat            # one entity
  python3 tag_accounts.py --all                     # all corporate entities
"""
import argparse, sys

import classify as C
import tags as T
from export_return import psql, ENTITIES
from step4 import pull_pl, classify_form

CORPORATE = ["maven", "hayat", "sweethome", "michael"]   # all taggable entities (incl. 1040 individual)


def generate(entity: str, year: int) -> dict:
    label, etype, form = ENTITIES[entity]
    rows = psql(f"SELECT id FROM companies WHERE slug ILIKE '%{entity}%' LIMIT 1;")
    if not rows:
        sys.exit(f"no company matching '{entity}'")
    cid = rows[0][0]
    pl = pull_pl(cid, year)
    if not pl:
        sys.exit(f"no {year} ledger for {entity}")

    cform = classify_form(entity, form)   # rentals classify against the 8825 columns
    fresh = {"entity": entity, "form": form, "version": 1, "source_year": year, "accounts": {}}
    for num, name, typ, amt in pl:
        if abs(amt) < 0.005:
            continue
        c = C.classify(num, name, typ, amt, cform)
        fresh["accounts"][num] = T.classification_to_tag(c)

    merged = T.merge(T.load_tags(entity), fresh)
    path = T.save_tags(entity, merged)

    accts = merged["accounts"]
    auto = sum(1 for t in accts.values() if t["status"] == "auto")
    human = sum(1 for t in accts.values() if t["status"] in T.HUMAN_STATUSES)
    flagged = sum(1 for t in accts.values() if t["flags"])
    unmatched = sum(1 for t in accts.values() if t["node"] is None)
    print(f"  {entity:<10} {form:<7} {len(accts):>2} accounts  "
          f"({auto} auto, {human} human-set, {flagged} flagged, {unmatched} unmatched)  → {path.split('/')[-1]}")
    return merged


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--entity", choices=list(ENTITIES))
    ap.add_argument("--all", action="store_true")
    ap.add_argument("--year", type=int, default=2025)
    args = ap.parse_args()
    if not args.entity and not args.all:
        sys.exit("pass --entity <name> or --all")
    targets = CORPORATE if args.all else [args.entity]
    print(f"Generating tax-line tags (year {args.year}):")
    for e in targets:
        generate(e, args.year)


if __name__ == "__main__":
    main()
