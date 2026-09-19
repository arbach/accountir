//! Mail a signed tax form via the app's Lob path (`tax::mail_form`).
//!   MAIL_COMPANY=<uuid> MAIL_FORM=<uuid> MAIL_TO='{"name":...,"address_line1":...,...}'
//!   MAIL_CERTIFIED=1|0 (default 1). DATABASE_URL + LOB_API_KEY as usual.
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL")?;
    let company: Uuid = std::env::var("MAIL_COMPANY")?.parse()?;
    let form: Uuid = std::env::var("MAIL_FORM")?.parse()?;
    let to: serde_json::Value = serde_json::from_str(&std::env::var("MAIL_TO")?)?;
    let certified = std::env::var("MAIL_CERTIFIED").map(|v| v != "0").unwrap_or(true);
    let pool = PgPoolOptions::new().max_connections(2).connect(&url).await?;
    let letter = accountir_cloud::tax::mail_form(&pool, company, form, &to, certified)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!(
        "mailed: lob_id={} tracking={} expected_delivery={} carrier={}",
        letter.get("id").and_then(|v| v.as_str()).unwrap_or("?"),
        letter.get("tracking_number").map(|v| v.to_string()).unwrap_or_default(),
        letter.get("expected_delivery_date").and_then(|v| v.as_str()).unwrap_or("?"),
        letter.get("carrier").and_then(|v| v.as_str()).unwrap_or("?"),
    );
    Ok(())
}
