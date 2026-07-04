use accountir_core::events::payload::compute_event_hash;
use accountir_core::events::types::Event;
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::store::projections;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("hash collision (event already exists)")]
    DuplicateHash,
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("payload error: {0}")]
    Payload(String),
}

impl From<StoreError> for AppError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::DuplicateHash => AppError::Conflict("duplicate event".into()),
            StoreError::Database(e) => AppError::Database(e),
            StoreError::Payload(m) => AppError::Internal(anyhow::anyhow!(m)),
        }
    }
}

/// Append an event for the current tenant inside an existing transaction.
/// Caller is responsible for setting `app.company_id` (RLS) before calling this.
/// Returns the new event's row id.
pub async fn append_event<'a>(
    tx: &mut Transaction<'a, Postgres>,
    company_id: Uuid,
    user_id: Uuid,
    event: &Event,
) -> Result<i64, StoreError> {
    // Policy: all accounting is in USD. Enforce it universally at the single
    // choke point every posting path funnels through (manual, invoice, Plaid,
    // statement upload, AI). There is no FX conversion by design, so a non-USD
    // line would silently corrupt every report total — reject it loudly instead.
    if let Event::JournalEntryPosted { lines, .. } = event {
        if let Some(bad) = lines.iter().find(|l| !l.currency.eq_ignore_ascii_case("USD")) {
            return Err(StoreError::Payload(format!(
                "only USD is supported; got line currency '{}'",
                bad.currency
            )));
        }
    }
    let timestamp: DateTime<Utc> = Utc::now();
    let timestamp_str = timestamp.to_rfc3339();
    let hash = compute_event_hash(event, &timestamp_str, &user_id.to_string())
        .map_err(|e| StoreError::Payload(e.to_string()))?;
    let payload = serde_json::to_value(event)
        .map_err(|e| StoreError::Payload(e.to_string()))?;
    let event_type = event.event_type();

    // Serialize appends PER COMPANY so concurrent writers can't both read the
    // same MAX(company_seq_id) and collide on insert. This matters now that the
    // owner's personal session can post into another entity (via the MCP
    // `entity` param) at the same moment that entity's own agent is posting:
    // without this lock, under READ COMMITTED both transactions compute the same
    // next_seq, the second hits the UNIQUE(company_id, company_seq_id) guard, and
    // its write is lost with a conflict error. A transaction-scoped advisory lock
    // keyed on the company makes the read-then-insert atomic per tenant while
    // still allowing full parallelism ACROSS companies. Auto-released at tx end.
    sqlx::query("SELECT pg_advisory_xact_lock(727, hashtext($1))")
        .bind(company_id.to_string())
        .execute(&mut **tx)
        .await?;

    // Per-tenant monotonic sequence. Safe under the advisory lock above:
    // UNIQUE(company_id, company_seq_id) remains the backstop guard.
    let next_seq: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(company_seq_id), 0) + 1 FROM events WHERE company_id = $1",
    )
    .bind(company_id)
    .fetch_one(&mut **tx)
    .await?;

    let row: Result<(i64,), sqlx::Error> = sqlx::query_as(
        r#"
        INSERT INTO events
            (company_id, company_seq_id, event_type, payload, hash, user_id, timestamp)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id
        "#,
    )
    .bind(company_id)
    .bind(next_seq)
    .bind(event_type)
    .bind(payload)
    .bind(&hash[..])
    .bind(user_id)
    .bind(timestamp)
    .fetch_one(&mut **tx)
    .await;

    let event_id = match row {
        Ok((id,)) => id,
        Err(sqlx::Error::Database(db_err)) if db_err.code().as_deref() == Some("23505") => {
            return Err(StoreError::DuplicateHash);
        }
        Err(e) => return Err(e.into()),
    };

    projections::apply(tx, company_id, event_id, event).await?;

    Ok(event_id)
}

/// Set the per-tx tenant scoping that RLS reads from.
pub async fn set_tenant<'a>(
    tx: &mut Transaction<'a, Postgres>,
    company_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query("SELECT set_config('app.company_id', $1, true)")
        .bind(company_id.to_string())
        .execute(&mut **tx)
        .await?;
    Ok(())
}
