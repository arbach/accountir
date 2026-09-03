//! Coinbase maintenance CLI: store a company's CDP API key and run a sync.
//!   Store: CB_ACTION=store CB_COMPANY=<uuid> CB_KEY_NAME=… CB_PRIVATE_KEY_B64=… [CB_LABEL=…]
//!   Sync:  CB_ACTION=sync  CB_COMPANY=<uuid>
//! DATABASE_URL as usual. Keys typically come from `pass coinbase/<company>/…`.
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = std::env::var("DATABASE_URL")?;
    let company: Uuid = std::env::var("CB_COMPANY")?.parse()?;
    let action = std::env::var("CB_ACTION").unwrap_or_else(|_| "sync".into());
    let pool = PgPoolOptions::new().max_connections(2).connect(&url).await?;

    match action.as_str() {
        "store" => {
            let key_name = std::env::var("CB_KEY_NAME")?;
            let key_b64 = std::env::var("CB_PRIVATE_KEY_B64")?;
            let label = std::env::var("CB_LABEL").unwrap_or_default();
            // Validate the key against the live API before storing.
            let client = accountir_cloud::coinbase::CoinbaseClient::new(&key_name, &key_b64)
                .map_err(|e| anyhow::anyhow!(e))?;
            let n = client.accounts().await.map_err(|e| anyhow::anyhow!(e))?.len();
            accountir_cloud::coinbase::set_connection(&pool, company, &key_name, &key_b64, &label)
                .await?;
            println!("stored Coinbase connection for {company} (key validated: {n} accounts visible)");
        }
        "sync" => {
            let (accounts, txs) = accountir_cloud::coinbase::sync(&pool, company)
                .await
                .map_err(|e| anyhow::anyhow!(e))?;
            println!("synced {accounts} accounts, {txs} transactions for {company}");
        }
        other => anyhow::bail!("unknown CB_ACTION '{other}' (store|sync)"),
    }
    Ok(())
}
