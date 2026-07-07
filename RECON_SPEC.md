# accountir-recon — Transaction Reconciliation & Consistency Auditor

**Status:** Spec (v1 design). **Owner:** aarbache. **Last updated:** 2026-07-06.

A standalone, **read-only, multi-tenant** auditor that verifies the *sanity and consistency*
of transactions in `accountir_cloud` against their **source documents** (statement files +
blockchain scanners). It **does not classify and does not fix** — it emits findings for a
human or another agent to act on.

> **Built.** Lives in its own repo `/home/ubuntu/repos/accountir-recon` (Python, stdlib-only).
> The reasoning engine is **Claude Opus driven via the `claude` CLI in streaming-JSON mode**
> (`recon/agent.py`): the deterministic layer gathers evidence (parsed statement lines + raw
> text + ledger tx + hard rules) per account×period and hands each batch to Opus, which returns
> the transactions needing attention. Deliverable: `reports/tx_needing_attention.md`. Connects
> as the SELECT-only `recon_ro` Postgres role (BYPASSRLS, cannot write). See that repo's README.

---

## 0. Nature & hard boundaries

- **Read-only auditor, not a fixer.** Verifies; emits findings. Never classifies, posts, or edits the ledger.
- **Independent process.** No changes to the `accountir` application code. It only *interacts with data*:
  1. Postgres `accountir_cloud` via a dedicated **SELECT-only DB role** (the enforcement mechanism — it *cannot* mutate, not just "won't").
  2. **Source statement files** on disk (`company_files.stored_path`, under `/var/lib/accountir-cloud/…`).
  3. **Blockchain scanners** (Etherscan / Moralis / BSCscan-class explorers) for crypto.
- **Writes only to its own store** — findings, the account↔statement map, parsed statement lines, and a
  review/suppression ledger. Nothing it writes touches the accounting DB.
- **Idempotent & re-runnable.** Same inputs → same findings. Re-runs surface only *new/changed* discrepancies;
  accepted items stay suppressed.
- **Deterministic & self-auditable.** Every finding cites its evidence (file sha256 + page + line; entry id +
  event id) and pinned parser/scanner versions, so a reviewer can verify the auditor itself.

## 1. Generic / multi-tenant by construction (v1 requirement)

Arbach is the **first** tenant, not the only one. The engine must run on any account with zero code changes.

- **No tenant-specific knowledge in code.** Entity list, ledger-account→bank/wallet map, wallet addresses,
  statement cadence, classification-consistency rules (+ legitimate-variation exceptions), intercompany
  relationships, and filed-return figures all live in a per-tenant **account profile** the engine *loads*.
  The engine is rule-driven and knows nothing about Fidelity, Hayat, or specific wallets.
- **Dynamic discovery.** Given a tenant, read companies / accounts / statement files / wallets straight from
  `accountir_cloud`. Works for any owner.
- **Pluggable adapters.** Statement-parser framework (per-institution adapters + generic pdftotext/OCR/LLM
  fallback) and a crypto-scanner abstraction keyed by chain. New format/chain = new adapter, not a new engine.
- **Graceful degradation / discovery mode.** A tenant with no profile still gets structural, USD, dup, and
  balance-tie checks where possible, **plus a report of missing profile config**. v1 covers all check types and
  is ready to audit Arbach immediately.

## 2. Entity iteration

Loop every entity in a tenant (for Arbach: Maven, On-Chain, SweetHome, hayat health, Michael Arbach personal;
Mindwell is a client, excluded), **and** treat them as a group for cross-entity checks. Scope each run by
**tax year** (books close annually; a filed return is authoritative for its year).

## 3. Source ingestion — files & scanners only

The app's `statement_lines` table is **empty** and `is_cleared` is populated for only one account, so the
statement↔transaction linkage does not exist in the data — the auditor builds it.

- **Bank/card/brokerage statements = the PDF/source files**, parsed by the auditor into a canonical line model
  (date, description, amount, running balance) — reuse `pdftotext`/`pdftoppm`/`tesseract`; LLM extraction fallback
  for unknown layouts. Parse into the auditor's **own** store (keeps it read-only + independent).
- **Crypto = blockchain scanners.** Pull wallet history per chain from the scanner (addresses from the profile),
  verify tx hashes, strip spam/spoofed-token inflows, and treat the chain as the "statement." Cross-check against
  `crypto_provenance` (tx_hash, chain, direction, verified).
  **Scanner tier (mirrors the app's own crypto stack):** primary = **Etherscan V2 multichain** (+ **Moralis** for
  BSC, which needs a User-Agent header past Cloudflare); **fallback = Alchemy JSON-RPC** when the free explorer is
  unreachable or rate-limited. Alchemy key = `ALCHEMY_API_KEY` in the repo `.env` (the same key `src/config.rs`
  resolves via `resolved_alchemy_key()`); Etherscan/Moralis keys in `/tmp/migrate/crypto_config.json` (chmod 600).
- **Wise / Plaid** are additional structured sources (`wise_transfers`, `plaid_imported_transactions`) reconciled
  the same way — each is a "statement" with its own completeness/dup/tie-out logic.

## 4. Statement completeness & de-duplication

- **4a. Account registry.** Map every ledger cash/card/wallet/brokerage account → institution, mask, currency,
  expected cadence. Flag accounts with **no registered statement source** and files matching **no known account**.
- **4b. Continuity / gaps.** Order statements per account; detect **missing periods** and **overlaps**. Decisive
  check: **closing balance of period N == opening balance of period N+1** end-to-end.
- **4c. Duplicates.** Exact file dup (`company_files.sha256`), same-period-same-account dup (re-upload under a
  new filename), and near-dup (overlapping ranges). Flag **category inconsistency** (e.g. `bank_statement` vs
  `statement`) that breaks "find all statements" queries.

## 5. Statement ↔ ledger reconciliation (both directions)

- **Completeness:** every *source line* has a matching *ledger tx* → catches **missing/unrecorded tx**.
- **No phantoms:** every *ledger tx in a statement-backed account* maps to a *source line* → catches
  **invented / double-booked tx** (no document behind it).
- **Matching engine** tolerates: post-date vs statement-date skew (±N days), **fee-inclusive "charged" amounts**
  (net vs charged), sign conventions, split/merged entries. Emits a **confidence score**; ambiguous matches go to
  a **human-review queue**, never auto-resolved.
- **Balance tie-out** (strongest check): booked running balance at each period close == statement closing balance,
  to the penny. Any period that doesn't tie is a finding.

## 6. Classification consistency (verify only — never re-classify)

Build a **(counterparty, direction, context) → account** map across the whole ledger; flag the *same* payer booked
to *different* accounts. **Nuance:** some variation is legitimate and rule-sanctioned (e.g. outflow-to-third-party
vs funding-into-same-rail; crypto to own wallet vs to a counterparty) — these exceptions live in the profile and
are suppressed. Flag genuine contradictions, ranked by dollar impact. A human decides.

## 7. Entity attribution / leakage

- Every tx whose **source belongs to entity A's account** must be **booked under entity A**. Mismatched
  `company_id` = leakage finding.
- **Intercompany mirror check:** an intercompany transfer must appear as *paired* entries in both entities and
  **net consistently**. Flag one-sided moves and net imbalances in the intercompany debt ledger.

## 8. Additional checks required to meet the purpose

1. **Multi-source coverage** — bank + crypto + Wise + Plaid, each reconciled (§3).
2. **USD-only guard** — flag any non-USD journal line (policy is all-USD; FX never applied).
3. **Structural integrity** — unbalanced entries (Σdebit≠Σcredit), orphan lines, lines on inactive/foreign
   accounts, referenced void entries, non-empty clearing/suspense accounts (must net ~0).
4. **In-ledger duplicate tx** — same date+amount+counterparty booked twice in one account.
5. **Filed-return drift** — for a filed year, flag when the current ledger has drifted from filed figures.
6. **Severity model + drill-down report** — each finding `{entity, account, period, severity, category,
   dollar_impact, source_ref, entry_ref, explanation}`, ranked most-material first; per-entity + roll-up;
   click-through from finding → source line **and** ledger entry.
7. **Review/suppression ledger** — mark findings accepted / known-exception / false-positive; suppressed items
   don't re-surface.
8. **Coverage scoreboard** — per account-year: `% source lines matched`, `% ledger tx matched`, `balance-tie
   status`. The "how clean is this entity" trend.

## 9. Pipeline & shape

Standalone repo/dir, any language, **read-only DB role is the safety guarantee**.

`extract` (parse statements + pull scanners/sources) → `normalize` (canonical line model) →
`match` (bidirectional, tolerant) → `check` (§4–8 rules) → `report` (findings + scoreboard) → `review`
(suppression state). Runs per entity in parallel; a final cross-entity pass for intercompany + leakage.

## 10. Open decisions

- Where the tool lives (own repo, since it's "independent from this system").
- Language/stack.
- Scanner providers + key management per chain (Etherscan/Moralis today).
- v1 rollout: all Arbach entities × all years × all sources at once, or calibrate the matcher on one known-good
  entity-year (Hayat 2025 is already statement-verified) before fanning out.
