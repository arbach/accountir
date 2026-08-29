//! Mutations to existing entries: void, unvoid, line reassign.

use accountir_core::events::types::Event;
use accountir_core::events::validation::validate_event;
use sqlx::{Acquire, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::store::event_store::{append_event, set_tenant};

/// Guard for the PUBLIC void/unvoid/reassign flows (web UI, AI tools, maint):
/// the entry must exist, be in the expected void state (so we never append
/// phantom no-op events), and must not be the GL side of an invoice or an
/// invoice payment — those must go through the invoice's own flow so the
/// subledger (status, paid_cents) stays in sync with the ledger.
///
/// NOT called by `void_entry_in_tx`: the invoice void flow uses that directly
/// and keeps its subledger consistent itself.
async fn guard_entry_mutation<'a>(
    tx: &mut Transaction<'a, Postgres>,
    entry_id: Uuid,
    expect_void: Option<bool>,
) -> AppResult<()> {
    let row: Option<(bool,)> =
        sqlx::query_as("SELECT is_void FROM journal_entries WHERE id = $1")
            .bind(entry_id)
            .fetch_optional(&mut **tx)
            .await?;
    let (is_void,) = row.ok_or(AppError::NotFound)?;
    if let Some(expected) = expect_void {
        if is_void != expected {
            return Err(AppError::Conflict(format!(
                "entry is {}",
                if is_void { "already void" } else { "not void" }
            )));
        }
    }
    let inv: Option<(String,)> = sqlx::query_as(
        "SELECT invoice_number FROM invoices WHERE posted_entry_id = $1",
    )
    .bind(entry_id)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some((number,)) = inv {
        return Err(AppError::Conflict(format!(
            "this entry posts invoice {number}; use the invoice's void flow so its status stays in sync"
        )));
    }
    let pay: Option<(String,)> = sqlx::query_as(
        "SELECT i.invoice_number FROM invoice_payments p
         JOIN invoices i ON i.id = p.invoice_id WHERE p.entry_id = $1",
    )
    .bind(entry_id)
    .fetch_optional(&mut **tx)
    .await?;
    if let Some((number,)) = pay {
        return Err(AppError::Conflict(format!(
            "this entry records a payment on invoice {number}; manage it from the invoice so paid amounts stay in sync"
        )));
    }
    Ok(())
}

pub async fn void_entry(
    pool: &PgPool,
    company_id: Uuid,
    user_id: Uuid,
    entry_id: Uuid,
    reason: String,
) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    guard_entry_mutation(&mut tx, entry_id, Some(false)).await?;
    void_entry_in_tx(&mut tx, company_id, user_id, entry_id, reason).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn void_entry_in_tx<'a>(
    tx: &mut Transaction<'a, Postgres>,
    company_id: Uuid,
    user_id: Uuid,
    entry_id: Uuid,
    reason: String,
) -> AppResult<()> {
    let event = Event::JournalEntryVoided {
        entry_id: entry_id.to_string(),
        reason,
    };
    append_event(tx, company_id, user_id, &event).await?;
    Ok(())
}

pub async fn unvoid_entry(
    pool: &PgPool,
    company_id: Uuid,
    user_id: Uuid,
    entry_id: Uuid,
    reason: String,
) -> AppResult<()> {
    let event = Event::JournalEntryUnvoided {
        entry_id: entry_id.to_string(),
        reason,
    };
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    guard_entry_mutation(&mut tx, entry_id, Some(true)).await?;
    append_event(&mut tx, company_id, user_id, &event).await?;
    tx.commit().await?;
    Ok(())
}

/// Correct an entry's memo (e.g. relabel a reclassified transaction). Emits
/// JournalEntryMemoUpdated; the original source text stays in entry_sources.
pub async fn update_entry_memo(
    pool: &PgPool,
    company_id: Uuid,
    user_id: Uuid,
    entry_id: Uuid,
    new_memo: &str,
) -> AppResult<()> {
    let new_memo = new_memo.trim();
    if new_memo.is_empty() {
        return Err(AppError::BadRequest("memo cannot be empty".into()));
    }
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let old: Option<(Option<String>,)> =
        sqlx::query_as("SELECT memo FROM journal_entries WHERE id = $1")
            .bind(entry_id)
            .fetch_optional(&mut *tx)
            .await?;
    let old_memo = old.ok_or(AppError::NotFound)?.0.unwrap_or_default();
    if old_memo == new_memo {
        tx.commit().await?;
        return Ok(());
    }
    let event = Event::JournalEntryMemoUpdated {
        entry_id: entry_id.to_string(),
        old_memo,
        new_memo: new_memo.to_string(),
    };
    validate_event(&event).map_err(|e| AppError::BadRequest(format!("invalid memo: {e}")))?;
    append_event(&mut tx, company_id, user_id, &event).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn reassign_line(
    pool: &PgPool,
    company_id: Uuid,
    user_id: Uuid,
    line_id: Uuid,
    new_account_id: Uuid,
) -> AppResult<()> {
    // Look up old account + entry for the event payload.
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT entry_id, account_id FROM journal_lines WHERE id = $1",
    )
    .bind(line_id)
    .fetch_optional(&mut *tx)
    .await?;
    let (entry_id, old_account_id) = row.ok_or(AppError::NotFound)?;
    if old_account_id == new_account_id {
        tx.commit().await?;
        return Ok(());
    }
    // Invoice-linked entries must be corrected through the invoice flow —
    // repointing their AR/revenue legs silently de-syncs the invoice subledger.
    guard_entry_mutation(&mut tx, entry_id, None).await?;
    // Guard: the target account must exist in THIS company. RLS scopes this
    // SELECT to the tenant, so a foreign/unknown account id is rejected. Without
    // it, the line could be repointed at an account invisible to every report —
    // the line would silently vanish and the entry would no longer balance.
    let target: Option<(Uuid,)> = sqlx::query_as(
        "SELECT id FROM accounts WHERE id = $1 AND is_active = true",
    )
    .bind(new_account_id)
    .fetch_optional(&mut *tx)
    .await?;
    if target.is_none() {
        return Err(AppError::BadRequest(
            "unknown or inactive target account".into(),
        ));
    }
    let event = Event::JournalLineReassigned {
        entry_id: entry_id.to_string(),
        line_id: line_id.to_string(),
        old_account_id: old_account_id.to_string(),
        new_account_id: new_account_id.to_string(),
    };
    append_event(&mut tx, company_id, user_id, &event).await?;
    tx.commit().await?;
    Ok(())
}
