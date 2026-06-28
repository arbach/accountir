//! One-off maintenance: void a journal entry through the proper event-sourced path
//! (appends a JournalEntryVoided event + projects is_void=true). Parameterized by env:
//!   DATABASE_URL, VOID_COMPANY, VOID_USER, VOID_ENTRY, [VOID_REASON]
//! Run: VOID_COMPANY=… VOID_USER=… VOID_ENTRY=… cargo run --release --bin maint_void
use accountir_cloud::commands::mutations;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL")?;
    let company: Uuid = std::env::var("VOID_COMPANY")?.parse()?;
    let user: Uuid = std::env::var("VOID_USER")?.parse()?;
    let entry: Uuid = std::env::var("VOID_ENTRY")?.parse()?;
    let reason = std::env::var("VOID_REASON")
        .unwrap_or_else(|_| "duplicate crypto entry (same on-chain tx booked twice)".into());

    let pool = PgPoolOptions::new().max_connections(2).connect(&url).await?;
    mutations::void_entry(&pool, company, user, entry, reason).await?;
    println!("voided entry {entry} in company {company}");
    Ok(())
}
