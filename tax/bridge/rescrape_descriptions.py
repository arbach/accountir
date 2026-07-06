#!/usr/bin/env python3
"""Recover exact source descriptions from statement PDFs and write them back to
entry_sources.source_detail. The importer truncated descriptions (e.g. "Pro LLC"
instead of "TRAVEL VISA PRO, LLC 415-229-3210 TX"); the full text is in the stored
statement PDFs. Match each parsed statement line to its journal entry by
(source_file, amount, date) and update source_detail. Idempotent; only UPDATEs
entry_sources.source_detail — never touches the ledger.

Usage: python3 rescrape_descriptions.py [--dry-run] [--limit N]
"""
import subprocess, re, sys
from collections import defaultdict

DRY = "--dry-run" in sys.argv
LIMIT = None
if "--limit" in sys.argv:
    LIMIT = int(sys.argv[sys.argv.index("--limit") + 1])


def psql(q, rw=False):
    args = ["sudo", "-u", "postgres", "psql", "accountir_cloud", "-tAF\t", "-c", q]
    return subprocess.run(args, capture_output=True, text=True).stdout


# A statement transaction line in -layout output: date, description, trailing amount.
LINE = re.compile(r"^\s*(\d{1,2}/\d{1,2})\s+(.+?)\s{2,}\$?(-?[\d,]+\.\d{2})-?\s*$")


def parse_pdf(path):
    txt = subprocess.run(["sudo", "pdftotext", "-layout", path, "-"],
                         capture_output=True, text=True).stdout
    out = []
    for ln in txt.split("\n"):
        m = LINE.match(ln)
        if not m:
            continue
        mmdd, desc, amt = m.group(1), m.group(2).strip(), m.group(3).replace(",", "")
        desc = re.sub(r"\s{2,}", " ", desc).strip()
        if len(desc) < 3 or not re.search(r"[A-Za-z]", desc):
            continue
        try:
            cents = abs(int(round(float(amt) * 100)))
        except ValueError:
            continue
        out.append((mmdd, desc, cents))
    return out


files = psql("""SELECT id, company_id, filename, stored_path FROM company_files
                WHERE content_type='application/pdf' AND category IN ('statement','bank_statement')
                ORDER BY filename;""").strip().split("\n")

tot_files = tot_matched = tot_lines = 0
for i, row in enumerate(files):
    if not row:
        continue
    if LIMIT and tot_files >= LIMIT:
        break
    fid, cid, fname, spath = row.split("\t")
    slines = parse_pdf(spath)
    if not slines:
        continue
    tot_lines += len(slines)
    # entries tied to this statement file, with their transaction amount (largest line)
    ents = psql(f"""SELECT es.entry_id, je.date::text, MAX(ABS(jl.amount))
                    FROM entry_sources es JOIN journal_entries je ON je.id=es.entry_id
                    JOIN journal_lines jl ON jl.entry_id=es.entry_id
                    WHERE es.company_id='{cid}' AND es.source_file=$${fname}$$
                    GROUP BY es.entry_id, je.date;""")
    by_amt = defaultdict(list)
    for er in ents.strip().split("\n"):
        if not er:
            continue
        eid, edate, eamt = er.split("\t")
        by_amt[int(eamt)].append((eid, edate))
    if not by_amt:
        continue
    updates, used = [], set()
    for mmdd, desc, cents in slines:
        cands = [c for c in by_amt.get(cents, []) if c[0] not in used]
        if not cands:
            continue
        mm, dd = mmdd.split("/")
        pick = next((c for c in cands if c[1][5:7] == mm.zfill(2) and c[1][8:10] == dd.zfill(2)), None)
        if pick is None and len(cands) == 1:
            pick = cands[0]
        if pick:
            used.add(pick[0])
            updates.append((pick[0], desc))
    if updates:
        tot_matched += len(updates)
        tot_files += 1
        if not DRY:
            stmts = "".join(
                f"UPDATE entry_sources SET source_detail=$${d[:250]}$$ WHERE entry_id='{eid}';"
                for eid, d in updates
            )
            psql(stmts, rw=True)
    if (i + 1) % 25 == 0:
        print(f"  ...{i+1} files, {tot_matched} descriptions recovered")

print(f"\n{'DRY-RUN: ' if DRY else ''}re-scrape done: {tot_matched} descriptions recovered "
      f"across {tot_files} statements ({tot_lines} statement lines parsed)")
