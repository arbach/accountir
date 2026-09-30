//! Store an entity's refund / payment bank account (account number encrypted at rest).
//!   BANK_COMPANY=<uuid> BANK_NAME="Chase" BANK_ROUTING=... BANK_ACCOUNT=... [BANK_TYPE=checking|savings]
//! Requires DATABASE_URL and PLAID_TOKEN_ENC_KEY.
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL")?;
    let company: Uuid = std::env::var("BANK_COMPANY")?.parse()?;
    let name = std::env::var("BANK_NAME").unwrap_or_default();
    let routing = std::env::var("BANK_ROUTING")?;
    let account = std::env::var("BANK_ACCOUNT")?;
    let kind = std::env::var("BANK_TYPE").unwrap_or_else(|_| "checking".into());
    let pool = PgPoolOptions::new().max_connections(2).connect(&url).await?;
    accountir_cloud::tax::set_bank_account(&pool, company, &name, &routing, &account, &kind)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    let back = accountir_cloud::tax::get_bank_account(&pool, company)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .ok_or_else(|| anyhow::anyhow!("stored but could not read back"))?;
    println!(
        "stored for {company}: {} routing {} ****{} ({}) — round-trip decrypt OK",
        back.bank_name, back.routing, back.last4, back.account_type
    );
    Ok(())
}
