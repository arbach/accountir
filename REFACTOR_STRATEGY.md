# Refactor Strategy — Remove Desktop, Keep Server + Web

**Status:** Strategy (not yet executing). **Owner:** aarbache. **Last updated:** 2026-07-06.

## Objective

Reduce the codebase to the **multi-tenant cloud server + web UI** only. Delete the
local-first desktop/TUI/CLI application and all its CI. Do it behind a
**comprehensive regression net** built and proven green *before* anything is deleted,
so we can prove the server/web behaves identically afterward.

---

## 1. What we keep vs. delete

### Keep (the "server + web version")
| Path | What it is |
|---|---|
| `cloud/` | `accountir-cloud` — Axum server + Askama HTML web UI + agentd. The product. |
| `core/` | `accountir-core` — pure domain/event types. Only dep of `cloud`. Becomes standalone. |
| `tax/` | Tax subsystem (see §3). Reads Postgres directly; independent of desktop. |
| `docker-compose.yml` | Postgres for cloud (note: env runs **native** Postgres, not Docker). |
| `BOOKKEEPING_HARD_RULES.md`, `README_ONBOARDING.md` | Product docs. |

### Delete (desktop / local-first / its CI)
| Path | Why |
|---|---|
| `src/` | Root `accountir` crate: TUI (ratatui), CLI (clap), local SQLite store, local dev server, gnucash import, crypto explorer. |
| `Cargo.toml`, `Cargo.lock` (root) | Root workspace/package definition. |
| `migrations/` | Local **SQLite** migrations. Cloud has its own in `cloud/migrations/`. |
| `tests/accounting_integration.rs` | Root-crate integration test. |
| `examples/gnucash.gnucash` | Desktop gnucash-import sample. |
| `scripts/install.sh`, `scripts/install.ps1` | Desktop binary installers. |
| `accountir.dbj`, `bugbearnewest` | Local SQLite database files. |
| `.github/workflows/*` (ci, build, dev, release) | All build `--package accountir` (desktop). Replaced in Phase 3. |

`core` becomes a standalone crate after the root workspace is deleted: it already has a
valid `[package]`, and `cloud` (its own workspace) references it via `../core`.

---

## 2. Dependency verdict — deletion is mechanically safe

- `cloud/Cargo.toml` depends **only** on `accountir-core` (`../core`), never the root `accountir` crate.
- Every subprocess `cloud` spawns is an *external* binary — `pdfform`(.py), `pdftotext`/`pdftoppm`/`tesseract`, `deno` (tax), `claude` (agentd). **None is the desktop binary.**
- Phase 1 turns "mechanically safe" into "proven safe."

---

## 3. The tax engine — clarification

It is **not** primarily a Python engine. Three languages:
- **`tax/opentax/`** — the actual engine: vendored fork of **OpenTax**, a **TypeScript/Deno** federal tax engine (~787 `.ts`). Runs via `deno task tax`. f1040/2025 works; entity forms (f1120, f1120-S+K-1+8825, state) are **scaffolded specs, not yet implemented**.
- **`tax/bridge/`** — **Python** scaffold (`export_return.py`, `classify.py`, `step4.py`) that reads the `accountir_cloud` **Postgres** ledger → OpenTax input nodes. Per its docs, **not yet end-to-end**.
- **`cloud/src/tax/`** (Rust) + **`cloud/scripts/pdfform.py`** — Rust orchestration shelling out to fill IRS PDFs + Lob mailing.

Because the bridge reads Postgres (the server's DB), the tax subsystem survives desktop removal untouched.

---

## 4. Guiding principle

**Lock behavior before touching anything.** Build the net against the current server,
snapshot real outputs as golden masters, delete desktop, then prove the net is still green.

---

## 5. Environment (probed 2026-07-06)

| Capability | Status |
|---|---|
| Deno 2.9.0 | ✅ OpenTax runnable |
| Postgres 18.4 (native, peer auth via `postgres` user) | ✅ `accountir_cloud` populated |
| `pg_dump`, python3 3.14, tesseract, pdftotext/ppm, `claude` | ✅ present |
| Docker | ❌ not installed (native Postgres instead) |
| Live data | 6 companies, 3,784 entries / 7,578 lines, 4,788 events, 19 tax forms, 123 vendors |
| Entities | maven (c_corp), hayat / on-chain / sweethome (s_corp), michael-arbach (individual), mindwell (no profile) |
| Existing tests | Only 3 unit files in `cloud` (password, session, plaid crypto); no `cloud/tests/`. |
| SQL checking | Runtime string queries (`sqlx::query` ×114, `query_as` ×60); **no `.sqlx` offline cache** → tests need a live DB. |

App connects via `DATABASE_URL=postgres://accountir:dev@localhost:5432/accountir_cloud`
(from `cloud/.env.example`). Tests will use a dedicated `accountir_test` role/db (peer-auth
provisioning via the `postgres` superuser).

---

## 6. Phased plan

### Phase 0 — Comprehensive safety net (BEFORE any deletion)

**Test DB provisioning.** Create a dedicated `accountir_test` role + database. Each test run
loads the sanitized seed (below) into a fresh database; the Axum app runs **in-process** via
`tower::ServiceExt::oneshot` (no network — deterministic, fast).

**Seed = sanitized snapshot of live `accountir_cloud`.** Generated with `pg_dump`, then:
- **Keep** accounting/tax tables: `companies`, `accounts`, `journal_entries`, `journal_lines`,
  `entry_sources`, `entry_categories`, `vendors`, `tax_profiles`, `tax_forms`, `events`,
  `documents`, `company_files` (metadata), `wise_transfers`, `plaid_imported_transactions`,
  `crypto_provenance`, `address_labels`.
- **Drop / pseudonymize** secrets + PII: `auth_users` (replace with fixed test users +
  known password hashes), `sessions`, `plaid_items`/`plaid_local_accounts` access tokens
  (scrub encrypted blobs), `wise_connections` creds, `chat_messages` (1,508 — drop or redact),
  `owner_signatures`, `agent_sessions`.
- Seed committed to `cloud/tests/fixtures/seed.sql` (safe to check in).

**Layer 1 — domain invariants (`core`, no DB):** every entry balances (Σdebit = Σcredit),
no posting to void entries, Merkle chain verification, money/rounding rules.

**Layer 2 — HTTP golden-master (`cloud`, against seeded DB):**
- Money-critical reports — assert **exact** figures per company/period:
  trial balance, balance sheet, income statement, cash-flow.
- Flows: entry create→render→verify; invoice create→render; transaction reclassify.
- Auth + admin + **tenant isolation (RLS)**: a member of company A cannot read company B.
- Golden values stored as committed snapshots; diff on regression.

**Layer 3 — tax reconciliation (`tax/`, Deno + Python + DB):**
- Run the bridge per entity-year for maven/hayat/on-chain/sweethome; assert computed
  ordinary income **ties to book net income** (the bridge's own hard rule). Log deltas.
- Where the bridge is not yet end-to-end, capture the current OpenTax input mapping as a
  golden snapshot so the refactor can't silently change it.

**Exit criteria:** full net runs green against the current tree.

### Phase 1 — Prove independence
- `grep -rn "accountir::" cloud core` and root-crate refs → expect **zero**.
- Build `cloud` + `core` in isolation *while desktop still exists*; any hidden coupling fails loud here, not later.

### Phase 2 — Delete desktop + workflows
- Remove every path in §1 "Delete."
- Confirm `core` builds standalone; `cloud` builds unchanged.

### Phase 3 — Prove green + new CI
- Re-run the entire Phase 0 net → must be **byte-identical**.
- New `.github/workflows/ci.yml` targeting `cloud`: `cargo fmt --check`, `cargo clippy -D warnings`,
  `cargo test` with a Postgres **service** seeded from the fixture, and (optionally) an
  `sqlx` prepare/offline check.

---

## 7. Git approach
- Work on a branch (e.g. `remove-desktop-keep-cloud`); `main` currently has uncommitted changes.
- Suggested commit sequence: (1) Phase 0 net, (2) Phase 2 deletions, (3) Phase 3 CI — so the
  "net is green" state is a reviewable checkpoint before deletion.

## 8. Risks & rollback
| Risk | Mitigation |
|---|---|
| Hidden desktop coupling in cloud | Phase 1 isolation build catches it pre-delete. |
| Golden values drift from real data changes | Regenerate seed + goldens deliberately; keep them in one fixtures dir. |
| Secrets/PII leaking into git | Sanitized seed (§6); never commit the raw dump. |
| Tax bridge not end-to-end | Layer 3 snapshots the mapping; full reconciliation as bridge matures. |
| `core` workspace breakage | It has a valid `[package]`; cloud references it by path. Verified in Phase 1/2. |

## 9. Out of scope (future stack improvements — noted, not part of this refactor)
- Split `cloud/src/web/routes.rs` (**4,551 lines / 166 KB**) into per-domain modules. Highest-ROI cleanup.
- Vendor HTMX (currently loaded from unpkg CDN) as a served static asset.
- Adopt `sqlx` compile-time-checked queries + committed `.sqlx` offline cache (176 call sites).
- Bump `askama` 0.12 → 0.14; general dep hygiene.
