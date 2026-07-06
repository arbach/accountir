#!/usr/bin/env python3
"""tags.py — the tax-line tag store (Tier 1).

A persistent, git-versioned per-account tax-line assignment — accountir's
equivalent of QuickBooks' "tax-line mapping". Files live at
maps/tax_lines_<entity>.json and are the AUTHORITATIVE source the return uses;
the classifier's rules (classify.py) are only the generator that proposes the
initial tags. Once a human confirms or changes a tag (via the step-4 mapper),
that decision is preserved across regenerations and carries forward every year.

Each account tag records where the account's book amount goes on the tax form:
  { node, field, line, sign, sep, factor, excluded, splits, category,
    confidence, status, flags, notes }
  status: "auto"      — proposed by the classifier, not yet reviewed
          "confirmed" — a human reviewed and accepted the auto proposal
          "override"  — a human changed the mapping
  splits: null, or [{node, field, line, amount|pct, sign}] — one book account
          split across several tax lines (e.g. Contractor → officer comp + 1099).

Non-destructive: tagging never touches the event-sourced ledger. It's tax-side
metadata, mirroring the plain tax_profiles / tax_forms tables.
"""
import json, os
from typing import Optional

HERE = os.path.dirname(os.path.abspath(__file__))
MAPS_DIR = os.path.join(HERE, "maps")

HUMAN_STATUSES = {"confirmed", "override"}


def tag_path(entity: str) -> str:
    return os.path.join(MAPS_DIR, f"tax_lines_{entity}.json")


def load_tags(entity: str) -> Optional[dict]:
    p = tag_path(entity)
    if not os.path.exists(p):
        return None
    with open(p) as f:
        return json.load(f)


def save_tags(entity: str, data: dict) -> str:
    os.makedirs(MAPS_DIR, exist_ok=True)
    p = tag_path(entity)
    with open(p, "w") as f:
        json.dump(data, f, indent=2)
    return p


def classification_to_tag(c) -> dict:
    """Freeze a classify.Classification into an 'auto' tag record."""
    return {
        "name": c.account_name,
        "category": c.category,
        "node": c.node,
        "field": c.field,
        "line": c.line_label,
        "sign": c.sign,
        "sep": c.sep,
        "factor": c.factor,
        "excluded": False,
        "splits": None,
        "confidence": c.confidence,
        "status": "auto",
        "flags": list(c.flags),
        "notes": list(c.notes),
    }


def merge(existing: Optional[dict], fresh: dict) -> dict:
    """Merge freshly-generated auto tags with an existing file. Human decisions
    (status confirmed/override) are preserved verbatim; auto entries are refreshed
    with the latest classifier output; brand-new accounts are added as auto;
    accounts that vanished from the books are dropped."""
    if not existing:
        return fresh
    out = dict(fresh)  # entity/form/version headers from fresh
    merged_accounts = {}
    ex_acc = existing.get("accounts", {})
    for acct, tag in fresh["accounts"].items():
        prior = ex_acc.get(acct)
        if prior and prior.get("status") in HUMAN_STATUSES:
            merged_accounts[acct] = prior            # keep human decision
        else:
            merged_accounts[acct] = tag              # refresh auto
    out["accounts"] = merged_accounts
    return out


def as_overrides(tags: Optional[dict]) -> dict:
    """Expose the tag store to classify.classify() as its `overrides` dict, so a
    tagged account is mapped deterministically (rules are the fallback only)."""
    if not tags:
        return {}
    ov = {}
    for acct, t in tags.get("accounts", {}).items():
        o = {}
        for k in ("category", "node", "field", "sign", "sep", "factor"):
            if t.get(k) is not None:
                o[k] = t[k]
        if t.get("line"):
            o["line_label"] = t["line"]
        ov[acct] = o
    return ov
