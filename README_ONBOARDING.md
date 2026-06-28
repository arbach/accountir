# accountir — Multi-Owner Accounting & Tax System (READ ME FIRST)

_Last updated: 2026-06-28. Onboarding reference for anyone (human or AI) working on this
system. **This is a GENERIC, multi-owner bookkeeping & tax platform** — it is designed so
each "owner" has their own set of companies, fully isolated from other owners. **Today there
is exactly one owner onboarded: Michael Arbach.** Most of this README documents the Arbach
instance because it's the only live data, but the architecture is generic — read §2–3 to
understand the platform, then §4+ for the current owner's specifics._

---
## 1. WHAT THIS IS
A self-hosted double-entry accounting + US-tax system (the **accountir-cloud** app) that does
the full cycle for a group of related companies: import bank/credit-card/Wise/crypto activity →
categorize via an AI chat assistant → produce financial statements → prepare, fill, and mail
IRS/state tax forms. The owner's standing rule: **be audit-ready; when in doubt over-pay the
IRS / claim every legitimate deduction; never underpay.**

It is **multi-owner**: many independent owners can each run their own companies on the same
instance, isolated by Postgres row-level security. **For now only Arbach exists**, but never
hard-code "Arbach" into the system — treat owner/company as data.

---
## 2. PLATFORM ARCHITECTURE (generic — applies to ANY owner)
- **Stack:** Rust/axum web app + `agentd` (per-company persistent Claude session) + Postgres DB **`accountir_cloud`**. Served `127.0.0.1:9877` → https://accounting.apix.us. Binary `/usr/local/bin/accountir-cloud`. systemd `accountir-cloud.service`.
- **Source:** `/home/ubuntu/repos/accountir` (single repo/folder, branch `main`). The local accounting engine is at the repo root (`src/`, `core/`); the **web app is under `cloud/`** (Rust/axum); HTML in `cloud/templates/*.html`. _(The cloud app used to live in a separate `accountir-cloud` worktree on branch `plaid-statements-parsing`; that was merged into `main` and consolidated into this one folder.)_

### Build & deploy UI / code changes  ⚠ READ THIS BEFORE EDITING THE UI
Templates use **askama (compile-time)** — the `.html` files and their inline CSS/JS are **compiled INTO the binary**. There is **no runtime templates/static directory**: editing a `.html` and restarting does **NOTHING**. **Every UI change requires a full rebuild + reinstall + restart.** The deploy procedure for *any* code or UI change:
```bash
cd /home/ubuntu/repos/accountir/cloud         # ⚠ the cargo WORKSPACE EXCLUDES `cloud`
cargo build --release                          #    (members=[".","core"]) — root `cargo build` will NOT build the app
sudo install -m755 target/release/accountir-cloud /usr/local/bin/
sudo systemctl restart accountir-cloud
```
Then verify: `systemctl status accountir-cloud` (active) and hit https://accounting.apix.us. Logs: `journalctl -u accountir-cloud -f`. Service binds `127.0.0.1:9877` → reverse-proxied to https://accounting.apix.us. Env/secrets in `/etc/accountir-cloud/env` (DB URL, LOB_API_KEY, …).
- **DB schema change?** Add a migration in `cloud/migrations/` (sqlx runs them on startup) — don't hand-edit prod tables for shipped features.
- **Roll back:** keep the prior `/usr/local/bin/accountir-cloud` (or rebuild the previous commit) and restart.

### Where the code lives (UI ↔ logic map)
- **UI markup:** `cloud/templates/*.html` — one file per page (e.g. `transactions.html`, `chat.html`, `report_income.html`, `tax_filing.html`, `address_book.html`); `base.html` is the shared layout + left/top nav + floating chat panel. Inline CSS/JS live in these files (no separate asset dir).
- **UI handlers + page structs:** `cloud/src/web/routes.rs` — every `/app/*` route, the askama `#[derive(Template)]` structs that back each `.html`, and the GET/POST handlers. **This is the main file to edit for UI behavior.** `cloud/src/web/mod.rs` wires the router.
- **Read models for pages:** `cloud/src/queries.rs` (the SELECTs/aggregates pages render). **Writes/commands:** `cloud/src/commands/` (account, entry, invoice, mutations) + `cloud/src/store/` (event store + projections).
- **AI chat:** `cloud/src/ai/` (`agent.rs`, `chat.rs`, `tools.rs` = the agent's tools) + binary `cloud/src/bin/agentd.rs`.
- **Auth/tenancy:** `cloud/src/auth/`, `cloud/src/tenant/`, `cloud/src/http/{auth_routes,tenant_routes}.rs`. **Integrations:** `cloud/src/plaid/` (banks), `cloud/src/wise.rs`, `cloud/src/tax/` (`lob.rs` = mail), `cloud/src/file_store.rs` (Documents), `cloud/src/docgen.rs` (report HTML/PDF).
- **Entrypoints:** `cloud/src/main.rs` (web), `cloud/src/db.rs` (migrations), `cloud/src/config.rs`.

### Tenancy model (owners → companies)
- **`auth_users`** = login users. **An OWNER is an auth_user.**
- **`companies`** = the tenants. Each has **`owner_user_id`** (the owner) and **`is_personal`** (true for the owner's personal/1040 entity, false for business entities). One owner ⇒ one personal company + N business companies.
- **`memberships`** + **`company_invitations`** = which users can access which companies.
- **Isolation:** every tenant table enforces RLS `tenant_isolation USING (company_id = current_company_id())`. The app sets the active company per session. `sudo -u postgres psql` is superuser and **bypasses RLS** (back-office only — be careful).
- **Personal-session powers:** the owner's `is_personal` company gets extra AI tools (`list_entities`, `move_file`) to re-file a dropped document to whichever of *their* companies it belongs to.

### Onboarding a NEW owner (e.g., a second "Arbach")
1. Create an `auth_users` row (the owner) + their personal company (`is_personal=true`, `owner_user_id`=them) and their business companies (`is_personal=false`, same owner).
2. Seed each company's chart of accounts; set `tax_profiles` (entity_type, legal_name, EIN, address).
3. Connect data sources for that owner (bank via Plaid, Wise/crypto if any). All their data is auto-isolated by RLS — they never see other owners.

### Core tables (amounts in **CENTS**; debit +, credit −)
`auth_users`, `companies`, `memberships` · `accounts` (type: asset/liability/equity/revenue/expense) · `journal_entries` (is_void; source manual/import/…) · `journal_lines` · `tax_profiles` · `tax_forms` (fields jsonb, file_path = served PDF) · `company_files` (Documents; tags text[], doc_year) · `address_labels` (wallet→name/kind/account_code) · **`vendors`** (vendor master).

### Subsystems
- **AI chat** (`/app/chat`): per-company Claude session via `agentd` (127.0.0.1:9878). Dropping a file (`/app/chat/upload`) stores it to `company_files` and sends extracted text to the agent. Tools in `cloud/src/ai/tools.rs` (post_journal_entry, list_transactions, read_document, set_address_label, fill_tax_form, fetch/mail_tax_form…).
- **Tax pipeline:** fetch IRS/state PDF → fill fields → review → mail via **Lob** (key in /etc/accountir-cloud/env; verify tracking — past letters silently failed on credit).
- **Bank import:** Plaid; plus offline parsed statements during migration.

⚠ **Field-mapping landmine:** IRS PDF field names are cryptic (`f1_47[0]`). Do **NOT** fill by field-id order — that caused catastrophic mis-placements (a $220k gain on the wages line). Always map fields by **widget y-position + the line label to its left**, then verify.

---
## 3. DATA & TOOLING USED IN MIGRATION (owner-agnostic mechanics)
- **`/tmp/migrate/`** (⚠ EPHEMERAL): `parsed2/*.json` parsed statements `{entity,account,txns:[{date,description,amount_cents}]}`; python scripts; tax templates/renders in `forms2025/`; `/tmp/invoices/` per-vendor CSVs; `AUDIT_READINESS_TODO.md`.
- **`/tmp/acc_extract/accounting/`**: `crypto.db` (SQLite `transfers`: txhash, chain, date, wallet, direction, counterparty, symbol, amount_usd, kind, contact, account_code) and `xero.py` (Wise+Xero API client) + `config.json`/`tokens.json` (auto-refresh). ⚠ **crypto.db is a CACHE — it is NOT the accountir books.**
- **APIs:** Wise (`wise_config.json` token; `/v1/transfers`, `/v1/balance-statements`, `/v1/accounts`), Xero (write=manualjournals), Moralis (crypto — currently 403; use public RPCs `bsc-dataseed1.binance.org` / `ethereum.publicnode.com` + `eth_getCode` to tell contract vs wallet), Lob (mail).
- **Money-flow model (confirm per owner):** `bank→Wise/crypto = TRANSFER (not expense); Wise/crypto→contractor = EXPENSE`. Self-transfers, DEX swaps, exchange moves, brokerage transfers (Robinhood), and refunds are NOT expense.

---
## 3A. USING / MANAGING THE UI (generic — every owner gets the same app)
Sign in at **https://accounting.apix.us** (`/login`; `/signup` for a new user). The app is a
left/top nav of sections; the **active company** drives everything (RLS scopes all data to it).

### Switching company & access control (top-left company menu)
- The company name (top-left) opens a menu of every company you can access — click one to **switch active company** (`/app/admin/companies/{id}/switch`). All pages then show that company's data.
- **Manage companies** (`/app/admin/companies`) — list/create companies for the owner.
- **Members** (`/app/admin/members`) — invite users, set roles (owner/admin/member), remove. Invites are accepted at `/accept-invite/{token}`.
- **Settings** (`/app/admin/settings`) — company settings (name, base currency, fiscal year).

### Day-to-day sections (left nav)
- **Dashboard** (`/app/dashboard`) — at-a-glance balances/activity for the active company.
- **Transactions** (`/app/transactions`) — ⭐ the main categorization workspace. Imported bank/Wise lines land here; assign each to an account. **Reclassify one** (row action) or select many and **bulk-reclassify** (`/transactions/bulk-reclassify`). This is where most daily bookkeeping happens.
- **Journal** (`/app/entries`) — manual double-entry. **New entry** → add lines (debit +/credit −, must balance) → post. **Void / Unvoid** an entry (we never hard-delete — voiding keeps the audit trail).
- **Accounts** (`/app/accounts`) — the chart of accounts; **New** to add an account (pick type: asset/liability/equity/revenue/expense).
- **Banks** (`/app/banks`) — **Link** a bank (Plaid), **Sync** to pull new transactions, **Historical** to backfill, **Statements** to view, **Unlink**. New connections go through `/banks/link`.
- **Wise** (`/app/wise`) — **Sync** Wise transfers into Transactions (uses the transfers API, which is complete).
- **Sales** (`/app/invoices`) — create (`/invoices/new`), **Issue**, **Send**, record **Payment**, **Void**. Customers live at `/app/customers`. A sent invoice has a public link `/invoice/{token}` the customer can open without logging in.
- **Address Book** (`/app/address-book`) — map a **wallet/payee → name + account_code** so future transactions auto-classify. Has server-side **search** (name/address/kind/note) and delete. This is the bridge between raw crypto/Wise counterparties and the vendor/expense accounts.
- **Documents** (`/app/documents`) — upload invoices/contracts/statements; set **doc year**, **lock** (prevent edits), **download**, **delete**. Tag files (e.g. `audit`, `reference`). On the owner's personal company you can move a file to another of your companies.
- **Reports** (`/app/reports`) — **Balance Sheet**, **Income Statement**, **Cash Flow**, **Trial Balance**, and **Tax Documents** (generate / print-all). Use these to confirm books reconcile before filing.
- **Tax** (`/app/tax`) — list of prepared tax forms; **Profile** (`/app/tax/profile`) holds entity_type/EIN/legal name/address; open a form's **PDF**, **Approve**, or **Delete**. ⚠ The PDF endpoint serves the stored `file_path` (not a live re-fill of `fields`) — see §2's field-mapping warning.
- **AI Chat** (`/app/chat`) — the per-company AI accountant. Ask it to categorize, post journal entries, look up transactions, fill forms, etc. **Drop a file** in the chat to store it to Documents and have the agent read it. There's also a floating chat panel on every page (the company name in its header shows which company it's acting on). Controls: clear history, stop a running response, view history.

### Operating rhythm (typical loop)
1. **Banks/Wise → Sync** new activity. 2. **Transactions** → categorize (or let **AI Chat** do it; use **Address Book** to make labels stick). 3. **Reports** → check Trial Balance / Income Statement reconcile. 4. **Tax** → prepare/approve forms. 5. **Documents** → attach the invoice/contract behind each material expense (audit-readiness).

================================================================
# CURRENT OWNER: MICHAEL ARBACH (the only owner today)
================================================================

## 4. ARBACH ENTITIES
| Entity | Type | EIN | Status | Notes |
|---|---|---|---|---|
| **Michael & Andrea Arbach** | Individual 1040 MFJ (`is_personal`) | SSN 813-14-0923 / 402-37-4451 | — | 3 kids: Alexander (2019), Suria (2021), **Palymra (08/20/2025, 154-29-4975)**. Wise profile 19611038. |
| **Maven Financial Technologies Inc** | C-corp (1120) | 92-3379962 | Active | Inc. 03/31/2023. Chase ...3350, Ink CC ...8856. Crypto af6/f27. Wise 39806240. |
| **On-Chain LLC** | S-corp (1120-S) | 82-3930173 | **Dissolved end-2023** | Missouri LLC. PNC ...8667/CC ...7933/LOC ...1703, Discover ...5601. Wise 19611031. **2021–2024 books NOT done.** |
| **Hayat Health LLC** | S-corp (1120-S) | 33-2127261 | Active | Chase ...5272. Revenue = Mindwell consulting (1099). |
| **SWEET HOME KC LLC** | S-corp (1120-S) | 93-2942628 | Active | 3 KC-MO rentals; income on Sch K line 2 / Form 8825. |
| **Mindwell** | (a client, NOT owned) | — | — | Hayat's $90k 2025 revenue is the Mindwell 1099. Should not be a tax entity. |
All S-corps + personal flow to Michael's 1040. **Maven is a C-corp — losses stay as NOL, do NOT flow to Michael.**

## 5. MONEY FLOW (Arbach)
Banks (Chase=Maven, PNC=On-Chain, Chase5272=Hayat) → **Wise** (multi-currency) + **crypto** (af6 `0xc51a72d1581b8a70a1fb60211781ccee78c28af6`, f27/Long `0xEf276f87A0272Ebf6033e65123371e00c0024bA8`) + **Zelle** → overseas contractors. Robinhood ($220k 2023) & inter-account moves = transfers, not expense.

## 6. CURRENT STATE — BOOKS (2025)
Maven **−$4,000** (=Xero ✓) · Hayat **−$26,602** (=Xero ✓) · SweetHome **+$23,809** · Personal **$233,674** (=1040 total income). Intercompany: **Hayat owes Maven $2,000** net. 2023/24: Maven over-booked vs filed; **On-Chain unbooked**.

## 7. CURRENT STATE — TAXES (2025, all DRAFT)
Michael 1040: income $233,674 → AGI $225,374 → taxable $193,874 → **fed $14,576** (LTCG rates; refund $424). IL **$10,592** (refund $437). + SweetHome IL $357. **Total ≈ $25,525; with the $6,600 CTC (3 kids, not yet applied) ≈ $18,925.** The **$220k LTCG** = Michael selling his **Maven C-corp stock** (sale 2025 / paid 2026 → installment, **elected out** → fully 2025; Sch D + 8949 Box F; not §1202). Distinct from the **$52,882 Maven IP sale** and the **2023 $125k LTCG**.

## 8. KEY CORRECTIONS MADE
Fixed catastrophic form field-mapping bugs ($220k on wages line; Maven 1120 all lines shifted; Sch E losses in income cols). Reconciled Wise via the transfers API (On-Chain ≈ $893k out 2021–24; Maven ≈ $359k). Hayat $18k→Maven booked as **intercompany loan** (not expense). Identified DEX swaps mis-read as payments. Built `vendors` master (94 payees, ~$814k; Cynops $244k largest; all overseas → W-8BEN/E). Personal books reconciled to the 1040.

## 9. OPEN ISSUES / TODO (priority)
1. 🔴 **On-Chain 2021–2024 UNBOOKED** (~$1M of activity; ~$543k contractor 2021–23). A 2023 return was filed (+$26,680) but no books; likely **unclaimed losses → refund opportunity** (amend).
2. 🔴 **Crypto sub-ledger (~$306k) not imported into the app** → app 9999 is empty, crypto invisible in UI. Needs import (known→5300 w/ vendor, unidentified→9999) with **entity/year attribution** (af6 used in both On-Chain & Maven years).
3. 🟠 **$57,311 unidentified crypto** (33 wallets) + **$62,680 exchange** + **~$135k unresolved Wise `acct_…`** need names.
4. 🟠 **Maven over-booked vs filed** (~$85–89k/yr) — generic "Subcontractors" bank lumps, likely **On-Chain's payments mis-booked to Maven**.
5. 🟠 **Audit docs:** W-8BEN/E + contracts for ~73 overseas vendors; loan notes (Truffle Pig↔$31,700 interest, etc.); $220k stock-sale agreement. See `AUDIT_READINESS_TODO.md`.
6. 🟡 **CTC** not applied (Sch 8812, $6,600 → total ≈ $18,925).
7. 🟡 **Invoice/contract chat feature** (planned): `attach_document` tool + `document_links` table so dropped invoices auto-classify + link to vendor/transaction.
8. 🟡 **Hayat K-1 PDF** still shows old −$44,602; regenerate to −$26,602 (1040 Sch E already correct).
9. ✅ Address-book search (server-side ILIKE) — built & deployed.

## 10. HOW-TO / GOTCHAS
- Query: `sudo -u postgres psql accountir_cloud` (cents; net income = `-sum(amount) for revenue+expense`).
- **crypto.db ≠ app books.** Tax PDFs: map fields by y-position + label; update both `file_path` and `tax_forms.fields`; never forge signatures.
- **Maven 2025 = −$4,000 intentionally** (IP at $52,882, owner's workpaper) — confirm before "completing."
- **On-Chain 2024+ activity → treat as Maven** (dissolved end-2023).
- Wise: use the transfers API (balance statements miss bank-funded transfers).
- Agent memory: `/home/ubuntu/.claude/projects/.../memory/` (`maven-xero-migration.md`, `xero-2025-trueup.md`).
- Per-vendor CSVs: `/tmp/invoices/<company>/<vendor>__$<total>_<n>tx.csv`.
