use chrono::{Datelike, NaiveDate};
use sqlx::{Acquire, PgPool, Row};
use uuid::Uuid;

use crate::error::AppResult;
use crate::store::event_store::set_tenant;

#[derive(Debug, Clone)]
pub struct AccountRow {
    pub id: Uuid,
    pub account_type: String,
    pub account_number: String,
    pub name: String,
    pub currency: Option<String>,
    /// Current tax-line tag (from tax_account_lines), if any.
    pub tax_line_field: Option<String>,
    pub tax_line_label: Option<String>,
    pub tax_line_status: Option<String>,
    /// Signed balance (debit +, credit -) within the selected date range.
    /// Zero when balances weren't requested (e.g. filter dropdowns).
    pub balance_cents: i64,
}

impl AccountRow {
    pub fn balance_display(&self) -> String {
        format_cents(self.balance_cents)
    }
}

/// One selectable tax-form line for the Accounts-page tax-line dropdown.
#[derive(Debug, Clone)]
pub struct TaxLineOpt {
    pub field: String,
    pub label: String,
    pub node: String,
}

/// The valid tax lines for a form, in form order. Mirrors the OpenTax engine
/// input nodes (tax/opentax) and the CoA templates (tax/coa/templates).
pub fn tax_line_options(form_code: &str) -> Vec<TaxLineOpt> {
    let mk = |field: &str, label: &str, node: &str| TaxLineOpt {
        field: field.to_string(),
        label: label.to_string(),
        node: node.to_string(),
    };
    match form_code {
        "f1120" => vec![
            mk("line1a_gross_receipts", "1a Gross receipts", "f1120"),
            mk("line2_cogs", "2 Cost of goods sold", "f1120"),
            mk("line4_dividends", "4 Dividends", "f1120"),
            mk("line5_interest", "5 Interest income", "f1120"),
            mk("line6_gross_rents", "6 Gross rents", "f1120"),
            mk("line8_capital_gain", "8 Capital gain", "f1120"),
            mk("line10_other_income", "10 Other income", "f1120"),
            mk("line12_officer_compensation", "12 Officer compensation", "f1120"),
            mk("line13_salaries_wages", "13 Salaries & wages", "f1120"),
            mk("line17_taxes_licenses", "17 Taxes & licenses", "f1120"),
            mk("line19_charitable", "19 Charitable contributions", "f1120"),
            mk("line20_depreciation", "20 Depreciation", "f1120"),
            mk("line26_other_deductions", "26 Other deductions", "f1120"),
            mk("basis_adjustment", "Sch M-2 / retained earnings (equity)", "schedule_m2"),
        ],
        "f1040" => vec![
            mk("line2b_interest", "Sch B — Interest income", "start"),
            mk("line3b_dividends", "Sch B — Ordinary dividends", "start"),
            mk("line7_capital_gain", "Sch D — Capital gain (8949)", "start"),
            mk("line5_schedule_e", "Sch E — Passthrough/rental (K-1)", "start"),
            mk("schedule1_other_income", "Sch 1 — Other income", "start"),
            mk("schedule1_hsa", "Sch 1 — HSA adjustment", "start"),
            mk("basis_adjustment", "Owner draw / basis — not reportable", "schedule_m2"),
        ],
        // default: f1120s (S-corp) — page 1 + separately-stated (Sch K) + rentals (8825)
        _ => vec![
            mk("line1a_gross_receipts", "1a Gross receipts", "f1120s"),
            mk("line2_cogs", "2 Cost of goods sold", "f1120s"),
            mk("line5_other_income", "5 Other income", "f1120s"),
            mk("line7_officer_compensation", "7 Officer compensation", "f1120s"),
            mk("line8_salaries_wages", "8 Salaries & wages", "f1120s"),
            mk("line9_repairs_maintenance", "9 Repairs & maintenance", "f1120s"),
            mk("line10_bad_debts", "10 Bad debts", "f1120s"),
            mk("line11_rents", "11 Rents", "f1120s"),
            mk("line12_taxes", "12 Taxes & licenses", "f1120s"),
            mk("line13_interest", "13 Interest", "f1120s"),
            mk("line14_depreciation", "14 Depreciation", "f1120s"),
            mk("line16_advertising", "16 Advertising", "f1120s"),
            mk("line17_pension_profit_sharing", "17 Pension/profit-sharing", "f1120s"),
            mk("line18_employee_benefits", "18 Employee benefits", "f1120s"),
            mk("line19_other_deductions", "19 Other deductions", "f1120s"),
            mk("line4_interest_income", "Sch K-4 — Interest income", "schedule_k"),
            mk("line5a_ordinary_dividends", "Sch K-5a — Dividends", "schedule_k"),
            mk("line12a_charitable", "Sch K-12a — Charitable", "schedule_k"),
            mk("gross_rents", "8825 — Gross rents", "f8825"),
            mk("expense_repairs", "8825 — Repairs", "f8825"),
            mk("expense_interest", "8825 — Mortgage interest", "f8825"),
            mk("expense_taxes", "8825 — Property taxes", "f8825"),
            mk("expense_insurance", "8825 — Insurance", "f8825"),
            mk("expense_utilities", "8825 — Utilities", "f8825"),
            mk("expense_depreciation", "8825 — Depreciation", "f8825"),
            mk("expense_other", "8825 — Other", "f8825"),
            mk("line16d_distributions", "Sch K-16d — Distributions (equity)", "schedule_k"),
            mk("basis_adjustment", "Sch M-2 / stock basis (equity)", "schedule_m2"),
        ],
    }
}

/// Step-2 tax-line mapping coverage: (tagged, total) active income/expense accounts.
pub async fn tagging_coverage(pool: &PgPool, company_id: Uuid) -> (i64, i64) {
    let mut conn = match pool.acquire().await {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };
    let mut tx = match conn.begin().await {
        Ok(t) => t,
        Err(_) => return (0, 0),
    };
    let _ = set_tenant(&mut tx, company_id).await;
    let row = sqlx::query(
        r#"
        SELECT count(*) FILTER (WHERE a.account_type IN ('revenue','expense')) AS total,
               count(*) FILTER (WHERE a.account_type IN ('revenue','expense') AND t.field IS NOT NULL) AS tagged
        FROM accounts a
        LEFT JOIN tax_account_lines t
               ON t.account_number = a.account_number AND t.company_id = a.company_id
        WHERE a.is_active = true
        "#,
    )
    .fetch_one(&mut *tx)
    .await;
    let _ = tx.commit().await;
    match row {
        Ok(r) => (r.get::<i64, _>("tagged"), r.get::<i64, _>("total")),
        Err(_) => (0, 0),
    }
}

/// Federal form for a company, derived from its tax profile's entity_type.
pub async fn company_form_code(pool: &PgPool, company_id: Uuid) -> String {
    let mut conn = match pool.acquire().await {
        Ok(c) => c,
        Err(_) => return "f1120s".to_string(),
    };
    let mut tx = match conn.begin().await {
        Ok(t) => t,
        Err(_) => return "f1120s".to_string(),
    };
    let _ = set_tenant(&mut tx, company_id).await;
    let et: Option<String> = sqlx::query_scalar(
        "SELECT entity_type FROM tax_profiles WHERE company_id = $1",
    )
    .bind(company_id)
    .fetch_optional(&mut *tx)
    .await
    .ok()
    .flatten();
    let _ = tx.commit().await;
    match et.as_deref() {
        Some("c_corp") => "f1120",
        Some("individual") => "f1040",
        _ => "f1120s",
    }
    .to_string()
}

/// Set (or clear, when `field` is empty) the tax-line tag for one account.
/// Upserts into tax_account_lines with status='override' (a human decision).
pub async fn set_account_tax_line(
    pool: &PgPool,
    company_id: Uuid,
    account_id: Uuid,
    field: &str,
) -> AppResult<String> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;

    // Look up the account (number + name) under tenant isolation.
    let acct = sqlx::query(
        "SELECT account_number, name FROM accounts WHERE id = $1 AND company_id = $2",
    )
    .bind(account_id)
    .bind(company_id)
    .fetch_optional(&mut *tx)
    .await?;
    let acct = match acct {
        Some(r) => r,
        None => {
            tx.commit().await?;
            return Ok("account not found".to_string());
        }
    };
    let number: String = acct.get(0);
    let name: String = acct.get(1);

    if field.trim().is_empty() {
        sqlx::query("DELETE FROM tax_account_lines WHERE company_id = $1 AND account_number = $2")
            .bind(company_id)
            .bind(&number)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Ok("Unassigned".to_string());
    }

    let form_code = company_form_code(pool, company_id).await;
    let opt = tax_line_options(&form_code)
        .into_iter()
        .find(|o| o.field == field);
    let (label, node) = match opt {
        Some(o) => (o.label, o.node),
        None => {
            tx.commit().await?;
            return Ok("invalid tax line".to_string());
        }
    };

    sqlx::query(
        r#"
        INSERT INTO tax_account_lines
            (company_id, account_number, account_name, form_code, node, field, line_label, status, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, 'override', now())
        ON CONFLICT (company_id, account_number) DO UPDATE SET
            node = EXCLUDED.node, field = EXCLUDED.field, line_label = EXCLUDED.line_label,
            status = 'override', flags = '[]'::jsonb, updated_at = now()
        "#,
    )
    .bind(company_id)
    .bind(&number)
    .bind(&name)
    .bind(&form_code)
    .bind(&node)
    .bind(field)
    .bind(&label)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(label)
}

pub async fn list_accounts(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<AccountRow>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT a.id, a.account_type::text, a.account_number, a.name, a.currency,
               t.field, t.line_label, t.status
        FROM accounts a
        LEFT JOIN tax_account_lines t
               ON t.account_number = a.account_number AND t.company_id = a.company_id
        WHERE a.is_active = true
        ORDER BY a.account_number ASC
        "#,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(rows
        .into_iter()
        .map(|r| AccountRow {
            id: r.get(0),
            account_type: r.get::<String, _>(1),
            account_number: r.get(2),
            name: r.get(3),
            currency: r.get(4),
            tax_line_field: r.get(5),
            tax_line_label: r.get(6),
            tax_line_status: r.get(7),
            balance_cents: 0,
        })
        .collect())
}

/// Chart of accounts with each account's signed balance (debit +, credit -)
/// over the given date range (unbounded when start/end are None), skipping void
/// entries. Ordered by account number.
pub async fn list_accounts_with_balances(
    pool: &PgPool,
    company_id: Uuid,
    start: Option<NaiveDate>,
    end: Option<NaiveDate>,
) -> AppResult<Vec<AccountRow>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT a.id, a.account_type::text, a.account_number, a.name, a.currency,
               t.field, t.line_label, t.status,
               COALESCE(SUM(jl.amount) FILTER (
                   WHERE je.is_void = false
                     AND ($2::date IS NULL OR je.date >= $2)
                     AND ($3::date IS NULL OR je.date <= $3)
               ), 0)::BIGINT AS balance_cents
        FROM accounts a
        LEFT JOIN tax_account_lines t
               ON t.account_number = a.account_number AND t.company_id = a.company_id
        LEFT JOIN journal_lines jl ON jl.account_id = a.id
        LEFT JOIN journal_entries je ON je.id = jl.entry_id
        WHERE a.is_active = true
        GROUP BY a.id, a.account_type, a.account_number, a.name, a.currency,
                 t.field, t.line_label, t.status
        ORDER BY a.account_number ASC
        "#,
    )
    .bind(company_id)
    .bind(start)
    .bind(end)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows
        .into_iter()
        .map(|r| AccountRow {
            id: r.get(0),
            account_type: r.get::<String, _>(1),
            account_number: r.get(2),
            name: r.get(3),
            currency: r.get(4),
            tax_line_field: r.get(5),
            tax_line_label: r.get(6),
            tax_line_status: r.get(7),
            balance_cents: r.get(8),
        })
        .collect())
}

#[derive(Debug, Clone)]
pub struct EntryRow {
    pub id: Uuid,
    pub date: NaiveDate,
    pub memo: String,
    pub reference: Option<String>,
    pub total_debits_cents: i64,
}

impl EntryRow {
    pub fn total_display(&self) -> String {
        format_cents(self.total_debits_cents)
    }
}

pub async fn list_entries(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<EntryRow>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT je.id,
               je.date,
               je.memo,
               je.reference,
               COALESCE(SUM(GREATEST(jl.amount, 0)), 0)::BIGINT AS total_debits_cents
        FROM journal_entries je
        LEFT JOIN journal_lines jl ON jl.entry_id = je.id
        WHERE je.is_void = false
        GROUP BY je.id, je.date, je.memo, je.reference
        ORDER BY je.date DESC, je.id DESC
        LIMIT 200
        "#,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(rows
        .into_iter()
        .map(|r| EntryRow {
            id: r.get(0),
            date: r.get(1),
            memo: r.get::<Option<String>, _>(2).unwrap_or_default(),
            reference: r.get(3),
            total_debits_cents: r.get(4),
        })
        .collect())
}

#[derive(Debug, Clone)]
pub struct TrialBalanceRow {
    pub account_number: String,
    pub name: String,
    pub account_type: String,
    pub debit_cents: i64,
    pub credit_cents: i64,
}

impl TrialBalanceRow {
    pub fn debit_display(&self) -> String {
        if self.debit_cents > 0 { format_cents(self.debit_cents) } else { String::new() }
    }
    pub fn credit_display(&self) -> String {
        if self.credit_cents > 0 { format_cents(self.credit_cents) } else { String::new() }
    }
}

pub fn format_cents(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    let abs = cents.unsigned_abs();
    let dollars = abs / 100;
    let frac = abs % 100;
    // Thousands separators: 1303039 -> "13,030.39"
    let digits = dollars.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(c);
    }
    format!("{sign}{grouped}.{frac:02}")
}

pub async fn trial_balance(
    pool: &PgPool,
    company_id: Uuid,
) -> AppResult<(Vec<TrialBalanceRow>, i64, i64)> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT a.account_number,
               a.name,
               a.account_type::text,
               COALESCE(SUM(CASE WHEN je.id IS NOT NULL AND jl.amount > 0 THEN jl.amount ELSE 0 END), 0)::BIGINT AS debit,
               COALESCE(SUM(CASE WHEN je.id IS NOT NULL AND jl.amount < 0 THEN -jl.amount ELSE 0 END), 0)::BIGINT AS credit
        FROM accounts a
        LEFT JOIN journal_lines jl ON jl.account_id = a.id
        -- is_void gate in the ON clause: a voided line survives the LEFT JOIN with
        -- je = NULL, so the sums must exclude rows where je.id IS NULL.
        LEFT JOIN journal_entries je ON je.id = jl.entry_id AND je.is_void = false
        WHERE a.is_active = true
        GROUP BY a.id, a.account_number, a.name, a.account_type
        HAVING COALESCE(SUM(CASE WHEN je.id IS NOT NULL THEN ABS(jl.amount) ELSE 0 END), 0) > 0
        ORDER BY a.account_number ASC
        "#,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    let mut total_debit = 0i64;
    let mut total_credit = 0i64;
    let trial: Vec<TrialBalanceRow> = rows
        .into_iter()
        .map(|r| {
            let debit: i64 = r.get(3);
            let credit: i64 = r.get(4);
            total_debit += debit;
            total_credit += credit;
            TrialBalanceRow {
                account_number: r.get(0),
                name: r.get(1),
                account_type: r.get(2),
                debit_cents: debit,
                credit_cents: credit,
            }
        })
        .collect();

    Ok((trial, total_debit, total_credit))
}

#[derive(Debug, Clone)]
pub struct PlaidItemRow {
    pub id: Uuid,
    pub institution_name: String,
    pub status: String,
    pub last_synced_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl PlaidItemRow {
    pub fn last_synced_display(&self) -> String {
        match self.last_synced_at {
            Some(ts) => ts.format("%Y-%m-%d %H:%M UTC").to_string(),
            None => "never".to_string(),
        }
    }
}

pub async fn list_plaid_items(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<PlaidItemRow>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT id, institution_name, status::text, last_synced_at
        FROM plaid_items
        ORDER BY institution_name ASC
        "#,
    )
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(rows
        .into_iter()
        .map(|r| PlaidItemRow {
            id: r.get(0),
            institution_name: r.get(1),
            status: r.get::<String, _>(2),
            last_synced_at: r.get(3),
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Dashboard / reports
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DashboardKpis {
    pub total_assets_cents: i64,
    pub total_liabilities_cents: i64,
    pub total_equity_cents: i64,
    pub net_income_ytd_cents: i64,
    pub net_income_mtd_cents: i64,
    pub bank_count: i64,
    pub recent_entries: Vec<EntryRow>,
}

impl DashboardKpis {
    pub fn assets_display(&self) -> String { format_cents(self.total_assets_cents) }
    pub fn liabilities_display(&self) -> String { format_cents(self.total_liabilities_cents) }
    pub fn equity_display(&self) -> String { format_cents(self.total_equity_cents) }
    pub fn net_income_ytd_display(&self) -> String { format_cents(self.net_income_ytd_cents) }
    pub fn net_income_mtd_display(&self) -> String { format_cents(self.net_income_mtd_cents) }
    pub fn net_ytd_class(&self) -> &'static str {
        if self.net_income_ytd_cents > 0 { "pos" } else if self.net_income_ytd_cents < 0 { "neg" } else { "muted" }
    }
}

/// Sum of journal_lines.amount per account_type, restricted to non-void entries.
/// Returns a HashMap<account_type, sum_cents>.
async fn sum_by_account_type(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    end: Option<NaiveDate>,
    start: Option<NaiveDate>,
) -> Result<std::collections::HashMap<String, i64>, sqlx::Error> {
    let q = r#"
        SELECT a.account_type::text,
               COALESCE(SUM(CASE WHEN je.id IS NOT NULL THEN jl.amount ELSE 0 END), 0)::BIGINT
        FROM accounts a
        LEFT JOIN journal_lines jl ON jl.account_id = a.id
        LEFT JOIN journal_entries je ON je.id = jl.entry_id AND je.is_void = false
        WHERE a.is_active = true
          AND ($1::date IS NULL OR je.date <= $1)
          AND ($2::date IS NULL OR je.date >= $2)
        GROUP BY a.account_type
    "#;
    let rows = sqlx::query(q)
        .bind(end)
        .bind(start)
        .fetch_all(&mut **tx)
        .await?;
    let mut map = std::collections::HashMap::new();
    for r in rows {
        map.insert(r.get::<String, _>(0), r.get::<i64, _>(1));
    }
    Ok(map)
}

pub async fn dashboard_kpis(pool: &PgPool, company_id: Uuid) -> AppResult<DashboardKpis> {
    let today = chrono::Utc::now().date_naive();
    let year_start = NaiveDate::from_ymd_opt(today.year_ce().1 as i32, 1, 1).unwrap();
    let month_start = NaiveDate::from_ymd_opt(today.year_ce().1 as i32, today.month0() + 1, 1).unwrap();

    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;

    // All-time balances per type for asset/liability/equity (positive amounts = debit-normal).
    // Convention: assets/expenses are debit-normal so balance = sum(amount).
    // Liabilities/equity/revenue are credit-normal so balance = -sum(amount).
    let all_time = sum_by_account_type(&mut tx, None, None).await?;
    let assets = *all_time.get("asset").unwrap_or(&0);
    let liabs = -*all_time.get("liability").unwrap_or(&0);
    let equity = -*all_time.get("equity").unwrap_or(&0);

    // YTD net income = -(revenue + expense) where revenue is credit-normal and expense is debit-normal.
    // Net income increases equity, so positive net income = -sum(rev) - sum(exp) inverted:
    // revenue contributes -sum_amount (since credit-normal), expense contributes -sum_amount.
    // Net income = revenue - expense = (-sum_rev) - (sum_exp).
    let ytd = sum_by_account_type(&mut tx, Some(today), Some(year_start)).await?;
    let net_ytd = -ytd.get("revenue").copied().unwrap_or(0) - ytd.get("expense").copied().unwrap_or(0);

    let mtd = sum_by_account_type(&mut tx, Some(today), Some(month_start)).await?;
    let net_mtd = -mtd.get("revenue").copied().unwrap_or(0) - mtd.get("expense").copied().unwrap_or(0);

    let bank_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM plaid_items")
        .fetch_one(&mut *tx)
        .await?;

    let recent = sqlx::query(
        r#"
        SELECT je.id, je.date, je.memo, je.reference,
               COALESCE(SUM(GREATEST(jl.amount, 0)), 0)::BIGINT
        FROM journal_entries je
        LEFT JOIN journal_lines jl ON jl.entry_id = je.id
        WHERE je.is_void = false
        GROUP BY je.id, je.date, je.memo, je.reference
        ORDER BY je.date DESC, je.id DESC
        LIMIT 5
        "#,
    )
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    let recent_entries = recent
        .into_iter()
        .map(|r| EntryRow {
            id: r.get(0),
            date: r.get(1),
            memo: r.get::<Option<String>, _>(2).unwrap_or_default(),
            reference: r.get(3),
            total_debits_cents: r.get(4),
        })
        .collect();

    Ok(DashboardKpis {
        total_assets_cents: assets,
        total_liabilities_cents: liabs,
        total_equity_cents: equity,
        net_income_ytd_cents: net_ytd,
        net_income_mtd_cents: net_mtd,
        bank_count: bank_count.0,
        recent_entries,
    })
}

#[derive(Debug, Clone)]
pub struct ReportLine {
    pub account_id: Uuid,
    pub account_number: String,
    pub name: String,
    pub amount_cents: i64,
}
impl ReportLine {
    pub fn amount_display(&self) -> String { format_cents(self.amount_cents) }
}

#[derive(Debug, Clone)]
pub struct IncomeStatement {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub revenues: Vec<ReportLine>,
    pub expenses: Vec<ReportLine>,
    pub total_revenue_cents: i64,
    pub total_expense_cents: i64,
}
impl IncomeStatement {
    pub fn total_revenue_display(&self) -> String { format_cents(self.total_revenue_cents) }
    pub fn total_expense_display(&self) -> String { format_cents(self.total_expense_cents) }
    pub fn net_income_cents(&self) -> i64 { self.total_revenue_cents - self.total_expense_cents }
    pub fn net_income_display(&self) -> String { format_cents(self.net_income_cents()) }
    pub fn net_class(&self) -> &'static str {
        if self.net_income_cents() > 0 { "pos" } else if self.net_income_cents() < 0 { "neg" } else { "muted" }
    }
}

pub async fn income_statement(
    pool: &PgPool,
    company_id: Uuid,
    start: NaiveDate,
    end: NaiveDate,
) -> AppResult<IncomeStatement> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query(
        r#"
        SELECT a.account_type::text, a.account_number, a.name,
               SUM(jl.amount)::BIGINT AS sum_amount, a.id
        FROM accounts a
        JOIN journal_lines jl ON jl.account_id = a.id
        JOIN journal_entries je ON je.id = jl.entry_id
        WHERE a.is_active = true AND a.account_type IN ('revenue', 'expense')
          AND je.is_void = false AND je.date BETWEEN $1 AND $2
        GROUP BY a.id, a.account_type, a.account_number, a.name
        HAVING SUM(jl.amount) <> 0
        ORDER BY a.account_number ASC
        "#,
    )
    .bind(start)
    .bind(end)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    let mut revenues = Vec::new();
    let mut expenses = Vec::new();
    let mut total_rev = 0i64;
    let mut total_exp = 0i64;
    for r in rows {
        let acct_type: String = r.get(0);
        let sum: i64 = r.get(3);
        match acct_type.as_str() {
            "revenue" => {
                let amount = -sum; // credit-normal → positive
                total_rev += amount;
                revenues.push(ReportLine {
                    account_id: r.get(4),
                    account_number: r.get(1),
                    name: r.get(2),
                    amount_cents: amount,
                });
            }
            "expense" => {
                let amount = sum; // debit-normal → positive as expense
                total_exp += amount;
                expenses.push(ReportLine {
                    account_id: r.get(4),
                    account_number: r.get(1),
                    name: r.get(2),
                    amount_cents: amount,
                });
            }
            _ => {}
        }
    }
    Ok(IncomeStatement {
        start,
        end,
        revenues,
        expenses,
        total_revenue_cents: total_rev,
        total_expense_cents: total_exp,
    })
}

#[derive(Debug, Clone)]
pub struct BalanceSheet {
    pub as_of: NaiveDate,
    pub assets: Vec<ReportLine>,
    pub liabilities: Vec<ReportLine>,
    pub equity: Vec<ReportLine>,
    pub net_income_cents: i64,
    pub total_assets_cents: i64,
    pub total_liab_cents: i64,
    pub total_equity_cents: i64,
}
impl BalanceSheet {
    pub fn total_assets_display(&self) -> String { format_cents(self.total_assets_cents) }
    pub fn total_liab_display(&self) -> String { format_cents(self.total_liab_cents) }
    // "Total equity" must include current-year net income (shown as its own line
    // just above), so it ties to the equity section and to Total liab. + equity.
    pub fn total_equity_display(&self) -> String {
        format_cents(self.total_equity_cents + self.net_income_cents)
    }
    pub fn net_income_display(&self) -> String { format_cents(self.net_income_cents) }
    pub fn liab_plus_equity_cents(&self) -> i64 { self.total_liab_cents + self.total_equity_cents + self.net_income_cents }
    pub fn liab_plus_equity_display(&self) -> String { format_cents(self.liab_plus_equity_cents()) }
    pub fn balance_class(&self) -> &'static str {
        if self.total_assets_cents == self.liab_plus_equity_cents() { "ok" } else { "err" }
    }
    pub fn balance_msg(&self) -> String {
        if self.total_assets_cents == self.liab_plus_equity_cents() {
            "Balanced".to_string()
        } else {
            format!("Off by {}", format_cents((self.total_assets_cents - self.liab_plus_equity_cents()).abs()))
        }
    }
}

pub async fn balance_sheet(pool: &PgPool, company_id: Uuid, as_of: NaiveDate) -> AppResult<BalanceSheet> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;

    let rows = sqlx::query(
        r#"
        SELECT a.account_type::text, a.account_number, a.name,
               SUM(jl.amount)::BIGINT AS sum_amount, a.id
        FROM accounts a
        JOIN journal_lines jl ON jl.account_id = a.id
        JOIN journal_entries je ON je.id = jl.entry_id
        WHERE a.is_active = true
          AND a.account_type IN ('asset', 'liability', 'equity')
          AND je.is_void = false AND je.date <= $1
        GROUP BY a.id, a.account_type, a.account_number, a.name
        HAVING SUM(jl.amount) <> 0
        ORDER BY a.account_number ASC
        "#,
    )
    .bind(as_of)
    .fetch_all(&mut *tx)
    .await?;

    // Net income up through as_of for current calendar year.
    let year = as_of.format("%Y").to_string().parse::<i32>().unwrap_or(2026);
    let year_start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
    let ni_row: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COALESCE(SUM(CASE WHEN a.account_type = 'revenue' THEN -jl.amount ELSE 0 END), 0)::BIGINT,
            COALESCE(SUM(CASE WHEN a.account_type = 'expense' THEN jl.amount ELSE 0 END), 0)::BIGINT
        FROM journal_lines jl
        JOIN accounts a ON a.id = jl.account_id
        JOIN journal_entries je ON je.id = jl.entry_id AND je.is_void = false
        WHERE je.date BETWEEN $1 AND $2
        "#,
    )
    .bind(year_start)
    .bind(as_of)
    .fetch_one(&mut *tx)
    .await?;
    let net_income = ni_row.0 - ni_row.1;
    tx.commit().await?;

    let mut assets = Vec::new();
    let mut liabilities = Vec::new();
    let mut equity = Vec::new();
    let mut total_assets = 0i64;
    let mut total_liab = 0i64;
    let mut total_equity = 0i64;
    for r in rows {
        let t: String = r.get(0);
        let sum: i64 = r.get(3);
        match t.as_str() {
            "asset" => {
                total_assets += sum;
                assets.push(ReportLine { account_id: r.get(4), account_number: r.get(1), name: r.get(2), amount_cents: sum });
            }
            "liability" => {
                let amt = -sum;
                total_liab += amt;
                liabilities.push(ReportLine { account_id: r.get(4), account_number: r.get(1), name: r.get(2), amount_cents: amt });
            }
            "equity" => {
                let amt = -sum;
                total_equity += amt;
                equity.push(ReportLine { account_id: r.get(4), account_number: r.get(1), name: r.get(2), amount_cents: amt });
            }
            _ => {}
        }
    }

    Ok(BalanceSheet {
        as_of,
        assets,
        liabilities,
        equity,
        net_income_cents: net_income,
        total_assets_cents: total_assets,
        total_liab_cents: total_liab,
        total_equity_cents: total_equity,
    })
}

#[derive(Debug, Clone)]
pub struct CashFlow {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub opening_cash_cents: i64,
    pub closing_cash_cents: i64,
    pub change_cents: i64,
    pub by_other_account: Vec<ReportLine>,
}
impl CashFlow {
    pub fn opening_display(&self) -> String { format_cents(self.opening_cash_cents) }
    pub fn closing_display(&self) -> String { format_cents(self.closing_cash_cents) }
    pub fn change_display(&self) -> String { format_cents(self.change_cents) }
    pub fn change_class(&self) -> &'static str {
        if self.change_cents > 0 { "pos" } else if self.change_cents < 0 { "neg" } else { "muted" }
    }
}

/// Simplified cash flow: net change in all asset accounts whose name contains "cash" or "bank",
/// plus a breakdown by counterpart account. Real GAAP statement-of-cash-flows is v2.
pub async fn cash_flow(
    pool: &PgPool,
    company_id: Uuid,
    start: NaiveDate,
    end: NaiveDate,
) -> AppResult<CashFlow> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;

    // Sum cash account balances at end and start.
    let opening: (i64,) = sqlx::query_as(
        r#"
        SELECT COALESCE(SUM(jl.amount), 0)::BIGINT
        FROM journal_lines jl
        JOIN accounts a ON a.id = jl.account_id
        JOIN journal_entries je ON je.id = jl.entry_id AND je.is_void = false
        WHERE a.account_type = 'asset'
          AND (a.name ILIKE '%cash%' OR a.name ILIKE '%bank%' OR a.name ILIKE '%checking%' OR a.name ILIKE '%savings%'
               OR a.id IN (SELECT local_account_id FROM plaid_local_accounts WHERE local_account_id IS NOT NULL))
          AND je.date < $1
        "#,
    )
    .bind(start)
    .fetch_one(&mut *tx)
    .await?;
    let closing: (i64,) = sqlx::query_as(
        r#"
        SELECT COALESCE(SUM(jl.amount), 0)::BIGINT
        FROM journal_lines jl
        JOIN accounts a ON a.id = jl.account_id
        JOIN journal_entries je ON je.id = jl.entry_id AND je.is_void = false
        WHERE a.account_type = 'asset'
          AND (a.name ILIKE '%cash%' OR a.name ILIKE '%bank%' OR a.name ILIKE '%checking%' OR a.name ILIKE '%savings%'
               OR a.id IN (SELECT local_account_id FROM plaid_local_accounts WHERE local_account_id IS NOT NULL))
          AND je.date <= $1
        "#,
    )
    .bind(end)
    .fetch_one(&mut *tx)
    .await?;

    // Counterpart accounts movement during the period (entries that touch a cash account):
    let breakdown = sqlx::query(
        r#"
        WITH cash_entries AS (
            SELECT DISTINCT je.id AS entry_id
            FROM journal_entries je
            JOIN journal_lines jl ON jl.entry_id = je.id
            JOIN accounts a ON a.id = jl.account_id
            WHERE je.is_void = false
              AND je.date BETWEEN $1 AND $2
              AND a.account_type = 'asset'
              AND (a.name ILIKE '%cash%' OR a.name ILIKE '%bank%' OR a.name ILIKE '%checking%' OR a.name ILIKE '%savings%'
                   OR a.id IN (SELECT local_account_id FROM plaid_local_accounts WHERE local_account_id IS NOT NULL))
        )
        SELECT a.account_number, a.name, COALESCE(SUM(jl.amount), 0)::BIGINT AS amt, a.id
        FROM cash_entries ce
        JOIN journal_lines jl ON jl.entry_id = ce.entry_id
        JOIN accounts a ON a.id = jl.account_id
        WHERE NOT (a.account_type = 'asset'
                   AND (a.name ILIKE '%cash%' OR a.name ILIKE '%bank%' OR a.name ILIKE '%checking%' OR a.name ILIKE '%savings%'
                        OR a.id IN (SELECT local_account_id FROM plaid_local_accounts WHERE local_account_id IS NOT NULL)))
        GROUP BY a.id, a.account_number, a.name
        HAVING COALESCE(SUM(jl.amount), 0) <> 0
        ORDER BY a.account_number ASC
        "#,
    )
    .bind(start)
    .bind(end)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    let by_other_account: Vec<ReportLine> = breakdown
        .into_iter()
        .map(|r| ReportLine {
            account_id: r.get(3),
            account_number: r.get(0),
            name: r.get(1),
            amount_cents: -r.get::<i64, _>(2), // sign-flip: cash inflow corresponds to credit on counterpart
        })
        .collect();

    Ok(CashFlow {
        start,
        end,
        opening_cash_cents: opening.0,
        closing_cash_cents: closing.0,
        change_cents: closing.0 - opening.0,
        by_other_account,
    })
}

// ---------------------------------------------------------------------------
// Transactions (line-level)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TransactionLine {
    pub line_id: Uuid,
    pub entry_id: Uuid,
    pub date: NaiveDate,
    pub memo: String,
    /// `memo` with wallet addresses shortened to 0x123…last5 and, when known,
    /// prefixed with the address-book name. Set by the web handler; defaults to memo.
    pub memo_display: String,
    pub reference: Option<String>,
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    pub amount_cents: i64,
    pub currency: String,
    pub source: Option<String>,
    pub is_void: bool,
    /// Vendor/payee from the vendors master (via journal_lines.vendor_id), if tagged.
    pub vendor_name: Option<String>,
    /// User-assigned category tag on the entry (e.g. "Loan"), if set.
    pub category: Option<String>,
}

impl TransactionLine {
    pub fn amount_display(&self) -> String { format_cents(self.amount_cents) }
    pub fn debit_display(&self) -> String {
        if self.amount_cents > 0 { format_cents(self.amount_cents) } else { String::new() }
    }
    pub fn credit_display(&self) -> String {
        if self.amount_cents < 0 { format_cents(-self.amount_cents) } else { String::new() }
    }
    pub fn source_label(&self) -> &str {
        self.source.as_deref().unwrap_or("manual")
    }
}

#[derive(Debug, Clone, Default)]
pub struct TransactionFilter {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
    /// Filter to one or more accounts. Empty = all accounts.
    pub account_ids: Vec<Uuid>,
    pub source: Option<String>,
    pub search: Option<String>,
    pub include_void: bool,
    /// "debit" (money in on the bank side) or "credit" (money out).
    pub direction: Option<String>,
    /// Absolute-amount bounds, in cents.
    pub min_cents: Option<i64>,
    pub max_cents: Option<i64>,
    /// Sort order: "date_desc" (default), "date_asc", "amount_desc", "amount_asc".
    pub sort: Option<String>,
    /// Filter to a vendor by name (substring match on the vendors master).
    pub vendor: Option<String>,
    /// Filter to a user-assigned entry category (exact match), if set.
    pub category: Option<String>,
}

pub async fn list_transactions(
    pool: &PgPool,
    company_id: Uuid,
    filter: &TransactionFilter,
) -> AppResult<Vec<TransactionLine>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;

    // Validated against a fixed allow-list, so it's safe to interpolate into SQL.
    let order_by = match filter.sort.as_deref() {
        Some("date_asc") => "je.date ASC, je.id ASC, jl.amount DESC",
        Some("amount_desc") => "abs(jl.amount) DESC, je.date DESC, je.id DESC",
        Some("amount_asc") => "abs(jl.amount) ASC, je.date DESC, je.id DESC",
        _ => "je.date DESC, je.id DESC, jl.amount DESC",
    };

    // When no specific account is picked we show only the asset/liability "bank side"
    // of each entry — otherwise every Plaid transaction shows up twice (bank line +
    // Uncategorized counterpart). Filtering by a specific account always wins.
    let rows = sqlx::query(&format!(
        r#"
        SELECT jl.id, je.id, je.date, je.memo, je.reference,
               a.id, a.account_number, a.name,
               jl.amount, jl.currency,
               je.source::text, je.is_void, v.name, ec.category
        FROM journal_lines jl
        JOIN journal_entries je ON je.id = jl.entry_id
        JOIN accounts a ON a.id = jl.account_id
        LEFT JOIN vendors v ON v.id = jl.vendor_id
        LEFT JOIN entry_categories ec ON ec.entry_id = je.id
        WHERE ($1::date IS NULL OR je.date >= $1)
          AND ($2::date IS NULL OR je.date <= $2)
          AND (cardinality($3::uuid[]) = 0 OR jl.account_id = ANY($3::uuid[]))
          -- Default view shows only the bank side (asset/liability) so Plaid entries don't
          -- double-list. A specific account OR a vendor filter overrides that (a vendor is
          -- tagged on the expense line, which would otherwise be excluded here).
          AND (cardinality($3::uuid[]) > 0 OR $10::text IS NOT NULL OR a.account_type IN ('asset', 'liability'))
          AND ($4::text IS NULL OR je.source::text = $4)
          -- Keyword search across the memo, reference, category, and any line's
          -- account (name/number) or vendor. EXISTS keeps it at the entry level
          -- so matches on a non-bank line still show the (bank-side) row once.
          AND ($5::text IS NULL OR (
                 je.memo ILIKE '%' || $5 || '%'
              OR je.reference ILIKE '%' || $5 || '%'
              OR ec.category ILIKE '%' || $5 || '%'
              OR EXISTS (
                   SELECT 1 FROM journal_lines jl2
                   JOIN accounts a2 ON a2.id = jl2.account_id
                   LEFT JOIN vendors v2 ON v2.id = jl2.vendor_id
                   WHERE jl2.entry_id = je.id
                     AND (a2.name ILIKE '%' || $5 || '%'
                       OR a2.account_number ILIKE '%' || $5 || '%'
                       OR v2.name ILIKE '%' || $5 || '%')
                 )
          ))
          AND ($6::boolean = true OR je.is_void = false)
          AND ($7::text IS NULL
               OR ($7 = 'debit' AND jl.amount > 0)
               OR ($7 = 'credit' AND jl.amount < 0))
          AND ($8::bigint IS NULL OR abs(jl.amount) >= $8)
          AND ($9::bigint IS NULL OR abs(jl.amount) <= $9)
          AND ($10::text IS NULL OR v.name ILIKE '%' || $10 || '%')
          AND ($11::text IS NULL OR ec.category = $11)
        ORDER BY {order_by}
        LIMIT 500
        "#,
    ))
    .bind(filter.start)
    .bind(filter.end)
    .bind(&filter.account_ids)
    .bind(filter.source.as_deref())
    .bind(filter.search.as_deref())
    .bind(filter.include_void)
    .bind(filter.direction.as_deref())
    .bind(filter.min_cents)
    .bind(filter.max_cents)
    .bind(filter.vendor.as_deref())
    .bind(filter.category.as_deref())
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;

    Ok(rows
        .into_iter()
        .map(|r| TransactionLine {
            line_id: r.get(0),
            entry_id: r.get(1),
            date: r.get(2),
            memo: r.get::<Option<String>, _>(3).unwrap_or_default(),
            memo_display: r.get::<Option<String>, _>(3).unwrap_or_default(),
            reference: r.get(4),
            account_id: r.get(5),
            account_number: r.get(6),
            account_name: r.get(7),
            amount_cents: r.get(8),
            currency: r.get(9),
            source: r.get(10),
            is_void: r.get(11),
            vendor_name: r.get(12),
            category: r.get(13),
        })
        .collect())
}

/// Upsert (or clear, when `category` is empty) the category tag on an entry.
pub async fn set_entry_category(
    pool: &PgPool,
    company_id: Uuid,
    entry_id: Uuid,
    category: &str,
) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let cat = category.trim();
    if cat.is_empty() {
        sqlx::query("DELETE FROM entry_categories WHERE company_id = $1 AND entry_id = $2")
            .bind(company_id)
            .bind(entry_id)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query(
            "INSERT INTO entry_categories (company_id, entry_id, category) VALUES ($1, $2, $3)
             ON CONFLICT (company_id, entry_id) DO UPDATE SET category = EXCLUDED.category, updated_at = now()",
        )
        .bind(company_id)
        .bind(entry_id)
        .bind(cat)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Distinct category tags used by a company (for filter dropdowns / suggestions).
pub async fn list_entry_categories(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<String>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT category FROM entry_categories WHERE company_id = $1 ORDER BY category",
    )
    .bind(company_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows)
}

// ── Vendors (master list + per-company booked totals from the ledger) ──
#[derive(Debug, Clone)]
pub struct VendorRow {
    pub id: Uuid,
    pub name: String,
    pub country: String,
    pub tax_form: String,
    pub tax_form_on_file: bool,
    pub contract_on_file: bool,
    pub default_account_code: String,
    pub booked_cents: i64,
    pub tx_count: i64,
    pub first_paid: Option<NaiveDate>,
    pub last_paid: Option<NaiveDate>,
}

impl VendorRow {
    pub fn booked_display(&self) -> String {
        format_cents(self.booked_cents)
    }
    pub fn first_paid_display(&self) -> String {
        self.first_paid.map(|d| d.to_string()).unwrap_or_else(|| "—".into())
    }
    pub fn last_paid_display(&self) -> String {
        self.last_paid.map(|d| d.to_string()).unwrap_or_else(|| "—".into())
    }
    pub fn w8_label(&self) -> &str {
        if self.tax_form_on_file { "on file" } else { "MISSING" }
    }
}

/// The vendors master joined with this company's booked totals (sum of non-void
/// journal lines tagged to each vendor). Vendors with no activity here show $0.
pub async fn list_vendors_with_totals(
    pool: &PgPool,
    company_id: Uuid,
    sort: &str,
) -> AppResult<Vec<VendorRow>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    // Validated against a fixed allow-list, so it's safe to interpolate. NULLS LAST keeps
    // never-paid vendors at the bottom regardless of direction.
    let order_by = match sort {
        "paid_asc" => "booked ASC NULLS FIRST, v.name ASC",
        "name" => "v.name ASC",
        "first_desc" => "first_paid DESC NULLS LAST, v.name ASC",
        "first_asc" => "first_paid ASC NULLS LAST, v.name ASC",
        "last_desc" => "last_paid DESC NULLS LAST, v.name ASC",
        "last_asc" => "last_paid ASC NULLS LAST, v.name ASC",
        "txns_desc" => "n DESC, v.name ASC",
        _ => "booked DESC NULLS LAST, v.name ASC", // paid_desc (default)
    };
    let rows = sqlx::query(&format!(
        r#"
        SELECT v.id, v.name, COALESCE(v.country,''), COALESCE(v.required_tax_form,''),
               COALESCE(v.tax_form_on_file,false), COALESCE(v.contract_on_file,false),
               COALESCE(v.default_account_code,''),
               COALESCE(SUM(CASE WHEN je.is_void = false THEN jl.amount ELSE 0 END), 0)::bigint AS booked,
               COUNT(CASE WHEN je.is_void = false THEN jl.id END)::bigint AS n,
               MIN(CASE WHEN je.is_void = false THEN je.date END) AS first_paid,
               MAX(CASE WHEN je.is_void = false THEN je.date END) AS last_paid
        FROM vendors v
        LEFT JOIN journal_lines jl ON jl.vendor_id = v.id
        LEFT JOIN journal_entries je ON je.id = jl.entry_id
        GROUP BY v.id, v.name, v.country, v.required_tax_form,
                 v.tax_form_on_file, v.contract_on_file, v.default_account_code
        ORDER BY {order_by}
        "#,
    ))
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows
        .into_iter()
        .map(|r| VendorRow {
            id: r.get(0),
            name: r.get(1),
            country: r.get(2),
            tax_form: r.get(3),
            tax_form_on_file: r.get(4),
            contract_on_file: r.get(5),
            default_account_code: r.get(6),
            booked_cents: r.get(7),
            tx_count: r.get(8),
            first_paid: r.get(9),
            last_paid: r.get(10),
        })
        .collect())
}

// ── Entry detail (clickable transaction → full entry + provenance + vendor) ──

#[derive(Debug, Clone)]
pub struct EntryLineRow {
    pub line_id: Uuid,
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    pub amount_cents: i64,
    pub currency: String,
    pub memo: Option<String>,
}

/// One side (from/to) of a transaction: an account, optionally a crypto wallet.
#[derive(Debug, Clone)]
pub struct Endpoint {
    pub account_id: Uuid,
    pub account_number: String,
    pub account_name: String,
    /// Full wallet address, when this is a crypto entry.
    pub wallet: Option<String>,
    /// Address-book name for the wallet, when known (e.g. "BitGetDeposit").
    pub wallet_name: Option<String>,
    /// Block-explorer link for the wallet, when known.
    pub wallet_url: Option<String>,
}

impl EntryLineRow {
    pub fn debit_display(&self) -> String {
        if self.amount_cents > 0 { format_cents(self.amount_cents) } else { String::new() }
    }
    pub fn credit_display(&self) -> String {
        if self.amount_cents < 0 { format_cents(-self.amount_cents) } else { String::new() }
    }
}

/// On-chain provenance for a crypto entry (from crypto_provenance).
#[derive(Debug, Clone)]
pub struct CryptoProvenance {
    pub tx_hash: String,
    pub chain: String,
    pub explorer_url: String,
    pub verified: bool,
    pub verify_error: Option<String>,
    pub counterparty: Option<String>,
    pub symbol: Option<String>,
    pub direction: Option<String>,
    pub from_address: Option<String>,
    pub to_address: Option<String>,
    /// Address-book names for the on-chain from/to wallets (filled by the handler).
    pub from_name: Option<String>,
    pub to_name: Option<String>,
}

impl CryptoProvenance {
    fn addr_url(&self, addr: &Option<String>) -> Option<String> {
        let a = addr.as_ref()?;
        let base = match self.chain.as_str() {
            "bsc" => "https://bscscan.com/address/",
            _ => "https://etherscan.io/address/",
        };
        Some(format!("{base}{a}"))
    }
    pub fn from_url(&self) -> Option<String> { self.addr_url(&self.from_address) }
    pub fn to_url(&self) -> Option<String> { self.addr_url(&self.to_address) }
}

/// A vendor/counterparty resolved from the address book.
#[derive(Debug, Clone)]
pub struct VendorLink {
    pub address: String,
    pub name: String,
    pub kind: String,
    pub account_code: String,
}

/// A supporting document (receipt, invoice, contract, …) attached to an entry
/// via `entry_documents` → `company_files`.
#[derive(Debug, Clone)]
pub struct EntryDocument {
    pub file_id: Uuid,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    /// The link's classification (invoice, receipt, contract, other …).
    pub doc_type: String,
    pub note: Option<String>,
}

impl EntryDocument {
    /// Human-readable file size, e.g. "12.3 KB".
    pub fn size_display(&self) -> String {
        let b = self.size_bytes as f64;
        if b < 1024.0 {
            format!("{} B", self.size_bytes)
        } else if b < 1024.0 * 1024.0 {
            format!("{:.1} KB", b / 1024.0)
        } else {
            format!("{:.1} MB", b / (1024.0 * 1024.0))
        }
    }
    /// True when this document is a viewable raster image.
    pub fn is_image(&self) -> bool {
        self.content_type.starts_with("image/")
    }
}

#[derive(Debug, Clone)]
pub struct EntryDetail {
    pub id: Uuid,
    pub date: NaiveDate,
    pub memo: String,
    pub memo_display: String,
    pub reference: Option<String>,
    pub source: Option<String>,
    pub is_void: bool,
    pub lines: Vec<EntryLineRow>,
    pub crypto: Option<CryptoProvenance>,
    pub vendor: Option<VendorLink>,
    /// Where this entry came from (entry_sources): statement | plaid | wise | reclass | crypto | manual.
    pub source_kind: Option<String>,
    /// Source statement file name, when the entry came from a parsed/uploaded statement.
    pub source_file: Option<String>,
    /// The exact original description / reference from the statement or source line.
    pub source_detail: Option<String>,
    /// company_files id for the source statement, so the UI can link to the stored PDF.
    pub source_file_id: Option<Uuid>,
    /// Money flowed from here (the credited account / sending wallet).
    pub from: Option<Endpoint>,
    /// …to here (the debited account / receiving wallet).
    pub to: Option<Endpoint>,
    /// Supporting documents attached to this entry (receipts, invoices, …).
    pub documents: Vec<EntryDocument>,
    /// Free-text user note on this transaction (entry_notes), if any.
    pub note: Option<String>,
}

impl EntryDetail {
    pub fn total_debits(&self) -> String {
        format_cents(self.lines.iter().filter(|l| l.amount_cents > 0).map(|l| l.amount_cents).sum())
    }
    pub fn total_credits(&self) -> String {
        format_cents(self.lines.iter().filter(|l| l.amount_cents < 0).map(|l| -l.amount_cents).sum())
    }
    pub fn source_label(&self) -> &str {
        self.source.as_deref().unwrap_or("manual")
    }
}

/// Full detail for one journal entry: all lines, on-chain provenance (if a crypto
/// entry), the source statement file (if imported), and the vendor (address book).
pub async fn get_entry_detail(
    pool: &PgPool,
    company_id: Uuid,
    entry_id: Uuid,
) -> AppResult<Option<EntryDetail>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;

    let hdr = sqlx::query(
        "SELECT date, memo, reference, source::text, is_void
         FROM journal_entries WHERE id = $1",
    )
    .bind(entry_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(h) = hdr else {
        tx.commit().await?;
        return Ok(None);
    };
    let memo: String = h.get::<Option<String>, _>(1).unwrap_or_default();
    let reference: Option<String> = h.get(2);
    let source: Option<String> = h.get(3);

    let line_rows = sqlx::query(
        "SELECT jl.id, jl.account_id, a.account_number, a.name, jl.amount, jl.currency, jl.memo
         FROM journal_lines jl JOIN accounts a ON a.id = jl.account_id
         WHERE jl.entry_id = $1 ORDER BY jl.amount DESC",
    )
    .bind(entry_id)
    .fetch_all(&mut *tx)
    .await?;
    let lines: Vec<EntryLineRow> = line_rows
        .into_iter()
        .map(|r| EntryLineRow {
            line_id: r.get(0),
            account_id: r.get(1),
            account_number: r.get(2),
            account_name: r.get(3),
            amount_cents: r.get(4),
            currency: r.get(5),
            memo: r.get(6),
        })
        .collect();

    let crypto = sqlx::query(
        "SELECT tx_hash, chain, explorer_url, verified, verify_error, counterparty, symbol,
                direction, from_address, to_address
         FROM crypto_provenance WHERE entry_id = $1",
    )
    .bind(entry_id)
    .fetch_optional(&mut *tx)
    .await?
    .map(|r| CryptoProvenance {
        tx_hash: r.get(0),
        chain: r.get(1),
        explorer_url: r.get(2),
        verified: r.get(3),
        verify_error: r.get(4),
        counterparty: r.get(5),
        symbol: r.get(6),
        direction: r.get(7),
        from_address: r.get(8),
        to_address: r.get(9),
        from_name: None,
        to_name: None,
    });

    // Source provenance (statement file / plaid / wise / reclass / manual).
    let (source_kind, source_file, source_detail) = sqlx::query(
        "SELECT source_kind, source_file, source_detail FROM entry_sources WHERE entry_id = $1",
    )
    .bind(entry_id)
    .fetch_optional(&mut *tx)
    .await?
    .map(|r| {
        (
            r.get::<String, _>(0).into(),
            r.get::<Option<String>, _>(1),
            r.get::<Option<String>, _>(2),
        )
    })
    .unwrap_or((None, None, None));

    // Resolve the source statement to its stored file (for a download link), including the
    // `upload:<name>` reference fallback used by some imports.
    let source_file = source_file.or_else(|| {
        reference
            .as_deref()
            .and_then(|r| r.strip_prefix("upload:"))
            .map(|s| s.to_string())
    });
    let source_file_id: Option<Uuid> = if let Some(ref f) = source_file {
        sqlx::query(
            "SELECT id FROM company_files WHERE company_id = $1 AND filename = $2 ORDER BY uploaded_at DESC LIMIT 1",
        )
        .bind(company_id)
        .bind(f)
        .fetch_optional(&mut *tx)
        .await?
        .map(|r| r.get::<Uuid, _>(0))
    } else {
        None
    };

    // Vendor: prefer the crypto counterparty; otherwise the first 0x address in the memo.
    let vendor_addr = crypto
        .as_ref()
        .and_then(|c| c.counterparty.clone())
        .or_else(|| first_wallet_address(&memo));
    let mut vendor = if let Some(addr) = vendor_addr {
        sqlx::query(
            "SELECT address, name, kind, account_code FROM address_labels
             WHERE address = lower($1)",
        )
        .bind(&addr)
        .fetch_optional(&mut *tx)
        .await?
        .map(|r| VendorLink {
            address: r.get(0),
            name: r.get(1),
            kind: r.get(2),
            account_code: r.get(3),
        })
    } else {
        None
    };

    // Fall back to the structured vendor (journal_lines.vendor_id → vendors master).
    if vendor.is_none() {
        vendor = sqlx::query(
            "SELECT v.name, COALESCE(v.vendor_type,''), COALESCE(v.default_account_code,'')
             FROM journal_lines jl JOIN vendors v ON v.id = jl.vendor_id
             WHERE jl.entry_id = $1 AND jl.vendor_id IS NOT NULL
             LIMIT 1",
        )
        .bind(entry_id)
        .fetch_optional(&mut *tx)
        .await?
        .map(|r| VendorLink {
            address: String::new(),
            name: r.get(0),
            kind: r.get(1),
            account_code: r.get(2),
        });
    }

    // Supporting documents attached to this entry (entry_documents → company_files).
    let documents: Vec<EntryDocument> = sqlx::query(
        "SELECT cf.id, cf.filename, cf.content_type, cf.size_bytes, ed.doc_type, ed.note
         FROM entry_documents ed JOIN company_files cf ON cf.id = ed.file_id
         WHERE ed.entry_id = $1
         ORDER BY ed.linked_at ASC",
    )
    .bind(entry_id)
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|r| EntryDocument {
        file_id: r.get(0),
        filename: r.get(1),
        content_type: r.get(2),
        size_bytes: r.get(3),
        doc_type: r.get(4),
        note: r.get(5),
    })
    .collect();

    let note: Option<String> = sqlx::query("SELECT note FROM entry_notes WHERE entry_id = $1")
        .bind(entry_id)
        .fetch_optional(&mut *tx)
        .await?
        .map(|r| r.get(0));

    tx.commit().await?;

    // From → To: money flows from the credited line (most negative) to the debited
    // line (most positive). For crypto entries, attach the full sending/receiving
    // wallet + an explorer address link.
    let addr_base = crypto.as_ref().map(|c| {
        if c.chain == "bsc" { "https://bscscan.com/address/" } else { "https://etherscan.io/address/" }
    });
    fn endpoint(l: &EntryLineRow, wallet: Option<String>, base: Option<&str>) -> Endpoint {
        let wallet_url = match (&wallet, base) {
            (Some(w), Some(b)) => Some(format!("{b}{w}")),
            _ => None,
        };
        Endpoint {
            account_id: l.account_id,
            account_number: l.account_number.clone(),
            account_name: l.account_name.clone(),
            wallet,
            wallet_name: None, // filled by the handler from the address book
            wallet_url,
        }
    }
    let from = lines
        .iter()
        .min_by_key(|l| l.amount_cents)
        .map(|l| endpoint(l, crypto.as_ref().and_then(|c| c.from_address.clone()), addr_base));
    let to = lines
        .iter()
        .max_by_key(|l| l.amount_cents)
        .map(|l| endpoint(l, crypto.as_ref().and_then(|c| c.to_address.clone()), addr_base));

    Ok(Some(EntryDetail {
        id: entry_id,
        date: h.get(0),
        memo_display: memo.clone(),
        memo,
        reference,
        source,
        is_void: h.get(4),
        lines,
        crypto,
        vendor,
        source_kind,
        source_file,
        source_detail,
        source_file_id,
        from,
        to,
        documents,
        note,
    }))
}

/// Upsert (or clear, when empty) the free-text note on a transaction.
pub async fn set_entry_note(
    pool: &PgPool,
    company_id: Uuid,
    entry_id: Uuid,
    note: &str,
) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let note = note.trim();
    if note.is_empty() {
        sqlx::query("DELETE FROM entry_notes WHERE company_id = $1 AND entry_id = $2")
            .bind(company_id)
            .bind(entry_id)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query(
            "INSERT INTO entry_notes (company_id, entry_id, note) VALUES ($1, $2, $3)
             ON CONFLICT (company_id, entry_id) DO UPDATE SET note = EXCLUDED.note, updated_at = now()",
        )
        .bind(company_id)
        .bind(entry_id)
        .bind(note)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Link an already-stored company file to a journal entry as a supporting
/// document. Idempotent on (entry_id, file_id).
pub async fn link_entry_document(
    pool: &PgPool,
    company_id: Uuid,
    entry_id: Uuid,
    file_id: Uuid,
    doc_type: &str,
) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query(
        "INSERT INTO entry_documents (company_id, entry_id, file_id, doc_type)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (entry_id, file_id) DO NOTHING",
    )
    .bind(company_id)
    .bind(entry_id)
    .bind(file_id)
    .bind(doc_type)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Detach a supporting document from an entry (the file itself stays in the
/// company document store).
pub async fn unlink_entry_document(
    pool: &PgPool,
    company_id: Uuid,
    entry_id: Uuid,
    file_id: Uuid,
) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query("DELETE FROM entry_documents WHERE entry_id = $1 AND file_id = $2")
        .bind(entry_id)
        .bind(file_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Set the classification + note on a linked supporting document (after the AI
/// has parsed it).
pub async fn annotate_entry_document(
    pool: &PgPool,
    company_id: Uuid,
    entry_id: Uuid,
    file_id: Uuid,
    doc_type: &str,
    note: &str,
) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query(
        "UPDATE entry_documents SET doc_type = $3, note = $4
         WHERE entry_id = $1 AND file_id = $2",
    )
    .bind(entry_id)
    .bind(file_id)
    .bind(doc_type)
    .bind(note)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Lightweight transaction context (for AI document reconciliation): the memo,
/// date, signed line amounts by account, and any current category.
pub async fn entry_context_text(
    pool: &PgPool,
    company_id: Uuid,
    entry_id: Uuid,
) -> AppResult<Option<String>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let hdr = sqlx::query(
        "SELECT je.date, je.memo, ec.category
         FROM journal_entries je
         LEFT JOIN entry_categories ec ON ec.entry_id = je.id
         WHERE je.id = $1",
    )
    .bind(entry_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(h) = hdr else {
        tx.commit().await?;
        return Ok(None);
    };
    let date: NaiveDate = h.get(0);
    let memo: String = h.get::<Option<String>, _>(1).unwrap_or_default();
    let category: Option<String> = h.get(2);
    let lines = sqlx::query(
        "SELECT a.account_number, a.name, jl.amount
         FROM journal_lines jl JOIN accounts a ON a.id = jl.account_id
         WHERE jl.entry_id = $1 ORDER BY jl.amount DESC",
    )
    .bind(entry_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    let mut out = format!("Date: {date}\nMemo: {memo}\n");
    if let Some(c) = category {
        out.push_str(&format!("Current category: {c}\n"));
    }
    out.push_str("Journal lines (cents, debit +, credit -):\n");
    for l in &lines {
        let num: String = l.get(0);
        let name: String = l.get(1);
        let amt: i64 = l.get(2);
        out.push_str(&format!("  {num} {name}: {}\n", format_cents(amt)));
    }
    Ok(Some(out))
}

/// First `0x…` hex wallet address (≥12 chars) found in a string, lowercased.
fn first_wallet_address(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut i = 0;
    while i + 2 < s.len() {
        if b[i] == b'0' && (b[i + 1] | 0x20) == b'x' {
            let mut j = i + 2;
            while j < s.len() && b[j].is_ascii_hexdigit() {
                j += 1;
            }
            if j - i >= 12 {
                return Some(s[i..j].to_lowercase());
            }
            i = j;
        } else {
            i += 1;
        }
    }
    None
}

/// Resolve the user's first company membership.
pub async fn resolve_company_id(pool: &PgPool, user_id: Uuid) -> AppResult<Option<Uuid>> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT company_id FROM memberships WHERE user_id = $1 ORDER BY created_at ASC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(id,)| id))
}

#[derive(Debug, Clone)]
pub struct CompanyRow {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub role: String,
}

pub async fn list_companies_for_user(pool: &PgPool, user_id: Uuid) -> AppResult<Vec<CompanyRow>> {
    let rows = sqlx::query(
        r#"
        SELECT c.id, c.name, c.slug, m.role::text
        FROM memberships m
        JOIN companies c ON c.id = m.company_id
        WHERE m.user_id = $1
        ORDER BY c.name ASC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| CompanyRow {
            id: r.get(0),
            name: r.get(1),
            slug: r.get(2),
            role: r.get(3),
        })
        .collect())
}

pub async fn user_has_membership(pool: &PgPool, user_id: Uuid, company_id: Uuid) -> AppResult<bool> {
    let row: Option<(i32,)> = sqlx::query_as(
        "SELECT 1 FROM memberships WHERE user_id = $1 AND company_id = $2",
    )
    .bind(user_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.is_some())
}

#[derive(Debug, Clone)]
pub struct MemberRow {
    pub user_id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
}

pub async fn list_members(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<MemberRow>> {
    let rows = sqlx::query(
        r#"
        SELECT u.id, u.email, u.name, m.role::text
        FROM memberships m
        JOIN auth_users u ON u.id = m.user_id
        WHERE m.company_id = $1
        ORDER BY m.role ASC, u.email ASC
        "#,
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| MemberRow {
            user_id: r.get(0),
            email: r.get(1),
            name: r.get(2),
            role: r.get(3),
        })
        .collect())
}

/// Create a new company with the given name, owned by user_id who becomes 'owner'.
pub async fn create_company(pool: &PgPool, user_id: Uuid, name: &str) -> AppResult<Uuid> {
    let mut tx = pool.begin().await?;
    let slug_base: String = name
        .chars()
        .filter_map(|c| {
            if c.is_ascii_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c.is_whitespace() || c == '-' || c == '_' {
                Some('-')
            } else {
                None
            }
        })
        .collect();
    let slug_base = slug_base.trim_matches('-').to_string();
    let slug_base = if slug_base.is_empty() { "company".to_string() } else { slug_base };
    let suffix = &Uuid::new_v4().simple().to_string()[..8];
    let slug = format!("{}-{}", slug_base.chars().take(31).collect::<String>(), suffix);
    let row: (Uuid,) = sqlx::query_as(
        "INSERT INTO companies (slug, name, owner_user_id) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(&slug)
    .bind(name)
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO memberships (user_id, company_id, role) VALUES ($1, $2, 'owner')")
        .bind(user_id)
        .bind(row.0)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(row.0)
}

/// Get current user's role in a given company.
pub async fn user_role_in(pool: &PgPool, user_id: Uuid, company_id: Uuid) -> AppResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT role::text FROM memberships WHERE user_id = $1 AND company_id = $2",
    )
    .bind(user_id)
    .bind(company_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(r,)| r))
}

/// True when role grants write/admin authority over membership/settings.
pub fn role_can_admin(role: &str) -> bool {
    matches!(role, "owner" | "admin")
}

pub async fn update_company_settings(
    pool: &PgPool,
    company_id: Uuid,
    name: &str,
    base_currency: &str,
    fiscal_year_start_month: i16,
) -> AppResult<()> {
    sqlx::query(
        "UPDATE companies SET name = $1, base_currency = $2, fiscal_year_start_month = $3, updated_at = now() WHERE id = $4",
    )
    .bind(name.trim())
    .bind(base_currency.trim())
    .bind(fiscal_year_start_month)
    .bind(company_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_company(pool: &PgPool, company_id: Uuid) -> AppResult<Option<(String, String, i16)>> {
    let row: Option<(String, String, i16)> = sqlx::query_as(
        "SELECT name, base_currency, fiscal_year_start_month FROM companies WHERE id = $1",
    )
    .bind(company_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn remove_member(pool: &PgPool, company_id: Uuid, user_id: Uuid) -> AppResult<()> {
    // Prevent removing the last owner.
    let owner_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM memberships WHERE company_id = $1 AND role = 'owner'",
    )
    .bind(company_id)
    .fetch_one(pool)
    .await?;
    let target_role: Option<(String,)> = sqlx::query_as(
        "SELECT role::text FROM memberships WHERE company_id = $1 AND user_id = $2",
    )
    .bind(company_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    if let Some((r,)) = &target_role {
        if r == "owner" && owner_count.0 <= 1 {
            return Err(crate::error::AppError::BadRequest(
                "cannot remove the last owner".into(),
            ));
        }
    }
    sqlx::query("DELETE FROM memberships WHERE company_id = $1 AND user_id = $2")
        .bind(company_id)
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn change_member_role(
    pool: &PgPool,
    company_id: Uuid,
    user_id: Uuid,
    new_role: &str,
) -> AppResult<()> {
    let new_role = match new_role {
        "owner" | "admin" | "accountant" | "viewer" => new_role,
        _ => return Err(crate::error::AppError::BadRequest("invalid role".into())),
    };
    // Prevent demoting last owner.
    if new_role != "owner" {
        let target_role: Option<(String,)> = sqlx::query_as(
            "SELECT role::text FROM memberships WHERE company_id = $1 AND user_id = $2",
        )
        .bind(company_id)
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        if let Some((r,)) = target_role {
            if r == "owner" {
                let owner_count: (i64,) = sqlx::query_as(
                    "SELECT COUNT(*) FROM memberships WHERE company_id = $1 AND role = 'owner'",
                )
                .bind(company_id)
                .fetch_one(pool)
                .await?;
                if owner_count.0 <= 1 {
                    return Err(crate::error::AppError::BadRequest(
                        "cannot demote the last owner".into(),
                    ));
                }
            }
        }
    }
    sqlx::query(&format!(
        "UPDATE memberships SET role = '{new_role}'::membership_role WHERE company_id = $1 AND user_id = $2"
    ))
    .bind(company_id)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct InvitationRow {
    pub id: Uuid,
    pub token: String,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub accepted_at: Option<chrono::DateTime<chrono::Utc>>,
}

pub async fn list_invitations(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<InvitationRow>> {
    let rows = sqlx::query(
        r#"
        SELECT id, token, role::text, created_at, expires_at, accepted_at
        FROM company_invitations
        WHERE company_id = $1
        ORDER BY created_at DESC
        LIMIT 50
        "#,
    )
    .bind(company_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| InvitationRow {
            id: r.get(0),
            token: r.get(1),
            role: r.get(2),
            created_at: r.get(3),
            expires_at: r.get(4),
            accepted_at: r.get(5),
        })
        .collect())
}

pub async fn create_invitation(
    pool: &PgPool,
    company_id: Uuid,
    invited_by: Uuid,
    role: &str,
    ttl_days: i64,
) -> AppResult<String> {
    use rand::RngCore;
    let role = match role {
        "owner" | "admin" | "accountant" | "viewer" => role,
        _ => return Err(crate::error::AppError::BadRequest("invalid role".into())),
    };
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);
    let expires_at = chrono::Utc::now() + chrono::Duration::days(ttl_days);
    sqlx::query(&format!(
        "INSERT INTO company_invitations (token, company_id, role, invited_by, expires_at) VALUES ($1, $2, '{role}'::membership_role, $3, $4)"
    ))
    .bind(&token)
    .bind(company_id)
    .bind(invited_by)
    .bind(expires_at)
    .execute(pool)
    .await?;
    Ok(token)
}

pub async fn accept_invitation(
    pool: &PgPool,
    token: &str,
    user_id: Uuid,
) -> AppResult<Uuid> {
    let row: Option<(Uuid, Uuid, String, chrono::DateTime<chrono::Utc>, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as(
            "SELECT id, company_id, role::text, expires_at, accepted_at FROM company_invitations WHERE token = $1",
        )
        .bind(token)
        .fetch_optional(pool)
        .await?;
    let (inv_id, company_id, role, expires_at, accepted_at) = row
        .ok_or_else(|| crate::error::AppError::NotFound)?;
    if accepted_at.is_some() {
        return Err(crate::error::AppError::Conflict("invitation already used".into()));
    }
    if chrono::Utc::now() > expires_at {
        return Err(crate::error::AppError::BadRequest("invitation expired".into()));
    }

    let mut tx = pool.begin().await?;
    sqlx::query(&format!(
        "INSERT INTO memberships (user_id, company_id, role) VALUES ($1, $2, '{role}'::membership_role) ON CONFLICT DO NOTHING"
    ))
    .bind(user_id)
    .bind(company_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE company_invitations SET accepted_at = now(), accepted_by = $1 WHERE id = $2")
        .bind(user_id)
        .bind(inv_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(company_id)
}

/// Add an existing user (by email) as a member of the company.
pub async fn add_member_by_email(
    pool: &PgPool,
    company_id: Uuid,
    email: &str,
    role: &str,
) -> AppResult<()> {
    let normalized = email.trim().to_lowercase();
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM auth_users WHERE email_normalized = $1",
    )
    .bind(&normalized)
    .fetch_optional(pool)
    .await?;
    let user_id = row.ok_or_else(|| {
        crate::error::AppError::BadRequest(format!("no user found with email {email}"))
    })?.0;
    let role_clause = match role {
        "owner" | "admin" | "accountant" | "viewer" => role,
        _ => return Err(crate::error::AppError::BadRequest("invalid role".into())),
    };
    // ON CONFLICT DO NOTHING in case already a member.
    sqlx::query(&format!(
        "INSERT INTO memberships (user_id, company_id, role) VALUES ($1, $2, '{role_clause}'::membership_role) ON CONFLICT DO NOTHING"
    ))
    .bind(user_id)
    .bind(company_id)
    .execute(pool)
    .await?;
    Ok(())
}

// ===========================================================================
// Invoicing
// ===========================================================================

#[derive(Debug, Clone)]
pub struct CustomerRow {
    pub id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub default_terms: String,
    pub is_active: bool,
}

pub async fn list_customers(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<CustomerRow>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query(
        "SELECT id, name, email, phone, city, state, default_terms, is_active
         FROM customers WHERE company_id = $1 ORDER BY name ASC",
    )
    .bind(company_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows
        .into_iter()
        .map(|r| CustomerRow {
            id: r.get(0),
            name: r.get(1),
            email: r.get(2),
            phone: r.get(3),
            city: r.get(4),
            state: r.get(5),
            default_terms: r.get(6),
            is_active: r.get(7),
        })
        .collect())
}

#[derive(Debug, Clone)]
pub struct CustomerDetail {
    pub id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: String,
    pub default_terms: String,
    pub notes: Option<String>,
}

pub async fn get_customer(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<Option<CustomerDetail>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let row = sqlx::query(
        "SELECT id, name, email, phone, address_line1, address_line2, city, state,
                postal_code, country, default_terms, notes
         FROM customers WHERE company_id = $1 AND id = $2",
    )
    .bind(company_id)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row.map(|r| CustomerDetail {
        id: r.get(0),
        name: r.get(1),
        email: r.get(2),
        phone: r.get(3),
        address_line1: r.get(4),
        address_line2: r.get(5),
        city: r.get(6),
        state: r.get(7),
        postal_code: r.get(8),
        country: r.get(9),
        default_terms: r.get(10),
        notes: r.get(11),
    }))
}

pub struct CreateCustomerInput {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address_line1: Option<String>,
    pub address_line2: Option<String>,
    pub city: Option<String>,
    pub state: Option<String>,
    pub postal_code: Option<String>,
    pub country: String,
    pub default_terms: String,
    pub notes: Option<String>,
}

// --- Company file store (with content de-duplication) ----------------------

pub struct CompanyFileRow {
    pub id: Uuid,
    pub category: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub uploaded_at: String,
    pub doc_year: Option<i32>,
    pub tags: Vec<String>,
    pub locked: bool,
}

/// Files ordered for the Documents view: newest period/tax year first, unknown
/// years last, then by category and recency within a year.
pub async fn list_company_files(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<CompanyFileRow>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query(
        "SELECT id, category, filename, content_type, size_bytes,
                to_char(uploaded_at, 'YYYY-MM-DD HH24:MI'), doc_year, tags, locked
         FROM company_files WHERE company_id = $1
         ORDER BY doc_year DESC NULLS LAST, category ASC, uploaded_at DESC",
    )
    .bind(company_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows
        .into_iter()
        .map(|r| CompanyFileRow {
            id: r.get(0),
            category: r.get(1),
            filename: r.get(2),
            content_type: r.get(3),
            size_bytes: r.get(4),
            uploaded_at: r.get(5),
            doc_year: r.get(6),
            tags: r.get(7),
            locked: r.get(8),
        })
        .collect())
}

/// Set or clear the lock on a document. Locked files cannot be deleted or have
/// their year changed (enforced in those queries' WHERE clauses).
pub async fn set_company_file_locked(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    locked: bool,
) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query("UPDATE company_files SET locked = $3 WHERE company_id = $1 AND id = $2")
        .bind(company_id)
        .bind(id)
        .bind(locked)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Set (or clear, with None) the period/tax year a document pertains to.
pub async fn update_company_file_year(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
    doc_year: Option<i32>,
) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query("UPDATE company_files SET doc_year = $3 WHERE company_id = $1 AND id = $2 AND locked = false")
        .bind(company_id)
        .bind(id)
        .bind(doc_year)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// True if a file with this content hash already exists for the company.
pub async fn company_file_exists(pool: &PgPool, company_id: Uuid, sha256: &str) -> AppResult<bool> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM company_files WHERE company_id = $1 AND sha256 = $2")
            .bind(company_id)
            .bind(sha256)
            .fetch_optional(&mut *tx)
            .await?;
    tx.commit().await?;
    Ok(row.is_some())
}

#[allow(clippy::too_many_arguments)]
/// Insert a file row, returning its id. On content duplicate (same company +
/// sha256) the existing row's id is returned and nothing is written.
#[allow(clippy::too_many_arguments)]
pub async fn insert_company_file(
    pool: &PgPool,
    company_id: Uuid,
    category: &str,
    filename: &str,
    content_type: &str,
    size_bytes: i64,
    sha256: &str,
    stored_path: &str,
    doc_year: Option<i32>,
) -> AppResult<Option<Uuid>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    // ON CONFLICT: a concurrent duplicate is silently ignored (dedup). Touch the
    // category on conflict so the existing row's id comes back via RETURNING.
    let row: Option<(Uuid,)> = sqlx::query_as(
        "INSERT INTO company_files
            (company_id, category, filename, content_type, size_bytes, sha256, stored_path, doc_year)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
         ON CONFLICT (company_id, sha256) DO UPDATE SET sha256 = EXCLUDED.sha256
         RETURNING id",
    )
    .bind(company_id)
    .bind(category)
    .bind(filename)
    .bind(content_type)
    .bind(size_bytes)
    .bind(sha256)
    .bind(stored_path)
    .bind(doc_year)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row.map(|(id,)| id))
}

/// Full file record, used when copying a file between entities.
pub struct StoredFileFull {
    pub category: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub sha256: String,
    pub stored_path: String,
    pub doc_year: Option<i32>,
}

pub async fn get_company_file_full(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
) -> AppResult<Option<StoredFileFull>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let row: Option<(String, String, String, i64, String, String, Option<i32>)> = sqlx::query_as(
        "SELECT category, filename, content_type, size_bytes, sha256, stored_path, doc_year
         FROM company_files WHERE company_id = $1 AND id = $2",
    )
    .bind(company_id)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row.map(
        |(category, filename, content_type, size_bytes, sha256, stored_path, doc_year)| {
            StoredFileFull {
                category, filename, content_type, size_bytes, sha256, stored_path, doc_year,
            }
        },
    ))
}

pub struct StoredFile {
    pub stored_path: String,
    pub content_type: String,
    pub filename: String,
}

pub async fn get_company_file(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
) -> AppResult<Option<StoredFile>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT stored_path, content_type, filename FROM company_files
         WHERE company_id = $1 AND id = $2",
    )
    .bind(company_id)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row.map(|(stored_path, content_type, filename)| StoredFile {
        stored_path,
        content_type,
        filename,
    }))
}

/// Delete a file row; returns its on-disk path so the caller can unlink it.
pub async fn delete_company_file(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
) -> AppResult<Option<String>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let row: Option<(String,)> = sqlx::query_as(
        "DELETE FROM company_files WHERE company_id = $1 AND id = $2 AND locked = false RETURNING stored_path",
    )
    .bind(company_id)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row.map(|(p,)| p))
}

// --- Address book (wallet/counterparty labels) -----------------------------

pub struct AddressLabelRow {
    pub id: Uuid,
    pub address: String,
    pub name: String,
    pub kind: String,
    pub account_code: String,
    pub note: String,
}

pub async fn list_address_labels(
    pool: &PgPool,
    company_id: Uuid,
    search: Option<&str>,
) -> AppResult<Vec<AddressLabelRow>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    // $2 NULL => no filter; otherwise case-insensitive match on name/address/kind/note.
    let rows = sqlx::query(
        "SELECT id, address, name, kind, account_code, note
         FROM address_labels
         WHERE company_id = $1
           AND ($2::text IS NULL
                OR name ILIKE '%' || $2 || '%'
                OR address ILIKE '%' || $2 || '%'
                OR kind ILIKE '%' || $2 || '%'
                OR note ILIKE '%' || $2 || '%')
         ORDER BY name ASC",
    )
    .bind(company_id)
    .bind(search)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows
        .into_iter()
        .map(|r| AddressLabelRow {
            id: r.get(0),
            address: r.get(1),
            name: r.get(2),
            kind: r.get(3),
            account_code: r.get(4),
            note: r.get(5),
        })
        .collect())
}

/// Insert or update a label (keyed on lowercased address, unique per company).
pub async fn upsert_address_label(
    pool: &PgPool,
    company_id: Uuid,
    address: &str,
    name: &str,
    kind: &str,
    account_code: &str,
    note: &str,
) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query(
        "INSERT INTO address_labels (company_id, address, name, kind, account_code, note)
         VALUES ($1, lower($2), $3, $4, $5, $6)
         ON CONFLICT (company_id, address)
         DO UPDATE SET name = EXCLUDED.name, kind = EXCLUDED.kind,
                       account_code = EXCLUDED.account_code, note = EXCLUDED.note",
    )
    .bind(company_id)
    .bind(address.trim())
    .bind(name.trim())
    .bind(kind.trim())
    .bind(account_code.trim())
    .bind(note.trim())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn delete_address_label(pool: &PgPool, company_id: Uuid, id: Uuid) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query("DELETE FROM address_labels WHERE company_id = $1 AND id = $2")
        .bind(company_id)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn create_customer(
    pool: &PgPool,
    company_id: Uuid,
    input: CreateCustomerInput,
) -> AppResult<Uuid> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO customers
            (id, company_id, name, email, phone, address_line1, address_line2,
             city, state, postal_code, country, default_terms, notes)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
    )
    .bind(id)
    .bind(company_id)
    .bind(&input.name)
    .bind(&input.email)
    .bind(&input.phone)
    .bind(&input.address_line1)
    .bind(&input.address_line2)
    .bind(&input.city)
    .bind(&input.state)
    .bind(&input.postal_code)
    .bind(&input.country)
    .bind(&input.default_terms)
    .bind(&input.notes)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(id)
}

#[derive(Debug, Clone)]
pub struct InvoiceRow {
    pub id: Uuid,
    pub invoice_number: String,
    pub customer_id: Uuid,
    pub customer_name: String,
    pub status: String,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub total_cents: i64,
    pub paid_cents: i64,
}

impl InvoiceRow {
    pub fn total_display(&self) -> String { format_cents(self.total_cents) }
    pub fn balance_cents(&self) -> i64 { self.total_cents - self.paid_cents }
    pub fn balance_display(&self) -> String { format_cents(self.balance_cents()) }
    pub fn status_badge(&self) -> &'static str {
        match self.status.as_str() {
            "paid" => "ok",
            "void" => "err",
            "overdue" => "err",
            "partial" => "warn",
            "sent" => "warn",
            _ => "",
        }
    }
    pub fn issue_us(&self) -> String { self.issue_date.format("%m/%d/%Y").to_string() }
    pub fn due_us(&self) -> String { self.due_date.format("%m/%d/%Y").to_string() }
}

pub async fn list_invoices(
    pool: &PgPool,
    company_id: Uuid,
    status_filter: Option<&str>,
) -> AppResult<Vec<InvoiceRow>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query(
        "SELECT i.id, i.invoice_number, i.customer_id, c.name,
                i.status::text, i.issue_date, i.due_date, i.total_cents, i.paid_cents
         FROM invoices i JOIN customers c ON c.id = i.customer_id
         WHERE i.company_id = $1
         AND ($2::text IS NULL OR i.status::text = $2)
         ORDER BY i.issue_date DESC, i.invoice_number DESC",
    )
    .bind(company_id)
    .bind(status_filter)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows.into_iter().map(|r| InvoiceRow {
        id: r.get(0), invoice_number: r.get(1), customer_id: r.get(2),
        customer_name: r.get(3), status: r.get(4), issue_date: r.get(5),
        due_date: r.get(6), total_cents: r.get(7), paid_cents: r.get(8),
    }).collect())
}

#[derive(Debug, Clone)]
pub struct InvoiceDetail {
    pub id: Uuid,
    pub invoice_number: String,
    pub status: String,
    pub issue_date: NaiveDate,
    pub due_date: NaiveDate,
    pub terms: String,
    pub currency: String,
    pub subtotal_cents: i64,
    pub tax_cents: i64,
    pub total_cents: i64,
    pub paid_cents: i64,
    pub memo: Option<String>,
    pub customer_notes: Option<String>,
    pub public_token: String,
    pub sent_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_sent_to: Option<String>,
    pub customer: CustomerDetail,
    pub lines: Vec<InvoiceLineRow>,
    pub payments: Vec<InvoicePaymentRow>,
}

impl InvoiceDetail {
    pub fn subtotal_display(&self) -> String { format_cents(self.subtotal_cents) }
    pub fn tax_display(&self) -> String { format_cents(self.tax_cents) }
    pub fn total_display(&self) -> String { format_cents(self.total_cents) }
    pub fn paid_display(&self) -> String { format_cents(self.paid_cents) }
    pub fn balance_cents(&self) -> i64 { self.total_cents - self.paid_cents }
    pub fn balance_display(&self) -> String { format_cents(self.balance_cents()) }
    pub fn issue_us(&self) -> String { self.issue_date.format("%m/%d/%Y").to_string() }
    pub fn due_us(&self) -> String { self.due_date.format("%m/%d/%Y").to_string() }
    pub fn is_draft(&self) -> bool { self.status == "draft" }
    pub fn is_void(&self) -> bool { self.status == "void" }
    pub fn is_paid(&self) -> bool { self.status == "paid" }
    pub fn status_badge(&self) -> &'static str {
        match self.status.as_str() {
            "paid" => "ok",
            "void" => "err",
            "partial" | "sent" => "warn",
            _ => "",
        }
    }
}

#[derive(Debug, Clone)]
pub struct InvoiceLineRow {
    pub description: String,
    pub quantity: rust_decimal::Decimal,
    pub unit_price_cents: i64,
    pub amount_cents: i64,
    pub tax_rate_pct: rust_decimal::Decimal,
    pub tax_cents: i64,
    pub revenue_account_id: Uuid,
    pub revenue_account_name: String,
}

impl InvoiceLineRow {
    pub fn unit_price_display(&self) -> String { format_cents(self.unit_price_cents) }
    pub fn amount_display(&self) -> String { format_cents(self.amount_cents) }
    pub fn tax_display(&self) -> String { format_cents(self.tax_cents) }
    pub fn quantity_display(&self) -> String {
        let s = self.quantity.to_string();
        if s.contains('.') {
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        } else { s }
    }
}

#[derive(Debug, Clone)]
pub struct InvoicePaymentRow {
    pub payment_date: NaiveDate,
    pub amount_cents: i64,
    pub method: String,
    pub reference: Option<String>,
    pub deposit_account_name: String,
}

impl InvoicePaymentRow {
    pub fn amount_display(&self) -> String { format_cents(self.amount_cents) }
    pub fn date_us(&self) -> String { self.payment_date.format("%m/%d/%Y").to_string() }
}

pub async fn get_invoice(
    pool: &PgPool,
    company_id: Uuid,
    id: Uuid,
) -> AppResult<Option<InvoiceDetail>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;

    let inv_row = sqlx::query(
        "SELECT i.id, i.invoice_number, i.status::text, i.issue_date, i.due_date,
                i.terms, i.currency, i.subtotal_cents, i.tax_cents, i.total_cents,
                i.paid_cents, i.memo, i.customer_notes, i.public_token,
                i.sent_at, i.last_sent_to,
                c.id, c.name, c.email, c.phone, c.address_line1, c.address_line2,
                c.city, c.state, c.postal_code, c.country, c.default_terms, c.notes
         FROM invoices i JOIN customers c ON c.id = i.customer_id
         WHERE i.company_id = $1 AND i.id = $2",
    )
    .bind(company_id)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(r) = inv_row else {
        tx.commit().await?;
        return Ok(None);
    };
    let customer = CustomerDetail {
        id: r.get(16),
        name: r.get(17),
        email: r.get(18),
        phone: r.get(19),
        address_line1: r.get(20),
        address_line2: r.get(21),
        city: r.get(22),
        state: r.get(23),
        postal_code: r.get(24),
        country: r.get(25),
        default_terms: r.get(26),
        notes: r.get(27),
    };

    let line_rows = sqlx::query(
        "SELECT il.description, il.quantity, il.unit_price_cents, il.amount_cents,
                il.tax_rate_pct, il.tax_cents, il.revenue_account_id, a.name
         FROM invoice_lines il JOIN accounts a ON a.id = il.revenue_account_id
         WHERE il.invoice_id = $1
         ORDER BY il.sort_order ASC",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    let lines = line_rows.into_iter().map(|l| InvoiceLineRow {
        description: l.get(0),
        quantity: l.get(1),
        unit_price_cents: l.get(2),
        amount_cents: l.get(3),
        tax_rate_pct: l.get(4),
        tax_cents: l.get(5),
        revenue_account_id: l.get(6),
        revenue_account_name: l.get(7),
    }).collect();

    let pay_rows = sqlx::query(
        "SELECT p.payment_date, p.amount_cents, p.method::text, p.reference, a.name
         FROM invoice_payments p JOIN accounts a ON a.id = p.deposit_account_id
         WHERE p.invoice_id = $1 ORDER BY p.payment_date DESC, p.created_at DESC",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    let payments = pay_rows.into_iter().map(|p| InvoicePaymentRow {
        payment_date: p.get(0),
        amount_cents: p.get(1),
        method: p.get(2),
        reference: p.get(3),
        deposit_account_name: p.get(4),
    }).collect();

    tx.commit().await?;

    Ok(Some(InvoiceDetail {
        id: r.get(0),
        invoice_number: r.get(1),
        status: r.get(2),
        issue_date: r.get(3),
        due_date: r.get(4),
        terms: r.get(5),
        currency: r.get(6),
        subtotal_cents: r.get(7),
        tax_cents: r.get(8),
        total_cents: r.get(9),
        paid_cents: r.get(10),
        memo: r.get(11),
        customer_notes: r.get(12),
        public_token: r.get(13),
        sent_at: r.get(14),
        last_sent_to: r.get(15),
        customer,
        lines,
        payments,
    }))
}

/// Resolve a public token to the invoice's id + company + company name.
/// Uses a transaction-scoped setting that the public_token policy reads.
pub async fn get_invoice_by_token(
    pool: &PgPool,
    token: &str,
) -> AppResult<Option<(Uuid, Uuid, String)>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    sqlx::query("SELECT set_config('app.invoice_public_token', $1, true)")
        .bind(token)
        .execute(&mut *tx)
        .await?;
    let row = sqlx::query(
        "SELECT i.id, i.company_id, c.name
         FROM invoices i JOIN companies c ON c.id = i.company_id
         WHERE i.public_token = $1",
    )
    .bind(token)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row.map(|r| (r.get::<Uuid, _>(0), r.get::<Uuid, _>(1), r.get::<String, _>(2))))
}

/// Re-fetch an invoice using the public token for RLS rather than tenant scope.
/// Used by the public preview page.
pub async fn get_invoice_public(
    pool: &PgPool,
    token: &str,
    invoice_id: Uuid,
) -> AppResult<Option<InvoiceDetail>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    sqlx::query("SELECT set_config('app.invoice_public_token', $1, true)")
        .bind(token)
        .execute(&mut *tx)
        .await?;

    let inv_row = sqlx::query(
        "SELECT i.id, i.invoice_number, i.status::text, i.issue_date, i.due_date,
                i.terms, i.currency, i.subtotal_cents, i.tax_cents, i.total_cents,
                i.paid_cents, i.memo, i.customer_notes, i.public_token,
                i.sent_at, i.last_sent_to,
                c.id, c.name, c.email, c.phone, c.address_line1, c.address_line2,
                c.city, c.state, c.postal_code, c.country, c.default_terms, c.notes
         FROM invoices i JOIN customers c ON c.id = i.customer_id
         WHERE i.id = $1 AND i.public_token = $2",
    )
    .bind(invoice_id)
    .bind(token)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(r) = inv_row else { tx.commit().await?; return Ok(None) };
    let customer = CustomerDetail {
        id: r.get(16), name: r.get(17), email: r.get(18), phone: r.get(19),
        address_line1: r.get(20), address_line2: r.get(21), city: r.get(22),
        state: r.get(23), postal_code: r.get(24), country: r.get(25),
        default_terms: r.get(26), notes: r.get(27),
    };
    let line_rows = sqlx::query(
        "SELECT description, quantity, unit_price_cents, amount_cents,
                tax_rate_pct, tax_cents, revenue_account_id
         FROM invoice_lines
         WHERE invoice_id = $1
         ORDER BY sort_order ASC",
    )
    .bind(invoice_id)
    .fetch_all(&mut *tx)
    .await?;
    let lines = line_rows.into_iter().map(|l| InvoiceLineRow {
        description: l.get(0), quantity: l.get(1), unit_price_cents: l.get(2),
        amount_cents: l.get(3), tax_rate_pct: l.get(4), tax_cents: l.get(5),
        revenue_account_id: l.get(6),
        revenue_account_name: String::new(),
    }).collect();
    let pay_rows = sqlx::query(
        "SELECT payment_date, amount_cents, method::text, reference
         FROM invoice_payments
         WHERE invoice_id = $1 ORDER BY payment_date DESC, created_at DESC",
    )
    .bind(invoice_id)
    .fetch_all(&mut *tx)
    .await?;
    let payments = pay_rows.into_iter().map(|p| InvoicePaymentRow {
        payment_date: p.get(0), amount_cents: p.get(1), method: p.get(2),
        reference: p.get(3),
        deposit_account_name: String::new(),
    }).collect();
    tx.commit().await?;
    Ok(Some(InvoiceDetail {
        id: r.get(0), invoice_number: r.get(1), status: r.get(2),
        issue_date: r.get(3), due_date: r.get(4), terms: r.get(5),
        currency: r.get(6), subtotal_cents: r.get(7), tax_cents: r.get(8),
        total_cents: r.get(9), paid_cents: r.get(10),
        memo: r.get(11), customer_notes: r.get(12), public_token: r.get(13),
        sent_at: r.get(14), last_sent_to: r.get(15),
        customer, lines, payments,
    }))
}

#[derive(Debug, Clone)]
pub struct DepositAccountOption {
    pub id: Uuid,
    pub display: String,
}

/// Asset accounts customers can deposit INTO (Cash/Bank), excluding A/R itself.
pub async fn list_deposit_accounts(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<DepositAccountOption>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query(
        "SELECT id, account_number, name FROM accounts
         WHERE company_id = $1
           AND account_type = 'asset'
           AND is_active = true
           AND account_number <> '1200'
         ORDER BY account_number ASC",
    )
    .bind(company_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows.into_iter().map(|r| DepositAccountOption {
        id: r.get(0),
        display: format!("{} — {}", r.get::<String, _>(1), r.get::<String, _>(2)),
    }).collect())
}

pub async fn list_revenue_accounts(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<DepositAccountOption>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let rows = sqlx::query(
        "SELECT id, account_number, name FROM accounts
         WHERE company_id = $1 AND account_type = 'revenue' AND is_active = true
         ORDER BY account_number ASC",
    )
    .bind(company_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows.into_iter().map(|r| DepositAccountOption {
        id: r.get(0),
        display: format!("{} — {}", r.get::<String, _>(1), r.get::<String, _>(2)),
    }).collect())
}
