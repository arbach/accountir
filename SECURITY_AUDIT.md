# Security Audit — accountir

**Date:** 2026-08-26
**Branch:** `tax-pipeline-integration`
**Scope:** Full codebase — the local-first desktop app (`src/`, binary `accountir`, HTTP sync server on `127.0.0.1:9876`), the multi-tenant cloud service (`cloud/`, binary `accountir-cloud` on `127.0.0.1:9877`), the AI agent tooling (`cloud/src/ai/`), Plaid/Wise integrations, secrets, dependencies, and file permissions.
**Method:** Manual review + three focused sub-audits (cloud web routes, Plaid/MCP/webhook, AI agent tools). All secret values were handled by location only — none are printed here.

---

## Environment constraints affecting remediation

- **No Rust toolchain is installed** (`cargo`/`rustc` absent). The running services are prebuilt binaries (`/usr/local/bin/accountir-cloud`, `target/release/accountir serve`). Source code fixes **cannot be compiled, deployed, or verified** in this environment, and restarting the existing binaries would **not** pick up any source change.
- **No npm/Node project exists** (no `package.json`/`package-lock.json` anywhere), so `npm audit` is not applicable. `cargo-audit` is not installed and could not be run. Dependencies were reviewed manually against `Cargo.lock` (see §Dependencies).
- Consequently: I applied **one** minimal source fix (F-1 below, the privilege-escalation bug — a pattern-identical change to existing sibling handlers) but did **not** restart any service, because the fix requires a rebuild I cannot perform here and restarting live financial services without an applied, verified change is not safe. Everything else is documented for the maintainer to apply and rebuild.

---

## Summary of findings

| ID | Severity | Component | Issue |
|----|----------|-----------|-------|
| H-1 | **High** | Local server | Unauthenticated API + `CorsLayer::very_permissive()` → any website can read financial data / drive imports via the browser |
| H-2 | **High** (fixed in source) | Cloud web | `admin_member_add` missing role check → any member can grant `owner` (privilege escalation) |
| M-1 | Medium | Local server | `import/bank-file` reads an arbitrary `file_path` — no auth, no path validation |
| M-2 | Medium | Plaid webhook | Unauthenticated JWKS-fetch amplification (forged `kid` flood → unbounded outbound Plaid calls) |
| M-3 | Medium | AI agent | `mail_tax_form` destination is fully model-controlled (exfil vector) — mitigated by server-enforced `signed` gate |
| M-4 | Medium | AI agent | Prompt-injection can drive tenant-scoped write tools (post/void/reclassify/import) and pivot across the user's own entities |
| M-5 | Medium | Cloud web | `banks_historical` / `banks_statements_check` scope Plaid-token reads by RLS only (no `company_id` predicate) |
| L-1 | Low | Cloud web | `invoice_send` interpolates customer name into email HTML without escaping |
| L-2 | Low | Plaid | `/plaid/link-event` logs full untrusted body (log injection / spam) |
| L-3 | Low | MCP | MCP bearer token stored & compared in plaintext, no expiry |
| L-4 | Low | Plaid | Webhook JWK `expired_at` never checked; key cache never evicts |
| L-5 | Low | AI/Wise/tax | `fetch_tax_form`/Wise: no request timeout; Deno helper runs with FS-wide permissions |
| L-6 | Low | Cloud web | No anti-CSRF tokens (mitigated by `SameSite=Lax`; defense-in-depth only) |
| I-* | Info | Various | See §Informational — secrets hygiene, whitelisted `format!` SQL, cookie config, prompt-injection→WebSearch |

**No hardcoded secrets** were found in source or git history. **No SQL injection**, **no shell injection**, **no classic SSRF**, and **no cross-tenant IDOR** were found in the cloud service — tenant isolation (membership-gated `active_company` + per-query `company_id` + Postgres FORCE-RLS) is applied consistently. **File permissions are clean.**

---

## High

### H-1 — Local sync server is unauthenticated and served with wide-open CORS
**Files:** `src/server/mod.rs:879` and `src/server/mod.rs:2267` (`CorsLayer::very_permissive()`); routers at `:882-906` and `:2270-2280`; bind `127.0.0.1:9876` at `:910`, `:2284`.

The local HTTP server exposes the user's full ledger and money-moving actions with **no authentication on any route** — e.g. `GET /transactions` (`bg_transactions` `:807`), `GET /accounts/banks`, `POST /import/bank-csv`, `POST /import/bank-file`, `POST /plaid/sync`, `POST /plaid/exchange-token` — and wraps them in `CorsLayer::very_permissive()`, which reflects the caller's `Origin` and allows any method/headers.

Although the socket is bound to `127.0.0.1` (not network-exposed — good), `localhost` is reachable from the user's **browser**. Any web page the user visits can issue `fetch('http://localhost:9876/transactions')`; because CORS reflects the origin, the malicious page's JavaScript can **read the response** (all financial transactions, account numbers, vendors) and can **POST** to trigger imports, Plaid sync, and token exchange. This is precisely the cross-origin exfiltration that CORS is meant to prevent, defeated by `very_permissive`.

**Fix (choose per how the TUI talks to the server):**
1. Replace `CorsLayer::very_permissive()` with a strict `CorsLayer` that only allows the specific origin the app front-end uses (or drop CORS entirely if only same-process/`open::that` navigations hit it), and
2. Require a local shared secret/session token on every non-`GET`, ideally all, route (the TUI already holds the DB handle; generate a per-run token and require it as a header). Also add a `Host`-header allowlist to blunt DNS-rebinding.

---

### H-2 — Privilege escalation: `admin_member_add` skips the admin-role check  *(fix applied in source, not yet deployed)*
**File:** `cloud/src/web/routes.rs:2030` (`admin_member_add`, route `POST /app/admin/members` at `:158` area).

Every other admin handler gates on the caller's role first — `admin_member_role` (`:1760`), `admin_member_remove` (`:1742`), `admin_invitation_create`, `admin_settings_save` all do `if !queries::role_can_admin(&role) { return forbidden(); }`. `admin_member_add` did **not**: it only verified the caller is *some* member of the active company, then called `queries::add_member_by_email(..., &req.role)` with an attacker-supplied role. A low-privilege member (even `viewer`) could therefore add an arbitrary email as **`owner`**, taking over the company.

**Fix applied** (mirrors the sibling handlers exactly, using existing functions):
```rust
let role = queries::user_role_in(&state.pool, user.id, company_id).await.ok().flatten().unwrap_or_default();
if !queries::role_can_admin(&role) { return forbidden(); }
let _ = queries::add_member_by_email(&state.pool, company_id, &req.email, &req.role).await;
```
**Status:** Source patched at `cloud/src/web/routes.rs:2030`. **Requires `cargo build --release` + redeploy of `accountir-cloud` to take effect** (no toolchain available here to build/verify).

---

## Medium

### M-1 — `import/bank-file` reads an arbitrary path from the request
**Files:** `src/server/mod.rs:424` (`bg_import_bank_file`, handler at `:370`) and `src/server/mod.rs:2175` (`import_bank_file`, handler at `:2131`).

The handler does `std::fs::read_to_string(PathBuf::from(&req.file_path))` with no sanitization, no allowlist, and (see H-1) no auth. A caller (including a malicious web page via the localhost origin) can supply any absolute path (e.g. `/etc/passwd`, `~/.ssh/...`). The file content is not returned in the response body, but it is copied into the app's `imports/` dir and ingested, and the error path echoes the path (`:444`) — an existence/oracle and unwanted local-file ingestion.

**Fix:** Require auth (H-1); validate that `file_path` resolves (after `canonicalize`) inside an expected import/download directory and rejects `..`/symlink escapes; restrict to expected extensions.

### M-2 — Plaid webhook: unauthenticated JWKS-fetch amplification
**File:** `cloud/src/plaid/webhook_verify.rs:62-75` (`jwk_for_kid`), reached from `cloud/src/http/plaid_routes.rs:44` (`webhook`).

The webhook is intentionally unauthenticated (signature-verified), which is correct — signature verification **is** enforced before the body is trusted (`plaid_routes.rs:51-57`), and the handler only logs. **However**, verification reads the attacker-controlled `kid` from the JWS header and, on cache miss, makes an outbound authenticated call to Plaid `/webhook_verification_key/get` *before* the signature check. Failed/unknown `kid`s are **not** negatively cached, so a flood of forged webhooks each with a fresh random `kid` drives unbounded outbound Plaid API traffic (rate-limit/cost exhaustion, verification-path DoS).

**Fix:** Negative-cache unknown/failed `kid`s with a short TTL; cap distinct-`kid` fetches per interval; rate-limit `/plaid/webhook` (e.g. `tower_governor`).

### M-3 — AI `mail_tax_form`: model-controlled destination (data-exfil vector)
**Files:** `cloud/src/ai/tools.rs:1052` → `cloud/src/tax/mod.rs:1001` (`mail_form`).

The physical-mail `to` address is fully controlled by the LLM. A tax form PDF contains EIN, legal name, address, and full financials, so prompt-injected content in an imported statement or uploaded document could instruct the agent to mail a form to an attacker. **Mitigation is present and server-enforced:** `mail_form` refuses unless `form.status == "signed"` (`tax/mod.rs:1015-1020`), which requires a human Approve+Sign in the UI. The "confirm in chat" instruction in the tool description is advisory only.

**Fix:** Keep the `signed` gate. Additionally whitelist mail destinations to known IRS service-center addresses (or confirm the destination out-of-band), so a signed form still can't be redirected by injected chat text.

### M-4 — AI agent write tools are prompt-injection-drivable (within one tenant / user's own entities)
**Files:** `cloud/src/ai/tools.rs` — `post_journal_entry` (`:1139`), `void_entry`/`unvoid_entry` (`:1115`), `reclassify_line` (`:643`), `import_statement` (`:678`), `set_tax_profile` (`:911`), `fill_tax_form` (`:954`), `create_account` (`:858`); ingestion points `read_document` (`:610`) and `import_statement`. Cross-entity pivot for personal sessions: `cloud/src/http/mcp_routes.rs:176-212`.

Identity/tenant is threaded **server-side** from the MCP token (`mcp_routes.rs:35-61`) — the model never supplies `company_id`/`user_id`, and all queries are `company_id`-scoped under FORCE-RLS. So this is **not** cross-tenant. But the write tools have no human-confirmation gate, so attacker-controlled text ingested via `read_document`/`import_statement` (bank memos, uploaded PDFs) could steer the agent into tampering with the tenant's own books, and — in a personal session — the `entity` argument (membership-gated to the same user's companies) widens the blast radius across that user's own entities.

**Fix:** Gate irreversible-ish writes (void, mail, bulk post, cross-entity writes) behind a UI confirmation not satisfiable from model-generated text; mark imported/document text as untrusted in the system prompt; surface the target entity to the user on cross-entity writes.

### M-5 — Two Plaid-token reads scope by RLS only (no explicit `company_id`)
**File:** `cloud/src/web/routes.rs:2118` (`banks_historical`) and `:2203` (`banks_statements_check`).

Both decrypt a bank's Plaid access token with `SELECT ... FROM plaid_items WHERE id = $1` and rely entirely on `set_tenant(company_id)` + FORCE-RLS for isolation. `plaid_items` does have FORCE ROW LEVEL SECURITY (`cloud/migrations/0002_tenant.sql`), so this is **not currently exploitable**, but it is inconsistent with the correct helper `item_access_token` (`:2246`) which adds `AND company_id = $2`. If RLS is ever weakened this becomes a cross-tenant token disclosure.

**Fix:** Add `AND company_id = $2` to both queries (or route them through `item_access_token`).

---

## Low

- **L-1 — `invoice_send` HTML injection into outbound email.** `cloud/src/web/routes.rs:5249` interpolates `invoice.customer.name` (and number/company) into email HTML via `format!` without `esc_html` (used elsewhere at `:2530`). A customer name with markup renders in the sent email. **Fix:** `esc_html` the interpolated fields. (Also `:2304` reflects a Plaid `sid` unescaped into an HTML attribute — source is trusted Plaid, escape for consistency.)
- **L-2 — `/plaid/link-event` logs the full untrusted body.** `cloud/src/http/plaid_routes.rs:72-85` (route `:31`) has no auth (by design) and logs `payload = %body`; embedded control chars enable log-line forgery and unauthenticated log-volume spam. **Fix:** truncate + strip control chars before logging; rate-limit.
- **L-3 — MCP bearer token stored/compared in plaintext, no expiry.** `cloud/src/http/mcp_routes.rs:35-61` looks up `agent_sessions.mcp_token = $1` by plaintext equality. Auth is correctly enforced and company-scoped; this is at-rest hardening. **Fix:** store & compare a SHA-256 hash; add `expires_at`.
- **L-4 — Webhook JWK expiry unchecked; key cache never evicts.** `cloud/src/plaid/webhook_verify.rs:62-75,94-97`. Plaid's `expired_at` is ignored and cached keys never age out. **Fix:** reject keys past `expired_at`; add a cache TTL.
- **L-5 — Missing timeouts / broad sandbox perms.** `fetch_tax_form` (`cloud/src/tax/mod.rs:574-602`) and Wise (`cloud/src/wise.rs:28,55`) use `reqwest` with no timeout (URL host is fixed / form code whitelisted `[a-z0-9]{,24}` — **no SSRF**). The Deno tax-PDF helper (`cloud/src/tax/mod.rs:443`, argv exec, no shell) runs with FS-wide `--allow-read --allow-write`. **Fix:** add request timeouts + max response size; scope Deno flags to the forms/tmp dirs.
- **L-6 — No anti-CSRF tokens** on state-changing `POST` handlers. Mitigated because the session cookie is `SameSite=Lax` (`cloud/src/web/routes.rs:757`, `:538`; `cloud/src/http/auth_routes.rs:207`), which blocks cross-site top-level POSTs. **Fix (defense-in-depth):** add per-form CSRF tokens.

---

## Informational / verified-safe

- **Secrets hygiene — good.** All secrets are loaded from env (`cloud/src/config.rs`, `src/config.rs`) — DB URL, `SESSION_COOKIE_KEY` (64 bytes, validated), `PLAID_SECRET`, `PLAID_TOKEN_ENC_KEY` (32 bytes), `ANTHROPIC_API_KEY`, Wise/explorer keys. **No hardcoded credentials** in source. `.env` is **not** tracked by git and is absent from git history; the only key-like strings in git are test fixtures (`access-production-deadbeef`, `[7u8;32]`). **Action for operators:** ensure production `SESSION_COOKIE_KEY` is a real `openssl rand -hex 64` (the committed `cloud/.env.example` shows all-zeros) and set `COOKIE_SECURE=true` behind TLS (`cloud/src/config.rs:55`, default `false`).
- **Token/session handling — good.** Session tokens are 32 random bytes, **SHA-256-hashed at rest** (`cloud/src/auth/session.rs:16,39`), expiring, and joined against `is_active` users. Passwords are hashed (`cloud/src/auth/password.rs`). Cookies are `HttpOnly`, `SameSite=Lax`, `Secure` gated on config.
- **Plaid token encryption — correct.** AES-256-GCM with a **fresh random 12-byte nonce per encryption** (`cloud/src/plaid/crypto.rs:28`) — no nonce reuse; authenticated. Key from env, 32-byte validated.
- **Webhook signature verification — correct.** `alg=ES256` pinned at both header and `Validation` (`cloud/src/plaid/webhook_verify.rs:88,100`), `iat` freshness ±5 min, body bound via `sha256(body)` — no alg-confusion/`none` bypass. (See M-2 for the amplification issue, which is orthogonal.)
- **No SQL injection.** All queries use bound params. The `format!`-built SQL in `cloud/src/queries.rs` (`:1048`,`:1233` order-by; `:2081`,`:2143`,`:2177`,`:2215` role) only interpolates **whitelisted** fixed fragments / enum values matched against `owner|admin|accountant|viewer` — verified safe. (Recommend binding anyway to kill the anti-pattern.)
- **No cross-tenant IDOR** in the cloud service: `active_company`/`resolve_company_id`/`resolve_entity` are all membership-gated, and resource queries additionally filter by `company_id` under FORCE-RLS (`cloud/migrations/0002_tenant.sql`, `current_company_id()` fails closed to NULL).
- **Path-serving handlers are allowlist-guarded.** `signature_font` (`cloud/src/web/routes.rs:1925`) validates `key` via `is_valid_font` (strict allowlist) before `read`. Document serving (`:3650`, `:4655`) reads DB-stored `stored_path` scoped to the tenant.
- **Command execution is shell-free.** `statement_processor.rs:88` (`claude`), `tax/runtime.rs` (Deno), `plaid/statements.rs` (`pdftotext`/`pdftoppm`/`tesseract`), `signature.rs:51` (`pdfform`) all use `Command` with fixed argv arrays and stdin/temp-file data — **no argument or shell injection**. Note: `statement_processor` feeds bank-statement text to a `claude -p` sub-agent restricted to `WebSearch` (Bash/Edit/Write/Read/etc. disallowed) — prompt-injection could at most cause a WebSearch of statement text (minor exfil surface); acceptable but worth tracking.

## File permissions
Checked `find` for world-writable and sensitive files under the app dir (excluding `target`/`.git`): **none world-writable.** `.env` is `-rw-------` (600). `accountir.dbj` is `-rw-rw----`. No `.pem`/`.key` files present. **No action needed.**

## Dependencies
No Node/npm project exists → `npm audit` N/A. `cargo-audit` is not installed and **no Rust toolchain is present**, so an automated advisory scan could not be run in this environment. Manual `Cargo.lock` review of security-relevant crates shows current, non-flagged versions: `axum 0.8.8`, `hyper 1.8.1`, `tokio 1.49.0`, `time 0.3.46`, `rusqlite 0.39.0`, `chrono 0.4.43`, `quick-xml 0.39.2`, `flate2 1.1.9`, `rust_decimal 1.40.0`, `smallvec 1.15.1`. **Recommendation for the maintainer:** run `cargo audit` (and `cargo audit` in `cloud/`) in a build environment as part of CI; nothing here indicates a known-vulnerable pinned version, but this could not be authoritatively confirmed without the tool.

---

## What was changed vs. left for the maintainer
- **Applied (source only, needs rebuild+redeploy):** H-2 role check in `cloud/src/web/routes.rs:2030`.
- **Not applied — require rebuild/redesign and cannot be compiled or verified here:** H-1, M-1..M-5, L-1..L-6. Each has a concrete fix above.
- **No services were restarted** — the one code fix needs a rebuild that is not possible in this environment, and restarting the prebuilt binaries would neither apply it nor safely serve a purpose.
