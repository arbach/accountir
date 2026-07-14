//! User-uploaded bank statement import: extract text (PDF or plain text/CSV),
//! AI-parse into transaction lines, and post them to the ledger against the
//! chosen account vs. Uncategorized — skipping lines that already exist there
//! (same date + amount), so re-uploading or overlapping statements merge
//! cleanly instead of duplicating.

use std::collections::HashMap;

use accountir_core::events::types::{Event, JournalEntrySource, JournalLineData};
use chrono::NaiveDate;
use sqlx::Acquire;
use uuid::Uuid;

use crate::commands::account::find_or_create_uncategorized;
use crate::error::AppResult;
use crate::plaid::statements::ParsedLine;
use crate::store::event_store::{append_event, set_tenant};

#[derive(Debug)]
pub struct UploadOutcome {
    pub parsed: usize,
    pub imported: u32,
    pub duplicates: u32,
    pub unparsed: u32,
}

/// Outcome of a deterministic, tie-verified statement import.
#[derive(Debug, serde::Serialize)]
pub struct ImportReport {
    pub parsed: usize,
    pub imported: u32,
    pub duplicates: u32,
    pub unparsed: u32,
    pub beginning_balance_cents: Option<i64>,
    pub ending_balance_cents: Option<i64>,
    pub lines_sum_cents: i64,
    /// beginning + Σ lines == ending, to the penny. None = balances not printed.
    pub parse_ties: Option<bool>,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
    pub posted: bool,
    pub note: String,
}

/// Deterministic per-line statement import: AI-parse the FULL statement
/// (balances + every line), verify beginning + Σ == ending TO THE PENNY, and
/// only then post one balanced entry per line against `account_id` vs
/// Uncategorized (dedup on date+amount). A parse that does not tie is refused
/// (unless `force`), so a skipped/mangled line can never silently enter the
/// books. This is the ingestion half of the bookkeeping loop; classification
/// of the Uncategorized legs happens afterwards, evidence-first.
pub async fn import_statement_verified(
    pool: &sqlx::PgPool,
    company_id: Uuid,
    user_id: Uuid,
    account_id: Uuid,
    file_name: &str,
    bytes: &[u8],
    force: bool,
) -> Result<ImportReport, String> {
    let text = if bytes.starts_with(b"%PDF") {
        crate::plaid::statements::extract_text_or_ocr(bytes).await?
    } else {
        String::from_utf8_lossy(bytes).to_string()
    };
    if text.trim().is_empty() {
        return Err("no text could be extracted from the file".to_string());
    }
    let stmt = crate::plaid::statements::parse_statement_with_ai(&text).await?;
    let parse_ties = stmt.ties();
    let lines_sum = stmt.lines_sum_cents();
    let mut report = ImportReport {
        parsed: stmt.transactions.len(),
        imported: 0,
        duplicates: 0,
        unparsed: 0,
        beginning_balance_cents: stmt.beginning_balance_cents,
        ending_balance_cents: stmt.ending_balance_cents,
        lines_sum_cents: lines_sum,
        parse_ties,
        period_start: stmt.period_start.clone(),
        period_end: stmt.period_end.clone(),
        posted: false,
        note: String::new(),
    };
    if parse_ties == Some(false) && !force {
        let delta = stmt.beginning_balance_cents.unwrap_or(0) + lines_sum
            - stmt.ending_balance_cents.unwrap_or(0);
        report.note = format!(
            "REFUSED to post: parse does not tie to the penny (beginning + Σlines − ending = {} cents \
             even after a corrective retry). Nothing was posted. Retry, or pass force=true only if the \
             statement itself is inconsistent.",
            delta
        );
        return Ok(report);
    }
    let (imported, duplicates, unparsed) =
        post_lines(pool, company_id, user_id, account_id, file_name, stmt.transactions)
            .await
            .map_err(|e| format!("posting failed: {e}"))?;
    report.imported = imported;
    report.duplicates = duplicates;
    report.unparsed = unparsed;
    report.posted = true;
    report.note = match parse_ties {
        Some(true) => "parse tied to the penny (beginning + Σ = ending); one balanced entry posted per new line".into(),
        Some(false) => "FORCED despite a failed tie — verify this account against the statement manually".into(),
        None => "statement balances not printed; lines posted without a tie check — verify the account balance".into(),
    };
    Ok(report)
}

pub async fn import_statement(
    pool: &sqlx::PgPool,
    company_id: Uuid,
    user_id: Uuid,
    account_id: Uuid,
    file_name: &str,
    bytes: &[u8],
) -> Result<UploadOutcome, String> {
    let text = if bytes.starts_with(b"%PDF") {
        crate::plaid::statements::extract_text_or_ocr(bytes).await?
    } else {
        String::from_utf8_lossy(bytes).to_string()
    };
    if text.trim().is_empty() {
        return Err("no text could be extracted from the file".to_string());
    }
    let lines = crate::plaid::statements::parse_with_ai(&text).await?;
    let parsed = lines.len();
    post_lines(pool, company_id, user_id, account_id, file_name, lines)
        .await
        .map(|(imported, duplicates, unparsed)| UploadOutcome { parsed, imported, duplicates, unparsed })
        .map_err(|e| format!("posting failed: {e}"))
}

/// Post parsed lines against `account_id`, treating an existing non-void line
/// on that account with the same date and amount as a duplicate. Counting is
/// multiset-style: two legitimate identical transactions on the statement only
/// dedupe against two existing ledger lines.
async fn post_lines(
    pool: &sqlx::PgPool,
    company_id: Uuid,
    user_id: Uuid,
    account_id: Uuid,
    file_name: &str,
    lines: Vec<ParsedLine>,
) -> AppResult<(u32, u32, u32)> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;

    // RLS scopes this lookup to the company, so it doubles as an ownership check.
    let acct: Option<(Option<String>,)> =
        sqlx::query_as("SELECT currency FROM accounts WHERE id = $1 AND is_active = true")
            .bind(account_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((currency,)) = acct else {
        return Err(crate::error::AppError::NotFound);
    };
    let currency = currency.unwrap_or_else(|| "USD".to_string());

    let existing: Vec<(NaiveDate, i64, i64)> = sqlx::query_as(
        r#"
        SELECT je.date, jl.amount, count(*)::bigint
        FROM journal_lines jl
        JOIN journal_entries je ON je.id = jl.entry_id
        WHERE jl.account_id = $1 AND je.is_void = false
        GROUP BY je.date, jl.amount
        "#,
    )
    .bind(account_id)
    .fetch_all(&mut *tx)
    .await?;
    let mut remaining: HashMap<(NaiveDate, i64), i64> =
        existing.into_iter().map(|(d, a, c)| ((d, a), c)).collect();

    let uncategorized_id = find_or_create_uncategorized(&mut tx, company_id, user_id).await?;
    let reference: String = format!("upload:{}", file_name.chars().take(60).collect::<String>());

    let mut imported = 0u32;
    let mut duplicates = 0u32;
    let mut unparsed = 0u32;
    for l in lines {
        let Ok(date) = NaiveDate::parse_from_str(&l.date, "%Y-%m-%d") else {
            unparsed += 1;
            continue;
        };
        if l.amount_cents == 0 {
            unparsed += 1;
            continue;
        }
        if let Some(c) = remaining.get_mut(&(date, l.amount_cents)) {
            if *c > 0 {
                *c -= 1;
                duplicates += 1;
                continue;
            }
        }
        let bank_line = JournalLineData {
            line_id: Uuid::new_v4().to_string(),
            account_id: account_id.to_string(),
            amount: l.amount_cents,
            currency: currency.clone(),
            exchange_rate: None,
            memo: None,
        };
        let counter_line = JournalLineData {
            line_id: Uuid::new_v4().to_string(),
            account_id: uncategorized_id.to_string(),
            amount: -l.amount_cents,
            currency: currency.clone(),
            exchange_rate: None,
            memo: None,
        };
        let event = Event::JournalEntryPosted {
            entry_id: Uuid::new_v4().to_string(),
            date,
            memo: l.description.clone(),
            lines: vec![bank_line, counter_line],
            reference: Some(reference.clone()),
            source: Some(JournalEntrySource::Import),
        };
        append_event(&mut tx, company_id, user_id, &event).await?;
        imported += 1;
    }
    tx.commit().await?;
    Ok((imported, duplicates, unparsed))
}
