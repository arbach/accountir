//! Wise (TransferWise) integration: pull profiles, balances, and the
//! authoritative transfers list directly from the Wise API, cache them per
//! company, and reconcile. Read-only against the ledger — restatement of
//! journal entries is a separate, explicitly-confirmed step.

use serde_json::Value;
use sqlx::{Acquire, PgPool};
use uuid::Uuid;

use crate::error::AppResult;
use crate::store::event_store::set_tenant;

const WISE_BASE: &str = "https://api.transferwise.com";

pub struct WiseClient {
    token: String,
    http: reqwest::Client,
}

impl WiseClient {
    pub fn new(token: &str) -> Self {
        Self { token: token.to_string(), http: reqwest::Client::new() }
    }

    async fn get(&self, path: &str) -> Result<Value, String> {
        let resp = self
            .http
            .get(format!("{WISE_BASE}{path}"))
            .bearer_auth(&self.token)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(format!("Wise {status}: {}", body.chars().take(200).collect::<String>()));
        }
        serde_json::from_str(&body).map_err(|e| format!("bad JSON: {e}"))
    }

    pub async fn profiles(&self) -> Result<Value, String> {
        self.get("/v2/profiles").await
    }

    /// Resolve a recipient account id to its holder name (best-effort; recipient
    /// accounts owned by another profile return 403 → None).
    async fn account_name(&self, account_id: &Value) -> Option<String> {
        let id = account_id.as_i64()?;
        let v = self.get(&format!("/v1/accounts/{id}")).await.ok()?;
        v.get("accountHolderName").and_then(|n| n.as_str()).map(str::to_string)
    }

    /// All transfers for a profile within a date window (paginated).
    pub async fn transfers(&self, profile_id: &str, start: &str, end: &str) -> Result<Vec<Value>, String> {
        let mut out = Vec::new();
        let mut offset = 0;
        loop {
            let path = format!(
                "/v1/transfers?profile={profile_id}&limit=100&offset={offset}\
                 &createdDateStart={start}T00:00:00.000Z&createdDateEnd={end}T23:59:59.999Z"
            );
            let chunk = self.get(&path).await?;
            let arr = chunk.as_array().cloned().unwrap_or_default();
            let n = arr.len();
            out.extend(arr);
            if n < 100 {
                break;
            }
            offset += 100;
        }
        Ok(out)
    }
}

pub struct WiseConn {
    pub profile_id: String,
    pub label: String,
    pub token: String,
}

pub async fn get_connection(pool: &PgPool, company_id: Uuid) -> AppResult<Option<WiseConn>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT profile_id, label, api_token FROM wise_connections WHERE company_id = $1 LIMIT 1",
    )
    .bind(company_id)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row.map(|(profile_id, label, token)| WiseConn { profile_id, label, token }))
}

/// Pull transfers from Wise and cache them. Returns (count, external_total_cents).
pub async fn sync(pool: &PgPool, company_id: Uuid, year: i32) -> Result<(usize, i64), String> {
    let conn = get_connection(pool, company_id)
        .await
        .map_err(|e| format!("db: {e}"))?
        .ok_or("no Wise connection for this company")?;
    let client = WiseClient::new(&conn.token);
    let start = format!("{year}-01-01");
    let end = format!("{year}-12-31");
    let transfers = client.transfers(&conn.profile_id, &start, &end).await?;

    // resolve recipient names (cache per account id)
    let mut name_cache: std::collections::HashMap<i64, Option<String>> = std::collections::HashMap::new();
    let mut rows: Vec<(i64, String, String, i64, String, i64, String, String, bool)> = Vec::new();
    let mut external_total: i64 = 0;
    for t in &transfers {
        if t.get("status").and_then(|s| s.as_str()) != Some("outgoing_payment_sent") {
            continue;
        }
        let tid = t.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
        let src = ((t.get("sourceValue").and_then(|v| v.as_f64()).unwrap_or(0.0)) * 100.0).round() as i64;
        let tgt = ((t.get("targetValue").and_then(|v| v.as_f64()).unwrap_or(0.0)) * 100.0).round() as i64;
        let srcc = t.get("sourceCurrency").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let tgtc = t.get("targetCurrency").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let created = t.get("created").and_then(|v| v.as_str()).unwrap_or("").chars().take(10).collect::<String>();
        let acct = t.get("targetAccount").cloned().unwrap_or(Value::Null);
        let acct_id = acct.as_i64().unwrap_or(0);
        let name = if let Some(n) = name_cache.get(&acct_id) {
            n.clone()
        } else {
            let n = client.account_name(&acct).await;
            name_cache.insert(acct_id, n.clone());
            n
        };
        let recipient = name.unwrap_or_else(|| format!("acct {acct_id}"));
        let is_self = recipient.eq_ignore_ascii_case(&conn.label);
        if !is_self {
            external_total += src;
        }
        rows.push((tid, created, t.get("status").and_then(|s| s.as_str()).unwrap_or("").to_string(),
            src, srcc, tgt, tgtc, recipient, is_self));
    }

    // store (replace existing for idempotent re-sync)
    let mut dbc = pool.acquire().await.map_err(|e| e.to_string())?;
    let mut tx = dbc.begin().await.map_err(|e| e.to_string())?;
    set_tenant(&mut tx, company_id).await.map_err(|e| e.to_string())?;
    for (tid, created, status, src, srcc, tgt, tgtc, recipient, is_self) in &rows {
        sqlx::query(
            "INSERT INTO wise_transfers
               (company_id, profile_id, transfer_id, created, status, source_value,
                source_currency, target_value, target_currency, recipient, is_self_topup)
             VALUES ($1,$2,$3,NULLIF($4,'')::date,$5,$6,$7,$8,$9,$10,$11)
             ON CONFLICT (company_id, transfer_id) DO UPDATE SET
               status=EXCLUDED.status, source_value=EXCLUDED.source_value,
               recipient=EXCLUDED.recipient, is_self_topup=EXCLUDED.is_self_topup",
        )
        .bind(company_id).bind(&conn.profile_id).bind(tid).bind(created).bind(status)
        .bind(src).bind(srcc).bind(tgt).bind(tgtc).bind(recipient).bind(is_self)
        .execute(&mut *tx).await.map_err(|e| e.to_string())?;
    }
    sqlx::query("UPDATE wise_connections SET last_synced = now() WHERE company_id = $1")
        .bind(company_id).execute(&mut *tx).await.map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;
    Ok((rows.len(), external_total))
}

pub struct WisePayee {
    pub recipient: String,
    pub total_cents: i64,
    pub count: i64,
    pub currencies: String,
    pub is_self: bool,
}

/// Per-recipient reconciliation summary from the cached transfers.
pub async fn payees(pool: &PgPool, company_id: Uuid) -> AppResult<Vec<WisePayee>> {
    let mut conn = pool.acquire().await?;
    let mut tx = conn.begin().await?;
    set_tenant(&mut tx, company_id).await?;
    let rows: Vec<(String, i64, i64, String, bool)> = sqlx::query_as(
        "SELECT recipient, SUM(source_value)::bigint, COUNT(*)::bigint,
                string_agg(DISTINCT target_currency, '/'), bool_or(is_self_topup)
         FROM wise_transfers WHERE company_id = $1
         GROUP BY recipient ORDER BY SUM(source_value) DESC",
    )
    .bind(company_id)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows
        .into_iter()
        .map(|(recipient, total_cents, count, currencies, is_self)| WisePayee {
            recipient, total_cents, count, currencies, is_self,
        })
        .collect())
}
