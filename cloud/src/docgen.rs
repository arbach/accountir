//! Generated report / tax documents: render a report (or a full year-end tax
//! package) to a print-ready HTML fragment and store it in `documents`, where
//! the Reports → Tax Documents tab lists them for viewing / saving as PDF.

use chrono::NaiveDate;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::queries;
use crate::store::event_store::set_tenant;

pub struct DocRow {
    pub id: Uuid,
    pub kind: String,
    pub title: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

impl DocRow {
    pub fn created_display(&self) -> String {
        self.created_at.format("%Y-%m-%d %H:%M UTC").to_string()
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn money(cents: i64) -> String {
    let neg = cents < 0;
    let abs = cents.unsigned_abs();
    let dollars = abs / 100;
    let rem = abs % 100;
    let mut s = dollars.to_string();
    let mut grouped = String::new();
    let bytes = s.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(*b as char);
    }
    s = format!("${grouped}.{rem:02}");
    if neg { format!("({s})") } else { s }
}

fn row(label: &str, cents: i64, strong: bool) -> String {
    let (o, c) = if strong { ("<strong>", "</strong>") } else { ("", "") };
    format!(
        "<tr><td>{o}{}{c}</td><td class=\"num\">{o}{}{c}</td></tr>",
        esc(label),
        money(cents)
    )
}

async fn company_name(pool: &PgPool, company_id: Uuid) -> AppResult<String> {
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    let name: Option<(String,)> = sqlx::query_as("SELECT name FROM companies WHERE id = $1")
        .bind(company_id)
        .fetch_optional(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(name.map(|(n,)| n).unwrap_or_else(|| "Company".to_string()))
}

fn section_header(company: &str, title: &str, sub: &str) -> String {
    format!(
        "<div class=\"doc-head\"><div class=\"co\">{}</div><h2>{}</h2><div class=\"sub\">{}</div></div>",
        esc(company),
        esc(title),
        esc(sub)
    )
}

async fn income_statement_html(
    pool: &PgPool,
    company_id: Uuid,
    company: &str,
    start: NaiveDate,
    end: NaiveDate,
) -> AppResult<String> {
    let r = queries::income_statement(pool, company_id, start, end).await?;
    let mut h = section_header(company, "Income Statement", &format!("{start} to {end}"));
    h.push_str("<table><tr><th>Account</th><th class=\"num\">Amount</th></tr>");
    h.push_str("<tr><td colspan=\"2\"><strong>Revenue</strong></td></tr>");
    for l in &r.revenues {
        h.push_str(&row(&format!("{} {}", l.account_number, l.name), l.amount_cents, false));
    }
    h.push_str(&row("Total revenue", r.total_revenue_cents, true));
    h.push_str("<tr><td colspan=\"2\"><strong>Expenses</strong></td></tr>");
    for l in &r.expenses {
        h.push_str(&row(&format!("{} {}", l.account_number, l.name), l.amount_cents, false));
    }
    h.push_str(&row("Total expenses", r.total_expense_cents, true));
    h.push_str(&row("Net income", r.net_income_cents(), true));
    h.push_str("</table>");
    Ok(h)
}

async fn balance_sheet_html(
    pool: &PgPool,
    company_id: Uuid,
    company: &str,
    as_of: NaiveDate,
) -> AppResult<String> {
    let r = queries::balance_sheet(pool, company_id, as_of).await?;
    let mut h = section_header(company, "Balance Sheet", &format!("As of {as_of}"));
    h.push_str("<table><tr><th>Account</th><th class=\"num\">Amount</th></tr>");
    h.push_str("<tr><td colspan=\"2\"><strong>Assets</strong></td></tr>");
    for l in &r.assets {
        h.push_str(&row(&format!("{} {}", l.account_number, l.name), l.amount_cents, false));
    }
    h.push_str(&row("Total assets", r.total_assets_cents, true));
    h.push_str("<tr><td colspan=\"2\"><strong>Liabilities</strong></td></tr>");
    for l in &r.liabilities {
        h.push_str(&row(&format!("{} {}", l.account_number, l.name), l.amount_cents, false));
    }
    h.push_str(&row("Total liabilities", r.total_liab_cents, true));
    h.push_str("<tr><td colspan=\"2\"><strong>Equity</strong></td></tr>");
    for l in &r.equity {
        h.push_str(&row(&format!("{} {}", l.account_number, l.name), l.amount_cents, false));
    }
    h.push_str(&row("Net income (period)", r.net_income_cents, false));
    h.push_str(&row("Total equity", r.total_equity_cents + r.net_income_cents, true));
    h.push_str("</table>");
    Ok(h)
}

async fn cash_flow_html(
    pool: &PgPool,
    company_id: Uuid,
    company: &str,
    start: NaiveDate,
    end: NaiveDate,
) -> AppResult<String> {
    let r = queries::cash_flow(pool, company_id, start, end).await?;
    let mut h = section_header(company, "Cash Flow", &format!("{start} to {end}"));
    h.push_str("<table><tr><th></th><th class=\"num\">Amount</th></tr>");
    h.push_str(&row("Opening cash", r.opening_cash_cents, false));
    h.push_str(&row("Closing cash", r.closing_cash_cents, false));
    h.push_str(&row("Net change", r.change_cents, true));
    h.push_str("<tr><td colspan=\"2\"><strong>By counterpart account</strong></td></tr>");
    for l in &r.by_other_account {
        h.push_str(&row(&format!("{} {}", l.account_number, l.name), l.amount_cents, false));
    }
    h.push_str("</table>");
    Ok(h)
}

async fn trial_balance_html(pool: &PgPool, company_id: Uuid, company: &str) -> AppResult<String> {
    let (rows, total_debit, total_credit) = queries::trial_balance(pool, company_id).await?;
    let mut h = section_header(company, "Trial Balance", "All activity to date");
    h.push_str("<table><tr><th>Account</th><th class=\"num\">Debit</th><th class=\"num\">Credit</th></tr>");
    for r in &rows {
        h.push_str(&format!(
            "<tr><td>{} {}</td><td class=\"num\">{}</td><td class=\"num\">{}</td></tr>",
            esc(&r.account_number),
            esc(&r.name),
            if r.debit_cents > 0 { money(r.debit_cents) } else { String::new() },
            if r.credit_cents > 0 { money(r.credit_cents) } else { String::new() },
        ));
    }
    h.push_str(&format!(
        "<tr><td><strong>Totals</strong></td><td class=\"num\"><strong>{}</strong></td><td class=\"num\"><strong>{}</strong></td></tr>",
        money(total_debit),
        money(total_credit)
    ));
    h.push_str("</table>");
    Ok(h)
}

/// Render a document of the given type. Returns (default title, kind, html).
pub async fn generate(
    pool: &PgPool,
    company_id: Uuid,
    doc_type: &str,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
    as_of: Option<NaiveDate>,
    year: Option<i32>,
) -> AppResult<(String, String, String)> {
    let company = company_name(pool, company_id).await?;
    let today = chrono::Utc::now().date_naive();
    let y = year.unwrap_or_else(|| today.format("%Y").to_string().parse().unwrap_or(2026));
    let y_start = NaiveDate::from_ymd_opt(y, 1, 1).ok_or(AppError::NotFound)?;
    let y_end = NaiveDate::from_ymd_opt(y, 12, 31).ok_or(AppError::NotFound)?;

    match doc_type {
        "income_statement" => {
            let (s, e) = (start.unwrap_or(y_start), end.unwrap_or(y_end));
            let html = income_statement_html(pool, company_id, &company, s, e).await?;
            Ok((format!("Income Statement {s} – {e}"), "report".into(), html))
        }
        "balance_sheet" => {
            let a = as_of.unwrap_or(today);
            let html = balance_sheet_html(pool, company_id, &company, a).await?;
            Ok((format!("Balance Sheet as of {a}"), "report".into(), html))
        }
        "cash_flow" => {
            let (s, e) = (start.unwrap_or(y_start), end.unwrap_or(y_end));
            let html = cash_flow_html(pool, company_id, &company, s, e).await?;
            Ok((format!("Cash Flow {s} – {e}"), "report".into(), html))
        }
        "trial_balance" => {
            let html = trial_balance_html(pool, company_id, &company).await?;
            Ok((format!("Trial Balance (generated {today})"), "report".into(), html))
        }
        "tax_package" => {
            let mut html = String::new();
            html.push_str(&income_statement_html(pool, company_id, &company, y_start, y_end).await?);
            html.push_str("<div class=\"page-break\"></div>");
            html.push_str(&balance_sheet_html(pool, company_id, &company, y_end).await?);
            html.push_str("<div class=\"page-break\"></div>");
            html.push_str(&cash_flow_html(pool, company_id, &company, y_start, y_end).await?);
            html.push_str("<div class=\"page-break\"></div>");
            html.push_str(&trial_balance_html(pool, company_id, &company).await?);
            Ok((format!("Tax Package {y} — {company}"), "tax".into(), html))
        }
        other => Err(AppError::BadRequest(format!(
            "unknown document type '{other}' (expected income_statement, balance_sheet, cash_flow, trial_balance, or tax_package)"
        ))),
    }
}

pub async fn save_document(
    pool: &PgPool,
    company_id: Uuid,
    kind: &str,
    title: &str,
    html: &str,
) -> AppResult<Uuid> {
    let id = Uuid::new_v4();
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query(
        "INSERT INTO documents (id, company_id, kind, title, html) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(id)
    .bind(company_id)
    .bind(kind)
    .bind(title)
    .bind(html)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn list_documents(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<DocRow>> {
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query(
        "SELECT id, kind, title, created_at FROM documents WHERE company_id = $1 ORDER BY created_at DESC",
    )
    .bind(company_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows
        .into_iter()
        .map(|r| DocRow {
            id: r.get(0),
            kind: r.get(1),
            title: r.get(2),
            created_at: r.get(3),
        })
        .collect())
}

pub async fn get_document(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
) -> AppResult<Option<(String, String)>> {
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT title, html FROM documents WHERE id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(row)
}

pub async fn delete_document(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = sqlx::Acquire::begin(&mut conn).await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query("DELETE FROM documents WHERE id = $1").bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Shared print-friendly shell for the document view pages.
pub fn doc_shell(title: &str, body: &str, print_all_button: bool) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>{t}</title><style>\
        body{{font-family:system-ui,-apple-system,sans-serif;max-width:860px;margin:1.5rem auto;padding:0 1rem;color:#1a1a1a}}\
        .toolbar{{display:flex;gap:8px;margin-bottom:1rem}} .toolbar a,.toolbar button{{font-size:14px;padding:6px 14px;\
        border:1px solid #ccc;border-radius:6px;background:#fff;cursor:pointer;text-decoration:none;color:#1a1a1a}}\
        .toolbar .primary{{background:#2563eb;border-color:#2563eb;color:#fff}}\
        .doc-head{{margin:1.2rem 0 .4rem}} .doc-head .co{{font-weight:700;font-size:18px}}\
        .doc-head h2{{margin:.1rem 0}} .doc-head .sub{{color:#666;font-size:13px}}\
        table{{width:100%;border-collapse:collapse;margin:.6rem 0 1.4rem;font-size:14px}}\
        th,td{{padding:5px 8px;border-bottom:1px solid #e5e5e5;text-align:left}} .num{{text-align:right;font-variant-numeric:tabular-nums}}\
        .page-break{{page-break-after:always}}\
        @media print{{.toolbar{{display:none}}body{{margin:0;max-width:none}}}}\
        </style></head><body>\
        <div class=\"toolbar\"><a href=\"/app/reports/tax-documents\">&larr; Tax Documents</a>\
        <button class=\"primary\" onclick=\"window.print()\">Save as PDF</button>{extra}</div>\
        {body}</body></html>",
        t = esc(title),
        extra = if print_all_button { "" } else { "" },
        body = body,
    )
}
