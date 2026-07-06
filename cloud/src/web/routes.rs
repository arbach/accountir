use askama::Template;
use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{NaiveDate, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::auth::password::{hash_password, verify_password};
use crate::auth::session::{create_session, delete_session, lookup_session, SESSION_COOKIE_NAME};
use crate::auth::AuthenticatedUser;
use crate::commands::{
    account::{create_account, rename_account, CreateAccountInput},
    entry::{post_entry, EntryLineInput, PostEntryInput},
    mutations::{reassign_line, unvoid_entry, update_entry_memo, void_entry},
};
use crate::error::AppError;
use crate::http::AppState;
use crate::queries;
use accountir_core::events::types::EventAccountType;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(root))
        .route("/login", get(login_form).post(login_submit))
        .route("/signup", get(signup_form).post(signup_submit))
        .route("/logout", post(logout_submit))
        .route("/app/accounts", get(accounts_list).post(account_create))
        .route("/app/accounts/new", get(account_new))
        .route("/app/accounts/{id}/tax-line", post(account_set_tax_line))
        .route("/app/accounts/{id}/rename", post(account_rename))
        .route("/app/vendors", get(vendors_list))
        .route(
            "/app/accounts/{id}/upload-statement",
            post(account_statement_upload)
                .layer(axum::extract::DefaultBodyLimit::max(25 * 1024 * 1024)),
        )
        .route("/app/entries", get(entries_list).post(entry_create))
        .route("/app/entries/new", get(entry_new))
        .route("/app/entries/lines/new", get(entry_line_fragment))
        .route("/app/banks", get(banks_list))
        .route("/app/banks/link", get(banks_link))
        .route("/app/banks/{id}/sync", post(banks_sync))
        .route("/app/banks/{id}/unlink", post(banks_unlink))
        .route("/app/banks/{id}/provision", post(banks_provision))
        .route("/app/banks/{id}/historical", get(banks_historical))
        .route("/app/banks/{id}/statements-check", get(banks_statements_check))
        .route("/app/banks/{id}/statements", get(banks_statements))
        .route(
            "/app/banks/{id}/statements/{statement_id}/import",
            post(banks_statement_import),
        )
        .route("/app/admin/companies", get(admin_companies).post(admin_company_create))
        .route("/app/admin/companies/{id}/switch", post(admin_company_switch))
        .route("/app/admin/members", get(admin_members).post(admin_member_add))
        .route("/app/admin/members/{user_id}/remove", post(admin_member_remove))
        .route("/app/admin/members/{user_id}/role", post(admin_member_role))
        .route("/app/admin/settings", get(admin_settings_view).post(admin_settings_save))
        .route("/app/admin/signature/typed", post(signature_save_typed))
        .route(
            "/app/admin/signature/upload",
            post(signature_upload).layer(axum::extract::DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
        .route("/app/admin/signature/clear", post(signature_clear))
        .route("/app/admin/signature.png", get(signature_preview))
        .route("/app/fonts/{key}", get(signature_font))
        .route("/app/admin/invitations", post(admin_invitation_create))
        .route("/accept-invite/{token}", get(accept_invite_view).post(accept_invite_submit))
        .route("/app/chat", get(chat_view))
        .route("/app/chat/messages", post(chat_send))
        .route("/app/chat/stream", post(chat_stream))
        .route("/app/chat/history", get(chat_history))
        .route(
            "/app/chat/upload",
            post(chat_upload).layer(axum::extract::DefaultBodyLimit::max(500 * 1024 * 1024)),
        )
        .route("/app/chat/clear", post(chat_clear_route))
        .route("/app/chat/stop", post(chat_stop))
        .route("/app/dashboard", get(dashboard))
        .route("/app/tax", get(tax_filing))
        .route("/app/tax/compute", post(tax_compute))
        .route("/app/tax/profile", post(tax_profile_save))
        .route(
            "/app/tax/profile/upload",
            post(tax_profile_upload)
                .layer(axum::extract::DefaultBodyLimit::max(25 * 1024 * 1024)),
        )
        .route("/app/tax/forms/{id}/pdf", get(tax_form_pdf))
        .route("/app/tax/forms/{id}/approve", post(tax_form_approve))
        .route("/app/tax/forms/{id}/delete", post(tax_form_delete))
        .route("/app/tax/sign-all", post(tax_sign_all))
        .route("/app/reports/tax-documents", get(tax_documents))
        .route("/app/reports/tax-documents/generate", post(tax_documents_generate))
        .route("/app/reports/tax-documents/print-all", get(tax_documents_print_all))
        .route("/app/reports/documents/{id}", get(document_view))
        .route("/app/reports/documents/{id}/delete", post(document_delete))
        .route("/app/transactions", get(transactions_list))
        .route("/app/transactions/bulk-reclassify", post(transactions_bulk_reclassify))
        .route("/app/transactions/{line_id}/reclassify", post(transaction_reclassify))
        .route("/app/transactions/{entry_id}/categorize", post(transaction_categorize))
        .route("/app/entries/{entry_id}/void", post(entry_void))
        .route("/app/entries/{entry_id}/unvoid", post(entry_unvoid))
        .route("/app/entries/{entry_id}", get(entry_detail_view))
        .route(
            "/app/entries/{entry_id}/documents/upload",
            post(entry_document_upload).layer(axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024)),
        )
        .route("/app/entries/{entry_id}/memo", post(entry_memo_update))
        .route("/app/reports", get(reports_index))
        .route("/app/reports/trial-balance", get(trial_balance))
        .route("/app/reports/income-statement", get(report_income))
        .route("/app/reports/balance-sheet", get(report_balance_sheet))
        .route("/app/reports/cash-flow", get(report_cash_flow))
        .route("/app/documents", get(documents_list))
        .route(
            "/app/documents/upload",
            post(documents_upload).layer(axum::extract::DefaultBodyLimit::max(500 * 1024 * 1024)),
        )
        .route("/app/documents/{id}/download", get(file_download))
        .route("/app/documents/{id}/delete", post(file_delete))
        .route("/app/documents/{id}/year", post(document_set_year))
        .route("/app/documents/{id}/lock", post(document_set_lock))
        .route("/app/wise", get(wise_view))
        .route("/app/wise/sync", post(wise_sync))
        // Back-compat: the store was formerly the "Files" tab.
        .route("/app/files", get(|| async { Redirect::permanent("/app/documents") }))
        .route("/app/address-book", get(address_book_list).post(address_label_create))
        .route("/app/address-book/{id}/delete", post(address_label_delete))
        .route("/app/customers", get(customers_list).post(customer_create))
        .route("/app/customers/new", get(customer_new_view))
        .route("/app/invoices", get(invoices_list).post(invoice_create))
        .route("/app/invoices/new", get(invoice_new_view))
        .route("/app/invoices/{id}", get(invoice_detail_view))
        .route("/app/invoices/{id}/issue", post(invoice_issue))
        .route("/app/invoices/{id}/payment", post(invoice_payment))
        .route("/app/invoices/{id}/void", post(invoice_void))
        .route("/app/invoices/{id}/send", post(invoice_send))
        .route("/invoice/{token}", get(invoice_public_view))
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "login.html")]
struct LoginTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
}

#[derive(Template)]
#[template(path = "signup.html")]
struct SignupTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
}

#[derive(Template)]
#[template(path = "accounts_list.html")]
struct AccountsListTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    accounts: Vec<queries::AccountRow>,
    tax_line_options: Vec<queries::TaxLineOpt>,
    form_code: String,
}

#[derive(Template)]
#[template(path = "accounts_new.html")]
struct AccountsNewTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
}

#[derive(Template)]
#[template(path = "entries_list.html")]
struct EntriesListTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    entries: Vec<queries::EntryRow>,
}

#[derive(Template)]
#[template(path = "entries_new.html")]
struct EntriesNewTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    accounts: Vec<queries::AccountRow>,
    today: String,
}

#[derive(Template)]
#[template(path = "entry_line_fragment.html")]
struct EntryLineFragmentTpl {
    accounts: Vec<queries::AccountRow>,
}

#[derive(Template)]
#[template(path = "banks_list.html")]
struct BanksListTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    items: Vec<queries::PlaidItemRow>,
}

#[derive(Template)]
#[template(path = "banks_link.html")]
struct BanksLinkTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
}

#[derive(Template)]
#[template(path = "chat.html")]
struct ChatTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    messages: Vec<ChatGroup>,
}

pub struct ChatGroup {
    pub lines: Vec<ChatLine>,
}
pub struct ChatLine {
    pub role: String,
    pub body: String,
}

#[derive(Template)]
#[template(path = "admin_settings.html")]
struct AdminSettingsTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    active_company_name: String,
    company_name: String,
    base_currency: String,
    fiscal_year_start_month: i16,
    can_edit: bool,
    all_companies: Vec<queries::CompanyRow>,
    active_company_id: String,
    // Owner signature (per user, shared across the owner's entities).
    sig_fonts: Vec<(&'static str, &'static str)>,
    sig_has: bool,
    sig_kind: String,
    sig_typed_text: String,
    sig_typed_font: String,
    sig_updated_at: String,
    owner_name: String,
}

#[derive(Template)]
#[template(path = "accept_invite.html")]
struct AcceptInviteTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    company_name: String,
    role: String,
    token: String,
    logged_in: bool,
    all_companies: Vec<queries::CompanyRow>,
    active_company_id: String,
    active_company_name: String,
}

#[derive(Template)]
#[template(path = "admin_companies.html")]
struct AdminCompaniesTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    companies: Vec<queries::CompanyRow>,
    active_company_id: Uuid,
    active_company_name: String,
}

#[derive(Template)]
#[template(path = "admin_members.html")]
struct AdminMembersTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    members: Vec<queries::MemberRow>,
    active_company_name: String,
    can_admin: bool,
    invitations: Vec<queries::InvitationRow>,
    invite_url_base: String,
}

#[derive(Template)]
#[template(path = "transactions.html")]
struct TransactionsTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    rows: Vec<queries::TransactionLine>,
    accounts: Vec<queries::AccountRow>,
    sources: Vec<&'static str>,
    selected_account_ids: std::collections::HashSet<String>,
    categories: Vec<String>,
    selected_category: Option<String>,
    selected_source: Option<String>,
    search: Option<String>,
    start_str: String,
    end_str: String,
    selected_type: Option<String>,
    min_amount_str: String,
    max_amount_str: String,
    selected_sort: String,
}

#[derive(Template)]
#[template(path = "dashboard.html")]
struct DashboardTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    company_name: String,
    kpis: queries::DashboardKpis,
}

#[derive(Template)]
#[template(path = "reports_index.html")]
struct ReportsIndexTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
}

#[derive(Template)]
#[template(path = "report_income.html")]
struct ReportIncomeTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    report: queries::IncomeStatement,
    company_name: String,
    generated_on: String,
}

#[derive(Template)]
#[template(path = "report_balance.html")]
struct ReportBalanceTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    report: queries::BalanceSheet,
    company_name: String,
    generated_on: String,
}

#[derive(Template)]
#[template(path = "report_cashflow.html")]
struct ReportCashFlowTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    report: queries::CashFlow,
    company_name: String,
    generated_on: String,
}

#[derive(Template)]
#[template(path = "trial_balance.html")]
struct TrialBalanceTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    rows: Vec<queries::TrialBalanceRow>,
    total_debit_display: String,
    total_credit_display: String,
    balance_msg: String,
    balance_class: String,
    company_name: String,
    generated_on: String,
}

fn render<T: Template>(t: T) -> Response {
    match t.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "template render failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "template error").into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// Auth helpers
// ---------------------------------------------------------------------------

/// Nav context — needed by base.html for the company switcher. Public so templates can reference it.
#[derive(Default, Clone)]
pub struct NavCtx {
    pub all_companies: Vec<queries::CompanyRow>,
    pub active_company_id: String,
    pub active_company_name: String,
}

async fn build_nav(state: &AppState, jar: &CookieJar, user_id: Uuid) -> NavCtx {
    let companies = queries::list_companies_for_user(&state.pool, user_id).await.unwrap_or_default();
    let active = active_company(state, jar, user_id).await.unwrap_or_else(Uuid::nil);
    let name = companies.iter().find(|c| c.id == active).map(|c| c.name.clone()).unwrap_or_default();
    NavCtx {
        all_companies: companies,
        active_company_id: active.to_string(),
        active_company_name: name,
    }
}

async fn current_user(state: &AppState, jar: &CookieJar) -> Option<AuthenticatedUser> {
    let token = jar.get(SESSION_COOKIE_NAME)?.value().to_string();
    let session = lookup_session(&state.pool, &token).await.ok().flatten()?;
    Some(session.into())
}

async fn require_user(
    state: &AppState,
    jar: &CookieJar,
) -> Result<AuthenticatedUser, Response> {
    match current_user(state, jar).await {
        Some(u) => Ok(u),
        None => Err(Redirect::to("/login").into_response()),
    }
}

const ACTIVE_COMPANY_COOKIE: &str = "accountir_active_company";

/// Active company = cookie value if user has membership in it, else first membership.
async fn active_company(
    state: &AppState,
    jar: &CookieJar,
    user_id: Uuid,
) -> Option<Uuid> {
    if let Some(c) = jar.get(ACTIVE_COMPANY_COOKIE) {
        if let Ok(uuid) = Uuid::parse_str(c.value()) {
            if queries::user_has_membership(&state.pool, user_id, uuid)
                .await
                .unwrap_or(false)
            {
                return Some(uuid);
            }
        }
    }
    queries::resolve_company_id(&state.pool, user_id).await.ok().flatten()
}

/// `?company=<uuid>` carried by a chat request so a browser tab stays pinned to
/// the company it rendered with, independent of the shared active-company
/// cookie. Optional/lenient: absent or malformed values fall back to the cookie.
#[derive(Deserialize, Default)]
struct ChatScope {
    company: Option<String>,
}

impl ChatScope {
    fn id(&self) -> Option<Uuid> {
        self.company.as_deref().and_then(|s| Uuid::parse_str(s.trim()).ok())
    }
}

/// Resolve the company for a chat request: honor an explicit per-tab company
/// when the user is a member of it (same membership gate the cookie uses), else
/// fall back to the shared active-company cookie. This is purely additive — a
/// request without `?company=` behaves exactly as before.
async fn chat_company(
    state: &AppState,
    jar: &CookieJar,
    explicit: Option<Uuid>,
    user_id: Uuid,
) -> Option<Uuid> {
    if let Some(uuid) = explicit {
        if queries::user_has_membership(&state.pool, user_id, uuid)
            .await
            .unwrap_or(false)
        {
            return Some(uuid);
        }
    }
    active_company(state, jar, user_id).await
}

fn build_active_company_cookie(state: &AppState, company_id: Uuid) -> Cookie<'static> {
    Cookie::build((ACTIVE_COMPANY_COOKIE, company_id.to_string()))
        .http_only(true)
        .secure(state.config.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::days(state.config.session_ttl_days))
        .build()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn root(State(state): State<AppState>, jar: CookieJar) -> Response {
    if current_user(&state, &jar).await.is_some() {
        Redirect::to("/app/dashboard").into_response()
    } else {
        Redirect::to("/login").into_response()
    }
}

async fn login_form(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = current_user(&state, &jar).await;
    render(LoginTpl {
        user_email: user.map(|u| u.email),
        flash: None,
        flash_kind: None,
        nav: NavCtx::default(),
    })
}

#[derive(Deserialize)]
struct LoginForm {
    email: String,
    password: String,
}

async fn login_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(req): Form<LoginForm>,
) -> Response {
    let normalized = req.email.trim().to_lowercase();

    let row: Result<Option<(Uuid, String)>, sqlx::Error> = sqlx::query_as(
        "SELECT id, password_hash FROM auth_users WHERE email_normalized = $1 AND is_active = true",
    )
    .bind(&normalized)
    .fetch_optional(&state.pool)
    .await;

    let (id, hash) = match row {
        Ok(Some(v)) => v,
        Ok(None) => return render_login_err("invalid email or password"),
        Err(e) => {
            tracing::error!(error = %e, "login query failed");
            return render_login_err("internal error");
        }
    };

    let ok = verify_password(&req.password, &hash).unwrap_or(false);
    if !ok {
        return render_login_err("invalid email or password");
    }

    match create_session(&state.pool, id, state.config.session_ttl_days, None).await {
        Ok((_, token)) => {
            let cookie = build_session_cookie(&state, token);
            (jar.add(cookie), Redirect::to("/app/dashboard")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "session create failed");
            render_login_err("internal error")
        }
    }
}

fn render_login_err(msg: &str) -> Response {
    render(LoginTpl {
        user_email: None,
        flash: Some(msg.into()),
        flash_kind: Some("err".into()),
        nav: NavCtx::default(),
    })
}

async fn signup_form(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = current_user(&state, &jar).await;
    render(SignupTpl {
        user_email: user.map(|u| u.email),
        flash: None,
        flash_kind: None,
        nav: NavCtx::default(),
    })
}

#[derive(Deserialize)]
struct SignupForm {
    email: String,
    password: String,
    name: Option<String>,
}

async fn signup_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(req): Form<SignupForm>,
) -> Response {
    let email = req.email.trim().to_string();
    let normalized = email.to_lowercase();
    if !email.contains('@') {
        return render_signup_err("valid email required");
    }
    if req.password.len() < 8 {
        return render_signup_err("password must be at least 8 characters");
    }

    let pw_hash = match hash_password(&req.password) {
        Ok(h) => h,
        Err(_) => return render_signup_err("internal error"),
    };

    let mut tx = match state.pool.begin().await {
        Ok(t) => t,
        Err(e) => {
            tracing::error!(error = %e, "tx begin failed");
            return render_signup_err("internal error");
        }
    };

    let user_row: Result<(Uuid,), sqlx::Error> = sqlx::query_as(
        "INSERT INTO auth_users (email, email_normalized, password_hash, name) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(&email)
    .bind(&normalized)
    .bind(&pw_hash)
    .bind(req.name.as_deref())
    .fetch_one(&mut *tx)
    .await;

    let user_id = match user_row {
        Ok((id,)) => id,
        Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("23505") => {
            return render_signup_err("email already registered — try logging in");
        }
        Err(e) => {
            tracing::error!(error = %e, "signup insert failed");
            return render_signup_err("internal error");
        }
    };

    // Auto-create personal company + owner membership.
    let display = req
        .name
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(|| email.split('@').next().unwrap_or("Personal").to_string());
    let slug_base = slugify(&display);
    let slug = format!("{}-{}", slug_base, &Uuid::new_v4().simple().to_string()[..8]);

    let company_row: Result<(Uuid,), sqlx::Error> = sqlx::query_as(
        "INSERT INTO companies (slug, name, owner_user_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&slug)
    .bind(&display)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await;

    let company_id = match company_row {
        Ok((id,)) => id,
        Err(e) => {
            tracing::error!(error = %e, "company create failed");
            return render_signup_err("internal error");
        }
    };

    let _ = sqlx::query(
        "INSERT INTO memberships (user_id, company_id, role) VALUES ($1, $2, 'owner')",
    )
    .bind(user_id)
    .bind(company_id)
    .execute(&mut *tx)
    .await;

    if let Err(e) = tx.commit().await {
        tracing::error!(error = %e, "signup commit failed");
        return render_signup_err("internal error");
    }

    match create_session(&state.pool, user_id, state.config.session_ttl_days, None).await {
        Ok((_, token)) => {
            let cookie = build_session_cookie(&state, token);
            (jar.add(cookie), Redirect::to("/app/dashboard")).into_response()
        }
        Err(_) => Redirect::to("/login").into_response(),
    }
}

fn render_signup_err(msg: &str) -> Response {
    render(SignupTpl {
        user_email: None,
        flash: Some(msg.into()),
        flash_kind: Some("err".into()),
        nav: NavCtx::default(),
    })
}

async fn logout_submit(State(state): State<AppState>, jar: CookieJar) -> Response {
    if let Some(c) = jar.get(SESSION_COOKIE_NAME) {
        let _ = delete_session(&state.pool, c.value()).await;
    }
    let jar = jar.remove(Cookie::from(SESSION_COOKIE_NAME));
    (jar, Redirect::to("/login")).into_response()
}

fn build_session_cookie(state: &AppState, token: String) -> Cookie<'static> {
    Cookie::build((SESSION_COOKIE_NAME, token))
        .http_only(true)
        .secure(state.config.cookie_secure)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::days(state.config.session_ttl_days))
        .build()
}

fn slugify(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last_dash = true;
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let trimmed = out.trim_matches('-');
    let slugged = if trimmed.is_empty() { "user" } else { trimmed };
    slugged.chars().take(31).collect()
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

async fn accounts_list(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let accounts = queries::list_accounts(&state.pool, company_id)
        .await
        .unwrap_or_default();
    let form_code = queries::company_form_code(&state.pool, company_id).await;
    let tax_line_options = queries::tax_line_options(&form_code);
    render(AccountsListTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        accounts,
        tax_line_options,
        form_code,
    })
}

#[derive(serde::Deserialize)]
struct TaxLineForm {
    tax_line: String,
}

#[derive(serde::Deserialize)]
struct RenameForm {
    name: String,
}

#[derive(serde::Deserialize)]
struct MemoForm {
    memo: String,
    redirect: Option<String>,
}

/// POST /app/entries/{entry_id}/memo — correct an entry's memo (event-sourced).
async fn entry_memo_update(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
    Form(req): Form<MemoForm>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let entry_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad entry id").into_response(),
    };
    match update_entry_memo(&state.pool, company_id, user.id, entry_id, &req.memo).await {
        Ok(_) => {
            let dest = req
                .redirect
                .filter(|r| r.starts_with("/app/"))
                .unwrap_or_else(|| format!("/app/entries/{id}"));
            Redirect::to(&dest).into_response()
        }
        Err(AppError::BadRequest(m)) => (StatusCode::BAD_REQUEST, m).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "memo update failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "could not update memo").into_response()
        }
    }
}

/// POST /app/accounts/{id}/rename — rename an account (pencil edit).
async fn account_rename(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
    Form(req): Form<RenameForm>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let account_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad account id").into_response(),
    };
    match rename_account(&state.pool, company_id, user.id, account_id, &req.name).await {
        Ok(_) => Redirect::to("/app/accounts").into_response(),
        Err(AppError::BadRequest(msg)) => (StatusCode::BAD_REQUEST, msg).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "rename account failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "could not rename").into_response()
        }
    }
}

/// POST /app/accounts/{id}/tax-line — set an account's tax-line tag (HTMX).
/// Returns a small fragment for the row's status cell.
async fn account_set_tax_line(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
    Form(req): Form<TaxLineForm>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let account_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad account id").into_response(),
    };
    match queries::set_account_tax_line(&state.pool, company_id, account_id, &req.tax_line).await {
        Ok(label) => {
            // `label` is from the fixed tax_line_options vocabulary (no user HTML).
            let cls = if label == "Unassigned" { "muted" } else { "tax-saved" };
            Html(format!(
                r#"<span class="{cls}" title="Saved — status: override">✓ {label}</span>"#
            ))
            .into_response()
        }
        Err(e) => {
            tracing::error!(error = ?e, "set tax line failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "could not save").into_response()
        }
    }
}

#[derive(Template)]
#[template(path = "vendors.html")]
struct VendorsTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    vendors: Vec<queries::VendorRow>,
    total_booked: String,
    missing_w8: usize,
    selected_sort: String,
}

#[derive(serde::Deserialize)]
struct VendorSortQuery {
    sort: Option<String>,
}

async fn vendors_list(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<VendorSortQuery>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let sort = match q.sort.as_deref() {
        Some(s @ ("paid_asc" | "name" | "first_desc" | "first_asc" | "last_desc" | "last_asc"
            | "txns_desc")) => s.to_string(),
        _ => "paid_desc".to_string(),
    };
    let vendors = queries::list_vendors_with_totals(&state.pool, company_id, &sort)
        .await
        .unwrap_or_default();
    let total_booked = queries::format_cents(vendors.iter().map(|v| v.booked_cents).sum());
    let missing_w8 = vendors.iter().filter(|v| v.tx_count > 0 && !v.tax_form_on_file).count();
    render(VendorsTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        vendors,
        total_booked,
        missing_w8,
        selected_sort: sort,
    })
}

async fn account_new(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    render(AccountsNewTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
    })
}

#[derive(Deserialize)]
struct AccountForm {
    account_number: String,
    name: String,
    account_type: String,
    currency: Option<String>,
    description: Option<String>,
}

async fn account_create(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(req): Form<AccountForm>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };

    let acct_type = match req.account_type.as_str() {
        "asset" => EventAccountType::Asset,
        "liability" => EventAccountType::Liability,
        "equity" => EventAccountType::Equity,
        "revenue" => EventAccountType::Revenue,
        "expense" => EventAccountType::Expense,
        _ => return account_form_err(&user.email, "invalid account type"),
    };

    let input = CreateAccountInput {
        account_type: acct_type,
        account_number: req.account_number.trim().to_string(),
        name: req.name.trim().to_string(),
        currency: req
            .currency
            .map(|c| c.trim().to_uppercase())
            .filter(|c| !c.is_empty()),
        description: req.description.filter(|d| !d.is_empty()),
    };

    match create_account(&state.pool, company_id, user.id, input).await {
        Ok(_) => Redirect::to("/app/accounts").into_response(),
        Err(AppError::Conflict(msg)) | Err(AppError::BadRequest(msg)) => {
            account_form_err(&user.email, &msg)
        }
        Err(e) => {
            tracing::error!(error = ?e, "account create failed");
            account_form_err(&user.email, "internal error")
        }
    }
}

fn account_form_err(email: &str, msg: &str) -> Response {
    render(AccountsNewTpl {
        user_email: Some(email.to_string()),
        flash: Some(msg.into()),
        flash_kind: Some("err".into()),
        nav: NavCtx::default(),
    })
}

// ---------------------------------------------------------------------------
// Entries
// ---------------------------------------------------------------------------

async fn entries_list(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let entries = queries::list_entries(&state.pool, company_id)
        .await
        .unwrap_or_default();
    render(EntriesListTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        entries,
    })
}

async fn entry_new(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let accounts = queries::list_accounts(&state.pool, company_id)
        .await
        .unwrap_or_default();
    render(EntriesNewTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        accounts,
        today: Utc::now().date_naive().to_string(),
    })
}

async fn entry_line_fragment(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let accounts = queries::list_accounts(&state.pool, company_id)
        .await
        .unwrap_or_default();
    render(EntryLineFragmentTpl { accounts })
}

async fn entry_create(
    State(state): State<AppState>,
    jar: CookieJar,
    body: String,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };

    // Parse repeating fields manually — serde_urlencoded doesn't collect repeats into Vec.
    let parsed: Vec<(String, String)> = url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect();
    let mut date = String::new();
    let mut memo = String::new();
    let mut reference: Option<String> = None;
    let mut account_ids: Vec<String> = Vec::new();
    let mut amounts: Vec<String> = Vec::new();
    let mut currencies: Vec<String> = Vec::new();
    for (k, v) in parsed {
        match k.as_str() {
            "date" => date = v,
            "memo" => memo = v,
            "reference" => {
                if !v.is_empty() {
                    reference = Some(v);
                }
            }
            "account_id" => account_ids.push(v),
            "amount" => amounts.push(v),
            "currency" => currencies.push(v),
            _ => {}
        }
    }
    let date_parsed = match NaiveDate::parse_from_str(&date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return entry_form_err(&state, &user, "invalid date").await,
    };

    if account_ids.is_empty()
        || account_ids.len() != amounts.len()
        || account_ids.len() != currencies.len()
    {
        return entry_form_err(&state, &user, "incomplete lines").await;
    }

    let mut lines: Vec<EntryLineInput> = Vec::with_capacity(account_ids.len());
    for ((acct, amt), curr) in account_ids
        .iter()
        .zip(amounts.iter())
        .zip(currencies.iter())
    {
        let acct_uuid = match Uuid::parse_str(acct) {
            Ok(u) => u,
            Err(_) => return entry_form_err(&state, &user, "invalid account").await,
        };
        let amount_cents = match parse_amount_cents(amt) {
            Some(c) => c,
            None => return entry_form_err(&state, &user, "invalid amount").await,
        };
        lines.push(EntryLineInput {
            account_id: acct_uuid,
            amount: amount_cents,
            currency: curr.trim().to_uppercase(),
            memo: None,
        });
    }

    let input = PostEntryInput {
        date: date_parsed,
        memo: memo.trim().to_string(),
        reference,
        lines,
    };

    match post_entry(&state.pool, company_id, user.id, input).await {
        Ok(_) => Redirect::to("/app/entries").into_response(),
        Err(AppError::BadRequest(msg)) | Err(AppError::Conflict(msg)) => {
            entry_form_err(&state, &user, &msg).await
        }
        Err(e) => {
            tracing::error!(error = ?e, "post_entry failed");
            entry_form_err(&state, &user, "internal error").await
        }
    }
}

fn parse_amount_cents(s: &str) -> Option<i64> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return None;
    }
    let dec: rust_decimal::Decimal = trimmed.parse().ok()?;
    let scaled = dec * rust_decimal::Decimal::from(100i64);
    use rust_decimal::prelude::ToPrimitive;
    scaled.round().to_i64()
}

async fn entry_form_err(state: &AppState, user: &AuthenticatedUser, msg: &str) -> Response {
    let company_id = match queries::resolve_company_id(&state.pool, user.id).await {
        Ok(Some(c)) => c,
        _ => return forbidden(),
    };
    let accounts = queries::list_accounts(&state.pool, company_id)
        .await
        .unwrap_or_default();
    render(EntriesNewTpl {
        user_email: Some(user.email.clone()),
        flash: Some(msg.into()),
        flash_kind: Some("err".into()),
        nav: NavCtx::default(),
        accounts,
        today: Utc::now().date_naive().to_string(),
    })
}

// ---------------------------------------------------------------------------
// AI chat
// ---------------------------------------------------------------------------

fn render_chat_message(m: &crate::ai::chat::StoredMessage) -> ChatGroup {
    let mut lines: Vec<ChatLine> = Vec::new();
    let role = m.role.clone();
    match &m.content {
        serde_json::Value::String(s) => {
            lines.push(ChatLine { role: role.clone(), body: s.clone() });
        }
        serde_json::Value::Array(blocks) => {
            for b in blocks {
                let typ = b.get("type").and_then(|v| v.as_str()).unwrap_or("");
                match typ {
                    "text" => {
                        let txt = b.get("text").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if !txt.is_empty() {
                            lines.push(ChatLine { role: role.clone(), body: txt });
                        }
                    }
                    // Tool calls and their results are internal bookkeeping
                    // operations — never surfaced in the chat transcript.
                    "tool_use" | "tool_result" => {}
                    _ => {}
                }
            }
        }
        _ => {}
    }
    ChatGroup { lines }
}

async fn chat_view(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(scope): axum::extract::Query<ChatScope>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match chat_company(&state, &jar, scope.id(), user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let history = crate::ai::chat::load_history(&state.pool, user.id, company_id)
        .await
        .unwrap_or_default();
    let messages: Vec<ChatGroup> = history.iter().map(render_chat_message).collect();
    render(ChatTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        messages,
    })
}

#[derive(Deserialize)]
struct ChatForm {
    message: String,
}

async fn chat_send(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(scope): axum::extract::Query<ChatScope>,
    Form(req): Form<ChatForm>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match chat_company(&state, &jar, scope.id(), user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let text = req.message.trim().to_string();
    if text.is_empty() {
        return Redirect::to("/app/chat").into_response();
    }
    // Turns run on the company's persistent Claude CLI session via accountir-agentd.
    if let Err(e) = crate::ai::agent::send_turn(&state.pool, user.id, company_id, text).await {
        tracing::error!(error = ?e, "agent chat turn failed");
    }
    Redirect::to("/app/chat").into_response()
}

/// SSE variant: relays the agent's stream-json events to the browser live,
/// then persists the conversation exactly like the form path.
async fn chat_stream(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(scope): axum::extract::Query<ChatScope>,
    Form(req): Form<ChatForm>,
) -> Response {
    use axum::response::sse::{Event, KeepAlive, Sse};
    use tokio_stream::StreamExt;

    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match chat_company(&state, &jar, scope.id(), user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let text = req.message.trim().to_string();
    if text.is_empty() {
        return (StatusCode::BAD_REQUEST, "empty message").into_response();
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
    let pool = state.pool.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::ai::agent::stream_turn(&pool, user.id, company_id, text, tx).await {
            tracing::error!(error = ?e, "agent stream turn failed");
        }
        // tx drops here -> stream ends -> browser sees the SSE close.
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx)
        .map(|v| Ok::<_, std::convert::Infallible>(Event::default().data(v.to_string())));
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

/// POST /app/chat/upload — give the agent a file to look at. The extracted
/// text goes to the agent as the turn body; history just records the upload.
/// Streams the agent's reply exactly like /app/chat/stream.
async fn chat_upload(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(scope): axum::extract::Query<ChatScope>,
    mut multipart: axum::extract::Multipart,
) -> Response {
    use axum::response::sse::{Event, KeepAlive, Sse};
    use tokio_stream::StreamExt;

    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match chat_company(&state, &jar, scope.id(), user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let fname = field.file_name().unwrap_or("file").to_string();
            match field.bytes().await {
                Ok(b) => {
                    if !b.is_empty() {
                        files.push((fname, b.to_vec()));
                    }
                }
                Err(e) => {
                    return (StatusCode::BAD_REQUEST, format!("upload failed: {e}")).into_response()
                }
            }
        }
    }
    if files.is_empty() {
        return (StatusCode::BAD_REQUEST, "no files in upload").into_response();
    }

    if files.len() > 100 {
        return (StatusCode::BAD_REQUEST, "at most 100 files per upload").into_response();
    }

    // Retain the original bytes of everything dropped into chat, deduped, so
    // the agent's source documents are kept for reference on the Files page.
    // Capture each file's id so the agent can re-file it under another entity
    // (the owner's personal session can move docs to the company they belong to).
    let mut stored_ids: Vec<(String, Option<Uuid>)> = Vec::new();
    for (fname, bytes) in &files {
        let ctype = if bytes.starts_with(b"%PDF") {
            "application/pdf"
        } else if bytes.starts_with(b"\x89PNG") {
            "image/png"
        } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            "image/jpeg"
        } else if bytes.starts_with(b"GIF8") {
            "image/gif"
        } else {
            "text/plain"
        };
        let year = crate::file_store::detect_year(fname, None);
        let id = store_company_file(&state.pool, company_id, "chat", fname, ctype, bytes, year).await;
        stored_ids.push((fname.clone(), id));
    }

    // Respond with the SSE stream IMMEDIATELY and extract inside it: OCR of a
    // large scanned batch takes minutes, and the SSO proxy 502s any request
    // that hasn't produced response headers within ~30s. Progress events keep
    // the chat UI informed while extraction runs.
    let names: Vec<String> = files.iter().map(|(n, _)| n.clone()).collect();
    let display = format!("📎 Uploaded: {}", names.join(", "));
    // Only the owner's personal session can re-file a doc to another entity, so
    // only it gets the move_file routing hint.
    let is_personal: bool = sqlx::query_scalar("SELECT is_personal FROM companies WHERE id = $1")
        .bind(company_id)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten()
        .unwrap_or(false);
    let (tx, rx) = tokio::sync::mpsc::channel::<serde_json::Value>(64);
    let pool = state.pool.clone();
    tokio::spawn(async move {
        // One shared text budget across all files; an unreadable file becomes
        // a note for the agent rather than failing the batch.
        const TOTAL_BUDGET: usize = 300_000;
        let mut remaining = TOTAL_BUDGET;
        let mut sections = String::new();
        let mut extracted_any = false;
        let total = files.len();
        for (i, (fname, bytes)) in files.iter().enumerate() {
            let _ = tx
                .send(serde_json::json!({
                    "type": "upload_progress",
                    "note": format!("Reading {fname} ({} of {total})…", i + 1),
                }))
                .await;
            let is_image = bytes.starts_with(b"\x89PNG")
                || bytes.starts_with(&[0xFF, 0xD8, 0xFF])
                || bytes.starts_with(b"GIF8");
            let res: Result<String, String> = if bytes.starts_with(b"%PDF") {
                crate::plaid::statements::extract_text_or_ocr(bytes).await
            } else if is_image {
                crate::plaid::statements::ocr_image(bytes).await
            } else if bytes.contains(&0) {
                Err("unsupported binary format (only PDF, images, and text/CSV are readable)".to_string())
            } else {
                Ok(String::from_utf8_lossy(bytes).to_string())
            };
            match res {
                Ok(text) if !text.trim().is_empty() => {
                    let content: String = text.chars().take(remaining).collect();
                    let truncated = content.chars().count() < text.chars().count();
                    remaining = remaining.saturating_sub(content.chars().count());
                    extracted_any = true;
                    sections.push_str(&format!(
                        "=== File: \"{fname}\"{} ===\n{content}\n=== End of \"{fname}\" ===\n\n",
                        if truncated { " (truncated — text budget exhausted)" } else { "" }
                    ));
                }
                Ok(_) => sections.push_str(&format!(
                    "=== File: \"{fname}\" — no text could be extracted ===\n\n"
                )),
                Err(e) => sections
                    .push_str(&format!("=== File: \"{fname}\" — could not read: {e} ===\n\n")),
            }
        }
        if !extracted_any {
            let _ = tx
                .send(serde_json::json!({
                    "type": "daemon_error",
                    "error": "no text could be extracted from any of the files (only PDF and text/CSV are supported)",
                }))
                .await;
            return;
        }
        let _ = tx
            .send(serde_json::json!({
                "type": "upload_progress",
                "note": format!("All {total} file(s) read — the agent is looking at them…"),
            }))
            .await;
        // Manifest: tell the agent the retained file ids so it can re-file a
        // document under the entity it belongs to (personal/owner session only).
        let manifest = if is_personal {
            let lines: String = stored_ids
                .iter()
                .filter_map(|(n, id)| id.map(|i| format!("  - file_id {i} = \"{n}\"\n")))
                .collect();
            if lines.is_empty() {
                String::new()
            } else {
                format!(
                    "\n[These files are retained under THIS personal entity:\n{lines}\
                     If a document clearly belongs to one of your other entities, (1) call move_file \
                     with its file_id and to_entity to file the original under that entity, and (2) post \
                     its accounting there using the entity parameter. State which entity you filed each under.]\n"
                )
            }
        } else {
            String::new()
        };
        let agent_text = format!(
            "[The user uploaded {total} file(s) in chat: {}. Their extracted text follows.]\n\n{sections}{manifest}\
             [End of uploaded files. Briefly say what each file appears to contain, then ask what the \
             user would like done with them — unless their intent is already clear from the conversation.]",
            names.join(", "),
        );
        if let Err(e) = crate::ai::agent::stream_turn_with_display(
            &pool, user.id, company_id, agent_text, display, tx,
        )
        .await
        {
            tracing::error!(error = ?e, "agent upload turn failed");
        }
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx)
        .map(|v| Ok::<_, std::convert::Infallible>(Event::default().data(v.to_string())));
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

/// JSON history for the floating chat widget: flat [{role, body}] lines.
async fn chat_history(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(scope): axum::extract::Query<ChatScope>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match chat_company(&state, &jar, scope.id(), user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let history = crate::ai::chat::load_history(&state.pool, user.id, company_id)
        .await
        .unwrap_or_default();
    let lines: Vec<serde_json::Value> = history
        .iter()
        .map(render_chat_message)
        .flat_map(|g| g.lines)
        .map(|l| serde_json::json!({ "role": l.role, "body": l.body }))
        .collect();
    Json(serde_json::json!({ "lines": lines })).into_response()
}

/// POST /app/chat/stop — stop the in-flight agent turn for the active company.
async fn chat_stop(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(scope): axum::extract::Query<ChatScope>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match chat_company(&state, &jar, scope.id(), user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    crate::ai::agent::cancel_turn(company_id).await;
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn chat_clear_route(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(scope): axum::extract::Query<ChatScope>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match chat_company(&state, &jar, scope.id(), user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let _ = crate::ai::chat::clear_history(&state.pool, user.id, company_id).await;
    // Also rotate the agent session so the AI genuinely forgets the conversation.
    crate::ai::agent::reset_session(company_id).await;
    Redirect::to("/app/chat").into_response()
}

// ---------------------------------------------------------------------------
// Admin (companies + members)
// ---------------------------------------------------------------------------

async fn admin_companies(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let companies = queries::list_companies_for_user(&state.pool, user.id)
        .await
        .unwrap_or_default();
    let active_id = active_company(&state, &jar, user.id).await.unwrap_or_else(Uuid::nil);
    let active_company_name = companies
        .iter()
        .find(|c| c.id == active_id)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| "—".to_string());
    render(AdminCompaniesTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        companies,
        active_company_id: active_id,
        active_company_name,
    })
}

#[derive(Deserialize)]
struct CompanyForm {
    name: String,
}

async fn admin_company_create(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(req): Form<CompanyForm>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let name = req.name.trim();
    if name.is_empty() {
        return Redirect::to("/app/admin/companies").into_response();
    }
    let new_id = match queries::create_company(&state.pool, user.id, name).await {
        Ok(id) => id,
        Err(e) => {
            tracing::error!(error = ?e, "create company failed");
            return Redirect::to("/app/admin/companies").into_response();
        }
    };
    // Switch to the new company immediately.
    let cookie = build_active_company_cookie(&state, new_id);
    (jar.add(cookie), Redirect::to("/app/dashboard")).into_response()
}

async fn admin_company_switch(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let target = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Redirect::to("/app/admin/companies").into_response(),
    };
    if !queries::user_has_membership(&state.pool, user.id, target)
        .await
        .unwrap_or(false)
    {
        return forbidden();
    }
    let cookie = build_active_company_cookie(&state, target);
    (jar.add(cookie), Redirect::to("/app/dashboard")).into_response()
}

async fn admin_members(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let members = queries::list_members(&state.pool, company_id).await.unwrap_or_default();
    let active_company_name = lookup_company_name(&state, company_id).await;
    let role = queries::user_role_in(&state.pool, user.id, company_id).await.ok().flatten().unwrap_or_default();
    let can_admin = queries::role_can_admin(&role);
    let invitations = queries::list_invitations(&state.pool, company_id).await.unwrap_or_default();
    let nav = build_nav(&state, &jar, user.id).await;
    render(AdminMembersTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav,
        members,
        active_company_name,
        can_admin,
        invitations,
        invite_url_base: "http://127.0.0.1:9877".to_string(),
    })
}

async fn admin_member_remove(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(target_user_id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await { Some(c) => c, None => return forbidden() };
    let role = queries::user_role_in(&state.pool, user.id, company_id).await.ok().flatten().unwrap_or_default();
    if !queries::role_can_admin(&role) { return forbidden(); }
    let target = match Uuid::parse_str(&target_user_id) { Ok(u) => u, Err(_) => return (StatusCode::BAD_REQUEST, "bad user_id").into_response() };
    let _ = queries::remove_member(&state.pool, company_id, target).await;
    Redirect::to("/app/admin/members").into_response()
}

#[derive(Deserialize)]
struct ChangeRoleForm { role: String }

async fn admin_member_role(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(target_user_id): axum::extract::Path<String>,
    Form(req): Form<ChangeRoleForm>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await { Some(c) => c, None => return forbidden() };
    let role = queries::user_role_in(&state.pool, user.id, company_id).await.ok().flatten().unwrap_or_default();
    if !queries::role_can_admin(&role) { return forbidden(); }
    let target = match Uuid::parse_str(&target_user_id) { Ok(u) => u, Err(_) => return (StatusCode::BAD_REQUEST, "bad user_id").into_response() };
    let _ = queries::change_member_role(&state.pool, company_id, target, &req.role).await;
    Redirect::to("/app/admin/members").into_response()
}

async fn admin_settings_view(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await { Some(c) => c, None => return forbidden() };
    let role = queries::user_role_in(&state.pool, user.id, company_id).await.ok().flatten().unwrap_or_default();
    let can_edit = queries::role_can_admin(&role);
    let (name, base_currency, fysm) = queries::get_company(&state.pool, company_id).await
        .ok().flatten().unwrap_or_else(|| ("".into(), "USD".into(), 1));
    let nav = build_nav(&state, &jar, user.id).await;
    let active_id = nav.active_company_id.clone();
    let active_name = nav.active_company_name.clone();
    let all = nav.all_companies.clone();
    let sig = crate::signature::get_meta(&state.pool, user.id).await.ok().flatten();
    // Prefill the typed signature with the owner's name (fallback: profile legal name / email).
    let mut owner_name = user.name.clone().unwrap_or_default().trim().to_string();
    if owner_name.is_empty() {
        owner_name = crate::tax::get_profile(&state.pool, company_id)
            .await
            .ok()
            .flatten()
            .map(|p| p.legal_name)
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| user.email.clone());
    }
    render(AdminSettingsTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav,
        active_company_name: active_name,
        company_name: name,
        base_currency,
        fiscal_year_start_month: fysm,
        can_edit,
        all_companies: all,
        active_company_id: active_id,
        sig_fonts: crate::signature::FONTS.to_vec(),
        sig_has: sig.is_some(),
        sig_kind: sig.as_ref().map(|s| s.kind.clone()).unwrap_or_default(),
        sig_typed_text: sig
            .as_ref()
            .and_then(|s| s.typed_text.clone())
            .unwrap_or_else(|| owner_name.clone()),
        sig_typed_font: sig
            .as_ref()
            .and_then(|s| s.typed_font.clone())
            .unwrap_or_else(|| "GreatVibes".to_string()),
        sig_updated_at: sig.as_ref().map(|s| s.updated_at.clone()).unwrap_or_default(),
        owner_name,
    })
}

#[derive(Deserialize)]
struct SettingsForm {
    name: String,
    base_currency: String,
    fiscal_year_start_month: i16,
}

async fn admin_settings_save(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(req): Form<SettingsForm>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await { Some(c) => c, None => return forbidden() };
    let role = queries::user_role_in(&state.pool, user.id, company_id).await.ok().flatten().unwrap_or_default();
    if !queries::role_can_admin(&role) { return forbidden(); }
    let _ = queries::update_company_settings(&state.pool, company_id, &req.name, &req.base_currency, req.fiscal_year_start_month).await;
    Redirect::to("/app/admin/settings").into_response()
}

// --- Owner signature (per user, shared across the owner's entities) ---------

#[derive(Deserialize)]
struct TypedSignatureForm {
    text: String,
    font: String,
}

/// Save a typed signature: render `text` in the chosen handwriting `font` to a PNG.
async fn signature_save_typed(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(req): Form<TypedSignatureForm>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    match crate::signature::save_typed(&state.pool, user.id, &req.text, &req.font).await {
        Ok(()) => Redirect::to("/app/admin/settings").into_response(),
        Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    }
}

/// Upload a signature image (PNG/JPEG).
async fn signature_upload(
    State(state): State<AppState>,
    jar: CookieJar,
    mut multipart: axum::extract::Multipart,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let content_type = field.content_type().unwrap_or("").to_string();
            let Ok(bytes) = field.bytes().await else { continue };
            return match crate::signature::save_image(&state.pool, user.id, &bytes, &content_type).await {
                Ok(()) => Redirect::to("/app/admin/settings").into_response(),
                Err(e) => (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            };
        }
    }
    (StatusCode::BAD_REQUEST, "no file uploaded").into_response()
}

async fn signature_clear(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let _ = crate::signature::clear(&state.pool, user.id).await;
    Redirect::to("/app/admin/settings").into_response()
}

/// Serve the current owner's signature bitmap (for the settings preview).
async fn signature_preview(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    match crate::signature::get_image(&state.pool, user.id).await {
        Ok(Some((bytes, ct))) => (
            [
                (axum::http::header::CONTENT_TYPE, ct),
                (axum::http::header::CACHE_CONTROL, "no-store".to_string()),
            ],
            bytes,
        )
            .into_response(),
        _ => (StatusCode::NOT_FOUND, "no signature").into_response(),
    }
}

/// Serve a handwriting font file so the browser can preview typed signatures live.
async fn signature_font(
    State(_state): State<AppState>,
    axum::extract::Path(key): axum::extract::Path<String>,
) -> Response {
    let key = key.strip_suffix(".ttf").unwrap_or(&key);
    if !crate::signature::is_valid_font(key) {
        return (StatusCode::NOT_FOUND, "unknown font").into_response();
    }
    let dir = std::env::var("FONTS_DIR").unwrap_or_else(|_| "/usr/local/lib/accountir/fonts".to_string());
    match std::fs::read(format!("{dir}/{key}.ttf")) {
        Ok(bytes) => (
            [
                (axum::http::header::CONTENT_TYPE, "font/ttf".to_string()),
                (axum::http::header::CACHE_CONTROL, "public, max-age=86400".to_string()),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "font file missing").into_response(),
    }
}

#[derive(Deserialize)]
struct InvitationForm { role: String }

async fn admin_invitation_create(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(req): Form<InvitationForm>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await { Some(c) => c, None => return forbidden() };
    let role = queries::user_role_in(&state.pool, user.id, company_id).await.ok().flatten().unwrap_or_default();
    if !queries::role_can_admin(&role) { return forbidden(); }
    let _ = queries::create_invitation(&state.pool, company_id, user.id, &req.role, 14).await;
    Redirect::to("/app/admin/members").into_response()
}

async fn accept_invite_view(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(token): axum::extract::Path<String>,
) -> Response {
    let row: Option<(Uuid, String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        "SELECT company_id, role::text, expires_at, accepted_at FROM company_invitations WHERE token = $1",
    )
    .bind(&token)
    .fetch_optional(&state.pool)
    .await
    .ok()
    .flatten();
    let (company_id, role, expires_at, accepted_at) = match row {
        Some(r) => r,
        None => return (StatusCode::NOT_FOUND, "invitation not found").into_response(),
    };
    if accepted_at.is_some() {
        return (StatusCode::OK, Html("<p>This invitation has already been used.</p>".to_string())).into_response();
    }
    if chrono::Utc::now() > expires_at {
        return (StatusCode::OK, Html("<p>This invitation has expired.</p>".to_string())).into_response();
    }
    let company_name = lookup_company_name(&state, company_id).await;
    let user = current_user(&state, &jar).await;
    let logged_in = user.is_some();
    let user_email = user.as_ref().map(|u| u.email.clone());
    let nav = if let Some(u) = &user { build_nav(&state, &jar, u.id).await } else { NavCtx::default() };
    render(AcceptInviteTpl {
        user_email,
        flash: None,
        flash_kind: None,
        nav: nav.clone(),
        company_name,
        role,
        token,
        logged_in,
        all_companies: nav.all_companies.clone(),
        active_company_id: nav.active_company_id.clone(),
        active_company_name: nav.active_company_name.clone(),
    })
}

async fn accept_invite_submit(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(token): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    match queries::accept_invitation(&state.pool, &token, user.id).await {
        Ok(company_id) => {
            let cookie = build_active_company_cookie(&state, company_id);
            (jar.add(cookie), Redirect::to("/app/dashboard")).into_response()
        }
        Err(e) => {
            tracing::warn!(error = ?e, "accept invitation failed");
            Redirect::to("/login").into_response()
        }
    }
}

#[derive(Deserialize)]
struct AddMemberForm {
    email: String,
    role: String,
}

async fn admin_member_add(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(req): Form<AddMemberForm>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let _ = queries::add_member_by_email(&state.pool, company_id, &req.email, &req.role).await;
    Redirect::to("/app/admin/members").into_response()
}

// ---------------------------------------------------------------------------
// Banks (Plaid)
// ---------------------------------------------------------------------------

async fn banks_list(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let items = queries::list_plaid_items(&state.pool, company_id)
        .await
        .unwrap_or_default();
    render(BanksListTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        items,
    })
}

async fn banks_link(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    render(BanksLinkTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
    })
}

#[derive(Deserialize)]
struct HistoricalQuery {
    start: Option<String>,
    end: Option<String>,
    min_dollars: Option<f64>,
    max_dollars: Option<f64>,
}

async fn banks_historical(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::extract::Query(q): axum::extract::Query<HistoricalQuery>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let item_uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad item id").into_response(),
    };

    use sqlx::Acquire;
    let mut conn = match state.pool.acquire().await { Ok(c) => c, Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "db").into_response() };
    let mut tx = match conn.begin().await { Ok(t) => t, Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "db").into_response() };
    if crate::store::event_store::set_tenant(&mut tx, company_id).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "tenant").into_response();
    }
    let row: Option<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
        "SELECT access_token_ciphertext, access_token_nonce FROM plaid_items WHERE id = $1",
    )
    .bind(item_uuid)
    .fetch_optional(&mut *tx)
    .await
    .ok()
    .flatten();
    let (ct, nonce) = match row { Some(r) => r, None => return (StatusCode::NOT_FOUND, "item").into_response() };
    let _ = tx.commit().await;

    let cipher = crate::plaid::crypto::TokenCipher::new(&state.config.plaid.token_enc_key);
    let access_token = match cipher.decrypt(&ct, &nonce) {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "decrypt").into_response(),
    };

    let today = Utc::now().date_naive();
    let start = q.start.as_deref().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| today - chrono::Duration::days(365 * 2));
    let end = q.end.as_deref().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(today);

    let plaid = crate::plaid::client::PlaidClient::new(state.config.plaid.clone());
    let txns = match plaid.transactions_get(&access_token, start, end).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "transactions_get failed");
            return (StatusCode::BAD_GATEWAY, format!("plaid: {e}")).into_response();
        }
    };

    let total = txns.len();
    let oldest = txns.iter().filter_map(|t| t.get("date").and_then(|v| v.as_str())).min().unwrap_or("").to_string();
    let newest = txns.iter().filter_map(|t| t.get("date").and_then(|v| v.as_str())).max().unwrap_or("").to_string();

    let matches: Vec<serde_json::Value> = if q.min_dollars.is_some() || q.max_dollars.is_some() {
        let min_c = q.min_dollars.map(|d| (d * 100.0).round() as i64);
        let max_c = q.max_dollars.map(|d| (d * 100.0).round() as i64);
        txns.iter()
            .filter(|t| {
                let amt = t.get("amount").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let cents = (amt * 100.0).round() as i64;
                let abs = cents.abs();
                min_c.map(|m| abs >= m).unwrap_or(true) && max_c.map(|m| abs <= m).unwrap_or(true)
            })
            .cloned()
            .collect()
    } else {
        Vec::new()
    };

    let body = serde_json::json!({
        "total_transactions": total,
        "oldest_date": oldest,
        "newest_date": newest,
        "matches": matches,
    });
    Json(body).into_response()
}

async fn banks_statements_check(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let item_uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad item id").into_response(),
    };

    use sqlx::Acquire;
    let mut conn = match state.pool.acquire().await { Ok(c) => c, Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "db").into_response() };
    let mut tx = match conn.begin().await { Ok(t) => t, Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "db").into_response() };
    if crate::store::event_store::set_tenant(&mut tx, company_id).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "tenant").into_response();
    }
    let row: Option<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
        "SELECT access_token_ciphertext, access_token_nonce FROM plaid_items WHERE id = $1",
    )
    .bind(item_uuid)
    .fetch_optional(&mut *tx).await.ok().flatten();
    let (ct, nonce) = match row { Some(r) => r, None => return (StatusCode::NOT_FOUND, "item").into_response() };
    let _ = tx.commit().await;

    let cipher = crate::plaid::crypto::TokenCipher::new(&state.config.plaid.token_enc_key);
    let access_token = match cipher.decrypt(&ct, &nonce) {
        Ok(t) => t,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "decrypt").into_response(),
    };

    let plaid = crate::plaid::client::PlaidClient::new(state.config.plaid.clone());
    match plaid.statements_list(&access_token).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (StatusCode::OK, Json(serde_json::json!({
            "statements_enabled": false,
            "error": format!("{e}"),
        }))).into_response(),
    }
}

/// Decrypt the Plaid access token for an item owned by the active company.
async fn item_access_token(
    state: &AppState,
    company_id: Uuid,
    item_uuid: Uuid,
) -> Result<String, Response> {
    use sqlx::Acquire;
    let mut conn = state
        .pool
        .acquire()
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "db").into_response())?;
    let mut tx = conn
        .begin()
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "db").into_response())?;
    crate::store::event_store::set_tenant(&mut tx, company_id)
        .await
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "tenant").into_response())?;
    let row: Option<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
        "SELECT access_token_ciphertext, access_token_nonce FROM plaid_items WHERE id = $1 AND company_id = $2",
    )
    .bind(item_uuid)
    .bind(company_id)
    .fetch_optional(&mut *tx)
    .await
    .ok()
    .flatten();
    let _ = tx.commit().await;
    let (ct, nonce) = row.ok_or_else(|| (StatusCode::NOT_FOUND, "item").into_response())?;
    let cipher = crate::plaid::crypto::TokenCipher::new(&state.config.plaid.token_enc_key);
    cipher
        .decrypt(&ct, &nonce)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "decrypt").into_response())
}

/// GET /app/banks/{id}/statements — list available Plaid statements with an import action.
async fn banks_statements(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let item_uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad item id").into_response(),
    };
    let access_token = match item_access_token(&state, company_id, item_uuid).await {
        Ok(t) => t,
        Err(r) => return r,
    };

    let plaid = crate::plaid::client::PlaidClient::new(state.config.plaid.clone());
    let listing = plaid.statements_list(&access_token).await;

    let mut rows = String::new();
    match &listing {
        Ok(v) => {
            if let Some(accounts) = v.get("accounts").and_then(|a| a.as_array()) {
                for acct in accounts {
                    let acct_name = acct
                        .get("account_name")
                        .and_then(|x| x.as_str())
                        .or_else(|| acct.get("official_name").and_then(|x| x.as_str()))
                        .unwrap_or("account");
                    if let Some(stmts) = acct.get("statements").and_then(|s| s.as_array()) {
                        for st in stmts {
                            let sid = st.get("statement_id").and_then(|x| x.as_str()).unwrap_or("");
                            let month = st.get("month").and_then(|x| x.as_i64()).unwrap_or(0);
                            let year = st.get("year").and_then(|x| x.as_i64()).unwrap_or(0);
                            rows.push_str(&format!(
                                "<tr><td>{acct}</td><td>{year}-{month:02}</td>\
                                 <td><form method=\"post\" action=\"/app/banks/{id}/statements/{sid}/import\" style=\"margin:0\">\
                                 <button type=\"submit\">Import &amp; parse</button></form></td></tr>",
                                acct = esc_html(acct_name),
                            ));
                        }
                    }
                }
            }
            if rows.is_empty() {
                rows.push_str("<tr><td colspan=3>No statements available for this item yet.</td></tr>");
            }
        }
        Err(e) => {
            rows.push_str(&format!(
                "<tr><td colspan=3>Statements not available: {}</td></tr>",
                esc_html(&format!("{e}"))
            ));
        }
    }

    // Must run inside a tenant-scoped transaction: statement_lines is FORCE-RLS and
    // set_config('app.company_id', …) is transaction-local, so querying the bare
    // pool leaves current_company_id() NULL and always returns 0.
    let parsed_count: i64 = {
        let mut val = 0i64;
        if let Ok(mut tx) = state.pool.begin().await {
            if crate::store::event_store::set_tenant(&mut tx, company_id).await.is_ok() {
                if let Ok(n) = sqlx::query_scalar::<_, i64>(
                    "SELECT count(*) FROM statement_lines WHERE company_id = $1 AND item_id = $2",
                )
                .bind(company_id)
                .bind(item_uuid)
                .fetch_one(&mut *tx)
                .await
                {
                    val = n;
                    let _ = tx.commit().await;
                }
            }
        }
        val
    };

    let page = format!(
        "<!doctype html><html><head><meta charset=utf-8><title>Statements</title>\
        <style>body{{font-family:system-ui,sans-serif;max-width:820px;margin:2rem auto;padding:0 1rem;color:#222}}\
        table{{width:100%;border-collapse:collapse;margin-top:1rem}}td,th{{padding:6px 8px;border-bottom:1px solid #eee;text-align:left}}\
        button{{cursor:pointer;padding:4px 10px}}a{{color:#2563eb}}</style></head><body>\
        <p><a href=\"/app/banks\">&larr; Banks</a></p><h1>Statements</h1>\
        <p>{parsed_count} parsed line(s) staged for this bank. Importing a statement downloads the PDF \
        and uses AI to extract its transactions (up to Plaid's 2-year statement window).</p>\
        <table><thead><tr><th>Account</th><th>Period</th><th></th></tr></thead><tbody>{rows}</tbody></table>\
        </body></html>"
    );
    Html(page).into_response()
}

/// POST /app/accounts/{id}/upload-statement — user uploads a statement file
/// (PDF or CSV/text); we AI-parse it and post the lines to that account,
/// merging with what's already in the ledger (same date + amount = duplicate).
async fn account_statement_upload(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
    mut multipart: axum::extract::Multipart,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let account_uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad account id").into_response(),
    };

    let mut file: Option<(String, Vec<u8>)> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("statement") {
            let fname = field.file_name().unwrap_or("statement").to_string();
            match field.bytes().await {
                Ok(b) => file = Some((fname, b.to_vec())),
                Err(e) => {
                    return (StatusCode::BAD_REQUEST, format!("upload failed: {e}")).into_response()
                }
            }
        }
    }
    let Some((fname, bytes)) = file else {
        return (StatusCode::BAD_REQUEST, "no statement file in upload").into_response();
    };
    if bytes.is_empty() {
        return (StatusCode::BAD_REQUEST, "uploaded file is empty").into_response();
    }

    let outcome = match crate::statement_upload::import_statement(
        &state.pool,
        company_id,
        user.id,
        account_uuid,
        &fname,
        &bytes,
    )
    .await
    {
        Ok(o) => o,
        Err(e) => {
            tracing::error!(error = %e, account = %account_uuid, "statement upload failed");
            return (StatusCode::UNPROCESSABLE_ENTITY, format!("statement import failed: {e}"))
                .into_response();
        }
    };
    tracing::info!(account = %account_uuid, file = %fname, parsed = outcome.parsed,
        imported = outcome.imported, duplicates = outcome.duplicates,
        unparsed = outcome.unparsed, "statement uploaded and imported");

    let page = format!(
        "<!doctype html><html><head><meta charset=utf-8><title>Statement imported</title>\
        <style>body{{font-family:system-ui,sans-serif;max-width:680px;margin:2rem auto;padding:0 1rem;color:#222}}\
        a{{color:#2563eb}}li{{margin:4px 0}}</style></head><body>\
        <p><a href=\"/app/accounts\">&larr; Chart of accounts</a></p><h1>Statement imported</h1>\
        <p>Parsed <strong>{fname}</strong> with AI:</p><ul>\
        <li><strong>{imported}</strong> new transaction(s) added</li>\
        <li><strong>{duplicates}</strong> duplicate(s) skipped (already in the ledger)</li>\
        <li><strong>{unparsed}</strong> line(s) ignored (no usable date/amount)</li></ul>\
        <p><a href=\"/app/transactions?account_id={acct}\">Review the account's transactions &rarr;</a></p>\
        </body></html>",
        fname = esc_html(&fname),
        imported = outcome.imported,
        duplicates = outcome.duplicates,
        unparsed = outcome.unparsed,
        acct = account_uuid,
    );
    Html(page).into_response()
}

/// POST /app/banks/{id}/statements/{statement_id}/import — download, parse, stage.
async fn banks_statement_import(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path((id, statement_id)): axum::extract::Path<(String, String)>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let item_uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad item id").into_response(),
    };

    let access_token = match item_access_token(&state, company_id, item_uuid).await {
        Ok(t) => t,
        Err(r) => return r,
    };
    let plaid = crate::plaid::client::PlaidClient::new(state.config.plaid.clone());

    let pdf = match plaid.statements_download(&access_token, &statement_id).await {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("download failed: {e}")).into_response(),
    };
    let text = match crate::plaid::statements::extract_text_or_ocr(&pdf).await {
        Ok(t) => t,
        Err(e) => return (StatusCode::UNPROCESSABLE_ENTITY, format!("pdf parse failed: {e}")).into_response(),
    };
    let lines = match crate::plaid::statements::parse_with_ai(&text).await {
        Ok(l) => l,
        Err(e) => return (StatusCode::BAD_GATEWAY, format!("ai parse failed: {e}")).into_response(),
    };

    let mut n = 0i64;
    if let Ok(mut conn) = state.pool.acquire().await {
        // Re-import is idempotent: drop any prior parse of this statement first.
        let _ = sqlx::query(
            "DELETE FROM statement_lines WHERE company_id=$1 AND item_id=$2 AND statement_id=$3",
        )
        .bind(company_id)
        .bind(item_uuid)
        .bind(&statement_id)
        .execute(&mut *conn)
        .await;
        for l in &lines {
            let d = NaiveDate::parse_from_str(&l.date, "%Y-%m-%d").ok();
            let res = sqlx::query(
                "INSERT INTO statement_lines (id, company_id, item_id, statement_id, txn_date, description, amount_cents) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7)",
            )
            .bind(Uuid::new_v4())
            .bind(company_id)
            .bind(item_uuid)
            .bind(&statement_id)
            .bind(d)
            .bind(&l.description)
            .bind(l.amount_cents)
            .execute(&mut *conn)
            .await;
            if res.is_ok() {
                n += 1;
            }
        }
    }
    tracing::info!(statement_id = %statement_id, lines = n, "statement parsed and staged");
    Redirect::to(&format!("/app/banks/{id}/statements")).into_response()
}

/// Minimal HTML escaping for interpolated, untrusted strings.
fn esc_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

async fn banks_unlink(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let item_uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad item id").into_response(),
    };
    // Delete CASCADEs plaid_local_accounts + plaid_imported_transactions + plaid_staged.
    // We don't try to call /item/remove on Plaid here — best-effort local cleanup.
    // Must run under tenant context: plaid_items is FORCE-RLS and the tenant is set
    // transaction-locally, so a bare-pool DELETE matches no rows and silently no-ops.
    if let Ok(mut tx) = state.pool.begin().await {
        if crate::store::event_store::set_tenant(&mut tx, company_id).await.is_ok() {
            let _ = sqlx::query("DELETE FROM plaid_items WHERE id = $1 AND company_id = $2")
                .bind(item_uuid)
                .bind(company_id)
                .execute(&mut *tx)
                .await;
            let _ = tx.commit().await;
        }
    }
    Redirect::to("/app/banks").into_response()
}

async fn banks_provision(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let item_uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad item id").into_response(),
    };
    match crate::plaid::sync::provision_existing_item(&state, company_id, user.id, item_uuid).await {
        Ok(n) => {
            tracing::info!(item_id = %item_uuid, provisioned = n, "provision ok");
            Redirect::to("/app/banks").into_response()
        }
        Err(e) => {
            tracing::error!(error = ?e, "provision failed");
            Redirect::to("/app/banks").into_response()
        }
    }
}

async fn banks_sync(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let item_uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad item id").into_response(),
    };
    match crate::plaid::sync::run_sync_for_item(&state, company_id, user.id, item_uuid).await {
        Ok((imported, skipped)) => {
            tracing::info!(item_id = %item_uuid, imported, skipped, "sync ok");
            Redirect::to("/app/banks").into_response()
        }
        Err(e) => {
            tracing::error!(error = ?e, "sync failed");
            Redirect::to("/app/banks").into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// Transactions
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct TxFilterQuery {
    start: Option<String>,
    end: Option<String>,
    /// Repeated `account_id=` params (multi-select). Empty = all accounts.
    #[serde(default)]
    account_id: Vec<String>,
    source: Option<String>,
    search: Option<String>,
    #[serde(rename = "type")]
    direction: Option<String>,
    min_amount: Option<String>,
    max_amount: Option<String>,
    sort: Option<String>,
    vendor: Option<String>,
    category: Option<String>,
}

/// Prefix the address-book name in front of any known 0x… wallet address in a
/// memo, keeping the full address (e.g. "VeraLabs 0x0a3d30b5…ad8168" full).
/// Unknown addresses are left unchanged.
fn relabel_wallets(memo: &str, labels: &std::collections::HashMap<String, String>) -> String {
    let b = memo.as_bytes();
    let mut out = String::with_capacity(memo.len());
    let mut i = 0;
    while i < memo.len() {
        if b[i] == b'0' && i + 1 < memo.len() && (b[i + 1] | 0x20) == b'x' {
            let mut j = i + 2;
            while j < memo.len() && b[j].is_ascii_hexdigit() {
                j += 1;
            }
            let addr = &memo[i..j];
            if addr.len() >= 12 {
                match labels.get(&addr.to_lowercase()) {
                    Some(name) => out.push_str(&format!("{name} {addr}")),
                    None => out.push_str(addr),
                }
                i = j;
                continue;
            }
        }
        let ch = memo[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Tolerant dollar-amount parse for filter inputs ("100", "1,250.50", "$40").
fn parse_filter_amount_cents(s: &str) -> Option<i64> {
    let cleaned: String = s.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
    parse_amount_cents(&cleaned)
}

async fn transactions_list(
    State(state): State<AppState>,
    jar: CookieJar,
    axum_extra::extract::Query(q): axum_extra::extract::Query<TxFilterQuery>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let start = q.start.as_deref().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let end = q.end.as_deref().and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok());
    let account_ids: Vec<Uuid> = q
        .account_id
        .iter()
        .filter(|s| !s.is_empty())
        .filter_map(|s| Uuid::parse_str(s).ok())
        .collect();
    let source = q.source.as_deref().filter(|s| !s.is_empty()).map(str::to_string);
    let search = q.search.as_deref().filter(|s| !s.is_empty()).map(str::to_string);
    let direction = q
        .direction
        .as_deref()
        .filter(|s| *s == "debit" || *s == "credit")
        .map(str::to_string);
    let min_cents = q.min_amount.as_deref().and_then(parse_filter_amount_cents);
    let max_cents = q.max_amount.as_deref().and_then(parse_filter_amount_cents);
    let sort = match q.sort.as_deref() {
        Some(s @ ("date_asc" | "amount_desc" | "amount_asc")) => s.to_string(),
        _ => "date_desc".to_string(),
    };
    let vendor = q.vendor.as_deref().filter(|s| !s.is_empty()).map(str::to_string);
    let category = q.category.as_deref().filter(|s| !s.is_empty()).map(str::to_string);
    let filter = queries::TransactionFilter {
        start, end,
        account_ids: account_ids.clone(),
        source: source.clone(),
        search: search.clone(),
        include_void: false,
        direction: direction.clone(),
        min_cents,
        max_cents,
        sort: Some(sort.clone()),
        vendor: vendor.clone(),
        category: category.clone(),
    };
    let mut rows = queries::list_transactions(&state.pool, company_id, &filter)
        .await
        .unwrap_or_default();
    // Shorten wallet addresses and apply address-book names in the displayed memo.
    let labels: std::collections::HashMap<String, String> =
        queries::list_address_labels(&state.pool, company_id, None)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|l| (l.address, l.name))
            .collect();
    if !labels.is_empty() {
        for r in rows.iter_mut() {
            r.memo_display = relabel_wallets(&r.memo, &labels);
        }
    }
    let accounts = queries::list_accounts(&state.pool, company_id).await.unwrap_or_default();
    let categories = queries::list_entry_categories(&state.pool, company_id).await.unwrap_or_default();
    render(TransactionsTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        rows,
        accounts,
        sources: vec!["manual", "import", "recurring", "system", "plaid"],
        selected_account_ids: account_ids.iter().map(|u| u.to_string()).collect(),
        categories,
        selected_category: category,
        selected_source: source,
        search,
        start_str: start.map(|d| d.to_string()).unwrap_or_default(),
        end_str: end.map(|d| d.to_string()).unwrap_or_default(),
        selected_type: direction,
        min_amount_str: q.min_amount.unwrap_or_default(),
        max_amount_str: q.max_amount.unwrap_or_default(),
        selected_sort: sort,
    })
}

#[derive(Template)]
#[template(path = "entry_detail.html")]
struct EntryDetailTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    entry: queries::EntryDetail,
    accounts: Vec<queries::AccountRow>,
}

/// Full detail for one journal entry: all lines, on-chain provenance (tx hash +
/// explorer link + verification) for crypto entries, the source statement file for
/// imports, and a link to the vendor (address book).
async fn entry_detail_view(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(entry_id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let Ok(eid) = Uuid::parse_str(&entry_id) else {
        return (StatusCode::BAD_REQUEST, "invalid entry id").into_response();
    };

    let mut entry = match queries::get_entry_detail(&state.pool, company_id, eid).await {
        Ok(Some(e)) => e,
        Ok(None) => return (StatusCode::NOT_FOUND, "entry not found").into_response(),
        Err(e) => {
            tracing::error!("entry_detail: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, "error").into_response();
        }
    };

    // Apply address-book names to the memo for display.
    let labels: std::collections::HashMap<String, String> =
        queries::list_address_labels(&state.pool, company_id, None)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|l| (l.address, l.name))
            .collect();
    if !labels.is_empty() {
        entry.memo_display = relabel_wallets(&entry.memo, &labels);
    }
    // Resolve the from/to wallet addresses to their address-book names.
    for ep in [entry.from.as_mut(), entry.to.as_mut()].into_iter().flatten() {
        if let Some(w) = &ep.wallet {
            ep.wallet_name = labels.get(&w.to_lowercase()).cloned();
        }
    }

    let accounts = queries::list_accounts(&state.pool, company_id)
        .await
        .unwrap_or_default();
    render(EntryDetailTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        entry,
        accounts,
    })
}

/// Sniff a content type + sensible filename for a supporting-doc upload when the
/// browser doesn't supply one (e.g. an image pasted from the clipboard).
fn sniff_upload(bytes: &[u8], field_ct: &str, field_name: Option<&str>, idx: usize) -> (String, String) {
    let (ct, ext) = if bytes.starts_with(b"\x89PNG") {
        ("image/png", "png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        ("image/jpeg", "jpg")
    } else if bytes.starts_with(b"GIF8") {
        ("image/gif", "gif")
    } else if bytes.starts_with(b"%PDF") {
        ("application/pdf", "pdf")
    } else if !field_ct.is_empty() && field_ct != "application/octet-stream" {
        (field_ct, "bin")
    } else {
        ("application/octet-stream", "bin")
    };
    let name = field_name
        .filter(|n| !n.trim().is_empty())
        .map(|n| n.to_string())
        .unwrap_or_else(|| format!("pasted-{}.{ext}", idx + 1));
    (ct.to_string(), name)
}

/// POST /app/entries/{entry_id}/documents/upload — attach a supporting document
/// (file or pasted image) to a transaction. Stores the file and links it.
async fn entry_document_upload(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(entry_id): axum::extract::Path<String>,
    mut multipart: axum::extract::Multipart,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let entry_uuid = match Uuid::parse_str(&entry_id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad entry id").into_response(),
    };
    let mut linked = 0usize;
    let mut idx = 0usize;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() != Some("file") {
            continue;
        }
        let field_ct = field.content_type().unwrap_or("").to_string();
        let field_name = field.file_name().map(|s| s.to_string());
        let Ok(bytes) = field.bytes().await else { continue };
        if bytes.is_empty() {
            continue;
        }
        let (ct, fname) = sniff_upload(&bytes, &field_ct, field_name.as_deref(), idx);
        idx += 1;
        if let Some(file_id) =
            store_company_file(&state.pool, company_id, "tx-doc", &fname, &ct, &bytes, None).await
        {
            if crate::queries::link_entry_document(&state.pool, company_id, entry_uuid, file_id, "other")
                .await
                .is_ok()
            {
                linked += 1;
            }
        }
    }
    if linked == 0 {
        return (StatusCode::BAD_REQUEST, "no file uploaded").into_response();
    }
    Redirect::to(&format!("/app/entries/{entry_id}")).into_response()
}

#[derive(Deserialize)]
struct ReclassifyForm {
    new_account_id: String,
    /// Optional path to return to (e.g. the entry detail page). Defaults to the list.
    redirect: Option<String>,
}

async fn transaction_reclassify(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(line_id): axum::extract::Path<String>,
    Form(req): Form<ReclassifyForm>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let line_uuid = match Uuid::parse_str(&line_id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad line_id").into_response(),
    };
    let new_acct = match Uuid::parse_str(&req.new_account_id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad new_account_id").into_response(),
    };
    if let Err(e) = reassign_line(&state.pool, company_id, user.id, line_uuid, new_acct).await {
        tracing::error!(error = ?e, "reassign failed");
    }
    // Return to the entry detail when invoked from there; else the transactions list.
    let dest = req
        .redirect
        .filter(|r| r.starts_with("/app/"))
        .unwrap_or_else(|| "/app/transactions".to_string());
    Redirect::to(&dest).into_response()
}

#[derive(serde::Deserialize)]
struct CategorizeForm {
    category: String,
    #[serde(default)]
    redirect: Option<String>,
}

/// Set (or clear, if blank) the user category tag on a transaction (entry).
async fn transaction_categorize(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(entry_id): axum::extract::Path<String>,
    Form(req): Form<CategorizeForm>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let entry_uuid = match Uuid::parse_str(&entry_id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad entry_id").into_response(),
    };
    if let Err(e) = queries::set_entry_category(&state.pool, company_id, entry_uuid, &req.category).await {
        tracing::error!(error = ?e, "set category failed");
    }
    let dest = req.redirect.filter(|s| s.starts_with("/app/")).unwrap_or_else(|| "/app/transactions".to_string());
    Redirect::to(&dest).into_response()
}

async fn transactions_bulk_reclassify(
    State(state): State<AppState>,
    jar: CookieJar,
    body: String,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let parsed: Vec<(String, String)> = url::form_urlencoded::parse(body.as_bytes())
        .into_owned()
        .collect();
    let mut line_ids = Vec::new();
    let mut new_account_id: Option<String> = None;
    for (k, v) in parsed {
        match k.as_str() {
            "line_ids" => line_ids.push(v),
            "new_account_id" => new_account_id = Some(v),
            _ => {}
        }
    }
    let new_acct = match new_account_id.and_then(|s| Uuid::parse_str(&s).ok()) {
        Some(u) => u,
        None => return (StatusCode::BAD_REQUEST, "missing new_account_id").into_response(),
    };
    for line in line_ids {
        if let Ok(line_uuid) = Uuid::parse_str(&line) {
            let _ = reassign_line(&state.pool, company_id, user.id, line_uuid, new_acct).await;
        }
    }
    Redirect::to("/app/transactions").into_response()
}

async fn entry_void(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(entry_id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let entry_uuid = match Uuid::parse_str(&entry_id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad entry_id").into_response(),
    };
    let _ = void_entry(&state.pool, company_id, user.id, entry_uuid, "voided via UI".into()).await;
    Redirect::to("/app/transactions").into_response()
}

async fn entry_unvoid(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(entry_id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let entry_uuid = match Uuid::parse_str(&entry_id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad entry_id").into_response(),
    };
    let _ = unvoid_entry(&state.pool, company_id, user.id, entry_uuid, "unvoided via UI".into()).await;
    Redirect::to("/app/transactions").into_response()
}

// ---------------------------------------------------------------------------
// Tax filing pipeline (profile → review → pull → fill → approve → mail)
// ---------------------------------------------------------------------------

/// One row of the tax-filing stepper. `state` is "done" | "current" | "todo"
/// (pipeline progress); `viewing` is whether the user is currently looking at it.
struct TaxStepTpl {
    n: u8,
    title: &'static str,
    desc: &'static str,
    state: &'static str,
    viewing: bool,
}

/// `?step=N` lets the user move back and forth through the pipeline to view or
/// revisit any step, independent of where the pipeline actually is. The `c*`
/// params carry the result of a Compute action back for a flash message.
#[derive(serde::Deserialize)]
struct TaxStepQuery {
    step: Option<u8>,
    cform: Option<String>,
    cval: Option<f64>,
    cok: Option<u8>,
    cerr: Option<String>,
}

/// POST /app/tax/compute — run the OpenTax engine of record for the active
/// company (ledger + tax-line tags → reconciled lines), then bounce back to
/// step 4 with the result. Deterministic: no free-form agent involved.
async fn tax_compute(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    if active_company(&state, &jar, user.id).await.is_none() {
        return forbidden();
    }
    let nav = build_nav(&state, &jar, user.id).await;
    use chrono::Datelike;
    let year = Utc::now().date_naive().year() - 1;
    let key = crate::tax::bridge_entity_key(&nav.active_company_name);
    let target = match key {
        None => "/app/tax?step=4&cerr=no-mapping".to_string(),
        Some(k) => match tokio::task::spawn_blocking(move || crate::tax::compute_return(k, year)).await {
            Ok(Ok(r)) => format!(
                "/app/tax?step=4&cform={}&cval={:.2}&cok={}",
                r.form,
                r.computed,
                if r.reconciles { 1 } else { 0 }
            ),
            Ok(Err(_)) | Err(_) => "/app/tax?step=4&cerr=compute-failed".to_string(),
        },
    };
    Redirect::to(&target).into_response()
}

#[derive(Template)]
#[template(path = "tax_filing.html")]
struct TaxFilingTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    tax_steps: Vec<TaxStepTpl>,
    /// The step the user is currently viewing (1..=7), and its neighbours for
    /// the Back / Next navigation.
    view_step: u8,
    prev_step: u8,
    next_step: u8,
    view_title: String,
    tagged_accounts: i64,
    total_accounts: i64,
    can_compute: bool,
    /// What the Next button does: "form" (fill profile), "agent" (send
    /// next_prompt to the AI), "approve" (review filled PDFs), "done".
    next_kind: &'static str,
    next_label: String,
    next_prompt: String,
    profile_entity: String,
    profile_legal_name: String,
    profile_ein: String,
    profile_line1: String,
    profile_line2: String,
    profile_city: String,
    profile_state: String,
    profile_zip: String,
    forms: Vec<crate::tax::TaxFormRow>,
    lob_configured: bool,
    /// Any form awaiting signature (status = approved).
    has_approved: bool,
    /// The owner has a signature saved in Settings.
    has_signature: bool,
}

async fn tax_filing(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<TaxStepQuery>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let profile = crate::tax::get_profile(&state.pool, company_id).await.unwrap_or(None);
    let addr_field = |k: &str| -> String {
        profile
            .as_ref()
            .and_then(|p| p.address.get(k))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let (profile_line1, profile_line2, profile_city, profile_state, profile_zip) = (
        addr_field("line1"),
        addr_field("line2"),
        addr_field("city"),
        addr_field("state"),
        addr_field("zip"),
    );
    let forms = crate::tax::list_forms(&state.pool, company_id).await.unwrap_or_default();
    let nav = build_nav(&state, &jar, user.id).await;
    let (tagged_accounts, total_accounts) = queries::tagging_coverage(&state.pool, company_id).await;
    let can_compute = crate::tax::bridge_entity_key(&nav.active_company_name).is_some();

    // Flash from a Compute action (POST /app/tax/compute redirects back here).
    let (compute_flash, compute_kind) = if let Some(cerr) = q.cerr.as_deref() {
        (
            Some(match cerr {
                "no-mapping" => "No OpenTax mapping for this company yet.".to_string(),
                _ => "Engine compute failed — review the ledger and tax-line tags.".to_string(),
            }),
            Some("error".to_string()),
        )
    } else if let Some(cform) = q.cform.as_deref() {
        let ok = q.cok == Some(1);
        (
            Some(format!(
                "OpenTax computed {} from the ledger: {:.2}{}",
                cform,
                q.cval.unwrap_or(0.0),
                if ok { " — reconciles to the books ✓" } else { " — Δ does not reconcile, review" }
            )),
            Some(if ok { "success" } else { "error" }.to_string()),
        )
    } else {
        (None, None)
    };

    // Where are we in the pipeline? Steps the user can't do (books review,
    // pulling and filling forms, mailing) are delegated to the agent by the
    // Next button; steps that need a human (profile, approval) point at the UI.
    let profile_complete = profile.as_ref().is_some_and(|p| {
        !p.legal_name.trim().is_empty()
            && p.address.get("line1").and_then(|v| v.as_str()).is_some_and(|s| !s.trim().is_empty())
    });
    let has_status = |s: &str| forms.iter().any(|f| f.status == s);
    let all_mailed = !forms.is_empty() && forms.iter().all(|f| f.status == "mailed");
    use chrono::Datelike;
    let tax_year = Utc::now().date_naive().year() - 1;
    let entity_label = match profile.as_ref().map(|p| p.entity_type.as_str()).unwrap_or("") {
        "individual" => "individual / personal return (Form 1040)",
        "schedule_c" => "sole proprietorship / single-member LLC (Schedule C)",
        "s_corp" => "S corporation (Form 1120-S)",
        "partnership" => "partnership (Form 1065)",
        "c_corp" => "C corporation (Form 1120)",
        _ => "business",
    };
    let company_name = nav.active_company_name.clone();
    let (current, next_kind, next_label, next_prompt) = if !profile_complete {
        (1u8, "form", "Next: complete the tax profile".to_string(), String::new())
    } else if forms.is_empty() {
        (2, "agent", format!("Next: AI reviews the books & prepares {tax_year} forms"), format!(
            "Let's file my {tax_year} taxes for {company_name}. First, review the {tax_year} books \
             using the accounting protocol and tell me what you found or fixed. Then determine which \
             IRS forms a {entity_label} needs and pull them. \
             Compute every return with the OpenTax engine of record — run \
             `python3 tax/bridge/step4.py --entity <slug> --year {tax_year} --fill` (each account is \
             mapped to its tax line via the tax_account_lines tag store, editable on the Accounts page) \
             and confirm it reconciles to the book net; then fill each form line-for-line from that \
             engine output (tax/bridge/out/<entity>_{tax_year}_fill.json). Do not hand-compute the numbers. \
             Stop before approval — I will review and approve the filled forms on the Tax page myself."
        ))
    } else if has_status("fetched") {
        (4, "agent", "Next: AI completes the pulled forms".to_string(), format!(
            "Continue my {tax_year} tax filing for {company_name}: compute the return with the OpenTax \
             engine of record — run `python3 tax/bridge/step4.py --entity <slug> --year {tax_year} --fill`, \
             confirm it reconciles to the book net, and fill each already-pulled IRS form line-for-line \
             from that engine output (tax/bridge/out/<entity>_{tax_year}_fill.json). Do not hand-compute. \
             Stop before approval — I will review and approve the filled forms on the Tax page myself."
        ))
    } else if has_status("filled") {
        (5, "approve", "Next: review & approve the filled forms".to_string(), String::new())
    } else if has_status("approved") {
        (6, "sign", "Next: sign the approved forms".to_string(), String::new())
    } else if has_status("signed") {
        (7, "agent", "Next: AI mails the signed forms".to_string(), format!(
            "Mail my signed {tax_year} tax forms for {company_name} via Lob (certified mail), \
             and confirm the tracking details."
        ))
    } else if all_mailed {
        (8, "done", "Filing complete — all forms mailed".to_string(), String::new())
    } else {
        // Forms exist in mixed/unknown states; let the agent sort it out.
        (4, "agent", "Next: AI continues the filing".to_string(), format!(
            "Continue my {tax_year} tax filing for {company_name} from where it left off. \
             Stop before approval — I will approve the filled forms myself."
        ))
    };
    let step_defs: [(&'static str, &'static str); 7] = [
        ("Tax profile", "entity type, legal name, EIN, mailing address — form below"),
        ("Books review", "the AI runs the full accounting protocol (transfers, duplicates, credit cards) before any numbers are used"),
        ("Pull forms", "the AI downloads the official IRS PDFs it determines you need"),
        ("Complete", "the AI fills the forms from your ledger, line by line"),
        ("Approve", "you review each filled PDF below and click Approve — nothing mails without it"),
        ("Sign", "your signature (from Settings) is stamped onto every approved form"),
        ("Mail", "signed forms go out via Lob, certified, with delivery tracking"),
    ];
    // The user can view/revisit any step via ?step=N; default to where the
    // pipeline actually is. This is a VIEW cursor — it never changes form state.
    let view_step = q.step.filter(|s| (1..=7).contains(s)).unwrap_or(current);
    let prev_step = if view_step > 1 { view_step - 1 } else { 1 };
    let next_step = if view_step < 7 { view_step + 1 } else { 7 };
    let view_title = step_defs
        .get((view_step - 1) as usize)
        .map(|(t, _)| t.to_string())
        .unwrap_or_default();
    let tax_steps: Vec<TaxStepTpl> = step_defs
        .iter()
        .enumerate()
        .map(|(i, (title, desc))| {
            let n = (i + 1) as u8;
            TaxStepTpl {
                n,
                title,
                desc,
                state: if n < current { "done" } else if n == current { "current" } else { "todo" },
                viewing: n == view_step,
            }
        })
        .collect();

    render(TaxFilingTpl {
        user_email: Some(user.email),
        flash: compute_flash,
        flash_kind: compute_kind,
        nav,
        tax_steps,
        view_step,
        prev_step,
        next_step,
        view_title,
        tagged_accounts,
        total_accounts,
        can_compute,
        next_kind,
        next_label,
        next_prompt,
        profile_entity: profile.as_ref().map(|p| p.entity_type.clone()).unwrap_or_default(),
        profile_legal_name: profile.as_ref().map(|p| p.legal_name.clone()).unwrap_or_default(),
        profile_ein: profile.as_ref().map(|p| p.ein.clone()).unwrap_or_default(),
        profile_line1,
        profile_line2,
        profile_city,
        profile_state,
        profile_zip,
        has_approved: forms.iter().any(|f| f.status == "approved"),
        has_signature: crate::signature::has_signature(&state.pool, user.id).await,
        forms,
        lob_configured: crate::tax::lob::configured(),
    })
}

#[derive(Deserialize)]
struct TaxProfileForm {
    entity_type: String,
    legal_name: String,
    ein: String,
    line1: String,
    line2: Option<String>,
    city: String,
    state: String,
    zip: String,
}

async fn tax_profile_save(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(req): Form<TaxProfileForm>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let address = serde_json::json!({
        "line1": req.line1, "line2": req.line2.unwrap_or_default(),
        "city": req.city, "state": req.state, "zip": req.zip,
    });
    if let Err(e) = crate::tax::set_profile(
        &state.pool,
        company_id,
        &req.entity_type,
        &req.legal_name,
        &req.ein,
        &address,
    )
    .await
    {
        tracing::error!(error = ?e, "tax profile save failed");
    }
    Redirect::to("/app/tax").into_response()
}

/// POST /app/tax/profile/upload — upload an entity document (EIN letter,
/// articles of organization, prior return); AI-parse it and fill the tax
/// profile with whatever it states, keeping existing values for the rest.
async fn tax_profile_upload(
    State(state): State<AppState>,
    jar: CookieJar,
    mut multipart: axum::extract::Multipart,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let _ = user;

    let mut docs: Vec<(String, Vec<u8>)> = Vec::new();
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("document") {
            let fname = field.file_name().unwrap_or("document").to_string();
            match field.bytes().await {
                Ok(b) => {
                    if !b.is_empty() {
                        docs.push((fname, b.to_vec()));
                    }
                }
                Err(e) => {
                    return (StatusCode::BAD_REQUEST, format!("upload failed: {e}")).into_response()
                }
            }
        }
    }
    if docs.is_empty() {
        return (StatusCode::BAD_REQUEST, "no documents in upload").into_response();
    }
    // All documents feed one combined parse: later documents can fill what
    // earlier ones don't state (EIN letter + articles + prior return).
    let mut text = String::new();
    let mut extracted_any = false;
    for (fname, bytes) in &docs {
        let extracted = if bytes.starts_with(b"%PDF") {
            match crate::plaid::statements::extract_text_or_ocr(bytes).await {
                Ok(t) => {
                    extracted_any = extracted_any || !t.trim().is_empty();
                    t
                }
                Err(e) => format!("(could not read this PDF: {e})"),
            }
        } else if bytes.contains(&0) {
            "(unsupported binary format)".to_string()
        } else {
            extracted_any = true;
            String::from_utf8_lossy(bytes).to_string()
        };
        text.push_str(&format!("=== Document: \"{fname}\" ===\n{extracted}\n\n"));
    }
    if !extracted_any {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            "no text could be extracted from any of the documents",
        )
            .into_response();
    }

    let parsed = match crate::tax::parse_entity_document(&text).await {
        Ok(v) => v,
        Err(e) => {
            return (StatusCode::UNPROCESSABLE_ENTITY, format!("could not parse document: {e}"))
                .into_response()
        }
    };

    // Merge: only overwrite with values the document actually stated.
    let existing = crate::tax::get_profile(&state.pool, company_id).await.unwrap_or(None);
    let pick = |key: &str, old: String| -> String {
        parsed
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(str::to_string)
            .unwrap_or(old)
    };
    let entity_type = pick(
        "entity_type",
        existing.as_ref().map(|p| p.entity_type.clone()).unwrap_or_default(),
    );
    let legal_name = pick(
        "legal_name",
        existing.as_ref().map(|p| p.legal_name.clone()).unwrap_or_default(),
    );
    let ein = pick("ein", existing.as_ref().map(|p| p.ein.clone()).unwrap_or_default());
    let address = parsed
        .get("address")
        .filter(|a| a.is_object())
        .cloned()
        .or_else(|| existing.as_ref().map(|p| p.address.clone()))
        .unwrap_or_else(|| serde_json::json!({}));

    if entity_type.is_empty() && legal_name.is_empty() && ein.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            "the document didn't contain recognizable entity details (entity type, legal name, or EIN)",
        )
            .into_response();
    }
    // entity_type may legitimately still be unknown; profile save requires it,
    // so default to schedule_c only if nothing else is known yet.
    let entity_type = if entity_type.is_empty() { "schedule_c".to_string() } else { entity_type };
    if let Err(e) = crate::tax::set_profile(
        &state.pool,
        company_id,
        &entity_type,
        &legal_name,
        &ein,
        &address,
    )
    .await
    {
        tracing::error!(error = ?e, "profile save from document failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "could not save profile").into_response();
    }
    tracing::info!(company = %company_id, "tax profile filled from uploaded entity document");
    Redirect::to("/app/tax").into_response()
}

async fn tax_form_pdf(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let _ = user;
    let form_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad form id").into_response(),
    };
    match crate::tax::get_form(&state.pool, company_id, form_id).await {
        Ok(Some(f)) => match std::fs::read(&f.file_path) {
            Ok(bytes) => (
                [(axum::http::header::CONTENT_TYPE, "application/pdf")],
                bytes,
            )
                .into_response(),
            Err(_) => (StatusCode::NOT_FOUND, "form file missing").into_response(),
        },
        Ok(None) => (StatusCode::NOT_FOUND, "form not found").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "could not load form").into_response(),
    }
}

async fn tax_form_approve(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let _ = user;
    if let Ok(form_id) = Uuid::parse_str(&id) {
        let _ = crate::tax::approve_form(&state.pool, company_id, form_id).await;
    }
    Redirect::to("/app/tax").into_response()
}

/// Step 6 · Sign: stamp the owner's signature onto every approved form.
async fn tax_sign_all(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    // The signature belongs to the owner (this user); it is shared across entities.
    let Ok(Some((sig_png, _ct))) = crate::signature::get_image(&state.pool, user.id).await else {
        return (
            StatusCode::BAD_REQUEST,
            "No signature on file — add your signature in Settings before signing.",
        )
            .into_response();
    };
    let signer = user
        .name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| user.email.clone());
    let forms = crate::tax::list_forms(&state.pool, company_id).await.unwrap_or_default();
    for f in forms.iter().filter(|f| f.status == "approved") {
        if let Err(e) = crate::tax::sign_form(&state.pool, company_id, f.id, &sig_png, &signer).await {
            tracing::error!(error = %e, form = %f.id, "sign failed");
        }
    }
    Redirect::to("/app/tax").into_response()
}

async fn tax_form_delete(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let _ = user;
    if let Ok(form_id) = Uuid::parse_str(&id) {
        let _ = crate::tax::delete_form(&state.pool, company_id, form_id).await;
    }
    Redirect::to("/app/tax").into_response()
}

// ---------------------------------------------------------------------------
// Generated documents (Reports → Tax Documents)
// ---------------------------------------------------------------------------

#[derive(Template)]
#[template(path = "tax_documents.html")]
struct TaxDocsTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    docs: Vec<crate::docgen::DocRow>,
    current_year: i32,
}

async fn tax_documents(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let docs = crate::docgen::list_documents(&state.pool, company_id)
        .await
        .unwrap_or_default();
    let current_year = chrono::Utc::now().format("%Y").to_string().parse().unwrap_or(2026);
    render(TaxDocsTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        docs,
        current_year,
    })
}

#[derive(Deserialize)]
struct GenerateDocForm {
    doc_type: String,
    year: Option<i32>,
}

async fn tax_documents_generate(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(req): Form<GenerateDocForm>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    match crate::docgen::generate(&state.pool, company_id, &req.doc_type, None, None, None, req.year)
        .await
    {
        Ok((title, kind, html)) => {
            match crate::docgen::save_document(&state.pool, company_id, &kind, &title, &html).await
            {
                Ok(id) => Redirect::to(&format!("/app/reports/documents/{id}")).into_response(),
                Err(e) => {
                    tracing::error!(error = ?e, "save document failed");
                    (StatusCode::INTERNAL_SERVER_ERROR, "could not save document").into_response()
                }
            }
        }
        Err(e) => (StatusCode::BAD_REQUEST, format!("could not generate: {e}")).into_response(),
    }
}

async fn document_view(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let _ = user;
    let doc_id = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return (StatusCode::BAD_REQUEST, "bad document id").into_response(),
    };
    match crate::docgen::get_document(&state.pool, company_id, doc_id).await {
        Ok(Some((title, html))) => Html(crate::docgen::doc_shell(&title, &html, false)).into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "document not found").into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "load document failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "could not load document").into_response()
        }
    }
}

/// One print view containing every saved document — "make all documents PDFs"
/// is one Ctrl+P / Save as PDF away.
async fn tax_documents_print_all(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let _ = user;
    let docs = crate::docgen::list_documents(&state.pool, company_id)
        .await
        .unwrap_or_default();
    let mut body = String::new();
    for d in &docs {
        if let Ok(Some((_, html))) = crate::docgen::get_document(&state.pool, company_id, d.id).await
        {
            body.push_str(&html);
            body.push_str("<div class=\"page-break\"></div>");
        }
    }
    if body.is_empty() {
        body = "<p>No documents yet — generate one from the Tax Documents page.</p>".to_string();
    }
    Html(crate::docgen::doc_shell("All documents", &body, false)).into_response()
}

async fn document_delete(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let _ = user;
    if let Ok(doc_id) = Uuid::parse_str(&id) {
        let _ = crate::docgen::delete_document(&state.pool, company_id, doc_id).await;
    }
    Redirect::to("/app/reports/tax-documents").into_response()
}

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------

async fn lookup_company_name(state: &AppState, company_id: Uuid) -> String {
    sqlx::query_scalar::<_, String>("SELECT name FROM companies WHERE id = $1")
        .bind(company_id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or_else(|_| "your company".to_string())
}

fn generated_on_str() -> String {
    Utc::now().format("%Y-%m-%d %H:%M UTC").to_string()
}

async fn dashboard(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let company_name = lookup_company_name(&state, company_id).await;
    let kpis = match queries::dashboard_kpis(&state.pool, company_id).await {
        Ok(k) => k,
        Err(e) => {
            tracing::error!(error = ?e, "dashboard kpis failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };
    render(DashboardTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        company_name,
        kpis,
    })
}

// ---------------------------------------------------------------------------
// Reports
// ---------------------------------------------------------------------------

async fn reports_index(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    render(ReportsIndexTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
    })
}

#[derive(Deserialize)]
struct DateRangeQuery {
    start: Option<String>,
    end: Option<String>,
}

fn default_year_range() -> (NaiveDate, NaiveDate) {
    let today = Utc::now().date_naive();
    let year = today.format("%Y").to_string().parse::<i32>().unwrap_or(2026);
    let start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
    (start, today)
}

async fn report_income(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<DateRangeQuery>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let (default_start, default_end) = default_year_range();
    let start = q
        .start
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(default_start);
    let end = q
        .end
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(default_end);
    let report = queries::income_statement(&state.pool, company_id, start, end)
        .await
        .unwrap_or_else(|_| queries::IncomeStatement {
            start,
            end,
            revenues: vec![],
            expenses: vec![],
            total_revenue_cents: 0,
            total_expense_cents: 0,
        });
    let company_name = lookup_company_name(&state, company_id).await;
    render(ReportIncomeTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        report,
        company_name,
        generated_on: generated_on_str(),
    })
}

#[derive(Deserialize)]
struct AsOfQuery {
    as_of: Option<String>,
}

async fn report_balance_sheet(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<AsOfQuery>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let as_of = q
        .as_of
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| Utc::now().date_naive());
    let report = queries::balance_sheet(&state.pool, company_id, as_of)
        .await
        .unwrap_or_else(|_| queries::BalanceSheet {
            as_of,
            assets: vec![],
            liabilities: vec![],
            equity: vec![],
            net_income_cents: 0,
            total_assets_cents: 0,
            total_liab_cents: 0,
            total_equity_cents: 0,
        });
    let company_name = lookup_company_name(&state, company_id).await;
    render(ReportBalanceTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        report,
        company_name,
        generated_on: generated_on_str(),
    })
}

async fn report_cash_flow(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<DateRangeQuery>,
) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let (default_start, default_end) = default_year_range();
    let start = q
        .start
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(default_start);
    let end = q
        .end
        .as_deref()
        .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or(default_end);
    let report = queries::cash_flow(&state.pool, company_id, start, end)
        .await
        .unwrap_or_else(|_| queries::CashFlow {
            start,
            end,
            opening_cash_cents: 0,
            closing_cash_cents: 0,
            change_cents: 0,
            by_other_account: vec![],
        });
    let company_name = lookup_company_name(&state, company_id).await;
    render(ReportCashFlowTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        report,
        company_name,
        generated_on: generated_on_str(),
    })
}

// ---------------------------------------------------------------------------
// Trial balance
// ---------------------------------------------------------------------------

async fn trial_balance(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await {
        Ok(u) => u,
        Err(r) => return r,
    };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c,
        None => return forbidden(),
    };
    let (rows, total_debit, total_credit) = queries::trial_balance(&state.pool, company_id)
        .await
        .unwrap_or_else(|_| (vec![], 0, 0));
    let (balance_msg, balance_class) = if total_debit == total_credit {
        ("✓ Balanced".to_string(), "ok".to_string())
    } else {
        (
            format!("⚠ Off by {} cents", (total_debit - total_credit).abs()),
            "err".to_string(),
        )
    };
    let company_name = lookup_company_name(&state, company_id).await;
    render(TrialBalanceTpl {
        user_email: Some(user.email),
        flash: None,
        flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        rows,
        total_debit_display: queries::format_cents(total_debit),
        total_credit_display: queries::format_cents(total_credit),
        balance_msg,
        balance_class,
        company_name,
        generated_on: generated_on_str(),
    })
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, "no company membership for this user").into_response()
}

// ===========================================================================
// Invoicing
// ===========================================================================

#[derive(Template)]
#[template(path = "customers_list.html")]
struct CustomersListTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    customers: Vec<queries::CustomerRow>,
}

#[derive(Template)]
#[template(path = "customer_new.html")]
struct CustomerNewTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
}

#[derive(Template)]
#[template(path = "invoices_list.html")]
struct InvoicesListTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    invoices: Vec<queries::InvoiceRow>,
    filter: Option<String>,
    summary_subtitle: String,
    outstanding_count: usize,
    outstanding_display: String,
    overdue_count: usize,
    overdue_display: String,
    paid_30d_display: String,
    draft_count: usize,
}

#[derive(Template)]
#[template(path = "invoice_new.html")]
struct InvoiceNewTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    customers: Vec<queries::CustomerRow>,
    revenue_accounts: Vec<queries::DepositAccountOption>,
    today_iso: String,
    preselect_customer: Option<String>,
}

#[derive(Template)]
#[template(path = "invoice_detail.html")]
struct InvoiceDetailTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    invoice: queries::InvoiceDetail,
    deposit_accounts: Vec<queries::DepositAccountOption>,
    public_url: String,
    today_iso: String,
    balance_dollars: String,
}

#[derive(Template)]
#[template(path = "invoice_public.html")]
struct InvoicePublicTpl {
    invoice: queries::InvoiceDetail,
    company_name: String,
    company_subtitle: String,
}

// --- Company file store ----------------------------------------------------

use crate::file_store::store_company_file;

/// One year's worth of documents for the grouped Documents view. `label` is the
/// year as a string, or "Undated" for files whose period year is unknown.
struct DocYearGroup {
    label: String,
    files: Vec<queries::CompanyFileRow>,
}

#[derive(Template)]
#[template(path = "documents.html")]
struct DocumentsTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    groups: Vec<DocYearGroup>,
    total: usize,
    all_years: Vec<String>,
    all_categories: Vec<String>,
    all_tags: Vec<String>,
    selected_year: String,
    selected_category: String,
    selected_tag: String,
    search: String,
}

#[derive(serde::Deserialize, Default)]
struct DocFilter {
    year: Option<String>,
    category: Option<String>,
    tag: Option<String>,
    q: Option<String>,
}

/// Group the company's files by period/tax year (newest first, "Undated" last).
/// list_company_files already orders by doc_year DESC NULLS LAST.
fn group_files_by_year(files: Vec<queries::CompanyFileRow>) -> Vec<DocYearGroup> {
    let mut groups: Vec<DocYearGroup> = Vec::new();
    for f in files {
        let label = match f.doc_year {
            Some(y) => y.to_string(),
            None => "Undated".to_string(),
        };
        match groups.last_mut() {
            Some(g) if g.label == label => g.files.push(f),
            _ => groups.push(DocYearGroup { label, files: vec![f] }),
        }
    }
    groups
}

async fn documents_list(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(f): axum::extract::Query<DocFilter>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let files = queries::list_company_files(&state.pool, company_id).await.unwrap_or_default();

    // Dropdown options from the full set (before filtering).
    let mut all_years: Vec<String> = files.iter()
        .map(|r| r.doc_year.map(|y| y.to_string()).unwrap_or_else(|| "Undated".to_string()))
        .collect();
    all_years.sort_by(|a, b| b.cmp(a)); // desc; "Undated" sorts after digits
    all_years.dedup();
    let mut all_categories: Vec<String> = files.iter().map(|r| r.category.clone()).collect();
    all_categories.sort();
    all_categories.dedup();
    let mut all_tags: Vec<String> = files.iter().flat_map(|r| r.tags.clone()).collect();
    all_tags.sort();
    all_tags.dedup();

    let sel_year = f.year.unwrap_or_default();
    let sel_cat = f.category.unwrap_or_default();
    let sel_tag = f.tag.unwrap_or_default();
    let search = f.q.unwrap_or_default();
    let needle = search.trim().to_lowercase();

    let filtered: Vec<queries::CompanyFileRow> = files.into_iter().filter(|r| {
        let yr = r.doc_year.map(|y| y.to_string()).unwrap_or_else(|| "Undated".to_string());
        (sel_year.is_empty() || yr == sel_year)
            && (sel_cat.is_empty() || r.category == sel_cat)
            && (sel_tag.is_empty() || r.tags.iter().any(|t| t == &sel_tag))
            && (needle.is_empty() || r.filename.to_lowercase().contains(&needle))
    }).collect();
    let total = filtered.len();

    render(DocumentsTpl {
        user_email: Some(user.email),
        flash: None, flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        groups: group_files_by_year(filtered),
        total,
        all_years,
        all_categories,
        all_tags,
        selected_year: sel_year,
        selected_category: sel_cat,
        selected_tag: sel_tag,
        search,
    })
}

async fn documents_upload(
    State(state): State<AppState>,
    jar: CookieJar,
    mut multipart: axum::extract::Multipart,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let mut category = "other".to_string();
    // Optional year override from the form; applies to every file in the batch.
    let mut year_override: Option<i32> = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("category") => {
                category = field.text().await.unwrap_or_default();
                if !matches!(category.as_str(), "statement" | "tax" | "other") {
                    category = "other".to_string();
                }
            }
            Some("doc_year") => {
                year_override = field
                    .text()
                    .await
                    .ok()
                    .and_then(|s| s.trim().parse::<i32>().ok())
                    .filter(|y| (2000..=2099).contains(y));
            }
            Some("file") => {
                let filename = field.file_name().unwrap_or("upload.bin").to_string();
                let content_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let Ok(bytes) = field.bytes().await else { continue };
                let year = year_override.or_else(|| crate::file_store::detect_year(&filename, None));
                let _ = store_company_file(
                    &state.pool, company_id, &category, &filename, &content_type, &bytes, year,
                ).await;
            }
            _ => {}
        }
    }
    Redirect::to("/app/documents").into_response()
}

#[derive(serde::Deserialize)]
struct DocYearForm {
    doc_year: String,
}

async fn document_set_year(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    axum::extract::Form(form): axum::extract::Form<DocYearForm>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let year = form.doc_year.trim().parse::<i32>().ok().filter(|y| (2000..=2099).contains(y));
    let _ = queries::update_company_file_year(&state.pool, company_id, id, year).await;
    Redirect::to("/app/documents").into_response()
}

#[derive(serde::Deserialize)]
struct DocLockForm {
    locked: Option<String>,
}

/// Lock or unlock a document. Locked files can't be deleted or have their year
/// changed (enforced in the DB queries). `locked=true` locks, anything else unlocks.
async fn document_set_lock(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    axum::extract::Form(form): axum::extract::Form<DocLockForm>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let locked = form.locked.as_deref() == Some("true");
    let _ = queries::set_company_file_locked(&state.pool, company_id, id, locked).await;
    Redirect::to("/app/documents").into_response()
}

async fn file_download(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    match queries::get_company_file(&state.pool, company_id, id).await {
        Ok(Some(f)) => match std::fs::read(&f.stored_path) {
            Ok(bytes) => (
                [
                    (axum::http::header::CONTENT_TYPE, f.content_type),
                    (
                        axum::http::header::CONTENT_DISPOSITION,
                        format!("inline; filename=\"{}\"", f.filename.replace('"', "")),
                    ),
                ],
                bytes,
            )
                .into_response(),
            Err(_) => (StatusCode::NOT_FOUND, "file missing on disk").into_response(),
        },
        Ok(None) => (StatusCode::NOT_FOUND, "file not found").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "could not load file").into_response(),
    }
}

async fn file_delete(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    if let Ok(Some(path)) = queries::delete_company_file(&state.pool, company_id, id).await {
        let _ = tokio::fs::remove_file(&path).await;
    }
    Redirect::to("/app/documents").into_response()
}

// --- Wise integration -------------------------------------------------------

struct WisePayeeRow {
    recipient: String,
    total: String,
    count: i64,
    currencies: String,
    is_self: bool,
}

#[derive(Template)]
#[template(path = "wise.html")]
struct WiseTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    connected: bool,
    label: String,
    payees: Vec<WisePayeeRow>,
    external_total: String,
    self_total: String,
    txn_count: usize,
}

fn cents(c: i64) -> String {
    let neg = c < 0;
    let a = c.abs();
    let s = format!("{}.{:02}", a / 100, a % 100);
    // thousands separators on the integer part
    let (int_part, frac) = s.split_once('.').unwrap();
    let mut out = String::new();
    for (i, ch) in int_part.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 { out.push(','); }
        out.push(ch);
    }
    let int_fmt: String = out.chars().rev().collect();
    format!("{}{}.{}", if neg { "-" } else { "" }, int_fmt, frac)
}

async fn wise_view(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let conn = crate::wise::get_connection(&state.pool, company_id).await.ok().flatten();
    let payees = crate::wise::payees(&state.pool, company_id).await.unwrap_or_default();
    let mut ext = 0i64;
    let mut slf = 0i64;
    let mut cnt = 0usize;
    let rows: Vec<WisePayeeRow> = payees.into_iter().map(|p| {
        cnt += p.count as usize;
        if p.is_self { slf += p.total_cents } else { ext += p.total_cents }
        WisePayeeRow {
            recipient: p.recipient, total: cents(p.total_cents), count: p.count,
            currencies: p.currencies, is_self: p.is_self,
        }
    }).collect();
    render(WiseTpl {
        user_email: Some(user.email),
        flash: None, flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        connected: conn.is_some(),
        label: conn.map(|c| c.label).unwrap_or_default(),
        payees: rows,
        external_total: cents(ext),
        self_total: cents(slf),
        txn_count: cnt,
    })
}

async fn wise_sync(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    match crate::wise::sync(&state.pool, company_id, 2025).await {
        Ok((n, ext)) => tracing::info!(company=%company_id, transfers=n, external=ext, "wise sync ok"),
        Err(e) => tracing::error!(company=%company_id, error=%e, "wise sync failed"),
    }
    Redirect::to("/app/wise").into_response()
}

#[derive(Template)]
#[template(path = "address_book.html")]
struct AddressBookTpl {
    user_email: Option<String>,
    flash: Option<String>,
    flash_kind: Option<String>,
    nav: NavCtx,
    labels: Vec<queries::AddressLabelRow>,
    search: String,
}

#[derive(serde::Deserialize, Default)]
struct AddressBookFilter {
    q: Option<String>,
}

async fn address_book_list(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(f): axum::extract::Query<AddressBookFilter>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let search = f.q.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let labels = queries::list_address_labels(&state.pool, company_id, search.as_deref())
        .await
        .unwrap_or_default();
    render(AddressBookTpl {
        user_email: Some(user.email),
        flash: None, flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        labels,
        search: search.unwrap_or_default(),
    })
}

#[derive(Deserialize)]
struct AddressLabelForm {
    address: String,
    name: String,
    kind: Option<String>,
    account_code: Option<String>,
    note: Option<String>,
}

async fn address_label_create(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Form(f): axum::extract::Form<AddressLabelForm>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    if !f.address.trim().is_empty() && !f.name.trim().is_empty() {
        let _ = queries::upsert_address_label(
            &state.pool, company_id, &f.address, &f.name,
            f.kind.as_deref().unwrap_or(""),
            f.account_code.as_deref().unwrap_or(""),
            f.note.as_deref().unwrap_or(""),
        ).await;
    }
    Redirect::to("/app/address-book").into_response()
}

async fn address_label_delete(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let _ = queries::delete_address_label(&state.pool, company_id, id).await;
    Redirect::to("/app/address-book").into_response()
}

async fn customers_list(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let customers = queries::list_customers(&state.pool, company_id).await.unwrap_or_default();
    render(CustomersListTpl {
        user_email: Some(user.email),
        flash: None, flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        customers,
    })
}

async fn customer_new_view(State(state): State<AppState>, jar: CookieJar) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    if active_company(&state, &jar, user.id).await.is_none() { return forbidden(); }
    render(CustomerNewTpl {
        user_email: Some(user.email),
        flash: None, flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
    })
}

#[derive(Deserialize)]
struct CustomerForm {
    name: String,
    email: Option<String>,
    phone: Option<String>,
    address_line1: Option<String>,
    address_line2: Option<String>,
    city: Option<String>,
    state: Option<String>,
    postal_code: Option<String>,
    default_terms: Option<String>,
    notes: Option<String>,
}

async fn customer_create(
    State(state): State<AppState>,
    jar: CookieJar,
    Form(req): Form<CustomerForm>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let clean = |s: Option<String>| s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return (StatusCode::BAD_REQUEST, "name is required").into_response();
    }
    let state_field = clean(req.state).map(|s| s.to_uppercase());
    let terms = req.default_terms.unwrap_or_else(|| "net_30".into());
    let input = queries::CreateCustomerInput {
        name,
        email: clean(req.email),
        phone: clean(req.phone),
        address_line1: clean(req.address_line1),
        address_line2: clean(req.address_line2),
        city: clean(req.city),
        state: state_field,
        postal_code: clean(req.postal_code),
        country: "US".into(),
        default_terms: terms,
        notes: clean(req.notes),
    };
    match queries::create_customer(&state.pool, company_id, input).await {
        Ok(_) => Redirect::to("/app/customers").into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "create customer failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "failed").into_response()
        }
    }
}

#[derive(Deserialize)]
struct InvoiceListQuery {
    status: Option<String>,
}

async fn invoices_list(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<InvoiceListQuery>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let filter = q.status.clone().filter(|s| !s.is_empty());
    let invoices = queries::list_invoices(&state.pool, company_id, filter.as_deref())
        .await
        .unwrap_or_default();

    let all = queries::list_invoices(&state.pool, company_id, None).await.unwrap_or_default();
    let today = chrono::Local::now().date_naive();
    let mut outstanding_count = 0usize;
    let mut outstanding_cents: i64 = 0;
    let mut overdue_count = 0usize;
    let mut overdue_cents: i64 = 0;
    let mut paid_30d: i64 = 0;
    let mut draft_count = 0usize;
    for i in &all {
        match i.status.as_str() {
            "sent" | "partial" => {
                outstanding_count += 1;
                outstanding_cents += i.balance_cents();
                if i.due_date < today {
                    overdue_count += 1;
                    overdue_cents += i.balance_cents();
                }
            }
            "paid" => {
                if i.issue_date >= today - chrono::Duration::days(30) {
                    paid_30d += i.total_cents;
                }
            }
            "draft" => { draft_count += 1; }
            _ => {}
        }
    }

    render(InvoicesListTpl {
        user_email: Some(user.email),
        flash: None, flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        invoices,
        filter,
        summary_subtitle: format!("{} total", all.len()),
        outstanding_count,
        outstanding_display: queries::format_cents(outstanding_cents),
        overdue_count,
        overdue_display: queries::format_cents(overdue_cents),
        paid_30d_display: queries::format_cents(paid_30d),
        draft_count,
    })
}

#[derive(Deserialize)]
struct InvoiceNewQuery {
    customer_id: Option<String>,
}

async fn invoice_new_view(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Query(q): axum::extract::Query<InvoiceNewQuery>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let customers = queries::list_customers(&state.pool, company_id).await.unwrap_or_default();
    let revenue_accounts = queries::list_revenue_accounts(&state.pool, company_id).await.unwrap_or_default();
    render(InvoiceNewTpl {
        user_email: Some(user.email),
        flash: None, flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        customers,
        revenue_accounts,
        today_iso: chrono::Local::now().date_naive().format("%Y-%m-%d").to_string(),
        preselect_customer: q.customer_id,
    })
}

async fn invoice_create(
    State(state): State<AppState>,
    jar: CookieJar,
    body: String,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };

    let parsed: Vec<(String, String)> =
        url::form_urlencoded::parse(body.as_bytes()).into_owned().collect();
    let mut customer_id: Option<Uuid> = None;
    let mut issue_date_str = String::new();
    let mut terms = String::new();
    let mut memo: Option<String> = None;
    let mut customer_notes: Option<String> = None;
    let mut desc: Vec<String> = Vec::new();
    let mut qty: Vec<String> = Vec::new();
    let mut unit_dollars: Vec<String> = Vec::new();
    let mut tax_pct: Vec<String> = Vec::new();
    let mut rev_acct: Vec<String> = Vec::new();
    for (k, v) in parsed {
        match k.as_str() {
            "customer_id" => customer_id = Uuid::parse_str(&v).ok(),
            "issue_date" => issue_date_str = v,
            "terms" => terms = v,
            "memo" if !v.is_empty() => memo = Some(v),
            "customer_notes" if !v.is_empty() => customer_notes = Some(v),
            "desc" | "desc[]" => desc.push(v),
            "qty" | "qty[]" => qty.push(v),
            "unit_dollars" | "unit_dollars[]" => unit_dollars.push(v),
            "tax_pct" | "tax_pct[]" => tax_pct.push(v),
            "revenue_account_id" | "revenue_account_id[]" => rev_acct.push(v),
            _ => {}
        }
    }
    let Some(customer_id) = customer_id else {
        return (StatusCode::BAD_REQUEST, "missing customer").into_response();
    };
    let issue_date = match NaiveDate::parse_from_str(&issue_date_str, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid date").into_response(),
    };
    if terms.is_empty() { terms = "net_30".into(); }
    let due_date = crate::commands::invoice::default_due_date(issue_date, &terms);

    let n = desc.len();
    if n == 0 || qty.len() != n || unit_dollars.len() != n
        || tax_pct.len() != n || rev_acct.len() != n {
        return (StatusCode::BAD_REQUEST, "line arrays mismatched").into_response();
    }
    let mut lines = Vec::with_capacity(n);
    for i in 0..n {
        let d = desc[i].trim();
        if d.is_empty() { continue; }
        let q: f64 = qty[i].parse().unwrap_or(0.0);
        let u: f64 = unit_dollars[i].parse().unwrap_or(0.0);
        let t: f64 = tax_pct[i].parse().unwrap_or(0.0);
        let r = match Uuid::parse_str(&rev_acct[i]) {
            Ok(r) => r,
            Err(_) => return (StatusCode::BAD_REQUEST, "invalid revenue account").into_response(),
        };
        lines.push(crate::commands::invoice::DraftLine {
            description: d.to_string(),
            quantity: q,
            unit_price_cents: (u * 100.0).round() as i64,
            tax_rate_pct: t,
            revenue_account_id: r,
        });
    }
    if lines.is_empty() {
        return (StatusCode::BAD_REQUEST, "need at least one non-empty line").into_response();
    }

    let input = crate::commands::invoice::DraftInvoiceInput {
        customer_id,
        issue_date,
        due_date,
        terms,
        memo,
        customer_notes,
        lines,
    };
    match crate::commands::invoice::create_draft(&state.pool, company_id, input).await {
        Ok(id) => Redirect::to(&format!("/app/invoices/{}", id)).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "create invoice failed");
            (StatusCode::INTERNAL_SERVER_ERROR, format!("{e}")).into_response()
        }
    }
}

async fn invoice_detail_view(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let invoice = match queries::get_invoice(&state.pool, company_id, id).await {
        Ok(Some(i)) => i,
        Ok(None) => return (StatusCode::NOT_FOUND, "not found").into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "get invoice failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal").into_response();
        }
    };
    let deposit_accounts = queries::list_deposit_accounts(&state.pool, company_id).await.unwrap_or_default();
    let public_url = format!("{}/invoice/{}", state.config.public_base_url, invoice.public_token);
    let balance_dollars = format!("{:.2}", invoice.balance_cents() as f64 / 100.0);
    render(InvoiceDetailTpl {
        user_email: Some(user.email),
        flash: None, flash_kind: None,
        nav: build_nav(&state, &jar, user.id).await,
        invoice,
        deposit_accounts,
        public_url,
        today_iso: chrono::Local::now().date_naive().format("%Y-%m-%d").to_string(),
        balance_dollars,
    })
}

async fn invoice_issue(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    match crate::commands::invoice::issue_invoice(&state.pool, company_id, user.id, id).await {
        Ok(_) => Redirect::to(&format!("/app/invoices/{}", id)).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "issue invoice failed");
            (StatusCode::CONFLICT, format!("{e}")).into_response()
        }
    }
}

#[derive(Deserialize)]
struct InvoicePaymentForm {
    payment_date: String,
    amount_dollars: String,
    method: String,
    deposit_account_id: Uuid,
    reference: Option<String>,
}

async fn invoice_payment(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Form(req): Form<InvoicePaymentForm>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let date = match NaiveDate::parse_from_str(&req.payment_date, "%Y-%m-%d") {
        Ok(d) => d,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid date").into_response(),
    };
    let dollars: f64 = req.amount_dollars.parse().unwrap_or(0.0);
    let cents = (dollars * 100.0).round() as i64;
    let input = crate::commands::invoice::PaymentInput {
        payment_date: date,
        amount_cents: cents,
        method: req.method,
        deposit_account_id: req.deposit_account_id,
        reference: req.reference.filter(|s| !s.is_empty()),
        memo: None,
    };
    match crate::commands::invoice::record_payment(&state.pool, company_id, user.id, id, input).await {
        Ok(_) => Redirect::to(&format!("/app/invoices/{}", id)).into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "payment failed");
            (StatusCode::CONFLICT, format!("{e}")).into_response()
        }
    }
}

#[derive(Deserialize)]
struct VoidForm {
    reason: String,
}

async fn invoice_void(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
    Form(req): Form<VoidForm>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    match crate::commands::invoice::void_invoice(&state.pool, company_id, user.id, id, req.reason).await {
        Ok(_) => Redirect::to(&format!("/app/invoices/{}", id)).into_response(),
        Err(e) => (StatusCode::CONFLICT, format!("{e}")).into_response(),
    }
}

async fn invoice_send(
    State(state): State<AppState>,
    jar: CookieJar,
    axum::extract::Path(id): axum::extract::Path<Uuid>,
) -> Response {
    let user = match require_user(&state, &jar).await { Ok(u) => u, Err(r) => return r };
    let company_id = match active_company(&state, &jar, user.id).await {
        Some(c) => c, None => return forbidden(),
    };
    let invoice = match queries::get_invoice(&state.pool, company_id, id).await {
        Ok(Some(i)) => i,
        _ => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };
    let email = match &invoice.customer.email {
        Some(e) if !e.is_empty() => e.clone(),
        _ => return (StatusCode::BAD_REQUEST, "customer has no email").into_response(),
    };
    if !state.email.is_configured() {
        return (StatusCode::SERVICE_UNAVAILABLE,
            "email not configured. Set RESEND_API_KEY env var. Public link is in the invoice detail page.")
            .into_response();
    }
    let company_name = queries::get_company(&state.pool, company_id).await
        .ok().flatten().map(|c| c.0).unwrap_or_else(|| "Maven".to_string());
    let public_url = format!("{}/invoice/{}", state.config.public_base_url, invoice.public_token);
    let html = format!(
        r#"<div style="font-family: -apple-system, sans-serif; max-width: 560px; margin: 0 auto; padding: 24px; color: #0f0f10;">
            <p>Hi {customer},</p>
            <p>Your invoice <strong>{number}</strong> from <strong>{company}</strong> is ready: <strong>{total}</strong> due by <strong>{due}</strong>.</p>
            <p><a href="{url}" style="display: inline-block; padding: 10px 18px; background: #0f0f10; color: #fff; text-decoration: none; border-radius: 4px;">View & pay invoice</a></p>
            <p style="font-size: 12px; color: #5b5b62; margin-top: 24px;">Or copy this link: {url}</p>
        </div>"#,
        customer = invoice.customer.name,
        number = invoice.invoice_number,
        company = company_name,
        total = invoice.total_display(),
        due = invoice.due_us(),
        url = public_url,
    );
    match state.email.send(
        &email,
        &format!("Invoice {} from {}", invoice.invoice_number, company_name),
        &html,
        None,
    ).await {
        Ok(_) => {
            let _ = crate::commands::invoice::mark_sent(&state.pool, company_id, id, &email).await;
            Redirect::to(&format!("/app/invoices/{}", id)).into_response()
        }
        Err(e) => {
            tracing::error!(error = ?e, "send email failed");
            (StatusCode::BAD_GATEWAY, format!("email send failed: {e}")).into_response()
        }
    }
}

async fn invoice_public_view(
    State(state): State<AppState>,
    axum::extract::Path(token): axum::extract::Path<String>,
) -> Response {
    let lookup = match queries::get_invoice_by_token(&state.pool, &token).await {
        Ok(Some(v)) => v,
        Ok(None) => return (StatusCode::NOT_FOUND, "invoice not found").into_response(),
        Err(e) => {
            tracing::error!(error = ?e, "invoice token lookup failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal").into_response();
        }
    };
    let (invoice_id, _company_id, company_name) = lookup;
    let invoice = match queries::get_invoice_public(&state.pool, &token, invoice_id).await {
        Ok(Some(i)) => i,
        _ => return (StatusCode::NOT_FOUND, "not found").into_response(),
    };
    render(InvoicePublicTpl { invoice, company_name, company_subtitle: String::new() })
}
