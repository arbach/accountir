#!/usr/bin/env python3
"""build_tax_profiles.py — populate a comprehensive tax profile per entity.

Fills the extended tax_profiles columns from data we already hold (bridge map filer
blocks, AGENT_BRIEF, filed returns, the SweetHome revocation doc). Only asserts what
is known; unknown fields are LEFT NULL and surfaced in the completeness report so the
owner can fill them — nothing is invented.

Usage:
  python3 build_tax_profiles.py --init      # add the extended columns
  python3 build_tax_profiles.py --apply     # upsert known data
  python3 build_tax_profiles.py --report    # completeness report (what's missing)
"""
import argparse, json, os, subprocess, sys
from export_return import psql, DB

HERE = os.path.dirname(os.path.abspath(__file__))
DDL = os.path.join(HERE, "sql", "tax_profiles_extend.sql")

MICHAEL = {"name": "Michael Arbach", "tin": "813-14-0923", "title": "Managing Member / President",
           "ownership_pct": 100}

# Known entity data. `_gaps` lists fields we deliberately leave for the owner to confirm.
PROFILES = {
    "MAVEN": {
        "match": "%MAVEN%",
        "fields": {
            "fiscal_year_end": "12-31", "state_of_formation": "IL", "entity_status": "active",
            "business_activity": "Financial technology / software services",
            "product_or_service": "Technology & consulting services",
        },
        "officers_owners": [dict(MICHAEL, title="President")],
        "_gaps": ["date_formed (brief: ~2023-05, need exact day)", "naics_code",
                  "shares issued/outstanding", "confirm Michael's ownership after the 2025 stock sale"],
    },
    "HAYAT": {
        "match": "%HAYAT%",
        "fields": {
            "fiscal_year_end": "12-31", "date_formed": "2024-07-19", "state_of_formation": "IL",
            "s_election_effective": "2024-07-19", "entity_status": "active",
            "business_activity": "Medical / health consulting",
            "product_or_service": "Health consulting services",
        },
        "officers_owners": [dict(MICHAEL, title="Managing Member")],
        "_gaps": ["naics_code (suggest 621999 or 541611 — confirm)",
                  "confirm S-election effective date (assumed = formation 2024-07-19)"],
    },
    "SWEET HOME": {
        "match": "%SWEET HOME%",
        "fields": {
            "fiscal_year_end": "12-31", "state_of_formation": "IL",
            "s_election_effective": "2024-01-01", "entity_status": "revoking",
            "business_activity": "Residential rental real estate",
            "product_or_service": "Residential rental (3 KC-area properties)",
        },
        "officers_owners": [dict(MICHAEL, title="Managing Member", shares=100)],
        "_gaps": ["date_formed (brief: ~2023-08, need exact day)", "naics_code (suggest 531110 — confirm)",
                  "state_of_formation (IL vs MO — properties are in MO)",
                  "S-corp revocation effective 2027-01-01 (see tax/sweethome/) — set dissolved/revoked when filed"],
    },
    "ON-CHAIN": {
        "match": "%ON-CHAIN%",
        "fields": {
            "fiscal_year_end": "12-31", "state_of_formation": "IL",
            "entity_status": "dissolved", "dissolved_date": "2023-12-31",
            "business_activity": "Software / blockchain services",
        },
        "officers_owners": [dict(MICHAEL, title="Managing Member")],
        "_gaps": ["exact dissolution date (assumed 2023-12-31)", "naics_code",
                  "S-election effective date", "confirm final return year filed"],
    },
    "ARBACH": {  # individual (Michael & Andrea)
        "match": "%ARBACH%",
        "fields": {
            "fiscal_year_end": "12-31", "filing_status": "mfj", "entity_status": "active",
            "state_of_formation": "IL",
        },
        "spouse": {"first_name": "Andrea", "last_name": "Arbach", "ssn": "402-37-4451"},
        "dependents": [
            {"first_name": "Alexander", "last_name": "Arbach", "dob": "2019-01-01",
             "relationship": "son", "ctc": True},
            {"first_name": "Suria", "last_name": "Arbach", "dob": "2021-01-01",
             "relationship": "daughter", "ctc": True},
            {"first_name": "Palmyra", "last_name": "Arbach", "ssn": "154-29-4975",
             "dob": "2025-08-20", "relationship": "daughter", "ctc": True},
        ],
        "_gaps": ["SSNs for Alexander & Suria (required for CTC / Schedule 8812)",
                  "phone number"],
    },
}

# Fields a complete profile should carry, per entity kind.
REQUIRED_ENTITY = ["ein", "entity_type", "legal_name", "accounting_method", "fiscal_year_end",
                   "date_formed", "state_of_formation", "naics_code", "business_activity"]
REQUIRED_INDIVIDUAL = ["ein", "filing_status", "accounting_method"]


def psql_exec(sql):
    r = subprocess.run(["sudo", "-u", "postgres", "psql", DB, "-v", "ON_ERROR_STOP=1"],
                       input=sql, capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"psql error: {r.stderr}")
    return r.stdout


def init():
    with open(DDL) as f:
        psql_exec(f.read())
    print("  extended tax_profiles columns ready")


def qlit(v):
    if v is None:
        return "NULL"
    if isinstance(v, (list, dict)):
        return "'" + json.dumps(v).replace("'", "''") + "'::jsonb"
    return "'" + str(v).replace("'", "''") + "'"


def apply():
    for key, p in PROFILES.items():
        sets = [f"{k} = {qlit(v)}" for k, v in p["fields"].items()]
        if "officers_owners" in p:
            sets.append(f"officers_owners = {qlit(p['officers_owners'])}")
        if "dependents" in p:
            sets.append(f"dependents = {qlit(p['dependents'])}")
        if "spouse" in p:
            sets.append(f"spouse = {qlit(p['spouse'])}")
        sets.append("updated_at = now()")
        sql = f"UPDATE public.tax_profiles SET {', '.join(sets)} WHERE legal_name ILIKE '{p['match']}';"
        psql_exec(sql)
        print(f"  {key:<12} profile updated")


def report():
    cols = ("legal_name, entity_type, ein, accounting_method, fiscal_year_end, date_formed, "
            "state_of_formation, naics_code, business_activity, s_election_effective, entity_status, "
            "filing_status, jsonb_array_length(officers_owners) AS owners, "
            "jsonb_array_length(dependents) AS deps")
    rows = psql(f"SELECT {cols} FROM public.tax_profiles ORDER BY entity_type, legal_name;")
    hdr = [c.strip().split(" AS ")[-1] for c in cols.split(",")]
    print("\nTAX PROFILE COMPLETENESS\n" + "=" * 70)
    for r in rows:
        d = dict(zip(hdr, r))
        name = d["legal_name"]
        indiv = d["entity_type"] == "individual"
        req = REQUIRED_INDIVIDUAL if indiv else REQUIRED_ENTITY
        missing = [f for f in req if not d.get(f) or d.get(f) in ("", "0")]
        status = "COMPLETE" if not missing else f"{len(missing)} missing"
        print(f"\n{name}  [{d['entity_type']}]  — {status}")
        print(f"  EIN {d['ein'] or '—'} · method {d['accounting_method'] or '—'} · "
              f"FYE {d['fiscal_year_end'] or '—'} · formed {d['date_formed'] or '—'} · "
              f"{d['state_of_formation'] or '—'} · NAICS {d['naics_code'] or '—'}")
        print(f"  activity: {d['business_activity'] or '—'}"
              + (f" · S-elec {d['s_election_effective']}" if d.get('s_election_effective') else "")
              + f" · status {d['entity_status'] or '—'}")
        if indiv:
            print(f"  filing {d['filing_status'] or '—'} · {d['deps']} dependents")
        else:
            print(f"  {d['owners']} officer(s)/owner(s)")
        if missing:
            print(f"  MISSING (required): {', '.join(missing)}")
        # surface curated gaps
        for k, p in PROFILES.items():
            if name.upper().find(p["match"].strip("%").upper()) >= 0:
                for g in p.get("_gaps", []):
                    print(f"    · confirm: {g}")
                break


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--init", action="store_true")
    ap.add_argument("--apply", action="store_true")
    ap.add_argument("--report", action="store_true")
    a = ap.parse_args()
    if not (a.init or a.apply or a.report):
        a.init = a.apply = a.report = True
    if a.init: init()
    if a.apply: apply()
    if a.report: report()


if __name__ == "__main__":
    main()
