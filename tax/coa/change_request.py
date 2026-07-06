#!/usr/bin/env python3
"""change_request.py — generate the bookkeeping change request (Tier 2).

Reads the Tier-3 gap report (coa_gap.py --json) and turns it into a reviewable
CHANGE_REQUEST.md: the accounts to ADD and the accounts to SPLIT / RECLASSIFY so
the books map cleanly to the tax forms. Structural fixes carry curated remediation
text keyed off the classifier flag, and each is marked whether it needs a source
document (so nothing is guessed).

These are BOOKKEEPING-SIDE operations: accountir is event-sourced with a
merkle/owner-signature audit chain, so they must be applied through the app, never
by writing accounts/journal_lines directly. The tax engine already represents the
intended result via split tags (maps/tax_lines_<entity>.json), so returns are
correct before the physical books are restructured.

Usage:
  python3 coa_gap.py --all --json gap.json
  python3 change_request.py --gap gap.json --out CHANGE_REQUEST.md
"""
import argparse, json, os

# Remediation keyed by a substring of the classifier flag. (title, action, needs_doc)
REMEDIATION = [
    ("contractor vs officer", (
        "Split into Officer Compensation vs Contract Labor",
        "Create a dedicated **Officer Compensation** account and move the owner's own pay there "
        "(it becomes W-2 wages on line 7 and must run through payroll). Leave only true third-party "
        "1099 contractors in Contract Labor. Tax total is unchanged; the *character* changes and "
        "S-corp reasonable-comp compliance is satisfied.", True)),
    ("mortgage P&I", (
        "Split Mortgage Payment into Interest vs Principal",
        "Principal is NOT a P&L expense — it pays down the loan liability on the balance sheet. "
        "Split each payment: interest → the deductible interest account (8825 line 9); principal → "
        "the mortgage loan liability. Pull the amortization schedule to get the split per period.", True)),
    ("contra-revenue", (
        "Confirm gross vs net revenue (platform/billing fees)",
        "If the platform withholds its fee before depositing, the booked revenue is NET. Either gross "
        "up revenue and keep the fee as an expense, or book the fee as contra-revenue — but do not do "
        "both (double-count). This likely flips the entity from a loss to a profit.", True)),
    ("uncategorized/misc", (
        "Break out the uncategorized / miscellaneous lump",
        "A catch-all account can't be placed on a return with confidence. Re-code its transactions to "
        "the specific expense accounts so each lands on its proper line.", True)),
    ("meals", (
        "Keep Meals in a dedicated 50% account",
        "Book meals to a single Meals (50%) account so the §274(n) limit applies mechanically and the "
        "disallowed half is a clean M-1 add-back. Split off any 100%-deductible items (e.g. company "
        "events) into their own account.", False)),
    ("FX/revaluation", (
        "Confirm treatment of FX / revaluation accounts",
        "Unrealized currency revaluations are usually book-only and non-deductible (an M-1 item). "
        "Confirm which portion is a realized gain/loss before it touches the return.", True)),
    ("charitable", (
        "Route charitable to a separately-stated account",
        "For an S-corp, charitable contributions are separately stated on Schedule K (they don't reduce "
        "ordinary income); for a C-corp they're limited to 10% of taxable income. Give them their own account.", False)),
    ("cash-basis prior-year", (
        "Confirm cash vs accrual on prior-year collections",
        "Revenue collected this year for prior-year work is current-year gross receipts on the cash basis. "
        "Confirm the entity's method so the timing is right.", True)),
    ("UNMATCHED", (
        "Assign a tax line to this account",
        "The classifier has no rule for this account name. Give it a clearer name or assign its tax line "
        "in the mapper so it never blocks a filing.", False)),
]


def remediate(flag):
    for key, val in REMEDIATION:
        if key.lower() in flag.lower():
            return val
    return ("Review classification", flag, True)


def render(entities):
    out = []
    out.append("# Chart-of-Accounts Change Request — tax-alignment\n")
    out.append("> Generated from `coa_gap.py`. These are **bookkeeping-side** changes. accountir is "
               "event-sourced with a cryptographic audit chain, so apply them **through the app**, not by "
               "writing `accounts`/`journal_lines` directly. Items marked **needs source doc** must not be "
               "guessed — pull the document first. The tax engine already reflects the intended result via "
               "split tags, so the 2025 returns are correct before the books are physically restructured.\n")

    # summary
    tot_add = sum(len(e["missing"]) for e in entities)
    tot_fix = sum(len({(f["account"], remediate(f["flag"])[0]) for f in e["flags"]}) for e in entities)
    out.append(f"**Summary:** {tot_add} accounts to add · {tot_fix} structural fixes across "
               f"{len(entities)} entities.\n")

    for e in entities:
        cov = e["coverage"]
        out.append(f"\n## {e['entity'].title()} — {e['form']} (template: {e['template']})\n")
        out.append(f"Coverage: **{cov[0]}/{cov[1]}** tax-lined categories have a dedicated account.\n")

        if e["missing"]:
            out.append("\n### Accounts to add\n")
            out.append("| New # | Account | Tax line | Why |")
            out.append("|---|---|---|---|")
            for m in sorted(e["missing"], key=lambda x: x["number"]):
                why = m.get("note") or f"so {m['category'].replace('_',' ')} transactions land on line {m['line']}"
                out.append(f"| {m['number']} | {m['name']} | {m['line']} | {why} |")

        # de-dup structural fixes by (account, remediation title)
        fixes, seen = [], set()
        for f in e["flags"]:
            title, action, needs = remediate(f["flag"])
            k = (f["account"], title)
            if k in seen:
                continue
            seen.add(k)
            fixes.append((f["account"], f["name"], title, action, needs))
        if fixes:
            out.append("\n### Structural fixes (split / reclassify / verify)\n")
            for acct, name, title, action, needs in sorted(fixes):
                tag = " **[needs source doc]**" if needs else ""
                out.append(f"- **{acct} {name} — {title}**{tag}\n  {action}")

    out.append("\n\n---\n_After the books are updated, re-run `tag_accounts.py --all` and `sync_tax_lines.py "
               "--all`; confirmed tags carry forward and only the changed accounts refresh._\n")
    return "\n".join(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--gap", required=True, help="gap report JSON from coa_gap.py --json")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()
    with open(args.gap) as f:
        entities = json.load(f)
    md = render(entities)
    with open(args.out, "w") as f:
        f.write(md)
    print(f"change request → {args.out}  ({len(entities)} entities)")


if __name__ == "__main__":
    main()
