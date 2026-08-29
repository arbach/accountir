# Accountir — Operations & Migration Handoff

> Audience: a new operator taking over or migrating this app. See `README.md` /
> `README_ONBOARDING.md` for product depth. This is the ops/migration source of truth.

## 1. What it is
An **event-sourced accounting** system with three cooperating parts:
- **accountir** — the core sync/ledger server + terminal UI (single Rust binary).
- **accountir-cloud** — a multi-tenant **SaaS web app** (accounting.apix.us).
- **accountir-agentd** — a daemon that runs **per-company Claude CLI agent sessions**
  (AI bookkeeping assistant) against the cloud database.

- **Live URL:** https://accounting.apix.us  (also `www.accounting.apix.us`)
- **Status:** Live, active (all three services running).
- **Source:** GitHub `github.com/arbach/accountir` (private). Local checkout at `/home/ubuntu/repos/accountir`.

## 2. Stack & runtime
- **Language:** **Rust** (all three binaries). Build with `cargo build --release`.
- **Web framework:** `axum` + `tower-http`. **TUI:** `ratatui` + `crossterm`.
- **Local ledger DB:** SQLite via `rusqlite` (bundled). **Cloud DB:** PostgreSQL.
- **Other crates:** tokio, serde/serde_json, clap, uuid, chrono, rust_decimal, sha2.
- **Process manager:** systemd (3 units). **Reverse proxy:** Caddy + oauth2-proxy.

## 3. Third-party dependencies (external services)
| Service | Used by | Purpose | Credential (in `/etc/accountir-cloud/env`) |
|---|---|---|---|
| PostgreSQL (local) | cloud + agentd | tenant data store | `DATABASE_URL` → db `accountir_cloud` |
| **Plaid** | accountir-cloud | bank/transaction aggregation | `PLAID_CLIENT_ID`, `PLAID_SECRET`, `PLAID_ENV`, `PLAID_REDIRECT_URI`, `PLAID_WEBHOOK_URL` |
| **Lob** | accountir-cloud | physical mail / print | `LOB_API_KEY` |
| **Google OAuth** (oauth2-proxy) | cloud | login | `/etc/oauth2-proxy/oauth2-proxy.cfg` |
| **Anthropic Claude CLI** | accountir-agentd | AI agent sessions | uses `ubuntu`'s Claude auth (`HOME=/home/ubuntu`); `AGENT_MODEL`, `AGENT_MCP_URL` |

> Plaid access tokens are encrypted at rest with `PLAID_TOKEN_ENC_KEY` — **migrate that key or every linked bank connection becomes undecryptable.**

## 4. Where information is written & stored  ← read this before migrating
1. **PostgreSQL `accountir_cloud`** (local `5432`, user `accountir`) — the SaaS
   system of record (tenants/companies, users, sessions, ledger events, Plaid
   items/tokens). Shared by **accountir-cloud** and **accountir-agentd**.
2. **SQLite `/home/ubuntu/accountir-data/accountir.db`** — the core sync server's
   event-sourced ledger (the `accountir` service runs from `WorkingDirectory
   /home/ubuntu/accountir-data`). Back this file up as a unit.
3. Secrets/config: `/etc/accountir-cloud/env`, `/etc/accountir-agentd/env`,
   `/etc/oauth2-proxy/oauth2-proxy.cfg`.

## 5. Configuration & secrets (env key names)
- **`/etc/accountir-cloud/env`:** `DATABASE_URL`, `BIND_ADDR` (→ :9877),
  `COOKIE_SECURE`, `SESSION_TTL_DAYS`, `SESSION_COOKIE_KEY`, `PUBLIC_BASE_URL`,
  `PLAID_CLIENT_ID/SECRET/ENV/REDIRECT_URI/WEBHOOK_URL`, `PLAID_TOKEN_ENC_KEY`,
  `LOB_API_KEY`, `RUST_LOG`.
- **`/etc/accountir-agentd/env`:** `DATABASE_URL`, `AGENTD_BIND`, `AGENT_MCP_URL`,
  `AGENT_MODEL`, `AGENT_TURN_TIMEOUT_SECS`, `RUST_LOG`.

## 6. Hosting & process
- **Host:** studio ops box `15.204.118.211` (see memory `this-host-is-caddy-prod-box`).
- **Services (systemd):**
  - `accountir.service` → `target/release/accountir -d /home/ubuntu/accountir-data/accountir.db serve` (user `ubuntu`).
  - `accountir-cloud.service` → `/usr/local/bin/accountir-cloud` (user `accountir-cloud`, `EnvironmentFile=/etc/accountir-cloud/env`), **port 9877**.
  - `accountir-agentd.service` → `/usr/local/bin/accountir-agentd` (user `ubuntu`, `HOME=/home/ubuntu`).
- **Caddy** `accounting.apix.us` → oauth2-proxy → `127.0.0.1:9877`.
- **oauth2-proxy.service** (generic) gates the cloud app.
- **DNS:** `apix.us` (Namecheap) → this box.

## 7. Build & deploy
```bash
cd /home/ubuntu/repos/accountir
git pull
cargo build --release                         # builds all binaries
sudo cp target/release/accountir-cloud /usr/local/bin/
sudo cp target/release/accountir-agentd /usr/local/bin/
sudo systemctl restart accountir accountir-cloud accountir-agentd
```
(Confirm exact binary names in `target/release/` — the sync server binary is `accountir`.)

## 8. Migration checklist (to a new host)
1. Install Rust toolchain, PostgreSQL, Caddy, oauth2-proxy, and the Claude CLI (logged in as the runtime user for agentd).
2. `git clone` the repo; `cargo build --release`.
3. **Postgres:** `pg_dump accountir_cloud` on the old host → restore on the new one; create the `accountir` role.
4. **SQLite:** copy `/home/ubuntu/accountir-data/accountir.db` verbatim.
5. Copy `/etc/accountir-cloud/env` + `/etc/accountir-agentd/env` — **including `PLAID_TOKEN_ENC_KEY` and `SESSION_COOKIE_KEY`** (rotating them breaks Plaid links / logs everyone out). Update `DATABASE_URL`, `BIND_ADDR`, `PUBLIC_BASE_URL` as needed.
6. Recreate the 3 systemd units + the `accountir-cloud` system user.
7. Recreate oauth2-proxy config + a Google OAuth client for the new callback URL.
8. Update **Plaid** dashboard redirect/webhook URIs to the new domain.
9. Recreate the Caddy site; point `apix.us` DNS at the new host.
10. Smoke test: login, a Plaid link flow (sandbox), an agent session.

## 9. Gotchas
- **Two databases, two engines** (Postgres for cloud, SQLite for the sync ledger) — migrate both.
- `accountir-agentd` shells out to the **Claude CLI** as the runtime user — that user must be logged into Claude on the new host.
- `PLAID_TOKEN_ENC_KEY` is load-bearing for all bank connections.
- Plaid `PLAID_ENV` may be sandbox vs production — verify before go-live.
