//! Sign an approved tax form via the app path (`tax::sign_form`), using the
//! owner's stored signature image.
//!   SIGN_COMPANY=<uuid> SIGN_FORM=<uuid> SIGN_USER=<uuid> [SIGN_NAME=<signer>]
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL")?;
    let company: Uuid = std::env::var("SIGN_COMPANY")?.parse()?;
    let form: Uuid = std::env::var("SIGN_FORM")?.parse()?;
    let user: Uuid = std::env::var("SIGN_USER")?.parse()?;
    let signer = std::env::var("SIGN_NAME").unwrap_or_else(|_| "Owner".into());
    let pool = PgPoolOptions::new().max_connections(2).connect(&url).await?;
    let (png, _ct) = accountir_cloud::signature::get_image(&pool, user)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .ok_or_else(|| anyhow::anyhow!("no signature on file for user"))?;
    accountir_cloud::tax::sign_form(&pool, company, form, &png, &signer)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("signed {form}");
    Ok(())
}
