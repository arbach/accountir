//! Mutations to existing entries: void, unvoid, line reassign.

use accountir_core::events::types::Event;
use sqlx::{Acquire, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::store::event_store::{append_event, set_tenant};

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
