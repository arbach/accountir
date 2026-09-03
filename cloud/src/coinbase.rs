//! Coinbase integration (company's own account): pull accounts + transactions
//! via the CDP API, cache them per company, and summarize for reconciliation.
//! Read-only against the ledger — restatement of journal entries goes through
//! the normal event-sourced entry flow, like the Wise integration.
//!
//! Auth: CDP API key = key name + Ed25519 private key (base64, 64 bytes:
//! seed||public). Each request gets a short-lived EdDSA JWT whose `uri` claim
//! binds it to the method + path.

use base64::Engine;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde_json::{json, Value};
use sqlx::{Acquire, PgPool};
use uuid::Uuid;

use crate::error::AppResult;
use crate::store::event_store::set_tenant;

const CB_HOST: &str = "api.coinbase.com";

/// PKCS#8 v1 DER prefix for an Ed25519 private key; append the 32-byte seed.
const ED25519_PKCS8_PREFIX: [u8; 16] = [
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x04, 0x22, 0x04,
    0x20,
];

pub struct CoinbaseClient {
    key_name: String,
    enc_key: EncodingKey,
    http: reqwest::Client,
}

impl CoinbaseClient {
    pub fn new(key_name: &str, private_key_b64: &str) -> Result<Self, String> {
        let raw = base64::engine::general_purpose::STANDARD
            .decode(private_key_b64.trim())
            .map_err(|e| format!("bad private key base64: {e}"))?;
        if raw.len() < 32 {
            return Err("private key too short".into());
        }
        let mut pkcs8 = Vec::with_capacity(48);
        pkcs8.extend_from_slice(&ED25519_PKCS8_PREFIX);
        pkcs8.extend_from_slice(&raw[..32]);
        let enc_key = EncodingKey::from_ed_der(&pkcs8);
        Ok(Self {
            key_name: key_name.trim().to_string(),
            enc_key,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
        })
    }

    fn jwt(&self, method: &str, path: &str) -> Result<String, String> {
        let path_no_query = path.split('?').next().unwrap_or(path);
        let now = chrono::Utc::now().timestamp();
        let claims = json!({
            "iss": "cdp",
            "sub": self.key_name,
            "nbf": now,
            "exp": now + 120,
            "uri": format!("{method} {CB_HOST}{path_no_query}"),
        });
        let mut header = Header::new(Algorithm::EdDSA);
        header.kid = Some(self.key_name.clone());
        encode(&header, &claims, &self.enc_key).map_err(|e| format!("jwt: {e}"))
    }

    async fn get(&self, path: &str) -> Result<Value, String> {
        let token = self.jwt("GET", path)?;
        let resp = self
            .http
            .get(format!("https://{CB_HOST}{path}"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!(
                "Coinbase {status}: {}",
                body.chars().take(200).collect::<String>()
            ));
        }
        serde_json::from_str(&body).map_err(|e| format!("bad JSON: {e}"))
    }

    /// Follow v2 pagination (`pagination.next_uri`) until exhausted.
    async fn get_all(&self, first_path: &str) -> Result<Vec<Value>, String> {
        let mut out = Vec::new();
        let mut path = Some(first_path.to_string());
        while let Some(p) = path {
            let page = self.get(&p).await?;
            if let Some(arr) = page.get("data").and_then(|d| d.as_array()) {
                out.extend(arr.iter().cloned());
            }
            path = page
                .get("pagination")
                .and_then(|pg| pg.get("next_uri"))
                .and_then(|u| u.as_str())
                .map(str::to_string);
        }
        Ok(out)
    }

    pub async fn accounts(&self) -> Result<Vec<Value>, String> {
        self.get_all("/v2/accounts?limit=100").await
    }

    pub async fn transactions(&self, account_id: &str) -> Result<Vec<Value>, String> {
        self.get_all(&format!("/v2/accounts/{account_id}/transactions?limit=100"))
            .await
    }
}

pub struct CoinbaseConn {
    pub key_name: String,
    pub private_key_b64: String,
    pub label: String,
}

pub async fn get_connection(pool: &PgPool, company_id: Uuid) -> AppResult<Option<CoinbaseConn>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT key_name, private_key_b64, label FROM coinbase_connections WHERE company_id = $1",
    )
    .bind(company_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row.map(|(key_name, private_key_b64, label)| CoinbaseConn {
        key_name,
        private_key_b64,
        label,
    }))
}

pub async fn set_connection(
    pool: &PgPool,
    company_id: Uuid,
    key_name: &str,
    private_key_b64: &str,
    label: &str,
) -> AppResult<()> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    sqlx::query(
        "INSERT INTO coinbase_connections (company_id, key_name, private_key_b64, label)
         VALUES ($1,$2,$3,$4)
         ON CONFLICT (company_id) DO UPDATE SET
           key_name=EXCLUDED.key_name, private_key_b64=EXCLUDED.private_key_b64, label=EXCLUDED.label",
    )
    .bind(company_id)
    .bind(key_name)
    .bind(private_key_b64)
    .bind(label)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Pull all accounts + transactions and upsert the cache.
/// Returns (accounts, transactions) counts.
pub async fn sync(pool: &PgPool, company_id: Uuid) -> Result<(usize, usize), String> {
    let conn = get_connection(pool, company_id)
        .await
        .map_err(|e| format!("db: {e}"))?
        .ok_or("no Coinbase connection for this company")?;
    let client = CoinbaseClient::new(&conn.key_name, &conn.private_key_b64)?;

    let accounts = client.accounts().await?;
    let mut rows = 0usize;
    let mut dbc = pool.acquire().await.map_err(|e| e.to_string())?;
    let mut tx = dbc.begin().await.map_err(|e| e.to_string())?;
    set_tenant(&mut tx, company_id).await.map_err(|e| e.to_string())?;

    for a in &accounts {
        let aid = a.get("id").and_then(|v| v.as_str()).unwrap_or_default();
        let aname = a.get("name").and_then(|v| v.as_str()).unwrap_or_default();
        if aid.is_empty() {
            continue;
        }
        let txs = client.transactions(aid).await?;
        for t in txs {
            let tid = t.get("id").and_then(|v| v.as_str()).unwrap_or_default();
            if tid.is_empty() {
                continue;
            }
            let amount = t
                .pointer("/amount/amount")
                .and_then(|v| v.as_str())
                .unwrap_or("0");
            let currency = t
                .pointer("/amount/currency")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let native: f64 = t
                .pointer("/native_amount/amount")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.0);
            let counterparty = t
                .pointer("/to/address")
                .or_else(|| t.pointer("/to/resource"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let hash = t
                .pointer("/network/hash")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            sqlx::query(
                "INSERT INTO coinbase_transactions
                   (company_id, tx_id, account_id, account_name, created_at, tx_type,
                    amount, currency, native_cents, status, counterparty, network_hash, raw)
                 VALUES ($1,$2,$3,$4,$5::timestamptz,$6,$7::numeric,$8,$9,$10,$11,$12,$13)
                 ON CONFLICT (company_id, tx_id) DO UPDATE SET
                   status=EXCLUDED.status, amount=EXCLUDED.amount,
                   native_cents=EXCLUDED.native_cents, raw=EXCLUDED.raw",
            )
            .bind(company_id)
            .bind(tid)
            .bind(aid)
            .bind(aname)
            .bind(t.get("created_at").and_then(|v| v.as_str()))
            .bind(t.get("type").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(amount)
            .bind(currency)
            .bind((native * 100.0).round() as i64)
            .bind(t.get("status").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(counterparty)
            .bind(hash)
            .bind(&t)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            rows += 1;
        }
    }
    sqlx::query("UPDATE coinbase_connections SET last_synced = now() WHERE company_id = $1")
        .bind(company_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok((accounts.len(), rows))
}
